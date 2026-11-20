//! Raw-byte Virtual QCY Earbuds Simulator implementing [`qcy_transport::DeviceScanner`]
//! and [`qcy_transport::BluetoothConnection`] at the real Bluetooth wire protocol level.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use qcy_models::{DeviceModelProfile, ModelRegistry};
use qcy_protocol::rcsp::RcspStreamAccumulator;
use qcy_protocol::tlv::encode_single_tlv;
use qcy_protocol::{
    CmdId, FitTestResult, OtaTransport, OtaType, ProtocolFamily, CHAR_BES_OTA,
    CHAR_BLUETRUM_FOTA_NOTIFY, CHAR_BLUETRUM_FOTA_WRITE, CHAR_JIELI_NOTIFY, CHAR_JIELI_WRITE,
    CHAR_LEGACY_BATTERY, CHAR_LEGACY_BUTTON, CHAR_LEGACY_EQ, CHAR_LEGACY_VERSION, CHAR_QCY_NOTIFY,
    CHAR_QCY_WRITE, CHAR_WUQI_OTA_NOTIFY, CHAR_WUQI_OTA_WRITE, COMPANY_ID_QCY, DEFAULT_BLE_MTU,
    SVC_BES_OTA, SVC_BLUETRUM_FOTA, SVC_JIELI_RCSP, SVC_QCY_CUSTOM, SVC_WUQI_OTA,
};
use qcy_transport::{
    now_epoch_ms, BluetoothAddress, BluetoothConnection, ConnectionEvent, DeviceScanner,
    DiscoveredDevice, GattCharacteristicInfo, GattProperties, GattServiceInfo, TransportError,
    TransportKind, TransportResult,
};
use tokio::sync::{broadcast, mpsc};
use uuid::Uuid;

pub mod state;
pub mod wire;

pub use state::{VirtualDeviceState, BUILTIN_VIRTUAL_MODELS};

/// Interactive raw-byte virtual QCY scanner & hardware controller.
#[derive(Debug, Clone)]
pub struct VirtualScanner {
    state: Arc<Mutex<VirtualDeviceState>>,
    events_tx: broadcast::Sender<ConnectionEvent>,
    notify_channels: Arc<Mutex<HashMap<Uuid, mpsc::Sender<Vec<u8>>>>>,
}

