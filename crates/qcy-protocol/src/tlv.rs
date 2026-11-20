//! QCY Unified Custom TLV packet framing (`[0xFF, PayloadLen, CmdID, VLen, Value...]`),
//! multi-TLV notification splitting, MTU chunking, stream reassembly, and typed
//! payload codecs for all 88+ active QCY command IDs.

use crate::adv::BatteryLevel;
use crate::types::{
    AiAncActivity, AiAncMatrixEntry, AiAncPreviewCommand, AiAncState, AlarmCommand, AncSetting,
    ButtonAction, ButtonBinding, DeviceAlarm, DeviceDateTime, DeviceEvent, FirmwareVersion,
    FitTestResult, GameModeProConfig, GraphicEqSetting, HeadGestureAction, HeadGestureBinding,
    HeadGestureEvent, HearingHealthConfig, InEarMusicBehavior, IrrFilterType, LedEffectConfig,
    LedEffectKind, ListeningMode, MultipointHostAction, MultipointHostStatus, PairedHostEntry,
    PairedHostList, ParametricEqBand, ParametricEqPreset, PowerManagerState, PrivacyModeProConfig,
    PromptToneVolume, RgbColor, SpatialAudioMode, TouchSensitivity, WearConfig, WearSensitivity,
    WhiteNoisePlayState, WhiteNoiseTrackInfo, WhiteNoiseTrackList,
};
use crate::uuids::CmdId;
use crate::{ProtocolError, ProtocolResult};

/// Start-of-frame header byte for QCY Unified Custom TLV frames (`0xFF`).
pub const TLV_FRAME_HEADER: u8 = 0xFF;

/// Single Type-Length-Value entry inside a `0xFF` unified QCY frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlvItem {
    pub cmd: u8,
    pub value: Vec<u8>,
}

impl TlvItem {
    pub fn new(cmd: u8, value: impl Into<Vec<u8>>) -> Self {
        Self {
            cmd,
            value: value.into(),
        }
    }

    pub fn from_cmd(cmd: CmdId, value: impl Into<Vec<u8>>) -> Self {
        Self::new(cmd.as_u8(), value)
    }

    pub fn cmd_id(&self) -> Option<CmdId> {
        CmdId::from_u8(self.cmd)
    }
}

/// Encodes one or more `TlvItem`s into a single QCY `[0xFF, PayloadLen, ...]` frame.
pub fn encode_tlv_frame(items: &[TlvItem]) -> ProtocolResult<Vec<u8>> {
    if items.is_empty() {
        return Err(ProtocolError::MalformedPacket(
            "cannot encode empty TLV frame".to_string(),
        ));
    }
    let mut payload_len: usize = 0;
    for item in items {
        if item.value.len() > u8::MAX as usize {
            return Err(ProtocolError::MalformedPacket(format!(
                "TLV value for cmd {:#04x} exceeds 255 bytes ({})",
                item.cmd,
                item.value.len()
            )));
        }
        payload_len += 2 + item.value.len();
    }
    if payload_len > u8::MAX as usize {
        return Err(ProtocolError::MalformedPacket(format!(
            "TLV frame payload length exceeds 255 bytes ({payload_len})"
        )));
    }

    let mut frame = Vec::with_capacity(2 + payload_len);
    frame.push(TLV_FRAME_HEADER);
    frame.push(payload_len as u8);
    for item in items {
        frame.push(item.cmd);
        frame.push(item.value.len() as u8);
        frame.extend_from_slice(&item.value);
    }
    Ok(frame)
}

/// Convenience helper encoding a single `(cmd, value)` TLV into a `0xFF` frame.
pub fn encode_single_tlv(cmd: u8, value: &[u8]) -> ProtocolResult<Vec<u8>> {
    encode_tlv_frame(&[TlvItem::new(cmd, value.to_vec())])
}

/// Encodes an attribute read request (`[0xFF, 0x03, 0xFE, 0x01, TargetCmdID]`).
pub fn encode_read_request(target_cmd: u8) -> Vec<u8> {
    vec![
        TLV_FRAME_HEADER,
        0x03,
        CmdId::RequestData.as_u8(),
        0x01,
        target_cmd,
    ]
}

/// Decodes a QCY `0xFF` frame containing one or more concatenated TLV records.
pub fn decode_tlv_frame(frame: &[u8]) -> ProtocolResult<Vec<TlvItem>> {
    if frame.len() < 4 {
        return Err(ProtocolError::MalformedPacket(format!(
            "TLV frame too short: {} bytes",
            frame.len()
        )));
    }
    if frame[0] != TLV_FRAME_HEADER {
        return Err(ProtocolError::MalformedPacket(format!(
            "unexpected TLV header byte: expected 0xFF, got {:#04x}",
            frame[0]
        )));
    }

    let declared_len = frame[1] as usize;
    let body_end = 2 + declared_len;
    if frame.len() < body_end {
        return Err(ProtocolError::MalformedPacket(format!(
            "truncated TLV frame: declared {} payload bytes, have {}",
            declared_len,
            frame.len() - 2
        )));
    }

    let mut items = Vec::new();
    let mut cursor = 2usize;
    while cursor < body_end {
        if cursor + 2 > body_end {
            return Err(ProtocolError::MalformedPacket(format!(
                "truncated TLV entry header at offset {cursor}"
            )));
        }
        let cmd = frame[cursor];
        let vlen = frame[cursor + 1] as usize;
        cursor += 2;
        if cursor + vlen > body_end {
            return Err(ProtocolError::MalformedPacket(format!(
                "truncated TLV value for cmd {cmd:#04x}: need {vlen} bytes, have {}",
                body_end - cursor
            )));
        }
        items.push(TlvItem {
            cmd,
            value: frame[cursor..cursor + vlen].to_vec(),
        });
        cursor += vlen;
    }

    Ok(items)
}

