use super::{guid_to_uuid, uuid_to_guid};
use crate::{
    BluetoothAddress, BluetoothConnection, GattServiceInfo, TransportError, TransportKind,
    TransportResult, SPP_SERVICE_UUID,
};
use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use uuid::Uuid;
use windows::Devices::Bluetooth::Rfcomm::{RfcommDeviceService, RfcommServiceId};
use windows::Devices::Bluetooth::{BluetoothCacheMode, BluetoothDevice};
use windows::Networking::Sockets::StreamSocket;
use windows::Storage::Streams::{DataReader, DataWriter, InputStreamOptions};

const RFCOMM_READ_CHUNK_SIZE: u32 = 4096;
const DEFAULT_RFCOMM_MTU: usize = 672;

struct RfcommHandles {
    device: Option<BluetoothDevice>,
    service: Option<RfcommDeviceService>,
    socket: Option<StreamSocket>,
    writer: Option<DataWriter>,
}

impl RfcommHandles {
    fn close_all(&mut self) {
        if let Some(writer) = self.writer.take() {
            let _ = writer.DetachStream();
            let _ = writer.Close();
        }
        if let Some(socket) = self.socket.take() {
            let _ = socket.Close();
        }
        if let Some(service) = self.service.take() {
            let _ = service.Close();
        }
        if let Some(device) = self.device.take() {
            let _ = device.Close();
        }
    }
}

/// Windows WinRT Classic Bluetooth RFCOMM / SPP connection.
pub struct WinrtRfcommConnection {
    address: BluetoothAddress,
    connected: Arc<AtomicBool>,
    handles: Arc<Mutex<RfcommHandles>>,
    rx_subscribers: Arc<Mutex<Vec<mpsc::Sender<Vec<u8>>>>>,
}

