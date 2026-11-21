//! Production Layer 4 QCY Desktop application engine (`AppCore`).
//!
//! Orchestrates:
//! - Dual transport backends: Real Windows WinRT BLE/RFCOMM (`qcy-transport`)
//!   AND the raw-byte Virtual QCY Earbuds Simulator (`qcy-simulator`)
//! - Real wire framing via `qcy-protocol` (`ProtocolRouter` / `ProtocolDriver`)
//!   with 100 ms command pacing on live BLE, background notification decoding,
//!   and TX/RX hex protocol logging
//! - Passive 24B BLE `QcyAdvertisement` battery preview & lid-open popup triggering
//! - `popViewRules` mutual-exclusion & reboot guard engine (`ModelRegistry`)
//! - Local `.ota`/`.bin` firmware flashing with transparent `otaType=9` notice
//! - Atomic JSON persistence (`%APPDATA%\QcyAudioDesktop\state.json`)

use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use qcy_models::{DeviceModelProfile, ModelRegistry};
use qcy_protocol::{
    ButtonAction, ButtonBinding, CmdId, DeviceCommand, EarSide, GameModeProConfig,
    GraphicEqSetting, HeadGestureAction, HeadGestureBinding, HeadGestureEvent, HearingHealthConfig,
    InEarMusicBehavior, LedEffectConfig, ListeningMode, MultipointHostAction, OtaError,
    OtaPreFlight, OtaSession, OtaTransport, OtaType, ParametricEqBand, ParametricEqPreset,
    PrivacyModeProConfig, ProtocolDriver, ProtocolRouter, QcyAdvertisement, SpatialAudioMode,
    TouchSensitivity, WearConfig, WearSensitivity, WireWrite, COMMAND_PACING_MS, COMPANY_ID_QCY,
    DEFAULT_EQ_FREQS_10, QCY_COMPANY_IDS,
};
use qcy_simulator::{VirtualDeviceState, VirtualScanner};
use qcy_transport::{
    now_epoch_ms, AdapterInfo, BluetoothAddress, BluetoothConnection, ConnectionEvent,
    ConnectionState, DeviceScanner, DiscoveredDevice as RawDiscoveredDevice, TransportKind,
};
use tokio::sync::{broadcast, Mutex as AsyncMutex};
use tokio::task::JoinHandle;
use uuid::Uuid;

pub mod state_sync;
pub mod storage;
pub mod types;

pub use storage::{
    is_simulator_saved_device, PersistedState, StorageStore, CURRENT_SCHEMA_VERSION,
};
pub use types::{
    AiAncActivityUi, AiAncMatrix, AiAncNoiseLevel, AiAncNoiseRow, AncPrimaryMode, AncScene,
    AncState, AppSettings, AppSettingsPatch, AppSnapshot, AppStatusSnapshot, AppTheme,
    BatteryPatch, BatteryTelemetry, ButtonMappingEntry, CommandOutcome, CoreError, CoreEvent,
    CoreResult, DiscoveredDeviceInfo, EqModeUi, EqPreset, EqPresetItem, EqState, FitTestEarStatus,
    FitTestResult, FocusWhiteNoiseState, GestureState, HeadControlsState, LastConnectedRecord,
    ListeningModeUi, ModesState, MultipointHostInfo, MultipointState, MutualExclusionPrompt,
    OtaPhaseUi, OtaState, PopupTriggerPayload, PromptLanguageUi, ProtocolLogEntry, ResetDeviceKind,
    RgbLightingState, SavedDeviceRecord, SetAncPayload, SetAudioEnhancersPayload,
    SetHeadControlsPatch, SetModesPayload, SetMultipointPayload, SetRgbLightingPayload,
    SimulatorControlState, SimulatorPatch, SpatialAudioModeUi, SpeakerRgbMode, UiConflictCondition,
    UiPopViewRule, WireParametricBand,
};

const MAX_PROTOCOL_LOGS: usize = 200;

struct ActiveLink {
    connection: Arc<dyn BluetoothConnection>,
    driver: Arc<dyn ProtocolDriver>,
    is_virtual: bool,
    notify_tasks: Vec<JoinHandle<()>>,
}

impl ActiveLink {
    fn abort_tasks(&mut self) {
        for handle in self.notify_tasks.drain(..) {
            handle.abort();
        }
    }
}

struct CoreShared {
    registry: ModelRegistry,
    storage: StorageStore,
    simulator: VirtualScanner,
    #[cfg(target_os = "windows")]
    winrt_manager: Option<qcy_transport::WinrtBluetoothManager>,
    snapshot: StdMutex<AppSnapshot>,
    logs: StdMutex<Vec<ProtocolLogEntry>>,
    last_pop_num: StdMutex<HashMap<String, u8>>,
    events_tx: broadcast::Sender<CoreEvent>,
    link: AsyncMutex<Option<ActiveLink>>,
    scan_task: AsyncMutex<Option<JoinHandle<()>>>,
    ota_cancel: Arc<AtomicBool>,
}

/// Central thread-safe QCY Audio Desktop application service.
#[derive(Clone)]
pub struct AppCore {
    shared: Arc<CoreShared>,
}

impl Default for AppCore {
    fn default() -> Self {
        Self::new()
    }
}

impl AppCore {
    /// Initializes `AppCore` backed by the default `%APPDATA%\QcyAudioDesktop\state.json`.
    pub fn new() -> Self {
        Self::with_storage_path(StorageStore::default_app_path())
    }

    /// Initializes `AppCore` with an explicit JSON storage file path (ideal for tests).
    pub fn with_storage_path(storage_path: impl Into<PathBuf>) -> Self {
        let storage = StorageStore::new(storage_path);
        let persisted = storage.load().unwrap_or_default();
        let registry = ModelRegistry::load_embedded();
        let simulator = VirtualScanner::new(VirtualDeviceState::default());

        #[cfg(target_os = "windows")]
        let winrt_manager = Some(qcy_transport::WinrtBluetoothManager::new());

        let adapter = AdapterInfo {
            is_present: false,
            address: None,
            is_classic_supported: false,
            is_low_energy_supported: false,
            is_peripheral_supported: false,
            error: None,
        };

        let sim_enabled = persisted.settings.simulator_enabled;
        let sim_snap = simulator.snapshot();
        let eq_state = EqState {
            custom_presets: persisted.custom_presets,
            ..EqState::default()
        };

        let mut saved_devices = persisted.saved_devices;
        if !sim_enabled {
            saved_devices.retain(|d| !is_simulator_saved_device(d));
        }

        let (
            initial_model,
            initial_device,
            initial_battery,
            initial_firmware,
            initial_color,
            initial_anc,
            initial_multipoint,
            initial_ota,
            initial_sim_state,
        ) = if sim_enabled {
            let model = registry
                .find_by_id(&sim_snap.model_id)
                .cloned()
                .unwrap_or_else(ModelRegistry::generic_fallback);
            let batt = sim_state_to_battery(&sim_snap, &model);
            let dev = DiscoveredDeviceInfo {
                id: format!("virtual:{}", model.model_id),
                address: BluetoothAddress::from_bytes(sim_snap.class_mac).to_mac_string(),
                name: sim_snap.device_name.clone(),
                rssi: Some(-48),
                transport_kind: TransportKind::Dual,
                manufacturer_data: HashMap::from([(
                    COMPANY_ID_QCY,
                    sim_snap.build_advertisement(false).encode(),
                )]),
                service_uuids: vec![qcy_protocol::SVC_QCY_CUSTOM.to_string()],
                is_paired: true,
                is_connected: true,
                last_seen_ms: now_epoch_ms(),
                model_id_hint: Some(model.model_id.clone()),
                numeric_model_id: model.numeric_model_id,
                is_virtual: true,
            };
            if saved_devices.is_empty() {
                saved_devices.push(SavedDeviceRecord {
                    address: dev.address.clone(),
                    name: dev.name.clone(),
                    model_id: model.model_id.clone(),
                    color_index: sim_snap.color_index,
                    last_connected_ms: now_epoch_ms(),
                    last_battery: Some(batt.clone()),
                    is_virtual: true,
                });
            }
            let fw = sim_snap.firmware_version.to_string();
            let ota = OtaState {
                phase: OtaPhaseUi::Idle,
                ota_type: model.ota_type,
                current_firmware: fw.clone(),
                file_path: None,
                progress_pct: 0.0,
                message: None,
            };
            let sim_ctrl = SimulatorControlState {
                enabled: true,
                model_id: model.model_id.clone(),
                lid_open: sim_snap.lid_open,
                latency_ms: sim_snap.latency_ms,
                battery: batt.clone(),
            };
            let mp = MultipointState {
                dual_device: sim_snap.dual_connect,
                multi_device: sim_snap.dual_connect,
                hosts: vec![MultipointHostInfo {
                    mac: "24:4B:FE:10:20:30".to_string(),
                    name: "Windows Desktop".to_string(),
                    is_this_device: true,
                    is_connected: true,
                }],
            };
            (
                model,
                Some(dev),
                batt,
                fw,
                sim_snap.color_index,
                AncState::default(),
                mp,
                ota,
                sim_ctrl,
            )
        } else {
            let model = ModelRegistry::generic_fallback();
            let ota = OtaState {
                phase: OtaPhaseUi::Idle,
                ota_type: model.ota_type,
                current_firmware: String::new(),
                file_path: None,
                progress_pct: 0.0,
                message: None,
            };
            let sim_ctrl = SimulatorControlState {
                enabled: false,
                model_id: sim_snap.model_id.clone(),
                lid_open: false,
                latency_ms: 0,
                battery: BatteryTelemetry::default(),
            };
            (
                model,
                None,
                BatteryTelemetry::default(),
                String::new(),
                0,
                AncState {
                    mode: AncPrimaryMode::Off,
                    ..AncState::default()
                },
                MultipointState::default(),
                ota,
                sim_ctrl,
            )
        };

        let snapshot = AppSnapshot {
            connection_state: if sim_enabled {
                ConnectionState::Connected
            } else {
                ConnectionState::Disconnected
            },
            is_virtual_session: sim_enabled,
            adapter,
            active_device: initial_device,
            firmware_version: initial_firmware,
            device_color_index: initial_color,
            saved_devices,
            battery: initial_battery,
            anc: initial_anc,
            eq: eq_state,
            gestures: GestureState::default(),
            modes: ModesState::default(),
            multipoint: initial_multipoint,
            rgb_lighting: RgbLightingState::default(),
            focus_white_noise: FocusWhiteNoiseState::default(),
            ota: initial_ota,
            last_fit_test: None,
            simulator: initial_sim_state,
            active_model: initial_model,
            settings: persisted.settings,
        };

        let (events_tx, _) = broadcast::channel(256);

        Self {
            shared: Arc::new(CoreShared {
                registry,
                storage,
                simulator,
                #[cfg(target_os = "windows")]
                winrt_manager,
                snapshot: StdMutex::new(snapshot),
                logs: StdMutex::new(Vec::new()),
                last_pop_num: StdMutex::new(HashMap::new()),
                events_tx,
                link: AsyncMutex::new(None),
                scan_task: AsyncMutex::new(None),
                ota_cancel: Arc::new(AtomicBool::new(false)),
            }),
        }
    }

    pub fn registry(&self) -> &ModelRegistry {
        &self.shared.registry
    }

    pub fn settings(&self) -> AppSettings {
        self.shared
            .snapshot
            .lock()
            .expect("snapshot lock")
            .settings
            .clone()
    }

    /// Subscribes to live [`CoreEvent`] notifications (`StateUpdated`, `PopupTriggered`,
    /// `ScanDiscovered`, `ProtocolLog`).
    pub fn subscribe_events(&self) -> broadcast::Receiver<CoreEvent> {
        self.shared.events_tx.subscribe()
    }

    /// Backward-compatible legacy snapshot preserved so `src-tauri` compiles cleanly
    /// while concurrent shell modules are built.
    pub async fn status_snapshot(&self) -> AppStatusSnapshot {
        let snap = self.get_app_snapshot();
        let active_device = if snap.settings.simulator_enabled && snap.is_virtual_session {
            self.shared
                .simulator
                .list_paired_devices()
                .await
                .unwrap_or_default()
                .into_iter()
                .next()
        } else {
            None
        };
        AppStatusSnapshot {
            connection_state: snap.connection_state,
            active_device,
            active_model: snap.active_model,
            anc_mode: snap.anc.to_legacy_anc_mode(),
            gaming_mode: snap.modes.game_mode,
            settings: snap.settings,
        }
    }

    // -----------------------------------------------------------------------
    // 1. Session, Adapter, Scan & Connection Management
    // -----------------------------------------------------------------------

    pub fn get_app_snapshot(&self) -> AppSnapshot {
        self.shared.snapshot.lock().expect("snapshot lock").clone()
    }

    pub fn list_models(&self) -> Vec<DeviceModelProfile> {
        self.shared.registry.models().to_vec()
    }

    pub async fn get_adapter_info(&self) -> AdapterInfo {
        #[cfg(target_os = "windows")]
        {
            if let Some(mgr) = &self.shared.winrt_manager {
                let info = mgr.adapter_info().await;
                let mut snap = self.shared.snapshot.lock().expect("snapshot lock");
                snap.adapter = info.clone();
                return info;
            }
        }
        self.shared
            .snapshot
            .lock()
            .expect("snapshot lock")
            .adapter
            .clone()
    }

    pub fn open_windows_bt_settings() -> CoreResult<()> {
        qcy_transport::open_windows_bluetooth_settings()
            .map_err(|e| CoreError::Transport(e.to_string()))
    }

    pub async fn start_scan(&self) -> CoreResult<()> {
        let is_sim = self.get_app_snapshot().settings.simulator_enabled;
        let mut scan_guard = self.shared.scan_task.lock().await;
        if let Some(old) = scan_guard.take() {
            old.abort();
        }

        if is_sim {
            let mut rx = self.shared.simulator.subscribe_events();
            let shared = Arc::clone(&self.shared);
            let handle = tokio::spawn(async move {
                while let Ok(evt) = rx.recv().await {
                    handle_scanner_event(&shared, evt, true);
                }
            });
            *scan_guard = Some(handle);
            self.shared
                .simulator
                .start_scan()
                .await
                .map_err(|e| CoreError::Transport(e.to_string()))?;
            return Ok(());
        }

        #[cfg(target_os = "windows")]
        if let Some(mgr) = &self.shared.winrt_manager {
            let mut rx = mgr.subscribe_events();
            let shared = Arc::clone(&self.shared);
            let handle = tokio::spawn(async move {
                while let Ok(evt) = rx.recv().await {
                    handle_scanner_event(&shared, evt, false);
                }
            });
            *scan_guard = Some(handle);
            mgr.start_scan()
                .await
                .map_err(|e| CoreError::Transport(e.to_string()))?;
            return Ok(());
        }

        Ok(())
    }