impl VirtualScanner {
    pub fn new(state: VirtualDeviceState) -> Self {
        let (events_tx, _) = broadcast::channel(64);
        Self {
            state: Arc::new(Mutex::new(state)),
            events_tx,
            notify_channels: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn from_profile(profile: &DeviceModelProfile) -> Self {
        Self::new(VirtualDeviceState::from_profile(profile))
    }

    /// Lists the 6 built-in virtual QCY hardware model IDs spanning all form factors & protocols.
    pub fn builtin_virtual_models() -> &'static [&'static str] {
        &BUILTIN_VIRTUAL_MODELS
    }

    /// Returns a cloned snapshot of the live virtual device hardware state.
    pub fn snapshot(&self) -> VirtualDeviceState {
        self.state.lock().expect("simulator lock").clone()
    }

    /// Switches the active virtual hardware profile (e.g. `"qcy-n70"`, `"qcy-ht08-jl"`, `"qcy-h3"`, `"qcy-sp75"`).
    pub fn switch_model(&self, model_id: &str) -> Result<VirtualDeviceState, String> {
        let next = VirtualDeviceState::for_model(model_id)?;
        {
            let mut guard = self.state.lock().expect("simulator lock");
            *guard = next.clone();
        }
        let _ = self.events_tx.send(ConnectionEvent::AdvertisementSeen(
            self.build_discovered(false),
        ));
        Ok(next)
    }

    /// Updates virtual L/R/Case battery levels, emits an updated BLE advertisement,
    /// and pushes a raw battery notification if a connection is subscribed.
    pub fn set_battery(&self, left: u8, right: u8, case: u8) {
        let (is_new_jl, family, batt_bytes) = {
            let mut st = self.state.lock().expect("simulator lock");
            st.left_battery = left.min(100);
            st.right_battery = right.min(100);
            st.case_battery = case.min(100);
            (
                st.protocol_family == ProtocolFamily::JieLiRcspNew,
                st.protocol_family.clone(),
                st.battery_wire_bytes(),
            )
        };
        let _ = self.events_tx.send(ConnectionEvent::AdvertisementSeen(
            self.build_discovered(false),
        ));
        self.push_battery_notification(&family, is_new_jl, batt_bytes);
    }

    /// Updates virtual L/R/Case charging flags, emitting BLE advertisement + raw notification.
    pub fn set_charging(&self, left: bool, right: bool, case: bool) {
        let (is_new_jl, family, batt_bytes) = {
            let mut st = self.state.lock().expect("simulator lock");
            st.left_charging = left;
            st.right_charging = right;
            st.case_charging = case;
            (
                st.protocol_family == ProtocolFamily::JieLiRcspNew,
                st.protocol_family.clone(),
                st.battery_wire_bytes(),
            )
        };
        let _ = self.events_tx.send(ConnectionEvent::AdvertisementSeen(
            self.build_discovered(false),
        ));
        self.push_battery_notification(&family, is_new_jl, batt_bytes);
    }

    /// Updates Left/Right in-ear wearing sensor state and pushes a raw `0x62` wear notification.
    pub fn set_in_ear(&self, left: bool, right: bool) {
        let (family, mask) = {
            let mut st = self.state.lock().expect("simulator lock");
            st.in_ear_left = left;
            st.in_ear_right = right;
            (st.protocol_family.clone(), st.wear_status_mask())
        };
        if let Ok(tlv) = encode_single_tlv(CmdId::WearStatus.as_u8(), &mask.to_le_bytes()) {
            self.push_tlv_or_wrapped(&family, tlv);
        }
    }

    /// Triggers a simulated charging-case lid-open event: increments `pop_num`, sets
    /// `lid_open = true`, and broadcasts a real 24B `QcyAdvertisement` with `is_pop = true`.
    pub fn trigger_lid_open_popup(&self) -> DiscoveredDevice {
        {
            let mut st = self.state.lock().expect("simulator lock");
            st.lid_open = true;
            st.pop_num = st.pop_num.wrapping_add(1);
        }
        let dev = self.build_discovered(true);
        let _ = self
            .events_tx
            .send(ConnectionEvent::AdvertisementSeen(dev.clone()));
        dev
    }

    /// Simulates a sudden link drop and emits `ConnectionEvent::Disconnected`.
    pub fn simulate_disconnect(&self, reason: &str) {
        let addr = {
            let mut st = self.state.lock().expect("simulator lock");
            st.connected = false;
            BluetoothAddress::from_bytes(st.class_mac)
        };
        let _ = self.events_tx.send(ConnectionEvent::Disconnected {
            address: addr,
            reason: reason.to_string(),
        });
    }

    /// Sets and broadcasts a simulated Ear-Tip Fit Test (`0x11`) completion result.
    pub fn simulate_fit_test_result(&self, left_result: u8, right_result: u8) {
        let family = {
            let mut st = self.state.lock().expect("simulator lock");
            st.fit_test_result = FitTestResult {
                status: 2,
                left_result,
                right_result,
            };
            st.protocol_family.clone()
        };
        if let Ok(tlv) = encode_single_tlv(CmdId::FitTest.as_u8(), &[2, left_result, right_result])
        {
            self.push_tlv_or_wrapped(&family, tlv);
        }
    }

    /// Configures artificial per-write async response latency (ms) for realism testing.
    pub fn set_latency_ms(&self, latency_ms: u64) {
        self.state.lock().expect("simulator lock").latency_ms = latency_ms;
    }

    /// Opens a raw-byte [`VirtualConnection`] bound to this scanner's live hardware state.
    pub fn open_connection(&self) -> VirtualConnection {
        let mut st = self.state.lock().expect("simulator lock");
        st.connected = true;
        let addr = BluetoothAddress::from_bytes(st.class_mac);
        drop(st);
        let _ = self
            .events_tx
            .send(ConnectionEvent::Connected { address: addr });
        VirtualConnection {
            address: addr,
            state: Arc::clone(&self.state),
            connected: AtomicBool::new(true),
            notify_channels: Arc::clone(&self.notify_channels),
            rfcomm_tx: Mutex::new(None),
            rcsp_acc: Mutex::new(RcspStreamAccumulator::new()),
        }
    }

    fn push_battery_notification(&self, family: &ProtocolFamily, is_new_jl: bool, batt: [u8; 3]) {
        match family {
            ProtocolFamily::QcyUnifiedTlv | ProtocolFamily::GenericPlaceholder => {
                if let Ok(tlv) = encode_single_tlv(CmdId::Battery.as_u8(), &batt) {
                    self.send_notify(CHAR_QCY_NOTIFY, tlv);
                }
            }
            ProtocolFamily::LegacyDirectGatt => {
                self.send_notify(CHAR_LEGACY_BATTERY, batt.to_vec());
            }
            ProtocolFamily::JieLiRcspNew if is_new_jl => {
                if let Ok(tlv) = encode_single_tlv(CmdId::Battery.as_u8(), &batt) {
                    if let Ok(rcsp) = qcy_protocol::rcsp::RcspFrame::response(
                        qcy_protocol::rcsp::OP_EXTRA_CUSTOM,
                        0x00,
                        0,
                        &tlv,
                    )
                    .encode()
                    {
                        self.send_notify(CHAR_JIELI_NOTIFY, rcsp);
                    }
                }
            }
            ProtocolFamily::JieLiRcspLegacy => {
                let body = [
                    0x04,
                    qcy_protocol::rcsp::JL_ADV_ATTR_BATTERY,
                    batt[0],
                    batt[1],
                    batt[2],
                ];
                if let Ok(rcsp) = qcy_protocol::rcsp::RcspFrame::response(
                    qcy_protocol::rcsp::OP_ADV_DEVICE_NOTIFY,
                    0x00,
                    0,
                    &body,
                )
                .encode()
                {
                    self.send_notify(CHAR_JIELI_NOTIFY, rcsp);
                }
            }
            _ => {}
        }
    }

    fn push_tlv_or_wrapped(&self, family: &ProtocolFamily, tlv: Vec<u8>) {
        match family {
            ProtocolFamily::JieLiRcspNew => {
                if let Ok(rcsp) = qcy_protocol::rcsp::RcspFrame::response(
                    qcy_protocol::rcsp::OP_EXTRA_CUSTOM,
                    0x00,
                    0,
                    &tlv,
                )
                .encode()
                {
                    self.send_notify(CHAR_JIELI_NOTIFY, rcsp);
                }
            }
            ProtocolFamily::QcyUnifiedTlv
            | ProtocolFamily::LegacyDirectGatt
            | ProtocolFamily::GenericPlaceholder => {
                self.send_notify(CHAR_QCY_NOTIFY, tlv);
            }
            _ => {}
        }
    }

    fn send_notify(&self, char_uuid: Uuid, payload: Vec<u8>) {
        let guard = self.notify_channels.lock().expect("notify map lock");
        if let Some(tx) = guard.get(&char_uuid) {
            let _ = tx.try_send(payload);
        }
    }

    fn build_discovered(&self, is_pop: bool) -> DiscoveredDevice {
        let state = self.snapshot();
        let adv = state.build_advertisement(is_pop);
        let mut manufacturer_data = HashMap::new();
        manufacturer_data.insert(COMPANY_ID_QCY, adv.encode());

        let primary_svc = match state.protocol_family {
            ProtocolFamily::JieLiRcspNew | ProtocolFamily::JieLiRcspLegacy => SVC_JIELI_RCSP,
            _ => SVC_QCY_CUSTOM,
        };

        let transport_kind = match OtaType::from_u8(state.ota_type).map(OtaType::route_info) {
            Some(info) if info.transport == OtaTransport::ClassicRfcomm => TransportKind::Dual,
            _ => TransportKind::Dual,
        };

        DiscoveredDevice {
            id: format!("virtual:{}", state.model_id),
            address: BluetoothAddress::from_bytes(state.class_mac),
            name: state.device_name,
            rssi: Some(-48),
            transport_kind,
            manufacturer_data,
            service_uuids: vec![primary_svc],
            is_paired: true,
            is_connected: state.connected,
            last_seen_ms: now_epoch_ms(),
        }
    }
}

#[async_trait]
impl DeviceScanner for VirtualScanner {
    async fn start_scan(&self) -> TransportResult<()> {
        let _ = self.events_tx.send(ConnectionEvent::AdvertisementSeen(
            self.build_discovered(false),
        ));
        Ok(())
    }

