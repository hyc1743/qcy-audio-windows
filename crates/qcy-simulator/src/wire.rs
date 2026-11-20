//! Raw-byte wire handler emulating real QCY Unified TLV (`0xA001`),
//! JieLi `isNewJL` RCSP (`0xA002` + `0xFF` TLV tunnel), Legacy JieLi native RCSP
//! (`0xA002` `0x03`/`0x07`/`0x08`/`0xC0`–`0xC5`), Legacy Direct GATT, and vendor OTA ACKs.

use qcy_protocol::ota::{
    BES_OTA_MAGIC, BLUETRUM_FOTA_MAGIC, PIXART_RX_MAGIC, PIXART_TX_MAGIC, WUQI_BLE_REQ_MAGIC,
    WUQI_BLE_RSP_MAGIC,
};
use qcy_protocol::rcsp::{
    change_jl_action_to_qcy, change_qcy_action_to_jl, combine_jl_button_event,
    split_jl_button_event, unwrap_qcy_tlv_from_rcsp, RcspFrame, RcspStreamAccumulator,
    JL_ADV_ATTR_BATTERY, JL_ADV_ATTR_KEY_SETTINGS, JL_ADV_ATTR_NAME, JL_ADV_ATTR_WORK_MODE,
    JL_SYS_ATTR_EQ, JL_SYS_ATTR_NOISE_MODE, JL_SYS_FUNC_PUBLIC, OP_ADV_GET_INFO,
    OP_ADV_MODIFY_INFO, OP_EXTRA_CUSTOM, OP_GET_SYS_INFO, OP_GET_TARGET_INFO, OP_OTA_ENTER_UPDATE,
    OP_OTA_EXIT_UPDATE, OP_OTA_GET_FILE_INFO_OFFSET, OP_OTA_INQUIRE_CAN_UPDATE,
    OP_OTA_NOTIFY_FILE_SIZE, OP_OTA_REBOOT, OP_OTA_SEND_BLOCK, OP_SET_SYS_INFO,
};
use qcy_protocol::tlv::{
    decode_ai_anc_matrix, decode_anc_setting, decode_btn_anc_cycle, decode_graphic_eq,
    decode_hearing_health, decode_led_effect, decode_parametric_eq_v1, decode_parametric_eq_v2,
    decode_qcy_bool, decode_tlv_frame, decode_wear_config, encode_ai_anc_matrix_entry,
    encode_anc_setting, encode_btn_anc_cycle, encode_graphic_eq, encode_hearing_health,
    encode_led_effect, encode_parametric_eq_v1, encode_parametric_eq_v2, encode_qcy_bool,
    encode_single_tlv, encode_wear_config, TlvItem,
};
use qcy_protocol::{
    AiAncActivity, AncSetting, ButtonAction, ButtonBinding, CmdId, DeviceAlarm, DeviceDateTime,
    GameModeProConfig, HeadGestureAction, HeadGestureBinding, HeadGestureEvent, ListeningMode,
    MultipointHostStatus, PowerManagerState, PrivacyModeProConfig, SpatialAudioMode,
    TouchSensitivity, WearSensitivity, CHAR_BES_OTA, CHAR_BLUETRUM_FOTA_WRITE, CHAR_LEGACY_BATTERY,
    CHAR_LEGACY_BUTTON, CHAR_LEGACY_EQ, CHAR_LEGACY_VERSION, CHAR_WUQI_OTA_WRITE,
};
use uuid::Uuid;

use crate::state::VirtualDeviceState;

