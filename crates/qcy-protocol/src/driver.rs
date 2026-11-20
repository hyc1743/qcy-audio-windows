//! Unified `ProtocolDriver` trait, concrete chipset drivers (`QcyUnifiedTlvDriver`,
//! `JieLiNewDriver`, `JieLiLegacyDriver`, `LegacyDirectGattDriver`), and `ProtocolRouter`.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;

use uuid::Uuid;

use crate::adv::{is_jl_device, is_new_jl, is_zr_device, supports_multi_eq, BatteryLevel};
use crate::rcsp::{
    decode_jl_legacy_frame, encode_jl_get_adv_info, encode_jl_get_target_info, encode_jl_rename,
    encode_jl_set_button, encode_jl_set_eq, encode_jl_set_noise_mode, encode_jl_set_work_mode,
    unwrap_qcy_tlv_from_rcsp, wrap_qcy_tlv_in_rcsp, RcspStreamAccumulator,
};
use crate::tlv::{
    decode_graphic_eq, decode_tlv_frame, decode_tlv_item, encode_ai_anc_matrix_entry,
    encode_ai_anc_preview, encode_ai_anc_toggle, encode_alarm_command, encode_anc_setting,
    encode_btn_anc_cycle, encode_graphic_eq, encode_hearing_health, encode_led_effect,
    encode_multipoint_host_action, encode_parametric_eq_v1, encode_parametric_eq_v2,
    encode_power_manager, encode_qcy_bool, encode_read_request, encode_side_parametric_eq,
    encode_single_tlv, encode_sync_time, encode_wear_config, TlvStreamReassembler,
};
use crate::types::{
    AncMode, AncSetting, ButtonAction, ButtonBinding, DeviceCommand, DeviceEvent, EarSide,
    EqDialect, FirmwareVersion, GraphicEqSetting, ParametricEqBand, ParametricEqPreset,
    ProtocolFamily, WireWrite, DEFAULT_EQ_FREQS_10, DEFAULT_EQ_FREQS_5_ZR,
};
use crate::uuids::{
    CmdId, CHAR_JIELI_NOTIFY, CHAR_JIELI_WRITE, CHAR_LEGACY_BATTERY, CHAR_LEGACY_BUTTON,
    CHAR_LEGACY_EQ, CHAR_LEGACY_VERSION, CHAR_QCY_NOTIFY, CHAR_QCY_WRITE, DEFAULT_BLE_MTU,
    SVC_JIELI_RCSP, SVC_QCY_CUSTOM,
};
use crate::{ProtocolError, ProtocolResult};

/// Unified clean-room driver trait abstracting all QCY daily-control BLE protocols.
///
/// Preserves the legacy `encode`/`decode` signatures alongside the full GATT-routed
/// `encode_command` / `decode_notification` API.
pub trait ProtocolDriver: Send + Sync {
    /// Returns the concrete protocol family implemented by this driver.
    fn family(&self) -> ProtocolFamily;

    /// Returns `(service_uuid, notify_characteristic_uuid)` pairs that must be subscribed
    /// after connecting.
    fn required_notify_characteristics(&self) -> Vec<(Uuid, Uuid)> {
        vec![(SVC_QCY_CUSTOM, CHAR_QCY_NOTIFY)]
    }

    /// Encodes a high-level `DeviceCommand` into one or more GATT wire operations
    /// targeting exact services/characteristics with pacing metadata.
    fn encode_command(&self, command: &DeviceCommand) -> ProtocolResult<Vec<WireWrite>>;

    /// Decodes an incoming GATT notification or direct characteristic read into
    /// zero or more typed `DeviceEvent`s.
    fn decode_notification(
        &self,
        char_uuid: Uuid,
        bytes: &[u8],
    ) -> ProtocolResult<Vec<DeviceEvent>>;

    /// Generates the post-connect initial state hydration requests for the supplied
    /// model capability `CmdId`s.
    fn initial_hydration_commands(&self, cmd_ids: &[CmdId]) -> ProtocolResult<Vec<WireWrite>> {
        let mut writes = Vec::new();
        for &cmd in cmd_ids {
            if cmd.is_ui_only() || cmd == CmdId::RequestData {
                continue;
            }
            writes.extend(self.encode_command(&DeviceCommand::RequestAttribute { cmd })?);
        }
        Ok(writes)
    }

    /// Legacy convenience wrapper returning the first encoded payload bytes.
    fn encode(&self, command: &DeviceCommand, _sequence: u8) -> ProtocolResult<Vec<u8>> {
        let writes = self.encode_command(command)?;
        Ok(writes
            .into_iter()
            .next()
            .map(|w| w.payload)
            .unwrap_or_default())
    }

    /// Legacy convenience wrapper decoding a notification payload on the driver's primary notify char.
    fn decode(&self, packet: &[u8]) -> ProtocolResult<Option<DeviceEvent>> {
        let notify_char = self
            .required_notify_characteristics()
            .first()
            .map(|(_, c)| *c)
            .unwrap_or(CHAR_QCY_NOTIFY);
        Ok(self
            .decode_notification(notify_char, packet)?
            .into_iter()
            .next())
    }
}

// ---------------------------------------------------------------------------
// Shared QCY TLV Command Encoder (Used by QcyUnifiedTlv, JieLiNew & LegacyDirectGatt)
// ---------------------------------------------------------------------------

