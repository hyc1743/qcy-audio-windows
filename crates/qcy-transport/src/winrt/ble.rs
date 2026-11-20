use super::{guid_to_uuid, ibuffer_to_vec, slice_to_ibuffer, uuid_to_guid};
use crate::{
    BluetoothAddress, BluetoothConnection, GattCharacteristicInfo, GattProperties, GattServiceInfo,
    TransportError, TransportKind, TransportResult,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use uuid::Uuid;
use windows::core::IInspectable;
use windows::Devices::Bluetooth::GenericAttributeProfile::{
    GattCharacteristic, GattCharacteristicProperties,
    GattClientCharacteristicConfigurationDescriptorValue, GattCommunicationStatus,
    GattDeviceService, GattSession, GattValueChangedEventArgs, GattWriteOption,
};
use windows::Devices::Bluetooth::{
    BluetoothCacheMode, BluetoothConnectionStatus, BluetoothLEDevice,
};
use windows::Foundation::TypedEventHandler;

const DEFAULT_BLE_MTU: usize = 23;

struct ActiveNotification {
    characteristic: GattCharacteristic,
    token: i64,
}

struct BleInner {
    device: Option<BluetoothLEDevice>,
    session: Option<GattSession>,
    conn_status_token: Option<i64>,
    mtu_token: Option<i64>,
    services: HashMap<Uuid, GattDeviceService>,
    characteristics: HashMap<(Uuid, Uuid), GattCharacteristic>,
    notifications: Vec<ActiveNotification>,
}

impl BleInner {
    fn close_all(&mut self) {
        // 1. Revoke all GATT ValueChanged callbacks & reset CCCD best-effort.
        for sub in self.notifications.drain(..) {
            let _ = sub.characteristic.RemoveValueChanged(sub.token);
            if let Ok(op) = sub
                .characteristic
                .WriteClientCharacteristicConfigurationDescriptorAsync(
                    GattClientCharacteristicConfigurationDescriptorValue::None,
                )
            {
                let _ = op.join();
            }
        }

        // 2. Drop cached characteristics before closing parent services.
        self.characteristics.clear();

        // 3. Close every cached GattDeviceService handle.
        for (_, svc) in self.services.drain() {
            let _ = svc.Close();
        }

        // 4. Close GattSession.
        if let Some(session) = self.session.take() {
            if let Some(tok) = self.mtu_token.take() {
                let _ = session.RemoveMaxPduSizeChanged(tok);
            }
            let _ = session.SetMaintainConnection(false);
            let _ = session.Close();
        }

        // 5. Revoke ConnectionStatusChanged and close BluetoothLEDevice.
        if let Some(device) = self.device.take() {
            if let Some(tok) = self.conn_status_token.take() {
                let _ = device.RemoveConnectionStatusChanged(tok);
            }
            let _ = device.Close();
        }
    }
}

/// Windows WinRT BLE GATT connection.
pub struct WinrtBleConnection {
    address: BluetoothAddress,
    connected: Arc<AtomicBool>,
    mtu: Arc<AtomicUsize>,
    inner: Arc<Mutex<BleInner>>,
}

impl WinrtBleConnection {
    /// Connects to a BLE peripheral by Bluetooth MAC address using WinRT `BluetoothLEDevice`.
    pub async fn connect(address: BluetoothAddress) -> TransportResult<Self> {
        let connected = Arc::new(AtomicBool::new(false));
        let mtu = Arc::new(AtomicUsize::new(DEFAULT_BLE_MTU));

        let connected_cb = Arc::clone(&connected);
        let mtu_cb = Arc::clone(&mtu);

        let inner = tokio::task::spawn_blocking(move || -> TransportResult<BleInner> {
            let device = BluetoothLEDevice::FromBluetoothAddressAsync(address.as_u64())
                .map_err(|e| {
                    TransportError::ConnectionFailed(format!(
                        "BluetoothLEDevice::FromBluetoothAddressAsync({address}) failed: {e}"
                    ))
                })?
                .join()
                .map_err(|e| {
                    TransportError::DeviceNotFound(format!(
                        "BLE device {address} could not be resolved: {e}"
                    ))
                })?;

            // Track live connection status transitions.
            let status_flag = Arc::clone(&connected_cb);
            let conn_handler =
                TypedEventHandler::<BluetoothLEDevice, IInspectable>::new(move |sender, _| {
                    if let Some(dev) = sender.as_ref() {
                        if let Ok(st) = dev.ConnectionStatus() {
                            status_flag.store(
                                st == BluetoothConnectionStatus::Connected,
                                Ordering::SeqCst,
                            );
                        }
                    }
                    Ok(())
                });
            let conn_status_token = device.ConnectionStatusChanged(&conn_handler).ok();

            // Create GattSession to read real MaxPduSize (MTU) and maintain connection.
            let mut session_opt = None;
            let mut mtu_token = None;
            if let Ok(dev_id) = device.BluetoothDeviceId() {
                if let Ok(op) = GattSession::FromDeviceIdAsync(&dev_id) {
                    if let Ok(session) = op.join() {
                        let _ = session.SetMaintainConnection(true);
                        if let Ok(pdu) = session.MaxPduSize() {
                            if pdu > 0 {
                                mtu_cb.store(pdu as usize, Ordering::SeqCst);
                            }
                        }
                        let mtu_atomic = Arc::clone(&mtu_cb);
                        let pdu_handler = TypedEventHandler::<GattSession, IInspectable>::new(
                            move |sender, _| {
                                if let Some(sess) = sender.as_ref() {
                                    if let Ok(pdu) = sess.MaxPduSize() {
                                        if pdu > 0 {
                                            mtu_atomic.store(pdu as usize, Ordering::SeqCst);
                                        }
                                    }
                                }
                                Ok(())
                            },
                        );
                        mtu_token = session.MaxPduSizeChanged(&pdu_handler).ok();
                        session_opt = Some(session);
                    }
                }
            }

            // Force an initial uncached service probe to trigger physical BLE link establishment.
            let svc_result = device
                .GetGattServicesWithCacheModeAsync(BluetoothCacheMode::Uncached)
                .map_err(|e| {
                    TransportError::ConnectionFailed(format!("GetGattServicesAsync failed: {e}"))
                })?
                .join()
                .map_err(|e| {
                    TransportError::ConnectionFailed(format!(
                        "GATT service probe join failed for {address}: {e}"
                    ))
                })?;

            let comm_status = svc_result
                .Status()
                .unwrap_or(GattCommunicationStatus::Unreachable);
            if comm_status != GattCommunicationStatus::Success {
                let _ = device.Close();
                return Err(TransportError::ConnectionFailed(format!(
                    "BLE device {address} GATT probe status: {comm_status:?}"
                )));
            }

            let mut services = HashMap::new();
            if let Ok(list) = svc_result.Services() {
                let count = list.Size().unwrap_or(0);
                for i in 0..count {
                    if let Ok(svc) = list.GetAt(i) {
                        if let Ok(guid) = svc.Uuid() {
                            services.insert(guid_to_uuid(guid), svc);
                        }
                    }
                }
            }

            // Refresh negotiated MTU now that the BLE link is established.
            if let Some(ref session) = session_opt {
                if let Ok(pdu) = session.MaxPduSize() {
                    if pdu > 0 {
                        mtu_cb.store(pdu as usize, Ordering::SeqCst);
                    }
                }
            }

            let is_now_connected = device
                .ConnectionStatus()
                .map(|s| s == BluetoothConnectionStatus::Connected)
                .unwrap_or(true);
            connected_cb.store(is_now_connected, Ordering::SeqCst);

            Ok(BleInner {
                device: Some(device),
                session: session_opt,
                conn_status_token,
                mtu_token,
                services,
                characteristics: HashMap::new(),
                notifications: Vec::new(),
            })
        })
        .await
        .map_err(|e| TransportError::ConnectionFailed(format!("BLE connect task failed: {e}")))??;

        Ok(Self {
            address,
            connected,
            mtu,
            inner: Arc::new(Mutex::new(inner)),
        })
    }

    fn resolve_characteristic_blocking(
        inner: &mut BleInner,
        service_uuid: Uuid,
        char_uuid: Uuid,
    ) -> TransportResult<GattCharacteristic> {
        if let Some(cached) = inner.characteristics.get(&(service_uuid, char_uuid)) {
            return Ok(cached.clone());
        }

        let device = inner.device.as_ref().ok_or(TransportError::NotConnected)?;

        // Ensure service is cached; try targeted UUID query first, then full discovery fallback.
        if let std::collections::hash_map::Entry::Vacant(slot) = inner.services.entry(service_uuid)
        {
            let svc_guid = uuid_to_guid(service_uuid);
            let mut found = None;

            if let Ok(op) = device
                .GetGattServicesForUuidWithCacheModeAsync(svc_guid, BluetoothCacheMode::Uncached)
            {
                if let Ok(res) = op.join() {
                    if res.Status().ok() == Some(GattCommunicationStatus::Success) {
                        if let Ok(list) = res.Services() {
                            if list.Size().unwrap_or(0) > 0 {
                                found = list.GetAt(0).ok();
                            }
                        }
                    }
                }
            }

            if found.is_none() {
                if let Ok(op) =
                    device.GetGattServicesWithCacheModeAsync(BluetoothCacheMode::Uncached)
                {
                    if let Ok(res) = op.join() {
                        if let Ok(list) = res.Services() {
                            let count = list.Size().unwrap_or(0);
                            for i in 0..count {
                                if let Ok(svc) = list.GetAt(i) {
                                    if let Ok(guid) = svc.Uuid() {
                                        let u = guid_to_uuid(guid);
                                        if u == service_uuid {
                                            found = Some(svc);
                                            break;
                                        } else {
                                            let _ = svc.Close();
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let service = found.ok_or_else(|| {
                TransportError::ServiceNotFound(format!("GATT service {service_uuid} not found"))
            })?;
            slot.insert(service);
        }

        let service = inner
            .services
            .get(&service_uuid)
            .ok_or_else(|| TransportError::ServiceNotFound(service_uuid.to_string()))?;

        // Targeted characteristic lookup, then full service characteristics fallback.
        let char_guid = uuid_to_guid(char_uuid);
        let mut found_char = None;

        if let Ok(op) = service
            .GetCharacteristicsForUuidWithCacheModeAsync(char_guid, BluetoothCacheMode::Uncached)
        {
            if let Ok(res) = op.join() {
                if res.Status().ok() == Some(GattCommunicationStatus::Success) {
                    if let Ok(list) = res.Characteristics() {
                        if list.Size().unwrap_or(0) > 0 {
                            found_char = list.GetAt(0).ok();
                        }
                    }
                }
            }
        }

        if found_char.is_none() {
            if let Ok(op) =
                service.GetCharacteristicsWithCacheModeAsync(BluetoothCacheMode::Uncached)
            {
                if let Ok(res) = op.join() {
                    if let Ok(list) = res.Characteristics() {
                        let count = list.Size().unwrap_or(0);
                        for i in 0..count {
                            if let Ok(ch) = list.GetAt(i) {
                                if let Ok(guid) = ch.Uuid() {
                                    if guid_to_uuid(guid) == char_uuid {
                                        found_char = Some(ch);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let ch = found_char.ok_or_else(|| {
            TransportError::CharacteristicNotFound(format!(
                "GATT characteristic {char_uuid} in service {service_uuid} not found"
            ))
        })?;

        inner
            .characteristics
            .insert((service_uuid, char_uuid), ch.clone());
        Ok(ch)
    }
}

impl Drop for WinrtBleConnection {
    fn drop(&mut self) {
        self.connected.store(false, Ordering::SeqCst);
        if let Ok(mut guard) = self.inner.lock() {
            guard.close_all();
        }
    }
}

#[async_trait]
impl BluetoothConnection for WinrtBleConnection {
    fn transport_kind(&self) -> TransportKind {
        TransportKind::Ble
    }

    fn address(&self) -> BluetoothAddress {
        self.address
    }

    fn mtu(&self) -> usize {
        self.mtu.load(Ordering::SeqCst)
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    async fn discover_services(&self) -> TransportResult<Vec<GattServiceInfo>> {
        let inner = Arc::clone(&self.inner);
        tokio::task::spawn_blocking(move || {
            let mut guard = inner
                .lock()
                .map_err(|_| TransportError::Io("BLE inner mutex poisoned".to_string()))?;
            let device = guard.device.as_ref().ok_or(TransportError::NotConnected)?;

            let result = device
                .GetGattServicesWithCacheModeAsync(BluetoothCacheMode::Uncached)
                .map_err(|e| TransportError::Io(format!("GetGattServicesAsync failed: {e}")))?
                .join()
                .map_err(|e| {
                    TransportError::Io(format!("GetGattServicesAsync join failed: {e}"))
                })?;

            let status = result
                .Status()
                .unwrap_or(GattCommunicationStatus::Unreachable);
            if status != GattCommunicationStatus::Success {
                return Err(TransportError::Io(format!(
                    "GATT service discovery failed with status {status:?}"
                )));
            }

            let mut out = Vec::new();
            let services_list = result
                .Services()
                .map_err(|e| TransportError::Io(e.to_string()))?;
            let svc_count = services_list.Size().unwrap_or(0);

            for i in 0..svc_count {
                let Ok(svc) = services_list.GetAt(i) else {
                    continue;
                };
                let Ok(svc_guid) = svc.Uuid() else { continue };
                let svc_uuid = guid_to_uuid(svc_guid);

                let mut chars = Vec::new();
                if let Ok(ch_op) =
                    svc.GetCharacteristicsWithCacheModeAsync(BluetoothCacheMode::Uncached)
                {
                    if let Ok(ch_res) = ch_op.join() {
                        if let Ok(ch_list) = ch_res.Characteristics() {
                            let ch_count = ch_list.Size().unwrap_or(0);
                            for j in 0..ch_count {
                                if let Ok(ch) = ch_list.GetAt(j) {
                                    if let Ok(ch_guid) = ch.Uuid() {
                                        let ch_uuid = guid_to_uuid(ch_guid);
                                        let props = parse_gatt_props(
                                            ch.CharacteristicProperties()
                                                .unwrap_or(GattCharacteristicProperties::None),
                                        );
                                        chars.push(GattCharacteristicInfo {
                                            uuid: ch_uuid,
                                            properties: props,
                                        });
                                        guard.characteristics.insert((svc_uuid, ch_uuid), ch);
                                    }
                                }
                            }
                        }
                    }
                }

                // Keep cached service handle open so notifications remain alive.
                guard.services.entry(svc_uuid).or_insert(svc);
                out.push(GattServiceInfo {
                    uuid: svc_uuid,
                    characteristics: chars,
                });
            }

            Ok(out)
        })
        .await
        .map_err(|e| TransportError::Io(format!("discover_services join error: {e}")))?
    }

    async fn read_gatt(&self, service: Uuid, characteristic: Uuid) -> TransportResult<Vec<u8>> {
        let inner = Arc::clone(&self.inner);
        tokio::task::spawn_blocking(move || {
            let ch = {
                let mut guard = inner
                    .lock()
                    .map_err(|_| TransportError::Io("BLE inner mutex poisoned".to_string()))?;
                Self::resolve_characteristic_blocking(&mut guard, service, characteristic)?
            };

            let read_res = ch
                .ReadValueWithCacheModeAsync(BluetoothCacheMode::Uncached)
                .map_err(|e| TransportError::Io(format!("ReadValueAsync failed: {e}")))?
                .join()
                .map_err(|e| TransportError::Io(format!("ReadValueAsync join failed: {e}")))?;

            let status = read_res
                .Status()
                .unwrap_or(GattCommunicationStatus::Unreachable);
            if status != GattCommunicationStatus::Success {
                return Err(TransportError::Io(format!(
                    "GATT read failed for {characteristic}: {status:?}"
                )));
            }

            let value_buf = read_res
                .Value()
                .map_err(|e| TransportError::Io(format!("GATT read Value() failed: {e}")))?;
            ibuffer_to_vec(&value_buf)
        })
        .await
        .map_err(|e| TransportError::Io(format!("read_gatt task join error: {e}")))?
    }

    async fn write_gatt(
        &self,
        service: Uuid,
        characteristic: Uuid,
        data: &[u8],
        with_response: bool,
    ) -> TransportResult<()> {
        let inner = Arc::clone(&self.inner);
        let payload = data.to_vec();
        tokio::task::spawn_blocking(move || {
            let ch = {
                let mut guard = inner
                    .lock()
                    .map_err(|_| TransportError::Io("BLE inner mutex poisoned".to_string()))?;
                Self::resolve_characteristic_blocking(&mut guard, service, characteristic)?
            };

            let buffer = slice_to_ibuffer(&payload)?;
            let option = if with_response {
                GattWriteOption::WriteWithResponse
            } else {
                GattWriteOption::WriteWithoutResponse
            };

            let write_res = ch
                .WriteValueWithResultAndOptionAsync(&buffer, option)
                .map_err(|e| {
                    TransportError::Io(format!("WriteValueWithResultAndOptionAsync failed: {e}"))
                })?
                .join()
                .map_err(|e| TransportError::Io(format!("GATT write join failed: {e}")))?;

            let status = write_res
                .Status()
                .unwrap_or(GattCommunicationStatus::Unreachable);
            if status != GattCommunicationStatus::Success {
                return Err(TransportError::Io(format!(
                    "GATT write failed for {characteristic}: {status:?}"
                )));
            }
            Ok(())
        })
        .await
        .map_err(|e| TransportError::Io(format!("write_gatt task join error: {e}")))?
    }

    async fn subscribe_gatt_notifications(
        &self,
        service: Uuid,
        characteristic: Uuid,
    ) -> TransportResult<mpsc::Receiver<Vec<u8>>> {
        let inner = Arc::clone(&self.inner);
        let (tx, rx) = mpsc::channel(128);

        tokio::task::spawn_blocking(move || {
            let ch = {
                let mut guard = inner
                    .lock()
                    .map_err(|_| TransportError::Io("BLE inner mutex poisoned".to_string()))?;
                Self::resolve_characteristic_blocking(&mut guard, service, characteristic)?
            };

            let props = ch
                .CharacteristicProperties()
                .unwrap_or(GattCharacteristicProperties::Notify);
            let cccd_val = if (props & GattCharacteristicProperties::Notify)
                != GattCharacteristicProperties::None
            {
                GattClientCharacteristicConfigurationDescriptorValue::Notify
            } else if (props & GattCharacteristicProperties::Indicate)
                != GattCharacteristicProperties::None
            {
                GattClientCharacteristicConfigurationDescriptorValue::Indicate
            } else {
                GattClientCharacteristicConfigurationDescriptorValue::Notify
            };

            let handler = TypedEventHandler::<GattCharacteristic, GattValueChangedEventArgs>::new(
                move |_sender, args| {
                    if let Some(args) = args.as_ref() {
                        if let Ok(buf) = args.CharacteristicValue() {
                            if let Ok(bytes) = ibuffer_to_vec(&buf) {
                                let _ = tx.try_send(bytes);
                            }
                        }
                    }
                    Ok(())
                },
            );

            let token = ch
                .ValueChanged(&handler)
                .map_err(|e| TransportError::Io(format!("ValueChanged subscribe failed: {e}")))?;

            let cccd_status = ch
                .WriteClientCharacteristicConfigurationDescriptorAsync(cccd_val)
                .map_err(|e| {
                    let _ = ch.RemoveValueChanged(token);
                    TransportError::Io(format!("WriteCCCD failed: {e}"))
                })?
                .join()
                .map_err(|e| {
                    let _ = ch.RemoveValueChanged(token);
                    TransportError::Io(format!("WriteCCCD join failed: {e}"))
                })?;

            if cccd_status != GattCommunicationStatus::Success {
                let _ = ch.RemoveValueChanged(token);
                return Err(TransportError::Io(format!(
                    "WriteCCCD failed for {characteristic}: {cccd_status:?}"
                )));
            }

            let mut guard = inner
                .lock()
                .map_err(|_| TransportError::Io("BLE inner mutex poisoned".to_string()))?;
            guard.notifications.push(ActiveNotification {
                characteristic: ch,
                token,
            });
            Ok(())
        })
        .await
        .map_err(|e| {
            TransportError::Io(format!("subscribe_gatt_notifications task join error: {e}"))
        })??;

        Ok(rx)
    }

    async fn open_rfcomm(&self, _service_uuid: Uuid) -> TransportResult<()> {
        Err(TransportError::Unsupported(
            "WinrtBleConnection does not support Classic RFCOMM; use WinrtRfcommConnection or WinrtDualConnection"
                .to_string(),
        ))
    }

    async fn write_rfcomm(&self, _data: &[u8]) -> TransportResult<()> {
        Err(TransportError::Unsupported(
            "WinrtBleConnection does not support Classic RFCOMM".to_string(),
        ))
    }

    async fn subscribe_rfcomm(&self) -> TransportResult<mpsc::Receiver<Vec<u8>>> {
        Err(TransportError::Unsupported(
            "WinrtBleConnection does not support Classic RFCOMM".to_string(),
        ))
    }

    async fn disconnect(&self) -> TransportResult<()> {
        self.connected.store(false, Ordering::SeqCst);
        let inner = Arc::clone(&self.inner);
        tokio::task::spawn_blocking(move || {
            if let Ok(mut guard) = inner.lock() {
                guard.close_all();
            }
        })
        .await
        .map_err(|e| TransportError::Io(format!("BLE disconnect task join error: {e}")))?;
        Ok(())
    }
}

fn parse_gatt_props(raw: GattCharacteristicProperties) -> GattProperties {
    GattProperties {
        read: (raw & GattCharacteristicProperties::Read) != GattCharacteristicProperties::None,
        write: (raw & GattCharacteristicProperties::Write) != GattCharacteristicProperties::None,
        write_without_response: (raw & GattCharacteristicProperties::WriteWithoutResponse)
            != GattCharacteristicProperties::None,
        notify: (raw & GattCharacteristicProperties::Notify) != GattCharacteristicProperties::None,
        indicate: (raw & GattCharacteristicProperties::Indicate)
            != GattCharacteristicProperties::None,
    }
}
