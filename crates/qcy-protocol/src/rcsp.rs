//! JieLi RCSP packet framing (`FE DC BA ... EF`), BLE stream accumulator,
//! `isNewJL` QCY-TLV tunneling (`CMD_EXTRA_CUSTOM = 0xFF`), and Legacy JieLi
//! native RCSP command/notification codecs.

use crate::adv::BatteryLevel;
use crate::tlv::decode_tlv_frame;
use crate::types::{ButtonAction, ButtonBinding, DeviceEvent, FirmwareVersion, GraphicEqSetting};
use crate::{ProtocolError, ProtocolResult};

/// 3-byte JieLi RCSP start-of-frame marker.
pub const RCSP_PREFIX: [u8; 3] = [0xFE, 0xDC, 0xBA];
/// 1-byte JieLi RCSP end-of-frame marker.
pub const RCSP_SUFFIX: u8 = 0xEF;

/// Flag bit indicating a command packet (`1`) vs response packet (`0`).
pub const RCSP_FLAG_COMMAND: u8 = 0x80;
/// Flag bit indicating the sender expects a response (`1`).
pub const RCSP_FLAG_NEEDS_RESPONSE: u8 = 0x40;

// --- Verified JieLi RCSP Opcodes ---

pub const OP_GET_TARGET_INFO: u8 = 0x03;
pub const OP_GET_SYS_INFO: u8 = 0x07;
pub const OP_SET_SYS_INFO: u8 = 0x08;
pub const OP_ADV_GET_INFO: u8 = 0xC0;
pub const OP_ADV_MODIFY_INFO: u8 = 0xC1;
pub const OP_ADV_DEVICE_NOTIFY: u8 = 0xC2;
pub const OP_ADV_NOTIFY_SETTINGS: u8 = 0xC3;
pub const OP_ADV_GET_DEVICE_SETTINGS: u8 = 0xC4;
pub const OP_EXTRA_CUSTOM: u8 = 0xFF;

// --- Verified JieLi RCSP OTA Opcodes (otaType = 2 & 9) ---

pub const OP_OTA_GET_MTU: u8 = 0xD1;
pub const OP_OTA_GET_MD5: u8 = 0xD4;
pub const OP_OTA_GET_FILE_INFO_OFFSET: u8 = 0xE1;
pub const OP_OTA_INQUIRE_CAN_UPDATE: u8 = 0xE2;
pub const OP_OTA_ENTER_UPDATE: u8 = 0xE3;
pub const OP_OTA_EXIT_UPDATE: u8 = 0xE4;
pub const OP_OTA_SEND_BLOCK: u8 = 0xE5;
pub const OP_OTA_REFRESH_STATUS: u8 = 0xE6;
pub const OP_OTA_REBOOT: u8 = 0xE7;
pub const OP_OTA_NOTIFY_FILE_SIZE: u8 = 0xE8;

// --- Legacy JL System / ADV Attribute Types ---

pub const JL_SYS_FUNC_PUBLIC: u8 = 0xFF;
pub const JL_SYS_ATTR_EQ: u8 = 0x04;
pub const JL_SYS_ATTR_NOISE_MODE: u8 = 0x0D;

pub const JL_ADV_ATTR_BATTERY: u8 = 0x00;
pub const JL_ADV_ATTR_NAME: u8 = 0x01;
pub const JL_ADV_ATTR_KEY_SETTINGS: u8 = 0x02;
pub const JL_ADV_ATTR_WORK_MODE: u8 = 0x05;

/// Parsed JieLi RCSP wire frame:
/// `[0xFE, 0xDC, 0xBA, flag, opcode, len_hi, len_lo, payload..., 0xEF]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RcspFrame {
    pub is_command: bool,
    pub needs_response: bool,
    pub opcode: u8,
    pub payload: Vec<u8>,
}