fn encode_tlv_command_frames(
    command: &DeviceCommand,
    eq_dialect: EqDialect,
    mtu: usize,
    is_new_jl_quirk: bool,
) -> ProtocolResult<Vec<Vec<u8>>> {
    match command {
        DeviceCommand::FindEarbuds { .. } => {
            // 0x4B is strictly UI-only; Find Earbuds uses local A2DP stereo chime playback.
            Ok(Vec::new())
        }
        DeviceCommand::RequestDeviceInfo => Ok(vec![
            encode_read_request(CmdId::Version.as_u8()),
            encode_read_request(CmdId::Battery.as_u8()),
            encode_read_request(CmdId::AncSetting.as_u8()),
        ]),
        DeviceCommand::RequestBattery => Ok(vec![encode_read_request(CmdId::Battery.as_u8())]),
        DeviceCommand::RequestAttribute { cmd } => {
            if cmd.is_ui_only() {
                return Ok(Vec::new());
            }
            Ok(vec![encode_read_request(cmd.as_u8())])
        }
        DeviceCommand::RawTlv { cmd, value } => Ok(vec![encode_single_tlv(*cmd, value)?]),

        DeviceCommand::ResetDefault => {
            Ok(vec![encode_single_tlv(CmdId::ResetDefault.as_u8(), &[])?])
        }
        DeviceCommand::ClearPairing => Ok(vec![encode_single_tlv(CmdId::ClearPair.as_u8(), &[])?]),
        DeviceCommand::FactoryReset => {
            Ok(vec![encode_single_tlv(CmdId::FactoryReset.as_u8(), &[])?])
        }
        DeviceCommand::SetDeviceName { name } => {
            let bytes = name.as_bytes();
            let capped = &bytes[..bytes.len().min(32)];
            Ok(vec![encode_single_tlv(CmdId::PairName.as_u8(), capped)?])
        }
        DeviceCommand::SetDeviceColor { color_index } => Ok(vec![encode_single_tlv(
            CmdId::DeviceColor.as_u8(),
            &[*color_index],
        )?]),

        DeviceCommand::SetAncMode { mode, depth } => {
            let setting = AncSetting::from_mode(*mode, *depth);
            Ok(vec![encode_single_tlv(
                CmdId::AncSetting.as_u8(),
                &encode_anc_setting(setting),
            )?])
        }
        DeviceCommand::SetAncSetting(setting) => Ok(vec![encode_single_tlv(
            CmdId::AncSetting.as_u8(),
            &encode_anc_setting(*setting),
        )?]),
        DeviceCommand::SetLegacyNoiseMode { mode, .. } => {
            Ok(vec![encode_single_tlv(CmdId::NoiseMode.as_u8(), &[*mode])?])
        }
        DeviceCommand::SetWindNoise { enabled } => Ok(vec![encode_single_tlv(
            CmdId::WindNoise.as_u8(),
            &[u8::from(*enabled)],
        )?]),
        DeviceCommand::SetButtonAncCycle { modes } => Ok(vec![encode_single_tlv(
            CmdId::BtnAncCfg.as_u8(),
            &encode_btn_anc_cycle(modes),
        )?]),
        DeviceCommand::ResetButtonAncCycle => Ok(vec![encode_single_tlv(
            CmdId::BtnAncCfg.as_u8(),
            &[0xFF, 0xFF, 0xFF],
        )?]),
        DeviceCommand::SetAiAncEnabled { enabled } => Ok(vec![encode_single_tlv(
            CmdId::AiAnc.as_u8(),
            &encode_ai_anc_toggle(*enabled),
        )?]),
        DeviceCommand::SetAiAncMatrixCell(entry) => Ok(vec![encode_single_tlv(
            CmdId::AiAncCfg2.as_u8(),
            &encode_ai_anc_matrix_entry(*entry),
        )?]),
        DeviceCommand::AiAncPreview(action) => Ok(vec![encode_single_tlv(
            CmdId::AiAncPreview.as_u8(),
            &encode_ai_anc_preview(*action),
        )?]),

        DeviceCommand::SetEqBands { gains_db } => {
            let freqs: &[u16] = match eq_dialect {
                EqDialect::LegacyGraphic5Zr => &DEFAULT_EQ_FREQS_5_ZR,
                _ => &DEFAULT_EQ_FREQS_10,
            };
            match eq_dialect {
                EqDialect::LegacyGraphic10 | EqDialect::LegacyGraphic5Zr => {
                    let setting = GraphicEqSetting {
                        eq_type: 0,
                        gains_db: gains_db
                            .iter()
                            .take(freqs.len())
                            .map(|g| g.round().clamp(-12.0, 12.0) as i8)
                            .collect(),
                        game_eq_type: None,
                        current_mode: None,
                    };
                    Ok(vec![encode_single_tlv(
                        CmdId::GainEq.as_u8(),
                        &encode_graphic_eq(&setting),
                    )?])
                }
                EqDialect::MultiEqV1 => {
                    let bands = gains_db
                        .iter()
                        .zip(freqs.iter())
                        .map(|(&g, &f)| ParametricEqBand::peak(f, g, 1.41))
                        .collect();
                    let preset = ParametricEqPreset {
                        eq_type: 0,
                        master_gain_db: 0.0,
                        bands,
                    };
                    Ok(vec![encode_single_tlv(
                        CmdId::MultiEq.as_u8(),
                        &encode_parametric_eq_v1(&preset),
                    )?])
                }
                EqDialect::MultiEqV2 => {
                    let bands = gains_db
                        .iter()
                        .zip(freqs.iter())
                        .map(|(&g, &f)| ParametricEqBand::peak(f, g, 1.41))
                        .collect();
                    let preset = ParametricEqPreset {
                        eq_type: 0,
                        master_gain_db: 0.0,
                        bands,
                    };
                    Ok(vec![encode_single_tlv(
                        CmdId::MultiEq2.as_u8(),
                        &encode_parametric_eq_v2(&preset),
                    )?])
                }
            }
        }
        DeviceCommand::SetGraphicEq(eq) => Ok(vec![encode_single_tlv(
            CmdId::GainEq.as_u8(),
            &encode_graphic_eq(eq),
        )?]),
        DeviceCommand::SetParametricEqV1(preset) => Ok(vec![encode_single_tlv(
            CmdId::MultiEq.as_u8(),
            &encode_parametric_eq_v1(preset),
        )?]),
        DeviceCommand::SetParametricEqV2(preset) => Ok(vec![encode_single_tlv(
            CmdId::MultiEq2.as_u8(),
            &encode_parametric_eq_v2(preset),
        )?]),
        DeviceCommand::SetSideParametricEq {
            side,
            preset,
            max_bands,
        } => {
            let cmd = match side {
                EarSide::Left => CmdId::MultiEqLeft,
                EarSide::Right => CmdId::MultiEqRight,
            };
            let payload = encode_side_parametric_eq(preset, mtu, *max_bands);
            Ok(vec![encode_single_tlv(cmd.as_u8(), &payload)?])
        }
        DeviceCommand::SetCustomEqTestMode { enabled } => Ok(vec![encode_single_tlv(
            CmdId::CustomEqTest.as_u8(),
            &[u8::from(*enabled)],
        )?]),
        DeviceCommand::SetDynamicEq { enabled } => Ok(vec![encode_single_tlv(
            CmdId::DynamicEq.as_u8(),
            &[u8::from(*enabled)],
        )?]),
        DeviceCommand::SetBassEnhancement { enabled } => Ok(vec![encode_single_tlv(
            CmdId::BassEnhancement.as_u8(),
            &[u8::from(*enabled)],
        )?]),
        DeviceCommand::SetThxEq { enabled } => Ok(vec![encode_single_tlv(
            CmdId::ThxEq.as_u8(),
            &[u8::from(*enabled)],
        )?]),
        DeviceCommand::SetAdaptiveVolume { enabled } => Ok(vec![encode_single_tlv(
            CmdId::AdaptVolume.as_u8(),
            &[u8::from(*enabled)],
        )?]),
        DeviceCommand::SetChannelBalance { balance } => Ok(vec![encode_single_tlv(
            CmdId::ChannelBalance.as_u8(),
            &[(*balance).min(100)],
        )?]),

        DeviceCommand::SetButtonBinding(binding) => Ok(vec![encode_single_tlv(
            CmdId::ButtonFunc.as_u8(),
            &[(binding.event_id & 0xFF) as u8, binding.action.as_u8()],
        )?]),
        DeviceCommand::ResetButtonBindings => Ok(vec![encode_single_tlv(
            CmdId::ButtonFunc.as_u8(),
            &[0xFF, 0xFF],
        )?]),
        DeviceCommand::SetSleepMode { enabled } => Ok(vec![encode_single_tlv(
            CmdId::SleepMode.as_u8(),
            &[u8::from(*enabled)],
        )?]),
        DeviceCommand::SetTouchTone { enabled } => Ok(vec![encode_single_tlv(
            CmdId::TouchTone.as_u8(),
            &[u8::from(*enabled)],
        )?]),
        DeviceCommand::SetTouchSensitivity { sensitivity } => Ok(vec![encode_single_tlv(
            CmdId::TouchSensitivity.as_u8(),
            &[sensitivity.as_u8()],
        )?]),
        DeviceCommand::SetHeadGestureEnabled { enabled } => Ok(vec![encode_single_tlv(
            CmdId::HeadAction.as_u8(),
            &[u8::from(*enabled)],
        )?]),
        DeviceCommand::SetHeadGestureBinding(binding) => Ok(vec![encode_single_tlv(
            CmdId::SensorConfig.as_u8(),
            &[binding.event.as_u8(), binding.action.as_u8()],
        )?]),
        DeviceCommand::SetHeadGestureTestMode { enabled } => Ok(vec![encode_single_tlv(
            CmdId::SensorTest.as_u8(),
            &[if *enabled { 0xFA } else { 0xFB }],
        )?]),

        DeviceCommand::SetGamingMode { enabled } => Ok(vec![encode_single_tlv(
            CmdId::GameMode.as_u8(),
            &[encode_qcy_bool(*enabled, is_new_jl_quirk)],
        )?]),
        DeviceCommand::SetGameModePro(cfg) => Ok(vec![encode_single_tlv(
            CmdId::GameConfig.as_u8(),
            &[u8::from(cfg.enabled), u8::from(cfg.follow_game_sound)],
        )?]),
        DeviceCommand::SetListeningMode { mode } => Ok(vec![encode_single_tlv(
            CmdId::MusicMode.as_u8(),
            &[mode.as_u8()],
        )?]),
        DeviceCommand::SetSpatialSound { enabled } => Ok(vec![encode_single_tlv(
            CmdId::SpaceSound.as_u8(),
            &[u8::from(*enabled)],
        )?]),
        DeviceCommand::SetSpatialAudioMode { mode } => Ok(vec![encode_single_tlv(
            CmdId::SpaceAudioMode.as_u8(),
            &[mode.as_u8()],
        )?]),
        DeviceCommand::SetLdac { enabled } => Ok(vec![encode_single_tlv(
            CmdId::Ldac.as_u8(),
            &[u8::from(*enabled)],
        )?]),
        DeviceCommand::SetAptx { enabled } => Ok(vec![encode_single_tlv(
            CmdId::Aptx.as_u8(),
            &[u8::from(*enabled)],
        )?]),
        DeviceCommand::SetLeAudio { enabled } => Ok(vec![encode_single_tlv(
            CmdId::LeAudio.as_u8(),
            &[u8::from(*enabled)],
        )?]),
        DeviceCommand::SetPrivacyModePro(cfg) => Ok(vec![encode_single_tlv(
            CmdId::PrivacyMode.as_u8(),
            &[u8::from(cfg.enabled), u8::from(cfg.call_only)],
        )?]),
        DeviceCommand::SetDialoguePerception { enabled } => Ok(vec![encode_single_tlv(
            CmdId::TalkCheck.as_u8(),
            &[u8::from(*enabled)],
        )?]),
        DeviceCommand::SetVoiceDetectionDelay { seconds } => Ok(vec![encode_single_tlv(
            CmdId::VoiceDetection.as_u8(),
            &[*seconds],
        )?]),
        DeviceCommand::SetEnvironmentAdaptation { enabled } => Ok(vec![encode_single_tlv(
            CmdId::EnvAdaptation.as_u8(),
            &[u8::from(*enabled)],
        )?]),
        DeviceCommand::SetPowerSavingMode { enabled } => Ok(vec![encode_single_tlv(
            CmdId::PowerSavingMode.as_u8(),
            &[u8::from(*enabled)],
        )?]),
        DeviceCommand::SetDisconnectWarning { enabled } => Ok(vec![encode_single_tlv(
            CmdId::DisconnectWarning.as_u8(),
            &[u8::from(*enabled)],
        )?]),

        DeviceCommand::SetDualConnect { enabled } => Ok(vec![encode_single_tlv(
            CmdId::DualConnect.as_u8(),
            &[u8::from(*enabled)],
        )?]),
        DeviceCommand::SetMultipointPairing { enabled } => Ok(vec![encode_single_tlv(
            CmdId::MultiPair.as_u8(),
            &[u8::from(*enabled)],
        )?]),
        DeviceCommand::ControlMultipointHost { action, mac } => Ok(vec![encode_single_tlv(
            CmdId::PhoneStatus.as_u8(),
            &encode_multipoint_host_action(*action, *mac),
        )?]),

        DeviceCommand::SetInEarCheck { enabled } => Ok(vec![encode_single_tlv(
            CmdId::InEarCheck.as_u8(),
            &[encode_qcy_bool(*enabled, is_new_jl_quirk)],
        )?]),
        DeviceCommand::SetWearConfig(cfg) => Ok(vec![encode_single_tlv(
            CmdId::WearConfig.as_u8(),
            &encode_wear_config(*cfg),
        )?]),
        DeviceCommand::SetWearSensitivity { sensitivity } => Ok(vec![encode_single_tlv(
            CmdId::InEarSensitivity.as_u8(),
            &[sensitivity.as_u8()],
        )?]),
        DeviceCommand::StartFitTest => Ok(vec![encode_single_tlv(
            CmdId::FitTest.as_u8(),
            &[0x01, 0x01, 0x01],
        )?]),
        DeviceCommand::SetHearingHealth(cfg) => Ok(vec![encode_single_tlv(
            CmdId::HearHealth.as_u8(),
            &encode_hearing_health(*cfg),
        )?]),

        DeviceCommand::SetAutoPowerOffMinutes { minutes } => Ok(vec![encode_single_tlv(
            CmdId::PowerManager.as_u8(),
            &encode_power_manager(*minutes),
        )?]),
        DeviceCommand::SetStandbyMinutes { minutes } => Ok(vec![encode_single_tlv(
            CmdId::Standby.as_u8(),
            &minutes.to_le_bytes(),
        )?]),
        DeviceCommand::SetPromptVoiceCode { code } => Ok(vec![encode_single_tlv(
            CmdId::VoiceName.as_u8(),
            code.as_bytes(),
        )?]),
        DeviceCommand::SetPromptLanguageIndex { index } => Ok(vec![encode_single_tlv(
            CmdId::VoiceLanguage.as_u8(),
            &[*index],
        )?]),
        DeviceCommand::SetPromptToneVolume { level } => Ok(vec![encode_single_tlv(
            CmdId::ToneVolume.as_u8(),
            &[*level],
        )?]),

        DeviceCommand::SetTwsEnable { enabled } => Ok(vec![encode_single_tlv(
            CmdId::TwsEnable.as_u8(),
            &[u8::from(*enabled)],
        )?]),
        DeviceCommand::SetLedSwitch { enabled } => Ok(vec![encode_single_tlv(
            CmdId::LedSwitch.as_u8(),
            &[u8::from(*enabled)],
        )?]),
        DeviceCommand::SetLedEffect(cfg) => Ok(vec![encode_single_tlv(
            CmdId::LedEffect.as_u8(),
            &encode_led_effect(cfg),
        )?]),
        DeviceCommand::SetFocusMode { enabled } => Ok(vec![encode_single_tlv(
            CmdId::FocusMode.as_u8(),
            &[u8::from(*enabled)],
        )?]),
        DeviceCommand::ToggleWhiteNoisePlayback => Ok(vec![encode_single_tlv(
            CmdId::MusicAction.as_u8(),
            &[0x00],
        )?]),
        DeviceCommand::SetWhiteNoiseVolume { volume } => Ok(vec![encode_single_tlv(
            CmdId::Volume.as_u8(),
            &[*volume, *volume, 0x00],
        )?]),
        DeviceCommand::SetWhiteNoisePreview { enabled } => Ok(vec![encode_single_tlv(
            CmdId::TonePlay.as_u8(),
            &[u8::from(*enabled)],
        )?]),
        DeviceCommand::SyncTime(dt) => Ok(vec![encode_single_tlv(
            CmdId::SyncTime.as_u8(),
            &encode_sync_time(*dt),
        )?]),
        DeviceCommand::ManageAlarm(cmd) => Ok(vec![encode_single_tlv(
            CmdId::Alarm.as_u8(),
            &encode_alarm_command(*cmd),
        )?]),
    }
}