/// Splits an outbound BLE payload into ATT MTU-safe write chunks (`mtu - 3`, minimum `20`).
pub fn chunk_payload_for_mtu(payload: &[u8], mtu: usize) -> Vec<Vec<u8>> {
    let max_chunk = mtu.saturating_sub(3).max(20);
    if payload.is_empty() {
        return Vec::new();
    }
    payload.chunks(max_chunk).map(|c| c.to_vec()).collect()
}

/// Reassembles QCY `0xFF` TLV frames that may arrive fragmented across multiple
/// BLE GATT notifications or coalesced into a single notification buffer.
#[derive(Debug, Default, Clone)]
pub struct TlvStreamReassembler {
    buffer: Vec<u8>,
}

impl TlvStreamReassembler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Feeds raw notification bytes and extracts all complete `0xFF` frames.
    pub fn push(&mut self, chunk: &[u8]) -> Vec<Vec<TlvItem>> {
        self.buffer.extend_from_slice(chunk);
        let mut out = Vec::new();

        loop {
            let Some(start) = self.buffer.iter().position(|&b| b == TLV_FRAME_HEADER) else {
                self.buffer.clear();
                break;
            };
            if start > 0 {
                self.buffer.drain(0..start);
            }
            if self.buffer.len() < 2 {
                break;
            }
            let total_len = 2 + (self.buffer[1] as usize);
            if self.buffer.len() < total_len {
                break;
            }
            let frame: Vec<u8> = self.buffer.drain(0..total_len).collect();
            if let Ok(items) = decode_tlv_frame(&frame) {
                out.push(items);
            }
        }

        out
    }
}

// ---------------------------------------------------------------------------
// Bool Helper (Handles isNewJL OFF=0x02 quirk on 0x06 / 0x09)
// ---------------------------------------------------------------------------

/// Encodes a QCY boolean byte; on `isNewJL`, commands `0x06` and `0x09` use `0x02` for OFF.
pub const fn encode_qcy_bool(enabled: bool, is_new_jl_quirk: bool) -> u8 {
    if enabled {
        0x01
    } else if is_new_jl_quirk {
        0x02
    } else {
        0x00
    }
}

/// Decodes a QCY boolean byte (`0x01` => `true`; `0x00` or `0x02` => `false`).
pub const fn decode_qcy_bool(raw: u8) -> bool {
    raw == 0x01
}

// ---------------------------------------------------------------------------
// Typed Payload Encoders (Value Bytes Only — Wrap via encode_single_tlv)
// ---------------------------------------------------------------------------

pub fn encode_anc_setting(setting: AncSetting) -> [u8; 3] {
    [setting.mode, setting.sub_scene, setting.level]
}

pub fn decode_anc_setting(val: &[u8]) -> ProtocolResult<AncSetting> {
    if val.len() < 3 {
        return Err(ProtocolError::MalformedPacket(format!(
            "ANC setting requires 3 bytes, got {}",
            val.len()
        )));
    }
    Ok(AncSetting::new(val[0], val[1], val[2]))
}

pub fn encode_ai_anc_toggle(enabled: bool) -> [u8; 6] {
    if enabled {
        [0x00, 0x00, 0x00, 0x00, 0x00, 0x01]
    } else {
        [0x04, 0x00, 0x00, 0x00, 0x00, 0x01]
    }
}

pub fn decode_ai_anc_state(val: &[u8]) -> ProtocolResult<AiAncState> {
    if val.len() < 5 {
        return Err(ProtocolError::MalformedPacket(format!(
            "AI-ANC state requires >= 5 bytes, got {}",
            val.len()
        )));
    }
    Ok(AiAncState {
        activity: AiAncActivity::from_u8(val[0]),
        noise_scene: val[1],
        setting: AncSetting::new(val[2], val[3], val[4]),
        touch_index: val.get(5).copied(),
    })
}

pub fn encode_ai_anc_matrix_entry(entry: AiAncMatrixEntry) -> [u8; 5] {
    [
        entry.activity.as_u8(),
        entry.noise_scene,
        entry.setting.mode,
        entry.setting.sub_scene,
        entry.setting.level,
    ]
}