impl RcspFrame {
    /// Constructs a command RCSP frame with sequence number `sn` prepended to `body`.
    pub fn command(opcode: u8, needs_response: bool, sn: u8, body: &[u8]) -> Self {
        let mut payload = Vec::with_capacity(1 + body.len());
        payload.push(sn);
        payload.extend_from_slice(body);
        Self {
            is_command: true,
            needs_response,
            opcode,
            payload,
        }
    }

    /// Constructs a response RCSP frame with `[status, sn, body...]`.
    pub fn response(opcode: u8, status: u8, sn: u8, body: &[u8]) -> Self {
        let mut payload = Vec::with_capacity(2 + body.len());
        payload.push(status);
        payload.push(sn);
        payload.extend_from_slice(body);
        Self {
            is_command: false,
            needs_response: false,
            opcode,
            payload,
        }
    }

    /// Returns the RCSP sequence number (`payload[0]` for commands, `payload[1]` for responses).
    pub fn sn(&self) -> Option<u8> {
        if self.is_command {
            self.payload.first().copied()
        } else {
            self.payload.get(1).copied()
        }
    }

    /// Returns the status byte for response frames (`payload[0]`).
    pub fn status(&self) -> Option<u8> {
        if self.is_command {
            None
        } else {
            self.payload.first().copied()
        }
    }

    /// Returns the body slice after `[sn]` (commands) or `[status, sn]` (responses).
    pub fn body(&self) -> &[u8] {
        if self.is_command {
            self.payload.get(1..).unwrap_or(&[])
        } else {
            self.payload.get(2..).unwrap_or(&[])
        }
    }

    /// Encodes this frame into wire bytes (`FE DC BA [flag] [opcode] [len_be] [payload] EF`).
    pub fn encode(&self) -> ProtocolResult<Vec<u8>> {
        if self.payload.len() > u16::MAX as usize {
            return Err(ProtocolError::MalformedPacket(format!(
                "RCSP payload exceeds 65535 bytes ({})",
                self.payload.len()
            )));
        }
        let mut flag = 0u8;
        if self.is_command {
            flag |= RCSP_FLAG_COMMAND;
        }
        if self.needs_response {
            flag |= RCSP_FLAG_NEEDS_RESPONSE;
        }
        let len_be = (self.payload.len() as u16).to_be_bytes();
        let mut out = Vec::with_capacity(8 + self.payload.len());
        out.extend_from_slice(&RCSP_PREFIX);
        out.push(flag);
        out.push(self.opcode);
        out.extend_from_slice(&len_be);
        out.extend_from_slice(&self.payload);
        out.push(RCSP_SUFFIX);
        Ok(out)
    }

    /// Decodes a single complete RCSP wire frame.
    pub fn decode(bytes: &[u8]) -> ProtocolResult<Self> {
        if bytes.len() < 8 {
            return Err(ProtocolError::MalformedPacket(format!(
                "RCSP frame too short: {} bytes",
                bytes.len()
            )));
        }
        if bytes[0..3] != RCSP_PREFIX {
            return Err(ProtocolError::MalformedPacket(
                "RCSP frame missing FE DC BA prefix".to_string(),
            ));
        }
        let flag = bytes[3];
        let opcode = bytes[4];
        let payload_len = u16::from_be_bytes([bytes[5], bytes[6]]) as usize;
        let expected_len = 8 + payload_len;
        if bytes.len() < expected_len {
            return Err(ProtocolError::MalformedPacket(format!(
                "truncated RCSP frame: expected {expected_len} bytes, got {}",
                bytes.len()
            )));
        }
        if bytes[expected_len - 1] != RCSP_SUFFIX {
            return Err(ProtocolError::MalformedPacket(format!(
                "RCSP frame missing 0xEF suffix (got {:#04x})",
                bytes[expected_len - 1]
            )));
        }
        Ok(Self {
            is_command: (flag & RCSP_FLAG_COMMAND) != 0,
            needs_response: (flag & RCSP_FLAG_NEEDS_RESPONSE) != 0,
            opcode,
            payload: bytes[7..7 + payload_len].to_vec(),
        })
    }
}