// ---------------------------------------------------------------------------
// Driver 1: QcyUnifiedTlvDriver (Service 0xA001, Chars 0x1001 / 0x1002)
// ---------------------------------------------------------------------------

/// Modern QCY Unified TLV driver over BLE GATT `0xA001` (`0x1001` write / `0x1002` notify).
pub struct QcyUnifiedTlvDriver {
    eq_dialect: EqDialect,
    mtu: usize,
    reassembler: Mutex<TlvStreamReassembler>,
}

impl Default for QcyUnifiedTlvDriver {
    fn default() -> Self {
        Self::new(EqDialect::MultiEqV2, DEFAULT_BLE_MTU)
    }
}

impl QcyUnifiedTlvDriver {
    pub fn new(eq_dialect: EqDialect, mtu: usize) -> Self {
        Self {
            eq_dialect,
            mtu,
            reassembler: Mutex::new(TlvStreamReassembler::new()),
        }
    }
}

impl ProtocolDriver for QcyUnifiedTlvDriver {
    fn family(&self) -> ProtocolFamily {
        ProtocolFamily::QcyUnifiedTlv
    }

    fn required_notify_characteristics(&self) -> Vec<(Uuid, Uuid)> {
        vec![(SVC_QCY_CUSTOM, CHAR_QCY_NOTIFY)]
    }

