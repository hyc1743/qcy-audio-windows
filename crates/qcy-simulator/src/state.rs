//! Full virtual hardware state for the raw-byte QCY simulator, supporting
//! live profile switching across all built-in virtual QCY models.

use qcy_models::{DeviceModelProfile, ModelRegistry};
use qcy_protocol::{
    AiAncActivity, AiAncMatrixEntry, AiAncState, AncMode, AncSetting, BatteryLevel, ButtonAction,
    ButtonBinding, DeviceAlarm, DeviceDateTime, FirmwareVersion, FitTestResult, GameModeProConfig,
    GraphicEqSetting, HeadGestureAction, HeadGestureBinding, HeadGestureEvent, HearingHealthConfig,
    InEarMusicBehavior, LedEffectConfig, LedEffectKind, ListeningMode, PairedHostEntry,
    PairedHostList, ParametricEqBand, ParametricEqPreset, PowerManagerState, PrivacyModeProConfig,
    PromptToneVolume, ProtocolFamily, QcyAdvertisement, RgbColor, SpatialAudioMode,
    TouchSensitivity, WearConfig, WearSensitivity, WhiteNoiseTrackInfo, WhiteNoiseTrackList,
    COMPANY_ID_QCY, DEFAULT_EQ_FREQS_10,
};
use serde::{Deserialize, Serialize};

/// Canonical 6 built-in virtual QCY models spanning every form factor and protocol family:
/// - `qcy-n70`: Flagship ANC TWS (`0x5D40`, WuQi, `QcyUnifiedTlv`, `otaType=11`)
/// - `qcy-ht08`: MeloBuds Pro ANC TWS (`0x4D4A`, WuQi, `QcyUnifiedTlv`, `otaType=11`)
/// - `qcy-ht08-jl`: MeloBuds Pro JieLi (`0x9072`, JieLi `isNewJL`, `JieLiRcspNew`, `otaType=2`)
/// - `qcy-c50i`: Crossky C50i Open-Ear (`0x807A`, JieLi `isNewJL`, `JieLiRcspNew`, `otaType=2`)
/// - `qcy-h3`: Over-Ear ANC Headphones (`0x417C`, Legacy JieLi, `JieLiRcspLegacy`, `otaType=2`)
/// - `qcy-sp75`: SP75 RGB Speaker (`0x0308`, WuQi, `QcyUnifiedTlv`, `otaType=11`)
pub const BUILTIN_VIRTUAL_MODELS: [&str; 6] = [
    "qcy-n70",
    "qcy-ht08",
    "qcy-ht08-jl",
    "qcy-c50i",
    "qcy-h3",
    "qcy-sp75",
];