    pub async fn stop_scan(&self) -> CoreResult<()> {
        if let Some(handle) = self.shared.scan_task.lock().await.take() {
            handle.abort();
        }
        let _ = self.shared.simulator.stop_scan().await;
        #[cfg(target_os = "windows")]
        if let Some(mgr) = &self.shared.winrt_manager {
            let _ = mgr.stop_scan().await;
        }
        Ok(())
    }

    pub async fn list_paired_devices(&self) -> CoreResult<Vec<DiscoveredDeviceInfo>> {
        let is_sim = self.get_app_snapshot().settings.simulator_enabled;
        let mut out = Vec::new();

        if is_sim {
            let sim_devs = self
                .shared
                .simulator
                .list_paired_devices()
                .await
                .map_err(|e| CoreError::Transport(e.to_string()))?;
            for d in &sim_devs {
                out.push(enrich_discovered_device(&self.shared.registry, d, true));
            }
            return Ok(out);
        }

        #[cfg(target_os = "windows")]
        if let Some(mgr) = &self.shared.winrt_manager {
            let live_devs = mgr
                .list_paired_devices()
                .await
                .map_err(|e| CoreError::Transport(e.to_string()))?;
            for d in &live_devs {
                out.push(enrich_discovered_device(&self.shared.registry, d, false));
            }
        }

        Ok(out)
    }

    /// Connects either to the raw-byte virtual simulator (`simulator_enabled == true` or
    /// virtual address) OR to a real Windows WinRT BLE/RFCOMM device, discovers GATT
    /// services, instantiates the matching `ProtocolDriver` via `ProtocolRouter`,
    /// subscribes to live notifications, and runs initial wire hydration!
    pub async fn connect_device(
        &self,
        address: &str,
        model_id: Option<&str>,
    ) -> CoreResult<AppSnapshot> {
        // Tear down any previous connection BEFORE opening a new one (prevents virtual shared-state teardown race).
        {
            let mut guard = self.shared.link.lock().await;
            if let Some(mut prev) = guard.take() {
                prev.abort_tasks();
                let _ = prev.connection.disconnect().await;
            }
        }

        let is_sim = self.get_app_snapshot().settings.simulator_enabled;
        if is_sim {
            if let Some(mid) = model_id {
                self.shared
                    .simulator
                    .switch_model(mid)
                    .map_err(CoreError::UnknownModel)?;
            }
            let conn: Arc<dyn BluetoothConnection> =
                Arc::new(self.shared.simulator.open_connection());
            let sim_state = self.shared.simulator.snapshot();
            let profile = self
                .shared
                .registry
                .find_by_id(&sim_state.model_id)
                .cloned()
                .unwrap_or_else(ModelRegistry::generic_fallback);
            return self
                .attach_connection(conn, profile, sim_state.device_name, true)
                .await;
        }

        #[cfg(target_os = "windows")]
        {
            let parsed_addr = BluetoothAddress::from_str(address)
                .map_err(|e| CoreError::InvalidPayload(e.to_string()))?;
            let dual = Arc::new(qcy_transport::WinrtDualConnection::new(parsed_addr));
            dual.connect_ble()
                .await
                .map_err(|e| CoreError::Transport(e.to_string()))?;
            let conn: Arc<dyn BluetoothConnection> = dual;
            let profile = model_id
                .and_then(|id| self.shared.registry.find_by_id(id).cloned())
                .unwrap_or_else(ModelRegistry::generic_fallback);
            return self
                .attach_connection(conn, profile.clone(), profile.name_en.clone(), false)
                .await;
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = (address, model_id);
            Err(CoreError::Transport(
                "live WinRT Bluetooth is only available on Windows".to_string(),
            ))
        }
    }

    pub async fn disconnect_device(&self) -> CoreResult<AppSnapshot> {
        let mut link_guard = self.shared.link.lock().await;
        if let Some(mut old) = link_guard.take() {
            old.abort_tasks();
            let _ = old.connection.disconnect().await;
        }
        drop(link_guard);

        let snap = {
            let mut st = self.shared.snapshot.lock().expect("snapshot lock");
            st.connection_state = ConnectionState::Disconnected;
            st.is_virtual_session = false;
            st.active_device = None;
            st.battery = BatteryTelemetry::default();
            st.firmware_version.clear();
            st.multipoint = MultipointState::default();
            st.last_fit_test = None;
            st.clone()
        };
        self.emit_state(snap.clone());
        Ok(snap)
    }

    pub fn remove_saved_device(&self, address: &str) -> CoreResult<AppSnapshot> {
        let snap = {
            let mut st = self.shared.snapshot.lock().expect("snapshot lock");
            st.saved_devices
                .retain(|d| !d.address.eq_ignore_ascii_case(address));
            st.clone()
        };
        self.persist_current()?;
        self.emit_state(snap.clone());
        Ok(snap)
    }

    // -----------------------------------------------------------------------
    // 2. ANC & AI-ANC Commands
    // -----------------------------------------------------------------------

    pub async fn set_anc_state(&self, payload: SetAncPayload) -> CoreResult<CommandOutcome> {
        let _ = self.ensure_active_virtual_link().await;
        if !payload.force {
            let target_val = i32::from(payload.mode != AncPrimaryMode::Off);
            if let Some(prompt) = self.check_conflicts("anc", target_val) {
                return Ok(CommandOutcome {
                    applied: false,
                    snapshot: self.get_app_snapshot(),
                    conflict_prompt: Some(prompt),
                });
            }
        }

        let (anc_setting, wind_noise_opt) = {
            let mut st = self.shared.snapshot.lock().expect("snapshot lock");
            st.anc.mode = payload.mode;
            if let Some(scene) = payload.scene {
                st.anc.scene = scene;
            }
            if let Some(lvl) = payload.scene_level {
                st.anc.scene_level = lvl.min(2);
            }
            if let Some(custom) = payload.custom_level {
                st.anc.custom_level = custom.min(15);
            }
            if let Some(tr) = payload.transparency_level {
                st.anc.transparency_level = tr.clamp(1, 6);
            }
            if let Some(vb) = payload.vocal_boost {
                st.anc.vocal_boost = vb;
            }
            if let Some(wn) = payload.wind_noise {
                st.anc.wind_noise = wn;
            }
            (st.anc.to_wire_setting(), payload.wind_noise)
        };

        self.send_protocol_command(
            &DeviceCommand::SetAncSetting(anc_setting),
            "set_anc_setting(0x17)",
        )
        .await?;
        if let Some(wn) = wind_noise_opt {
            let _ = self
                .send_protocol_command(
                    &DeviceCommand::SetWindNoise { enabled: wn },
                    "set_wind_noise(0x2A)",
                )
                .await;
        }

        let snap = self.get_app_snapshot();
        self.emit_state(snap.clone());
        Ok(CommandOutcome {
            applied: true,
            snapshot: snap,
            conflict_prompt: None,
        })
    }

    pub async fn set_ai_anc(
        &self,
        enabled: bool,
        matrix: Option<AiAncMatrix>,
    ) -> CoreResult<CommandOutcome> {
        let _ = self.ensure_active_virtual_link().await;
        {
            let mut st = self.shared.snapshot.lock().expect("snapshot lock");
            st.anc.ai_anc_enabled = enabled;
            if let Some(m) = matrix.clone() {
                st.anc.ai_matrix = m;
            }
        }
        self.send_protocol_command(
            &DeviceCommand::SetAiAncEnabled { enabled },
            "set_ai_anc(0x54)",
        )
        .await?;
        if let Some(m) = matrix {
            for entry in state_sync::ai_matrix_to_wire_entries(&m) {
                let _ = self
                    .send_protocol_command(
                        &DeviceCommand::SetAiAncMatrixCell(entry),
                        "set_ai_anc_matrix(0x56)",
                    )
                    .await;
            }
        }

        let snap = self.get_app_snapshot();
        self.emit_state(snap.clone());
        Ok(CommandOutcome {
            applied: true,
            snapshot: snap,
            conflict_prompt: None,
        })
    }

    pub async fn set_anc_button_cycle(&self, modes: Vec<String>) -> CoreResult<CommandOutcome> {
        let _ = self.ensure_active_virtual_link().await;
        if modes.len() >= 2 {
            self.shared
                .snapshot
                .lock()
                .expect("snapshot lock")
                .anc
                .btn_anc_cfg = modes.clone();
            let wire_modes: Vec<qcy_protocol::AncSetting> = modes
                .iter()
                .map(|m| state_sync::anc_cycle_label_to_setting(m))
                .collect();
            self.send_protocol_command(
                &DeviceCommand::SetButtonAncCycle { modes: wire_modes },
                "set_anc_button_cycle(0x65)",
            )
            .await?;
        }
        let snap = self.get_app_snapshot();
        self.emit_state(snap.clone());
        Ok(CommandOutcome {
            applied: true,
            snapshot: snap,
            conflict_prompt: None,
        })
    }

    // -----------------------------------------------------------------------
    // 3. Equalizer, Personalized Hearing EQ & Audio Enhancers
    // -----------------------------------------------------------------------

    pub async fn apply_eq_preset(&self, preset_id: &str) -> CoreResult<CommandOutcome> {
        let _ = self.ensure_active_virtual_link().await;
        let maybe_custom = self
            .shared
            .snapshot
            .lock()
            .expect("snapshot lock")
            .eq
            .custom_presets
            .iter()
            .find(|p| p.id.eq_ignore_ascii_case(preset_id))
            .cloned();

        let (cmd, gains, peq_bands, master_gain) = if let Some(custom) = maybe_custom {
            let peq = state_sync::ui_bands_to_parametric_preset(
                &custom.parametric_bands,
                custom.master_gain_db,
                custom.eq_index,
            );
            let cmd = if !peq.bands.is_empty() {
                DeviceCommand::SetParametricEqV2(peq)
            } else {
                DeviceCommand::SetEqBands {
                    gains_db: custom.gains_10band_db.clone(),
                }
            };
            (
                cmd,
                custom.gains_10band_db,
                custom.parametric_bands,
                custom.master_gain_db,
            )
        } else if let Some(builtin) = self.shared.registry.find_eq_preset(preset_id) {
            let freqs = &self
                .shared
                .registry
                .eq_catalog()
                .frequency_grids
                .bands_10_hz;
            let supports_peq = self
                .get_app_snapshot()
                .active_model
                .capabilities
                .parametric_eq;
            let cmd = if supports_peq {
                DeviceCommand::SetParametricEqV2(builtin.to_parametric_eq(freqs))
            } else {
                DeviceCommand::SetGraphicEq(builtin.to_graphic_eq())
            };
            let ui_bands: Vec<WireParametricBand> = builtin
                .parametric_bands
                .iter()
                .map(|b| WireParametricBand {
                    freq_hz: b.freq_hz,
                    gain_db: b.gain_db,
                    q: b.q,
                    irr_type: b.filter_type,
                    enabled: true,
                })
                .collect();
            (
                cmd,
                builtin.gains_10band_db.clone(),
                ui_bands,
                builtin.master_gain_db,
            )
        } else {
            return Err(CoreError::InvalidPayload(format!(
                "unknown EQ preset id: {preset_id}"
            )));
        };

        {
            let mut st = self.shared.snapshot.lock().expect("snapshot lock");
            st.eq.active_preset_id = preset_id.to_string();
            st.eq.gains_10band_db = gains;
            st.eq.parametric_bands = peq_bands;
            st.eq.master_gain_db = master_gain;
        }
        self.send_protocol_command(&cmd, &format!("apply_eq_preset({preset_id})"))
            .await?;

        let snap = self.get_app_snapshot();
        self.emit_state(snap.clone());
        Ok(CommandOutcome {
            applied: true,
            snapshot: snap,
            conflict_prompt: None,
        })
    }

    pub async fn set_graphic_eq(
        &self,
        gains_db: Vec<f32>,
        master_gain_db: Option<f32>,
    ) -> CoreResult<CommandOutcome> {
        let _ = self.ensure_active_virtual_link().await;
        let clamped: Vec<f32> = gains_db.into_iter().map(|g| g.clamp(-12.0, 12.0)).collect();
        let master = master_gain_db.unwrap_or(0.0);
        {
            let mut st = self.shared.snapshot.lock().expect("snapshot lock");
            st.eq.eq_mode = EqModeUi::Graphic;
            st.eq.active_preset_id = "custom".to_string();
            st.eq.gains_10band_db = clamped.clone();
            if let Some(m) = master_gain_db {
                st.eq.master_gain_db = m;
            }
        }

        let supports_peq = self
            .get_app_snapshot()
            .active_model
            .capabilities
            .parametric_eq;
        let cmd = if supports_peq {
            let bands = clamped
                .iter()
                .zip(DEFAULT_EQ_FREQS_10.iter())
                .map(|(&g, &f)| ParametricEqBand::peak(f, g, 1.41))
                .collect();
            DeviceCommand::SetParametricEqV2(ParametricEqPreset {
                eq_type: 0,
                master_gain_db: master,
                bands,
            })
        } else {
            DeviceCommand::SetGraphicEq(GraphicEqSetting {
                eq_type: 0,
                gains_db: clamped.iter().map(|g| g.round() as i8).collect(),
                game_eq_type: None,
                current_mode: None,
            })
        };
        self.send_protocol_command(&cmd, "set_graphic_eq").await?;

        let snap = self.get_app_snapshot();
        self.emit_state(snap.clone());
        Ok(CommandOutcome {
            applied: true,
            snapshot: snap,
            conflict_prompt: None,
        })
    }

