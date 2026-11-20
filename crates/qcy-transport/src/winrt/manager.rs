use super::{guid_to_uuid, ibuffer_to_vec};
use crate::{
    merge_discovered_device, now_epoch_ms, AdapterInfo, BluetoothAddress, ConnectionEvent,
    DeviceScanner, DiscoveredDevice, TransportError, TransportKind, TransportResult,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use windows::Devices::Bluetooth::Advertisement::{
    BluetoothLEAdvertisementReceivedEventArgs, BluetoothLEAdvertisementWatcher,
    BluetoothLEScanningMode,
};
use windows::Devices::Bluetooth::{
    BluetoothAdapter, BluetoothConnectionStatus, BluetoothDevice, BluetoothLEDevice,
};
use windows::Devices::Enumeration::DeviceInformation;
use windows::Foundation::TypedEventHandler;

struct ActiveWatcher {
    watcher: BluetoothLEAdvertisementWatcher,
    received_token: i64,
}

impl ActiveWatcher {
    fn stop_and_detach(&self) {
        let _ = self.watcher.Stop();
        let _ = self.watcher.RemoveReceived(self.received_token);
    }
}

/// Windows WinRT Bluetooth discovery and adapter manager.
pub struct WinrtBluetoothManager {
    events_tx: broadcast::Sender<ConnectionEvent>,
    active_watcher: Arc<Mutex<Option<ActiveWatcher>>>,
}

impl Default for WinrtBluetoothManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WinrtBluetoothManager {
    pub fn new() -> Self {
        let (events_tx, _) = broadcast::channel(256);
        Self {
            events_tx,
            active_watcher: Arc::new(Mutex::new(None)),
        }
    }

    /// Probes the default Windows Bluetooth adapter via WinRT `BluetoothAdapter::GetDefaultAsync`.
    pub async fn adapter_info(&self) -> AdapterInfo {
        tokio::task::spawn_blocking(probe_adapter_blocking)
            .await
            .unwrap_or_else(|e| AdapterInfo {
                is_present: false,
                address: None,
                is_classic_supported: false,
                is_low_energy_supported: false,
                is_peripheral_supported: false,
                error: Some(format!("adapter probe task panicked: {e}")),
            })
    }

    /// Emits a connection lifecycle event to all subscribers.
    pub fn publish_event(&self, event: ConnectionEvent) {
        let _ = self.events_tx.send(event);
    }
}

impl Drop for WinrtBluetoothManager {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.active_watcher.lock() {
            if let Some(active) = guard.take() {
                active.stop_and_detach();
            }
        }
    }
}

#[async_trait]
impl DeviceScanner for WinrtBluetoothManager {
    async fn start_scan(&self) -> TransportResult<()> {
        let watcher_slot = Arc::clone(&self.active_watcher);
        let events_tx = self.events_tx.clone();

        tokio::task::spawn_blocking(move || {
            let mut guard = watcher_slot
                .lock()
                .map_err(|_| TransportError::Io("watcher mutex poisoned".to_string()))?;

            if guard.is_some() {
                return Ok(());
            }

            let watcher = BluetoothLEAdvertisementWatcher::new().map_err(|e| {
                TransportError::AdapterUnavailable(format!(
                    "failed to create BLE advertisement watcher: {e}"
                ))
            })?;

            watcher
                .SetScanningMode(BluetoothLEScanningMode::Active)
                .map_err(|e| TransportError::Io(format!("SetScanningMode(Active) failed: {e}")))?;

            let handler = TypedEventHandler::<
                BluetoothLEAdvertisementWatcher,
                BluetoothLEAdvertisementReceivedEventArgs,
            >::new(move |_sender, args| {
                if let Some(args) = args.as_ref() {
                    if let Ok(device) = parse_advertisement_args(args) {
                        let _ = events_tx.send(ConnectionEvent::AdvertisementSeen(device));
                    }
                }
                Ok(())
            });

            let received_token = watcher
                .Received(&handler)
                .map_err(|e| TransportError::Io(format!("watcher.Received failed: {e}")))?;

            if let Err(e) = watcher.Start() {
                let _ = watcher.RemoveReceived(received_token);
                return Err(TransportError::AdapterUnavailable(format!(
                    "BLE watcher Start failed: {e}"
                )));
            }

            *guard = Some(ActiveWatcher {
                watcher,
                received_token,
            });
            Ok(())
        })
        .await
        .map_err(|e| TransportError::Io(format!("start_scan task join error: {e}")))?
    }