    async fn stop_scan(&self) -> TransportResult<()> {
        Ok(())
    }

    async fn list_paired_devices(&self) -> TransportResult<Vec<DiscoveredDevice>> {
        Ok(vec![self.build_discovered(false)])
    }

    fn subscribe_events(&self) -> broadcast::Receiver<ConnectionEvent> {
        self.events_tx.subscribe()
    }
}

/// Raw-byte virtual Bluetooth connection implementing [`BluetoothConnection`].
pub struct VirtualConnection {
    address: BluetoothAddress,
    state: Arc<Mutex<VirtualDeviceState>>,
    connected: AtomicBool,
    notify_channels: Arc<Mutex<HashMap<Uuid, mpsc::Sender<Vec<u8>>>>>,
    rfcomm_tx: Mutex<Option<mpsc::Sender<Vec<u8>>>>,
    rcsp_acc: Mutex<RcspStreamAccumulator>,
}

impl VirtualConnection {
    /// Standalone constructor creating an isolated default virtual connection (`qcy-n70`).
    pub fn new(address: BluetoothAddress) -> Self {
        let st = VirtualDeviceState {
            class_mac: address.to_bytes(),
            ..VirtualDeviceState::default()
        };
        Self {
            address,
            state: Arc::new(Mutex::new(st)),
            connected: AtomicBool::new(true),
            notify_channels: Arc::new(Mutex::new(HashMap::new())),
            rfcomm_tx: Mutex::new(None),
            rcsp_acc: Mutex::new(RcspStreamAccumulator::new()),
        }
    }