pub fn decode_ai_anc_matrix(val: &[u8]) -> ProtocolResult<Vec<AiAncMatrixEntry>> {
    if val.len() < 5 || !val.len().is_multiple_of(5) {
        return Err(ProtocolError::MalformedPacket(format!(
            "AI-ANC matrix must be a non-empty multiple of 5 bytes, got {}",
            val.len()
        )));
    }
    Ok(val
        .as_chunks::<5>()
        .0
        .iter()
        .map(|c| AiAncMatrixEntry {
            activity: AiAncActivity::from_u8(c[0]),
            noise_scene: c[1],
            setting: AncSetting::new(c[2], c[3], c[4]),
        })
        .collect())
}

pub fn encode_ai_anc_preview(cmd: AiAncPreviewCommand) -> [u8; 4] {
    match cmd {
        AiAncPreviewCommand::Enter => [0x01, 0x00, 0x00, 0x00],
        AiAncPreviewCommand::Preview(s) => [0x02, s.mode, s.sub_scene, s.level],
        AiAncPreviewCommand::Exit => [0x03, 0x00, 0x00, 0x00],
    }
}

pub fn encode_btn_anc_cycle(modes: &[AncSetting]) -> Vec<u8> {
    let mut out = Vec::with_capacity(modes.len() * 3);
    for s in modes {
        out.extend_from_slice(&encode_anc_setting(*s));
    }
    out
}

pub fn decode_btn_anc_cycle(val: &[u8]) -> ProtocolResult<Vec<AncSetting>> {
    if !val.len().is_multiple_of(3) {
        return Err(ProtocolError::MalformedPacket(format!(
            "btnAncCfg must be a multiple of 3 bytes, got {}",
            val.len()
        )));
    }
    Ok(val
        .as_chunks::<3>()
        .0
        .iter()
        .map(|c| AncSetting::new(c[0], c[1], c[2]))
        .collect())
}

// --- EQ Dialect A: Legacy Graphic EQ (0xFC / GATT 0x000B) ---

pub fn encode_graphic_eq(eq: &GraphicEqSetting) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + eq.gains_db.len());
    out.push(eq.eq_type);
    for &g in &eq.gains_db {
        out.push(g as u8);
    }
    out
}

pub fn decode_graphic_eq(val: &[u8]) -> ProtocolResult<GraphicEqSetting> {
    if val.len() < 2 {
        return Err(ProtocolError::MalformedPacket(format!(
            "graphic EQ requires >= 2 bytes, got {}",
            val.len()
        )));
    }
    let eq_type = val[0];
    if val.len() == 13 {
        let gains_db = val[1..11].iter().map(|&b| b as i8).collect();
        Ok(GraphicEqSetting {
            eq_type,
            gains_db,
            game_eq_type: Some(val[11]),
            current_mode: Some(val[12]),
        })
    } else {
        let gains_db = val[1..].iter().map(|&b| b as i8).collect();
        Ok(GraphicEqSetting {
            eq_type,
            gains_db,
            game_eq_type: None,
            current_mode: None,
        })
    }
}

// --- EQ Dialect B: Parametric Multi-EQ v1 (0x20, 6B/band) ---

fn scale_i16_x100(val: f32) -> [u8; 2] {
    let scaled = (val * 100.0)
        .round()
        .clamp(i16::MIN as f32, i16::MAX as f32) as i16;
    scaled.to_le_bytes()
}

pub fn encode_parametric_eq_v1(preset: &ParametricEqPreset) -> Vec<u8> {
    let mut out = Vec::with_capacity(3 + preset.bands.len() * 6);
    out.push(preset.eq_type);
    out.extend_from_slice(&scale_i16_x100(preset.master_gain_db));
    for band in &preset.bands {
        out.extend_from_slice(&band.freq_hz.to_le_bytes());
        out.extend_from_slice(&scale_i16_x100(band.gain_db));
        out.extend_from_slice(&scale_i16_x100(band.q));
    }
    out
}

pub fn decode_parametric_eq_v1(val: &[u8]) -> ProtocolResult<ParametricEqPreset> {
    if val.len() < 3 || !(val.len() - 3).is_multiple_of(6) {
        return Err(ProtocolError::MalformedPacket(format!(
            "Multi-EQ v1 payload must be 3 + 6*N bytes, got {}",
            val.len()
        )));
    }
    let eq_type = val[0];
    let master_gain_db = i16::from_le_bytes([val[1], val[2]]) as f32 / 100.0;
    let mut bands = Vec::new();
    for chunk in val[3..].as_chunks::<6>().0 {
        let freq_hz = u16::from_le_bytes([chunk[0], chunk[1]]);
        let gain_db = i16::from_le_bytes([chunk[2], chunk[3]]) as f32 / 100.0;
        let q = i16::from_le_bytes([chunk[4], chunk[5]]) as f32 / 100.0;
        let band = ParametricEqBand::peak(freq_hz, gain_db, q);
        if !band.is_firmware_dummy_slot() {
            bands.push(band);
        }
    }
    Ok(ParametricEqPreset {
        eq_type,
        master_gain_db,
        bands,
    })
}