    async fn stop_scan(&self) -> TransportResult<()> {
        let watcher_slot = Arc::clone(&self.active_watcher);
        tokio::task::spawn_blocking(move || {
            let mut guard = watcher_slot
                .lock()
                .map_err(|_| TransportError::Io("watcher mutex poisoned".to_string()))?;
            if let Some(active) = guard.take() {
                active.stop_and_detach();
            }
            Ok(())
        })
        .await
        .map_err(|e| TransportError::Io(format!("stop_scan task join error: {e}")))?
    }

    async fn list_paired_devices(&self) -> TransportResult<Vec<DiscoveredDevice>> {
        tokio::task::spawn_blocking(enumerate_paired_devices_blocking)
            .await
            .map_err(|e| TransportError::Io(format!("list_paired_devices task join error: {e}")))?
    }

    fn subscribe_events(&self) -> broadcast::Receiver<ConnectionEvent> {
        self.events_tx.subscribe()
    }
}

fn probe_adapter_blocking() -> AdapterInfo {
    let op = match BluetoothAdapter::GetDefaultAsync() {
        Ok(op) => op,
        Err(e) => {
            return AdapterInfo {
                is_present: false,
                address: None,
                is_classic_supported: false,
                is_low_energy_supported: false,
                is_peripheral_supported: false,
                error: Some(format!("GetDefaultAsync failed: {e}")),
            };
        }
    };

    let adapter = match op.join() {
        Ok(adapter) => adapter,
        Err(e) => {
            return AdapterInfo {
                is_present: false,
                address: None,
                is_classic_supported: false,
                is_low_energy_supported: false,
                is_peripheral_supported: false,
                error: Some(format!("no default Bluetooth adapter available: {e}")),
            };
        }
    };

    let address = adapter.BluetoothAddress().ok().map(BluetoothAddress::new);
    let is_classic_supported = adapter.IsClassicSupported().unwrap_or(false);
    let is_low_energy_supported = adapter.IsLowEnergySupported().unwrap_or(false);
    let is_peripheral_supported = adapter.IsPeripheralRoleSupported().unwrap_or(false);

    AdapterInfo {
        is_present: true,
        address,
        is_classic_supported,
        is_low_energy_supported,
        is_peripheral_supported,
        error: None,
    }
}

fn parse_advertisement_args(
    args: &BluetoothLEAdvertisementReceivedEventArgs,
) -> TransportResult<DiscoveredDevice> {
    let address = BluetoothAddress::new(
        args.BluetoothAddress()
            .map_err(|e| TransportError::Io(e.to_string()))?,
    );
    let rssi = args.RawSignalStrengthInDBm().ok();
    let advert = args
        .Advertisement()
        .map_err(|e| TransportError::Io(e.to_string()))?;

    let name = advert
        .LocalName()
        .map(|s| s.to_string())
        .unwrap_or_default();

    let mut manufacturer_data = HashMap::new();
    if let Ok(mfg_list) = advert.ManufacturerData() {
        let count = mfg_list.Size().unwrap_or(0);
        for i in 0..count {
            if let Ok(item) = mfg_list.GetAt(i) {
                if let (Ok(company_id), Ok(data_buf)) = (item.CompanyId(), item.Data()) {
                    if let Ok(bytes) = ibuffer_to_vec(&data_buf) {
                        manufacturer_data.insert(company_id, bytes);
                    }
                }
            }
        }
    }

    let mut service_uuids = Vec::new();
    if let Ok(uuids) = advert.ServiceUuids() {
        let count = uuids.Size().unwrap_or(0);
        for i in 0..count {
            if let Ok(guid) = uuids.GetAt(i) {
                let uuid = guid_to_uuid(guid);
                if !service_uuids.contains(&uuid) {
                    service_uuids.push(uuid);
                }
            }
        }
    }

    Ok(DiscoveredDevice {
        id: format!("ble:{address}"),
        address,
        name,
        rssi,
        transport_kind: TransportKind::Ble,
        manufacturer_data,
        service_uuids,
        is_paired: false,
        is_connected: false,
        last_seen_ms: now_epoch_ms(),
    })
}