    /// Returns a cloned snapshot of the live simulated hardware state.
    pub fn state_snapshot(&self) -> VirtualDeviceState {
        self.state.lock().expect("simulator state lock").clone()
    }

    fn emit_notify(&self, char_uuid: Uuid, packets: Vec<Vec<u8>>) {
        let guard = self.notify_channels.lock().expect("notify map lock");
        if let Some(tx) = guard.get(&char_uuid) {
            for pkt in packets {
                let _ = tx.try_send(pkt);
            }
        }
    }
}

fn props_write_notify(read: bool, write: bool, notify: bool) -> GattProperties {
    GattProperties {
        read,
        write,
        write_without_response: write,
        notify,
        indicate: false,
    }
}

#[async_trait]
impl BluetoothConnection for VirtualConnection {
    fn transport_kind(&self) -> TransportKind {
        TransportKind::Dual
    }

    fn address(&self) -> BluetoothAddress {
        self.address
    }

    fn mtu(&self) -> usize {
        DEFAULT_BLE_MTU
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst) && self.state.lock().expect("state lock").connected
    }

    async fn discover_services(&self) -> TransportResult<Vec<GattServiceInfo>> {
        if !self.is_connected() {
            return Err(TransportError::NotConnected);
        }
        let st = self.state.lock().expect("state lock");
        let mut services = Vec::new();

        match st.protocol_family {
            ProtocolFamily::JieLiRcspNew | ProtocolFamily::JieLiRcspLegacy => {
                services.push(GattServiceInfo {
                    uuid: SVC_JIELI_RCSP,
                    characteristics: vec![
                        GattCharacteristicInfo {
                            uuid: CHAR_JIELI_WRITE,
                            properties: props_write_notify(false, true, false),
                        },
                        GattCharacteristicInfo {
                            uuid: CHAR_JIELI_NOTIFY,
                            properties: props_write_notify(false, false, true),
                        },
                    ],
                });
            }
            ProtocolFamily::LegacyDirectGatt => {
                services.push(GattServiceInfo {
                    uuid: SVC_QCY_CUSTOM,
                    characteristics: vec![
                        GattCharacteristicInfo {
                            uuid: CHAR_QCY_WRITE,
                            properties: props_write_notify(false, true, false),
                        },
                        GattCharacteristicInfo {
                            uuid: CHAR_QCY_NOTIFY,
                            properties: props_write_notify(false, false, true),
                        },
                        GattCharacteristicInfo {
                            uuid: CHAR_LEGACY_VERSION,
                            properties: props_write_notify(true, false, false),
                        },
                        GattCharacteristicInfo {
                            uuid: CHAR_LEGACY_BATTERY,
                            properties: props_write_notify(true, false, true),
                        },
                        GattCharacteristicInfo {
                            uuid: CHAR_LEGACY_EQ,
                            properties: props_write_notify(true, true, false),
                        },
                        GattCharacteristicInfo {
                            uuid: CHAR_LEGACY_BUTTON,
                            properties: props_write_notify(true, true, false),
                        },
                    ],
                });
            }
            _ => {
                services.push(GattServiceInfo {
                    uuid: SVC_QCY_CUSTOM,
                    characteristics: vec![
                        GattCharacteristicInfo {
                            uuid: CHAR_QCY_WRITE,
                            properties: props_write_notify(false, true, false),
                        },
                        GattCharacteristicInfo {
                            uuid: CHAR_QCY_NOTIFY,
                            properties: props_write_notify(false, false, true),
                        },
                    ],
                });
            }
        }

        // Expose matching BLE OTA service for the active model's otaType
        match OtaType::from_u8(st.ota_type) {
            Some(OtaType::WuQiBle | OtaType::WuQiBleTone) => {
                services.push(GattServiceInfo {
                    uuid: SVC_WUQI_OTA,
                    characteristics: vec![
                        GattCharacteristicInfo {
                            uuid: CHAR_WUQI_OTA_WRITE,
                            properties: props_write_notify(false, true, false),
                        },
                        GattCharacteristicInfo {
                            uuid: CHAR_WUQI_OTA_NOTIFY,
                            properties: props_write_notify(false, false, true),
                        },
                    ],
                });
            }
            Some(OtaType::BluetrumFota) => {
                services.push(GattServiceInfo {
                    uuid: SVC_BLUETRUM_FOTA,
                    characteristics: vec![
                        GattCharacteristicInfo {
                            uuid: CHAR_BLUETRUM_FOTA_WRITE,
                            properties: props_write_notify(false, true, false),
                        },
                        GattCharacteristicInfo {
                            uuid: CHAR_BLUETRUM_FOTA_NOTIFY,
                            properties: props_write_notify(false, false, true),
                        },
                    ],
                });
            }
            Some(OtaType::BesBle) => {
                services.push(GattServiceInfo {
                    uuid: SVC_BES_OTA,
                    characteristics: vec![GattCharacteristicInfo {
                        uuid: CHAR_BES_OTA,
                        properties: props_write_notify(false, true, true),
                    }],
                });
            }
            _ => {}
        }

        Ok(services)
    }