/// Stateful stream accumulator extracting complete `FE DC BA ... EF` frames from
/// fragmented or coalesced BLE GATT notifications.
#[derive(Debug, Default, Clone)]
pub struct RcspStreamAccumulator {
    buffer: Vec<u8>,
}

impl RcspStreamAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Pushes raw BLE notification bytes and returns all complete decoded `RcspFrame`s.
    pub fn push(&mut self, data: &[u8]) -> Vec<RcspFrame> {
        self.buffer.extend_from_slice(data);
        let mut frames = Vec::new();

        loop {
            let Some(sync_pos) = self.buffer.windows(3).position(|w| w == RCSP_PREFIX) else {
                // Retain at most the last 2 bytes in case FE DC is split across notifications.
                if self.buffer.len() > 2 {
                    let drain_to = self.buffer.len() - 2;
                    self.buffer.drain(0..drain_to);
                }
                break;
            };
            if sync_pos > 0 {
                self.buffer.drain(0..sync_pos);
            }
            if self.buffer.len() < 8 {
                break;
            }
            let payload_len = u16::from_be_bytes([self.buffer[5], self.buffer[6]]) as usize;
            let total_len = 8 + payload_len;
            if self.buffer.len() < total_len {
                break;
            }
            if self.buffer[total_len - 1] != RCSP_SUFFIX {
                // False sync header inside noise; advance past first byte and resync.
                self.buffer.drain(0..1);
                continue;
            }
            let raw: Vec<u8> = self.buffer.drain(0..total_len).collect();
            if let Ok(frame) = RcspFrame::decode(&raw) {
                frames.push(frame);
            }
        }

        frames
    }
}

// ---------------------------------------------------------------------------
// isNewJL QCY-TLV Tunneling (CMD_EXTRA_CUSTOM = 0xFF)
// ---------------------------------------------------------------------------

/// Wraps a complete QCY `[0xFF, PayloadLen, ...]` TLV frame inside a JieLi RCSP
/// `CMD_EXTRA_CUSTOM` (`0xFF`) command frame for `isNewJL` (`modelID & 0x8070 == 0x8070`).
pub fn wrap_qcy_tlv_in_rcsp(qcy_tlv_frame: &[u8], sn: u8) -> ProtocolResult<Vec<u8>> {
    RcspFrame::command(OP_EXTRA_CUSTOM, true, sn, qcy_tlv_frame).encode()
}

