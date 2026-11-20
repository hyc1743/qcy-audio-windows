//! Strongly-typed clean-room domain models for all QCY Audio device features,
//! commands, notifications, and wire operations.

use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::uuids::{CmdId, COMMAND_PACING_MS};

/// High-level daily-control wire protocol family selected for a connected QCY device.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolFamily {
    /// QCY Unified Custom TLV over BLE GATT service `0xA001` (`0x1001` / `0x1002`).
    QcyUnifiedTlv,
    /// Modern JieLi (`modelID & 0x8070 == 0x8070`) tunneling QCY TLVs inside RCSP `0xFF` on `0xA002`.
    #[serde(rename = "jieli_rcsp_new", alias = "jie_li_rcsp_new")]
    JieLiRcspNew,
    /// Legacy JieLi native RCSP commands on `0xA002` (`0x03`, `0x07`, `0x08`, `0xC0`–`0xC5`).
    #[serde(rename = "jieli_rcsp_legacy", alias = "jie_li_rcsp_legacy")]
    JieLiRcspLegacy,
    /// Older `0xA001` devices exposing direct GATT characteristics (`0x0007`/`0x0008`/`0x000B`/`0x000D`).
    LegacyDirectGatt,
    /// Placeholder protocol family retained for generic fallback profiles.
    GenericPlaceholder,
    /// Custom or experimental protocol identifier.
    Custom(String),
}

/// High-level ANC / Transparency operating mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AncMode {
    Off,
    NoiseCancellation,
    Transparency,
    Adaptive,
    WindNoiseReduction,
    VocalBoostTransparency,
    CustomDepth,
}

/// Exact 3-byte `0x17` (`CMDID_ANCSETTING`) wire tuple `[mode, subScene, level]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AncSetting {
    /// `1` = ANC, `2` = Off/Normal, `3` = Transparency, `5` = Custom Depth.
    pub mode: u8,
    /// Under ANC: `1..3` scenes, `4` = Anti-Wind, `5` = Adaptive; under Transparency: `1` = Standard, `2` = Vocal Boost.
    pub sub_scene: u8,
    /// Depth or gain level (`0..2` for ANC scenes, `0..6` for Transparency, `0..15` for Custom Depth).
    pub level: u8,
}

impl AncSetting {
    pub const fn new(mode: u8, sub_scene: u8, level: u8) -> Self {
        Self {
            mode,
            sub_scene,
            level,
        }
    }

    pub const fn off() -> Self {
        Self::new(2, 0, 0)
    }

    pub const fn anc_scene(sub_scene: u8, level: u8) -> Self {
        Self::new(1, sub_scene, level)
    }

    pub const fn wind_noise_reduction() -> Self {
        Self::new(1, 4, 0)
    }

    pub const fn adaptive() -> Self {
        Self::new(1, 5, 0)
    }

    pub const fn transparency(level: u8) -> Self {
        Self::new(3, 1, level)
    }

    pub const fn vocal_boost_transparency() -> Self {
        Self::new(3, 2, 0)
    }

    pub const fn custom_depth(level: u8) -> Self {
        let clamped = if level > 15 { 15 } else { level };
        Self::new(5, 0, clamped)
    }

    /// Maps a high-level `(AncMode, Option<u8>)` to a canonical `0x17` wire tuple.
    pub const fn from_mode(mode: AncMode, depth: Option<u8>) -> Self {
        match mode {
            AncMode::Off => Self::off(),
            AncMode::NoiseCancellation => {
                let d = match depth {
                    Some(v) => v,
                    None => 2,
                };
                if d > 3 {
                    Self::custom_depth(d)
                } else {
                    Self::anc_scene(if d == 0 { 1 } else { d }, 0)
                }
            }
            AncMode::Transparency => Self::transparency(match depth {
                Some(v) => v,
                None => 0,
            }),
            AncMode::Adaptive => Self::adaptive(),
            AncMode::WindNoiseReduction => Self::wind_noise_reduction(),
            AncMode::VocalBoostTransparency => Self::vocal_boost_transparency(),
            AncMode::CustomDepth => Self::custom_depth(match depth {
                Some(v) => v,
                None => 8,
            }),
        }
    }

    /// Converts this 3-byte tuple back into `(AncMode, Option<u8>)`.
    pub const fn to_mode_and_depth(self) -> (AncMode, Option<u8>) {
        match (self.mode, self.sub_scene) {
            (2, _) => (AncMode::Off, None),
            (1, 4) => (AncMode::WindNoiseReduction, None),
            (1, 5) => (AncMode::Adaptive, None),
            (1, scene) => (
                AncMode::NoiseCancellation,
                Some(if self.level > 0 { self.level } else { scene }),
            ),
            (3, 2) => (AncMode::VocalBoostTransparency, None),
            (3, _) => (AncMode::Transparency, Some(self.level)),
            (5, _) => (AncMode::CustomDepth, Some(self.level)),
            _ => (AncMode::Off, None),
        }
    }
}