/// Encodes the current value of `cmd` inside `state` as a single QCY `[0xFF, ...]` TLV frame.
pub fn build_tlv_snapshot_for_cmd(
    state: &VirtualDeviceState,
    cmd: CmdId,
    is_new_jl: bool,
) -> Option<Vec<u8>> {
    let val: Vec<u8> = match cmd {
        CmdId::Battery => state.battery_wire_bytes().to_vec(),
        CmdId::Version => vec![
            state.firmware_version.major,
            state.firmware_version.minor,
            state.firmware_version.patch,
        ],
        CmdId::PairName => state.device_name.as_bytes().to_vec(),
        CmdId::DeviceColor => vec![state.color_index],
        CmdId::AncSetting => encode_anc_setting(state.anc_setting).to_vec(),
        CmdId::NoiseMode => vec![state.legacy_noise_mode],
        CmdId::WindNoise => vec![u8::from(state.wind_noise)],
        CmdId::AncWear | CmdId::WearStatus => state.wear_status_mask().to_le_bytes().to_vec(),
        CmdId::BtnAncCfg => encode_btn_anc_cycle(&state.btn_anc_cycle),
        CmdId::AiAnc => {
            let mut out = vec![
                state.ai_anc_state.activity.as_u8(),
                state.ai_anc_state.noise_scene,
                state.ai_anc_state.setting.mode,
                state.ai_anc_state.setting.sub_scene,
                state.ai_anc_state.setting.level,
            ];
            if let Some(ti) = state.ai_anc_state.touch_index {
                out.push(ti);
            }
            out
        }
        CmdId::AiAncCfg => state
            .ai_anc_default_matrix
            .iter()
            .flat_map(|e| encode_ai_anc_matrix_entry(*e))
            .collect(),
        CmdId::AiAncCfg2 => state
            .ai_anc_user_matrix
            .iter()
            .flat_map(|e| encode_ai_anc_matrix_entry(*e))
            .collect(),
        CmdId::GainEq => encode_graphic_eq(&state.graphic_eq),
        CmdId::MultiEq => encode_parametric_eq_v1(&state.parametric_eq_v1),
        CmdId::MultiEq2 => encode_parametric_eq_v2(&state.parametric_eq_v2),
        CmdId::EqCount => vec![state.eq_band_count],
        CmdId::DynamicEq => vec![u8::from(state.dynamic_eq)],
        CmdId::BassEnhancement => vec![u8::from(state.bass_enhancement)],
        CmdId::ThxEq => vec![u8::from(state.thx_eq)],
        CmdId::AdaptVolume => vec![u8::from(state.adaptive_volume)],
        CmdId::ChannelBalance => vec![state.channel_balance],
        CmdId::ButtonFunc => state
            .button_bindings
            .iter()
            .flat_map(|b| [(b.event_id & 0xFF) as u8, b.action.as_u8()])
            .collect(),
        CmdId::SleepMode => vec![u8::from(state.sleep_mode)],
        CmdId::TouchTone => vec![u8::from(state.touch_tone)],
        CmdId::TouchSensitivity => vec![state.touch_sensitivity.as_u8()],
        CmdId::HeadAction => vec![u8::from(state.head_gesture_enabled)],
        CmdId::SensorConfig => state
            .head_gesture_bindings
            .iter()
            .flat_map(|b| [b.event.as_u8(), b.action.as_u8()])
            .collect(),
        CmdId::GameMode => vec![encode_qcy_bool(state.gaming_mode, is_new_jl)],
        CmdId::GameConfig => vec![
            u8::from(state.game_mode_pro.enabled),
            u8::from(state.game_mode_pro.follow_game_sound),
        ],
        CmdId::MusicMode => vec![state.listening_mode.as_u8()],
        CmdId::SpaceSound => vec![u8::from(state.spatial_sound)],
        CmdId::SpaceAudioMode => vec![state.spatial_audio_mode.as_u8()],
        CmdId::Ldac => vec![u8::from(state.ldac)],
        CmdId::Aptx => vec![u8::from(state.aptx)],
        CmdId::LeAudio => vec![u8::from(state.le_audio)],
        CmdId::PrivacyMode => vec![
            u8::from(state.privacy_mode_pro.enabled),
            u8::from(state.privacy_mode_pro.call_only),
        ],
        CmdId::TalkCheck => vec![u8::from(state.dialogue_perception)],
        CmdId::VoiceDetection => vec![state.voice_detection_delay],
        CmdId::EnvAdaptation => vec![u8::from(state.env_adaptation)],
        CmdId::PowerSavingMode => vec![u8::from(state.power_saving)],
        CmdId::DisconnectWarning => vec![u8::from(state.disconnect_warning)],
        CmdId::DualConnect => vec![u8::from(state.dual_connect)],
        CmdId::PhoneInfo => encode_paired_host_list(&state.paired_hosts),
        CmdId::InEarCheck => vec![encode_qcy_bool(state.in_ear_check, is_new_jl)],
        CmdId::WearConfig => encode_wear_config(state.wear_config).to_vec(),
        CmdId::InEarSensitivity => vec![state.wear_sensitivity.as_u8()],
        CmdId::FitTest => vec![
            state.fit_test_result.status,
            state.fit_test_result.left_result,
            state.fit_test_result.right_result,
        ],
        CmdId::HearHealth => encode_hearing_health(state.hearing_health).to_vec(),
        CmdId::PowerManager => {
            let mut out = Vec::with_capacity(4);
            out.extend_from_slice(&state.power_manager.scheduled_minutes.to_le_bytes());
            out.extend_from_slice(&state.power_manager.remaining_minutes.to_le_bytes());
            out
        }
        CmdId::Standby => state.standby_minutes.to_le_bytes().to_vec(),
        CmdId::VoiceName => state.prompt_voice_code.as_bytes().to_vec(),
        CmdId::VoiceLanguage => vec![state.prompt_language_index],
        CmdId::ToneVolume => vec![
            state.prompt_tone_volume.current,
            state.prompt_tone_volume.max.unwrap_or(10),
        ],
        CmdId::TwsEnable => vec![u8::from(state.tws_enable)],
        CmdId::LedSwitch => vec![u8::from(state.led_switch)],
        CmdId::LedEffect => encode_led_effect(&state.led_effect),
        CmdId::FocusMode => vec![u8::from(state.focus_mode)],
        CmdId::Volume => vec![state.white_noise_volume, state.white_noise_volume, 0],
        CmdId::MusicState => {
            let mut out = Vec::with_capacity(6);
            out.extend_from_slice(&1u32.to_le_bytes());
            out.push(u8::from(state.white_noise_playing));
            out.push(0);
            out
        }
        CmdId::MusicInfo => {
            let mut out = vec![state.white_noise_tracks.start_tone_id];
            for t in &state.white_noise_tracks.tracks {
                out.extend_from_slice(&t.music_id.to_le_bytes());
                out.extend_from_slice(&t.total.to_le_bytes());
            }
            out
        }
        CmdId::Alarm => {
            let mut out = vec![0x01];
            for a in &state.alarms {
                out.extend_from_slice(&[
                    a.alarm_id,
                    u8::from(a.enabled),
                    a.hour,
                    a.minute,
                    a.cycle_mask,
                    0x00,
                ]);
            }
            out
        }
        _ => return None,
    };

    encode_single_tlv(cmd.as_u8(), &val).ok()
}

fn encode_paired_host_list(list: &qcy_protocol::PairedHostList) -> Vec<u8> {
    let mut out = vec![list.total_count];
    for entry in list.connected.iter().chain(list.disconnected.iter()) {
        let name_bytes = entry.name.as_bytes();
        let entry_len = 7 + name_bytes.len();
        out.push(entry_len as u8);
        out.push(entry.status);
        out.extend_from_slice(&entry.mac);
        out.extend_from_slice(name_bytes);
    }
    out
}

