//! Exact 1-to-1 JSON-compatible domain models matching `src/types/qcy.ts` and
//! `src/ipc/client.ts` so `src-tauri/src/commands.rs` can bridge directly over `AppCore`.

use std::collections::HashMap;

use qcy_models::{
    DeviceModelProfile, FormFactor, PopViewRule, PopViewRuleMatch, RuleAction, RuleCompareOp,
};
use qcy_protocol::{AncMode, AncSetting, LedEffectKind};
use qcy_transport::{
    AdapterInfo, ConnectionState, DiscoveredDevice as RawDiscoveredDevice, TransportKind,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Core Errors & Broadcast Events
// ---------------------------------------------------------------------------

#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreError {
    #[error("no active device connection")]
    NotConnected,
    #[error("unknown device model: {0}")]
    UnknownModel(String),
    #[error("invalid payload: {0}")]
    InvalidPayload(String),
    #[error("transport error: {0}")]
    Transport(String),
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("ota error: {0}")]
    Ota(String),
}

pub type CoreResult<T> = Result<T, CoreError>;

/// Events broadcast by `AppCore` to `src-tauri` for forwarding as Tauri IPC events:
/// - `StateUpdated` -> `qcy://state-updated`
/// - `PopupTriggered` -> `qcy://popup-triggered`
/// - `ScanDiscovered` -> `qcy://scan-discovered`
/// - `ProtocolLog` -> `qcy://protocol-log`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", content = "payload", rename_all = "snake_case")]
pub enum CoreEvent {
    StateUpdated(Box<AppSnapshot>),
    PopupTriggered(PopupTriggerPayload),
    ScanDiscovered(DiscoveredDeviceInfo),
    ProtocolLog(ProtocolLogEntry),
}

// ---------------------------------------------------------------------------
// Theme & App Settings (Matches src/types/qcy.ts AppSettings)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppTheme {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSettings {
    pub locale: String,
    pub theme: AppTheme,
    pub minimize_to_tray: bool,
    pub show_connection_popup: bool,
    pub popup_duration_sec: u32,
    pub auto_start: bool,
    #[serde(alias = "virtual_simulator_enabled")]
    pub simulator_enabled: bool,
    pub developer_packet_log: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            locale: "zh-CN".to_string(),
            theme: AppTheme::System,
            minimize_to_tray: true,
            show_connection_popup: true,
            popup_duration_sec: 5,
            auto_start: false,
            simulator_enabled: false,
            developer_packet_log: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSettingsPatch {
    pub locale: Option<String>,
    pub theme: Option<AppTheme>,
    pub minimize_to_tray: Option<bool>,
    pub show_connection_popup: Option<bool>,
    pub popup_duration_sec: Option<u32>,
    pub auto_start: Option<bool>,
    #[serde(alias = "virtual_simulator_enabled")]
    pub simulator_enabled: Option<bool>,
    pub developer_packet_log: Option<bool>,
}

// ---------------------------------------------------------------------------
// Discovered & Saved Devices
// ---------------------------------------------------------------------------

/// Enriched `DiscoveredDevice` matching `src/types/qcy.ts` verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredDeviceInfo {
    pub id: String,
    pub address: String,
    pub name: String,
    pub rssi: Option<i16>,
    pub transport_kind: TransportKind,
    pub manufacturer_data: HashMap<u16, Vec<u8>>,
    pub service_uuids: Vec<String>,
    pub is_paired: bool,
    pub is_connected: bool,
    pub last_seen_ms: u64,
    pub model_id_hint: Option<String>,
    pub numeric_model_id: Option<u16>,
    pub is_virtual: bool,
}