/// 3-byte firmware version (`0x30` or legacy GATT `0x0007`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct FirmwareVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
}

impl fmt::Display for FirmwareVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

// ---------------------------------------------------------------------------
// AI-ANC (0x54, 0x55, 0x56, 0x5A)
// ---------------------------------------------------------------------------

/// Activity state reported by `0x54` (`AIANCSettingDataBean`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiAncActivity {
    Detecting,
    Stay,
    Walk,
    Run,
    Off,
    Unknown(u8),
}

impl AiAncActivity {
    pub const fn from_u8(raw: u8) -> Self {
        match raw {
            0 => Self::Detecting,
            1 => Self::Stay,
            2 => Self::Walk,
            3 => Self::Run,
            4 => Self::Off,
            other => Self::Unknown(other),
        }
    }

    pub const fn as_u8(self) -> u8 {
        match self {
            Self::Detecting => 0,
            Self::Stay => 1,
            Self::Walk => 2,
            Self::Run => 3,
            Self::Off => 4,
            Self::Unknown(v) => v,
        }
    }
}

/// Real-time AI-ANC state (`0x54`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiAncState {
    pub activity: AiAncActivity,
    pub noise_scene: u8,
    pub setting: AncSetting,
    pub touch_index: Option<u8>,
}

/// Single cell inside the 3×3 AI-ANC activity × ambient-noise matrix (`0x55` / `0x56`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiAncMatrixEntry {
    pub activity: AiAncActivity,
    pub noise_scene: u8,
    pub setting: AncSetting,
}

/// Control action for `0x5A` (`CMDID_AI_ANC_PREVIEW`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AiAncPreviewCommand {
    Enter,
    Preview(AncSetting),
    Exit,
}

// ---------------------------------------------------------------------------
// Equalizer Types (All 4 Wire Dialects + Exact 9 Biquad Filter Types)
// ---------------------------------------------------------------------------

/// Standard 10-band graphic EQ center frequencies (Hz).
pub const DEFAULT_EQ_FREQS_10: [u16; 10] = [31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];
/// Legacy ZR chipset (`0x0090`) 5-band graphic EQ center frequencies (Hz).
pub const DEFAULT_EQ_FREQS_5_ZR: [u16; 5] = [31, 125, 500, 2000, 8000];

/// Exact QCY `EQParamBean.irrType` biquad filter wire IDs (`0x22`, `0x46`, `0x47`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum IrrFilterType {
    /// `0` — High-Pass (HP).
    HighPass = 0,
    /// `1` — Low-Pass (LP).
    LowPass = 1,
    /// `2` — Peaking / Bell (PEAK, default parametric band).
    Peak = 2,
    /// `3` — High-Shelf (HS).
    HighShelf = 3,
    /// `4` — Low-Shelf (LS).
    LowShelf = 4,
    /// `5` — High-Shelf with Q (HSQ).
    HighShelfQ = 5,
    /// `6` — Low-Shelf with Q (LSQ).
    LowShelfQ = 6,
    /// `7` — Advanced High-Pass (ADV_HP).
    AdvHighPass = 7,
    /// `8` — Advanced Low-Pass (ADV_LP).
    AdvLowPass = 8,
    /// `0xFF` — Unused padding slot.
    Pad = 0xFF,
}

impl IrrFilterType {
    pub const fn from_u8(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(Self::HighPass),
            1 => Some(Self::LowPass),
            2 => Some(Self::Peak),
            3 => Some(Self::HighShelf),
            4 => Some(Self::LowShelf),
            5 => Some(Self::HighShelfQ),
            6 => Some(Self::LowShelfQ),
            7 => Some(Self::AdvHighPass),
            8 => Some(Self::AdvLowPass),
            0xFF => Some(Self::Pad),
            _ => None,
        }
    }

    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Which wire dialect to use when encoding EQ updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EqDialect {
    /// `0xFC` / legacy GATT `0x000B` (10-band signed int8 dB gains).
    LegacyGraphic10,
    /// `0xFC` / legacy GATT `0x000B` (ZR chipset 5-band signed int8 dB gains).
    LegacyGraphic5Zr,
    /// `0x20` (`CMDID_MULTIEQ`) — 6 bytes per band (no `irrType`).
    MultiEqV1,
    /// `0x22` (`CMDID_MULTIEQ2`) — 7 bytes per band (with `IrrFilterType`).
    MultiEqV2,
}