    fn encode_command(&self, command: &DeviceCommand) -> ProtocolResult<Vec<WireWrite>> {
        let frames = encode_tlv_command_frames(command, self.eq_dialect, self.mtu, false)?;
        Ok(frames
            .into_iter()
            .map(|payload| WireWrite::write(SVC_QCY_CUSTOM, CHAR_QCY_WRITE, payload))
            .collect())
    }

    fn decode_notification(
        &self,
        char_uuid: Uuid,
        bytes: &[u8],
    ) -> ProtocolResult<Vec<DeviceEvent>> {
        if char_uuid == CHAR_LEGACY_VERSION
            || char_uuid == CHAR_LEGACY_BATTERY
            || char_uuid == CHAR_LEGACY_EQ
            || char_uuid == CHAR_LEGACY_BUTTON
        {
            return decode_legacy_direct_char(char_uuid, bytes);
        }

        let mut events = Vec::new();
        let frames = self
            .reassembler
            .lock()
            .expect("tlv reassembler lock")
            .push(bytes);

        for items in frames {
            for item in &items {
                events.extend(decode_tlv_item(item)?);
            }
        }
        Ok(events)
    }
}

// ---------------------------------------------------------------------------
// Driver 2: JieLiNewDriver (Service 0xA002, isNewJL TLV-in-RCSP 0xFF)
// ---------------------------------------------------------------------------