    pub async fn set_parametric_eq(
        &self,
        bands: Vec<WireParametricBand>,
        master_gain_db: Option<f32>,
    ) -> CoreResult<CommandOutcome> {
        let _ = self.ensure_active_virtual_link().await;
        let master = master_gain_db.unwrap_or(0.0);
        {
            let mut st = self.shared.snapshot.lock().expect("snapshot lock");
            st.eq.eq_mode = EqModeUi::Parametric;
            st.eq.active_preset_id = "custom".to_string();
            st.eq.parametric_bands = bands.clone();
            if let Some(m) = master_gain_db {
                st.eq.master_gain_db = m;
            }
        }
        let preset = state_sync::ui_bands_to_parametric_preset(&bands, master, 0);
        self.send_protocol_command(
            &DeviceCommand::SetParametricEqV2(preset),
            "set_parametric_eq(0x22)",
        )
        .await?;

        let snap = self.get_app_snapshot();
        self.emit_state(snap.clone());
        Ok(CommandOutcome {
            applied: true,
            snapshot: snap,
            conflict_prompt: None,
        })
    }

    pub fn save_custom_eq(&self, mut preset: EqPresetItem) -> CoreResult<AppSnapshot> {
        preset.category = "custom".to_string();
        let snap = {
            let mut st = self.shared.snapshot.lock().expect("snapshot lock");
            if let Some(existing) = st.eq.custom_presets.iter_mut().find(|p| p.id == preset.id) {
                *existing = preset.clone();
            } else {
                st.eq.custom_presets.push(preset.clone());
            }
            st.eq.active_preset_id = preset.id;
            st.clone()
        };
        self.persist_current()?;
        self.emit_state(snap.clone());
        Ok(snap)
    }

    pub fn delete_custom_eq(&self, preset_id: &str) -> CoreResult<AppSnapshot> {
        let snap = {
            let mut st = self.shared.snapshot.lock().expect("snapshot lock");
            st.eq.custom_presets.retain(|p| p.id != preset_id);
            if st.eq.active_preset_id == preset_id {
                st.eq.active_preset_id = "default".to_string();
            }
            st.clone()
        };
        self.persist_current()?;
        self.emit_state(snap.clone());
        Ok(snap)
    }

    pub fn import_eq_json(&self, json: &str) -> CoreResult<AppSnapshot> {
        let parsed: serde_json::Value = serde_json::from_str(json)
            .map_err(|e| CoreError::InvalidPayload(format!("invalid EQ JSON: {e}")))?;
        let items: Vec<EqPresetItem> = if parsed.is_array() {
            serde_json::from_value(parsed)
                .map_err(|e| CoreError::InvalidPayload(format!("invalid EQ preset array: {e}")))?
        } else {
            vec![serde_json::from_value(parsed)
                .map_err(|e| CoreError::InvalidPayload(format!("invalid EQ preset object: {e}")))?]
        };
        for item in items {
            self.save_custom_eq(item)?;
        }
        Ok(self.get_app_snapshot())
    }

    pub fn export_eq_json(&self, preset_id: Option<&str>) -> CoreResult<String> {
        let st = self.get_app_snapshot();
        if let Some(pid) = preset_id {
            if let Some(found) = st.eq.custom_presets.iter().find(|p| p.id == pid) {
                return serde_json::to_string_pretty(found)
                    .map_err(|e| CoreError::InvalidPayload(e.to_string()));
            }
        }
        let exported = EqPresetItem {
            id: st.eq.active_preset_id,
            eq_index: 99,
            category: "custom".to_string(),
            name_en: "Exported EQ".to_string(),
            name_zh: "导出音效".to_string(),
            subtitle_en: None,
            subtitle_zh: None,
            master_gain_db: st.eq.master_gain_db,
            gains_10band_db: st.eq.gains_10band_db,
            parametric_bands: st.eq.parametric_bands,
        };
        serde_json::to_string_pretty(&exported)
            .map_err(|e| CoreError::InvalidPayload(e.to_string()))
    }

    pub async fn apply_personalized_eq(
        &self,
        left_gains_db: Vec<f32>,
        right_gains_db: Vec<f32>,
        combined_gains_db: Vec<f32>,
        balance: Option<f32>,
    ) -> CoreResult<CommandOutcome> {
        let _ = self.ensure_active_virtual_link().await;
        let hearing_freqs = &self.shared.registry.hearing_test_config().bands_hz;
        let max_bands = self
            .get_app_snapshot()
            .active_model
            .capabilities
            .max_eq_bands as usize;

        {
            let mut st = self.shared.snapshot.lock().expect("snapshot lock");
            st.eq.personalized_enabled = true;
            st.eq.personalized_left_db = left_gains_db.clone();
            st.eq.personalized_right_db = right_gains_db.clone();
            if combined_gains_db.len() == 10 {
                st.eq.gains_10band_db = combined_gains_db;
            }
            if let Some(b) = balance {
                st.eq.channel_balance = b.clamp(-1.0, 1.0);
            }
            st.eq.active_preset_id = "personalized".to_string();
        }

        let left_preset = ParametricEqPreset {
            eq_type: 11,
            master_gain_db: -3.0,
            bands: left_gains_db
                .iter()
                .zip(hearing_freqs.iter())
                .map(|(&g, &f)| ParametricEqBand::peak(f, g, 0.7))
                .collect(),
        };
        let right_preset = ParametricEqPreset {
            eq_type: 11,
            master_gain_db: -3.0,
            bands: right_gains_db
                .iter()
                .zip(hearing_freqs.iter())
                .map(|(&g, &f)| ParametricEqBand::peak(f, g, 0.7))
                .collect(),
        };
        self.send_protocol_command(
            &DeviceCommand::SetSideParametricEq {
                side: EarSide::Left,
                preset: left_preset,
                max_bands: max_bands.max(8),
            },
            "apply_personalized_eq_left(0x46)",
        )
        .await?;
        self.send_protocol_command(
            &DeviceCommand::SetSideParametricEq {
                side: EarSide::Right,
                preset: right_preset,
                max_bands: max_bands.max(8),
            },
            "apply_personalized_eq_right(0x47)",
        )
        .await?;

        if let Some(b) = balance {
            let wire_bal = ((b.clamp(-1.0, 1.0) + 1.0) * 50.0).round() as u8;
            let _ = self
                .send_protocol_command(
                    &DeviceCommand::SetChannelBalance { balance: wire_bal },
                    "set_channel_balance(0x16)",
                )
                .await;
        }

        let snap = self.get_app_snapshot();
        self.emit_state(snap.clone());
        Ok(CommandOutcome {
            applied: true,
            snapshot: snap,
            conflict_prompt: None,
        })
    }

    pub async fn set_audio_enhancers(
        &self,
        payload: SetAudioEnhancersPayload,
    ) -> CoreResult<CommandOutcome> {
        let _ = self.ensure_active_virtual_link().await;
        if !payload.force {
            for (feature, opt_val) in [
                ("dynamic_eq", payload.dynamic_eq),
                ("thx_eq", payload.thx_eq),
                ("virtual_bass", payload.virtual_bass),
                ("adaptive_volume", payload.adaptive_volume),
            ] {
                if let Some(v) = opt_val {
                    if let Some(prompt) = self.check_conflicts(feature, i32::from(v)) {
                        return Ok(CommandOutcome {
                            applied: false,
                            snapshot: self.get_app_snapshot(),
                            conflict_prompt: Some(prompt),
                        });
                    }
                }
            }
        }
        if let Some(v) = payload.dynamic_eq {
            self.shared.snapshot.lock().expect("lock").eq.dynamic_eq = v;
            self.send_protocol_command(
                &DeviceCommand::SetDynamicEq { enabled: v },
                "dynamic_eq(0x27)",
            )
            .await?;
        }
        if let Some(v) = payload.thx_eq {
            self.shared.snapshot.lock().expect("lock").eq.thx_eq = v;
            self.send_protocol_command(&DeviceCommand::SetThxEq { enabled: v }, "thx_eq(0x63)")
                .await?;
        }
        if let Some(v) = payload.virtual_bass {
            self.shared.snapshot.lock().expect("lock").eq.virtual_bass = v;
            self.send_protocol_command(
                &DeviceCommand::SetBassEnhancement { enabled: v },
                "virtual_bass(0x57)",
            )
            .await?;
        }
        if let Some(v) = payload.adaptive_volume {
            self.shared
                .snapshot
                .lock()
                .expect("lock")
                .eq
                .adaptive_volume = v;
            self.send_protocol_command(
                &DeviceCommand::SetAdaptiveVolume { enabled: v },
                "adaptive_volume(0x40)",
            )
            .await?;
        }
        if let Some(bal) = payload.channel_balance {
            let clamped = bal.clamp(-1.0, 1.0);
            self.shared
                .snapshot
                .lock()
                .expect("lock")
                .eq
                .channel_balance = clamped;
            let wire_bal = ((clamped + 1.0) * 50.0).round() as u8;
            self.send_protocol_command(
                &DeviceCommand::SetChannelBalance { balance: wire_bal },
                "channel_balance(0x16)",
            )
            .await?;
        }

        let snap = self.get_app_snapshot();
        self.emit_state(snap.clone());
        Ok(CommandOutcome {
            applied: true,
            snapshot: snap,
            conflict_prompt: None,
        })
    }

    // -----------------------------------------------------------------------
    // 4. Gestures, Touch & IMU Head Controls
    // -----------------------------------------------------------------------

    pub async fn set_button_action(
        &self,
        key_id: u16,
        event_id: u16,
        cmd: u8,
    ) -> CoreResult<CommandOutcome> {
        let _ = self.ensure_active_virtual_link().await;
        {
            let mut st = self.shared.snapshot.lock().expect("lock");
            if let Some(existing) = st
                .gestures
                .mappings
                .iter_mut()
                .find(|m| m.key_id == key_id && m.event_id == event_id)
            {
                existing.cmd = cmd;
            } else {
                st.gestures.mappings.push(ButtonMappingEntry {
                    key_id,
                    event_id,
                    cmd,
                });
            }
        }
        self.send_protocol_command(
            &DeviceCommand::SetButtonBinding(ButtonBinding {
                event_id,
                action: ButtonAction::from_u8(cmd),
            }),
            "set_button_action(0x2B)",
        )
        .await?;

        let snap = self.get_app_snapshot();
        self.emit_state(snap.clone());
        Ok(CommandOutcome {
            applied: true,
            snapshot: snap,
            conflict_prompt: None,
        })
    }

    pub async fn reset_buttons(&self) -> CoreResult<CommandOutcome> {
        let _ = self.ensure_active_virtual_link().await;
        self.shared.snapshot.lock().expect("lock").gestures.mappings =
            GestureState::default().mappings;
        self.send_protocol_command(&DeviceCommand::ResetButtonBindings, "reset_buttons(0x2B)")
            .await?;
        let snap = self.get_app_snapshot();
        self.emit_state(snap.clone());
        Ok(CommandOutcome {
            applied: true,
            snapshot: snap,
            conflict_prompt: None,
        })
    }

    pub async fn set_touch_config(
        &self,
        touch_enable: Option<bool>,
        touch_sensitivity: Option<u8>,
    ) -> CoreResult<CommandOutcome> {
        let _ = self.ensure_active_virtual_link().await;
        if let Some(en) = touch_enable {
            self.shared
                .snapshot
                .lock()
                .expect("lock")
                .gestures
                .touch_enable = en;
            self.send_protocol_command(
                &DeviceCommand::SetSleepMode { enabled: !en },
                "set_touch_enable(0x10)",
            )
            .await?;
        }
        if let Some(sens) = touch_sensitivity {
            self.shared
                .snapshot
                .lock()
                .expect("lock")
                .gestures
                .touch_sensitivity = sens;
            self.send_protocol_command(
                &DeviceCommand::SetTouchSensitivity {
                    sensitivity: TouchSensitivity::from_u8(sens),
                },
                "set_touch_sensitivity(0x33)",
            )
            .await?;
        }
        let snap = self.get_app_snapshot();
        self.emit_state(snap.clone());
        Ok(CommandOutcome {
            applied: true,
            snapshot: snap,
            conflict_prompt: None,
        })
    }

    pub async fn set_head_controls(
        &self,
        patch: SetHeadControlsPatch,
    ) -> CoreResult<CommandOutcome> {
        let _ = self.ensure_active_virtual_link().await;
        if let Some(en) = patch.enabled {
            self.shared
                .snapshot
                .lock()
                .expect("lock")
                .gestures
                .head_controls
                .enabled = en;
            self.send_protocol_command(
                &DeviceCommand::SetHeadGestureEnabled { enabled: en },
                "set_head_controls(0x50)",
            )
            .await?;
        }
        for (ev, opt_cmd) in [
            (HeadGestureEvent::Nod, patch.nod_cmd),
            (HeadGestureEvent::Shake, patch.shake_cmd),
            (HeadGestureEvent::TiltLeft, patch.tilt_left_cmd),
            (HeadGestureEvent::TiltRight, patch.tilt_right_cmd),
        ] {
            if let Some(cmd) = opt_cmd {
                {
                    let mut st = self.shared.snapshot.lock().expect("lock");
                    match ev {
                        HeadGestureEvent::Nod => st.gestures.head_controls.nod_cmd = cmd,
                        HeadGestureEvent::Shake => st.gestures.head_controls.shake_cmd = cmd,
                        HeadGestureEvent::TiltLeft => st.gestures.head_controls.tilt_left_cmd = cmd,
                        HeadGestureEvent::TiltRight => {
                            st.gestures.head_controls.tilt_right_cmd = cmd
                        }
                        HeadGestureEvent::Raw(_) => {}
                    }
                }
                self.send_protocol_command(
                    &DeviceCommand::SetHeadGestureBinding(HeadGestureBinding {
                        event: ev,
                        action: HeadGestureAction::from_u8(cmd),
                    }),
                    "set_head_gesture_binding(0x51)",
                )
                .await?;
            }
        }
        let snap = self.get_app_snapshot();
        self.emit_state(snap.clone());
        Ok(CommandOutcome {
            applied: true,
            snapshot: snap,
            conflict_prompt: None,
        })
    }

    // -----------------------------------------------------------------------
    // 5. Modes, Multipoint, RGB Lighting, Focus Mode, Fit Test & Maintenance
    // -----------------------------------------------------------------------