/// Legacy integer-gain graphic EQ (`0xFC` / GATT `0x000B` / Legacy JieLi RCSP).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphicEqSetting {
    pub eq_type: u8,
    pub gains_db: Vec<i8>,
    pub game_eq_type: Option<u8>,
    pub current_mode: Option<u8>,
}

/// Single parametric biquad EQ band (`0x20`, `0x22`, `0x46`, `0x47`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ParametricEqBand {
    pub freq_hz: u16,
    /// Gain in dB (encoded on wire as `round(gain_db * 100.0)` `i16_LE`).
    pub gain_db: f32,
    /// Filter Q factor (encoded on wire as `round(q * 100.0)` `i16_LE`).
    pub q: f32,
    /// Filter type (`IrrFilterType::Peak` default for `0x22`/`0x46`/`0x47`; omitted on `0x20`).
    pub filter_type: IrrFilterType,
}

impl ParametricEqBand {
    pub const fn peak(freq_hz: u16, gain_db: f32, q: f32) -> Self {
        Self {
            freq_hz,
            gain_db,
            q,
            filter_type: IrrFilterType::Peak,
        }
    }

    pub const fn pad() -> Self {
        Self {
            freq_hz: 0,
            gain_db: 0.0,
            q: 0.0,
            filter_type: IrrFilterType::Pad,
        }
    }

    /// Detects the firmware RX sentinel band `(freq=100, gain=0.0, Q=0.01)` that
    /// QCY firmware emits for unused slots and the Android app ignores.
    pub fn is_firmware_dummy_slot(&self) -> bool {
        self.filter_type == IrrFilterType::Pad
            || (self.freq_hz == 100 && self.gain_db.abs() < 0.005 && (self.q - 0.01).abs() < 0.005)
    }
}

/// Complete parametric EQ preset payload (`0x20`, `0x22`, `0x46`, `0x47`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParametricEqPreset {
    pub eq_type: u8,
    /// Master preamp gain in dB (`i16_LE` × 100 on wire).
    pub master_gain_db: f32,
    pub bands: Vec<ParametricEqBand>,
}

/// Target channel for independent L/R parametric EQ (`0x46` / `0x47`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EarSide {
    Left,
    Right,
}

// ---------------------------------------------------------------------------
// Touch / Button Remapping (0x2B, GATT 0x000D, JL RCSP) & IMU Head Controls
// ---------------------------------------------------------------------------

/// Verified button / touch action IDs (`cmd 0..18`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ButtonAction {
    None,
    PlayPause,
    PreviousTrack,
    NextTrack,
    VoiceAssistant,
    VolumeUp,
    VolumeDown,
    GameMode,
    AncCycle,
    ModeSwitch,
    SpatialSound,
    MovieMode,
    MusicMode,
    GameModeAlt,
    Raw(u8),
}

impl ButtonAction {
    pub const fn from_u8(raw: u8) -> Self {
        match raw {
            0 => Self::None,
            1 => Self::PlayPause,
            2 => Self::PreviousTrack,
            3 => Self::NextTrack,
            4 => Self::VoiceAssistant,
            5 => Self::VolumeUp,
            6 => Self::VolumeDown,
            7 => Self::GameMode,
            11 => Self::AncCycle,
            14 => Self::ModeSwitch,
            15 => Self::SpatialSound,
            16 => Self::MovieMode,
            17 => Self::MusicMode,
            18 => Self::GameModeAlt,
            other => Self::Raw(other),
        }
    }

    pub const fn as_u8(self) -> u8 {
        match self {
            Self::None => 0,
            Self::PlayPause => 1,
            Self::PreviousTrack => 2,
            Self::NextTrack => 3,
            Self::VoiceAssistant => 4,
            Self::VolumeUp => 5,
            Self::VolumeDown => 6,
            Self::GameMode => 7,
            Self::AncCycle => 11,
            Self::ModeSwitch => 14,
            Self::SpatialSound => 15,
            Self::MovieMode => 16,
            Self::MusicMode => 17,
            Self::GameModeAlt => 18,
            Self::Raw(v) => v,
        }
    }
}

/// Single gesture-to-action binding (`0x2B`, GATT `0x000D`, or JL RCSP).
///
/// `event_id` is `u16` to support both 8-bit TLV events (`1..10`, `17..22`, `35..37`)
/// and 16-bit Legacy JieLi over-ear events (`0x0301`, `0x0501`, `0x0701..0x0704`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ButtonBinding {
    pub event_id: u16,
    pub action: ButtonAction,
}

