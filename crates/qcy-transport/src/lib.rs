use async_trait::async_trait;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::sync::{broadcast, mpsc};

pub use uuid::Uuid;

#[cfg(target_os = "windows")]
pub mod winrt;

#[cfg(target_os = "windows")]
pub use winrt::{
    open_windows_bluetooth_settings, WinrtBleConnection, WinrtBluetoothManager,
    WinrtDualConnection, WinrtRfcommConnection,
};

/// Standard Bluetooth Classic Serial Port Profile (SPP) UUID (`00001101-0000-1000-8000-00805F9B34FB`).
pub const SPP_SERVICE_UUID: Uuid = Uuid::from_u128(0x0000_1101_0000_1000_8000_0080_5F9B_34FB);

/// Bluetooth Base UUID for 16-bit and 32-bit short UUID expansion.
const BLUETOOTH_BASE_UUID_U128: u128 = 0x0000_0000_0000_1000_8000_0080_5F9B_34FB;

/// Expands a 16-bit Bluetooth SIG short UUID (e.g., `0x180F`) into a full 128-bit UUID.
pub const fn short_uuid_to_128(short_id: u16) -> Uuid {
    Uuid::from_u128(BLUETOOTH_BASE_UUID_U128 | ((short_id as u128) << 96))
}

/// Returns current UNIX epoch time in milliseconds.
pub fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportKind {
    Ble,
    ClassicRfcomm,
    Dual,
}

/// 48-bit Bluetooth MAC address stored in the lower 6 bytes of a `u64` (big-endian octet order).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct BluetoothAddress(u64);

impl BluetoothAddress {
    pub const MASK_48: u64 = 0x0000_FFFF_FFFF_FFFF;

    pub const fn new(raw: u64) -> Self {
        Self(raw & Self::MASK_48)
    }

    pub const fn as_u64(self) -> u64 {
        self.0
    }

    pub const fn from_bytes(bytes: [u8; 6]) -> Self {
        let raw = ((bytes[0] as u64) << 40)
            | ((bytes[1] as u64) << 32)
            | ((bytes[2] as u64) << 24)
            | ((bytes[3] as u64) << 16)
            | ((bytes[4] as u64) << 8)
            | (bytes[5] as u64);
        Self(raw)
    }

    pub const fn to_bytes(self) -> [u8; 6] {
        [
            ((self.0 >> 40) & 0xFF) as u8,
            ((self.0 >> 32) & 0xFF) as u8,
            ((self.0 >> 24) & 0xFF) as u8,
            ((self.0 >> 16) & 0xFF) as u8,
            ((self.0 >> 8) & 0xFF) as u8,
            (self.0 & 0xFF) as u8,
        ]
    }

    /// Formats as canonical uppercase colon-separated MAC (`AA:BB:CC:DD:EE:FF`).
    pub fn to_mac_string(self) -> String {
        self.to_string()
    }
}

impl fmt::Display for BluetoothAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let b = self.to_bytes();
        write!(
            f,
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            b[0], b[1], b[2], b[3], b[4], b[5]
        )
    }
}

impl FromStr for BluetoothAddress {
    type Err = TransportError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        let bytes = if trimmed.contains(':') || trimmed.contains('-') {
            let sep = if trimmed.contains(':') { ':' } else { '-' };
            let parts: Vec<&str> = trimmed.split(sep).collect();
            if parts.len() != 6 || parts.iter().any(|p| p.len() != 2) {
                return Err(TransportError::InvalidAddress(trimmed.to_string()));
            }
            let mut out = [0u8; 6];
            for (idx, part) in parts.iter().enumerate() {
                out[idx] = u8::from_str_radix(part, 16)
                    .map_err(|_| TransportError::InvalidAddress(trimmed.to_string()))?;
            }
            out
        } else if trimmed.len() == 12 {
            let mut out = [0u8; 6];
            for idx in 0..6 {
                let slice = &trimmed[idx * 2..idx * 2 + 2];
                out[idx] = u8::from_str_radix(slice, 16)
                    .map_err(|_| TransportError::InvalidAddress(trimmed.to_string()))?;
            }
            out
        } else {
            return Err(TransportError::InvalidAddress(trimmed.to_string()));
        };
        Ok(Self::from_bytes(bytes))
    }
}