/// Modern JieLi (`modelID & 0x8070 == 0x8070`) tunneling QCY `[0xFF, ...]` TLV
/// frames inside RCSP `CMD_EXTRA_CUSTOM` (`0xFF`) over GATT `0xA002` (`0x0001`/`0x0002`).
///
/// Automatically applies the verified `isNewJL` quirk: `OFF = 0x02` for `0x06` (`InEarCheck`)
/// and `0x09` (`GameMode`).
pub struct JieLiNewDriver {
    eq_dialect: EqDialect,
    mtu: usize,
    seq: AtomicU8,
    accumulator: Mutex<RcspStreamAccumulator>,
}

impl Default for JieLiNewDriver {
    fn default() -> Self {
        Self::new(EqDialect::MultiEqV2, DEFAULT_BLE_MTU)
    }
}

impl JieLiNewDriver {
    pub fn new(eq_dialect: EqDialect, mtu: usize) -> Self {
        Self {
            eq_dialect,
            mtu,
            seq: AtomicU8::new(1),
            accumulator: Mutex::new(RcspStreamAccumulator::new()),
        }
    }

    fn next_sn(&self) -> u8 {
        self.seq.fetch_add(1, Ordering::Relaxed)
    }
}

impl ProtocolDriver for JieLiNewDriver {
    fn family(&self) -> ProtocolFamily {
        ProtocolFamily::JieLiRcspNew
    }