/// Touch sensor sensitivity threshold (`0x33`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TouchSensitivity {
    Moderate,
    Normal,
    Sensitive,
    Raw(u8),
}

impl TouchSensitivity {
    pub const fn from_u8(raw: u8) -> Self {
        match raw {
            0x26 => Self::Moderate,
            0x2D => Self::Normal,
            0x34 => Self::Sensitive,
            other => Self::Raw(other),
        }
    }

    pub const fn as_u8(self) -> u8 {
        match self {
            Self::Moderate => 0x26,
            Self::Normal => 0x2D,
            Self::Sensitive => 0x34,
            Self::Raw(v) => v,
        }
    }
}

/// IMU head gesture event (`0x51` / `0x52`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeadGestureEvent {
    Nod,
    Shake,
    TiltLeft,
    TiltRight,
    Raw(u8),
}

impl HeadGestureEvent {
    pub const fn from_u8(raw: u8) -> Self {
        match raw {
            1 => Self::Nod,
            2 => Self::Shake,
            3 => Self::TiltLeft,
            4 => Self::TiltRight,
            other => Self::Raw(other),
        }
    }

    pub const fn as_u8(self) -> u8 {
        match self {
            Self::Nod => 1,
            Self::Shake => 2,
            Self::TiltLeft => 3,
            Self::TiltRight => 4,
            Self::Raw(v) => v,
        }
    }
}

/// IMU head gesture action (`0x51`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeadGestureAction {
    None,
    AnswerCall,
    RejectCall,
    PreviousTrack,
    NextTrack,
    Raw(u8),
}

impl HeadGestureAction {
    pub const fn from_u8(raw: u8) -> Self {
        match raw {
            0 => Self::None,
            1 => Self::AnswerCall,
            2 => Self::RejectCall,
            3 => Self::PreviousTrack,
            4 => Self::NextTrack,
            other => Self::Raw(other),
        }
    }

    pub const fn as_u8(self) -> u8 {
        match self {
            Self::None => 0,
            Self::AnswerCall => 1,
            Self::RejectCall => 2,
            Self::PreviousTrack => 3,
            Self::NextTrack => 4,
            Self::Raw(v) => v,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HeadGestureBinding {
    pub event: HeadGestureEvent,
    pub action: HeadGestureAction,
}

// ---------------------------------------------------------------------------
// Modes, Wear Detection, Multipoint, Power, Audio & Speaker Subsystems
// ---------------------------------------------------------------------------

/// Listening / scene mode (`0x2E`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ListeningMode {
    Music,
    Game,
    Movie,
    Raw(u8),
}

impl ListeningMode {
    pub const fn from_u8(raw: u8) -> Self {
        match raw {
            1 => Self::Music,
            3 => Self::Game,
            4 => Self::Movie,
            other => Self::Raw(other),
        }
    }

    pub const fn as_u8(self) -> u8 {
        match self {
            Self::Music => 1,
            Self::Game => 3,
            Self::Movie => 4,
            Self::Raw(v) => v,
        }
    }
}

/// 3-mode Spatial Audio selector (`0x5B`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpatialAudioMode {
    Off,
    Fixed,
    HeadTracking,
    Raw(u8),
}

impl SpatialAudioMode {
    pub const fn from_u8(raw: u8) -> Self {
        match raw {
            1 => Self::Off,
            2 => Self::Fixed,
            3 => Self::HeadTracking,
            other => Self::Raw(other),
        }
    }

    pub const fn as_u8(self) -> u8 {
        match self {
            Self::Off => 1,
            Self::Fixed => 2,
            Self::HeadTracking => 3,
            Self::Raw(v) => v,
        }
    }
}

/// Game Mode Pro (`0x4A`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameModeProConfig {
    pub enabled: bool,
    pub follow_game_sound: bool,
}

/// Privacy Mode Pro (`0x5F`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivacyModeProConfig {
    pub enabled: bool,
    /// `false` = Call + Music (`0`), `true` = Call Only (`1`).
    pub call_only: bool,
}

/// In-ear auto-play/pause music behavior (`0x2C` byte 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InEarMusicBehavior {
    Off,
    SinglePauseAutoPlay,
    SinglePauseNoAutoPlay,
    DualPauseAutoPlay,
    Raw(u8),
}

impl InEarMusicBehavior {
    pub const fn from_u8(raw: u8) -> Self {
        match raw {
            0 => Self::Off,
            1 => Self::SinglePauseAutoPlay,
            2 => Self::SinglePauseNoAutoPlay,
            3 => Self::DualPauseAutoPlay,
            other => Self::Raw(other),
        }
    }