impl WinrtRfcommConnection {
    /// Creates an unbound Classic Bluetooth RFCOMM handle for the target MAC address.
    /// Call `open_rfcomm(service_uuid)` to resolve the SDP record and open the RFCOMM stream.
    pub fn new(address: BluetoothAddress) -> Self {
        Self {
            address,
            connected: Arc::new(AtomicBool::new(false)),
            handles: Arc::new(Mutex::new(RfcommHandles {
                device: None,
                service: None,
                socket: None,
                writer: None,
            })),
            rx_subscribers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Convenience constructor that immediately opens an RFCOMM stream for `service_uuid`.
    pub async fn connect(address: BluetoothAddress, service_uuid: Uuid) -> TransportResult<Self> {
        let conn = Self::new(address);
        conn.open_rfcomm(service_uuid).await?;
        Ok(conn)
    }
}

impl Drop for WinrtRfcommConnection {
    fn drop(&mut self) {
        self.connected.store(false, Ordering::SeqCst);
        if let Ok(mut guard) = self.handles.lock() {
            guard.close_all();
        }
        if let Ok(mut subs) = self.rx_subscribers.lock() {
            subs.clear();
        }
    }
}

#[async_trait]
impl BluetoothConnection for WinrtRfcommConnection {
    fn transport_kind(&self) -> TransportKind {
        TransportKind::ClassicRfcomm
    }

    fn address(&self) -> BluetoothAddress {
        self.address
    }

    fn mtu(&self) -> usize {
        DEFAULT_RFCOMM_MTU
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    async fn discover_services(&self) -> TransportResult<Vec<GattServiceInfo>> {
        Err(TransportError::Unsupported(
            "WinrtRfcommConnection does not support BLE GATT discovery".to_string(),
        ))
    }

    async fn read_gatt(&self, _service: Uuid, _characteristic: Uuid) -> TransportResult<Vec<u8>> {
        Err(TransportError::Unsupported(
            "WinrtRfcommConnection does not support BLE GATT".to_string(),
        ))
    }

    async fn write_gatt(
        &self,
        _service: Uuid,
        _characteristic: Uuid,
        _data: &[u8],
        _with_response: bool,
    ) -> TransportResult<()> {
        Err(TransportError::Unsupported(
            "WinrtRfcommConnection does not support BLE GATT".to_string(),
        ))
    }

    async fn subscribe_gatt_notifications(
        &self,
        _service: Uuid,
        _characteristic: Uuid,
    ) -> TransportResult<mpsc::Receiver<Vec<u8>>> {
        Err(TransportError::Unsupported(
            "WinrtRfcommConnection does not support BLE GATT".to_string(),
        ))
    }

    async fn open_rfcomm(&self, service_uuid: Uuid) -> TransportResult<()> {
        let address = self.address;
        let handles = Arc::clone(&self.handles);
        let connected = Arc::clone(&self.connected);
        let subscribers = Arc::clone(&self.rx_subscribers);

        let reader = tokio::task::spawn_blocking(move || -> TransportResult<DataReader> {
            // Close any previous RFCOMM session cleanly before re-opening.
            if let Ok(mut guard) = handles.lock() {
                guard.close_all();
            }

            let device = BluetoothDevice::FromBluetoothAddressAsync(address.as_u64())
                .map_err(|e| {
                    TransportError::ConnectionFailed(format!(
                        "BluetoothDevice::FromBluetoothAddressAsync({address}) failed: {e}"
                    ))
                })?
                .join()
                .map_err(|e| {
                    TransportError::DeviceNotFound(format!(
                        "Classic Bluetooth device {address} not found (ensure device is paired in Windows Settings): {e}"
                    ))
                })?;

            let service = resolve_rfcomm_service(&device, service_uuid)?;

            let host_name = service.ConnectionHostName().map_err(|e| {
                TransportError::ConnectionFailed(format!("RFCOMM ConnectionHostName failed: {e}"))
            })?;
            let service_name = service.ConnectionServiceName().map_err(|e| {
                TransportError::ConnectionFailed(format!(
                    "RFCOMM ConnectionServiceName failed: {e}"
                ))
            })?;
            let protection = service.ProtectionLevel().map_err(|e| {
                TransportError::ConnectionFailed(format!("RFCOMM ProtectionLevel failed: {e}"))
            })?;

            let socket = StreamSocket::new().map_err(|e| {
                TransportError::ConnectionFailed(format!("StreamSocket::new failed: {e}"))
            })?;

            socket
                .ConnectWithProtectionLevelAsync(&host_name, &service_name, protection)
                .map_err(|e| {
                    TransportError::ConnectionFailed(format!(
                        "StreamSocket::ConnectAsync({address}, {service_uuid}) failed: {e}"
                    ))
                })?
                .join()
                .map_err(|e| {
                    TransportError::ConnectionFailed(format!(
                        "RFCOMM connect to {address} ({service_uuid}) failed: {e}"
                    ))
                })?;

            let output_stream = socket.OutputStream().map_err(|e| {
                TransportError::Io(format!("StreamSocket::OutputStream failed: {e}"))
            })?;
            let writer = DataWriter::CreateDataWriter(&output_stream).map_err(|e| {
                TransportError::Io(format!("DataWriter::CreateDataWriter failed: {e}"))
            })?;

            let input_stream = socket.InputStream().map_err(|e| {
                TransportError::Io(format!("StreamSocket::InputStream failed: {e}"))
            })?;
            let reader = DataReader::CreateDataReader(&input_stream).map_err(|e| {
                TransportError::Io(format!("DataReader::CreateDataReader failed: {e}"))
            })?;
            reader
                .SetInputStreamOptions(InputStreamOptions::Partial)
                .map_err(|e| {
                    TransportError::Io(format!(
                        "DataReader::SetInputStreamOptions(Partial) failed: {e}"
                    ))
                })?;

            {
                let mut guard = handles
                    .lock()
                    .map_err(|_| TransportError::Io("RFCOMM mutex poisoned".to_string()))?;
                guard.device = Some(device);
                guard.service = Some(service);
                guard.socket = Some(socket);
                guard.writer = Some(writer);
            }
            connected.store(true, Ordering::SeqCst);

            Ok(reader)
        })
        .await
        .map_err(|e| TransportError::ConnectionFailed(format!("open_rfcomm task failed: {e}")))??;

        // Spawn dedicated blocking reader loop forwarding partial chunks to all mpsc subscribers.
        let reader_connected = Arc::clone(&self.connected);
        let _ = std::thread::Builder::new()
            .name(format!("qcy-rfcomm-rx-{address}"))
            .spawn(move || {
                while reader_connected.load(Ordering::SeqCst) {
                    let loaded = match reader.LoadAsync(RFCOMM_READ_CHUNK_SIZE) {
                        Ok(op) => match op.join() {
                            Ok(n) => n,
                            Err(_) => break,
                        },
                        Err(_) => break,
                    };

                    if loaded == 0 {
                        // Remote endpoint closed the RFCOMM stream cleanly.
                        break;
                    }

                    let available = reader.UnconsumedBufferLength().unwrap_or(0) as usize;
                    if available == 0 {
                        continue;
                    }
                    let mut buf = vec![0u8; available];
                    if reader.ReadBytes(&mut buf).is_err() {
                        break;
                    }

                    if let Ok(mut subs) = subscribers.lock() {
                        subs.retain(|tx| !tx.is_closed());
                        for tx in subs.iter() {
                            let _ = tx.try_send(buf.clone());
                        }
                    }
                }

                reader_connected.store(false, Ordering::SeqCst);
                let _ = reader.DetachStream();
                let _ = reader.Close();
            });

        Ok(())
    }

    async fn write_rfcomm(&self, data: &[u8]) -> TransportResult<()> {
        if !self.is_connected() {
            return Err(TransportError::NotConnected);
        }
        let handles = Arc::clone(&self.handles);
        let payload = data.to_vec();

        tokio::task::spawn_blocking(move || {
            let guard = handles
                .lock()
                .map_err(|_| TransportError::Io("RFCOMM mutex poisoned".to_string()))?;
            let writer = guard.writer.as_ref().ok_or(TransportError::NotConnected)?;

            if !payload.is_empty() {
                writer
                    .WriteBytes(&payload)
                    .map_err(|e| TransportError::Io(format!("RFCOMM WriteBytes failed: {e}")))?;
                writer
                    .StoreAsync()
                    .map_err(|e| TransportError::Io(format!("RFCOMM StoreAsync failed: {e}")))?
                    .join()
                    .map_err(|e| {
                        TransportError::Io(format!("RFCOMM StoreAsync join failed: {e}"))
                    })?;
                writer
                    .FlushAsync()
                    .map_err(|e| TransportError::Io(format!("RFCOMM FlushAsync failed: {e}")))?
                    .join()
                    .map_err(|e| {
                        TransportError::Io(format!("RFCOMM FlushAsync join failed: {e}"))
                    })?;
            }
            Ok(())
        })
        .await
        .map_err(|e| TransportError::Io(format!("write_rfcomm task join error: {e}")))?
    }

    async fn subscribe_rfcomm(&self) -> TransportResult<mpsc::Receiver<Vec<u8>>> {
        let (tx, rx) = mpsc::channel(128);
        let mut subs = self
            .rx_subscribers
            .lock()
            .map_err(|_| TransportError::Io("RFCOMM subscribers mutex poisoned".to_string()))?;
        subs.retain(|s| !s.is_closed());
        subs.push(tx);
        Ok(rx)
    }

    async fn disconnect(&self) -> TransportResult<()> {
        self.connected.store(false, Ordering::SeqCst);
        let handles = Arc::clone(&self.handles);
        let subscribers = Arc::clone(&self.rx_subscribers);

        tokio::task::spawn_blocking(move || {
            if let Ok(mut guard) = handles.lock() {
                guard.close_all();
            }
            if let Ok(mut subs) = subscribers.lock() {
                subs.clear();
            }
        })
        .await
        .map_err(|e| TransportError::Io(format!("RFCOMM disconnect task join error: {e}")))?;
        Ok(())
    }
}

fn resolve_rfcomm_service(
    device: &BluetoothDevice,
    service_uuid: Uuid,
) -> TransportResult<RfcommDeviceService> {
    let rfcomm_id = if service_uuid == SPP_SERVICE_UUID {
        RfcommServiceId::SerialPort().map_err(|e| TransportError::Io(e.to_string()))?
    } else {
        RfcommServiceId::FromUuid(uuid_to_guid(service_uuid))
            .map_err(|e| TransportError::Io(e.to_string()))?
    };

    // 1. Targeted SDP lookup by RFCOMM ServiceId (uncached).
    if let Ok(op) =
        device.GetRfcommServicesForIdWithCacheModeAsync(&rfcomm_id, BluetoothCacheMode::Uncached)
    {
        if let Ok(res) = op.join() {
            if let Ok(list) = res.Services() {
                if list.Size().unwrap_or(0) > 0 {
                    if let Ok(svc) = list.GetAt(0) {
                        return Ok(svc);
                    }
                }
            }
        }
    }

    // 2. Full RFCOMM service enumeration fallback (handles custom vendor SPP UUIDs).
    let all_res = device
        .GetRfcommServicesWithCacheModeAsync(BluetoothCacheMode::Uncached)
        .map_err(|e| TransportError::Io(format!("GetRfcommServicesAsync failed: {e}")))?
        .join()
        .map_err(|e| TransportError::Io(format!("GetRfcommServicesAsync join failed: {e}")))?;

    let services = all_res
        .Services()
        .map_err(|e| TransportError::Io(format!("RFCOMM Services() failed: {e}")))?;
    let count = services.Size().unwrap_or(0);

    let mut matched = None;
    for i in 0..count {
        let Ok(svc) = services.GetAt(i) else {
            continue;
        };
        let is_match = svc
            .ServiceId()
            .and_then(|id| id.Uuid())
            .map(|g| guid_to_uuid(g) == service_uuid)
            .unwrap_or(false);
        if is_match && matched.is_none() {
            matched = Some(svc);
        } else {
            let _ = svc.Close();
        }
    }

    matched.ok_or_else(|| {
        TransportError::ServiceNotFound(format!(
            "RFCOMM service {service_uuid} not found on device"
        ))
    })
}