    async fn read_gatt(&self, _service: Uuid, characteristic: Uuid) -> TransportResult<Vec<u8>> {
        if !self.is_connected() {
            return Err(TransportError::NotConnected);
        }
        let st = self.state.lock().expect("state lock");
        wire::handle_legacy_direct_read(&st, characteristic)
            .ok_or_else(|| TransportError::CharacteristicNotFound(characteristic.to_string()))
    }

    async fn write_gatt(
        &self,
        _service: Uuid,
        characteristic: Uuid,
        data: &[u8],
        _with_response: bool,
    ) -> TransportResult<()> {
        if !self.is_connected() {
            return Err(TransportError::NotConnected);
        }
        let latency_ms = self.state.lock().expect("state lock").latency_ms;
        if latency_ms > 0 {
            tokio::time::sleep(Duration::from_millis(latency_ms)).await;
        }

        if characteristic == CHAR_QCY_WRITE {
            let replies = {
                let mut st = self.state.lock().expect("state lock");
                wire::handle_qcy_tlv_write(&mut st, data, false)
            };
            self.emit_notify(CHAR_QCY_NOTIFY, replies);
        } else if characteristic == CHAR_JIELI_WRITE {
            let replies = {
                let mut st = self.state.lock().expect("state lock");
                let mut acc = self.rcsp_acc.lock().expect("rcsp acc lock");
                wire::handle_jieli_rcsp_write(&mut st, &mut acc, data)
            };
            self.emit_notify(CHAR_JIELI_NOTIFY, replies);
        } else if characteristic == CHAR_LEGACY_EQ || characteristic == CHAR_LEGACY_BUTTON {
            let replies = {
                let mut st = self.state.lock().expect("state lock");
                wire::handle_legacy_direct_write(&mut st, characteristic, data)
            };
            self.emit_notify(characteristic, replies);
        } else if characteristic == CHAR_WUQI_OTA_WRITE {
            let replies = {
                let mut st = self.state.lock().expect("state lock");
                wire::handle_ota_write(&mut st, Some(characteristic), data)
            };
            self.emit_notify(CHAR_WUQI_OTA_NOTIFY, replies);
        } else if characteristic == CHAR_BLUETRUM_FOTA_WRITE {
            let replies = {
                let mut st = self.state.lock().expect("state lock");
                wire::handle_ota_write(&mut st, Some(characteristic), data)
            };
            self.emit_notify(CHAR_BLUETRUM_FOTA_NOTIFY, replies);
        } else if characteristic == CHAR_BES_OTA {
            let replies = {
                let mut st = self.state.lock().expect("state lock");
                wire::handle_ota_write(&mut st, Some(characteristic), data)
            };
            self.emit_notify(CHAR_BES_OTA, replies);
        } else {
            return Err(TransportError::CharacteristicNotFound(
                characteristic.to_string(),
            ));
        }

        Ok(())
    }

    async fn subscribe_gatt_notifications(
        &self,
        _service: Uuid,
        characteristic: Uuid,
    ) -> TransportResult<mpsc::Receiver<Vec<u8>>> {
        if !self.is_connected() {
            return Err(TransportError::NotConnected);
        }
        let (tx, rx) = mpsc::channel(128);
        self.notify_channels
            .lock()
            .expect("notify map lock")
            .insert(characteristic, tx);
        Ok(rx)
    }

    async fn open_rfcomm(&self, _service_uuid: Uuid) -> TransportResult<()> {
        self.connected.store(true, Ordering::SeqCst);
        self.state.lock().expect("state lock").connected = true;
        Ok(())
    }

    async fn write_rfcomm(&self, data: &[u8]) -> TransportResult<()> {
        if !self.is_connected() {
            return Err(TransportError::NotConnected);
        }
        let replies = {
            let mut st = self.state.lock().expect("state lock");
            wire::handle_ota_write(&mut st, None, data)
        };
        if let Some(tx) = self.rfcomm_tx.lock().expect("rfcomm tx lock").as_ref() {
            for pkt in replies {
                let _ = tx.try_send(pkt);
            }
        }
        Ok(())
    }