    pub const fn as_u8(self) -> u8 {
        match self {
            Self::Off => 0,
            Self::SinglePauseAutoPlay => 1,
            Self::SinglePauseNoAutoPlay => 2,
            Self::DualPauseAutoPlay => 3,
            Self::Raw(v) => v,
        }
    }
}

/// Advanced In-Ear Wear Detection config (`0x2C`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WearConfig {
    pub enabled: bool,
    pub music_behavior: InEarMusicBehavior,
    pub control_anc: bool,
}

/// In-Ear sensor sensitivity (`0x48`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WearSensitivity {
    Moderate,
    Normal,
    Sensitive,
    Raw(u8),
}

impl WearSensitivity {
    pub const fn from_u8(raw: u8) -> Self {
        match raw {
            1 => Self::Moderate,
            2 => Self::Normal,
            3 => Self::Sensitive,
            other => Self::Raw(other),
        }
    }

    pub const fn as_u8(self) -> u8 {
        match self {
            Self::Moderate => 1,
            Self::Normal => 2,
            Self::Sensitive => 3,
            Self::Raw(v) => v,
        }
    }
}

/// Scheduled auto power-off state (`0x14`). `0xFFFF` means disabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerManagerState {
    pub scheduled_minutes: u16,
    pub remaining_minutes: u16,
}

impl PowerManagerState {
    pub const DISABLED: u16 = 0xFFFF;
}

/// Prompt tone volume (`0x1D`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptToneVolume {
    pub current: u8,
    pub max: Option<u8>,
}

/// Single paired or connected host entry inside `0x4D` (`PhoneBeanList`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairedHostEntry {
    pub status: u8,
    /// `true` when `status == 2 || status == 3`.
    pub connected: bool,
    pub mac: [u8; 6],
    pub name: String,
}

/// Decoded multipoint host list (`0x4D`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairedHostList {
    pub total_count: u8,
    pub connected: Vec<PairedHostEntry>,
    pub disconnected: Vec<PairedHostEntry>,
}

/// Host management action sent on `0x4F` (`CMDID_PHONE_STATUS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum MultipointHostAction {
    Connect = 1,
    Delete = 2,
    Disconnect = 3,
}

/// Notification received on `0x4F` (`PhoneStatusBean`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultipointHostStatus {
    pub mac: [u8; 6],
    pub status: u8,
}

impl MultipointHostStatus {
    pub const UPLOAD_START: u8 = 10;
    pub const UPLOAD_END: u8 = 11;

    pub const fn is_upload_start(&self) -> bool {
        self.status == Self::UPLOAD_START
    }

    pub const fn is_upload_end(&self) -> bool {
        self.status == Self::UPLOAD_END
    }
}

/// Hearing Health configuration (`0x58`, `HearingDataBean`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HearingHealthConfig {
    pub alarm_volume: bool,
    pub alarm_time: bool,
    pub limit_minutes: u16,
}

/// Ear-tip Fit / Seal Test result (`0x11`, `CompactnessDataBean`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FitTestResult {
    /// `1` = testing in progress, `2` = test complete.
    pub status: u8,
    pub left_result: u8,
    pub right_result: u8,
}

/// RGB color triplet (`0x36`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Speaker RGB LED lighting effect index (`0x36`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LedEffectKind {
    Solid,
    Breathing,
    Gradient,
    MusicRhythm,
    Raw(u8),
}

impl LedEffectKind {
    pub const fn from_u8(raw: u8) -> Self {
        match raw {
            1 => Self::Solid,
            2 => Self::Breathing,
            3 => Self::Gradient,
            4 => Self::MusicRhythm,
            other => Self::Raw(other),
        }
    }

    pub const fn as_u8(self) -> u8 {
        match self {
            Self::Solid => 1,
            Self::Breathing => 2,
            Self::Gradient => 3,
            Self::MusicRhythm => 4,
            Self::Raw(v) => v,
        }
    }
}

/// Speaker RGB lighting configuration (`0x36`, `LEDEffectDataBean`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedEffectConfig {
    pub speed: u8,
    pub brightness: u8,
    pub effect: LedEffectKind,
    pub colors: Vec<RgbColor>,
}

/// Hardware white-noise / focus playback status (`0x3A`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WhiteNoisePlayState {
    pub music_index: u32,
    pub status: u8,
    pub play_mode: u8,
}

/// Single stored white-noise track metadata (`0x3B`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WhiteNoiseTrackInfo {
    pub music_id: u32,
    pub total: u16,
}