    pub async fn set_modes(&self, payload: SetModesPayload) -> CoreResult<CommandOutcome> {
        let _ = self.ensure_active_virtual_link().await;

        // Evaluate popViewRules conflict guards before applying mutating toggles
        if !payload.force {
            if let Some(ldac) = payload.ldac {
                if let Some(prompt) = self.check_conflicts("ldac", i32::from(ldac)) {
                    return Ok(CommandOutcome {
                        applied: false,
                        snapshot: self.get_app_snapshot(),
                        conflict_prompt: Some(prompt),
                    });
                }
            }
            if let Some(sp) = payload.spatial_audio {
                let candidate_vals: &[i32] = match sp {
                    SpatialAudioModeUi::Off => &[0, 1],
                    SpatialAudioModeUi::Fixed => &[2, 10],
                    SpatialAudioModeUi::HeadTracking => &[3, 10],
                };
                for &val in candidate_vals {
                    if let Some(prompt) = self.check_conflicts("spatial_audio", val) {
                        return Ok(CommandOutcome {
                            applied: false,
                            snapshot: self.get_app_snapshot(),
                            conflict_prompt: Some(prompt),
                        });
                    }
                }
            }
            if let Some(lea) = payload.le_audio {
                if let Some(prompt) = self.check_conflicts("le_audio", i32::from(lea)) {
                    return Ok(CommandOutcome {
                        applied: false,
                        snapshot: self.get_app_snapshot(),
                        conflict_prompt: Some(prompt),
                    });
                }
            }
        } else {
            // Forced apply: clear mutually exclusive features when enabling LDAC or Spatial Audio
            if payload.ldac == Some(true) {
                {
                    let mut st = self.shared.snapshot.lock().expect("lock");
                    st.multipoint.dual_device = false;
                    st.multipoint.multi_device = false;
                    st.modes.spatial_audio = SpatialAudioModeUi::Off;
                }
                let _ = self
                    .send_protocol_command(
                        &DeviceCommand::SetDualConnect { enabled: false },
                        "clear_dual_connect_for_ldac",
                    )
                    .await;
                let _ = self
                    .send_protocol_command(
                        &DeviceCommand::SetSpatialAudioMode {
                            mode: SpatialAudioMode::Off,
                        },
                        "clear_spatial_for_ldac",
                    )
                    .await;
            }
            if matches!(
                payload.spatial_audio,
                Some(SpatialAudioModeUi::Fixed | SpatialAudioModeUi::HeadTracking)
            ) {
                self.shared.snapshot.lock().expect("lock").modes.ldac = false;
                let _ = self
                    .send_protocol_command(
                        &DeviceCommand::SetLdac { enabled: false },
                        "clear_ldac_for_spatial",
                    )
                    .await;
            }
        }

        if let Some(lm) = payload.listening_mode {
            self.shared
                .snapshot
                .lock()
                .expect("lock")
                .modes
                .listening_mode = lm;
            let wire = match lm {
                ListeningModeUi::Music => ListeningMode::Music,
                ListeningModeUi::Game => ListeningMode::Game,
                ListeningModeUi::Movie => ListeningMode::Movie,
            };
            self.send_protocol_command(
                &DeviceCommand::SetListeningMode { mode: wire },
                "listening_mode(0x2E)",
            )
            .await?;
        }
        if let Some(gm) = payload.game_mode {
            self.shared.snapshot.lock().expect("lock").modes.game_mode = gm;
            self.send_protocol_command(
                &DeviceCommand::SetGamingMode { enabled: gm },
                "game_mode(0x09)",
            )
            .await?;
        }
        if payload.game_mode_pro.is_some() || payload.game_sync_eq.is_some() {
            let cfg = {
                let mut st = self.shared.snapshot.lock().expect("lock");
                if let Some(p) = payload.game_mode_pro {
                    st.modes.game_mode_pro = p;
                    st.modes.game_mode = p;
                }
                if let Some(s) = payload.game_sync_eq {
                    st.modes.game_sync_eq = s;
                }
                GameModeProConfig {
                    enabled: st.modes.game_mode_pro,
                    follow_game_sound: st.modes.game_sync_eq,
                }
            };
            self.send_protocol_command(&DeviceCommand::SetGameModePro(cfg), "game_mode_pro(0x4A)")
                .await?;
        }
        if let Some(sp) = payload.spatial_audio {
            self.shared
                .snapshot
                .lock()
                .expect("lock")
                .modes
                .spatial_audio = sp;
            let wire = match sp {
                SpatialAudioModeUi::Off => SpatialAudioMode::Off,
                SpatialAudioModeUi::Fixed => SpatialAudioMode::Fixed,
                SpatialAudioModeUi::HeadTracking => SpatialAudioMode::HeadTracking,
            };
            self.send_protocol_command(
                &DeviceCommand::SetSpatialAudioMode { mode: wire },
                "spatial_audio(0x5B)",
            )
            .await?;
        }
        if let Some(ldac) = payload.ldac {
            self.shared.snapshot.lock().expect("lock").modes.ldac = ldac;
            self.send_protocol_command(&DeviceCommand::SetLdac { enabled: ldac }, "ldac(0x23)")
                .await?;
        }
        if let Some(lea) = payload.le_audio {
            self.shared.snapshot.lock().expect("lock").modes.le_audio = lea;
            self.send_protocol_command(
                &DeviceCommand::SetLeAudio { enabled: lea },
                "le_audio(0x60)",
            )
            .await?;
        }
        if payload.in_ear_mode.is_some() || payload.in_ear_auto_anc.is_some() {
            let cfg = {
                let mut st = self.shared.snapshot.lock().expect("lock");
                if let Some(m) = payload.in_ear_mode {
                    st.modes.in_ear_mode = m;
                }
                if let Some(a) = payload.in_ear_auto_anc {
                    st.modes.in_ear_auto_anc = a;
                }
                WearConfig {
                    enabled: st.modes.in_ear_mode > 0,
                    music_behavior: InEarMusicBehavior::from_u8(st.modes.in_ear_mode),
                    control_anc: st.modes.in_ear_auto_anc,
                }
            };
            self.send_protocol_command(&DeviceCommand::SetWearConfig(cfg), "wear_config(0x2C)")
                .await?;
        }
        if let Some(sens) = payload.in_ear_sensitivity {
            self.shared
                .snapshot
                .lock()
                .expect("lock")
                .modes
                .in_ear_sensitivity = sens;
            self.send_protocol_command(
                &DeviceCommand::SetWearSensitivity {
                    sensitivity: WearSensitivity::from_u8(sens),
                },
                "wear_sensitivity(0x48)",
            )
            .await?;
        }
        if payload.hearing_low_vol_remind.is_some()
            || payload.hearing_duration_remind.is_some()
            || payload.hearing_duration_min.is_some()
        {
            let cfg = {
                let mut st = self.shared.snapshot.lock().expect("lock");
                if let Some(v) = payload.hearing_low_vol_remind {
                    st.modes.hearing_low_vol_remind = v;
                }
                if let Some(v) = payload.hearing_duration_remind {
                    st.modes.hearing_duration_remind = v;
                }
                if let Some(m) = payload.hearing_duration_min {
                    st.modes.hearing_duration_min = m;
                }
                HearingHealthConfig {
                    alarm_volume: st.modes.hearing_low_vol_remind,
                    alarm_time: st.modes.hearing_duration_remind,
                    limit_minutes: st.modes.hearing_duration_min,
                }
            };
            self.send_protocol_command(
                &DeviceCommand::SetHearingHealth(cfg),
                "hearing_health(0x58)",
            )
            .await?;
        }
        if payload.privacy_mode.is_some() || payload.privacy_scope.is_some() {
            let cfg = {
                let mut st = self.shared.snapshot.lock().expect("lock");
                if let Some(v) = payload.privacy_mode {
                    st.modes.privacy_mode = v;
                }
                if let Some(s) = payload.privacy_scope {
                    st.modes.privacy_scope = s;
                }
                PrivacyModeProConfig {
                    enabled: st.modes.privacy_mode,
                    call_only: st.modes.privacy_scope == 1,
                }
            };
            self.send_protocol_command(
                &DeviceCommand::SetPrivacyModePro(cfg),
                "privacy_mode(0x5F)",
            )
            .await?;
        }
        if let Some(v) = payload.dialogue_perception {
            self.shared
                .snapshot
                .lock()
                .expect("lock")
                .modes
                .dialogue_perception = v;
            self.send_protocol_command(
                &DeviceCommand::SetDialoguePerception { enabled: v },
                "dialogue_perception(0x64)",
            )
            .await?;
        }
        if let Some(sec) = payload.speak_to_chat_sec {
            self.shared
                .snapshot
                .lock()
                .expect("lock")
                .modes
                .speak_to_chat_sec = sec;
            self.send_protocol_command(
                &DeviceCommand::SetVoiceDetectionDelay { seconds: sec },
                "voice_detection(0x5C)",
            )
            .await?;
        }
        if let Some(min) = payload.auto_power_off_min {
            self.shared
                .snapshot
                .lock()
                .expect("lock")
                .modes
                .auto_power_off_min = min;
            let wire_min = if min == 0 { 0xFFFF } else { min };
            self.send_protocol_command(
                &DeviceCommand::SetAutoPowerOffMinutes { minutes: wire_min },
                "auto_power_off(0x14)",
            )
            .await?;
        }
        if let Some(min) = payload.disconnect_power_off_min {
            self.shared
                .snapshot
                .lock()
                .expect("lock")
                .modes
                .disconnect_power_off_min = min;
            let wire_min = if min == 0 { 0xFFFF } else { min };
            self.send_protocol_command(
                &DeviceCommand::SetStandbyMinutes { minutes: wire_min },
                "standby(0x1F)",
            )
            .await?;
        }
        if let Some(v) = payload.power_saving {
            self.shared
                .snapshot
                .lock()
                .expect("lock")
                .modes
                .power_saving = v;
            self.send_protocol_command(
                &DeviceCommand::SetPowerSavingMode { enabled: v },
                "power_saving(0x53)",
            )
            .await?;
        }
        if let Some(v) = payload.disconnect_warning {
            self.shared
                .snapshot
                .lock()
                .expect("lock")
                .modes
                .disconnect_warning = v;
            self.send_protocol_command(
                &DeviceCommand::SetDisconnectWarning { enabled: v },
                "disconnect_warning(0x5D)",
            )
            .await?;
        }
        if let Some(lang) = payload.prompt_language {
            self.shared
                .snapshot
                .lock()
                .expect("lock")
                .modes
                .prompt_language = lang;
            let prefix = match self.get_app_snapshot().active_model.chipset_family.as_str() {
                "jieli" => "JL",
                "bluetrum" => "LX",
                _ => "WQ",
            };
            let suffix = match lang {
                PromptLanguageUi::En => "00",
                PromptLanguageUi::Zh => "01",
            };
            self.send_protocol_command(
                &DeviceCommand::SetPromptVoiceCode {
                    code: format!("{prefix}{suffix}"),
                },
                "prompt_language(0x19)",
            )
            .await?;
        }
        if let Some(vol) = payload.prompt_volume {
            self.shared
                .snapshot
                .lock()
                .expect("lock")
                .modes
                .prompt_volume = vol;
            self.send_protocol_command(
                &DeviceCommand::SetPromptToneVolume { level: vol },
                "prompt_volume(0x1D)",
            )
            .await?;
        }

        let snap = self.get_app_snapshot();
        self.emit_state(snap.clone());
        Ok(CommandOutcome {
            applied: true,
            snapshot: snap,
            conflict_prompt: None,
        })
    }

    pub async fn set_multipoint(
        &self,
        payload: SetMultipointPayload,
    ) -> CoreResult<CommandOutcome> {
        let _ = self.ensure_active_virtual_link().await;
        if !payload.force {
            if let Some(dd) = payload.dual_device {
                if let Some(prompt) = self.check_conflicts("dual_device", i32::from(dd)) {
                    return Ok(CommandOutcome {
                        applied: false,
                        snapshot: self.get_app_snapshot(),
                        conflict_prompt: Some(prompt),
                    });
                }
            }
            if let Some(md) = payload.multi_device {
                if let Some(prompt) = self.check_conflicts("multi_device", i32::from(md)) {
                    return Ok(CommandOutcome {
                        applied: false,
                        snapshot: self.get_app_snapshot(),
                        conflict_prompt: Some(prompt),
                    });
                }
            }
        } else if payload.dual_device == Some(true) || payload.multi_device == Some(true) {
            self.shared.snapshot.lock().expect("lock").modes.ldac = false;
            let _ = self
                .send_protocol_command(
                    &DeviceCommand::SetLdac { enabled: false },
                    "clear_ldac_for_multipoint",
                )
                .await;
        }

        if let Some(enabled) = payload.dual_device.or(payload.multi_device) {
            {
                let mut st = self.shared.snapshot.lock().expect("lock");
                st.multipoint.dual_device = enabled;
                st.multipoint.multi_device = enabled;
            }
            self.send_protocol_command(
                &DeviceCommand::SetDualConnect { enabled },
                "dual_connect(0x24)",
            )
            .await?;
        }
        if let Some(enter) = payload.enter_pairing {
            self.send_protocol_command(
                &DeviceCommand::SetMultipointPairing { enabled: enter },
                "multi_pair(0x66)",
            )
            .await?;
        }
        for (action, opt_mac) in [
            (MultipointHostAction::Connect, payload.connect_mac),
            (MultipointHostAction::Disconnect, payload.disconnect_mac),
            (MultipointHostAction::Delete, payload.delete_mac),
        ] {
            if let Some(mac_str) = opt_mac {
                let parsed = BluetoothAddress::from_str(&mac_str)
                    .map_err(|e| CoreError::InvalidPayload(e.to_string()))?;
                self.send_protocol_command(
                    &DeviceCommand::ControlMultipointHost {
                        action,
                        mac: parsed.to_bytes(),
                    },
                    "multipoint_host_action(0x4F)",
                )
                .await?;
            }
        }

        let snap = self.get_app_snapshot();
        self.emit_state(snap.clone());
        Ok(CommandOutcome {
            applied: true,
            snapshot: snap,
            conflict_prompt: None,
        })
    }

