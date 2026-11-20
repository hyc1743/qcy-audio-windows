//! Pure state-synchronization helpers translating between `qcy_protocol` wire
//! events/types and `qcy_core::types::AppSnapshot` UI state.

use std::collections::HashMap;

use qcy_protocol::{
    format_mac, AiAncActivity, AiAncMatrixEntry, AncSetting, DeviceEvent, HeadGestureEvent,
    IrrFilterType, ListeningMode, ParametricEqBand, ParametricEqPreset, RgbColor, SpatialAudioMode,
};
use qcy_transport::now_epoch_ms;

use crate::types::{
    AiAncActivityUi, AiAncMatrix, AiAncNoiseLevel, AncPrimaryMode, AppSnapshot, ButtonMappingEntry,
    FitTestEarStatus, FitTestResult, ListeningModeUi, MultipointHostInfo, PromptLanguageUi,
    SpatialAudioModeUi, SpeakerRgbMode, WireParametricBand,
};

/// Formats raw bytes as uppercase space-separated hex (`"FF 04 17 02 01 05"`).
pub fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Parses `"#RRGGBB"` (or `"RRGGBB"`) into an [`RgbColor`], falling back to QCY orange `#FF7A2F`.
pub fn parse_hex_rgb(hex: &str) -> RgbColor {
    let clean = hex.trim().trim_start_matches('#');
    if clean.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&clean[0..2], 16),
            u8::from_str_radix(&clean[2..4], 16),
            u8::from_str_radix(&clean[4..6], 16),
        ) {
            return RgbColor { r, g, b };
        }
    }
    RgbColor {
        r: 0xFF,
        g: 0x7A,
        b: 0x2F,
    }
}

/// Formats an [`RgbColor`] into `"#RRGGBB"`.
pub fn format_hex_rgb(c: RgbColor) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b)
}

/// Converts a UI `btn_anc_cfg` mode string (`"anc_adaptive"`, `"anc_indoor"`,
/// `"anc_commute"`, `"anc_outdoor"`, `"anc_anti_wind"`, `"vocal_boost"`,
/// `"transparency"`, `"off"`, plus legacy aliases `"anc"`, `"adaptive"`, `"wind"`)
/// into a 3-byte `0x17` `AncSetting`.
pub fn anc_cycle_label_to_setting(label: &str) -> AncSetting {
    match label.trim().to_ascii_lowercase().as_str() {
        "anc_adaptive" | "adaptive" => AncSetting::adaptive(),
        "anc_indoor" | "indoor" => AncSetting::anc_scene(1, 1),
        "anc_commute" | "commute" | "anc" | "noise_cancellation" => AncSetting::anc_scene(2, 1),
        "anc_outdoor" | "outdoor" => AncSetting::anc_scene(3, 1),
        "anc_anti_wind" | "anti_wind" | "wind" => AncSetting::wind_noise_reduction(),
        "vocal_boost" => AncSetting::vocal_boost_transparency(),
        "transparency" | "transparent" => AncSetting::transparency(0),
        _ => AncSetting::off(),
    }
}

/// Converts a 3-byte `AncSetting` back into a canonical `AncControlPanel` `btn_anc_cfg` UI label.
pub fn anc_setting_to_cycle_label(s: AncSetting) -> String {
    match (s.mode, s.sub_scene) {
        (1, 1) => "anc_indoor".to_string(),
        (1, 2) => "anc_commute".to_string(),
        (1, 3) => "anc_outdoor".to_string(),
        (1, 4) => "anc_anti_wind".to_string(),
        (1, 5) | (1, _) | (5, _) => "anc_adaptive".to_string(),
        (3, 2) => "vocal_boost".to_string(),
        (3, _) => "transparency".to_string(),
        _ => "off".to_string(),
    }
}