    async fn subscribe_rfcomm(&self) -> TransportResult<mpsc::Receiver<Vec<u8>>> {
        let (tx, rx) = mpsc::channel(64);
        *self.rfcomm_tx.lock().expect("rfcomm tx lock") = Some(tx);
        Ok(rx)
    }

    async fn disconnect(&self) -> TransportResult<()> {
        self.connected.store(false, Ordering::SeqCst);
        self.state.lock().expect("state lock").connected = false;
        self.notify_channels
            .lock()
            .expect("notify map lock")
            .clear();
        *self.rfcomm_tx.lock().expect("rfcomm tx lock") = None;
        Ok(())
    }
}

/// Convenience helper returning the matching `DeviceModelProfile` for the simulator's current model.
pub fn resolve_virtual_model_profile(state: &VirtualDeviceState) -> DeviceModelProfile {
    ModelRegistry::default()
        .find_by_id(&state.model_id)
        .cloned()
        .unwrap_or_else(ModelRegistry::generic_fallback)
}

#[cfg(test)]
mod tests {
    use super::*;
    use qcy_protocol::{
        AncMode, AncSetting, ButtonAction, ButtonBinding, DeviceCommand, DeviceEvent,
        IrrFilterType, LedEffectConfig, LedEffectKind, OtaSession, ParametricEqBand,
        ParametricEqPreset, ProtocolRouter, QcyAdvertisement, RgbColor, TouchSensitivity,
        CHAR_JIELI_NOTIFY, CHAR_QCY_NOTIFY, CHAR_WUQI_OTA_NOTIFY, CHAR_WUQI_OTA_WRITE,
        SVC_JIELI_RCSP, SVC_QCY_CUSTOM, SVC_WUQI_OTA,
    };

    #[tokio::test]
    async fn virtual_scanner_lists_and_advertises_24b_ble_popup() {
        let scanner = VirtualScanner::new(VirtualDeviceState::for_model("qcy-n70").unwrap());
        let reg = ModelRegistry::default();
        let mut events = scanner.subscribe_events();

        scanner.start_scan().await.expect("start scan");
        let evt = events.recv().await.expect("scan adv");
        let ConnectionEvent::AdvertisementSeen(dev) = evt else {
            panic!("expected AdvertisementSeen");
        };
        let raw_mfg = dev
            .manufacturer_data
            .get(&COMPANY_ID_QCY)
            .expect("0x071E manufacturer data");
        assert_eq!(raw_mfg.len(), 24);
        let parsed = QcyAdvertisement::parse(COMPANY_ID_QCY, raw_mfg).expect("parse 24B adv");
        assert_eq!(parsed.model_id, 0x5D40);
        assert!(!parsed.is_pop);
        assert_eq!(parsed.left_battery.percent, Some(88));
        assert_eq!(
            reg.match_discovered_device(Some(&dev.name), Some(&parsed))
                .model_id,
            "qcy-n70"
        );

        // Trigger lid-open popup -> pop_num increments & is_pop == true
        let initial_pop = parsed.pop_num.unwrap();
        let pop_dev = scanner.trigger_lid_open_popup();
        let pop_adv = QcyAdvertisement::parse(
            COMPANY_ID_QCY,
            pop_dev.manufacturer_data.get(&COMPANY_ID_QCY).unwrap(),
        )
        .expect("parse popup adv");
        assert!(pop_adv.is_pop);
        assert_eq!(pop_adv.pop_num, Some(initial_pop.wrapping_add(1)));
    }