    fn required_notify_characteristics(&self) -> Vec<(Uuid, Uuid)> {
        vec![(SVC_JIELI_RCSP, CHAR_JIELI_NOTIFY)]
    }

    fn encode_command(&self, command: &DeviceCommand) -> ProtocolResult<Vec<WireWrite>> {
        let tlv_frames = encode_tlv_command_frames(command, self.eq_dialect, self.mtu, true)?;
        let mut writes = Vec::with_capacity(tlv_frames.len());
        for tlv in tlv_frames {
            let wrapped = wrap_qcy_tlv_in_rcsp(&tlv, self.next_sn())?;
            writes.push(WireWrite::write(SVC_JIELI_RCSP, CHAR_JIELI_WRITE, wrapped));
        }
        Ok(writes)
    }

    fn decode_notification(
        &self,
        _char_uuid: Uuid,
        bytes: &[u8],
    ) -> ProtocolResult<Vec<DeviceEvent>> {
        let frames = self
            .accumulator
            .lock()
            .expect("rcsp accumulator lock")
            .push(bytes);

        let mut events = Vec::new();
        for frame in &frames {
            if let Some(inner_tlv) = unwrap_qcy_tlv_from_rcsp(frame) {
                for item in decode_tlv_frame(inner_tlv)? {
                    events.extend(decode_tlv_item(&item)?);
                }
            } else {
                events.extend(decode_jl_legacy_frame(frame));
            }
        }
        Ok(events)
    }
}

// ---------------------------------------------------------------------------
// Driver 3: JieLiLegacyDriver (Service 0xA002, Native RCSP 0x03/0x07/0x08/0xC0-C5)
// ---------------------------------------------------------------------------

/// Legacy JieLi (`!isNewJL`) driver speaking native RCSP commands over `0xA002`
/// (`0x0001`/`0x0002`), including `changeQCY2JL(0) = 0x7F` button remapping.
pub struct JieLiLegacyDriver {
    seq: AtomicU8,
    accumulator: Mutex<RcspStreamAccumulator>,
}

impl Default for JieLiLegacyDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl JieLiLegacyDriver {
    pub fn new() -> Self {
        Self {
            seq: AtomicU8::new(1),
            accumulator: Mutex::new(RcspStreamAccumulator::new()),
        }
    }

    fn next_sn(&self) -> u8 {
        self.seq.fetch_add(1, Ordering::Relaxed)
    }
}

impl ProtocolDriver for JieLiLegacyDriver {
    fn family(&self) -> ProtocolFamily {
        ProtocolFamily::JieLiRcspLegacy
    }

    fn required_notify_characteristics(&self) -> Vec<(Uuid, Uuid)> {
        vec![(SVC_JIELI_RCSP, CHAR_JIELI_NOTIFY)]
    }

    fn encode_command(&self, command: &DeviceCommand) -> ProtocolResult<Vec<WireWrite>> {
        let frames: Vec<Vec<u8>> = match command {
            DeviceCommand::FindEarbuds { .. } => Vec::new(),
            DeviceCommand::RequestDeviceInfo => vec![
                encode_jl_get_target_info(self.next_sn())?,
                encode_jl_get_adv_info(self.next_sn())?,
            ],
            DeviceCommand::RequestBattery => vec![encode_jl_get_adv_info(self.next_sn())?],
            DeviceCommand::RequestAttribute { cmd } => match cmd {
                CmdId::Version => vec![encode_jl_get_target_info(self.next_sn())?],
                CmdId::Battery | CmdId::ButtonFunc | CmdId::PairName | CmdId::GameMode => {
                    vec![encode_jl_get_adv_info(self.next_sn())?]
                }
                _ => Vec::new(),
            },
            DeviceCommand::SetDeviceName { name } => {
                vec![encode_jl_rename(name, self.next_sn())?]
            }
            DeviceCommand::SetButtonBinding(binding) => {
                vec![encode_jl_set_button(
                    binding.event_id,
                    binding.action,
                    self.next_sn(),
                )?]
            }
            DeviceCommand::SetGamingMode { enabled } => {
                vec![encode_jl_set_work_mode(*enabled, self.next_sn())?]
            }
            DeviceCommand::SetGraphicEq(eq) => vec![encode_jl_set_eq(eq, self.next_sn())?],
            DeviceCommand::SetEqBands { gains_db } => {
                let eq = GraphicEqSetting {
                    eq_type: 6,
                    gains_db: gains_db
                        .iter()
                        .take(10)
                        .map(|g| g.round().clamp(-12.0, 12.0) as i8)
                        .collect(),
                    game_eq_type: None,
                    current_mode: None,
                };
                vec![encode_jl_set_eq(&eq, self.next_sn())?]
            }
            DeviceCommand::SetAncMode { mode, depth } => {
                let jl_mode = match mode {
                    AncMode::Off => 0,
                    AncMode::NoiseCancellation | AncMode::Adaptive | AncMode::CustomDepth => 1,
                    AncMode::Transparency | AncMode::VocalBoostTransparency => 2,
                    AncMode::WindNoiseReduction => 1,
                };
                let pct = depth.unwrap_or(100).min(100);
                vec![encode_jl_set_noise_mode(
                    jl_mode,
                    pct,
                    0,
                    100,
                    self.next_sn(),
                )?]
            }
            DeviceCommand::SetLegacyNoiseMode { mode, percent } => {
                vec![encode_jl_set_noise_mode(
                    *mode,
                    percent.unwrap_or(100),
                    0,
                    100,
                    self.next_sn(),
                )?]
            }
            _ => return Err(ProtocolError::UnsupportedCommand),
        };

        Ok(frames
            .into_iter()
            .map(|payload| WireWrite::write(SVC_JIELI_RCSP, CHAR_JIELI_WRITE, payload))
            .collect())
    }