impl DiscoveredDeviceInfo {
    pub fn from_raw(
        raw: &RawDiscoveredDevice,
        model_id_hint: Option<String>,
        numeric_model_id: Option<u16>,
        is_virtual: bool,
    ) -> Self {
        Self {
            id: raw.id.clone(),
            address: raw.address.to_mac_string(),
            name: raw.name.clone(),
            rssi: raw.rssi,
            transport_kind: raw.transport_kind,
            manufacturer_data: raw.manufacturer_data.clone(),
            service_uuids: raw.service_uuids.iter().map(ToString::to_string).collect(),
            is_paired: raw.is_paired,
            is_connected: raw.is_connected,
            last_seen_ms: raw.last_seen_ms,
            model_id_hint,
            numeric_model_id,
            is_virtual,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedDeviceRecord {
    pub address: String,
    pub name: String,
    pub model_id: String,
    pub color_index: u8,
    pub last_connected_ms: u64,
    pub last_battery: Option<BatteryTelemetry>,
    pub is_virtual: bool,
}

/// Alias matching `#37` terminology for Find-My-Earbuds last-connected history.
pub type LastConnectedRecord = SavedDeviceRecord;

// ---------------------------------------------------------------------------
// Conflict Prompts (Matches src/types/qcy.ts MutualExclusionPrompt)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiConflictCondition {
    pub feature: String,
    pub cmd_id: u16,
    /// `"eq"` or `"neq"` matching `src/types/qcy.ts`.
    pub compare: String,
    pub value: i32,
    pub reboot: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiPopViewRule {
    pub target_feature: String,
    pub target_cmd_id: u16,
    pub target_value: i32,
    /// `"confirm_dialog"` or `"toast_blocked"` matching `src/types/qcy.ts`.
    pub action: String,
    pub requires_reboot: bool,
    pub title_key: String,
    pub message_key: String,
    pub reboot_notice_key: Option<String>,
    pub conflicts_with: Vec<UiConflictCondition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutualExclusionPrompt {
    pub rule: UiPopViewRule,
    pub conflicting_features: Vec<String>,
    pub requires_reboot: bool,
    pub title_key: String,
    pub message_key: String,
    pub reboot_notice_key: Option<String>,
}

impl MutualExclusionPrompt {
    pub fn from_rule_match(m: &PopViewRuleMatch, source_rule: Option<&PopViewRule>) -> Self {
        let action_str = match m.action {
            RuleAction::ConfirmDialog => "confirm_dialog",
            RuleAction::Toast => "toast_blocked",
        }
        .to_string();

        let conflicts_with = source_rule
            .map(|r| {
                r.conflicts_with
                    .iter()
                    .map(|c| UiConflictCondition {
                        feature: c.feature.clone(),
                        cmd_id: c.cmd_id,
                        compare: match c.compare {
                            RuleCompareOp::Eq => "eq".to_string(),
                            RuleCompareOp::Ne => "neq".to_string(),
                        },
                        value: c.value,
                        reboot: c.reboot,
                    })
                    .collect()
            })
            .unwrap_or_default();

        let title_key = m
            .title_key
            .clone()
            .unwrap_or_else(|| "rules.conflict_title".to_string());

        Self {
            rule: UiPopViewRule {
                target_feature: m.target_feature.clone(),
                target_cmd_id: m.target_cmd_id,
                target_value: source_rule.and_then(|r| r.target_value).unwrap_or(1),
                action: action_str,
                requires_reboot: m.requires_reboot,
                title_key: title_key.clone(),
                message_key: m.message_key.clone(),
                reboot_notice_key: m.reboot_notice_key.clone(),
                conflicts_with,
            },
            conflicting_features: m.conflicting_features.clone(),
            requires_reboot: m.requires_reboot,
            title_key,
            message_key: m.message_key.clone(),
            reboot_notice_key: m.reboot_notice_key.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Telemetry, ANC, EQ, Gestures, Modes, Multipoint, RGB, Focus, OTA, FitTest
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BatteryTelemetry {
    pub left: Option<u8>,
    pub right: Option<u8>,
    pub case_level: Option<u8>,
    pub left_charging: bool,
    pub right_charging: bool,
    pub case_charging: bool,
    pub left_in_case: bool,
    pub right_in_case: bool,
    pub is_tws_connected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AncPrimaryMode {
    #[default]
    Off,
    Anc,
    Transparency,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AncScene {
    Adaptive,
    Indoor,
    Commute,
    Outdoor,
    AntiWind,
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiAncActivityUi {
    Detecting,
    Stay,
    Walk,
    Run,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiAncNoiseLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiAncNoiseRow {
    pub low: AncPrimaryMode,
    pub medium: AncPrimaryMode,
    pub high: AncPrimaryMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiAncMatrix {
    pub stay: AiAncNoiseRow,
    pub walk: AiAncNoiseRow,
    pub run: AiAncNoiseRow,
}

impl Default for AiAncMatrix {
    fn default() -> Self {
        Self {
            stay: AiAncNoiseRow {
                low: AncPrimaryMode::Off,
                medium: AncPrimaryMode::Anc,
                high: AncPrimaryMode::Anc,
            },
            walk: AiAncNoiseRow {
                low: AncPrimaryMode::Transparency,
                medium: AncPrimaryMode::Anc,
                high: AncPrimaryMode::Anc,
            },
            run: AiAncNoiseRow {
                low: AncPrimaryMode::Transparency,
                medium: AncPrimaryMode::Transparency,
                high: AncPrimaryMode::Anc,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AncState {
    pub mode: AncPrimaryMode,
    pub scene: AncScene,
    pub scene_level: u8,
    pub custom_level: u8,
    pub transparency_level: u8,
    pub vocal_boost: bool,
    pub wind_noise: bool,
    pub ai_anc_enabled: bool,
    pub ai_activity: AiAncActivityUi,
    pub ai_noise_level: AiAncNoiseLevel,
    pub ai_matrix: AiAncMatrix,
    pub btn_anc_cfg: Vec<String>,
}

impl Default for AncState {
    fn default() -> Self {
        Self {
            mode: AncPrimaryMode::Anc,
            scene: AncScene::Adaptive,
            scene_level: 2,
            custom_level: 10,
            transparency_level: 4,
            vocal_boost: false,
            wind_noise: false,
            ai_anc_enabled: false,
            ai_activity: AiAncActivityUi::Stay,
            ai_noise_level: AiAncNoiseLevel::Medium,
            ai_matrix: AiAncMatrix::default(),
            btn_anc_cfg: vec![
                "anc_adaptive".to_string(),
                "transparency".to_string(),
                "off".to_string(),
            ],
        }
    }
}

impl AncState {
    /// Converts the UI `AncState` to the exact 3-byte `0x17` wire `AncSetting`.
    pub fn to_wire_setting(&self) -> AncSetting {
        match self.mode {
            AncPrimaryMode::Off => AncSetting::off(),
            AncPrimaryMode::Custom => AncSetting::custom_depth(self.custom_level.min(15)),
            AncPrimaryMode::Transparency => {
                if self.vocal_boost {
                    AncSetting::vocal_boost_transparency()
                } else {
                    AncSetting::transparency(self.transparency_level.min(6))
                }
            }
            AncPrimaryMode::Anc => match self.scene {
                AncScene::Adaptive => AncSetting::adaptive(),
                AncScene::AntiWind => AncSetting::wind_noise_reduction(),
                AncScene::Indoor | AncScene::Low => {
                    AncSetting::anc_scene(1, self.scene_level.min(2))
                }
                AncScene::Commute | AncScene::Medium => {
                    AncSetting::anc_scene(2, self.scene_level.min(2))
                }
                AncScene::Outdoor | AncScene::High => {
                    AncSetting::anc_scene(3, self.scene_level.min(2))
                }
            },
        }
    }

    /// Updates this UI `AncState` from an incoming 3-byte `0x17` wire `AncSetting`.
    pub fn apply_wire_setting(&mut self, s: AncSetting) {
        match (s.mode, s.sub_scene) {
            (2, _) => self.mode = AncPrimaryMode::Off,
            (5, _) => {
                self.mode = AncPrimaryMode::Custom;
                self.custom_level = s.level.min(15);
            }
            (3, 2) => {
                self.mode = AncPrimaryMode::Transparency;
                self.vocal_boost = true;
            }
            (3, _) => {
                self.mode = AncPrimaryMode::Transparency;
                self.vocal_boost = false;
                if s.level > 0 {
                    self.transparency_level = s.level.min(6);
                }
            }
            (1, 4) => {
                self.mode = AncPrimaryMode::Anc;
                self.scene = AncScene::AntiWind;
            }
            (1, 5) => {
                self.mode = AncPrimaryMode::Anc;
                self.scene = AncScene::Adaptive;
            }
            (1, 1) => {
                self.mode = AncPrimaryMode::Anc;
                self.scene = AncScene::Indoor;
                self.scene_level = s.level.min(2);
            }
            (1, 2) => {
                self.mode = AncPrimaryMode::Anc;
                self.scene = AncScene::Commute;
                self.scene_level = s.level.min(2);
            }
            (1, 3) => {
                self.mode = AncPrimaryMode::Anc;
                self.scene = AncScene::Outdoor;
                self.scene_level = s.level.min(2);
            }
            _ => {}
        }
    }

    pub fn to_legacy_anc_mode(&self) -> AncMode {
        self.to_wire_setting().to_mode_and_depth().0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireParametricBand {
    pub freq_hz: u16,
    pub gain_db: f32,
    pub q: f32,
    pub irr_type: u8,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EqPresetItem {
    pub id: String,
    #[serde(default)]
    pub eq_index: u8,
    #[serde(default = "default_custom_category")]
    pub category: String,
    pub name_en: String,
    pub name_zh: String,
    #[serde(default)]
    pub subtitle_en: Option<String>,
    #[serde(default)]
    pub subtitle_zh: Option<String>,
    #[serde(default)]
    pub master_gain_db: f32,
    pub gains_10band_db: Vec<f32>,
    #[serde(default)]
    pub parametric_bands: Vec<WireParametricBand>,
}

fn default_custom_category() -> String {
    "custom".to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EqModeUi {
    Graphic,
    Parametric,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EqState {
    pub active_preset_id: String,
    pub eq_mode: EqModeUi,
    pub master_gain_db: f32,
    pub gains_10band_db: Vec<f32>,
    pub parametric_bands: Vec<WireParametricBand>,
    pub active_target_curve_id: Option<String>,
    pub personalized_enabled: bool,
    pub personalized_left_db: Vec<f32>,
    pub personalized_right_db: Vec<f32>,
    pub custom_presets: Vec<EqPresetItem>,
    pub dynamic_eq: bool,
    pub thx_eq: bool,
    pub virtual_bass: bool,
    pub adaptive_volume: bool,
    pub channel_balance: f32,
}

impl Default for EqState {
    fn default() -> Self {
        Self {
            active_preset_id: "default".to_string(),
            eq_mode: EqModeUi::Graphic,
            master_gain_db: 0.0,
            gains_10band_db: vec![0.0; 10],
            parametric_bands: Vec::new(),
            active_target_curve_id: None,
            personalized_enabled: false,
            personalized_left_db: vec![0.0; 8],
            personalized_right_db: vec![0.0; 8],
            custom_presets: Vec::new(),
            dynamic_eq: false,
            thx_eq: false,
            virtual_bass: false,
            adaptive_volume: false,
            channel_balance: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ButtonMappingEntry {
    pub key_id: u16,
    pub event_id: u16,
    pub cmd: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeadControlsState {
    pub enabled: bool,
    pub nod_cmd: u8,
    pub shake_cmd: u8,
    pub tilt_left_cmd: u8,
    pub tilt_right_cmd: u8,
}

impl Default for HeadControlsState {
    fn default() -> Self {
        Self {
            enabled: false,
            nod_cmd: 1,
            shake_cmd: 2,
            tilt_left_cmd: 3,
            tilt_right_cmd: 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GestureState {
    pub touch_enable: bool,
    pub touch_sensitivity: u8,
    pub mappings: Vec<ButtonMappingEntry>,
    pub head_controls: HeadControlsState,
}

impl Default for GestureState {
    fn default() -> Self {
        Self {
            touch_enable: true,
            touch_sensitivity: 45,
            mappings: vec![
                ButtonMappingEntry {
                    key_id: 1,
                    event_id: 1,
                    cmd: 0,
                },
                ButtonMappingEntry {
                    key_id: 1,
                    event_id: 3,
                    cmd: 1,
                },
                ButtonMappingEntry {
                    key_id: 1,
                    event_id: 5,
                    cmd: 2,
                },
                ButtonMappingEntry {
                    key_id: 1,
                    event_id: 9,
                    cmd: 11,
                },
                ButtonMappingEntry {
                    key_id: 2,
                    event_id: 2,
                    cmd: 0,
                },
                ButtonMappingEntry {
                    key_id: 2,
                    event_id: 4,
                    cmd: 1,
                },
                ButtonMappingEntry {
                    key_id: 2,
                    event_id: 6,
                    cmd: 3,
                },
                ButtonMappingEntry {
                    key_id: 2,
                    event_id: 10,
                    cmd: 11,
                },
            ],
            head_controls: HeadControlsState::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ListeningModeUi {
    Music,
    Game,
    Movie,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpatialAudioModeUi {
    Off,
    Fixed,
    HeadTracking,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptLanguageUi {
    En,
    Zh,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModesState {
    pub listening_mode: ListeningModeUi,
    pub game_mode: bool,
    pub game_mode_pro: bool,
    pub game_sync_eq: bool,
    pub spatial_audio: SpatialAudioModeUi,
    pub ldac: bool,
    pub le_audio: bool,
    pub in_ear_mode: u8,
    pub in_ear_auto_anc: bool,
    pub in_ear_sensitivity: u8,
    pub hearing_low_vol_remind: bool,
    pub hearing_duration_remind: bool,
    pub hearing_duration_min: u16,
    pub privacy_mode: bool,
    pub privacy_scope: u8,
    pub dialogue_perception: bool,
    pub speak_to_chat_sec: u8,
    pub auto_power_off_min: u16,
    pub disconnect_power_off_min: u16,
    pub power_saving: bool,
    pub disconnect_warning: bool,
    pub prompt_language: PromptLanguageUi,
    pub prompt_volume: u8,
}

impl Default for ModesState {
    fn default() -> Self {
        Self {
            listening_mode: ListeningModeUi::Music,
            game_mode: false,
            game_mode_pro: false,
            game_sync_eq: false,
            spatial_audio: SpatialAudioModeUi::Off,
            ldac: false,
            le_audio: false,
            in_ear_mode: 1,
            in_ear_auto_anc: true,
            in_ear_sensitivity: 2,
            hearing_low_vol_remind: false,
            hearing_duration_remind: false,
            hearing_duration_min: 60,
            privacy_mode: false,
            privacy_scope: 0,
            dialogue_perception: false,
            speak_to_chat_sec: 0,
            auto_power_off_min: 0,
            disconnect_power_off_min: 10,
            power_saving: false,
            disconnect_warning: false,
            prompt_language: PromptLanguageUi::En,
            prompt_volume: 6,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultipointHostInfo {
    pub mac: String,
    pub name: String,
    pub is_this_device: bool,
    pub is_connected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct MultipointState {
    pub dual_device: bool,
    pub multi_device: bool,
    pub hosts: Vec<MultipointHostInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeakerRgbMode {
    Solid,
    SingleBreathing,
    GradientBreathing,
    Rhythm,
}

impl SpeakerRgbMode {
    pub const fn to_wire(self) -> LedEffectKind {
        match self {
            Self::Solid => LedEffectKind::Solid,
            Self::SingleBreathing => LedEffectKind::Breathing,
            Self::GradientBreathing => LedEffectKind::Gradient,
            Self::Rhythm => LedEffectKind::MusicRhythm,
        }
    }

    pub const fn from_wire(k: LedEffectKind) -> Self {
        match k {
            LedEffectKind::Solid => Self::Solid,
            LedEffectKind::Breathing => Self::SingleBreathing,
            LedEffectKind::Gradient => Self::GradientBreathing,
            LedEffectKind::MusicRhythm => Self::Rhythm,
            LedEffectKind::Raw(_) => Self::Solid,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RgbLightingState {
    pub enabled: bool,
    pub mode: SpeakerRgbMode,
    pub color_hex: String,
    pub brightness: u8,
    pub speed: u8,
    pub tws_pair_enabled: bool,
}

impl Default for RgbLightingState {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: SpeakerRgbMode::Solid,
            color_hex: "#FF7A2F".to_string(),
            brightness: 80,
            speed: 50,
            tws_pair_enabled: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FocusWhiteNoiseState {
    pub hardware_playing: bool,
    pub hardware_track_index: u32,
    pub hardware_volume: u8,
    pub sync_progress: Option<u8>,
}

impl Default for FocusWhiteNoiseState {
    fn default() -> Self {
        Self {
            hardware_playing: false,
            hardware_track_index: 0,
            hardware_volume: 65,
            sync_progress: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OtaPhaseUi {
    Idle,
    Validating,
    Transferring,
    Verifying,
    Rebooting,
    Completed,
    UnsupportedAuthRequired,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OtaState {
    pub phase: OtaPhaseUi,
    pub ota_type: u8,
    pub current_firmware: String,
    pub file_path: Option<String>,
    pub progress_pct: f32,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FitTestEarStatus {
    Untested,
    Good,
    Fair,
    Failed,
}

impl FitTestEarStatus {
    pub const fn from_wire(code: u8) -> Self {
        match code {
            0 => Self::Untested,
            1 => Self::Good,
            2 => Self::Fair,
            _ => Self::Failed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FitTestResult {
    pub left: FitTestEarStatus,
    pub right: FitTestEarStatus,
    pub tested_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulatorControlState {
    pub enabled: bool,
    pub model_id: String,
    pub lid_open: bool,
    pub latency_ms: u64,
    pub battery: BatteryTelemetry,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolLogEntry {
    pub timestamp_ms: u64,
    /// `"tx"` or `"rx"`.
    pub direction: String,
    pub transport: String,
    pub uuid: String,
    pub hex: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PopupTriggerPayload {
    pub device_name: String,
    pub model_id: String,
    pub form_factor: FormFactor,
    pub color_index: u8,
    pub battery: BatteryTelemetry,
    pub anc_mode: AncPrimaryMode,
    pub is_virtual: bool,
    pub auto_dismiss_ms: u64,
}

/// Complete frontend state snapshot matching `src/types/qcy.ts` `AppSnapshot` verbatim.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppSnapshot {
    pub connection_state: ConnectionState,
    pub is_virtual_session: bool,
    pub adapter: AdapterInfo,
    pub active_device: Option<DiscoveredDeviceInfo>,
    pub active_model: DeviceModelProfile,
    pub firmware_version: String,
    pub device_color_index: u8,
    pub saved_devices: Vec<SavedDeviceRecord>,
    pub battery: BatteryTelemetry,
    pub anc: AncState,
    pub eq: EqState,
    pub gestures: GestureState,
    pub modes: ModesState,
    pub multipoint: MultipointState,
    pub rgb_lighting: RgbLightingState,
    pub focus_white_noise: FocusWhiteNoiseState,
    pub ota: OtaState,
    pub last_fit_test: Option<FitTestResult>,
    pub simulator: SimulatorControlState,
    pub settings: AppSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandOutcome {
    pub applied: bool,
    pub snapshot: AppSnapshot,
    pub conflict_prompt: Option<MutualExclusionPrompt>,
}

// ---------------------------------------------------------------------------
// IPC Command Payload Structs (Matches src/ipc/client.ts verbatim)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SetAncPayload {
    pub mode: AncPrimaryMode,
    pub scene: Option<AncScene>,
    pub scene_level: Option<u8>,
    pub custom_level: Option<u8>,
    pub transparency_level: Option<u8>,
    pub vocal_boost: Option<bool>,
    pub wind_noise: Option<bool>,
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SetAudioEnhancersPayload {
    pub dynamic_eq: Option<bool>,
    pub thx_eq: Option<bool>,
    pub virtual_bass: Option<bool>,
    pub adaptive_volume: Option<bool>,
    pub channel_balance: Option<f32>,
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SetHeadControlsPatch {
    pub enabled: Option<bool>,
    pub nod_cmd: Option<u8>,
    pub shake_cmd: Option<u8>,
    pub tilt_left_cmd: Option<u8>,
    pub tilt_right_cmd: Option<u8>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SetModesPayload {
    pub listening_mode: Option<ListeningModeUi>,
    pub game_mode: Option<bool>,
    pub game_mode_pro: Option<bool>,
    pub game_sync_eq: Option<bool>,
    pub spatial_audio: Option<SpatialAudioModeUi>,
    pub ldac: Option<bool>,
    pub le_audio: Option<bool>,
    pub in_ear_mode: Option<u8>,
    pub in_ear_auto_anc: Option<bool>,
    pub in_ear_sensitivity: Option<u8>,
    pub hearing_low_vol_remind: Option<bool>,
    pub hearing_duration_remind: Option<bool>,
    pub hearing_duration_min: Option<u16>,
    pub privacy_mode: Option<bool>,
    pub privacy_scope: Option<u8>,
    pub dialogue_perception: Option<bool>,
    pub speak_to_chat_sec: Option<u8>,
    pub auto_power_off_min: Option<u16>,
    pub disconnect_power_off_min: Option<u16>,
    pub power_saving: Option<bool>,
    pub disconnect_warning: Option<bool>,
    pub prompt_language: Option<PromptLanguageUi>,
    pub prompt_volume: Option<u8>,
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SetMultipointPayload {
    pub dual_device: Option<bool>,
    pub multi_device: Option<bool>,
    pub connect_mac: Option<String>,
    pub disconnect_mac: Option<String>,
    pub delete_mac: Option<String>,
    pub enter_pairing: Option<bool>,
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SetRgbLightingPayload {
    pub enabled: Option<bool>,
    pub mode: Option<SpeakerRgbMode>,
    pub color_hex: Option<String>,
    pub brightness: Option<u8>,
    pub speed: Option<u8>,
    pub tws_pair: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BatteryPatch {
    pub left: Option<u8>,
    pub right: Option<u8>,
    pub case_level: Option<u8>,
    pub left_charging: Option<bool>,
    pub right_charging: Option<bool>,
    pub case_charging: Option<bool>,
    pub left_in_case: Option<bool>,
    pub right_in_case: Option<bool>,
    pub is_tws_connected: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SimulatorPatch {
    pub lid_open: Option<bool>,
    pub latency_ms: Option<u64>,
    pub battery: Option<BatteryPatch>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResetDeviceKind {
    SoftReset,
    ClearPairing,
    FactoryReset,
}

// ---------------------------------------------------------------------------
// Backward-Compatible Legacy Snapshot Types (Kept for concurrent src-tauri build)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EqPreset {
    pub id: String,
    pub name: String,
    pub gains_db: Vec<f32>,
    pub built_in: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStatusSnapshot {
    pub connection_state: ConnectionState,
    pub active_device: Option<RawDiscoveredDevice>,
    pub active_model: DeviceModelProfile,
    pub anc_mode: AncMode,
    pub gaming_mode: bool,
    pub settings: AppSettings,
}