    #[tokio::test]
    async fn end_to_end_qcy_unified_tlv_protocol_driver_roundtrip() {
        let scanner = VirtualScanner::new(VirtualDeviceState::for_model("qcy-n70").unwrap());
        let conn = scanner.open_connection();
        let services = conn.discover_services().await.expect("discover");
        let svc_uuids: Vec<Uuid> = services.iter().map(|s| s.uuid).collect();
        assert!(svc_uuids.contains(&SVC_QCY_CUSTOM));

        let driver = ProtocolRouter::select_driver(Some(0x5D40), &svc_uuids);
        assert_eq!(driver.family(), ProtocolFamily::QcyUnifiedTlv);

        let mut rx = conn
            .subscribe_gatt_notifications(SVC_QCY_CUSTOM, CHAR_QCY_NOTIFY)
            .await
            .expect("subscribe 0x1002");

        // 1. Custom 16-level ANC depth write -> raw TLV -> decode back
        for w in driver
            .encode_command(&DeviceCommand::SetAncSetting(AncSetting::custom_depth(12)))
            .unwrap()
        {
            conn.write_gatt(w.service, w.characteristic, &w.payload, w.with_response)
                .await
                .unwrap();
        }
        let raw_anc = rx.recv().await.expect("anc notify");
        let anc_events = driver
            .decode_notification(CHAR_QCY_NOTIFY, &raw_anc)
            .unwrap();
        assert!(anc_events.contains(&DeviceEvent::AncModeChanged {
            mode: AncMode::CustomDepth,
            depth: Some(12),
        }));

        // 2. Parametric EQ v2 (0x22) with IrrFilterType -> raw TLV -> decode back
        let peq = ParametricEqPreset {
            eq_type: 2,
            master_gain_db: -1.5,
            bands: vec![
                ParametricEqBand {
                    freq_hz: 80,
                    gain_db: 3.5,
                    q: 0.9,
                    filter_type: IrrFilterType::LowShelf,
                },
                ParametricEqBand {
                    freq_hz: 3200,
                    gain_db: 2.0,
                    q: 1.4,
                    filter_type: IrrFilterType::Peak,
                },
            ],
        };
        for w in driver
            .encode_command(&DeviceCommand::SetParametricEqV2(peq.clone()))
            .unwrap()
        {
            conn.write_gatt(w.service, w.characteristic, &w.payload, w.with_response)
                .await
                .unwrap();
        }
        let raw_eq = rx.recv().await.expect("eq notify");
        let eq_events = driver
            .decode_notification(CHAR_QCY_NOTIFY, &raw_eq)
            .unwrap();
        assert_eq!(eq_events, vec![DeviceEvent::ParametricEqV2Updated(peq)]);

        // 3. Config-only active wire CMDIDs (0x33 touchSensitivity & 0x63 thxEq)
        for cmd in [
            DeviceCommand::SetTouchSensitivity {
                sensitivity: TouchSensitivity::Sensitive,
            },
            DeviceCommand::SetThxEq { enabled: true },
        ] {
            for w in driver.encode_command(&cmd).unwrap() {
                conn.write_gatt(w.service, w.characteristic, &w.payload, w.with_response)
                    .await
                    .unwrap();
            }
        }
        let ev_sens = driver
            .decode_notification(CHAR_QCY_NOTIFY, &rx.recv().await.unwrap())
            .unwrap();
        assert_eq!(
            ev_sens,
            vec![DeviceEvent::TouchSensitivityUpdated {
                sensitivity: TouchSensitivity::Sensitive
            }]
        );
        let ev_thx = driver
            .decode_notification(CHAR_QCY_NOTIFY, &rx.recv().await.unwrap())
            .unwrap();
        assert_eq!(ev_thx, vec![DeviceEvent::ThxEqChanged { enabled: true }]);

        // 4. Interactive battery update pushes live raw notification
        scanner.set_battery(55, 60, 40);
        let batt_ev = driver
            .decode_notification(CHAR_QCY_NOTIFY, &rx.recv().await.unwrap())
            .unwrap();
        assert_eq!(
            batt_ev,
            vec![DeviceEvent::BatteryUpdated {
                left: Some(55),
                right: Some(60),
                case: Some(40),
                left_charging: false,
                right_charging: false,
                case_charging: false,
            }]
        );
    }

    #[tokio::test]
    async fn end_to_end_jieli_new_jl_rcsp_tunnel_and_off_0x02_quirk() {
        let scanner = VirtualScanner::new(VirtualDeviceState::for_model("qcy-ht08-jl").unwrap());
        let conn = scanner.open_connection();
        let services = conn.discover_services().await.unwrap();
        let svc_uuids: Vec<Uuid> = services.iter().map(|s| s.uuid).collect();
        assert!(svc_uuids.contains(&SVC_JIELI_RCSP));

        let driver = ProtocolRouter::select_driver(Some(0x9072), &svc_uuids);
        assert_eq!(driver.family(), ProtocolFamily::JieLiRcspNew);

        let mut rx = conn
            .subscribe_gatt_notifications(SVC_JIELI_RCSP, CHAR_JIELI_NOTIFY)
            .await
            .unwrap();

        // Game Mode ON -> OFF (verifying OFF=0x02 wire encoding + decode round-trip through RCSP 0xFF)
        for enabled in [true, false] {
            for w in driver
                .encode_command(&DeviceCommand::SetGamingMode { enabled })
                .unwrap()
            {
                conn.write_gatt(w.service, w.characteristic, &w.payload, w.with_response)
                    .await
                    .unwrap();
            }
            let raw = rx.recv().await.expect("rcsp notify");
            assert_eq!(&raw[0..3], &[0xFE, 0xDC, 0xBA]);
            let ev = driver.decode_notification(CHAR_JIELI_NOTIFY, &raw).unwrap();
            assert_eq!(ev, vec![DeviceEvent::GamingModeChanged { enabled }]);
        }
    }