    pub async fn set_rgb_lighting(
        &self,
        payload: SetRgbLightingPayload,
    ) -> CoreResult<CommandOutcome> {
        let _ = self.ensure_active_virtual_link().await;
        let (enabled_opt, tws_opt, effect_cfg) = {
            let mut st = self.shared.snapshot.lock().expect("lock");
            if let Some(en) = payload.enabled {
                st.rgb_lighting.enabled = en;
            }
            if let Some(m) = payload.mode {
                st.rgb_lighting.mode = m;
            }
            if let Some(hex) = payload.color_hex {
                st.rgb_lighting.color_hex = hex;
            }
            if let Some(b) = payload.brightness {
                st.rgb_lighting.brightness = b.min(100);
            }
            if let Some(s) = payload.speed {
                st.rgb_lighting.speed = s.min(100);
            }
            if let Some(tws) = payload.tws_pair {
                st.rgb_lighting.tws_pair_enabled = tws;
            }
            let color = state_sync::parse_hex_rgb(&st.rgb_lighting.color_hex);
            (
                payload.enabled,
                payload.tws_pair,
                LedEffectConfig {
                    speed: st.rgb_lighting.speed,
                    brightness: st.rgb_lighting.brightness,
                    effect: st.rgb_lighting.mode.to_wire(),
                    colors: vec![color],
                },
            )
        };

        if let Some(en) = enabled_opt {
            self.send_protocol_command(
                &DeviceCommand::SetLedSwitch { enabled: en },
                "led_switch(0x35)",
            )
            .await?;
        }
        self.send_protocol_command(&DeviceCommand::SetLedEffect(effect_cfg), "led_effect(0x36)")
            .await?;
        if let Some(tws) = tws_opt {
            self.send_protocol_command(
                &DeviceCommand::SetTwsEnable { enabled: tws },
                "tws_enable(0x34)",
            )
            .await?;
        }

        let snap = self.get_app_snapshot();
        self.emit_state(snap.clone());
        Ok(CommandOutcome {
            applied: true,
            snapshot: snap,
            conflict_prompt: None,
        })
    }

    pub async fn set_focus_white_noise(
        &self,
        playing: Option<bool>,
        track_index: Option<u32>,
        volume: Option<u8>,
    ) -> CoreResult<CommandOutcome> {
        let _ = self.ensure_active_virtual_link().await;
        if let Some(pl) = playing {
            self.shared
                .snapshot
                .lock()
                .expect("lock")
                .focus_white_noise
                .hardware_playing = pl;
            self.send_protocol_command(
                &DeviceCommand::SetFocusMode { enabled: pl },
                "focus_mode(0x39)",
            )
            .await?;
            let _ = self
                .send_protocol_command(
                    &DeviceCommand::ToggleWhiteNoisePlayback,
                    "music_action(0x04)",
                )
                .await;
        }
        if let Some(idx) = track_index {
            self.shared
                .snapshot
                .lock()
                .expect("lock")
                .focus_white_noise
                .hardware_track_index = idx;
        }
        if let Some(vol) = volume {
            let clamped = vol.min(100);
            self.shared
                .snapshot
                .lock()
                .expect("lock")
                .focus_white_noise
                .hardware_volume = clamped;
            self.send_protocol_command(
                &DeviceCommand::SetWhiteNoiseVolume { volume: clamped },
                "white_noise_volume(0x08)",
            )
            .await?;
        }

        let snap = self.get_app_snapshot();
        self.emit_state(snap.clone());
        Ok(CommandOutcome {
            applied: true,
            snapshot: snap,
            conflict_prompt: None,
        })
    }

    pub async fn run_fit_test(&self) -> CoreResult<FitTestResult> {
        let _ = self.ensure_active_virtual_link().await;
        self.send_protocol_command(&DeviceCommand::StartFitTest, "start_fit_test(0x11)")
            .await?;
        // Allow async virtual/BLE notification handler to process incoming 0x11 result
        tokio::time::sleep(Duration::from_millis(25)).await;
        let existing = self.shared.snapshot.lock().expect("lock").last_fit_test;
        let result = existing.unwrap_or(FitTestResult {
            left: FitTestEarStatus::Good,
            right: FitTestEarStatus::Good,
            tested_at_ms: now_epoch_ms(),
        });
        {
            self.shared.snapshot.lock().expect("lock").last_fit_test = Some(result);
        }
        self.emit_state(self.get_app_snapshot());
        Ok(result)
    }

    pub async fn set_device_color(&self, color_index: u8) -> CoreResult<AppSnapshot> {
        let _ = self.ensure_active_virtual_link().await;
        self.shared
            .snapshot
            .lock()
            .expect("lock")
            .device_color_index = color_index;
        let _ = self
            .send_protocol_command(
                &DeviceCommand::SetDeviceColor { color_index },
                "set_device_color(0x42)",
            )
            .await;
        let snap = self.get_app_snapshot();
        self.emit_state(snap.clone());
        Ok(snap)
    }

    pub async fn rename_device(&self, name: &str) -> CoreResult<CommandOutcome> {
        let _ = self.ensure_active_virtual_link().await;
        let clean = name.trim();
        if clean.is_empty() {
            return Err(CoreError::InvalidPayload(
                "device name cannot be empty".to_string(),
            ));
        }
        {
            let mut st = self.shared.snapshot.lock().expect("lock");
            if let Some(dev) = st.active_device.as_mut() {
                dev.name = clean.to_string();
            }
        }
        self.send_protocol_command(
            &DeviceCommand::SetDeviceName {
                name: clean.to_string(),
            },
            "rename_device(0x18)",
        )
        .await?;
        let snap = self.get_app_snapshot();
        self.emit_state(snap.clone());
        Ok(CommandOutcome {
            applied: true,
            snapshot: snap,
            conflict_prompt: None,
        })
    }

    pub async fn reset_device(&self, kind: ResetDeviceKind) -> CoreResult<CommandOutcome> {
        let _ = self.ensure_active_virtual_link().await;
        let cmd = match kind {
            ResetDeviceKind::SoftReset => DeviceCommand::ResetDefault,
            ResetDeviceKind::ClearPairing => DeviceCommand::ClearPairing,
            ResetDeviceKind::FactoryReset => DeviceCommand::FactoryReset,
        };
        self.send_protocol_command(&cmd, "reset_device").await?;
        {
            let mut st = self.shared.snapshot.lock().expect("lock");
            st.anc.mode = AncPrimaryMode::Off;
            st.eq.active_preset_id = "default".to_string();
            st.eq.gains_10band_db = vec![0.0; 10];
        }
        let snap = self.get_app_snapshot();
        self.emit_state(snap.clone());
        Ok(CommandOutcome {
            applied: true,
            snapshot: snap,
            conflict_prompt: None,
        })
    }

    // -----------------------------------------------------------------------
    // 6. Local OTA Firmware Update Engine
    // -----------------------------------------------------------------------

    /// Starts a local firmware update from a file on disk (`file_path`).
    pub async fn start_local_ota(&self, file_path: &str) -> CoreResult<OtaState> {
        let ota_type_raw = self.get_app_snapshot().active_model.ota_type;
        if ota_type_raw == OtaType::JieLiBleAuth.as_u8() {
            return Ok(self.set_ota_unsupported_auth(Some(file_path.to_string())));
        }
        let bytes = std::fs::read(file_path).map_err(|e| {
            CoreError::Ota(format!("failed to read firmware file '{file_path}': {e}"))
        })?;
        self.start_local_ota_bytes(file_path, bytes).await
    }

    /// Starts a local firmware update from in-memory firmware bytes (also used by tests).
    pub async fn start_local_ota_bytes(
        &self,
        file_label: &str,
        bytes: Vec<u8>,
    ) -> CoreResult<OtaState> {
        let _ = self.ensure_active_virtual_link().await;
        self.shared.ota_cancel.store(false, Ordering::SeqCst);

        let snap_now = self.get_app_snapshot();
        let ota_type_raw = snap_now.active_model.ota_type;
        if ota_type_raw == OtaType::JieLiBleAuth.as_u8() {
            return Ok(self.set_ota_unsupported_auth(Some(file_label.to_string())));
        }
        let Some(ota_type) = OtaType::from_u8(ota_type_raw) else {
            let state = OtaState {
                phase: OtaPhaseUi::Error,
                ota_type: ota_type_raw,
                current_firmware: snap_now.firmware_version,
                file_path: Some(file_label.to_string()),
                progress_pct: 0.0,
                message: Some(format!("unsupported ota_type: {ota_type_raw}")),
            };
            self.update_ota_state(state.clone());
            return Ok(state);
        };

        // Pre-flight safety check (>=30% battery & both TWS buds connected)
        let preflight = OtaPreFlight {
            left_battery: snap_now.battery.left,
            right_battery: snap_now.battery.right,
            is_tws: snap_now.active_model.capabilities.has_case_battery,
            both_buds_connected: !snap_now.active_model.capabilities.has_case_battery
                || (snap_now.battery.left.is_some() && snap_now.battery.right.is_some()),
            min_battery_percent: 30,
        };
        if let Err(err) = preflight.validate() {
            let state = OtaState {
                phase: OtaPhaseUi::Error,
                ota_type: ota_type_raw,
                current_firmware: snap_now.firmware_version,
                file_path: Some(file_label.to_string()),
                progress_pct: 0.0,
                message: Some(err.to_string()),
            };
            self.update_ota_state(state.clone());
            return Ok(state);
        }

        let mtu = {
            let guard = self.shared.link.lock().await;
            guard.as_ref().map(|l| l.connection.mtu()).unwrap_or(256)
        };

        let mut session = match OtaSession::new(ota_type, bytes, mtu) {
            Ok(s) => s,
            Err(OtaError::UnsupportedAuthRequired { .. }) => {
                return Ok(self.set_ota_unsupported_auth(Some(file_label.to_string())));
            }
            Err(e) => {
                let state = OtaState {
                    phase: OtaPhaseUi::Error,
                    ota_type: ota_type_raw,
                    current_firmware: snap_now.firmware_version,
                    file_path: Some(file_label.to_string()),
                    progress_pct: 0.0,
                    message: Some(e.to_string()),
                };
                self.update_ota_state(state.clone());
                return Ok(state);
            }
        };

        let route = session.route_info();
        self.update_ota_state(OtaState {
            phase: OtaPhaseUi::Validating,
            ota_type: ota_type_raw,
            current_firmware: snap_now.firmware_version.clone(),
            file_path: Some(file_label.to_string()),
            progress_pct: 0.0,
            message: None,
        });

        let start_pkts = session
            .build_start_packets()
            .map_err(|e| CoreError::Ota(e.to_string()))?;
        for pkt in start_pkts {
            self.write_ota_packet(route, &pkt, "ota_start").await?;
        }

        while let Some(chunk) = session
            .next_data_packet()
            .map_err(|e| CoreError::Ota(e.to_string()))?
        {
            if self.shared.ota_cancel.load(Ordering::SeqCst) {
                let cancelled = OtaState {
                    phase: OtaPhaseUi::Idle,
                    ota_type: ota_type_raw,
                    current_firmware: snap_now.firmware_version,
                    file_path: None,
                    progress_pct: 0.0,
                    message: Some("cancelled".to_string()),
                };
                self.update_ota_state(cancelled.clone());
                return Ok(cancelled);
            }
            self.write_ota_packet(route, &chunk, "ota_chunk").await?;
            self.update_ota_state(OtaState {
                phase: OtaPhaseUi::Transferring,
                ota_type: ota_type_raw,
                current_firmware: snap_now.firmware_version.clone(),
                file_path: Some(file_label.to_string()),
                progress_pct: session.progress_percent(),
                message: None,
            });
        }

        let finish_pkts = session
            .build_finish_packets()
            .map_err(|e| CoreError::Ota(e.to_string()))?;
        self.update_ota_state(OtaState {
            phase: OtaPhaseUi::Verifying,
            ota_type: ota_type_raw,
            current_firmware: snap_now.firmware_version.clone(),
            file_path: Some(file_label.to_string()),
            progress_pct: 99.0,
            message: None,
        });
        for pkt in finish_pkts {
            self.write_ota_packet(route, &pkt, "ota_finish").await?;
        }

        session.mark_completed();
        let completed = OtaState {
            phase: OtaPhaseUi::Completed,
            ota_type: ota_type_raw,
            current_firmware: snap_now.firmware_version,
            file_path: Some(file_label.to_string()),
            progress_pct: 100.0,
            message: None,
        };
        self.update_ota_state(completed.clone());
        Ok(completed)
    }

    pub fn cancel_ota(&self) -> OtaState {
        self.shared.ota_cancel.store(true, Ordering::SeqCst);
        let state = {
            let mut st = self.shared.snapshot.lock().expect("lock");
            st.ota.phase = OtaPhaseUi::Idle;
            st.ota.progress_pct = 0.0;
            st.ota.clone()
        };
        self.emit_state(self.get_app_snapshot());
        state
    }

    // -----------------------------------------------------------------------
    // 7. Simulator Controls, Settings, Popups & Protocol Logs
    // -----------------------------------------------------------------------