/// Complete virtual hardware state backing the raw-byte Bluetooth simulator.
///
/// Preserves all existing top-level fields (`model_id`, `device_name`,
/// `left_battery`, `right_battery`, `case_battery`, `left_charging`,
/// `right_charging`, `case_charging`, `anc_mode`, `gaming_mode`, `lid_open`)
/// for compatibility with `qcy-core`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualDeviceState {
    // --- Core identity & battery (backward-compatible fields) ---
    pub model_id: String,
    pub device_name: String,
    pub left_battery: u8,
    pub right_battery: u8,
    pub case_battery: u8,
    pub left_charging: bool,
    pub right_charging: bool,
    pub case_charging: bool,
    pub anc_mode: AncMode,
    pub gaming_mode: bool,
    pub lid_open: bool,

    // --- Hardware & BLE advertising metadata ---
    pub numeric_model_id: u16,
    pub protocol_family: ProtocolFamily,
    pub ota_type: u8,
    pub firmware_version: FirmwareVersion,
    pub color_index: u8,
    pub pop_num: u8,
    pub class_mac: [u8; 6],
    pub other_mac: [u8; 6],
    pub latency_ms: u64,
    pub is_tws: bool,
    pub connected: bool,

    // --- ANC & AI-ANC ---
    pub anc_setting: AncSetting,
    pub legacy_noise_mode: u8,
    pub wind_noise: bool,
    pub btn_anc_cycle: Vec<AncSetting>,
    pub ai_anc_state: AiAncState,
    pub ai_anc_default_matrix: Vec<AiAncMatrixEntry>,
    pub ai_anc_user_matrix: Vec<AiAncMatrixEntry>,

    // --- Equalizer & Audio Enhancers ---
    pub graphic_eq: GraphicEqSetting,
    pub parametric_eq_v1: ParametricEqPreset,
    pub parametric_eq_v2: ParametricEqPreset,
    pub left_eq: ParametricEqPreset,
    pub right_eq: ParametricEqPreset,
    pub eq_band_count: u8,
    pub custom_eq_test_mode: bool,
    pub dynamic_eq: bool,
    pub bass_enhancement: bool,
    pub thx_eq: bool,
    pub adaptive_volume: bool,
    pub channel_balance: u8,

    // --- Buttons, Touch & IMU Head Controls ---
    pub button_bindings: Vec<ButtonBinding>,
    pub sleep_mode: bool,
    pub touch_tone: bool,
    pub touch_sensitivity: TouchSensitivity,
    pub head_gesture_enabled: bool,
    pub head_gesture_bindings: Vec<HeadGestureBinding>,
    pub head_gesture_test_active: bool,

    // --- Modes, Codecs & Multipoint ---
    pub game_mode_pro: GameModeProConfig,
    pub listening_mode: ListeningMode,
    pub spatial_sound: bool,
    pub spatial_audio_mode: SpatialAudioMode,
    pub ldac: bool,
    pub aptx: bool,
    pub le_audio: bool,
    pub privacy_mode_pro: PrivacyModeProConfig,
    pub dialogue_perception: bool,
    pub voice_detection_delay: u8,
    pub env_adaptation: bool,
    pub power_saving: bool,
    pub disconnect_warning: bool,
    pub dual_connect: bool,
    pub multipoint_pairing: bool,
    pub paired_hosts: PairedHostList,

    // --- Wear, Fit Test, Hearing Health, Power & Prompts ---
    pub in_ear_check: bool,
    pub wear_config: WearConfig,
    pub wear_sensitivity: WearSensitivity,
    pub in_ear_left: bool,
    pub in_ear_right: bool,
    pub fit_test_result: FitTestResult,
    pub hearing_health: HearingHealthConfig,
    pub power_manager: PowerManagerState,
    pub standby_minutes: u16,
    pub prompt_voice_code: String,
    pub prompt_language_index: u8,
    pub prompt_tone_volume: PromptToneVolume,

    // --- Speaker RGB, TWS, White Noise, Clock & Alarms ---
    pub tws_enable: bool,
    pub led_switch: bool,
    pub led_effect: LedEffectConfig,
    pub focus_mode: bool,
    pub white_noise_playing: bool,
    pub white_noise_volume: u8,
    pub white_noise_preview: bool,
    pub white_noise_tracks: WhiteNoiseTrackList,
    pub synced_time: Option<DeviceDateTime>,
    pub alarms: Vec<DeviceAlarm>,

    // --- OTA simulation counters ---
    pub ota_packets_received: usize,
    pub ota_bytes_received: usize,
}

impl Default for VirtualDeviceState {
    fn default() -> Self {
        let reg = ModelRegistry::default();
        let profile = reg
            .find_by_id("qcy-n70")
            .cloned()
            .unwrap_or_else(ModelRegistry::generic_fallback);
        Self::from_profile(&profile)
    }
}

impl VirtualDeviceState {
    /// Constructs a realistic virtual hardware state for any `model_id` in [`ModelRegistry`].
    pub fn for_model(model_id: &str) -> Result<Self, String> {
        let reg = ModelRegistry::default();
        let profile = reg
            .find_by_id(model_id)
            .ok_or_else(|| format!("unknown QCY model_id: {model_id}"))?;
        Ok(Self::from_profile(profile))
    }