/// Onboard white-noise track table (`0x3B`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WhiteNoiseTrackList {
    pub start_tone_id: u8,
    pub tracks: Vec<WhiteNoiseTrackInfo>,
}

/// Wall-clock timestamp synchronized via `0x3E` (`CMDID_SYNC_TIME`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceDateTime {
    /// Year offset from 2000 (`year % 100`, e.g., `26` for 2026).
    pub year_mod_100: u8,
    /// `1..=12`.
    pub month: u8,
    /// `1..=31`.
    pub day: u8,
    /// `0..=23`.
    pub hour: u8,
    /// `0..=59`.
    pub minute: u8,
    /// `0..=59`.
    pub second: u8,
    /// Bitmask `1 << (day_of_week - 1)` (`Sun = 0x01` .. `Sat = 0x40`).
    pub weekday_mask: u8,
}

/// Hardware alarm clock entry (`0x3F`, `AlarmDataBean`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceAlarm {
    pub alarm_id: u8,
    pub enabled: bool,
    pub hour: u8,
    pub minute: u8,
    pub cycle_mask: u8,
}

/// Hardware alarm CRUD operation (`0x3F` TX).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum AlarmCommand {
    Add(DeviceAlarm),
    Delete { alarm_id: u8 },
    Edit { alarm: DeviceAlarm, flag: u8 },
}

// ---------------------------------------------------------------------------
// Wire Output Descriptor
// ---------------------------------------------------------------------------

/// A single concrete Bluetooth GATT write or read operation emitted by a `ProtocolDriver`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireWrite {
    pub service: Uuid,
    pub characteristic: Uuid,
    pub payload: Vec<u8>,
    pub with_response: bool,
    /// Minimum delay to wait after this write (`100 ms` default per `CommandProcessor`).
    pub pacing_ms: u64,
    /// `true` when this operation represents a direct GATT characteristic read (`0x0007`/`0x0008`/`0x000B`/`0x000D`).
    pub is_read: bool,
}

impl WireWrite {
    pub fn write(service: Uuid, characteristic: Uuid, payload: Vec<u8>) -> Self {
        Self {
            service,
            characteristic,
            payload,
            with_response: true,
            pacing_ms: COMMAND_PACING_MS,
            is_read: false,
        }
    }