    #[tokio::test]
    async fn end_to_end_legacy_jieli_h3_native_rcsp_and_button_0x7f_mapping() {
        let scanner = VirtualScanner::new(VirtualDeviceState::for_model("qcy-h3").unwrap());
        let conn = scanner.open_connection();
        let services = conn.discover_services().await.unwrap();
        let svc_uuids: Vec<Uuid> = services.iter().map(|s| s.uuid).collect();

        let driver = ProtocolRouter::select_driver(Some(0x417C), &svc_uuids);
        assert_eq!(driver.family(), ProtocolFamily::JieLiRcspLegacy);

        let mut rx = conn
            .subscribe_gatt_notifications(SVC_JIELI_RCSP, CHAR_JIELI_NOTIFY)
            .await
            .unwrap();

        // Set MFB Double Tap (0x0702) -> Action None (0 -> 0x7F on wire -> 0 on decode!)
        for w in driver
            .encode_command(&DeviceCommand::SetButtonBinding(ButtonBinding {
                event_id: 0x0702,
                action: ButtonAction::None,
            }))
            .unwrap()
        {
            conn.write_gatt(w.service, w.characteristic, &w.payload, w.with_response)
                .await
                .unwrap();
        }
        let raw = rx.recv().await.expect("legacy jl notify");
        let events = driver.decode_notification(CHAR_JIELI_NOTIFY, &raw).unwrap();
        let btn_evt = events
            .iter()
            .find_map(|e| match e {
                DeviceEvent::ButtonBindingsUpdated { bindings } => Some(bindings),
                _ => None,
            })
            .expect("button bindings update");
        assert!(btn_evt.contains(&ButtonBinding {
            event_id: 0x0702,
            action: ButtonAction::None,
        }));
    }

    #[tokio::test]
    async fn end_to_end_sp75_rgb_lighting_and_wuqi_ble_ota_simulation() {
        let scanner = VirtualScanner::new(VirtualDeviceState::for_model("qcy-sp75").unwrap());
        let conn = scanner.open_connection();
        let services = conn.discover_services().await.unwrap();
        let svc_uuids: Vec<Uuid> = services.iter().map(|s| s.uuid).collect();
        assert!(svc_uuids.contains(&SVC_WUQI_OTA));

        let driver = ProtocolRouter::select_driver(Some(0x0308), &svc_uuids);
        let mut rx = conn
            .subscribe_gatt_notifications(SVC_QCY_CUSTOM, CHAR_QCY_NOTIFY)
            .await
            .unwrap();

        let rgb = LedEffectConfig {
            speed: 5,
            brightness: 95,
            effect: LedEffectKind::MusicRhythm,
            colors: vec![RgbColor {
                r: 0,
                g: 255,
                b: 128,
            }],
        };
        for w in driver
            .encode_command(&DeviceCommand::SetLedEffect(rgb.clone()))
            .unwrap()
        {
            conn.write_gatt(w.service, w.characteristic, &w.payload, w.with_response)
                .await
                .unwrap();
        }
        let ev = driver
            .decode_notification(CHAR_QCY_NOTIFY, &rx.recv().await.unwrap())
            .unwrap();
        assert_eq!(ev, vec![DeviceEvent::LedEffectUpdated(rgb)]);

        // WuQi BLE OTA session round-trip through VirtualConnection
        let mut ota_rx = conn
            .subscribe_gatt_notifications(SVC_WUQI_OTA, CHAR_WUQI_OTA_NOTIFY)
            .await
            .unwrap();
        let mut ota = OtaSession::new(OtaType::WuQiBleTone, vec![0xAB; 512], 256).unwrap();
        for pkt in ota.build_start_packets().unwrap() {
            conn.write_gatt(SVC_WUQI_OTA, CHAR_WUQI_OTA_WRITE, &pkt, true)
                .await
                .unwrap();
            let ack = ota_rx.recv().await.expect("wuqi ota ack");
            assert_eq!(ack[0], 0x33);
            assert_eq!(ack[3], 0x00);
        }
        while let Some(chunk) = ota.next_data_packet().unwrap() {
            conn.write_gatt(SVC_WUQI_OTA, CHAR_WUQI_OTA_WRITE, &chunk, true)
                .await
                .unwrap();
            let ack = ota_rx.recv().await.expect("wuqi chunk ack");
            assert_eq!(ack[0], 0x33);
        }
        assert!(conn.state_snapshot().ota_bytes_received > 512);
    }

    #[tokio::test]
    async fn all_six_builtin_virtual_models_switch_cleanly() {
        let scanner = VirtualScanner::new(VirtualDeviceState::default());
        assert_eq!(VirtualScanner::builtin_virtual_models().len(), 6);
        for &model_id in VirtualScanner::builtin_virtual_models() {
            let st = scanner.switch_model(model_id).expect("switch model");
            assert_eq!(st.model_id, model_id);
        }
    }
}