    /// Constructs a realistic virtual hardware state from a [`DeviceModelProfile`].
    pub fn from_profile(profile: &DeviceModelProfile) -> Self {
        let numeric_model_id = profile.numeric_model_id.unwrap_or(0x5D40);
        let protocol_family = match &profile.protocol_family {
            ProtocolFamily::GenericPlaceholder => ProtocolFamily::QcyUnifiedTlv,
            other => other.clone(),
        };
        let is_tws = profile.capabilities.has_case_battery;
        let default_ai_matrix: Vec<AiAncMatrixEntry> = (0..9u8)
            .map(|i| AiAncMatrixEntry {
                activity: AiAncActivity::from_u8(1 + (i / 3)),
                noise_scene: 1 + (i % 3),
                setting: AncSetting::anc_scene(1 + (i % 3), 1),
            })
            .collect();

        let default_peq_bands: Vec<ParametricEqBand> = DEFAULT_EQ_FREQS_10
            .iter()
            .map(|&f| ParametricEqBand::peak(f, 0.0, 1.41))
            .collect();
        let default_peq = ParametricEqPreset {
            eq_type: 1,
            master_gain_db: 0.0,
            bands: default_peq_bands,
        };

        let button_bindings = if profile.protocol_family == ProtocolFamily::JieLiRcspLegacy
            && !profile.capabilities.has_case_battery
        {
            vec![
                ButtonBinding {
                    event_id: 0x0701,
                    action: ButtonAction::PlayPause,
                },
                ButtonBinding {
                    event_id: 0x0702,
                    action: ButtonAction::NextTrack,
                },
                ButtonBinding {
                    event_id: 0x0703,
                    action: ButtonAction::VoiceAssistant,
                },
            ]
        } else {
            vec![
                ButtonBinding {
                    event_id: 1,
                    action: ButtonAction::None,
                },
                ButtonBinding {
                    event_id: 2,
                    action: ButtonAction::None,
                },
                ButtonBinding {
                    event_id: 3,
                    action: ButtonAction::PlayPause,
                },
                ButtonBinding {
                    event_id: 4,
                    action: ButtonAction::PlayPause,
                },
                ButtonBinding {
                    event_id: 5,
                    action: ButtonAction::PreviousTrack,
                },
                ButtonBinding {
                    event_id: 6,
                    action: ButtonAction::NextTrack,
                },
                ButtonBinding {
                    event_id: 9,
                    action: ButtonAction::VoiceAssistant,
                },
                ButtonBinding {
                    event_id: 10,
                    action: ButtonAction::AncCycle,
                },
            ]
        };

        let voice_code = match profile.chipset_family.as_str() {
            "jieli" => "JL00",
            "bluetrum" => "LX00",
            _ => "WQ00",
        }
        .to_string();

        let anc_setting = if profile.capabilities.anc {
            AncSetting::anc_scene(2, 1)
        } else {
            AncSetting::off()
        };
        let (anc_mode, _) = anc_setting.to_mode_and_depth();

        Self {
            model_id: profile.model_id.clone(),
            device_name: profile.name_en.clone(),
            left_battery: 88,
            right_battery: if is_tws { 92 } else { 88 },
            case_battery: if is_tws { 76 } else { 0 },
            left_charging: false,
            right_charging: false,
            case_charging: false,
            anc_mode,
            gaming_mode: false,
            lid_open: true,
            numeric_model_id,
            protocol_family,
            ota_type: profile.ota_type,
            firmware_version: FirmwareVersion {
                major: 1,
                minor: 2,
                patch: 5,
            },
            color_index: 0,
            pop_num: 1,
            class_mac: [
                0x84,
                0xAC,
                0x60,
                ((numeric_model_id >> 8) & 0xFF) as u8,
                (numeric_model_id & 0xFF) as u8,
                0x01,
            ],
            other_mac: [
                0x84,
                0xAC,
                0x60,
                ((numeric_model_id >> 8) & 0xFF) as u8,
                (numeric_model_id & 0xFF) as u8,
                0x02,
            ],
            latency_ms: 0,
            is_tws,
            connected: true,
            anc_setting,
            legacy_noise_mode: if profile.capabilities.anc { 1 } else { 0 },
            wind_noise: false,
            btn_anc_cycle: vec![
                AncSetting::anc_scene(2, 1),
                AncSetting::transparency(0),
                AncSetting::off(),
            ],
            ai_anc_state: AiAncState {
                activity: AiAncActivity::Off,
                noise_scene: 1,
                setting: anc_setting,
                touch_index: Some(0),
            },
            ai_anc_default_matrix: default_ai_matrix.clone(),
            ai_anc_user_matrix: default_ai_matrix,
            graphic_eq: GraphicEqSetting {
                eq_type: 1,
                gains_db: vec![0; 10],
                game_eq_type: None,
                current_mode: None,
            },
            parametric_eq_v1: default_peq.clone(),
            parametric_eq_v2: default_peq.clone(),
            left_eq: default_peq.clone(),
            right_eq: default_peq,
            eq_band_count: profile.capabilities.eq_bands.max(5),
            custom_eq_test_mode: false,
            dynamic_eq: false,
            bass_enhancement: false,
            thx_eq: false,
            adaptive_volume: false,
            channel_balance: 50,
            button_bindings,
            sleep_mode: false,
            touch_tone: true,
            touch_sensitivity: TouchSensitivity::Normal,
            head_gesture_enabled: false,
            head_gesture_bindings: vec![
                HeadGestureBinding {
                    event: HeadGestureEvent::Nod,
                    action: HeadGestureAction::AnswerCall,
                },
                HeadGestureBinding {
                    event: HeadGestureEvent::Shake,
                    action: HeadGestureAction::RejectCall,
                },
            ],
            head_gesture_test_active: false,
            game_mode_pro: GameModeProConfig {
                enabled: false,
                follow_game_sound: false,
            },
            listening_mode: ListeningMode::Music,
            spatial_sound: false,
            spatial_audio_mode: SpatialAudioMode::Off,
            ldac: false,
            aptx: false,
            le_audio: false,
            privacy_mode_pro: PrivacyModeProConfig {
                enabled: false,
                call_only: false,
            },
            dialogue_perception: false,
            voice_detection_delay: 0,
            env_adaptation: false,
            power_saving: false,
            disconnect_warning: false,
            dual_connect: profile.capabilities.dual_device,
            multipoint_pairing: false,
            paired_hosts: PairedHostList {
                total_count: 2,
                connected: vec![PairedHostEntry {
                    status: 2,
                    connected: true,
                    mac: [0x24, 0x4B, 0xFE, 0x10, 0x20, 0x30],
                    name: "Windows Desktop".to_string(),
                }],
                disconnected: vec![PairedHostEntry {
                    status: 0,
                    connected: false,
                    mac: [0xA8, 0x66, 0x7F, 0x99, 0x88, 0x77],
                    name: "Mobile Phone".to_string(),
                }],
            },
            in_ear_check: profile.capabilities.in_ear_detection,
            wear_config: WearConfig {
                enabled: profile.capabilities.in_ear_detection,
                music_behavior: InEarMusicBehavior::SinglePauseAutoPlay,
                control_anc: false,
            },
            wear_sensitivity: WearSensitivity::Normal,
            in_ear_left: true,
            in_ear_right: true,
            fit_test_result: FitTestResult {
                status: 2,
                left_result: 1,
                right_result: 1,
            },
            hearing_health: HearingHealthConfig {
                alarm_volume: false,
                alarm_time: false,
                limit_minutes: 60,
            },
            power_manager: PowerManagerState {
                scheduled_minutes: PowerManagerState::DISABLED,
                remaining_minutes: 0,
            },
            standby_minutes: 30,
            prompt_voice_code: voice_code,
            prompt_language_index: 0,
            prompt_tone_volume: PromptToneVolume {
                current: 6,
                max: Some(10),
            },
            tws_enable: false,
            led_switch: profile.capabilities.rgb_lighting,
            led_effect: LedEffectConfig {
                speed: 3,
                brightness: 80,
                effect: LedEffectKind::Gradient,
                colors: vec![
                    RgbColor {
                        r: 255,
                        g: 122,
                        b: 47,
                    },
                    RgbColor {
                        r: 0,
                        g: 102,
                        b: 255,
                    },
                ],
            },
            focus_mode: false,
            white_noise_playing: false,
            white_noise_volume: 60,
            white_noise_preview: false,
            white_noise_tracks: WhiteNoiseTrackList {
                start_tone_id: 1,
                tracks: vec![
                    WhiteNoiseTrackInfo {
                        music_id: 1,
                        total: 120,
                    },
                    WhiteNoiseTrackInfo {
                        music_id: 2,
                        total: 180,
                    },
                ],
            },
            synced_time: None,
            alarms: Vec::new(),
            ota_packets_received: 0,
            ota_bytes_received: 0,
        }
    }