    pub fn read(service: Uuid, characteristic: Uuid) -> Self {
        Self {
            service,
            characteristic,
            payload: Vec::new(),
            with_response: true,
            pacing_ms: COMMAND_PACING_MS,
            is_read: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Unified DeviceCommand & DeviceEvent Enums (Backward-Compatible + Complete)
// ---------------------------------------------------------------------------

/// Complete host-to-device command enum.
///
/// Retains all existing skeleton variants (`RequestDeviceInfo`, `RequestBattery`,
/// `SetAncMode`, `SetGamingMode`, `SetEqBands`, `FindEarbuds`) for zero-breakage
/// compatibility across the workspace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DeviceCommand {
    // --- Preserved core variants ---
    RequestDeviceInfo,
    RequestBattery,
    SetAncMode {
        mode: AncMode,
        depth: Option<u8>,
    },
    SetGamingMode {
        enabled: bool,
    },
    SetEqBands {
        gains_db: Vec<f32>,
    },
    /// Note: `0x4B` is UI-only and never transmitted over BLE; drivers emit zero wire writes.
    FindEarbuds {
        left: bool,
        right: bool,
    },

    // --- Generic attribute read & raw TLV ---
    RequestAttribute {
        cmd: CmdId,
    },
    RawTlv {
        cmd: u8,
        value: Vec<u8>,
    },

    // --- Device lifecycle & identity ---
    ResetDefault,
    ClearPairing,
    FactoryReset,
    SetDeviceName {
        name: String,
    },
    SetDeviceColor {
        color_index: u8,
    },

    // --- Full ANC / AI-ANC ---
    SetAncSetting(AncSetting),
    SetLegacyNoiseMode {
        mode: u8,
        percent: Option<u8>,
    },
    SetWindNoise {
        enabled: bool,
    },
    SetButtonAncCycle {
        modes: Vec<AncSetting>,
    },
    ResetButtonAncCycle,
    SetAiAncEnabled {
        enabled: bool,
    },
    SetAiAncMatrixCell(AiAncMatrixEntry),
    AiAncPreview(AiAncPreviewCommand),

    // --- All 4 EQ dialects & audio enhancements ---
    SetGraphicEq(GraphicEqSetting),
    SetParametricEqV1(ParametricEqPreset),
    SetParametricEqV2(ParametricEqPreset),
    SetSideParametricEq {
        side: EarSide,
        preset: ParametricEqPreset,
        max_bands: usize,
    },
    SetCustomEqTestMode {
        enabled: bool,
    },
    SetDynamicEq {
        enabled: bool,
    },
    SetBassEnhancement {
        enabled: bool,
    },
    SetThxEq {
        enabled: bool,
    },
    SetAdaptiveVolume {
        enabled: bool,
    },
    SetChannelBalance {
        /// `0..=100`, `50` = center.
        balance: u8,
    },

    // --- Touch, buttons & IMU head gestures ---
    SetButtonBinding(ButtonBinding),
    ResetButtonBindings,
    SetSleepMode {
        enabled: bool,
    },
    SetTouchTone {
        enabled: bool,
    },
    SetTouchSensitivity {
        sensitivity: TouchSensitivity,
    },
    SetHeadGestureEnabled {
        enabled: bool,
    },
    SetHeadGestureBinding(HeadGestureBinding),
    SetHeadGestureTestMode {
        enabled: bool,
    },

    // --- Modes, codecs & toggles ---
    SetGameModePro(GameModeProConfig),
    SetListeningMode {
        mode: ListeningMode,
    },
    SetSpatialSound {
        enabled: bool,
    },
    SetSpatialAudioMode {
        mode: SpatialAudioMode,
    },
    SetLdac {
        enabled: bool,
    },
    SetAptx {
        enabled: bool,
    },
    SetLeAudio {
        enabled: bool,
    },
    SetPrivacyModePro(PrivacyModeProConfig),
    SetDialoguePerception {
        enabled: bool,
    },
    SetVoiceDetectionDelay {
        /// `0` = Off, or `5`, `10`, `15`, `20` seconds.
        seconds: u8,
    },
    SetEnvironmentAdaptation {
        enabled: bool,
    },
    SetPowerSavingMode {
        enabled: bool,
    },
    SetDisconnectWarning {
        enabled: bool,
    },

    // --- Multipoint / Dual-device ---
    SetDualConnect {
        enabled: bool,
    },
    SetMultipointPairing {
        enabled: bool,
    },
    ControlMultipointHost {
        action: MultipointHostAction,
        mac: [u8; 6],
    },

    // --- In-ear detection, fit test & hearing health ---
    SetInEarCheck {
        enabled: bool,
    },
    SetWearConfig(WearConfig),
    SetWearSensitivity {
        sensitivity: WearSensitivity,
    },
    StartFitTest,
    SetHearingHealth(HearingHealthConfig),

    // --- Power management & prompts ---
    SetAutoPowerOffMinutes {
        /// Minutes (`0xFFFF` = Disabled).
        minutes: u16,
    },
    SetStandbyMinutes {
        /// Minutes (`0xFFFF` = Never).
        minutes: u16,
    },
    SetPromptVoiceCode {
        /// 4-byte ASCII code, e.g. `"WQ00"`, `"JL01"`, `"LX00"`.
        code: String,
    },
    SetPromptLanguageIndex {
        index: u8,
    },
    SetPromptToneVolume {
        level: u8,
    },

    // --- Speaker RGB, TWS, White Noise, Clock & Alarms ---
    SetTwsEnable {
        enabled: bool,
    },
    SetLedSwitch {
        enabled: bool,
    },
    SetLedEffect(LedEffectConfig),
    SetFocusMode {
        enabled: bool,
    },
    ToggleWhiteNoisePlayback,
    SetWhiteNoiseVolume {
        volume: u8,
    },
    SetWhiteNoisePreview {
        enabled: bool,
    },
    SyncTime(DeviceDateTime),
    ManageAlarm(AlarmCommand),
}

/// Complete device-to-host event enum.
///
/// Retains all existing skeleton variants (`BatteryUpdated`, `AncModeChanged`,
/// `GamingModeChanged`, `RawNotification`) intact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DeviceEvent {
    // --- Preserved core variants ---
    BatteryUpdated {
        left: Option<u8>,
        right: Option<u8>,
        case: Option<u8>,
        left_charging: bool,
        right_charging: bool,
        case_charging: bool,
    },
    AncModeChanged {
        mode: AncMode,
        depth: Option<u8>,
    },
    GamingModeChanged {
        enabled: bool,
    },
    RawNotification {
        payload: Vec<u8>,
    },

    // --- Device info & identity ---
    FirmwareVersion(FirmwareVersion),
    DeviceNameChanged {
        name: String,
    },
    DeviceColorChanged {
        color_index: u8,
    },

    // --- Full ANC / AI-ANC ---
    AncSettingUpdated(AncSetting),
    LegacyNoiseModeUpdated {
        mode: u8,
    },
    WindNoiseChanged {
        enabled: bool,
    },
    AdaptiveAncCalibration {
        /// `2` = calibrating, `1` = complete.
        status: u8,
    },
    AncWearStatus {
        mask: u16,
    },
    WearStatusUpdated {
        mask: u16,
    },
    ButtonAncCycleUpdated {
        modes: Vec<AncSetting>,
    },
    AiAncStateUpdated(AiAncState),
    AiAncDefaultMatrixUpdated {
        entries: Vec<AiAncMatrixEntry>,
    },
    AiAncUserMatrixUpdated {
        entries: Vec<AiAncMatrixEntry>,
    },

    // --- All EQ & audio events ---
    GraphicEqUpdated(GraphicEqSetting),
    ParametricEqV1Updated(ParametricEqPreset),
    ParametricEqV2Updated(ParametricEqPreset),
    EqBandCountUpdated {
        count: u8,
    },
    DynamicEqChanged {
        enabled: bool,
    },
    BassEnhancementChanged {
        enabled: bool,
    },
    ThxEqChanged {
        enabled: bool,
    },
    AdaptiveVolumeChanged {
        enabled: bool,
    },
    ChannelBalanceUpdated {
        balance: u8,
    },

    // --- Touch, buttons & IMU ---
    ButtonBindingsUpdated {
        bindings: Vec<ButtonBinding>,
    },
    SleepModeChanged {
        enabled: bool,
    },
    TouchToneChanged {
        enabled: bool,
    },
    TouchSensitivityUpdated {
        sensitivity: TouchSensitivity,
    },
    HeadGestureEnabledChanged {
        enabled: bool,
    },
    HeadGestureBindingsUpdated {
        bindings: Vec<HeadGestureBinding>,
    },
    HeadGestureDetected {
        event: HeadGestureEvent,
    },
    CameraShutterTriggered {
        value: u8,
    },

    // --- Modes, codecs & toggles ---
    GameModeProUpdated(GameModeProConfig),
    ListeningModeUpdated {
        mode: ListeningMode,
    },
    SpatialSoundChanged {
        enabled: bool,
    },
    SpatialAudioModeUpdated {
        mode: SpatialAudioMode,
    },
    LdacChanged {
        enabled: bool,
    },
    AptxChanged {
        enabled: bool,
    },
    LeAudioChanged {
        enabled: bool,
    },
    PrivacyModeProUpdated(PrivacyModeProConfig),
    DialoguePerceptionChanged {
        enabled: bool,
    },
    VoiceDetectionDelayUpdated {
        seconds: u8,
    },
    EnvironmentAdaptationChanged {
        enabled: bool,
    },
    PowerSavingModeChanged {
        enabled: bool,
    },
    DisconnectWarningChanged {
        enabled: bool,
    },

    // --- Multipoint ---
    DualConnectChanged {
        enabled: bool,
    },
    PairedHostsUpdated(PairedHostList),
    MultipointHostStatusUpdated(MultipointHostStatus),

    // --- In-ear, fit test & hearing health ---
    InEarCheckChanged {
        enabled: bool,
    },
    WearConfigUpdated(WearConfig),
    WearSensitivityUpdated {
        sensitivity: WearSensitivity,
    },
    FitTestUpdated(FitTestResult),
    HearingHealthUpdated(HearingHealthConfig),

    // --- Power & prompts ---
    PowerManagerUpdated(PowerManagerState),
    StandbyMinutesUpdated {
        minutes: u16,
    },
    PromptVoiceCodeUpdated {
        code: String,
    },
    PromptLanguageIndexUpdated {
        index: u8,
    },
    PromptToneVolumeUpdated(PromptToneVolume),

    // --- Speaker RGB, TWS, White Noise & Alarms ---
    TwsEnableChanged {
        enabled: bool,
    },
    LedSwitchChanged {
        enabled: bool,
    },
    LedEffectUpdated(LedEffectConfig),
    FocusModeChanged {
        enabled: bool,
    },
    WhiteNoiseVolumeUpdated {
        volume: u8,
        flag: u8,
    },
    WhiteNoisePlayStateUpdated(WhiteNoisePlayState),
    WhiteNoiseTrackListUpdated(WhiteNoiseTrackList),
    AlarmsUpdated {
        header: u8,
        alarms: Vec<DeviceAlarm>,
    },

    // --- Unrecognized / Raw TLV item ---
    RawTlv {
        cmd: u8,
        value: Vec<u8>,
    },
}