/// Converts a 3×3 UI [`AiAncMatrix`] into 9 wire [`AiAncMatrixEntry`] cells for `0x56`.
pub fn ai_matrix_to_wire_entries(matrix: &AiAncMatrix) -> Vec<AiAncMatrixEntry> {
    let rows = [
        (AiAncActivity::Stay, &matrix.stay),
        (AiAncActivity::Walk, &matrix.walk),
        (AiAncActivity::Run, &matrix.run),
    ];
    let mut out = Vec::with_capacity(9);
    for (activity, row) in rows {
        for (noise_scene, mode) in [(1u8, row.low), (2u8, row.medium), (3u8, row.high)] {
            let setting = match mode {
                AncPrimaryMode::Off => AncSetting::off(),
                AncPrimaryMode::Transparency => AncSetting::transparency(0),
                AncPrimaryMode::Custom => AncSetting::custom_depth(10),
                AncPrimaryMode::Anc => AncSetting::anc_scene(noise_scene, 1),
            };
            out.push(AiAncMatrixEntry {
                activity,
                noise_scene,
                setting,
            });
        }
    }
    out
}

/// Converts a slice of UI [`WireParametricBand`] into a wire [`ParametricEqPreset`].
pub fn ui_bands_to_parametric_preset(
    bands: &[WireParametricBand],
    master_gain_db: f32,
    eq_type: u8,
) -> ParametricEqPreset {
    ParametricEqPreset {
        eq_type,
        master_gain_db,
        bands: bands
            .iter()
            .filter(|b| b.enabled)
            .map(|b| ParametricEqBand {
                freq_hz: b.freq_hz,
                gain_db: b.gain_db,
                q: b.q,
                filter_type: IrrFilterType::from_u8(b.irr_type).unwrap_or(IrrFilterType::Peak),
            })
            .collect(),
    }
}

/// Builds the current feature value map consumed by `ModelRegistry::evaluate_pop_view_rules`.
pub fn build_feature_state_map(snap: &AppSnapshot) -> HashMap<String, i32> {
    let mut map = HashMap::new();
    map.insert("ldac".to_string(), i32::from(snap.modes.ldac));
    map.insert(
        "dual_device".to_string(),
        i32::from(snap.multipoint.dual_device),
    );
    map.insert(
        "multi_device".to_string(),
        i32::from(snap.multipoint.multi_device || snap.multipoint.dual_device),
    );
    let uses_3mode_spatial = snap.active_model.pop_view_rules.iter().any(|r| {
        (r.target_feature == "spatial_audio" && r.target_cmd_id == 91)
            || r.conflicts_with
                .iter()
                .any(|c| c.feature == "spatial_audio" && c.cmd_id == 91)
    });
    let spatial_val = if uses_3mode_spatial {
        match snap.modes.spatial_audio {
            SpatialAudioModeUi::Off => 1,
            SpatialAudioModeUi::Fixed => 2,
            SpatialAudioModeUi::HeadTracking => 3,
        }
    } else {
        match snap.modes.spatial_audio {
            SpatialAudioModeUi::Off => 0,
            SpatialAudioModeUi::Fixed | SpatialAudioModeUi::HeadTracking => 10,
        }
    };
    map.insert("spatial_audio".to_string(), spatial_val);
    map.insert("game_mode".to_string(), i32::from(snap.modes.game_mode));
    map.insert(
        "game_mode_pro".to_string(),
        i32::from(snap.modes.game_mode_pro),
    );
    map.insert("le_audio".to_string(), i32::from(snap.modes.le_audio));
    map.insert(
        "privacy_mode_pro".to_string(),
        i32::from(snap.modes.privacy_mode),
    );
    map.insert(
        "dialogue_perception".to_string(),
        i32::from(snap.modes.dialogue_perception),
    );
    map.insert(
        "anc".to_string(),
        i32::from(snap.anc.mode != AncPrimaryMode::Off),
    );
    map.insert("ai_anc".to_string(), i32::from(snap.anc.ai_anc_enabled));
    map.insert("dynamic_eq".to_string(), i32::from(snap.eq.dynamic_eq));
    map.insert("thx_eq".to_string(), i32::from(snap.eq.thx_eq));
    map.insert("virtual_bass".to_string(), i32::from(snap.eq.virtual_bass));
    map.insert(
        "adaptive_volume".to_string(),
        i32::from(snap.eq.adaptive_volume),
    );
    map.insert(
        "eq".to_string(),
        i32::from(snap.eq.active_preset_id == "default"),
    );
    map
}