    fn decode_notification(
        &self,
        _char_uuid: Uuid,
        bytes: &[u8],
    ) -> ProtocolResult<Vec<DeviceEvent>> {
        let frames = self
            .accumulator
            .lock()
            .expect("rcsp accumulator lock")
            .push(bytes);

        let mut events = Vec::new();
        for frame in &frames {
            events.extend(decode_jl_legacy_frame(frame));
        }
        Ok(events)
    }
}

// ---------------------------------------------------------------------------
// Driver 4: LegacyDirectGattDriver (0xA001 Direct Chars 0x0007/0008/000B/000D)
// ---------------------------------------------------------------------------

/// Decodes raw bytes read from or notified on legacy `0xA001` direct GATT characteristics.
pub fn decode_legacy_direct_char(
    char_uuid: Uuid,
    bytes: &[u8],
) -> ProtocolResult<Vec<DeviceEvent>> {
    if char_uuid == CHAR_LEGACY_VERSION {
        if bytes.len() < 3 {
            return Err(ProtocolError::MalformedPacket(
                "legacy 0x0007 version requires 3 bytes".to_string(),
            ));
        }
        return Ok(vec![DeviceEvent::FirmwareVersion(FirmwareVersion {
            major: bytes[0],
            minor: bytes[1],
            patch: bytes[2],
        })]);
    }
    if char_uuid == CHAR_LEGACY_BATTERY {
        if bytes.len() < 3 {
            return Err(ProtocolError::MalformedPacket(
                "legacy 0x0008 battery requires 3 bytes".to_string(),
            ));
        }
        let l = BatteryLevel::from_wire_byte(bytes[0]);
        let r = BatteryLevel::from_wire_byte(bytes[1]);
        let c = BatteryLevel::from_wire_byte(bytes[2]);
        return Ok(vec![DeviceEvent::BatteryUpdated {
            left: l.percent,
            right: r.percent,
            case: c.percent,
            left_charging: l.charging,
            right_charging: r.charging,
            case_charging: c.charging,
        }]);
    }
    if char_uuid == CHAR_LEGACY_EQ {
        return Ok(vec![DeviceEvent::GraphicEqUpdated(decode_graphic_eq(
            bytes,
        )?)]);
    }
    if char_uuid == CHAR_LEGACY_BUTTON {
        let bindings = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| ButtonBinding {
                event_id: c[0] as u16,
                action: ButtonAction::from_u8(c[1]),
            })
            .collect();
        return Ok(vec![DeviceEvent::ButtonBindingsUpdated { bindings }]);
    }
    Ok(vec![DeviceEvent::RawNotification {
        payload: bytes.to_vec(),
    }])
}

/// Driver for older `0xA001` devices that use direct GATT characteristics
/// (`0x0007` Version, `0x0008` Battery, `0x000B` Graphic EQ, `0x000D` Buttons)
/// alongside TLV `0x1001`/`0x1002` for all remaining features.
pub struct LegacyDirectGattDriver {
    is_zr_5band: bool,
    inner_tlv: QcyUnifiedTlvDriver,
}

impl LegacyDirectGattDriver {
    pub fn new(is_zr_5band: bool, mtu: usize) -> Self {
        let eq_dialect = if is_zr_5band {
            EqDialect::LegacyGraphic5Zr
        } else {
            EqDialect::LegacyGraphic10
        };
        Self {
            is_zr_5band,
            inner_tlv: QcyUnifiedTlvDriver::new(eq_dialect, mtu),
        }
    }
}

impl ProtocolDriver for LegacyDirectGattDriver {
    fn family(&self) -> ProtocolFamily {
        ProtocolFamily::LegacyDirectGatt
    }

    fn required_notify_characteristics(&self) -> Vec<(Uuid, Uuid)> {
        vec![
            (SVC_QCY_CUSTOM, CHAR_QCY_NOTIFY),
            (SVC_QCY_CUSTOM, CHAR_LEGACY_BATTERY),
        ]
    }

