use super::{WinrtBleConnection, WinrtRfcommConnection};
use crate::{
    BluetoothAddress, BluetoothConnection, GattServiceInfo, TransportError, TransportKind,
    TransportResult,
};
use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

/// Unified dual-transport Windows WinRT connection supporting both BLE GATT and Classic RFCOMM/SPP.
///
/// Protocols that use BLE only, Classic RFCOMM only, or both concurrently can drive this single
/// `BluetoothConnection` handle. Each underlying transport can be attached eagerly or opened lazily.
pub struct WinrtDualConnection {
    address: BluetoothAddress,
    ble: Mutex<Option<Arc<WinrtBleConnection>>>,
    rfcomm: Mutex<Option<Arc<WinrtRfcommConnection>>>,
    cached_mtu: AtomicUsize,
    ble_connected: AtomicBool,
    rfcomm_connected: AtomicBool,
}

impl WinrtDualConnection {
    /// Creates a lazy dual-transport handle for `address`.
    /// BLE GATT connects automatically on the first GATT call; RFCOMM opens on `open_rfcomm`.
    pub fn new(address: BluetoothAddress) -> Self {
        Self {
            address,
            ble: Mutex::new(None),
            rfcomm: Mutex::new(None),
            cached_mtu: AtomicUsize::new(23),
            ble_connected: AtomicBool::new(false),
            rfcomm_connected: AtomicBool::new(false),
        }
    }

    /// Creates a dual-transport wrapper from already-established BLE and/or RFCOMM connections.
    pub fn from_parts(
        address: BluetoothAddress,
        ble: Option<WinrtBleConnection>,
        rfcomm: Option<WinrtRfcommConnection>,
    ) -> Self {
        let ble_connected = ble.as_ref().map(|c| c.is_connected()).unwrap_or(false);
        let rfcomm_connected = rfcomm.as_ref().map(|c| c.is_connected()).unwrap_or(false);
        let mtu = ble
            .as_ref()
            .map(|c| c.mtu())
            .or_else(|| rfcomm.as_ref().map(|c| c.mtu()))
            .unwrap_or(23);

        Self {
            address,
            ble: Mutex::new(ble.map(Arc::new)),
            rfcomm: Mutex::new(rfcomm.map(Arc::new)),
            cached_mtu: AtomicUsize::new(mtu),
            ble_connected: AtomicBool::new(ble_connected),
            rfcomm_connected: AtomicBool::new(rfcomm_connected),
        }
    }

    /// Explicitly establishes the BLE GATT link if not already connected.
    pub async fn connect_ble(&self) -> TransportResult<()> {
        let _ = self.ensure_ble().await?;
        Ok(())
    }

    async fn ensure_ble(&self) -> TransportResult<Arc<WinrtBleConnection>> {
        let mut guard = self.ble.lock().await;
        if let Some(existing) = guard.as_ref() {
            if existing.is_connected() {
                self.cached_mtu.store(existing.mtu(), Ordering::SeqCst);
                self.ble_connected.store(true, Ordering::SeqCst);
                return Ok(Arc::clone(existing));
            }
        }
        let conn = Arc::new(WinrtBleConnection::connect(self.address).await?);
        self.cached_mtu.store(conn.mtu(), Ordering::SeqCst);
        self.ble_connected
            .store(conn.is_connected(), Ordering::SeqCst);
        *guard = Some(Arc::clone(&conn));
        Ok(conn)
    }

    async fn ensure_rfcomm(&self) -> Arc<WinrtRfcommConnection> {
        let mut guard = self.rfcomm.lock().await;
        if let Some(existing) = guard.as_ref() {
            return Arc::clone(existing);
        }
        let conn = Arc::new(WinrtRfcommConnection::new(self.address));
        *guard = Some(Arc::clone(&conn));
        conn
    }
}

#[async_trait]
impl BluetoothConnection for WinrtDualConnection {
    fn transport_kind(&self) -> TransportKind {
        TransportKind::Dual
    }

    fn address(&self) -> BluetoothAddress {
        self.address
    }

    fn mtu(&self) -> usize {
        self.cached_mtu.load(Ordering::SeqCst)
    }

    fn is_connected(&self) -> bool {
        self.ble_connected.load(Ordering::SeqCst) || self.rfcomm_connected.load(Ordering::SeqCst)
    }

    async fn discover_services(&self) -> TransportResult<Vec<GattServiceInfo>> {
        let ble = self.ensure_ble().await?;
        ble.discover_services().await
    }

    async fn read_gatt(&self, service: Uuid, characteristic: Uuid) -> TransportResult<Vec<u8>> {
        let ble = self.ensure_ble().await?;
        let res = ble.read_gatt(service, characteristic).await;
        self.ble_connected
            .store(ble.is_connected(), Ordering::SeqCst);
        res
    }

    async fn write_gatt(
        &self,
        service: Uuid,
        characteristic: Uuid,
        data: &[u8],
        with_response: bool,
    ) -> TransportResult<()> {
        let ble = self.ensure_ble().await?;
        let res = ble
            .write_gatt(service, characteristic, data, with_response)
            .await;
        self.ble_connected
            .store(ble.is_connected(), Ordering::SeqCst);
        res
    }

    async fn subscribe_gatt_notifications(
        &self,
        service: Uuid,
        characteristic: Uuid,
    ) -> TransportResult<mpsc::Receiver<Vec<u8>>> {
        let ble = self.ensure_ble().await?;
        let res = ble
            .subscribe_gatt_notifications(service, characteristic)
            .await;
        self.ble_connected
            .store(ble.is_connected(), Ordering::SeqCst);
        res
    }

    async fn open_rfcomm(&self, service_uuid: Uuid) -> TransportResult<()> {
        let rfcomm = self.ensure_rfcomm().await;
        rfcomm.open_rfcomm(service_uuid).await?;
        self.rfcomm_connected
            .store(rfcomm.is_connected(), Ordering::SeqCst);
        if !self.ble_connected.load(Ordering::SeqCst) {
            self.cached_mtu.store(rfcomm.mtu(), Ordering::SeqCst);
        }
        Ok(())
    }

    async fn write_rfcomm(&self, data: &[u8]) -> TransportResult<()> {
        let rfcomm = {
            let guard = self.rfcomm.lock().await;
            guard.clone().ok_or(TransportError::NotConnected)?
        };
        let res = rfcomm.write_rfcomm(data).await;
        self.rfcomm_connected
            .store(rfcomm.is_connected(), Ordering::SeqCst);
        res
    }

    async fn subscribe_rfcomm(&self) -> TransportResult<mpsc::Receiver<Vec<u8>>> {
        let rfcomm = self.ensure_rfcomm().await;
        rfcomm.subscribe_rfcomm().await
    }

    async fn disconnect(&self) -> TransportResult<()> {
        self.ble_connected.store(false, Ordering::SeqCst);
        self.rfcomm_connected.store(false, Ordering::SeqCst);

        let ble = self.ble.lock().await.take();
        if let Some(conn) = ble {
            let _ = conn.disconnect().await;
        }

        let rfcomm = self.rfcomm.lock().await.take();
        if let Some(conn) = rfcomm {
            let _ = conn.disconnect().await;
        }

        Ok(())
    }
}