/// Extracts the inner QCY `[0xFF, PayloadLen, ...]` TLV frame from an RCSP `0xFF` frame.
pub fn unwrap_qcy_tlv_from_rcsp(frame: &RcspFrame) -> Option<&[u8]> {
    if frame.opcode != OP_EXTRA_CUSTOM {
        return None;
    }
    let body = frame.body();
    if body.first() == Some(&0xFF) && decode_tlv_frame(body).is_ok() {
        return Some(body);
    }
    // Some firmware responses omit the status byte or prepend an extra custom sub-header.
    for offset in 0..frame.payload.len() {
        let candidate = &frame.payload[offset..];
        if candidate.first() == Some(&0xFF) && decode_tlv_frame(candidate).is_ok() {
            return Some(candidate);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Legacy JieLi Native RCSP Helpers (!isNewJL)
// ---------------------------------------------------------------------------

/// `JLDeviceImpl.changeQCY2JL`: maps QCY Action `0x00` (None) to JieLi `0x7F`.
pub const fn change_qcy_action_to_jl(action: u8) -> u8 {
    if action == 0x00 {
        0x7F
    } else {
        action
    }
}

/// `JLDeviceImpl.changeJL2QCY`: maps JieLi Action `0x7F` (None) back to QCY `0x00`.
pub const fn change_jl_action_to_qcy(action: u8) -> u8 {
    if action == 0x7F {
        0x00
    } else {
        action
    }
}

/// Splits a QCY `event_id` into Legacy JieLi `(key_idx, gesture_action)`.
///
/// - TWS `1..=10`: odd => Left (`key_idx = 1`), even => Right (`key_idx = 2`),
///   `gesture = ((event_id - 1) / 2) + 1`.
/// - Over-ear 16-bit (`0x0301`, `0x0501`, `0x0701..0x0704`):
///   `key_idx = (event_id >> 8) as u8`, `gesture = (event_id & 0xFF) as u8`.
pub const fn split_jl_button_event(event_id: u16) -> (u8, u8) {
    if event_id > 0xFF {
        ((event_id >> 8) as u8, (event_id & 0xFF) as u8)
    } else if event_id == 0 {
        (1, 1)
    } else {
        let key_idx = if (event_id % 2) == 1 { 1 } else { 2 };
        let gesture = (((event_id - 1) / 2) + 1) as u8;
        (key_idx, gesture)
    }
}

/// Inverse of `split_jl_button_event`.
pub const fn combine_jl_button_event(key_idx: u8, gesture: u8) -> u16 {
    if key_idx == 1 || key_idx == 2 {
        let base = gesture.saturating_sub(1) as u16 * 2;
        base + (key_idx as u16)
    } else {
        ((key_idx as u16) << 8) | (gesture as u16)
    }
}

/// Encodes `CMD_GET_TARGET_INFO` (`0x03`) querying version & capabilities.
pub fn encode_jl_get_target_info(sn: u8) -> ProtocolResult<Vec<u8>> {
    // 4-byte mask (0xFFFFFFFF = all target info fields) + platform byte (0x00 Android/host)
    RcspFrame::command(
        OP_GET_TARGET_INFO,
        true,
        sn,
        &[0xFF, 0xFF, 0xFF, 0xFF, 0x00],
    )
    .encode()
}

/// Encodes `CMD_ADV_GET_INFO` (`0xC0`) querying battery, name, buttons, and work mode.
pub fn encode_jl_get_adv_info(sn: u8) -> ProtocolResult<Vec<u8>> {
    RcspFrame::command(OP_ADV_GET_INFO, true, sn, &[0xFF, 0xFF, 0xFF, 0xFF]).encode()
}

/// Encodes a Legacy JieLi RCSP `CMD_ADV_MODIFY_INFO` (`0xC1`) button binding update.
pub fn encode_jl_set_button(
    event_id: u16,
    action: ButtonAction,
    sn: u8,
) -> ProtocolResult<Vec<u8>> {
    let (key_idx, gesture) = split_jl_button_event(event_id);
    let jl_action = change_qcy_action_to_jl(action.as_u8());
    // LTV inside 0xC1: [len=4, type=0x02 (KEY_SETTINGS), key_idx, gesture, jl_action]
    RcspFrame::command(
        OP_ADV_MODIFY_INFO,
        true,
        sn,
        &[0x04, JL_ADV_ATTR_KEY_SETTINGS, key_idx, gesture, jl_action],
    )
    .encode()
}

/// Encodes Legacy JieLi RCSP `CMD_ADV_MODIFY_INFO` (`0xC1`) device rename (`attr = 0x01`).
pub fn encode_jl_rename(name: &str, sn: u8) -> ProtocolResult<Vec<u8>> {
    let name_bytes = name.as_bytes();
    let max_len = name_bytes.len().min(32);
    let mut body = Vec::with_capacity(2 + max_len);
    body.push((1 + max_len) as u8);
    body.push(JL_ADV_ATTR_NAME);
    body.extend_from_slice(&name_bytes[..max_len]);
    RcspFrame::command(OP_ADV_MODIFY_INFO, true, sn, &body).encode()
}

/// Encodes Legacy JieLi RCSP work mode / game mode (`1` = Normal, `2` = Game/Low-Latency).
pub fn encode_jl_set_work_mode(game_mode: bool, sn: u8) -> ProtocolResult<Vec<u8>> {
    let mode_byte = if game_mode { 0x02 } else { 0x01 };
    RcspFrame::command(
        OP_ADV_MODIFY_INFO,
        true,
        sn,
        &[0x02, JL_ADV_ATTR_WORK_MODE, mode_byte],
    )
    .encode()
}

/// Encodes Legacy JieLi RCSP `CMD_SET_SYS_INFO` (`0x08`) graphic EQ (`attr = 0x04`).
pub fn encode_jl_set_eq(eq: &GraphicEqSetting, sn: u8) -> ProtocolResult<Vec<u8>> {
    // [function=0xFF, ltv_len, attr=0x04, eq_mode_with_dynamic_flag, band_count, gains...]
    let band_count = eq.gains_db.len().min(10);
    let mut body = Vec::with_capacity(5 + band_count);
    body.push(JL_SYS_FUNC_PUBLIC);
    body.push((3 + band_count) as u8);
    body.push(JL_SYS_ATTR_EQ);
    body.push(eq.eq_type | 0x80);
    body.push(band_count as u8);
    for &g in eq.gains_db.iter().take(band_count) {
        body.push(g as u8);
    }
    RcspFrame::command(OP_SET_SYS_INFO, true, sn, &body).encode()
}

/// Encodes Legacy JieLi RCSP `CMD_SET_SYS_INFO` (`0x08`) ANC noise mode (`attr = 0x0D`),
/// linearly interpolating gain across `[min_gain, max_gain]` by `percent` (`0..=100`).
pub fn encode_jl_set_noise_mode(
    mode: u8,
    percent: u8,
    min_gain: i16,
    max_gain: i16,
    sn: u8,
) -> ProtocolResult<Vec<u8>> {
    let pct = percent.min(100) as i32;
    let gain = (min_gain as i32) + ((max_gain as i32 - min_gain as i32) * pct) / 100;
    let gain_be = (gain.clamp(i16::MIN as i32, i16::MAX as i32) as i16).to_be_bytes();
    // [function=0xFF, ltv_len=4, attr=0x0D, mode, gain_hi, gain_lo]
    RcspFrame::command(
        OP_SET_SYS_INFO,
        true,
        sn,
        &[
            JL_SYS_FUNC_PUBLIC,
            0x04,
            JL_SYS_ATTR_NOISE_MODE,
            mode,
            gain_be[0],
            gain_be[1],
        ],
    )
    .encode()
}

/// Decodes Legacy JieLi native RCSP notifications/responses into unified `DeviceEvent`s.
pub fn decode_jl_legacy_frame(frame: &RcspFrame) -> Vec<DeviceEvent> {
    let body = frame.body();
    let mut events = Vec::new();

    match frame.opcode {
        OP_ADV_GET_INFO | OP_ADV_DEVICE_NOTIFY | OP_ADV_NOTIFY_SETTINGS => {
            // Parse JieLi LTV sequence: [len, attr_type, value[0..len-1]]
            let mut cursor = 0usize;
            let mut button_bindings = Vec::new();
            while cursor + 2 <= body.len() {
                let ltv_len = body[cursor] as usize;
                if ltv_len == 0 || cursor + 1 + ltv_len > body.len() {
                    break;
                }
                let attr_type = body[cursor + 1];
                let val = &body[cursor + 2..cursor + 1 + ltv_len];
                match attr_type {
                    JL_ADV_ATTR_BATTERY if val.len() >= 3 => {
                        let l = BatteryLevel::from_wire_byte(val[0]);
                        let r = BatteryLevel::from_wire_byte(val[1]);
                        let c = BatteryLevel::from_wire_byte(val[2]);
                        events.push(DeviceEvent::BatteryUpdated {
                            left: l.percent,
                            right: r.percent,
                            case: c.percent,
                            left_charging: l.charging,
                            right_charging: r.charging,
                            case_charging: c.charging,
                        });
                    }
                    JL_ADV_ATTR_NAME => {
                        let end = val.iter().position(|&b| b == 0).unwrap_or(val.len());
                        events.push(DeviceEvent::DeviceNameChanged {
                            name: String::from_utf8_lossy(&val[..end]).into_owned(),
                        });
                    }
                    JL_ADV_ATTR_KEY_SETTINGS => {
                        for chunk in val.as_chunks::<3>().0 {
                            let event_id = combine_jl_button_event(chunk[0], chunk[1]);
                            let qcy_action = change_jl_action_to_qcy(chunk[2]);
                            button_bindings.push(ButtonBinding {
                                event_id,
                                action: ButtonAction::from_u8(qcy_action),
                            });
                        }
                    }
                    JL_ADV_ATTR_WORK_MODE if !val.is_empty() => {
                        events.push(DeviceEvent::GamingModeChanged {
                            enabled: val[0] == 0x02,
                        });
                    }
                    _ => {}
                }
                cursor += 1 + ltv_len;
            }
            if !button_bindings.is_empty() {
                events.push(DeviceEvent::ButtonBindingsUpdated {
                    bindings: button_bindings,
                });
            }
        }
        OP_GET_TARGET_INFO => {
            // LTV list: attr 0x00 = version (2 bytes or ASCII), attr 0x01 = battery
            let mut cursor = 0usize;
            while cursor + 2 <= body.len() {
                let ltv_len = body[cursor] as usize;
                if ltv_len == 0 || cursor + 1 + ltv_len > body.len() {
                    break;
                }
                let attr_type = body[cursor + 1];
                let val = &body[cursor + 2..cursor + 1 + ltv_len];
                if attr_type == 0x00 && val.len() >= 2 {
                    // JieLi 16-bit packed version: bits 12..15 major, 8..11 minor, 4..7 patch
                    let raw = u16::from_be_bytes([val[0], val[1]]);
                    events.push(DeviceEvent::FirmwareVersion(FirmwareVersion {
                        major: ((raw >> 12) & 0x0F) as u8,
                        minor: ((raw >> 8) & 0x0F) as u8,
                        patch: ((raw >> 4) & 0x0F) as u8,
                    }));
                }
                cursor += 1 + ltv_len;
            }
        }
        OP_GET_SYS_INFO | OP_SET_SYS_INFO => {
            // Body starts with function byte (0xFF) followed by LTV entries
            let mut cursor = usize::from(!body.is_empty());
            while cursor + 2 <= body.len() {
                let ltv_len = body[cursor] as usize;
                if ltv_len == 0 || cursor + 1 + ltv_len > body.len() {
                    break;
                }
                let attr_type = body[cursor + 1];
                let val = &body[cursor + 2..cursor + 1 + ltv_len];
                if attr_type == JL_SYS_ATTR_EQ && val.len() >= 2 {
                    let eq_type = val[0] & 0x7F;
                    let gains_slice = if (val[0] & 0x80) != 0 && val.len() >= 2 {
                        let count = val[1] as usize;
                        &val[2..val.len().min(2 + count)]
                    } else {
                        &val[1..]
                    };
                    events.push(DeviceEvent::GraphicEqUpdated(GraphicEqSetting {
                        eq_type,
                        gains_db: gains_slice.iter().map(|&b| b as i8).collect(),
                        game_eq_type: None,
                        current_mode: None,
                    }));
                } else if attr_type == JL_SYS_ATTR_NOISE_MODE && !val.is_empty() {
                    events.push(DeviceEvent::LegacyNoiseModeUpdated { mode: val[0] });
                }
                cursor += 1 + ltv_len;
            }
        }
        _ => {}
    }

    if events.is_empty() {
        events.push(DeviceEvent::RawNotification {
            payload: frame.payload.clone(),
        });
    }
    events
}