    pub async fn set_simulator_enabled(&self, enabled: bool) -> CoreResult<AppSnapshot> {
        if !enabled {
            let _ = self.stop_scan().await;
            {
                let mut link_guard = self.shared.link.lock().await;
                if let Some(mut old) = link_guard.take() {
                    old.abort_tasks();
                    let _ = old.connection.disconnect().await;
                }
            }
            let snap = {
                let mut st = self.shared.snapshot.lock().expect("lock");
                st.settings.simulator_enabled = false;
                st.simulator.enabled = false;
                st.simulator.lid_open = false;
                st.simulator.battery = BatteryTelemetry::default();
                st.is_virtual_session = false;
                st.connection_state = ConnectionState::Disconnected;
                st.active_device = None;
                st.active_model = ModelRegistry::generic_fallback();
                st.battery = BatteryTelemetry::default();
                st.firmware_version.clear();
                st.device_color_index = 0;
                st.anc = AncState {
                    mode: AncPrimaryMode::Off,
                    ..AncState::default()
                };
                st.multipoint = MultipointState::default();
                st.ota = OtaState {
                    phase: OtaPhaseUi::Idle,
                    ota_type: st.active_model.ota_type,
                    current_firmware: String::new(),
                    file_path: None,
                    progress_pct: 0.0,
                    message: None,
                };
                st.last_fit_test = None;
                st.saved_devices.retain(|d| !is_simulator_saved_device(d));
                st.clone()
            };
            self.persist_current()?;
            self.emit_state(snap.clone());
            return Ok(snap);
        }

        let (model_id, addr) = {
            let sim_snap = self.shared.simulator.snapshot();
            let model = self
                .shared
                .registry
                .find_by_id(&sim_snap.model_id)
                .cloned()
                .unwrap_or_else(ModelRegistry::generic_fallback);
            let batt = sim_state_to_battery(&sim_snap, &model);
            let mut st = self.shared.snapshot.lock().expect("lock");
            st.settings.simulator_enabled = true;
            st.simulator.enabled = true;
            st.simulator.lid_open = sim_snap.lid_open;
            st.simulator.battery = batt.clone();
            st.battery = batt;
            st.is_virtual_session = true;
            let mid = st.simulator.model_id.clone();
            (
                mid,
                BluetoothAddress::from_bytes(sim_snap.class_mac).to_mac_string(),
            )
        };
        self.persist_current()?;
        self.connect_device(&addr, Some(&model_id)).await
    }

    pub async fn set_simulator_model(&self, model_id: &str) -> CoreResult<AppSnapshot> {
        let next_sim = self
            .shared
            .simulator
            .switch_model(model_id)
            .map_err(CoreError::UnknownModel)?;
        let addr = BluetoothAddress::from_bytes(next_sim.class_mac).to_mac_string();
        if self.get_app_snapshot().settings.simulator_enabled {
            return self.connect_device(&addr, Some(model_id)).await;
        }
        let profile = self
            .shared
            .registry
            .find_by_id(model_id)
            .cloned()
            .ok_or_else(|| CoreError::UnknownModel(model_id.to_string()))?;
        let snap = {
            let mut st = self.shared.snapshot.lock().expect("lock");
            st.active_model = profile.clone();
            st.simulator.model_id = profile.model_id.clone();
            st.ota.ota_type = profile.ota_type;
            st.clone()
        };
        self.emit_state(snap.clone());
        Ok(snap)
    }

    pub async fn update_simulator_state(&self, patch: SimulatorPatch) -> CoreResult<AppSnapshot> {
        let _ = self.ensure_active_virtual_link().await;
        if let Some(ms) = patch.latency_ms {
            self.shared.simulator.set_latency_ms(ms);
            self.shared
                .snapshot
                .lock()
                .expect("lock")
                .simulator
                .latency_ms = ms;
        }
        if let Some(lid) = patch.lid_open {
            self.shared
                .snapshot
                .lock()
                .expect("lock")
                .simulator
                .lid_open = lid;
        }
        if let Some(b) = patch.battery {
            let cur = self.shared.simulator.snapshot();
            let left = b.left.unwrap_or(cur.left_battery);
            let right = b.right.unwrap_or(cur.right_battery);
            let case = b.case_level.unwrap_or(cur.case_battery);
            self.shared.simulator.set_battery(left, right, case);

            let lc = b.left_charging.unwrap_or(cur.left_charging);
            let rc = b.right_charging.unwrap_or(cur.right_charging);
            let cc = b.case_charging.unwrap_or(cur.case_charging);
            self.shared.simulator.set_charging(lc, rc, cc);

            if b.left_in_case.is_some() || b.right_in_case.is_some() {
                let in_l = !b.left_in_case.unwrap_or(!cur.in_ear_left);
                let in_r = !b.right_in_case.unwrap_or(!cur.in_ear_right);
                self.shared.simulator.set_in_ear(in_l, in_r);
            }

            let mut st = self.shared.snapshot.lock().expect("lock");
            let has_case = st.active_model.capabilities.has_case_battery;
            let mut next_batt = st.simulator.battery.clone();
            next_batt.left = Some(left);
            next_batt.right = if has_case { Some(right) } else { None };
            next_batt.case_level = if has_case { Some(case) } else { None };
            next_batt.left_charging = lc;
            next_batt.right_charging = rc;
            next_batt.case_charging = cc;
            if let Some(lic) = b.left_in_case {
                next_batt.left_in_case = lic;
            }
            if let Some(ric) = b.right_in_case {
                next_batt.right_in_case = ric;
            }
            if st.settings.simulator_enabled {
                st.simulator.battery = next_batt.clone();
                st.battery = next_batt;
            }
        }
        let snap = self.get_app_snapshot();
        self.emit_state(snap.clone());
        Ok(snap)
    }

    pub fn trigger_simulator_popup(&self) -> PopupTriggerPayload {
        let dev = self.shared.simulator.trigger_lid_open_popup();
        handle_scanner_event(&self.shared, ConnectionEvent::AdvertisementSeen(dev), true);
        let payload = self.build_popup_payload();
        let _ = self
            .shared
            .events_tx
            .send(CoreEvent::PopupTriggered(payload.clone()));
        payload
    }

    pub fn build_popup_payload(&self) -> PopupTriggerPayload {
        let st = self.get_app_snapshot();
        PopupTriggerPayload {
            device_name: st
                .active_device
                .as_ref()
                .map(|d| d.name.clone())
                .unwrap_or_else(|| st.active_model.name_en.clone()),
            model_id: st.active_model.model_id.clone(),
            form_factor: st.active_model.form_factor,
            color_index: st.device_color_index,
            battery: st.battery,
            anc_mode: st.anc.mode,
            is_virtual: st.is_virtual_session,
            auto_dismiss_ms: (st.settings.popup_duration_sec as u64) * 1000,
        }
    }

    pub fn update_app_settings(&self, patch: AppSettingsPatch) -> CoreResult<AppSnapshot> {
        let snap = {
            let mut st = self.shared.snapshot.lock().expect("lock");
            if let Some(locale) = patch.locale {
                st.settings.locale = locale;
            }
            if let Some(theme) = patch.theme {
                st.settings.theme = theme;
            }
            if let Some(v) = patch.minimize_to_tray {
                st.settings.minimize_to_tray = v;
            }
            if let Some(v) = patch.show_connection_popup {
                st.settings.show_connection_popup = v;
            }
            if let Some(sec) = patch.popup_duration_sec {
                st.settings.popup_duration_sec = sec.clamp(2, 30);
            }
            if let Some(v) = patch.auto_start {
                st.settings.auto_start = v;
            }
            if let Some(v) = patch.simulator_enabled {
                st.settings.simulator_enabled = v;
                st.simulator.enabled = v;
                if !v && st.is_virtual_session {
                    st.is_virtual_session = false;
                    st.connection_state = ConnectionState::Disconnected;
                    st.active_device = None;
                    st.active_model = ModelRegistry::generic_fallback();
                    st.battery = BatteryTelemetry::default();
                    st.simulator.battery = BatteryTelemetry::default();
                    st.firmware_version.clear();
                    st.multipoint = MultipointState::default();
                    st.saved_devices.retain(|d| !is_simulator_saved_device(d));
                }
            }
            if let Some(v) = patch.developer_packet_log {
                st.settings.developer_packet_log = v;
            }
            st.clone()
        };
        self.persist_current()?;
        self.emit_state(snap.clone());
        Ok(snap)
    }

    pub fn get_protocol_logs(&self) -> Vec<ProtocolLogEntry> {
        self.shared.logs.lock().expect("logs lock").clone()
    }

    pub fn clear_protocol_logs(&self) {
        self.shared.logs.lock().expect("logs lock").clear();
    }

    // -----------------------------------------------------------------------
    // Internal Link, Wire Dispatch, Pacing, Conflict Guard & Persistence
    // -----------------------------------------------------------------------

    async fn ensure_active_virtual_link(&self) -> CoreResult<()> {
        let has_link = self.shared.link.lock().await.is_some();
        if has_link {
            return Ok(());
        }
        let snap = self.get_app_snapshot();
        if snap.settings.simulator_enabled {
            let model_id = snap.simulator.model_id;
            let addr = BluetoothAddress::from_bytes(self.shared.simulator.snapshot().class_mac)
                .to_mac_string();
            let _ = self.connect_device(&addr, Some(&model_id)).await?;
        }
        Ok(())
    }

    async fn attach_connection(
        &self,
        connection: Arc<dyn BluetoothConnection>,
        profile: DeviceModelProfile,
        display_name: String,
        is_virtual: bool,
    ) -> CoreResult<AppSnapshot> {
        let services = connection
            .discover_services()
            .await
            .map_err(|e| CoreError::Transport(e.to_string()))?;
        let svc_uuids: Vec<Uuid> = services.iter().map(|s| s.uuid).collect();
        let char_uuids: Vec<Uuid> = services
            .iter()
            .flat_map(|s| s.characteristics.iter().map(|c| c.uuid))
            .collect();

        let driver: Arc<dyn ProtocolDriver> =
            Arc::from(ProtocolRouter::select_driver_with_characteristics(
                profile.numeric_model_id,
                &svc_uuids,
                &char_uuids,
                connection.mtu(),
            ));

        let mut notify_tasks = Vec::new();
        for (svc, ch) in driver.required_notify_characteristics() {
            if let Ok(mut rx) = connection.subscribe_gatt_notifications(svc, ch).await {
                let shared = Arc::clone(&self.shared);
                let drv = Arc::clone(&driver);
                let transport_label = if is_virtual { "virtual" } else { "ble_gatt" }.to_string();
                let handle = tokio::spawn(async move {
                    while let Some(raw) = rx.recv().await {
                        push_log(
                            &shared,
                            "rx",
                            &transport_label,
                            &ch.to_string(),
                            &raw,
                            "gatt_notify",
                        );
                        if let Ok(events) = drv.decode_notification(ch, &raw) {
                            let updated = {
                                let mut st = shared.snapshot.lock().expect("snapshot lock");
                                for ev in &events {
                                    state_sync::apply_device_event(&mut st, ev);
                                }
                                st.clone()
                            };
                            let _ = shared
                                .events_tx
                                .send(CoreEvent::StateUpdated(Box::new(updated)));
                        }
                    }
                });
                notify_tasks.push(handle);
            }
        }

        // Store the active link (any prior link was already torn down before connect)
        {
            let mut guard = self.shared.link.lock().await;
            if let Some(mut prev) = guard.take() {
                prev.abort_tasks();
            }
            *guard = Some(ActiveLink {
                connection: Arc::clone(&connection),
                driver: Arc::clone(&driver),
                is_virtual,
                notify_tasks,
            });
        }

        let addr_str = connection.address().to_mac_string();
        {
            let mut st = self.shared.snapshot.lock().expect("lock");
            st.connection_state = ConnectionState::Connected;
            st.is_virtual_session = is_virtual;
            st.active_model = profile.clone();
            st.ota.ota_type = profile.ota_type;
            if is_virtual {
                let sim_snap = self.shared.simulator.snapshot();
                let batt = sim_state_to_battery(&sim_snap, &profile);
                st.simulator.model_id = profile.model_id.clone();
                st.simulator.battery = batt.clone();
                st.battery = batt;
            } else {
                st.battery = BatteryTelemetry::default();
            }

            st.active_device = Some(DiscoveredDeviceInfo {
                id: format!(
                    "{}:{}",
                    if is_virtual { "virtual" } else { "ble" },
                    addr_str
                ),
                address: addr_str.clone(),
                name: display_name.clone(),
                rssi: Some(-48),
                transport_kind: connection.transport_kind(),
                manufacturer_data: HashMap::new(),
                service_uuids: svc_uuids.iter().map(ToString::to_string).collect(),
                is_paired: true,
                is_connected: true,
                last_seen_ms: now_epoch_ms(),
                model_id_hint: Some(profile.model_id.clone()),
                numeric_model_id: profile.numeric_model_id,
                is_virtual,
            });

            let rec = SavedDeviceRecord {
                address: addr_str.clone(),
                name: display_name,
                model_id: profile.model_id.clone(),
                color_index: st.device_color_index,
                last_connected_ms: now_epoch_ms(),
                last_battery: Some(st.battery.clone()),
                is_virtual,
            };
            if let Some(existing) = st.saved_devices.iter_mut().find(|d| d.address == addr_str) {
                *existing = rec;
            } else {
                st.saved_devices.push(rec);
            }
        }

        let _ = self.persist_current();
        // Hydrate device info (firmware version, battery, ANC) over real wire frames
        let _ = self
            .send_protocol_command(&DeviceCommand::RequestDeviceInfo, "hydrate_device_info")
            .await;
        let _ = self
            .send_protocol_command(
                &DeviceCommand::RequestAttribute {
                    cmd: CmdId::ButtonFunc,
                },
                "hydrate_buttons",
            )
            .await;

        let latest = self.get_app_snapshot();
        self.emit_state(latest.clone());
        Ok(latest)
    }