// --- EQ Dialect C & D: Parametric Multi-EQ v2 (0x22) & L/R (0x46 / 0x47, 7B/band) ---

pub fn encode_parametric_eq_v2(preset: &ParametricEqPreset) -> Vec<u8> {
    let mut out = Vec::with_capacity(3 + preset.bands.len() * 7);
    out.push(preset.eq_type);
    out.extend_from_slice(&scale_i16_x100(preset.master_gain_db));
    for band in &preset.bands {
        out.extend_from_slice(&band.freq_hz.to_le_bytes());
        out.extend_from_slice(&scale_i16_x100(band.gain_db));
        out.extend_from_slice(&scale_i16_x100(band.q));
        out.push(band.filter_type.as_u8());
    }
    out
}

/// Encodes L/R independent parametric EQ (`0x46`/`0x47`), padding unused slots with
/// `(0, 0.0, 0.0, IrrFilterType::Pad)` up to `min((mtu - 7) / 6, max_bands)` per `QCYDeviceImpl.setEQWtihCID`.
pub fn encode_side_parametric_eq(
    preset: &ParametricEqPreset,
    mtu: usize,
    max_bands: usize,
) -> Vec<u8> {
    let mtu_cap = mtu.saturating_sub(7) / 6;
    let target_slots = mtu_cap.min(max_bands).max(preset.bands.len());
    let mut padded = preset.bands.clone();
    while padded.len() < target_slots {
        padded.push(ParametricEqBand::pad());
    }
    encode_parametric_eq_v2(&ParametricEqPreset {
        eq_type: preset.eq_type,
        master_gain_db: preset.master_gain_db,
        bands: padded,
    })
}

pub fn decode_parametric_eq_v2(val: &[u8]) -> ProtocolResult<ParametricEqPreset> {
    if val.len() < 3 || !(val.len() - 3).is_multiple_of(7) {
        return Err(ProtocolError::MalformedPacket(format!(
            "Multi-EQ v2 payload must be 3 + 7*N bytes, got {}",
            val.len()
        )));
    }
    let eq_type = val[0];
    let master_gain_db = i16::from_le_bytes([val[1], val[2]]) as f32 / 100.0;
    let mut bands = Vec::new();
    for chunk in val[3..].as_chunks::<7>().0 {
        let freq_hz = u16::from_le_bytes([chunk[0], chunk[1]]);
        let gain_db = i16::from_le_bytes([chunk[2], chunk[3]]) as f32 / 100.0;
        let q = i16::from_le_bytes([chunk[4], chunk[5]]) as f32 / 100.0;
        let filter_type = IrrFilterType::from_u8(chunk[6]).unwrap_or(IrrFilterType::Peak);
        let band = ParametricEqBand {
            freq_hz,
            gain_db,
            q,
            filter_type,
        };
        if !band.is_firmware_dummy_slot() {
            bands.push(band);
        }
    }
    Ok(ParametricEqPreset {
        eq_type,
        master_gain_db,
        bands,
    })
}

// --- Multipoint (0x4D, 0x4F) ---

pub fn decode_paired_host_list(val: &[u8]) -> ProtocolResult<PairedHostList> {
    if val.is_empty() {
        return Err(ProtocolError::MalformedPacket(
            "empty 0x4D PhoneBeanList payload".to_string(),
        ));
    }
    let total_count = val[0];
    let mut connected = Vec::new();
    let mut disconnected = Vec::new();
    let mut cursor = 1usize;

    while cursor < val.len() {
        let entry_len = val[cursor] as usize;
        let entry_start = cursor + 1;
        let entry_end = entry_start + entry_len;
        if entry_end > val.len() || entry_len < 7 {
            break;
        }
        let entry = &val[entry_start..entry_end];
        let status = entry[0];
        let mut mac = [0u8; 6];
        mac.copy_from_slice(&entry[1..7]);
        let name_bytes = &entry[7..];
        let name_end = name_bytes
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(name_bytes.len());
        let name = String::from_utf8_lossy(&name_bytes[..name_end]).into_owned();
        let is_conn = status == 2 || status == 3;
        let parsed = PairedHostEntry {
            status,
            connected: is_conn,
            mac,
            name,
        };
        if is_conn {
            connected.push(parsed);
        } else {
            disconnected.push(parsed);
        }
        cursor = entry_end;
    }

    Ok(PairedHostList {
        total_count,
        connected,
        disconnected,
    })
}

pub fn encode_multipoint_host_action(action: MultipointHostAction, mac: [u8; 6]) -> [u8; 7] {
    [action as u8, mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]]
}

pub fn decode_multipoint_host_status(val: &[u8]) -> ProtocolResult<MultipointHostStatus> {
    if val.len() < 7 {
        return Err(ProtocolError::MalformedPacket(format!(
            "0x4F PhoneStatusBean requires 7 bytes, got {}",
            val.len()
        )));
    }
    let mut mac = [0u8; 6];
    mac.copy_from_slice(&val[0..6]);
    Ok(MultipointHostStatus {
        mac,
        status: val[6],
    })
}