/// Processes a raw QCY `[0xFF, PayloadLen, ...]` TLV frame written to `0x1001`
/// (or tunneled inside JieLi RCSP `0xFF` when `is_new_jl == true`), mutating
/// `state` and returning authentic raw `[0xFF, ...]` TLV notification packets.
pub fn handle_qcy_tlv_write(
    state: &mut VirtualDeviceState,
    raw_frame: &[u8],
    is_new_jl: bool,
) -> Vec<Vec<u8>> {
    let Ok(items) = decode_tlv_frame(raw_frame) else {
        return Vec::new();
    };
    let mut replies = Vec::new();
    for item in items {
        replies.extend(apply_tlv_item(state, &item, is_new_jl));
    }
    replies
}

fn apply_tlv_item(state: &mut VirtualDeviceState, item: &TlvItem, is_new_jl: bool) -> Vec<Vec<u8>> {
    let Some(cmd) = item.cmd_id() else {
        return Vec::new();
    };
    let val = item.value.as_slice();

    match cmd {
        CmdId::RequestData => {
            if let Some(&target) = val.first() {
                if let Some(target_cmd) = CmdId::from_u8(target) {
                    if target_cmd == CmdId::PhoneInfo {
                        let start = encode_single_tlv(
                            CmdId::PhoneStatus.as_u8(),
                            &[0, 0, 0, 0, 0, 0, MultipointHostStatus::UPLOAD_START],
                        )
                        .ok();
                        let body = build_tlv_snapshot_for_cmd(state, CmdId::PhoneInfo, is_new_jl);
                        let end = encode_single_tlv(
                            CmdId::PhoneStatus.as_u8(),
                            &[0, 0, 0, 0, 0, 0, MultipointHostStatus::UPLOAD_END],
                        )
                        .ok();
                        return [start, body, end].into_iter().flatten().collect();
                    }
                    return build_tlv_snapshot_for_cmd(state, target_cmd, is_new_jl)
                        .into_iter()
                        .collect();
                }
            }
            Vec::new()
        }
        CmdId::ResetDefault | CmdId::FactoryReset => {
            let model_id = state.model_id.clone();
            if let Ok(fresh) = VirtualDeviceState::for_model(&model_id) {
                *state = fresh;
            }
            build_tlv_snapshot_for_cmd(state, CmdId::AncSetting, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::ClearPair => {
            state.paired_hosts.disconnected.clear();
            state.paired_hosts.total_count = state.paired_hosts.connected.len() as u8;
            build_tlv_snapshot_for_cmd(state, CmdId::PhoneInfo, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::PairName => {
            state.device_name = String::from_utf8_lossy(val).into_owned();
            build_tlv_snapshot_for_cmd(state, CmdId::PairName, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::DeviceColor if !val.is_empty() => {
            state.color_index = val[0];
            build_tlv_snapshot_for_cmd(state, CmdId::DeviceColor, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::AncSetting => {
            if let Ok(setting) = decode_anc_setting(val) {
                state.anc_setting = setting;
                state.anc_mode = setting.to_mode_and_depth().0;
                let mut out: Vec<Vec<u8>> =
                    build_tlv_snapshot_for_cmd(state, CmdId::AncSetting, is_new_jl)
                        .into_iter()
                        .collect();
                if setting.mode == 1 && setting.sub_scene == 5 {
                    // Adaptive ANC emits calibration completion status 1 on 0x28.
                    if let Ok(cal) = encode_single_tlv(CmdId::AncResult.as_u8(), &[0x01]) {
                        out.push(cal);
                    }
                }
                out
            } else {
                Vec::new()
            }
        }
        CmdId::NoiseMode if !val.is_empty() => {
            state.legacy_noise_mode = val[0];
            build_tlv_snapshot_for_cmd(state, CmdId::NoiseMode, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::WindNoise if !val.is_empty() => {
            state.wind_noise = decode_qcy_bool(val[0]);
            build_tlv_snapshot_for_cmd(state, CmdId::WindNoise, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::BtnAncCfg => {
            if val == [0xFF, 0xFF, 0xFF] {
                state.btn_anc_cycle = vec![
                    AncSetting::anc_scene(2, 1),
                    AncSetting::transparency(0),
                    AncSetting::off(),
                ];
            } else if let Ok(modes) = decode_btn_anc_cycle(val) {
                state.btn_anc_cycle = modes;
            }
            build_tlv_snapshot_for_cmd(state, CmdId::BtnAncCfg, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::AiAnc if !val.is_empty() => {
            state.ai_anc_state.activity = if val[0] == 0x04 {
                AiAncActivity::Off
            } else {
                AiAncActivity::Stay
            };
            build_tlv_snapshot_for_cmd(state, CmdId::AiAnc, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::AiAncCfg2 => {
            if let Ok(entries) = decode_ai_anc_matrix(val) {
                for entry in entries {
                    if let Some(slot) = state.ai_anc_user_matrix.iter_mut().find(|e| {
                        e.activity == entry.activity && e.noise_scene == entry.noise_scene
                    }) {
                        *slot = entry;
                    } else {
                        state.ai_anc_user_matrix.push(entry);
                    }
                }
            }
            build_tlv_snapshot_for_cmd(state, CmdId::AiAncCfg2, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::AiAncPreview if val.len() >= 4 => {
            if val[0] == 2 {
                state.anc_setting = AncSetting::new(val[1], val[2], val[3]);
                state.anc_mode = state.anc_setting.to_mode_and_depth().0;
            }
            build_tlv_snapshot_for_cmd(state, CmdId::AncSetting, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::GainEq => {
            if let Ok(eq) = decode_graphic_eq(val) {
                state.graphic_eq = eq;
            }
            build_tlv_snapshot_for_cmd(state, CmdId::GainEq, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::MultiEq => {
            if let Ok(eq) = decode_parametric_eq_v1(val) {
                state.parametric_eq_v1 = eq;
            }
            build_tlv_snapshot_for_cmd(state, CmdId::MultiEq, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::MultiEq2 => {
            if let Ok(eq) = decode_parametric_eq_v2(val) {
                state.parametric_eq_v2 = eq;
            }
            build_tlv_snapshot_for_cmd(state, CmdId::MultiEq2, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::MultiEqLeft => {
            if let Ok(eq) = decode_parametric_eq_v2(val) {
                state.left_eq = eq;
            }
            encode_single_tlv(
                CmdId::MultiEq2.as_u8(),
                &encode_parametric_eq_v2(&state.left_eq),
            )
            .into_iter()
            .collect()
        }
        CmdId::MultiEqRight => {
            if let Ok(eq) = decode_parametric_eq_v2(val) {
                state.right_eq = eq;
            }
            encode_single_tlv(
                CmdId::MultiEq2.as_u8(),
                &encode_parametric_eq_v2(&state.right_eq),
            )
            .into_iter()
            .collect()
        }
        CmdId::CustomEqTest if !val.is_empty() => {
            state.custom_eq_test_mode = decode_qcy_bool(val[0]);
            Vec::new()
        }
        CmdId::DynamicEq if !val.is_empty() => {
            state.dynamic_eq = decode_qcy_bool(val[0]);
            build_tlv_snapshot_for_cmd(state, CmdId::DynamicEq, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::BassEnhancement if !val.is_empty() => {
            state.bass_enhancement = decode_qcy_bool(val[0]);
            build_tlv_snapshot_for_cmd(state, CmdId::BassEnhancement, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::ThxEq if !val.is_empty() => {
            state.thx_eq = decode_qcy_bool(val[0]);
            build_tlv_snapshot_for_cmd(state, CmdId::ThxEq, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::AdaptVolume if !val.is_empty() => {
            state.adaptive_volume = decode_qcy_bool(val[0]);
            build_tlv_snapshot_for_cmd(state, CmdId::AdaptVolume, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::ChannelBalance if !val.is_empty() => {
            state.channel_balance = val[0].min(100);
            build_tlv_snapshot_for_cmd(state, CmdId::ChannelBalance, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::ButtonFunc if val.len() >= 2 => {
            if val[0] == 0xFF && val[1] == 0xFF {
                if let Ok(fresh) = VirtualDeviceState::for_model(&state.model_id) {
                    state.button_bindings = fresh.button_bindings;
                }
            } else {
                let event_id = val[0] as u16;
                let action = ButtonAction::from_u8(val[1]);
                if let Some(slot) = state
                    .button_bindings
                    .iter_mut()
                    .find(|b| b.event_id == event_id)
                {
                    slot.action = action;
                } else {
                    state
                        .button_bindings
                        .push(ButtonBinding { event_id, action });
                }
            }
            build_tlv_snapshot_for_cmd(state, CmdId::ButtonFunc, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::SleepMode if !val.is_empty() => {
            state.sleep_mode = decode_qcy_bool(val[0]);
            build_tlv_snapshot_for_cmd(state, CmdId::SleepMode, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::TouchTone if !val.is_empty() => {
            state.touch_tone = decode_qcy_bool(val[0]);
            build_tlv_snapshot_for_cmd(state, CmdId::TouchTone, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::TouchSensitivity if !val.is_empty() => {
            state.touch_sensitivity = TouchSensitivity::from_u8(val[0]);
            build_tlv_snapshot_for_cmd(state, CmdId::TouchSensitivity, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::HeadAction if !val.is_empty() => {
            state.head_gesture_enabled = decode_qcy_bool(val[0]);
            build_tlv_snapshot_for_cmd(state, CmdId::HeadAction, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::SensorConfig if val.len() >= 2 => {
            let event = HeadGestureEvent::from_u8(val[0]);
            let action = HeadGestureAction::from_u8(val[1]);
            if let Some(slot) = state
                .head_gesture_bindings
                .iter_mut()
                .find(|b| b.event == event)
            {
                slot.action = action;
            } else {
                state
                    .head_gesture_bindings
                    .push(HeadGestureBinding { event, action });
            }
            build_tlv_snapshot_for_cmd(state, CmdId::SensorConfig, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::SensorTest if !val.is_empty() => {
            state.head_gesture_test_active = val[0] == 0xFA;
            if state.head_gesture_test_active {
                encode_single_tlv(CmdId::SensorTest.as_u8(), &[HeadGestureEvent::Nod.as_u8()])
                    .into_iter()
                    .collect()
            } else {
                Vec::new()
            }
        }
        CmdId::GameMode if !val.is_empty() => {
            state.gaming_mode = decode_qcy_bool(val[0]);
            build_tlv_snapshot_for_cmd(state, CmdId::GameMode, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::GameConfig if val.len() >= 2 => {
            state.game_mode_pro = GameModeProConfig {
                enabled: decode_qcy_bool(val[0]),
                follow_game_sound: decode_qcy_bool(val[1]),
            };
            state.gaming_mode = state.game_mode_pro.enabled;
            build_tlv_snapshot_for_cmd(state, CmdId::GameConfig, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::MusicMode if !val.is_empty() => {
            state.listening_mode = ListeningMode::from_u8(val[0]);
            build_tlv_snapshot_for_cmd(state, CmdId::MusicMode, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::SpaceSound if !val.is_empty() => {
            state.spatial_sound = decode_qcy_bool(val[0]);
            build_tlv_snapshot_for_cmd(state, CmdId::SpaceSound, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::SpaceAudioMode if !val.is_empty() => {
            state.spatial_audio_mode = SpatialAudioMode::from_u8(val[0]);
            build_tlv_snapshot_for_cmd(state, CmdId::SpaceAudioMode, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::Ldac if !val.is_empty() => {
            state.ldac = decode_qcy_bool(val[0]);
            build_tlv_snapshot_for_cmd(state, CmdId::Ldac, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::Aptx if !val.is_empty() => {
            state.aptx = decode_qcy_bool(val[0]);
            build_tlv_snapshot_for_cmd(state, CmdId::Aptx, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::LeAudio if !val.is_empty() => {
            state.le_audio = decode_qcy_bool(val[0]);
            build_tlv_snapshot_for_cmd(state, CmdId::LeAudio, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::PrivacyMode if val.len() >= 2 => {
            state.privacy_mode_pro = PrivacyModeProConfig {
                enabled: decode_qcy_bool(val[0]),
                call_only: val[1] == 1,
            };
            build_tlv_snapshot_for_cmd(state, CmdId::PrivacyMode, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::TalkCheck if !val.is_empty() => {
            state.dialogue_perception = decode_qcy_bool(val[0]);
            build_tlv_snapshot_for_cmd(state, CmdId::TalkCheck, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::VoiceDetection if !val.is_empty() => {
            state.voice_detection_delay = val[0];
            build_tlv_snapshot_for_cmd(state, CmdId::VoiceDetection, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::EnvAdaptation if !val.is_empty() => {
            state.env_adaptation = decode_qcy_bool(val[0]);
            build_tlv_snapshot_for_cmd(state, CmdId::EnvAdaptation, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::PowerSavingMode if !val.is_empty() => {
            state.power_saving = decode_qcy_bool(val[0]);
            build_tlv_snapshot_for_cmd(state, CmdId::PowerSavingMode, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::DisconnectWarning if !val.is_empty() => {
            state.disconnect_warning = decode_qcy_bool(val[0]);
            build_tlv_snapshot_for_cmd(state, CmdId::DisconnectWarning, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::DualConnect if !val.is_empty() => {
            state.dual_connect = decode_qcy_bool(val[0]);
            build_tlv_snapshot_for_cmd(state, CmdId::DualConnect, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::MultiPair if !val.is_empty() => {
            state.multipoint_pairing = decode_qcy_bool(val[0]);
            Vec::new()
        }
        CmdId::PhoneStatus if val.len() >= 7 => {
            let action = val[0];
            let mut mac = [0u8; 6];
            mac.copy_from_slice(&val[1..7]);
            match action {
                1 => {
                    // Connect disconnected host
                    if let Some(pos) = state
                        .paired_hosts
                        .disconnected
                        .iter()
                        .position(|h| h.mac == mac)
                    {
                        let mut host = state.paired_hosts.disconnected.remove(pos);
                        host.status = 2;
                        host.connected = true;
                        state.paired_hosts.connected.push(host);
                    }
                }
                2 => {
                    // Delete host
                    state.paired_hosts.connected.retain(|h| h.mac != mac);
                    state.paired_hosts.disconnected.retain(|h| h.mac != mac);
                    state.paired_hosts.total_count = (state.paired_hosts.connected.len()
                        + state.paired_hosts.disconnected.len())
                        as u8;
                }
                3 => {
                    // Disconnect host
                    if let Some(pos) = state
                        .paired_hosts
                        .connected
                        .iter()
                        .position(|h| h.mac == mac)
                    {
                        let mut host = state.paired_hosts.connected.remove(pos);
                        host.status = 0;
                        host.connected = false;
                        state.paired_hosts.disconnected.push(host);
                    }
                }
                _ => {}
            }
            let mut rsp = Vec::with_capacity(7);
            rsp.extend_from_slice(&mac);
            rsp.push(action);
            let status_pkt = encode_single_tlv(CmdId::PhoneStatus.as_u8(), &rsp).ok();
            let list_pkt = build_tlv_snapshot_for_cmd(state, CmdId::PhoneInfo, is_new_jl);
            [status_pkt, list_pkt].into_iter().flatten().collect()
        }
        CmdId::InEarCheck if !val.is_empty() => {
            state.in_ear_check = decode_qcy_bool(val[0]);
            build_tlv_snapshot_for_cmd(state, CmdId::InEarCheck, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::WearConfig => {
            if let Ok(cfg) = decode_wear_config(val) {
                state.wear_config = cfg;
                state.in_ear_check = cfg.enabled;
            }
            build_tlv_snapshot_for_cmd(state, CmdId::WearConfig, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::InEarSensitivity if !val.is_empty() => {
            state.wear_sensitivity = WearSensitivity::from_u8(val[0]);
            build_tlv_snapshot_for_cmd(state, CmdId::InEarSensitivity, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::FitTest => {
            // Emit realistic testing (status=1) then completed (status=2, good seal L/R) frames.
            let testing = encode_single_tlv(CmdId::FitTest.as_u8(), &[1, 0, 0]).ok();
            let done = build_tlv_snapshot_for_cmd(state, CmdId::FitTest, is_new_jl);
            [testing, done].into_iter().flatten().collect()
        }
        CmdId::HearHealth => {
            if let Ok(cfg) = decode_hearing_health(val) {
                state.hearing_health = cfg;
            }
            build_tlv_snapshot_for_cmd(state, CmdId::HearHealth, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::PowerManager if val.len() >= 2 => {
            let raw = u16::from_le_bytes([val[0], val[1]]);
            state.power_manager = PowerManagerState {
                scheduled_minutes: if raw == 0 {
                    PowerManagerState::DISABLED
                } else {
                    raw
                },
                remaining_minutes: if raw == PowerManagerState::DISABLED {
                    0
                } else {
                    raw
                },
            };
            build_tlv_snapshot_for_cmd(state, CmdId::PowerManager, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::Standby if val.len() >= 2 => {
            state.standby_minutes = u16::from_le_bytes([val[0], val[1]]);
            build_tlv_snapshot_for_cmd(state, CmdId::Standby, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::VoiceName => {
            state.prompt_voice_code = String::from_utf8_lossy(val).into_owned();
            build_tlv_snapshot_for_cmd(state, CmdId::VoiceName, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::VoiceLanguage if !val.is_empty() => {
            state.prompt_language_index = val[0];
            build_tlv_snapshot_for_cmd(state, CmdId::VoiceLanguage, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::ToneVolume if !val.is_empty() => {
            state.prompt_tone_volume.current = val[0];
            build_tlv_snapshot_for_cmd(state, CmdId::ToneVolume, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::TwsEnable if !val.is_empty() => {
            state.tws_enable = decode_qcy_bool(val[0]);
            build_tlv_snapshot_for_cmd(state, CmdId::TwsEnable, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::LedSwitch if !val.is_empty() => {
            state.led_switch = decode_qcy_bool(val[0]);
            build_tlv_snapshot_for_cmd(state, CmdId::LedSwitch, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::LedEffect => {
            if let Ok(cfg) = decode_led_effect(val) {
                state.led_effect = cfg;
            }
            build_tlv_snapshot_for_cmd(state, CmdId::LedEffect, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::FocusMode if !val.is_empty() => {
            state.focus_mode = decode_qcy_bool(val[0]);
            build_tlv_snapshot_for_cmd(state, CmdId::FocusMode, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::MusicAction => {
            state.white_noise_playing = !state.white_noise_playing;
            build_tlv_snapshot_for_cmd(state, CmdId::MusicState, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::Volume if !val.is_empty() => {
            state.white_noise_volume = val[0];
            build_tlv_snapshot_for_cmd(state, CmdId::Volume, is_new_jl)
                .into_iter()
                .collect()
        }
        CmdId::TonePlay if !val.is_empty() => {
            state.white_noise_preview = decode_qcy_bool(val[0]);
            Vec::new()
        }
        CmdId::SyncTime if val.len() >= 7 => {
            state.synced_time = Some(DeviceDateTime {
                year_mod_100: val[0],
                month: val[1],
                day: val[2],
                hour: val[3],
                minute: val[4],
                second: val[5],
                weekday_mask: val[6],
            });
            Vec::new()
        }
        CmdId::Alarm if val.len() >= 7 => {
            let op = val[0];
            let alarm = DeviceAlarm {
                alarm_id: val[1],
                enabled: val[2] == 1,
                hour: val[3],
                minute: val[4],
                cycle_mask: val[5],
            };
            match op {
                1 => state.alarms.push(alarm),
                2 => state.alarms.retain(|a| a.alarm_id != alarm.alarm_id),
                3 => {
                    if let Some(existing) = state
                        .alarms
                        .iter_mut()
                        .find(|a| a.alarm_id == alarm.alarm_id)
                    {
                        *existing = alarm;
                    } else {
                        state.alarms.push(alarm);
                    }
                }
                _ => {}
            }
            build_tlv_snapshot_for_cmd(state, CmdId::Alarm, is_new_jl)
                .into_iter()
                .collect()
        }
        _ => Vec::new(),
    }
}

/// Processes raw JieLi RCSP bytes written to `0xA002 / 0x0001`, supporting BOTH:
/// - `isNewJL` (`OP_EXTRA_CUSTOM = 0xFF`) tunneling QCY `[0xFF, ...]` TLVs
/// - Legacy JieLi native RCSP (`0x03`, `0x07`, `0x08`, `0xC0`, `0xC1`)
/// - JieLi unauthenticated BLE OTA (`0xE1`–`0xE8`)
pub fn handle_jieli_rcsp_write(
    state: &mut VirtualDeviceState,
    acc: &mut RcspStreamAccumulator,
    raw_bytes: &[u8],
) -> Vec<Vec<u8>> {
    let frames = acc.push(raw_bytes);
    let mut replies = Vec::new();

    for frame in frames {
        let sn = frame.sn().unwrap_or(0);
        match frame.opcode {
            OP_EXTRA_CUSTOM => {
                if let Some(inner_tlv) = unwrap_qcy_tlv_from_rcsp(&frame) {
                    let tlv_replies = handle_qcy_tlv_write(state, inner_tlv, true);
                    for tlv_pkt in tlv_replies {
                        if let Ok(wrapped) =
                            RcspFrame::response(OP_EXTRA_CUSTOM, 0x00, sn, &tlv_pkt).encode()
                        {
                            replies.push(wrapped);
                        }
                    }
                }
            }
            OP_GET_TARGET_INFO => {
                // Pack major.minor.patch into 16-bit JieLi version field (attr 0x00)
                let packed = (((state.firmware_version.major as u16) & 0x0F) << 12)
                    | (((state.firmware_version.minor as u16) & 0x0F) << 8)
                    | (((state.firmware_version.patch as u16) & 0x0F) << 4);
                let v = packed.to_be_bytes();
                let body = [0x03, 0x00, v[0], v[1]];
                if let Ok(rsp) = RcspFrame::response(OP_GET_TARGET_INFO, 0x00, sn, &body).encode() {
                    replies.push(rsp);
                }
            }
            OP_ADV_GET_INFO => {
                let body = build_jl_adv_info_body(state);
                if let Ok(rsp) = RcspFrame::response(OP_ADV_GET_INFO, 0x00, sn, &body).encode() {
                    replies.push(rsp);
                }
            }
            OP_ADV_MODIFY_INFO => {
                let body = frame.body();
                let mut cursor = 0usize;
                while cursor + 2 <= body.len() {
                    let ltv_len = body[cursor] as usize;
                    if ltv_len == 0 || cursor + 1 + ltv_len > body.len() {
                        break;
                    }
                    let attr = body[cursor + 1];
                    let val = &body[cursor + 2..cursor + 1 + ltv_len];
                    match attr {
                        JL_ADV_ATTR_NAME => {
                            state.device_name = String::from_utf8_lossy(val).into_owned();
                        }
                        JL_ADV_ATTR_KEY_SETTINGS if val.len() >= 3 => {
                            let event_id = combine_jl_button_event(val[0], val[1]);
                            let action = ButtonAction::from_u8(change_jl_action_to_qcy(val[2]));
                            if let Some(slot) = state
                                .button_bindings
                                .iter_mut()
                                .find(|b| b.event_id == event_id)
                            {
                                slot.action = action;
                            } else {
                                state
                                    .button_bindings
                                    .push(ButtonBinding { event_id, action });
                            }
                        }
                        JL_ADV_ATTR_WORK_MODE if !val.is_empty() => {
                            state.gaming_mode = val[0] == 0x02;
                        }
                        _ => {}
                    }
                    cursor += 1 + ltv_len;
                }
                let adv_body = build_jl_adv_info_body(state);
                if let Ok(rsp) = RcspFrame::response(OP_ADV_GET_INFO, 0x00, sn, &adv_body).encode()
                {
                    replies.push(rsp);
                }
            }
            OP_SET_SYS_INFO | OP_GET_SYS_INFO => {
                let body = frame.body();
                let mut cursor = usize::from(!body.is_empty());
                while cursor + 2 <= body.len() {
                    let ltv_len = body[cursor] as usize;
                    if ltv_len == 0 || cursor + 1 + ltv_len > body.len() {
                        break;
                    }
                    let attr = body[cursor + 1];
                    let val = &body[cursor + 2..cursor + 1 + ltv_len];
                    match attr {
                        JL_SYS_ATTR_EQ if val.len() >= 2 => {
                            state.graphic_eq.eq_type = val[0] & 0x7F;
                            let gains_slice = if (val[0] & 0x80) != 0 && val.len() >= 2 {
                                let cnt = val[1] as usize;
                                &val[2..val.len().min(2 + cnt)]
                            } else {
                                &val[1..]
                            };
                            state.graphic_eq.gains_db =
                                gains_slice.iter().map(|&b| b as i8).collect();
                        }
                        JL_SYS_ATTR_NOISE_MODE if !val.is_empty() => {
                            state.legacy_noise_mode = val[0];
                        }
                        _ => {}
                    }
                    cursor += 1 + ltv_len;
                }
                // Echo updated EQ + noise mode via OP_GET_SYS_INFO response
                let mut rsp_body = vec![JL_SYS_FUNC_PUBLIC];
                let gains_len = state.graphic_eq.gains_db.len().min(10);
                rsp_body.push((3 + gains_len) as u8);
                rsp_body.push(JL_SYS_ATTR_EQ);
                rsp_body.push(state.graphic_eq.eq_type | 0x80);
                rsp_body.push(gains_len as u8);
                for &g in state.graphic_eq.gains_db.iter().take(gains_len) {
                    rsp_body.push(g as u8);
                }
                rsp_body.extend_from_slice(&[
                    0x02,
                    JL_SYS_ATTR_NOISE_MODE,
                    state.legacy_noise_mode,
                ]);
                if let Ok(rsp) = RcspFrame::response(OP_GET_SYS_INFO, 0x00, sn, &rsp_body).encode()
                {
                    replies.push(rsp);
                }
            }
            // JieLi BLE OTA simulation (otaType = 2)
            OP_OTA_GET_FILE_INFO_OFFSET => {
                state.ota_packets_received += 1;
                if let Ok(rsp) =
                    RcspFrame::response(OP_OTA_GET_FILE_INFO_OFFSET, 0x00, sn, &[0, 0, 0, 0, 0, 0])
                        .encode()
                {
                    replies.push(rsp);
                }
            }
            OP_OTA_INQUIRE_CAN_UPDATE
            | OP_OTA_ENTER_UPDATE
            | OP_OTA_NOTIFY_FILE_SIZE
            | OP_OTA_REBOOT
            | OP_OTA_EXIT_UPDATE => {
                state.ota_packets_received += 1;
                if let Ok(rsp) = RcspFrame::response(frame.opcode, 0x00, sn, &[0x00]).encode() {
                    replies.push(rsp);
                }
            }
            OP_OTA_SEND_BLOCK => {
                state.ota_packets_received += 1;
                state.ota_bytes_received += frame.body().len();
                if let Ok(rsp) = RcspFrame::response(OP_OTA_SEND_BLOCK, 0x00, sn, &[0x00]).encode()
                {
                    replies.push(rsp);
                }
            }
            _ => {}
        }
    }

    replies
}

fn build_jl_adv_info_body(state: &VirtualDeviceState) -> Vec<u8> {
    let mut body = Vec::new();
    // Battery LTV
    let batt = state.battery_wire_bytes();
    body.extend_from_slice(&[0x04, JL_ADV_ATTR_BATTERY, batt[0], batt[1], batt[2]]);
    // Device name LTV
    let name_bytes = state.device_name.as_bytes();
    let capped = &name_bytes[..name_bytes.len().min(32)];
    body.push((1 + capped.len()) as u8);
    body.push(JL_ADV_ATTR_NAME);
    body.extend_from_slice(capped);
    // Key bindings LTV (with 0 -> 0x7F JieLi translation)
    if !state.button_bindings.is_empty() {
        let mut keys = Vec::with_capacity(state.button_bindings.len() * 3);
        for b in &state.button_bindings {
            let (key_idx, gesture) = split_jl_button_event(b.event_id);
            keys.push(key_idx);
            keys.push(gesture);
            keys.push(change_qcy_action_to_jl(b.action.as_u8()));
        }
        body.push((1 + keys.len()) as u8);
        body.push(JL_ADV_ATTR_KEY_SETTINGS);
        body.extend_from_slice(&keys);
    }
    // Work mode LTV
    body.extend_from_slice(&[
        0x02,
        JL_ADV_ATTR_WORK_MODE,
        if state.gaming_mode { 0x02 } else { 0x01 },
    ]);
    body
}

/// Handles legacy direct GATT reads (`0x0007`, `0x0008`, `0x000B`, `0x000D`).
pub fn handle_legacy_direct_read(state: &VirtualDeviceState, char_uuid: Uuid) -> Option<Vec<u8>> {
    if char_uuid == CHAR_LEGACY_VERSION {
        return Some(vec![
            state.firmware_version.major,
            state.firmware_version.minor,
            state.firmware_version.patch,
        ]);
    }
    if char_uuid == CHAR_LEGACY_BATTERY {
        return Some(state.battery_wire_bytes().to_vec());
    }
    if char_uuid == CHAR_LEGACY_EQ {
        return Some(encode_graphic_eq(&state.graphic_eq));
    }
    if char_uuid == CHAR_LEGACY_BUTTON {
        return Some(
            state
                .button_bindings
                .iter()
                .flat_map(|b| [(b.event_id & 0xFF) as u8, b.action.as_u8()])
                .collect(),
        );
    }
    None
}

/// Handles legacy direct GATT writes (`0x000B` EQ, `0x000D` Buttons).
pub fn handle_legacy_direct_write(
    state: &mut VirtualDeviceState,
    char_uuid: Uuid,
    data: &[u8],
) -> Vec<Vec<u8>> {
    if char_uuid == CHAR_LEGACY_EQ {
        if let Ok(eq) = decode_graphic_eq(data) {
            state.graphic_eq = eq;
            return vec![encode_graphic_eq(&state.graphic_eq)];
        }
    } else if char_uuid == CHAR_LEGACY_BUTTON && data.len() >= 2 {
        if data[0] == 0xFF && data[1] == 0xFF {
            if let Ok(fresh) = VirtualDeviceState::for_model(&state.model_id) {
                state.button_bindings = fresh.button_bindings;
            }
        } else {
            let event_id = data[0] as u16;
            let action = ButtonAction::from_u8(data[1]);
            if let Some(slot) = state
                .button_bindings
                .iter_mut()
                .find(|b| b.event_id == event_id)
            {
                slot.action = action;
            } else {
                state
                    .button_bindings
                    .push(ButtonBinding { event_id, action });
            }
        }
        return vec![handle_legacy_direct_read(state, CHAR_LEGACY_BUTTON).unwrap_or_default()];
    }
    Vec::new()
}

/// Handles vendor BLE/RFCOMM OTA packets and produces realistic vendor ACK notifications.
pub fn handle_ota_write(
    state: &mut VirtualDeviceState,
    char_uuid: Option<Uuid>,
    data: &[u8],
) -> Vec<Vec<u8>> {
    state.ota_packets_received += 1;
    state.ota_bytes_received += data.len();

    if char_uuid == Some(CHAR_WUQI_OTA_WRITE) && data.len() >= 5 && data[0..3] == WUQI_BLE_REQ_MAGIC
    {
        let seq = data[3];
        let cmd = data[4];
        return vec![vec![WUQI_BLE_RSP_MAGIC, seq, cmd, 0x00]];
    }
    if char_uuid == Some(CHAR_BLUETRUM_FOTA_WRITE) {
        if data == BLUETRUM_FOTA_MAGIC {
            return vec![vec![0x00, 0x91, 0x00]];
        }
        if data.len() >= 2 {
            return vec![vec![data[0], data[1], 0x00]];
        }
    }
    if char_uuid == Some(CHAR_BES_OTA) {
        if data.len() >= 5 && data[1..5] == BES_OTA_MAGIC {
            return vec![vec![0x81, 0x00]];
        }
        return vec![vec![0x11, 0x22]]; // BES segment pass ACK
    }
    // RFCOMM routes: PixArt ("PA" -> "PI"), Qualcomm GAIA, Actions
    if data.len() >= 4 && data[0..2] == PIXART_TX_MAGIC {
        return vec![vec![
            PIXART_RX_MAGIC[0],
            PIXART_RX_MAGIC[1],
            data[2],
            data[3],
            0x00,
        ]];
    }
    vec![vec![0x00]]
}