fn enumerate_paired_devices_blocking() -> TransportResult<Vec<DiscoveredDevice>> {
    let mut by_address: HashMap<BluetoothAddress, DiscoveredDevice> = HashMap::new();
    let now = now_epoch_ms();

    // 1. Paired Classic Bluetooth devices (BR/EDR)
    if let Ok(classic_selector) = BluetoothDevice::GetDeviceSelectorFromPairingState(true) {
        if let Ok(op) = DeviceInformation::FindAllAsyncAqsFilter(&classic_selector) {
            if let Ok(collection) = op.join() {
                let count = collection.Size().unwrap_or(0);
                for i in 0..count {
                    let Ok(info) = collection.GetAt(i) else {
                        continue;
                    };
                    let Ok(id) = info.Id() else { continue };
                    let fallback_name = info.Name().map(|s| s.to_string()).unwrap_or_default();

                    let Ok(dev_op) = BluetoothDevice::FromIdAsync(&id) else {
                        continue;
                    };
                    let Ok(device) = dev_op.join() else {
                        continue;
                    };

                    if let Ok(raw_addr) = device.BluetoothAddress() {
                        let address = BluetoothAddress::new(raw_addr);
                        let name = device
                            .Name()
                            .map(|s| s.to_string())
                            .ok()
                            .filter(|n| !n.is_empty())
                            .unwrap_or(fallback_name);
                        let is_connected = device
                            .ConnectionStatus()
                            .map(|s| s == BluetoothConnectionStatus::Connected)
                            .unwrap_or(false);

                        let discovered = DiscoveredDevice {
                            id: id.to_string(),
                            address,
                            name,
                            rssi: None,
                            transport_kind: TransportKind::ClassicRfcomm,
                            manufacturer_data: HashMap::new(),
                            service_uuids: Vec::new(),
                            is_paired: true,
                            is_connected,
                            last_seen_ms: now,
                        };

                        by_address
                            .entry(address)
                            .and_modify(|e| merge_discovered_device(e, discovered.clone()))
                            .or_insert(discovered);
                    }
                    let _ = device.Close();
                }
            }
        }
    }

    // 2. Paired Bluetooth Low Energy (BLE) devices
    if let Ok(ble_selector) = BluetoothLEDevice::GetDeviceSelectorFromPairingState(true) {
        if let Ok(op) = DeviceInformation::FindAllAsyncAqsFilter(&ble_selector) {
            if let Ok(collection) = op.join() {
                let count = collection.Size().unwrap_or(0);
                for i in 0..count {
                    let Ok(info) = collection.GetAt(i) else {
                        continue;
                    };
                    let Ok(id) = info.Id() else { continue };
                    let fallback_name = info.Name().map(|s| s.to_string()).unwrap_or_default();

                    let Ok(dev_op) = BluetoothLEDevice::FromIdAsync(&id) else {
                        continue;
                    };
                    let Ok(device) = dev_op.join() else {
                        continue;
                    };

                    if let Ok(raw_addr) = device.BluetoothAddress() {
                        let address = BluetoothAddress::new(raw_addr);
                        let name = device
                            .Name()
                            .map(|s| s.to_string())
                            .ok()
                            .filter(|n| !n.is_empty())
                            .unwrap_or(fallback_name);
                        let is_connected = device
                            .ConnectionStatus()
                            .map(|s| s == BluetoothConnectionStatus::Connected)
                            .unwrap_or(false);

                        let discovered = DiscoveredDevice {
                            id: id.to_string(),
                            address,
                            name,
                            rssi: None,
                            transport_kind: TransportKind::Ble,
                            manufacturer_data: HashMap::new(),
                            service_uuids: Vec::new(),
                            is_paired: true,
                            is_connected,
                            last_seen_ms: now,
                        };

                        by_address
                            .entry(address)
                            .and_modify(|e| merge_discovered_device(e, discovered.clone()))
                            .or_insert(discovered);
                    }
                    let _ = device.Close();
                }
            }
        }
    }

    let mut devices: Vec<DiscoveredDevice> = by_address.into_values().collect();
    devices.sort_by(|a, b| {
        b.is_connected
            .cmp(&a.is_connected)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(devices)
}