// --- Power, Hearing, Fit Test, Wear, LED, White Noise, Clock & Alarms ---

pub fn encode_power_manager(minutes: u16) -> [u8; 4] {
    let le = minutes.to_le_bytes();
    [le[0], le[1], 0x00, 0x00]
}

pub fn decode_power_manager(val: &[u8]) -> ProtocolResult<PowerManagerState> {
    if val.len() < 4 {
        return Err(ProtocolError::MalformedPacket(format!(
            "0x14 PowerManager requires 4 bytes, got {}",
            val.len()
        )));
    }
    let raw_power = u16::from_le_bytes([val[0], val[1]]);
    let scheduled_minutes = if raw_power == 0 {
        PowerManagerState::DISABLED
    } else {
        raw_power
    };
    let remaining_minutes = u16::from_le_bytes([val[2], val[3]]);
    Ok(PowerManagerState {
        scheduled_minutes,
        remaining_minutes,
    })
}

pub fn encode_hearing_health(cfg: HearingHealthConfig) -> [u8; 4] {
    let le = cfg.limit_minutes.to_le_bytes();
    [
        u8::from(cfg.alarm_volume),
        u8::from(cfg.alarm_time),
        le[0],
        le[1],
    ]
}

pub fn decode_hearing_health(val: &[u8]) -> ProtocolResult<HearingHealthConfig> {
    if val.len() < 4 {
        return Err(ProtocolError::MalformedPacket(format!(
            "0x58 HearingHealth requires 4 bytes, got {}",
            val.len()
        )));
    }
    Ok(HearingHealthConfig {
        alarm_volume: val[0] == 1,
        alarm_time: val[1] == 1,
        limit_minutes: u16::from_le_bytes([val[2], val[3]]),
    })
}

pub fn encode_wear_config(cfg: WearConfig) -> [u8; 3] {
    [
        u8::from(cfg.enabled),
        cfg.music_behavior.as_u8(),
        u8::from(cfg.control_anc),
    ]
}

pub fn decode_wear_config(val: &[u8]) -> ProtocolResult<WearConfig> {
    if val.len() < 3 {
        return Err(ProtocolError::MalformedPacket(format!(
            "0x2C WearConfig requires 3 bytes, got {}",
            val.len()
        )));
    }
    Ok(WearConfig {
        enabled: val[0] == 1,
        music_behavior: InEarMusicBehavior::from_u8(val[1]),
        control_anc: val[2] == 1,
    })
}

pub fn encode_led_effect(cfg: &LedEffectConfig) -> Vec<u8> {
    let mut out = Vec::with_capacity(3 + cfg.colors.len() * 3);
    out.push(cfg.speed);
    out.push(cfg.brightness);
    out.push(cfg.effect.as_u8());
    for c in &cfg.colors {
        out.push(c.r);
        out.push(c.g);
        out.push(c.b);
    }
    out
}

pub fn decode_led_effect(val: &[u8]) -> ProtocolResult<LedEffectConfig> {
    if val.len() < 3 {
        return Err(ProtocolError::MalformedPacket(format!(
            "0x36 LEDEffect requires >= 3 bytes, got {}",
            val.len()
        )));
    }
    let colors = val[3..]
        .as_chunks::<3>()
        .0
        .iter()
        .map(|c| RgbColor {
            r: c[0],
            g: c[1],
            b: c[2],
        })
        .collect();
    Ok(LedEffectConfig {
        speed: val[0],
        brightness: val[1],
        effect: LedEffectKind::from_u8(val[2]),
        colors,
    })
}

pub fn encode_sync_time(dt: DeviceDateTime) -> [u8; 7] {
    [
        dt.year_mod_100,
        dt.month,
        dt.day,
        dt.hour,
        dt.minute,
        dt.second,
        dt.weekday_mask,
    ]
}

pub fn encode_alarm_command(cmd: AlarmCommand) -> [u8; 7] {
    match cmd {
        AlarmCommand::Add(a) => [
            0x01,
            a.alarm_id,
            u8::from(a.enabled),
            a.hour,
            a.minute,
            a.cycle_mask,
            0x00,
        ],
        AlarmCommand::Delete { alarm_id } => [0x02, alarm_id, 0x00, 0x00, 0x00, 0x00, 0x00],
        AlarmCommand::Edit { alarm, flag } => [
            0x03,
            alarm.alarm_id,
            u8::from(alarm.enabled),
            alarm.hour,
            alarm.minute,
            alarm.cycle_mask,
            flag,
        ],
    }
}

pub fn decode_alarms(val: &[u8]) -> ProtocolResult<(u8, Vec<DeviceAlarm>)> {
    if val.is_empty() {
        return Err(ProtocolError::MalformedPacket(
            "empty 0x3F Alarm payload".to_string(),
        ));
    }
    let header = val[0];
    let alarms = val[1..]
        .as_chunks::<6>()
        .0
        .iter()
        .map(|c| DeviceAlarm {
            alarm_id: c[0],
            enabled: c[1] == 1,
            hour: c[2],
            minute: c[3],
            cycle_mask: c[4],
        })
        .collect();
    Ok((header, alarms))
}