    fn encode_command(&self, command: &DeviceCommand) -> ProtocolResult<Vec<WireWrite>> {
        match command {
            DeviceCommand::RequestDeviceInfo => Ok(vec![
                WireWrite::read(SVC_QCY_CUSTOM, CHAR_LEGACY_VERSION),
                WireWrite::read(SVC_QCY_CUSTOM, CHAR_LEGACY_BATTERY),
                WireWrite::read(SVC_QCY_CUSTOM, CHAR_LEGACY_EQ),
                WireWrite::read(SVC_QCY_CUSTOM, CHAR_LEGACY_BUTTON),
            ]),
            DeviceCommand::RequestBattery => {
                Ok(vec![WireWrite::read(SVC_QCY_CUSTOM, CHAR_LEGACY_BATTERY)])
            }
            DeviceCommand::RequestAttribute { cmd } => match cmd {
                CmdId::Version => Ok(vec![WireWrite::read(SVC_QCY_CUSTOM, CHAR_LEGACY_VERSION)]),
                CmdId::Battery => Ok(vec![WireWrite::read(SVC_QCY_CUSTOM, CHAR_LEGACY_BATTERY)]),
                CmdId::GainEq => Ok(vec![WireWrite::read(SVC_QCY_CUSTOM, CHAR_LEGACY_EQ)]),
                CmdId::ButtonFunc => Ok(vec![WireWrite::read(SVC_QCY_CUSTOM, CHAR_LEGACY_BUTTON)]),
                _ => self.inner_tlv.encode_command(command),
            },
            DeviceCommand::SetGraphicEq(eq) => Ok(vec![WireWrite::write(
                SVC_QCY_CUSTOM,
                CHAR_LEGACY_EQ,
                encode_graphic_eq(eq),
            )]),
            DeviceCommand::SetEqBands { gains_db } => {
                let count = if self.is_zr_5band { 5 } else { 10 };
                let eq = GraphicEqSetting {
                    eq_type: 0,
                    gains_db: gains_db
                        .iter()
                        .take(count)
                        .map(|g| g.round().clamp(-12.0, 12.0) as i8)
                        .collect(),
                    game_eq_type: None,
                    current_mode: None,
                };
                Ok(vec![WireWrite::write(
                    SVC_QCY_CUSTOM,
                    CHAR_LEGACY_EQ,
                    encode_graphic_eq(&eq),
                )])
            }
            DeviceCommand::SetButtonBinding(binding) => Ok(vec![WireWrite::write(
                SVC_QCY_CUSTOM,
                CHAR_LEGACY_BUTTON,
                vec![(binding.event_id & 0xFF) as u8, binding.action.as_u8()],
            )]),
            DeviceCommand::ResetButtonBindings => Ok(vec![WireWrite::write(
                SVC_QCY_CUSTOM,
                CHAR_LEGACY_BUTTON,
                vec![0xFF, 0xFF],
            )]),
            _ => self.inner_tlv.encode_command(command),
        }
    }

    fn decode_notification(
        &self,
        char_uuid: Uuid,
        bytes: &[u8],
    ) -> ProtocolResult<Vec<DeviceEvent>> {
        if char_uuid == CHAR_LEGACY_VERSION
            || char_uuid == CHAR_LEGACY_BATTERY
            || char_uuid == CHAR_LEGACY_EQ
            || char_uuid == CHAR_LEGACY_BUTTON
        {
            decode_legacy_direct_char(char_uuid, bytes)
        } else {
            self.inner_tlv.decode_notification(char_uuid, bytes)
        }
    }
}

// ---------------------------------------------------------------------------
// ProtocolRouter
// ---------------------------------------------------------------------------

/// Selects the exact daily-control `ProtocolDriver` matching `QCYDeviceManager$mBleCallback$1`.
pub struct ProtocolRouter;

impl ProtocolRouter {
    /// Selects a driver from `model_id` and discovered GATT service UUIDs.
    pub fn select_driver(model_id: Option<u16>, service_uuids: &[Uuid]) -> Box<dyn ProtocolDriver> {
        Self::select_driver_with_characteristics(model_id, service_uuids, &[], DEFAULT_BLE_MTU)
    }

    /// Selects a driver from `model_id`, discovered GATT service UUIDs, discovered
    /// characteristic UUIDs (to detect legacy `0x000B` graphic-EQ peripherals), and MTU.
    pub fn select_driver_with_characteristics(
        model_id: Option<u16>,
        service_uuids: &[Uuid],
        char_uuids: &[Uuid],
        mtu: usize,
    ) -> Box<dyn ProtocolDriver> {
        let has_jieli_svc = service_uuids.contains(&SVC_JIELI_RCSP);
        let has_legacy_eq = char_uuids.contains(&CHAR_LEGACY_EQ);
        let model = model_id.unwrap_or(0);

        if has_jieli_svc || (model_id.is_some() && is_jl_device(model)) {
            if model_id.is_some() && is_new_jl(model) {
                return Box::new(JieLiNewDriver::new(EqDialect::MultiEqV2, mtu));
            }
            return Box::new(JieLiLegacyDriver::new());
        }

        if has_legacy_eq && !supports_multi_eq(model, true, None) {
            return Box::new(LegacyDirectGattDriver::new(
                model_id.is_some_and(is_zr_device),
                mtu,
            ));
        }

        let eq_dialect = if model_id.is_some_and(is_zr_device) {
            EqDialect::LegacyGraphic5Zr
        } else {
            EqDialect::MultiEqV2
        };
        Box::new(QcyUnifiedTlvDriver::new(eq_dialect, mtu))
    }
}