    async fn send_protocol_command(&self, cmd: &DeviceCommand, summary: &str) -> CoreResult<()> {
        let (conn, writes, is_virtual) = {
            let guard = self.shared.link.lock().await;
            let Some(link) = guard.as_ref() else {
                return Ok(());
            };
            let writes = match link.driver.encode_command(cmd) {
                Ok(w) => w,
                Err(qcy_protocol::ProtocolError::UnsupportedCommand) => return Ok(()),
                Err(e) => return Err(CoreError::Protocol(e.to_string())),
            };
            (Arc::clone(&link.connection), writes, link.is_virtual)
        };

        let transport_label = if is_virtual { "virtual" } else { "ble_gatt" };
        for (idx, w) in writes.into_iter().enumerate() {
            if idx > 0 && !is_virtual {
                let delay = w.pacing_ms.max(COMMAND_PACING_MS);
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
            self.execute_wire_write(&conn, &w, transport_label, summary)
                .await?;
        }
        Ok(())
    }

    async fn execute_wire_write(
        &self,
        conn: &Arc<dyn BluetoothConnection>,
        w: &WireWrite,
        transport_label: &str,
        summary: &str,
    ) -> CoreResult<()> {
        if w.is_read {
            let raw = conn
                .read_gatt(w.service, w.characteristic)
                .await
                .map_err(|e| CoreError::Transport(e.to_string()))?;
            push_log(
                &self.shared,
                "rx",
                transport_label,
                &w.characteristic.to_string(),
                &raw,
                summary,
            );
            let driver = {
                self.shared
                    .link
                    .lock()
                    .await
                    .as_ref()
                    .map(|l| Arc::clone(&l.driver))
            };
            if let Some(drv) = driver {
                if let Ok(events) = drv.decode_notification(w.characteristic, &raw) {
                    let mut st = self.shared.snapshot.lock().expect("lock");
                    for ev in &events {
                        state_sync::apply_device_event(&mut st, ev);
                    }
                }
            }
        } else {
            push_log(
                &self.shared,
                "tx",
                transport_label,
                &w.characteristic.to_string(),
                &w.payload,
                summary,
            );
            conn.write_gatt(w.service, w.characteristic, &w.payload, w.with_response)
                .await
                .map_err(|e| CoreError::Transport(e.to_string()))?;
        }
        Ok(())
    }

    async fn write_ota_packet(
        &self,
        route: qcy_protocol::OtaRouteInfo,
        pkt: &[u8],
        label: &str,
    ) -> CoreResult<()> {
        let (conn, is_virtual) = {
            let guard = self.shared.link.lock().await;
            let link = guard.as_ref().ok_or(CoreError::NotConnected)?;
            (Arc::clone(&link.connection), link.is_virtual)
        };
        let transport_label = if is_virtual {
            "virtual_ota"
        } else if route.transport == OtaTransport::BleGatt {
            "ble_ota"
        } else {
            "rfcomm_ota"
        };

        if route.transport == OtaTransport::BleGatt {
            let write_char = route
                .write_char_uuid
                .ok_or_else(|| CoreError::Ota("missing BLE OTA write UUID".to_string()))?;
            push_log(
                &self.shared,
                "tx",
                transport_label,
                &write_char.to_string(),
                pkt,
                label,
            );
            conn.write_gatt(route.service_uuid, write_char, pkt, true)
                .await
                .map_err(|e| CoreError::Ota(e.to_string()))?;
        } else {
            push_log(
                &self.shared,
                "tx",
                transport_label,
                &route.service_uuid.to_string(),
                pkt,
                label,
            );
            conn.open_rfcomm(route.service_uuid)
                .await
                .map_err(|e| CoreError::Ota(e.to_string()))?;
            conn.write_rfcomm(pkt)
                .await
                .map_err(|e| CoreError::Ota(e.to_string()))?;
        }
        Ok(())
    }

    fn set_ota_unsupported_auth(&self, file_path: Option<String>) -> OtaState {
        let cur_fw = self.get_app_snapshot().firmware_version;
        let state = OtaState {
            phase: OtaPhaseUi::UnsupportedAuthRequired,
            ota_type: OtaType::JieLiBleAuth.as_u8(),
            current_firmware: cur_fw,
            file_path,
            progress_pct: 0.0,
            message: Some("ota.jieli_v2_auth_unsupported".to_string()),
        };
        self.update_ota_state(state.clone());
        state
    }

    fn update_ota_state(&self, ota: OtaState) {
        let snap = {
            let mut st = self.shared.snapshot.lock().expect("lock");
            st.ota = ota;
            st.clone()
        };
        self.emit_state(snap);
    }

    fn check_conflicts(
        &self,
        feature_key: &str,
        target_value: i32,
    ) -> Option<MutualExclusionPrompt> {
        let snap = self.get_app_snapshot();
        let current_state = state_sync::build_feature_state_map(&snap);
        let hits = self.shared.registry.evaluate_pop_view_rules_value(
            &snap.active_model.model_id,
            feature_key,
            target_value,
            &current_state,
        );
        // Prefer conflict rules with explicit conflicting features over generic reboot notices
        let chosen = hits
            .iter()
            .find(|h| !h.conflicting_features.is_empty())
            .or_else(|| hits.first())?;
        let source_rule = snap.active_model.pop_view_rules.iter().find(|r| {
            r.target_feature == chosen.target_feature && r.target_cmd_id == chosen.target_cmd_id
        });
        Some(MutualExclusionPrompt::from_rule_match(chosen, source_rule))
    }

    fn persist_current(&self) -> CoreResult<()> {
        let persisted = {
            let st = self.shared.snapshot.lock().expect("lock");
            PersistedState {
                schema_version: CURRENT_SCHEMA_VERSION,
                settings: st.settings.clone(),
                custom_presets: st.eq.custom_presets.clone(),
                saved_devices: st
                    .saved_devices
                    .iter()
                    .filter(|d| !is_simulator_saved_device(d))
                    .cloned()
                    .collect(),
            }
        };
        self.shared.storage.save(&persisted)
    }

    fn emit_state(&self, snap: AppSnapshot) {
        let _ = self
            .shared
            .events_tx
            .send(CoreEvent::StateUpdated(Box::new(snap)));
    }
}

fn push_log(
    shared: &CoreShared,
    direction: &str,
    transport: &str,
    uuid: &str,
    bytes: &[u8],
    summary: &str,
) {
    let entry = ProtocolLogEntry {
        timestamp_ms: now_epoch_ms(),
        direction: direction.to_string(),
        transport: transport.to_string(),
        uuid: uuid.to_string(),
        hex: state_sync::bytes_to_hex(bytes),
        summary: summary.to_string(),
    };
    {
        let mut logs = shared.logs.lock().expect("logs lock");
        logs.push(entry.clone());
        if logs.len() > MAX_PROTOCOL_LOGS {
            logs.remove(0);
        }
    }
    let _ = shared.events_tx.send(CoreEvent::ProtocolLog(entry));
}

fn parse_qcy_adv_from_raw(raw: &RawDiscoveredDevice) -> Option<QcyAdvertisement> {
    for company_id in QCY_COMPANY_IDS {
        if let Some(payload) = raw.manufacturer_data.get(&company_id) {
            if let Ok(adv) = QcyAdvertisement::parse(company_id, payload) {
                return Some(adv);
            }
        }
    }
    None
}

fn enrich_discovered_device(
    registry: &ModelRegistry,
    raw: &RawDiscoveredDevice,
    is_virtual: bool,
) -> DiscoveredDeviceInfo {
    let adv = parse_qcy_adv_from_raw(raw);
    let matched = registry.match_discovered_device(Some(&raw.name), adv.as_ref());
    DiscoveredDeviceInfo::from_raw(
        raw,
        Some(matched.model_id.clone()),
        adv.as_ref()
            .map(|a| a.model_id)
            .or(matched.numeric_model_id),
        is_virtual,
    )
}

fn handle_scanner_event(shared: &CoreShared, evt: ConnectionEvent, is_virtual: bool) {
    match evt {
        ConnectionEvent::AdvertisementSeen(raw) => {
            let enriched = enrich_discovered_device(&shared.registry, &raw, is_virtual);
            let _ = shared
                .events_tx
                .send(CoreEvent::ScanDiscovered(enriched.clone()));

            if let Some(adv) = parse_qcy_adv_from_raw(&raw) {
                let model = shared
                    .registry
                    .match_discovered_device(Some(&raw.name), Some(&adv));
                let has_case = model.capabilities.has_case_battery;
                let battery = BatteryTelemetry {
                    left: adv.left_battery.percent,
                    right: if has_case {
                        adv.right_battery.percent
                    } else {
                        None
                    },
                    case_level: if has_case {
                        adv.case_battery.percent
                    } else {
                        None
                    },
                    left_charging: adv.left_battery.charging,
                    right_charging: adv.right_battery.charging,
                    case_charging: adv.case_battery.charging,
                    left_in_case: false,
                    right_in_case: false,
                    is_tws_connected: adv.is_tws,
                };

                // Deduplicate lid-open popup by (address, pop_num)
                if adv.is_pop {
                    let should_popup = {
                        let st = shared.snapshot.lock().expect("lock");
                        if !st.settings.show_connection_popup {
                            false
                        } else if let Some(pop_num) = adv.pop_num {
                            let mut seen = shared.last_pop_num.lock().expect("pop_num lock");
                            let prev = seen.insert(enriched.address.clone(), pop_num);
                            prev != Some(pop_num)
                        } else {
                            true
                        }
                    };
                    if should_popup {
                        let st = shared.snapshot.lock().expect("lock");
                        let payload = PopupTriggerPayload {
                            device_name: if raw.name.is_empty() {
                                model.name_en.clone()
                            } else {
                                raw.name.clone()
                            },
                            model_id: model.model_id.clone(),
                            form_factor: model.form_factor,
                            color_index: adv.color_index,
                            battery,
                            anc_mode: st.anc.mode,
                            is_virtual,
                            auto_dismiss_ms: (st.settings.popup_duration_sec as u64) * 1000,
                        };
                        let _ = shared.events_tx.send(CoreEvent::PopupTriggered(payload));
                    }
                }
            }
        }
        ConnectionEvent::Disconnected { .. } => {
            let snap = {
                let mut st = shared.snapshot.lock().expect("lock");
                st.connection_state = ConnectionState::Disconnected;
                st.clone()
            };
            let _ = shared
                .events_tx
                .send(CoreEvent::StateUpdated(Box::new(snap)));
        }
        ConnectionEvent::Connected { .. } => {}
    }
}

fn sim_state_to_battery(sim: &VirtualDeviceState, model: &DeviceModelProfile) -> BatteryTelemetry {
    let has_case = model.capabilities.has_case_battery;
    BatteryTelemetry {
        left: Some(sim.left_battery),
        right: if has_case {
            Some(sim.right_battery)
        } else {
            None
        },
        case_level: if has_case {
            Some(sim.case_battery)
        } else {
            None
        },
        left_charging: sim.left_charging,
        right_charging: sim.right_charging,
        case_charging: sim.case_charging,
        left_in_case: !sim.in_ear_left,
        right_in_case: !sim.in_ear_right,
        is_tws_connected: sim.is_tws,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_storage_path(name: &str) -> PathBuf {
        let n = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "qcy_core_test_{name}_{}_{n}.json",
            std::process::id()
        ))
    }

