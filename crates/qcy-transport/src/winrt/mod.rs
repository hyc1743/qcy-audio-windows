//! Windows WinRT Bluetooth transport implementation (`windows = "0.62"`).
//! Supports dual-mode BLE GATT + BLE Active Advertisements and Classic Bluetooth RFCOMM/SPP.

pub mod ble;
pub mod dual;
pub mod manager;
pub mod rfcomm;

pub use ble::WinrtBleConnection;
pub use dual::WinrtDualConnection;
pub use manager::WinrtBluetoothManager;
pub use rfcomm::WinrtRfcommConnection;

use crate::{TransportError, TransportResult};
use std::process::Command;
use uuid::Uuid;
use windows::core::GUID;
use windows::Storage::Streams::{DataReader, DataWriter, IBuffer};

/// Converts a `uuid::Uuid` to a WinRT `GUID`.
pub(crate) fn uuid_to_guid(uuid: Uuid) -> GUID {
    GUID::from_u128(uuid.as_u128())
}

/// Converts a WinRT `GUID` to a `uuid::Uuid`.
pub(crate) fn guid_to_uuid(guid: GUID) -> Uuid {
    Uuid::from_u128(guid.to_u128())
}

/// Copies all bytes out of a WinRT `IBuffer`.
pub(crate) fn ibuffer_to_vec(buffer: &IBuffer) -> TransportResult<Vec<u8>> {
    let reader = DataReader::FromBuffer(buffer)
        .map_err(|e| TransportError::Io(format!("DataReader::FromBuffer failed: {e}")))?;
    let len = reader
        .UnconsumedBufferLength()
        .map_err(|e| TransportError::Io(format!("UnconsumedBufferLength failed: {e}")))?
        as usize;
    let mut bytes = vec![0u8; len];
    if len > 0 {
        reader
            .ReadBytes(&mut bytes)
            .map_err(|e| TransportError::Io(format!("DataReader::ReadBytes failed: {e}")))?;
    }
    let _ = reader.Close();
    Ok(bytes)
}

/// Packs a byte slice into a WinRT `IBuffer`.
pub(crate) fn slice_to_ibuffer(data: &[u8]) -> TransportResult<IBuffer> {
    let writer = DataWriter::new()
        .map_err(|e| TransportError::Io(format!("DataWriter::new failed: {e}")))?;
    if !data.is_empty() {
        writer
            .WriteBytes(data)
            .map_err(|e| TransportError::Io(format!("DataWriter::WriteBytes failed: {e}")))?;
    }
    let buf = writer
        .DetachBuffer()
        .map_err(|e| TransportError::Io(format!("DataWriter::DetachBuffer failed: {e}")))?;
    let _ = writer.Close();
    Ok(buf)
}

/// Opens Windows Settings directly to the Bluetooth & Devices page (`ms-settings:bluetooth`).
pub fn open_windows_bluetooth_settings() -> TransportResult<()> {
    Command::new("explorer.exe")
        .arg("ms-settings:bluetooth")
        .spawn()
        .map_err(|e| {
            TransportError::Io(format!("failed to open Windows Bluetooth settings: {e}"))
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DeviceScanner, SPP_SERVICE_UUID};

    #[test]
    fn guid_uuid_roundtrip() {
        let guid = uuid_to_guid(SPP_SERVICE_UUID);
        let back = guid_to_uuid(guid);
        assert_eq!(back, SPP_SERVICE_UUID);
    }

    #[test]
    fn ibuffer_roundtrip_bytes() {
        let input = [0x01u8, 0xFE, 0xA5, 0x00, 0xFF];
        let buf = slice_to_ibuffer(&input).expect("pack ibuffer");
        let out = ibuffer_to_vec(&buf).expect("unpack ibuffer");
        assert_eq!(out, input);
    }

    #[tokio::test]
    async fn winrt_manager_adapter_info_and_paired_devices_smoke() {
        let manager = WinrtBluetoothManager::new();
        let info = manager.adapter_info().await;
        // Must never panic; if a radio is present, address must be populated.
        if info.is_present {
            assert!(info.address.is_some());
            assert!(info.is_classic_supported || info.is_low_energy_supported);
        }

        // Listing paired devices must complete cleanly without panicking.
        let paired = manager.list_paired_devices().await;
        assert!(paired.is_ok(), "list_paired_devices failed: {paired:?}");

        // If a BLE-capable adapter is present, verify active scan start/stop lifecycle works cleanly.
        if info.is_present && info.is_low_energy_supported {
            let _rx = manager.subscribe_events();
            manager.start_scan().await.expect("start_scan");
            manager.stop_scan().await.expect("stop_scan");
        }
    }
}