pub fn decode_white_noise_play_state(val: &[u8]) -> ProtocolResult<WhiteNoisePlayState> {
    if val.len() < 6 {
        return Err(ProtocolError::MalformedPacket(format!(
            "0x3A MusicState requires 6 bytes, got {}",
            val.len()
        )));
    }
    Ok(WhiteNoisePlayState {
        music_index: u32::from_le_bytes([val[0], val[1], val[2], val[3]]),
        status: val[4],
        play_mode: val[5],
    })
}

pub fn decode_white_noise_track_list(val: &[u8]) -> ProtocolResult<WhiteNoiseTrackList> {
    if val.is_empty() {
        return Err(ProtocolError::MalformedPacket(
            "empty 0x3B MusicInfo payload".to_string(),
        ));
    }
    let start_tone_id = val[0];
    let tracks = val[1..]
        .as_chunks::<6>()
        .0
        .iter()
        .map(|c| WhiteNoiseTrackInfo {
            music_id: u32::from_le_bytes([c[0], c[1], c[2], c[3]]),
            total: u16::from_le_bytes([c[4], c[5]]),
        })
        .collect();
    Ok(WhiteNoiseTrackList {
        start_tone_id,
        tracks,
    })
}

/// Decodes a single `TlvItem` into one or more strongly-typed `DeviceEvent`s.
///
/// For `0x17` (`AncSetting`), emits BOTH `AncSettingUpdated` (exact 3-byte tuple)
/// AND `AncModeChanged` (high-level mode+depth) so callers listening to either
/// representation stay synchronized.
pub fn decode_tlv_item(item: &TlvItem) -> ProtocolResult<Vec<DeviceEvent>> {
    let val = item.value.as_slice();
    let Some(cmd) = item.cmd_id() else {
        return Ok(vec![DeviceEvent::RawTlv {
            cmd: item.cmd,
            value: item.value.clone(),
        }]);
    };

    let events = match cmd {
        CmdId::Battery => {
            if val.len() < 3 {
                return Err(ProtocolError::MalformedPacket(format!(
                    "0x2F Battery requires 3 bytes, got {}",
                    val.len()
                )));
            }
            let l = BatteryLevel::from_wire_byte(val[0]);
            let r = BatteryLevel::from_wire_byte(val[1]);
            let c = BatteryLevel::from_wire_byte(val[2]);
            vec![DeviceEvent::BatteryUpdated {
                left: l.percent,
                right: r.percent,
                case: c.percent,
                left_charging: l.charging,
                right_charging: r.charging,
                case_charging: c.charging,
            }]
        }
        CmdId::Version => {
            if val.len() < 3 {
                return Err(ProtocolError::MalformedPacket(format!(
                    "0x30 Version requires 3 bytes, got {}",
                    val.len()
                )));
            }
            vec![DeviceEvent::FirmwareVersion(FirmwareVersion {
                major: val[0],
                minor: val[1],
                patch: val[2],
            })]
        }
        CmdId::PairName => {
            let trimmed = val.iter().position(|&b| b == 0).unwrap_or(val.len());
            vec![DeviceEvent::DeviceNameChanged {
                name: String::from_utf8_lossy(&val[..trimmed]).into_owned(),
            }]
        }
        CmdId::DeviceColor if !val.is_empty() => {
            vec![DeviceEvent::DeviceColorChanged {
                color_index: val[0],
            }]
        }
        CmdId::AncSetting => {
            let setting = decode_anc_setting(val)?;
            let (mode, depth) = setting.to_mode_and_depth();
            vec![
                DeviceEvent::AncSettingUpdated(setting),
                DeviceEvent::AncModeChanged { mode, depth },
            ]
        }
        CmdId::NoiseMode if !val.is_empty() => {
            vec![DeviceEvent::LegacyNoiseModeUpdated { mode: val[0] }]
        }
        CmdId::WindNoise if !val.is_empty() => vec![DeviceEvent::WindNoiseChanged {
            enabled: decode_qcy_bool(val[0]),
        }],
        CmdId::AncResult if !val.is_empty() => {
            vec![DeviceEvent::AdaptiveAncCalibration { status: val[0] }]
        }
        CmdId::AncWear => {
            let mask = if val.len() >= 4 {
                let lo = val[2].wrapping_add(3) as u16;
                let hi = val[3].wrapping_add(3) as u16;
                lo | (hi << 8)
            } else if val.len() >= 2 {
                u16::from_le_bytes([val[0], val[1]])
            } else {
                return Err(ProtocolError::MalformedPacket(
                    "0x29 AncWear requires >= 2 bytes".to_string(),
                ));
            };
            vec![DeviceEvent::AncWearStatus { mask }]
        }
        CmdId::WearStatus if val.len() >= 2 => vec![DeviceEvent::WearStatusUpdated {
            mask: u16::from_le_bytes([val[0], val[1]]),
        }],
        CmdId::BtnAncCfg => vec![DeviceEvent::ButtonAncCycleUpdated {
            modes: decode_btn_anc_cycle(val)?,
        }],
        CmdId::AiAnc => vec![DeviceEvent::AiAncStateUpdated(decode_ai_anc_state(val)?)],
        CmdId::AiAncCfg => vec![DeviceEvent::AiAncDefaultMatrixUpdated {
            entries: decode_ai_anc_matrix(val)?,
        }],
        CmdId::AiAncCfg2 => vec![DeviceEvent::AiAncUserMatrixUpdated {
            entries: decode_ai_anc_matrix(val)?,
        }],
        CmdId::GainEq => vec![DeviceEvent::GraphicEqUpdated(decode_graphic_eq(val)?)],
        CmdId::MultiEq => vec![DeviceEvent::ParametricEqV1Updated(decode_parametric_eq_v1(
            val,
        )?)],
        CmdId::MultiEq2 => vec![DeviceEvent::ParametricEqV2Updated(decode_parametric_eq_v2(
            val,
        )?)],
        CmdId::EqCount if !val.is_empty() => vec![DeviceEvent::EqBandCountUpdated {
            count: val[0].max(5),
        }],
        CmdId::DynamicEq if !val.is_empty() => vec![DeviceEvent::DynamicEqChanged {
            enabled: decode_qcy_bool(val[0]),
        }],
        CmdId::BassEnhancement if !val.is_empty() => vec![DeviceEvent::BassEnhancementChanged {
            enabled: decode_qcy_bool(val[0]),
        }],
        CmdId::ThxEq if !val.is_empty() => vec![DeviceEvent::ThxEqChanged {
            enabled: decode_qcy_bool(val[0]),
        }],
        CmdId::AdaptVolume if !val.is_empty() => vec![DeviceEvent::AdaptiveVolumeChanged {
            enabled: decode_qcy_bool(val[0]),
        }],
        CmdId::ChannelBalance if !val.is_empty() => {
            vec![DeviceEvent::ChannelBalanceUpdated { balance: val[0] }]
        }
        CmdId::ButtonFunc => {
            let bindings = val
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| ButtonBinding {
                    event_id: c[0] as u16,
                    action: ButtonAction::from_u8(c[1]),
                })
                .collect();
            vec![DeviceEvent::ButtonBindingsUpdated { bindings }]
        }
        CmdId::SleepMode if !val.is_empty() => vec![DeviceEvent::SleepModeChanged {
            enabled: decode_qcy_bool(val[0]),
        }],
        CmdId::TouchTone if !val.is_empty() => vec![DeviceEvent::TouchToneChanged {
            enabled: decode_qcy_bool(val[0]),
        }],
        CmdId::TouchSensitivity if !val.is_empty() => vec![DeviceEvent::TouchSensitivityUpdated {
            sensitivity: TouchSensitivity::from_u8(val[0]),
        }],
        CmdId::HeadAction if !val.is_empty() => vec![DeviceEvent::HeadGestureEnabledChanged {
            enabled: decode_qcy_bool(val[0]),
        }],
        CmdId::SensorConfig => {
            let bindings = val
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| HeadGestureBinding {
                    event: HeadGestureEvent::from_u8(c[0]),
                    action: HeadGestureAction::from_u8(c[1]),
                })
                .collect();
            vec![DeviceEvent::HeadGestureBindingsUpdated { bindings }]
        }
        CmdId::SensorTest if !val.is_empty() => vec![DeviceEvent::HeadGestureDetected {
            event: HeadGestureEvent::from_u8(val[0]),
        }],
        CmdId::TakePhoto if !val.is_empty() => {
            vec![DeviceEvent::CameraShutterTriggered { value: val[0] }]
        }
        CmdId::GameMode if !val.is_empty() => vec![DeviceEvent::GamingModeChanged {
            enabled: decode_qcy_bool(val[0]),
        }],
        CmdId::GameConfig if val.len() >= 2 => {
            vec![DeviceEvent::GameModeProUpdated(GameModeProConfig {
                enabled: decode_qcy_bool(val[0]),
                follow_game_sound: decode_qcy_bool(val[1]),
            })]
        }
        CmdId::MusicMode if !val.is_empty() => vec![DeviceEvent::ListeningModeUpdated {
            mode: ListeningMode::from_u8(val[0]),
        }],
        CmdId::SpaceSound if !val.is_empty() => vec![DeviceEvent::SpatialSoundChanged {
            enabled: decode_qcy_bool(val[0]),
        }],
        CmdId::SpaceAudioMode if !val.is_empty() => vec![DeviceEvent::SpatialAudioModeUpdated {
            mode: SpatialAudioMode::from_u8(val[0]),
        }],
        CmdId::Ldac if !val.is_empty() => vec![DeviceEvent::LdacChanged {
            enabled: decode_qcy_bool(val[0]),
        }],
        CmdId::Aptx if !val.is_empty() => vec![DeviceEvent::AptxChanged {
            enabled: decode_qcy_bool(val[0]),
        }],
        CmdId::LeAudio if !val.is_empty() => vec![DeviceEvent::LeAudioChanged {
            enabled: decode_qcy_bool(val[0]),
        }],
        CmdId::PrivacyMode if val.len() >= 2 => {
            vec![DeviceEvent::PrivacyModeProUpdated(PrivacyModeProConfig {
                enabled: decode_qcy_bool(val[0]),
                call_only: val[1] == 1,
            })]
        }
        CmdId::TalkCheck if !val.is_empty() => vec![DeviceEvent::DialoguePerceptionChanged {
            enabled: decode_qcy_bool(val[0]),
        }],
        CmdId::VoiceDetection if !val.is_empty() => {
            vec![DeviceEvent::VoiceDetectionDelayUpdated { seconds: val[0] }]
        }
        CmdId::EnvAdaptation if !val.is_empty() => {
            vec![DeviceEvent::EnvironmentAdaptationChanged {
                enabled: decode_qcy_bool(val[0]),
            }]
        }
        CmdId::PowerSavingMode if !val.is_empty() => vec![DeviceEvent::PowerSavingModeChanged {
            enabled: decode_qcy_bool(val[0]),
        }],
        CmdId::DisconnectWarning if !val.is_empty() => {
            vec![DeviceEvent::DisconnectWarningChanged {
                enabled: decode_qcy_bool(val[0]),
            }]
        }
        CmdId::DualConnect if !val.is_empty() => vec![DeviceEvent::DualConnectChanged {
            enabled: decode_qcy_bool(val[0]),
        }],
        CmdId::PhoneInfo => vec![DeviceEvent::PairedHostsUpdated(decode_paired_host_list(
            val,
        )?)],
        CmdId::PhoneStatus => vec![DeviceEvent::MultipointHostStatusUpdated(
            decode_multipoint_host_status(val)?,
        )],
        CmdId::InEarCheck if !val.is_empty() => vec![DeviceEvent::InEarCheckChanged {
            enabled: decode_qcy_bool(val[0]),
        }],
        CmdId::WearConfig => vec![DeviceEvent::WearConfigUpdated(decode_wear_config(val)?)],
        CmdId::InEarSensitivity if !val.is_empty() => vec![DeviceEvent::WearSensitivityUpdated {
            sensitivity: WearSensitivity::from_u8(val[0]),
        }],
        CmdId::FitTest if val.len() >= 3 => vec![DeviceEvent::FitTestUpdated(FitTestResult {
            status: val[0],
            left_result: val[1],
            right_result: val[2],
        })],
        CmdId::HearHealth => vec![DeviceEvent::HearingHealthUpdated(decode_hearing_health(
            val,
        )?)],
        CmdId::PowerManager => vec![DeviceEvent::PowerManagerUpdated(decode_power_manager(val)?)],
        CmdId::Standby if val.len() >= 2 => vec![DeviceEvent::StandbyMinutesUpdated {
            minutes: u16::from_le_bytes([val[0], val[1]]),
        }],
        CmdId::VoiceName => vec![DeviceEvent::PromptVoiceCodeUpdated {
            code: String::from_utf8_lossy(val).into_owned(),
        }],
        CmdId::VoiceLanguage if !val.is_empty() => {
            vec![DeviceEvent::PromptLanguageIndexUpdated { index: val[0] }]
        }
        CmdId::ToneVolume if !val.is_empty() => {
            vec![DeviceEvent::PromptToneVolumeUpdated(PromptToneVolume {
                current: val[0],
                max: val.get(1).copied(),
            })]
        }
        CmdId::TwsEnable if !val.is_empty() => vec![DeviceEvent::TwsEnableChanged {
            enabled: decode_qcy_bool(val[0]),
        }],
        CmdId::LedSwitch if !val.is_empty() => vec![DeviceEvent::LedSwitchChanged {
            enabled: decode_qcy_bool(val[0]),
        }],
        CmdId::LedEffect => vec![DeviceEvent::LedEffectUpdated(decode_led_effect(val)?)],
        CmdId::FocusMode if !val.is_empty() => vec![DeviceEvent::FocusModeChanged {
            enabled: decode_qcy_bool(val[0]),
        }],
        CmdId::Volume if !val.is_empty() => vec![DeviceEvent::WhiteNoiseVolumeUpdated {
            volume: val[0],
            flag: val.get(2).copied().unwrap_or(0),
        }],
        CmdId::MusicState => vec![DeviceEvent::WhiteNoisePlayStateUpdated(
            decode_white_noise_play_state(val)?,
        )],
        CmdId::MusicInfo => vec![DeviceEvent::WhiteNoiseTrackListUpdated(
            decode_white_noise_track_list(val)?,
        )],
        CmdId::Alarm => {
            let (header, alarms) = decode_alarms(val)?;
            vec![DeviceEvent::AlarmsUpdated { header, alarms }]
        }
        _ => vec![DeviceEvent::RawTlv {
            cmd: item.cmd,
            value: item.value.clone(),
        }],
    };

    Ok(events)
}
