//! Clean-room local firmware image validation, pre-flight safety checks,
//! pure-Rust CRC helpers, WuQi tone pack `.bin` container framing, and chunked
//! OTA state machines across all 9 active QCY `otaType` routes.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::rcsp::{
    RcspFrame, OP_OTA_ENTER_UPDATE, OP_OTA_EXIT_UPDATE, OP_OTA_GET_FILE_INFO_OFFSET,
    OP_OTA_INQUIRE_CAN_UPDATE, OP_OTA_NOTIFY_FILE_SIZE, OP_OTA_REBOOT, OP_OTA_SEND_BLOCK,
};
use crate::uuids::{OtaRouteInfo, OtaType};

/// Minimum sane firmware image size (`64` bytes).
pub const MIN_FIRMWARE_BYTES: usize = 64;
/// Maximum sane firmware image size (`16 MiB`).
pub const MAX_FIRMWARE_BYTES: usize = 16 * 1024 * 1024;
/// Default minimum battery threshold required before flashing (`30%`).
pub const DEFAULT_MIN_OTA_BATTERY: u8 = 30;

/// WuQi Tone Pack `.bin` envelope magic (`0xABCD6543`, `OTAUpgrade.getBytes`).
pub const WUQI_TONE_MAGIC: u32 = 0xABCD_6543;
/// WuQi Tone Pack default flash offset (`0x00001000`).
pub const WUQI_TONE_FLASH_OFFSET: u32 = 0x0000_1000;
/// WuQi BLE OTA request prefix `[0x70, 0x07, 0x6E]`.
pub const WUQI_BLE_REQ_MAGIC: [u8; 3] = [0x70, 0x07, 0x6E];
/// WuQi BLE OTA response magic `[0x33]`.
pub const WUQI_BLE_RSP_MAGIC: u8 = 0x33;

/// Bluetrum FOTA identification magic (`FOtaPresenter`).
pub const BLUETRUM_FOTA_MAGIC: [u8; 7] = [0xCC, 0xAA, 0x55, 0xEE, 0x12, 0x19, 0xE4];
/// BES (Bestechnic) OTA magic (`"BEST"`).
pub const BES_OTA_MAGIC: [u8; 4] = *b"BEST";
/// PixArt / AWOTA TX header (`"PA"`).
pub const PIXART_TX_MAGIC: [u8; 2] = *b"PA";
/// PixArt / AWOTA RX header (`"PI"`).
pub const PIXART_RX_MAGIC: [u8; 2] = *b"PI";
/// Actions OTA knock-door handshake bytes (`"01234567"`).
pub const ACTIONS_KNOCK_DOOR: [u8; 8] = *b"01234567";
/// Actions AOTA container magic (`"AOTA"`).
pub const ACTIONS_AOTA_MAGIC: [u8; 4] = *b"AOTA";

// ---------------------------------------------------------------------------
// Errors & Pre-Flight Safety Checks
// ---------------------------------------------------------------------------

#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OtaError {
    #[error("firmware image invalid: {0}")]
    InvalidImage(String),
    #[error("pre-flight safety check failed: {0}")]
    PreFlightFailed(String),
    #[error("OTA route otaType={} requires proprietary native authentication ({reason})", ota_type.as_u8())]
    UnsupportedAuthRequired { ota_type: OtaType, reason: String },
    #[error("unexpected OTA device response: {0}")]
    ProtocolError(String),
    #[error("CRC mismatch: expected {expected:#010x}, got {actual:#010x}")]
    CrcMismatch { expected: u32, actual: u32 },
}

pub type OtaResult<T> = Result<T, OtaError>;

/// Pre-flight safety parameters verified before entering device bootloader/OTA mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OtaPreFlight {
    pub left_battery: Option<u8>,
    pub right_battery: Option<u8>,
    pub is_tws: bool,
    pub both_buds_connected: bool,
    pub min_battery_percent: u8,
}