    /// Encodes live battery bytes `[left, right, case]` matching `0x2F` / BLE Adv `[5..7]`.
    pub fn battery_wire_bytes(&self) -> [u8; 3] {
        let l = BatteryLevel {
            percent: if self.left_battery == 0 {
                None
            } else {
                Some(self.left_battery)
            },
            charging: self.left_charging,
        };
        let r = BatteryLevel {
            percent: if !self.is_tws || self.right_battery == 0 {
                None
            } else {
                Some(self.right_battery)
            },
            charging: self.right_charging,
        };
        let c = BatteryLevel {
            percent: if !self.is_tws || self.case_battery == 0 {
                None
            } else {
                Some(self.case_battery)
            },
            charging: self.case_charging,
        };
        [l.to_wire_byte(), r.to_wire_byte(), c.to_wire_byte()]
    }

    /// Computes the live 16-bit wear bitmask for `0x62` (`bit 0` = Left, `bit 1` = Right).
    pub fn wear_status_mask(&self) -> u16 {
        (u16::from(self.in_ear_left)) | (u16::from(self.in_ear_right) << 1)
    }

    /// Builds a real 24-byte QCY BLE Manufacturer-Specific Data advertisement (`0x071E`).
    pub fn build_advertisement(&self, is_pop: bool) -> QcyAdvertisement {
        let batt = self.battery_wire_bytes();
        QcyAdvertisement {
            company_id: COMPANY_ID_QCY,
            model_id: self.numeric_model_id,
            reserved_byte2: 0x01,
            is_left: true,
            color_index: self.color_index & 0x07,
            is_tws: self.is_tws,
            is_pop,
            raw_flag_byte4: (if self.is_tws { 0x40 } else { 0x00 })
                | (if is_pop { 0x20 } else { 0x00 }),
            left_battery: BatteryLevel::from_wire_byte(batt[0]),
            right_battery: BatteryLevel::from_wire_byte(batt[1]),
            case_battery: BatteryLevel::from_wire_byte(batt[2]),
            reserved_8_10: [0x00, 0x00, 0x00],
            class_mac: Some(self.class_mac),
            pop_num: Some(self.pop_num),
            other_mac: if self.is_tws {
                Some(self.other_mac)
            } else {
                None
            },
        }
    }
}