impl From<u64> for BluetoothAddress {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

impl Serialize for BluetoothAddress {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_mac_string())
    }
}

impl<'de> Deserialize<'de> for BluetoothAddress {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum AddrRepr {
            Str(String),
            Num(u64),
        }

        match AddrRepr::deserialize(deserializer)? {
            AddrRepr::Str(s) => BluetoothAddress::from_str(&s).map_err(serde::de::Error::custom),
            AddrRepr::Num(n) => Ok(BluetoothAddress::new(n)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredDevice {
    pub id: String,
    pub address: BluetoothAddress,
    pub name: String,
    pub rssi: Option<i16>,
    pub transport_kind: TransportKind,
    pub manufacturer_data: HashMap<u16, Vec<u8>>,
    pub service_uuids: Vec<Uuid>,
    pub is_paired: bool,
    pub is_connected: bool,
    pub last_seen_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConnectionEvent {
    Connected {
        address: BluetoothAddress,
    },
    Disconnected {
        address: BluetoothAddress,
        reason: String,
    },
    AdvertisementSeen(DiscoveredDevice),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterInfo {
    pub is_present: bool,
    pub address: Option<BluetoothAddress>,
    pub is_classic_supported: bool,
    pub is_low_energy_supported: bool,
    pub is_peripheral_supported: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct GattProperties {
    pub read: bool,
    pub write: bool,
    pub write_without_response: bool,
    pub notify: bool,
    pub indicate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GattCharacteristicInfo {
    pub uuid: Uuid,
    pub properties: GattProperties,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GattServiceInfo {
    pub uuid: Uuid,
    pub characteristics: Vec<GattCharacteristicInfo>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TransportError {
    #[error("bluetooth adapter unavailable: {0}")]
    AdapterUnavailable(String),
    #[error("invalid bluetooth address: {0}")]
    InvalidAddress(String),
    #[error("device not found: {0}")]
    DeviceNotFound(String),
    #[error("service not found: {0}")]
    ServiceNotFound(String),
    #[error("characteristic not found: {0}")]
    CharacteristicNotFound(String),
    #[error("connection failed: {0}")]
    ConnectionFailed(String),
    #[error("device is not connected")]
    NotConnected,
    #[error("io error: {0}")]
    Io(String),
    #[error("unsupported transport operation: {0}")]
    Unsupported(String),
}

pub type TransportResult<T> = Result<T, TransportError>;

/// Extracts the raw manufacturer-specific payload for a given Bluetooth SIG company ID.
pub fn extract_manufacturer_payload(device: &DiscoveredDevice, company_id: u16) -> Option<&[u8]> {
    device.manufacturer_data.get(&company_id).map(Vec::as_slice)
}

/// Checks whether a discovered device advertised or exposed a specific GATT/SPP UUID.
pub fn has_service_uuid(device: &DiscoveredDevice, uuid: Uuid) -> bool {
    device.service_uuids.contains(&uuid)
}

/// Merges two discoveries for the same Bluetooth address (e.g. BLE + Classic paired records).
pub fn merge_discovered_device(existing: &mut DiscoveredDevice, incoming: DiscoveredDevice) {
    if existing.transport_kind != incoming.transport_kind {
        existing.transport_kind = TransportKind::Dual;
    }
    if existing.name.is_empty() && !incoming.name.is_empty() {
        existing.name = incoming.name;
    }
    if incoming.rssi.is_some() {
        existing.rssi = incoming.rssi;
    }
    existing.is_paired |= incoming.is_paired;
    existing.is_connected |= incoming.is_connected;
    existing.last_seen_ms = existing.last_seen_ms.max(incoming.last_seen_ms);
    for (company_id, payload) in incoming.manufacturer_data {
        existing.manufacturer_data.insert(company_id, payload);
    }
    for uuid in incoming.service_uuids {
        if !existing.service_uuids.contains(&uuid) {
            existing.service_uuids.push(uuid);
        }
    }
}

#[async_trait]
pub trait DeviceScanner: Send + Sync {
    async fn start_scan(&self) -> TransportResult<()>;
    async fn stop_scan(&self) -> TransportResult<()>;
    async fn list_paired_devices(&self) -> TransportResult<Vec<DiscoveredDevice>>;
    fn subscribe_events(&self) -> broadcast::Receiver<ConnectionEvent>;
}

#[async_trait]
pub trait BluetoothConnection: Send + Sync {
    fn transport_kind(&self) -> TransportKind;
    fn address(&self) -> BluetoothAddress;
    fn mtu(&self) -> usize;
    fn is_connected(&self) -> bool;

    async fn discover_services(&self) -> TransportResult<Vec<GattServiceInfo>>;

    async fn read_gatt(&self, service: Uuid, characteristic: Uuid) -> TransportResult<Vec<u8>>;

    async fn write_gatt(
        &self,
        service: Uuid,
        characteristic: Uuid,
        data: &[u8],
        with_response: bool,
    ) -> TransportResult<()>;

    async fn subscribe_gatt_notifications(
        &self,
        service: Uuid,
        characteristic: Uuid,
    ) -> TransportResult<mpsc::Receiver<Vec<u8>>>;

    async fn open_rfcomm(&self, service_uuid: Uuid) -> TransportResult<()>;

    async fn write_rfcomm(&self, data: &[u8]) -> TransportResult<()>;

    async fn subscribe_rfcomm(&self) -> TransportResult<mpsc::Receiver<Vec<u8>>>;

    async fn disconnect(&self) -> TransportResult<()>;
}

#[cfg(not(target_os = "windows"))]
pub fn open_windows_bluetooth_settings() -> TransportResult<()> {
    Err(TransportError::Unsupported(
        "Windows Bluetooth settings are only available on Windows".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bluetooth_address_format_and_parse_variants() {
        let addr = BluetoothAddress::from_str("aa:bb:cc:dd:ee:ff").expect("colon parse");
        assert_eq!(addr.to_mac_string(), "AA:BB:CC:DD:EE:FF");
        assert_eq!(addr.as_u64(), 0x0000_AABB_CCDD_EEFF);
        assert_eq!(addr.to_bytes(), [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);

        let dash = BluetoothAddress::from_str("AA-BB-CC-DD-EE-FF").expect("dash parse");
        let compact = BluetoothAddress::from_str("aabbccddeeff").expect("compact parse");
        assert_eq!(addr, dash);
        assert_eq!(addr, compact);

        // Masks upper 16 bits cleanly
        let masked = BluetoothAddress::new(0xFFFF_1122_3344_5566);
        assert_eq!(masked.to_mac_string(), "11:22:33:44:55:66");

        // Rejects malformed addresses
        assert!(BluetoothAddress::from_str("AA:BB:CC").is_err());
        assert!(BluetoothAddress::from_str("ZZ:BB:CC:DD:EE:FF").is_err());
        assert!(BluetoothAddress::from_str("AA:B:CC:DD:EE:FF").is_err());
        assert!(BluetoothAddress::from_str("").is_err());
    }

    #[test]
    fn bluetooth_address_serde_accepts_string_and_u64() {
        let addr = BluetoothAddress::from_str("00:1A:7D:DA:71:11").expect("parse");
        let json = serde_json::to_string(&addr).expect("serialize");
        assert_eq!(json, "\"00:1A:7D:DA:71:11\"");

        let from_str: BluetoothAddress = serde_json::from_str(&json).expect("deser str");
        let from_num: BluetoothAddress =
            serde_json::from_str(&addr.as_u64().to_string()).expect("deser u64");
        assert_eq!(addr, from_str);
        assert_eq!(addr, from_num);
    }

    #[test]
    fn short_uuid_expansion_matches_spp() {
        assert_eq!(short_uuid_to_128(0x1101), SPP_SERVICE_UUID);
        assert_eq!(
            short_uuid_to_128(0x180F).to_string(),
            "0000180f-0000-1000-8000-00805f9b34fb"
        );
    }

    #[test]
    fn advertisement_helpers_and_dual_merge() {
        let addr = BluetoothAddress::from_str("51:43:59:11:22:33").expect("parse");
        let mut mfg = HashMap::new();
        mfg.insert(0x05D6, vec![0x01, 0x02, 0x5A, 0x64]);
        let svc_a = short_uuid_to_128(0xAE00);
        let svc_b = SPP_SERVICE_UUID;

        let mut ble_dev = DiscoveredDevice {
            id: format!("ble:{addr}"),
            address: addr,
            name: String::new(),
            rssi: Some(-54),
            transport_kind: TransportKind::Ble,
            manufacturer_data: mfg,
            service_uuids: vec![svc_a],
            is_paired: false,
            is_connected: false,
            last_seen_ms: 1000,
        };

        assert_eq!(
            extract_manufacturer_payload(&ble_dev, 0x05D6),
            Some(&[0x01, 0x02, 0x5A, 0x64][..])
        );
        assert_eq!(extract_manufacturer_payload(&ble_dev, 0xFFFF), None);
        assert!(has_service_uuid(&ble_dev, svc_a));
        assert!(!has_service_uuid(&ble_dev, svc_b));

        let classic_dev = DiscoveredDevice {
            id: format!("classic:{addr}"),
            address: addr,
            name: "QCY MeloBuds Pro".to_string(),
            rssi: None,
            transport_kind: TransportKind::ClassicRfcomm,
            manufacturer_data: HashMap::new(),
            service_uuids: vec![svc_b],
            is_paired: true,
            is_connected: true,
            last_seen_ms: 2000,
        };

        merge_discovered_device(&mut ble_dev, classic_dev);
        assert_eq!(ble_dev.transport_kind, TransportKind::Dual);
        assert_eq!(ble_dev.name, "QCY MeloBuds Pro");
        assert_eq!(ble_dev.rssi, Some(-54));
        assert!(ble_dev.is_paired);
        assert!(ble_dev.is_connected);
        assert_eq!(ble_dev.last_seen_ms, 2000);
        assert!(has_service_uuid(&ble_dev, svc_a));
        assert!(has_service_uuid(&ble_dev, svc_b));
        assert_eq!(
            extract_manufacturer_payload(&ble_dev, 0x05D6),
            Some(&[0x01, 0x02, 0x5A, 0x64][..])
        );
    }

    #[test]
    fn transport_error_display_messages() {
        let err = TransportError::InvalidAddress("bad".to_string());
        assert_eq!(err.to_string(), "invalid bluetooth address: bad");
        assert_eq!(
            TransportError::NotConnected.to_string(),
            "device is not connected"
        );
    }

    #[test]
    fn discovered_device_roundtrip_json() {
        let mut mfg = HashMap::new();
        mfg.insert(0x004C, vec![0xDE, 0xAD]);
        let dev = DiscoveredDevice {
            id: "virtual-001".to_string(),
            address: BluetoothAddress::from_str("AA:BB:CC:DD:EE:FF").expect("parse"),
            name: "QCY Virtual Earbuds".to_string(),
            rssi: Some(-52),
            transport_kind: TransportKind::Dual,
            manufacturer_data: mfg,
            service_uuids: vec![SPP_SERVICE_UUID],
            is_paired: true,
            is_connected: false,
            last_seen_ms: 1_700_000_000_000,
        };
        let json = serde_json::to_string(&dev).expect("serialize");
        let parsed: DiscoveredDevice = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(dev, parsed);
    }
}