/// Applies a decoded protocol [`DeviceEvent`] onto the live [`AppSnapshot`].
pub fn apply_device_event(snap: &mut AppSnapshot, event: &DeviceEvent) {
    match event {
        DeviceEvent::BatteryUpdated {
            left,
            right,
            case,
            left_charging,
            right_charging,
            case_charging,
        } => {
            snap.battery.left = *left;
            snap.battery.right = if snap.active_model.capabilities.has_case_battery {
                *right
            } else {
                None
            };
            snap.battery.case_level = if snap.active_model.capabilities.has_case_battery {
                *case
            } else {
                None
            };
            snap.battery.left_charging = *left_charging;
            snap.battery.right_charging = *right_charging;
            snap.battery.case_charging = *case_charging;
            snap.battery.is_tws_connected = left.is_some() && right.is_some();
            if snap.is_virtual_session {
                snap.simulator.battery = snap.battery.clone();
            }
        }
        DeviceEvent::FirmwareVersion(v) => {
            snap.firmware_version = v.to_string();
            snap.ota.current_firmware = v.to_string();
        }
        DeviceEvent::DeviceNameChanged { name } => {
            if let Some(dev) = snap.active_device.as_mut() {
                dev.name = name.clone();
            }
        }
        DeviceEvent::DeviceColorChanged { color_index } => {
            snap.device_color_index = *color_index;
        }
        DeviceEvent::AncSettingUpdated(setting) => {
            snap.anc.apply_wire_setting(*setting);
        }
        DeviceEvent::AncModeChanged { .. } => {}
        DeviceEvent::WindNoiseChanged { enabled } => {
            snap.anc.wind_noise = *enabled;
        }
        DeviceEvent::ButtonAncCycleUpdated { modes } => {
            if !modes.is_empty() {
                snap.anc.btn_anc_cfg = modes
                    .iter()
                    .copied()
                    .map(anc_setting_to_cycle_label)
                    .collect();
            }
        }
        DeviceEvent::AiAncStateUpdated(st) => {
            snap.anc.ai_anc_enabled = st.activity != AiAncActivity::Off;
            snap.anc.ai_activity = match st.activity {
                AiAncActivity::Detecting => AiAncActivityUi::Detecting,
                AiAncActivity::Stay => AiAncActivityUi::Stay,
                AiAncActivity::Walk => AiAncActivityUi::Walk,
                AiAncActivity::Run => AiAncActivityUi::Run,
                _ => AiAncActivityUi::Stay,
            };
            snap.anc.ai_noise_level = match st.noise_scene {
                1 => AiAncNoiseLevel::Low,
                3 => AiAncNoiseLevel::High,
                _ => AiAncNoiseLevel::Medium,
            };
        }
        DeviceEvent::GraphicEqUpdated(eq) => {
            if eq.gains_db.len() == 10 {
                snap.eq.gains_10band_db = eq.gains_db.iter().map(|&g| g as f32).collect();
            }
        }
        DeviceEvent::ParametricEqV1Updated(eq) | DeviceEvent::ParametricEqV2Updated(eq) => {
            snap.eq.master_gain_db = eq.master_gain_db;
            snap.eq.parametric_bands = eq
                .bands
                .iter()
                .map(|b| WireParametricBand {
                    freq_hz: b.freq_hz,
                    gain_db: b.gain_db,
                    q: b.q,
                    irr_type: b.filter_type.as_u8(),
                    enabled: true,
                })
                .collect();
        }
        DeviceEvent::DynamicEqChanged { enabled } => snap.eq.dynamic_eq = *enabled,
        DeviceEvent::BassEnhancementChanged { enabled } => snap.eq.virtual_bass = *enabled,
        DeviceEvent::ThxEqChanged { enabled } => snap.eq.thx_eq = *enabled,
        DeviceEvent::AdaptiveVolumeChanged { enabled } => snap.eq.adaptive_volume = *enabled,
        DeviceEvent::ChannelBalanceUpdated { balance } => {
            snap.eq.channel_balance = ((*balance as f32) - 50.0) / 50.0;
        }
        DeviceEvent::ButtonBindingsUpdated { bindings } => {
            snap.gestures.mappings = bindings
                .iter()
                .map(|b| {
                    let key_id = if b.event_id > 0xFF {
                        b.event_id >> 8
                    } else if b.event_id % 2 == 1 {
                        1
                    } else {
                        2
                    };
                    ButtonMappingEntry {
                        key_id,
                        event_id: b.event_id,
                        cmd: b.action.as_u8(),
                    }
                })
                .collect();
        }
        DeviceEvent::SleepModeChanged { enabled } => {
            // SleepMode = touch lock -> touch_enable is inverse if used as lock; preserve explicit sync.
            snap.gestures.touch_enable = !*enabled;
        }
        DeviceEvent::TouchSensitivityUpdated { sensitivity } => {
            snap.gestures.touch_sensitivity = sensitivity.as_u8();
        }
        DeviceEvent::HeadGestureEnabledChanged { enabled } => {
            snap.gestures.head_controls.enabled = *enabled;
        }
        DeviceEvent::HeadGestureBindingsUpdated { bindings } => {
            for b in bindings {
                match b.event {
                    HeadGestureEvent::Nod => snap.gestures.head_controls.nod_cmd = b.action.as_u8(),
                    HeadGestureEvent::Shake => {
                        snap.gestures.head_controls.shake_cmd = b.action.as_u8()
                    }
                    HeadGestureEvent::TiltLeft => {
                        snap.gestures.head_controls.tilt_left_cmd = b.action.as_u8()
                    }
                    HeadGestureEvent::TiltRight => {
                        snap.gestures.head_controls.tilt_right_cmd = b.action.as_u8()
                    }
                    HeadGestureEvent::Raw(_) => {}
                }
            }
        }
        DeviceEvent::GamingModeChanged { enabled } => snap.modes.game_mode = *enabled,
        DeviceEvent::GameModeProUpdated(cfg) => {
            snap.modes.game_mode_pro = cfg.enabled;
            snap.modes.game_mode = cfg.enabled;
            snap.modes.game_sync_eq = cfg.follow_game_sound;
        }
        DeviceEvent::ListeningModeUpdated { mode } => {
            snap.modes.listening_mode = match mode {
                ListeningMode::Game => ListeningModeUi::Game,
                ListeningMode::Movie => ListeningModeUi::Movie,
                _ => ListeningModeUi::Music,
            };
        }
        DeviceEvent::SpatialSoundChanged { enabled } => {
            snap.modes.spatial_audio = if *enabled {
                SpatialAudioModeUi::Fixed
            } else {
                SpatialAudioModeUi::Off
            };
        }
        DeviceEvent::SpatialAudioModeUpdated { mode } => {
            snap.modes.spatial_audio = match mode {
                SpatialAudioMode::Fixed => SpatialAudioModeUi::Fixed,
                SpatialAudioMode::HeadTracking => SpatialAudioModeUi::HeadTracking,
                _ => SpatialAudioModeUi::Off,
            };
        }
        DeviceEvent::LdacChanged { enabled } => snap.modes.ldac = *enabled,
        DeviceEvent::LeAudioChanged { enabled } => snap.modes.le_audio = *enabled,
        DeviceEvent::PrivacyModeProUpdated(cfg) => {
            snap.modes.privacy_mode = cfg.enabled;
            snap.modes.privacy_scope = u8::from(cfg.call_only);
        }
        DeviceEvent::DialoguePerceptionChanged { enabled } => {
            snap.modes.dialogue_perception = *enabled;
        }
        DeviceEvent::VoiceDetectionDelayUpdated { seconds } => {
            snap.modes.speak_to_chat_sec = *seconds;
        }
        DeviceEvent::PowerSavingModeChanged { enabled } => snap.modes.power_saving = *enabled,
        DeviceEvent::DisconnectWarningChanged { enabled } => {
            snap.modes.disconnect_warning = *enabled;
        }
        DeviceEvent::DualConnectChanged { enabled } => {
            snap.multipoint.dual_device = *enabled;
            snap.multipoint.multi_device = *enabled;
        }
        DeviceEvent::PairedHostsUpdated(list) => {
            let mut hosts = Vec::new();
            for entry in list.connected.iter().chain(list.disconnected.iter()) {
                hosts.push(MultipointHostInfo {
                    mac: format_mac(entry.mac),
                    name: entry.name.clone(),
                    is_this_device: false,
                    is_connected: entry.connected,
                });
            }
            snap.multipoint.hosts = hosts;
        }
        DeviceEvent::InEarCheckChanged { enabled } => {
            if !*enabled {
                snap.modes.in_ear_mode = 0;
            } else if snap.modes.in_ear_mode == 0 {
                snap.modes.in_ear_mode = 1;
            }
        }
        DeviceEvent::WearConfigUpdated(cfg) => {
            snap.modes.in_ear_mode = if cfg.enabled {
                cfg.music_behavior.as_u8()
            } else {
                0
            };
            snap.modes.in_ear_auto_anc = cfg.control_anc;
        }
        DeviceEvent::WearSensitivityUpdated { sensitivity } => {
            snap.modes.in_ear_sensitivity = sensitivity.as_u8();
        }
        DeviceEvent::WearStatusUpdated { mask } => {
            snap.battery.left_in_case = (*mask & 0x01) == 0;
            snap.battery.right_in_case = (*mask & 0x02) == 0;
        }
        DeviceEvent::FitTestUpdated(res) if res.status == 2 => {
            snap.last_fit_test = Some(FitTestResult {
                left: FitTestEarStatus::from_wire(res.left_result),
                right: FitTestEarStatus::from_wire(res.right_result),
                tested_at_ms: now_epoch_ms(),
            });
        }
        DeviceEvent::HearingHealthUpdated(hh) => {
            snap.modes.hearing_low_vol_remind = hh.alarm_volume;
            snap.modes.hearing_duration_remind = hh.alarm_time;
            snap.modes.hearing_duration_min = hh.limit_minutes;
        }
        DeviceEvent::PowerManagerUpdated(pm) => {
            snap.modes.auto_power_off_min = if pm.scheduled_minutes == 0xFFFF {
                0
            } else {
                pm.scheduled_minutes
            };
        }
        DeviceEvent::StandbyMinutesUpdated { minutes } => {
            snap.modes.disconnect_power_off_min = if *minutes == 0xFFFF { 0 } else { *minutes };
        }
        DeviceEvent::PromptVoiceCodeUpdated { code } => {
            snap.modes.prompt_language = if code.ends_with("01") {
                PromptLanguageUi::Zh
            } else {
                PromptLanguageUi::En
            };
        }
        DeviceEvent::PromptLanguageIndexUpdated { index } => {
            snap.modes.prompt_language = if *index == 1 {
                PromptLanguageUi::Zh
            } else {
                PromptLanguageUi::En
            };
        }
        DeviceEvent::PromptToneVolumeUpdated(v) => {
            snap.modes.prompt_volume = v.current;
        }
        DeviceEvent::TwsEnableChanged { enabled } => {
            snap.rgb_lighting.tws_pair_enabled = *enabled;
        }
        DeviceEvent::LedSwitchChanged { enabled } => {
            snap.rgb_lighting.enabled = *enabled;
        }
        DeviceEvent::LedEffectUpdated(cfg) => {
            snap.rgb_lighting.speed = cfg.speed;
            snap.rgb_lighting.brightness = cfg.brightness;
            snap.rgb_lighting.mode = SpeakerRgbMode::from_wire(cfg.effect);
            if let Some(first) = cfg.colors.first() {
                snap.rgb_lighting.color_hex = format_hex_rgb(*first);
            }
        }
        DeviceEvent::FocusModeChanged { enabled } => {
            snap.focus_white_noise.hardware_playing = *enabled;
        }
        DeviceEvent::WhiteNoiseVolumeUpdated { volume, .. } => {
            snap.focus_white_noise.hardware_volume = *volume;
        }
        DeviceEvent::WhiteNoisePlayStateUpdated(st) => {
            snap.focus_white_noise.hardware_playing = st.status == 1;
            snap.focus_white_noise.hardware_track_index = st.music_index;
        }
        _ => {}
    }
}