impl OtaPreFlight {
    /// Validates TWS dual-bud connectivity and battery thresholds before starting OTA.
    pub fn validate(&self) -> OtaResult<()> {
        if self.is_tws && !self.both_buds_connected {
            return Err(OtaError::PreFlightFailed(
                "both Left and Right earbuds must be connected before flashing firmware"
                    .to_string(),
            ));
        }
        let min_bat = self.min_battery_percent;
        if let Some(l) = self.left_battery {
            if l < min_bat {
                return Err(OtaError::PreFlightFailed(format!(
                    "left/main battery too low ({l}% < {min_bat}%)"
                )));
            }
        }
        if self.is_tws {
            if let Some(r) = self.right_battery {
                if r < min_bat {
                    return Err(OtaError::PreFlightFailed(format!(
                        "right earbud battery too low ({r}% < {min_bat}%)"
                    )));
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Pure-Rust Checksums (IEEE CRC32, MPEG2 CRC32, CRC16-CCITT)
// ---------------------------------------------------------------------------

/// Standard IEEE 802.3 CRC-32 (reflected polynomial `0xEDB88320`, init `0xFFFFFFFF`, final XOR `0xFFFFFFFF`).
pub fn crc32_ieee(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if (crc & 1) != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

/// CRC-32/MPEG-2 (non-reflected polynomial `0x04C11DB7`, init `0xFFFFFFFF`, no final XOR)
/// used by PixArt/AWOTA (`T5OTAPresenter`).
pub fn crc32_mpeg2(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= (byte as u32) << 24;
        for _ in 0..8 {
            if (crc & 0x8000_0000) != 0 {
                crc = (crc << 1) ^ 0x04C1_1DB7;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

/// CRC-16/CCITT-FALSE (polynomial `0x1021`, init `0xFFFF`).
pub fn crc16_ccitt(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &byte in data {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            if (crc & 0x8000) != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

// ---------------------------------------------------------------------------
// Firmware Image Header Validation & WuQi Tone Pack Envelope
// ---------------------------------------------------------------------------

/// Detected binary container kind of a local `.ota` / `.bin` file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirmwareContainerKind {
    WuQiTonePack,
    ActionsAota,
    RawVendorBinary,
}

/// Validated metadata for a local firmware image.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirmwareImageInfo {
    pub ota_type: OtaType,
    pub size_bytes: usize,
    pub crc32_ieee: u32,
    pub container: FirmwareContainerKind,
    pub version_hint: Option<String>,
}

/// Wraps a raw WuQi white-noise tone payload inside the `OTAUpgrade.getBytes`
/// `.bin` envelope (`magic = 0xABCD6543`, `fwVer = 1`, `offset = 0x1000`,
/// `reserved = 0xFFFFFFFF`, `[len, len, 0, 0]`, `headerCrc32`, `payload`, `dataCrc32`).
pub fn wrap_wuqi_tone_pack(payload: &[u8]) -> OtaResult<Vec<u8>> {
    if payload.is_empty() {
        return Err(OtaError::InvalidImage(
            "WuQi tone payload cannot be empty".to_string(),
        ));
    }
    let len_u32 = u32::try_from(payload.len())
        .map_err(|_| OtaError::InvalidImage("WuQi tone payload exceeds u32".to_string()))?;

    let mut header = Vec::with_capacity(32);
    header.extend_from_slice(&WUQI_TONE_MAGIC.to_le_bytes());
    header.extend_from_slice(&1u32.to_le_bytes());
    header.extend_from_slice(&WUQI_TONE_FLASH_OFFSET.to_le_bytes());
    header.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
    header.extend_from_slice(&len_u32.to_le_bytes());
    header.extend_from_slice(&len_u32.to_le_bytes());
    header.extend_from_slice(&0u32.to_le_bytes());
    header.extend_from_slice(&0u32.to_le_bytes());

    let header_crc = crc32_ieee(&header);
    let data_crc = crc32_ieee(payload);

    let mut out = Vec::with_capacity(36 + payload.len() + 4);
    out.extend_from_slice(&header);
    out.extend_from_slice(&header_crc.to_le_bytes());
    out.extend_from_slice(payload);
    out.extend_from_slice(&data_crc.to_le_bytes());
    Ok(out)
}

/// Parses and CRC-verifies a wrapped WuQi tone `.bin` envelope, returning the inner payload.
pub fn parse_wuqi_tone_pack(bytes: &[u8]) -> OtaResult<&[u8]> {
    if bytes.len() < 40 {
        return Err(OtaError::InvalidImage(format!(
            "WuQi tone pack too short: {} bytes",
            bytes.len()
        )));
    }
    let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    if magic != WUQI_TONE_MAGIC {
        return Err(OtaError::InvalidImage(format!(
            "unexpected WuQi tone magic: expected {WUQI_TONE_MAGIC:#010x}, got {magic:#010x}"
        )));
    }
    let expected_hdr_crc = u32::from_le_bytes([bytes[32], bytes[33], bytes[34], bytes[35]]);
    let actual_hdr_crc = crc32_ieee(&bytes[0..32]);
    if expected_hdr_crc != actual_hdr_crc {
        return Err(OtaError::CrcMismatch {
            expected: expected_hdr_crc,
            actual: actual_hdr_crc,
        });
    }
    let payload_len = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]) as usize;
    if bytes.len() != 36 + payload_len + 4 {
        return Err(OtaError::InvalidImage(format!(
            "WuQi tone pack size mismatch: header declares {payload_len} payload bytes, total file is {} bytes",
            bytes.len()
        )));
    }
    let payload = &bytes[36..36 + payload_len];
    let crc_off = 36 + payload_len;
    let expected_data_crc = u32::from_le_bytes([
        bytes[crc_off],
        bytes[crc_off + 1],
        bytes[crc_off + 2],
        bytes[crc_off + 3],
    ]);
    let actual_data_crc = crc32_ieee(payload);
    if expected_data_crc != actual_data_crc {
        return Err(OtaError::CrcMismatch {
            expected: expected_data_crc,
            actual: actual_data_crc,
        });
    }
    Ok(payload)
}

/// Validates a local firmware binary against the selected `OtaType` route.
pub fn validate_firmware_image(ota_type: OtaType, data: &[u8]) -> OtaResult<FirmwareImageInfo> {
    if ota_type == OtaType::JieLiBleAuth {
        return Err(OtaError::UnsupportedAuthRequired {
            ota_type,
            reason: "JieLi otaType=9 requires the proprietary 17-byte RcspAuth native challenge (libjl_ota_auth.so), which is intentionally excluded from clean-room builds; otaType=2 unauthenticated JieLi BLE OTA is supported".to_string(),
        });
    }
    if data.len() < MIN_FIRMWARE_BYTES {
        return Err(OtaError::InvalidImage(format!(
            "firmware image too small ({} bytes < {MIN_FIRMWARE_BYTES})",
            data.len()
        )));
    }
    if data.len() > MAX_FIRMWARE_BYTES {
        return Err(OtaError::InvalidImage(format!(
            "firmware image too large ({} bytes > {MAX_FIRMWARE_BYTES})",
            data.len()
        )));
    }

    let (container, version_hint) = if data.len() >= 40
        && u32::from_le_bytes([data[0], data[1], data[2], data[3]]) == WUQI_TONE_MAGIC
    {
        parse_wuqi_tone_pack(data)?;
        let fw_ver = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        (
            FirmwareContainerKind::WuQiTonePack,
            Some(format!("tone-v{fw_ver}")),
        )
    } else if data.len() >= 20 && data[0..4] == ACTIONS_AOTA_MAGIC {
        let ver_slice = &data[4..20];
        let end = ver_slice
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(ver_slice.len());
        let ver = String::from_utf8_lossy(&ver_slice[..end])
            .trim()
            .to_string();
        (
            FirmwareContainerKind::ActionsAota,
            if ver.is_empty() { None } else { Some(ver) },
        )
    } else if ota_type == OtaType::ActionsSpp {
        return Err(OtaError::InvalidImage(
            "Actions SPP firmware image must start with \"AOTA\" magic header".to_string(),
        ));
    } else {
        (FirmwareContainerKind::RawVendorBinary, None)
    };

    Ok(FirmwareImageInfo {
        ota_type,
        size_bytes: data.len(),
        crc32_ieee: crc32_ieee(data),
        container,
        version_hint,
    })
}

// ---------------------------------------------------------------------------
// Unified Chunked OTA Transfer State Machine
// ---------------------------------------------------------------------------

/// High-level OTA transfer phase reported to UI/orchestrator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum OtaPhase {
    Ready,
    Handshaking,
    NegotiatingOffset,
    Transferring {
        bytes_sent: usize,
        total_bytes: usize,
    },
    Verifying,
    Rebooting,
    Completed,
    UnsupportedAuthRequired {
        ota_type: OtaType,
        reason: String,
    },
}

/// Pure-Rust OTA session framing and chunking state machine covering all supported routes.
#[derive(Debug, Clone)]
pub struct OtaSession {
    ota_type: OtaType,
    route: OtaRouteInfo,
    image: Vec<u8>,
    image_info: FirmwareImageInfo,
    chunk_size: usize,
    offset: usize,
    seq: u8,
    phase: OtaPhase,
}

impl OtaSession {
    /// Creates a new validated OTA session. Returns `Err(OtaError::UnsupportedAuthRequired)`
    /// transparently for `OtaType::JieLiBleAuth` (`otaType = 9`).
    pub fn new(ota_type: OtaType, image: Vec<u8>, mtu: usize) -> OtaResult<Self> {
        let image_info = validate_firmware_image(ota_type, &image)?;
        let route = ota_type.route_info();
        let chunk_size = match ota_type {
            OtaType::PixArtAwota => 248,
            OtaType::QualcommGaia => 873,
            OtaType::ActionsSpp => 256,
            OtaType::BluetrumFota => 240,
            OtaType::JieLiBleUnauth => mtu.saturating_sub(16).clamp(64, 240),
            OtaType::WuQiBle | OtaType::WuQiBleTone | OtaType::BesBle => {
                mtu.saturating_sub(12).clamp(64, 496)
            }
            OtaType::JieLiBleAuth => unreachable!("rejected in validate_firmware_image"),
        };

        Ok(Self {
            ota_type,
            route,
            image,
            image_info,
            chunk_size,
            offset: 0,
            seq: 0,
            phase: OtaPhase::Ready,
        })
    }

    pub fn ota_type(&self) -> OtaType {
        self.ota_type
    }

    pub fn route_info(&self) -> OtaRouteInfo {
        self.route
    }

    pub fn image_info(&self) -> &FirmwareImageInfo {
        &self.image_info
    }

    pub fn chunk_size(&self) -> usize {
        self.chunk_size
    }

    pub fn phase(&self) -> &OtaPhase {
        &self.phase
    }

    pub fn progress_percent(&self) -> f32 {
        if self.image.is_empty() {
            100.0
        } else {
            (self.offset.min(self.image.len()) as f32 * 100.0) / (self.image.len() as f32)
        }
    }

    /// Sets resume offset (used when device reports resume point via `GetOffset` / `0xE1`).
    pub fn set_resume_offset(&mut self, offset: usize) -> OtaResult<()> {
        if offset > self.image.len() {
            return Err(OtaError::ProtocolError(format!(
                "resume offset {offset} exceeds image length {}",
                self.image.len()
            )));
        }
        self.offset = offset;
        self.phase = OtaPhase::Transferring {
            bytes_sent: self.offset,
            total_bytes: self.image.len(),
        };
        Ok(())
    }

    fn next_seq(&mut self, modulo: u8) -> u8 {
        let cur = self.seq;
        self.seq = if modulo == 0 {
            self.seq.wrapping_add(1)
        } else {
            (self.seq + 1) % modulo
        };
        cur
    }

    /// Builds the initial vendor handshake / enter-OTA packet(s) for this route.
    pub fn build_start_packets(&mut self) -> OtaResult<Vec<Vec<u8>>> {
        self.phase = OtaPhase::Handshaking;
        let total_u32 = self.image.len() as u32;
        let crc_u32 = self.image_info.crc32_ieee;

        let packets = match self.ota_type {
            // WuQi BLE OTA (0x7033): [0x70, 0x07, 0x6E, seq(0..127), cmd, len_lo, len_hi, payload...]
            OtaType::WuQiBle | OtaType::WuQiBleTone => {
                let s0 = self.next_seq(128);
                let s1 = self.next_seq(128);
                vec![
                    build_wuqi_packet(s0, 0x01, &[]),                      // CheckState
                    build_wuqi_packet(s1, 0x02, &total_u32.to_le_bytes()), // EnterUpgradeMode
                ]
            }
            // Bluetrum FOTA (0xFF12): Identification magic + GET_INFO (0x91) + OTA_INFO (0xA0)
            OtaType::BluetrumFota => {
                let s0 = self.next_seq(0);
                let s1 = self.next_seq(0);
                let mut ota_info = Vec::with_capacity(8);
                ota_info.extend_from_slice(&total_u32.to_le_bytes());
                ota_info.extend_from_slice(&crc_u32.to_le_bytes());
                vec![
                    BLUETRUM_FOTA_MAGIC.to_vec(),
                    build_bluetrum_packet(s0, 0x91, &[]),
                    build_bluetrum_packet(s1, 0xA0, &ota_info),
                ]
            }
            // BES BLE OTA (66666666-...): "BEST" magic + 0x80 start metadata
            OtaType::BesBle => {
                let mut pkt = Vec::with_capacity(13);
                pkt.push(0x80);
                pkt.extend_from_slice(&BES_OTA_MAGIC);
                pkt.extend_from_slice(&total_u32.to_le_bytes());
                pkt.extend_from_slice(&crc_u32.to_le_bytes());
                vec![pkt]
            }
            // JieLi Unauth BLE OTA (0xA002): RCSP 0xE1 GetFileOffset -> 0xE2 InquireCanUpdate -> 0xE3 EnterUpdate
            OtaType::JieLiBleUnauth => {
                let s0 = self.next_seq(0);
                let s1 = self.next_seq(0);
                let s2 = self.next_seq(0);
                let s3 = self.next_seq(0);
                vec![
                    RcspFrame::command(OP_OTA_GET_FILE_INFO_OFFSET, true, s0, &[])
                        .encode()
                        .map_err(|e| OtaError::ProtocolError(e.to_string()))?,
                    RcspFrame::command(OP_OTA_INQUIRE_CAN_UPDATE, true, s1, &[0x00])
                        .encode()
                        .map_err(|e| OtaError::ProtocolError(e.to_string()))?,
                    RcspFrame::command(OP_OTA_ENTER_UPDATE, true, s2, &[])
                        .encode()
                        .map_err(|e| OtaError::ProtocolError(e.to_string()))?,
                    RcspFrame::command(OP_OTA_NOTIFY_FILE_SIZE, true, s3, &total_u32.to_be_bytes())
                        .encode()
                        .map_err(|e| OtaError::ProtocolError(e.to_string()))?,
                ]
            }
            // PixArt / AWOTA (Classic SPP): "PA" + CT_06_OTA (0x06) + OTA_MODE (0x30) + SET_LOAD_INFO (0x32)
            OtaType::PixArtAwota => {
                let mut info = Vec::with_capacity(8);
                info.extend_from_slice(&total_u32.to_le_bytes());
                info.extend_from_slice(&crc32_mpeg2(&self.image).to_le_bytes());
                vec![
                    build_pixart_packet(0x30, &[]),
                    build_pixart_packet(0x32, &info),
                ]
            }
            // Qualcomm GAIA (Classic RFCOMM): Upgrade CONNECT command (vendor 0x000A, cmd 0x0640)
            OtaType::QualcommGaia => {
                vec![build_gaia_v1_packet(0x000A, 0x0640, &[])]
            }
            // Actions (Classic SPP): "01234567" knockDoor + super-TLV 0x80 init
            OtaType::ActionsSpp => {
                let mut init = vec![0x80, 0x01];
                init.extend_from_slice(&total_u32.to_le_bytes());
                vec![ACTIONS_KNOCK_DOOR.to_vec(), init]
            }
            OtaType::JieLiBleAuth => unreachable!(),
        };

        Ok(packets)
    }

    /// Produces the next chunked firmware data packet, advancing `offset`, or `None` once all bytes are sent.
    pub fn next_data_packet(&mut self) -> OtaResult<Option<Vec<u8>>> {
        if self.offset >= self.image.len() {
            return Ok(None);
        }
        let end = (self.offset + self.chunk_size).min(self.image.len());
        let chunk = self.image[self.offset..end].to_vec();
        let chunk_offset = self.offset;
        self.offset = end;
        self.phase = OtaPhase::Transferring {
            bytes_sent: self.offset,
            total_bytes: self.image.len(),
        };

        let pkt = match self.ota_type {
            OtaType::WuQiBle | OtaType::WuQiBleTone => {
                let seq = self.next_seq(128);
                let mut body = Vec::with_capacity(4 + chunk.len());
                body.extend_from_slice(&(chunk_offset as u32).to_le_bytes());
                body.extend_from_slice(&chunk);
                build_wuqi_packet(seq, 0x04, &body)
            }
            OtaType::BluetrumFota => {
                let seq = self.next_seq(0);
                let mut body = Vec::with_capacity(4 + chunk.len());
                body.extend_from_slice(&(chunk_offset as u32).to_le_bytes());
                body.extend_from_slice(&chunk);
                build_bluetrum_packet(seq, 0x20, &body)
            }
            OtaType::BesBle => {
                let mut pkt = Vec::with_capacity(1 + chunk.len());
                pkt.push(0x85);
                pkt.extend_from_slice(&chunk);
                pkt
            }
            OtaType::JieLiBleUnauth => {
                let seq = self.next_seq(0);
                RcspFrame::response(OP_OTA_SEND_BLOCK, 0x00, seq, &chunk)
                    .encode()
                    .map_err(|e| OtaError::ProtocolError(e.to_string()))?
            }
            OtaType::PixArtAwota => build_pixart_packet(0x33, &chunk), // LOAD_MORE
            OtaType::QualcommGaia => {
                // GAIA Upgrade CONTROL (0x0642) DATA opcode (0x04) + last_packet flag
                let is_last = u8::from(self.offset >= self.image.len());
                let mut payload = Vec::with_capacity(4 + chunk.len());
                payload.push(0x04);
                payload.extend_from_slice(&((1 + chunk.len()) as u16).to_be_bytes());
                payload.push(is_last);
                payload.extend_from_slice(&chunk);
                build_gaia_v1_packet(0x000A, 0x0642, &payload)
            }
            OtaType::ActionsSpp => {
                let mut pkt = Vec::with_capacity(7 + chunk.len());
                pkt.push(0x80);
                pkt.push(0x02);
                pkt.extend_from_slice(&(chunk_offset as u32).to_le_bytes());
                pkt.push(chunk.len() as u8);
                pkt.extend_from_slice(&chunk);
                pkt
            }
            OtaType::JieLiBleAuth => unreachable!(),
        };

        Ok(Some(pkt))
    }

    /// Responds to a JieLi device-pulled block request (`0xE5` specifying `[offset_be_u32, len_be_u16]`).
    pub fn build_jieli_pulled_block(
        &mut self,
        offset: usize,
        len: usize,
        sn: u8,
    ) -> OtaResult<Vec<u8>> {
        if self.ota_type != OtaType::JieLiBleUnauth {
            return Err(OtaError::ProtocolError(
                "device-pulled block helper is only valid for JieLi otaType=2".to_string(),
            ));
        }
        if offset > self.image.len() {
            return Err(OtaError::ProtocolError(format!(
                "requested offset {offset} beyond image len {}",
                self.image.len()
            )));
        }
        let end = (offset + len).min(self.image.len());
        self.offset = end;
        self.phase = OtaPhase::Transferring {
            bytes_sent: self.offset,
            total_bytes: self.image.len(),
        };
        RcspFrame::response(OP_OTA_SEND_BLOCK, 0x00, sn, &self.image[offset..end])
            .encode()
            .map_err(|e| OtaError::ProtocolError(e.to_string()))
    }

    /// Builds final verification / commit / reboot packet(s) once all firmware bytes have been sent.
    pub fn build_finish_packets(&mut self) -> OtaResult<Vec<Vec<u8>>> {
        self.phase = OtaPhase::Verifying;
        let crc_ieee = self.image_info.crc32_ieee;
        let packets = match self.ota_type {
            OtaType::WuQiBle | OtaType::WuQiBleTone => {
                let s0 = self.next_seq(128);
                let s1 = self.next_seq(128);
                vec![
                    build_wuqi_packet(s0, 0x05, &crc_ieee.to_le_bytes()), // TransferEnd
                    build_wuqi_packet(s1, 0x07, &[]),                     // ResetDevice
                ]
            }
            OtaType::BluetrumFota => {
                let s0 = self.next_seq(0);
                vec![build_bluetrum_packet(s0, 0x90, &crc_ieee.to_le_bytes())]
            }
            OtaType::BesBle => {
                let mut crc_check = vec![0x82];
                crc_check.extend_from_slice(&crc_ieee.to_le_bytes());
                let mut confirm = vec![0x92];
                confirm.extend_from_slice(&BES_OTA_MAGIC);
                vec![crc_check, confirm, vec![0x90, 0x11]]
            }
            OtaType::JieLiBleUnauth => {
                let s0 = self.next_seq(0);
                let s1 = self.next_seq(0);
                vec![
                    RcspFrame::command(OP_OTA_REBOOT, true, s0, &[0x00])
                        .encode()
                        .map_err(|e| OtaError::ProtocolError(e.to_string()))?,
                    RcspFrame::command(OP_OTA_EXIT_UPDATE, true, s1, &[])
                        .encode()
                        .map_err(|e| OtaError::ProtocolError(e.to_string()))?,
                ]
            }
            OtaType::PixArtAwota => {
                let crc_mpeg = crc32_mpeg2(&self.image);
                vec![
                    build_pixart_packet(0x34, &crc_mpeg.to_le_bytes()), // CALC_CRC32
                    build_pixart_packet(0x35, &[]),                     // UPDATE
                    build_pixart_packet(0x3E, &[]),                     // REBOOT
                ]
            }
            OtaType::QualcommGaia => vec![
                // COMMIT_CFM (0x0F) + DISCONNECT (0x0641)
                build_gaia_v1_packet(0x000A, 0x0642, &[0x0F, 0x00, 0x01, 0x00]),
                build_gaia_v1_packet(0x000A, 0x0641, &[]),
            ],
            OtaType::ActionsSpp => {
                let mut finish = vec![0x80, 0x03];
                finish.extend_from_slice(&crc_ieee.to_le_bytes());
                vec![finish]
            }
            OtaType::JieLiBleAuth => unreachable!(),
        };
        self.phase = OtaPhase::Rebooting;
        Ok(packets)
    }

    pub fn mark_completed(&mut self) {
        self.phase = OtaPhase::Completed;
    }
}

/// Helper computing missing chunk indices from an Actions OTA bitmap (`0` bit = missing).
pub fn actions_missing_chunk_indices(bitmap: &[u8], total_chunks: usize) -> Vec<usize> {
    let mut missing = Vec::new();
    for idx in 0..total_chunks {
        let byte_idx = idx / 8;
        let bit_idx = idx % 8;
        let bit_set = bitmap
            .get(byte_idx)
            .map(|&b| ((b >> bit_idx) & 1) != 0)
            .unwrap_or(false);
        if !bit_set {
            missing.push(idx);
        }
    }
    missing
}

// ---------------------------------------------------------------------------
// Vendor Low-Level Packet Builders
// ---------------------------------------------------------------------------

fn build_wuqi_packet(seq: u8, cmd: u8, payload: &[u8]) -> Vec<u8> {
    let len_le = (payload.len() as u16).to_le_bytes();
    let mut out = Vec::with_capacity(7 + payload.len());
    out.extend_from_slice(&WUQI_BLE_REQ_MAGIC);
    out.push(seq & 0x7F);
    out.push(cmd);
    out.extend_from_slice(&len_le);
    out.extend_from_slice(payload);
    out
}

fn build_bluetrum_packet(seq: u8, cmd: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + payload.len());
    out.push(seq);
    out.push(cmd);
    out.push((payload.len() & 0xFF) as u8);
    out.extend_from_slice(payload);
    out
}

fn build_pixart_packet(cmd: u8, payload: &[u8]) -> Vec<u8> {
    // "PA" + CT_06_OTA (0x06) + cmd + len_le_u16 + payload + crc16_ccitt_le
    let len_le = (payload.len() as u16).to_le_bytes();
    let mut out = Vec::with_capacity(8 + payload.len());
    out.extend_from_slice(&PIXART_TX_MAGIC);
    out.push(0x06);
    out.push(cmd);
    out.extend_from_slice(&len_le);
    out.extend_from_slice(payload);
    let crc = crc16_ccitt(&out[2..]);
    out.extend_from_slice(&crc.to_le_bytes());
    out
}

fn build_gaia_v1_packet(vendor_id: u16, command_id: u16, payload: &[u8]) -> Vec<u8> {
    // GAIA v1 RFCOMM frame: [0xFF, 0x01, flags=0x00, payload_len, vendor_be_u16, cmd_be_u16, payload...]
    let mut out = Vec::with_capacity(8 + payload.len());
    out.push(0xFF);
    out.push(0x01);
    out.push(0x00);
    out.push((payload.len() & 0xFF) as u8);
    out.extend_from_slice(&vendor_id.to_be_bytes());
    out.extend_from_slice(&command_id.to_be_bytes());
    out.extend_from_slice(payload);
    out
}