    #[tokio::test]
    async fn app_core_provides_initial_snapshot() {
        let path = temp_storage_path("initial");
        let core = AppCore::with_storage_path(&path);
        let full = core.get_app_snapshot();
        assert!(!full.settings.simulator_enabled);
        assert!(!full.simulator.enabled);
        assert!(!full.is_virtual_session);
        assert_eq!(full.connection_state, ConnectionState::Disconnected);
        assert!(full.active_device.is_none());
        assert_eq!(full.battery, BatteryTelemetry::default());
        assert!(full.battery.left.is_none());
        assert!(full.battery.right.is_none());
        assert!(full.battery.case_level.is_none());
        assert!(full.saved_devices.is_empty());
        assert!(full.multipoint.hosts.is_empty());

        let legacy_snap = core.status_snapshot().await;
        assert_eq!(legacy_snap.connection_state, ConnectionState::Disconnected);
        assert!(legacy_snap.active_device.is_none());
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn simulator_toggle_off_clears_virtual_device_and_battery() {
        let path = temp_storage_path("sim_toggle_off");
        let core = AppCore::with_storage_path(&path);
        let mut events = core.subscribe_events();

        // Explicit opt-in connects virtual N70 with live battery
        let on_snap = core
            .set_simulator_enabled(true)
            .await
            .expect("enable simulator");
        assert!(on_snap.settings.simulator_enabled);
        assert!(on_snap.is_virtual_session);
        assert_eq!(on_snap.connection_state, ConnectionState::Connected);
        assert!(on_snap.active_device.is_some());
        assert!(on_snap.battery.left.is_some());

        // Disabling simulator disconnects immediately, clears device/battery/virtual saved records, and emits StateUpdated
        let off_snap = core
            .set_simulator_enabled(false)
            .await
            .expect("disable simulator");
        assert!(!off_snap.settings.simulator_enabled);
        assert!(!off_snap.simulator.enabled);
        assert!(!off_snap.is_virtual_session);
        assert_eq!(off_snap.connection_state, ConnectionState::Disconnected);
        assert!(off_snap.active_device.is_none());
        assert_eq!(off_snap.battery, BatteryTelemetry::default());
        assert!(off_snap.saved_devices.iter().all(|d| !d.is_virtual));

        let mut saw_disconnected_event = false;
        while let Ok(evt) = events.try_recv() {
            if let CoreEvent::StateUpdated(s) = evt {
                if s.connection_state == ConnectionState::Disconnected && s.active_device.is_none()
                {
                    saw_disconnected_event = true;
                }
            }
        }
        assert!(
            saw_disconnected_event,
            "disabling simulator must broadcast CoreEvent::StateUpdated with Disconnected state"
        );

        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn v1_to_v2_storage_migration_disables_default_simulator_and_scrubs_virtual_devices() {
        let path = temp_storage_path("v1_migration");

        // Write a legacy v0.1.0 / v0.1.1 state.json (missing schema_version, simulator_enabled=true, virtual + real devices)
        let legacy_json = serde_json::json!({
            "settings": {
                "locale": "en-US",
                "theme": "dark",
                "minimize_to_tray": true,
                "show_connection_popup": true,
                "popup_duration_sec": 6,
                "auto_start": false,
                "virtual_simulator_enabled": true,
                "developer_packet_log": false
            },
            "custom_presets": [
                {
                    "id": "user-flat",
                    "eq_index": 99,
                    "category": "custom",
                    "name_en": "User Flat",
                    "name_zh": "用户平直",
                    "master_gain_db": 0.0,
                    "gains_10band_db": [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                    "parametric_bands": []
                }
            ],
            "saved_devices": [
                {
                    "address": "84:AC:60:5D:40:01",
                    "name": "QCY MeloBuds N70",
                    "model_id": "qcy-n70",
                    "color_index": 0,
                    "last_connected_ms": 1000,
                    "last_battery": null,
                    "is_virtual": true
                },
                {
                    "address": "AA:BB:CC:11:22:33",
                    "name": "Simulated Earbuds",
                    "model_id": "qcy-ht08",
                    "color_index": 0,
                    "last_connected_ms": 1001,
                    "last_battery": null,
                    "is_virtual": false
                },
                {
                    "address": "84:AC:60:99:88:77",
                    "name": "Real QCY HT08",
                    "model_id": "qcy-ht08",
                    "color_index": 1,
                    "last_connected_ms": 2000,
                    "last_battery": null,
                    "is_virtual": false
                }
            ]
        });
        std::fs::write(&path, serde_json::to_string_pretty(&legacy_json).unwrap())
            .expect("write legacy state.json");

        // Loading via AppCore must migrate v0/v1 -> v2 cleanly
        let core = AppCore::with_storage_path(&path);
        let snap = core.get_app_snapshot();
        assert!(!snap.settings.simulator_enabled);
        assert!(!snap.is_virtual_session);
        assert_eq!(snap.connection_state, ConnectionState::Disconnected);
        assert!(snap.active_device.is_none());
        assert_eq!(snap.battery, BatteryTelemetry::default());
        // User's real settings, custom presets, and real Bluetooth saved devices survive migration!
        assert_eq!(snap.settings.locale, "en-US");
        assert_eq!(snap.settings.theme, AppTheme::Dark);
        assert_eq!(snap.eq.custom_presets.len(), 1);
        assert_eq!(snap.saved_devices.len(), 1);
        assert_eq!(snap.saved_devices[0].address, "84:AC:60:99:88:77");

        // Verify persisted file on disk was upgraded to schema_version == 2
        let persisted_raw = std::fs::read_to_string(&path).expect("read migrated file");
        let persisted: PersistedState = serde_json::from_str(&persisted_raw).expect("parse v2");
        assert_eq!(persisted.schema_version, CURRENT_SCHEMA_VERSION);
        assert!(!persisted.settings.simulator_enabled);
        assert_eq!(persisted.saved_devices.len(), 1);

        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn end_to_end_virtual_models_anc_eq_gestures_conflicts_ota_and_persistence() {
        let path = temp_storage_path("e2e");
        let core = AppCore::with_storage_path(&path);
        let mut events = core.subscribe_events();

        // Explicit simulator opt-in before running raw-byte virtual hardware E2E
        core.set_simulator_enabled(true)
            .await
            .expect("enable simulator");

        // 1. Connect & switch across Unified TLV (N70), JieLi New (HT08-JL), JieLi Legacy (H3), and SP75 RGB
        let n70 = core.set_simulator_model("qcy-n70").await.expect("n70");
        assert_eq!(n70.active_model.model_id, "qcy-n70");
        assert_eq!(n70.connection_state, ConnectionState::Connected);

        // 2. ANC Custom Depth + AI-ANC + ANC Button Cycle over real wire bytes
        let anc_out = core
            .set_anc_state(SetAncPayload {
                mode: AncPrimaryMode::Custom,
                custom_level: Some(14),
                wind_noise: Some(true),
                ..Default::default()
            })
            .await
            .expect("set anc");
        assert!(anc_out.applied);
        assert_eq!(anc_out.snapshot.anc.mode, AncPrimaryMode::Custom);
        assert_eq!(anc_out.snapshot.anc.custom_level, 14);

        let ai_out = core
            .set_ai_anc(true, Some(AiAncMatrix::default()))
            .await
            .expect("ai anc");
        assert!(ai_out.snapshot.anc.ai_anc_enabled);

        let cyc_out = core
            .set_anc_button_cycle(vec!["anc".to_string(), "off".to_string()])
            .await
            .expect("btn cycle");
        assert_eq!(cyc_out.snapshot.anc.btn_anc_cfg, vec!["anc", "off"]);

        // 3. EQ preset, Parametric EQ, Custom preset CRUD, JSON Import/Export & Personalized L/R EQ
        let eq_pop = core.apply_eq_preset("pop").await.expect("pop");
        assert_eq!(eq_pop.snapshot.eq.active_preset_id, "pop");

        let peq_out = core
            .set_parametric_eq(
                vec![WireParametricBand {
                    freq_hz: 120,
                    gain_db: 3.5,
                    q: 1.0,
                    irr_type: 4,
                    enabled: true,
                }],
                Some(-1.5),
            )
            .await
            .expect("peq");
        assert_eq!(peq_out.snapshot.eq.parametric_bands.len(), 1);

        let saved_eq = core
            .save_custom_eq(EqPresetItem {
                id: "my-studio".to_string(),
                eq_index: 99,
                category: "custom".to_string(),
                name_en: "My Studio".to_string(),
                name_zh: "我的监听".to_string(),
                subtitle_en: None,
                subtitle_zh: None,
                master_gain_db: -1.0,
                gains_10band_db: vec![1.0; 10],
                parametric_bands: vec![],
            })
            .expect("save custom eq");
        assert_eq!(saved_eq.eq.custom_presets.len(), 1);
        let exported = core.export_eq_json(Some("my-studio")).expect("export");
        assert!(exported.contains("My Studio"));
        core.delete_custom_eq("my-studio").expect("delete");
        let reimported = core.import_eq_json(&exported).expect("import");
        assert_eq!(reimported.eq.custom_presets.len(), 1);

        let pers = core
            .apply_personalized_eq(vec![1.0; 8], vec![2.0; 8], vec![1.5; 10], Some(0.2))
            .await
            .expect("personalized eq");
        assert!(pers.snapshot.eq.personalized_enabled);

        // 4. Gestures, Touch Sensitivity & IMU Head Controls
        let btn = core.set_button_action(1, 3, 11).await.expect("btn");
        assert!(btn
            .snapshot
            .gestures
            .mappings
            .iter()
            .any(|m| m.event_id == 3 && m.cmd == 11));
        let hc = core
            .set_head_controls(SetHeadControlsPatch {
                enabled: Some(true),
                nod_cmd: Some(1),
                ..Default::default()
            })
            .await
            .expect("head controls");
        assert!(hc.snapshot.gestures.head_controls.enabled);

        // 5. popViewRules Conflict Guard on HT08: Dual-Device ON + LDAC ON
        core.set_simulator_model("qcy-ht08").await.expect("ht08");
        core.set_multipoint(SetMultipointPayload {
            dual_device: Some(true),
            force: true,
            ..Default::default()
        })
        .await
        .expect("dual device on");

        let blocked = core
            .set_modes(SetModesPayload {
                ldac: Some(true),
                force: false,
                ..Default::default()
            })
            .await
            .expect("ldac conflict check");
        assert!(!blocked.applied, "LDAC should prompt when dual_device=true");
        let prompt = blocked.conflict_prompt.expect("conflict prompt");
        assert!(prompt.requires_reboot);
        assert!(prompt
            .conflicting_features
            .iter()
            .any(|f| f == "dual_device"));

        let forced = core
            .set_modes(SetModesPayload {
                ldac: Some(true),
                force: true,
                ..Default::default()
            })
            .await
            .expect("ldac forced");
        assert!(forced.applied);
        assert!(forced.snapshot.modes.ldac);
        assert!(!forced.snapshot.multipoint.dual_device);

        // 6. Fit Test + Popup Trigger + Protocol TX/RX Log Capture
        let fit = core.run_fit_test().await.expect("fit test");
        assert_eq!(fit.left, FitTestEarStatus::Good);
        let popup = core.trigger_simulator_popup();
        assert_eq!(popup.model_id, "qcy-ht08");
        assert!(!core.get_protocol_logs().is_empty());

        // 7. SP75 RGB Lighting + Local WuQi OTA simulation
        core.set_simulator_model("qcy-sp75").await.expect("sp75");
        let rgb = core
            .set_rgb_lighting(SetRgbLightingPayload {
                enabled: Some(true),
                mode: Some(SpeakerRgbMode::Rhythm),
                color_hex: Some("#00FF80".to_string()),
                brightness: Some(90),
                speed: Some(70),
                tws_pair: Some(true),
            })
            .await
            .expect("rgb");
        assert_eq!(rgb.snapshot.rgb_lighting.mode, SpeakerRgbMode::Rhythm);
        assert!(rgb.snapshot.rgb_lighting.tws_pair_enabled);

        let ota_ok = core
            .start_local_ota_bytes("sp75_test.ota", vec![0xAB; 512])
            .await
            .expect("sp75 ota");
        assert_eq!(ota_ok.phase, OtaPhaseUi::Completed);
        assert!((ota_ok.progress_pct - 100.0).abs() < 0.1);

        // 8. JieLi New (HT08-JL) & Legacy JieLi (H3) wire round-trips
        core.set_simulator_model("qcy-ht08-jl")
            .await
            .expect("ht08-jl");
        let gm = core
            .set_modes(SetModesPayload {
                game_mode: Some(true),
                force: true,
                ..Default::default()
            })
            .await
            .expect("jl game mode");
        assert!(gm.snapshot.modes.game_mode);

        core.set_simulator_model("qcy-h3").await.expect("h3");
        let ren = core.rename_device("My QCY H3").await.expect("rename");
        assert_eq!(
            ren.snapshot.active_device.as_ref().map(|d| d.name.as_str()),
            Some("My QCY H3")
        );

        // 9. Transparent otaType=9 UnsupportedAuthRequired status
        {
            core.shared
                .snapshot
                .lock()
                .expect("lock")
                .active_model
                .ota_type = 9;
        }
        let auth_ota = core
            .start_local_ota_bytes("jl_auth.ota", vec![0x11; 256])
            .await
            .expect("ota type 9");
        assert_eq!(auth_ota.phase, OtaPhaseUi::UnsupportedAuthRequired);
        assert_eq!(
            auth_ota.message.as_deref(),
            Some("ota.jieli_v2_auth_unsupported")
        );

        // 10. Atomic persistence reload check
        core.update_app_settings(AppSettingsPatch {
            locale: Some("en-US".to_string()),
            theme: Some(AppTheme::Dark),
            popup_duration_sec: Some(8),
            ..Default::default()
        })
        .expect("update settings");
        let reloaded = AppCore::with_storage_path(&path);
        let reloaded_snap = reloaded.get_app_snapshot();
        assert_eq!(reloaded_snap.settings.locale, "en-US");
        assert_eq!(reloaded_snap.settings.theme, AppTheme::Dark);
        assert_eq!(reloaded_snap.settings.popup_duration_sec, 8);
        assert_eq!(reloaded_snap.eq.custom_presets.len(), 1);
        assert!(events.try_recv().is_ok());

        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn verifier_round1_fixes_btn_anc_cycle_multipoint_and_conflict_rules() {
        let path = temp_storage_path("verifier_r1");
        let core = AppCore::with_storage_path(&path);

        // 1. Default btn_anc_cfg uses UI pill labels and all 8 AncControlPanel labels round-trip bidirectionally
        assert_eq!(
            core.get_app_snapshot().anc.btn_anc_cfg,
            vec!["anc_adaptive", "transparency", "off"]
        );
        let all_labels = [
            "anc_adaptive",
            "anc_indoor",
            "anc_commute",
            "anc_outdoor",
            "anc_anti_wind",
            "vocal_boost",
            "transparency",
            "off",
        ];
        for label in all_labels {
            let wire = state_sync::anc_cycle_label_to_setting(label);
            assert_eq!(state_sync::anc_setting_to_cycle_label(wire), label);
        }

        // 2. Initial multipoint.multi_device matches dual_device (no false startup conflict)
        let init = core.get_app_snapshot();
        assert_eq!(init.multipoint.multi_device, init.multipoint.dual_device);

        // Explicitly opt in to virtual simulator before wire-level simulator protocol checks
        core.set_simulator_enabled(true)
            .await
            .expect("enable simulator");

        // 3. set_multipoint(multi_device: Some(true)) syncs dual_device and sends 0x24
        core.set_simulator_model("qcy-n70").await.expect("n70");
        core.clear_protocol_logs();
        let mp = core
            .set_multipoint(SetMultipointPayload {
                multi_device: Some(true),
                force: true,
                ..Default::default()
            })
            .await
            .expect("multi_device on");
        assert!(mp.snapshot.multipoint.dual_device);
        assert!(mp.snapshot.multipoint.multi_device);
        assert!(core
            .get_protocol_logs()
            .iter()
            .any(|l| l.direction == "tx" && l.summary.contains("dual_connect(0x24)")));

        // 4. Spatial audio rules using target_value 2/3 (qcy-ht21 cmd 91) AND value 10 (qcy-c50i cmd 34)
        core.set_simulator_model("qcy-ht21").await.expect("ht21");
        core.set_modes(SetModesPayload {
            ldac: Some(true),
            force: true,
            ..Default::default()
        })
        .await
        .expect("enable ldac");
        let sp_blocked = core
            .set_modes(SetModesPayload {
                spatial_audio: Some(SpatialAudioModeUi::HeadTracking),
                force: false,
                ..Default::default()
            })
            .await
            .expect("spatial check");
        assert!(
            !sp_blocked.applied,
            "HeadTracking (cmd 91 value 3) should conflict with LDAC on qcy-ht21"
        );
        assert!(sp_blocked.conflict_prompt.is_some());

        // 5. Audio enhancers conflict checks trigger when !payload.force (dynamic_eq on qcy-c50s, adaptive_volume on qcy-c50i)
        core.set_simulator_model("qcy-c50s").await.expect("c50s");
        core.set_modes(SetModesPayload {
            ldac: Some(true),
            force: true,
            ..Default::default()
        })
        .await
        .expect("ldac on c50s");
        let dyn_blocked = core
            .set_audio_enhancers(SetAudioEnhancersPayload {
                dynamic_eq: Some(true),
                force: false,
                ..Default::default()
            })
            .await
            .expect("dynamic_eq check");
        assert!(
            !dyn_blocked.applied,
            "dynamic_eq should trigger conflict prompt with LDAC when force=false"
        );
        assert!(dyn_blocked.conflict_prompt.is_some());

        core.set_simulator_model("qcy-c50i").await.expect("c50i");
        core.set_modes(SetModesPayload {
            ldac: Some(true),
            force: true,
            ..Default::default()
        })
        .await
        .expect("ldac on c50i");
        let adapt_blocked = core
            .set_audio_enhancers(SetAudioEnhancersPayload {
                adaptive_volume: Some(true),
                force: false,
                ..Default::default()
            })
            .await
            .expect("adaptive_volume check");
        assert!(
            !adapt_blocked.applied,
            "adaptive_volume should trigger conflict prompt with LDAC on qcy-c50i"
        );
        assert!(adapt_blocked.conflict_prompt.is_some());

        let _ = std::fs::remove_file(&path);
    }
}
