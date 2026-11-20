//! BLE 24-byte Manufacturer-Specific Data parser/encoder and 16-bit `modelID`
//! chipset / form-factor classifier.

use serde::{Deserialize, Serialize};

use crate::types::ProtocolFamily;
use crate::uuids::{is_qcy_company_id, OtaType};
use crate::{ProtocolError, ProtocolResult};

/// Decoded single-cell battery reading from BLE advertising `[5..7]` or TLV `0x2F`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BatteryLevel {
    /// Percentage `1..=100`, or `None` when absent/disconnected (`0` or `0x7F`).
    pub percent: Option<u8>,
    /// `true` when bit 7 (`0x80`) is set.
    pub charging: bool,
}

impl BatteryLevel {
    /// Decodes a single QCY packed battery byte (`bit7` = charging, `bits0..6` = percentage).
    pub const fn from_wire_byte(raw: u8) -> Self {
        let charging = (raw & 0x80) != 0;
        let level = raw & 0x7F;
        let percent = if level == 0 || level == 0x7F {
            None
        } else if level > 100 {
            Some(100)
        } else {
            Some(level)
        };
        Self { percent, charging }
    }

    /// Encodes a `BatteryLevel` back into its QCY packed wire byte.
    pub const fn to_wire_byte(self) -> u8 {
        let base = match self.percent {
            Some(pct) => {
                if pct > 100 {
                    100
                } else {
                    pct
                }
            }
            None => 0x00,
        };
        if self.charging {
            base | 0x80
        } else {
            base
        }
    }
}

/// Converts a 6-byte swapped QCY BLE advertisement MAC slice (`[b0,b1,b2,b3,b4,b5]`)
/// into a canonical big-endian MAC address (`[b1,b0,b2,b5,b4,b3]`).
///
/// This permutation is an involution (`unswap_adv_mac(unswap_adv_mac(x)) == x`).
pub const fn unswap_adv_mac(raw: [u8; 6]) -> [u8; 6] {
    [raw[1], raw[0], raw[2], raw[5], raw[4], raw[3]]
}

/// Encodes a canonical big-endian 6-byte MAC address into the swapped QCY BLE
/// advertisement byte order (`[b1,b0,b2,b5,b4,b3]`).
pub const fn swap_adv_mac(mac: [u8; 6]) -> [u8; 6] {
    unswap_adv_mac(mac)
}

/// Formats a 6-byte big-endian MAC address as uppercase `AA:BB:CC:DD:EE:FF`.
pub fn format_mac(mac: [u8; 6]) -> String {
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    )
}

/// Decoded QCY BLE Manufacturer-Specific Data payload (up to 24 bytes after the 2-byte Company ID).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QcyAdvertisement {
    /// Bluetooth SIG Company ID (`0x071E`, `0x521C`, or `0x522C`).
    pub company_id: u16,
    /// 16-bit Big-Endian `modelID` from bytes `[0..1]`.
    pub model_id: u16,
    /// Reserved byte `[2]` preserved verbatim for lossless round-trips.
    pub reserved_byte2: u8,
    /// Primary advertising earbud side (`byte[3] & 0x01 != 0`).
    pub is_left: bool,
    /// Device colorway index (`(byte[3] >> 1) & 0x07`).
    pub color_index: u8,
    /// Whether earbuds are paired as TWS (`byte[4] & 0x40 != 0`).
    pub is_tws: bool,
    /// Whether the popup animation trigger bitmask is set (`byte[4] & 0x22 != 0`).
    pub is_pop: bool,
    /// Extra bits of byte `[4]` outside `0x62` preserved on round-trip.
    pub raw_flag_byte4: u8,
    /// Left earbud (or main unit) battery status from byte `[5]`.
    pub left_battery: BatteryLevel,
    /// Right earbud battery status from byte `[6]`.
    pub right_battery: BatteryLevel,
    /// Charging case battery status from byte `[7]`.
    pub case_battery: BatteryLevel,
    /// Reserved bytes `[8..11]` preserved verbatim.
    pub reserved_8_10: [u8; 3],
    /// Unswapped Primary Classic Bluetooth MAC from bytes `[11..17]` (when `len >= 17`).
    pub class_mac: Option<[u8; 6]>,
    /// Rolling case lid-open deduplication counter from byte `[17]` (when `len >= 18`).
    pub pop_num: Option<u8>,
    /// Unswapped Secondary/Peer Classic Bluetooth MAC from bytes `[18..24]` (when `len >= 24`).
    pub other_mac: Option<[u8; 6]>,
}

impl QcyAdvertisement {
    /// Minimum payload length containing model ID, flags, and battery trio (`8` bytes).
    pub const MIN_LEN: usize = 8;
    /// Full payload length including both Classic MAC addresses and `pop_num` (`24` bytes).
    pub const FULL_LEN: usize = 24;

    /// Parses a QCY BLE Manufacturer-Specific Data payload (without the 2-byte Company ID prefix).
    pub fn parse(company_id: u16, data: &[u8]) -> ProtocolResult<Self> {
        if !is_qcy_company_id(company_id) {
            return Err(ProtocolError::MalformedPacket(format!(
                "unrecognized QCY company ID: {company_id:#06x}"
            )));
        }
        if data.len() < Self::MIN_LEN {
            return Err(ProtocolError::MalformedPacket(format!(
                "QCY manufacturer data too short: expected >= {} bytes, got {}",
                Self::MIN_LEN,
                data.len()
            )));
        }

        let model_id = u16::from_be_bytes([data[0], data[1]]);
        let reserved_byte2 = data[2];
        let is_left = (data[3] & 0x01) != 0;
        let color_index = (data[3] >> 1) & 0x07;
        let raw_flag_byte4 = data[4];
        let is_tws = (raw_flag_byte4 & 0x40) != 0;
        let is_pop = (raw_flag_byte4 & 0x22) != 0;

        let left_battery = BatteryLevel::from_wire_byte(data[5]);
        let right_battery = BatteryLevel::from_wire_byte(data[6]);
        let case_battery = BatteryLevel::from_wire_byte(data[7]);

        let mut reserved_8_10 = [0u8; 3];
        if data.len() >= 11 {
            reserved_8_10.copy_from_slice(&data[8..11]);
        }

        let class_mac = if data.len() >= 17 {
            let mut raw = [0u8; 6];
            raw.copy_from_slice(&data[11..17]);
            Some(unswap_adv_mac(raw))
        } else {
            None
        };

        let pop_num = if data.len() >= 18 {
            Some(data[17])
        } else {
            None
        };

        let other_mac = if data.len() >= 24 {
            let mut raw = [0u8; 6];
            raw.copy_from_slice(&data[18..24]);
            Some(unswap_adv_mac(raw))
        } else {
            None
        };

        Ok(Self {
            company_id,
            model_id,
            reserved_byte2,
            is_left,
            color_index,
            is_tws,
            is_pop,
            raw_flag_byte4,
            left_battery,
            right_battery,
            case_battery,
            reserved_8_10,
            class_mac,
            pop_num,
            other_mac,
        })
    }

    /// Encodes this advertisement into QCY BLE Manufacturer-Specific Data bytes
    /// (excluding the 2-byte Company ID). Produces 24 bytes when `other_mac` is set,
    /// 18 bytes when `pop_num` is set, 17 bytes when `class_mac` is set, or 8 bytes otherwise.
    pub fn encode(&self) -> Vec<u8> {
        let model_be = self.model_id.to_be_bytes();
        let byte3 = (u8::from(self.is_left)) | ((self.color_index & 0x07) << 1);
        let mut byte4 = self.raw_flag_byte4 & !0x62;
        if self.is_tws {
            byte4 |= 0x40;
        }
        if self.is_pop {
            let preserved_pop = self.raw_flag_byte4 & 0x22;
            byte4 |= if preserved_pop != 0 {
                preserved_pop
            } else {
                0x20
            };
        }

        let mut out = Vec::with_capacity(Self::FULL_LEN);
        out.extend_from_slice(&[
            model_be[0],
            model_be[1],
            self.reserved_byte2,
            byte3,
            byte4,
            self.left_battery.to_wire_byte(),
            self.right_battery.to_wire_byte(),
            self.case_battery.to_wire_byte(),
        ]);

        if self.class_mac.is_some() || self.pop_num.is_some() || self.other_mac.is_some() {
            out.extend_from_slice(&self.reserved_8_10);
            let swapped_primary = swap_adv_mac(self.class_mac.unwrap_or([0u8; 6]));
            out.extend_from_slice(&swapped_primary);
        }

        if self.pop_num.is_some() || self.other_mac.is_some() {
            out.push(self.pop_num.unwrap_or(0));
        }

        if let Some(peer) = self.other_mac {
            out.extend_from_slice(&swap_adv_mac(peer));
        }

        out
    }

    /// Classifies the chipset family from `model_id`.
    pub const fn chipset(&self) -> ChipsetFamily {
        classify_chipset(self.model_id)
    }

    /// Classifies the form factor hint from `model_id`.
    pub const fn form_factor_hint(&self) -> DeviceFormFactorHint {
        classify_form_factor(self.model_id)
    }

    /// Returns the expected daily-control protocol family from `model_id`.
    pub const fn expected_protocol_family(&self) -> ProtocolFamily {
        classify_protocol_family(self.model_id)
    }
}

// ---------------------------------------------------------------------------
// 16-bit modelID Bitmask Classifiers (QCYDeviceBean Parity)
// ---------------------------------------------------------------------------

/// Hardware chipset families decoded from QCY's 16-bit `modelID` bitmasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChipsetFamily {
    QualcommQcc,
    WuQi,
    Bluetrum,
    JieLiLegacy,
    JieLiNew,
    Zr,
    Rd,
    Realtek,
    Bes,
    Unknown,
}

impl ChipsetFamily {
    /// Returns the typical default `OtaType` for this chipset family, if statically known.
    pub const fn default_ota_type(self) -> Option<OtaType> {
        match self {
            Self::QualcommQcc => Some(OtaType::QualcommGaia),
            Self::WuQi => Some(OtaType::WuQiBleTone),
            Self::Bluetrum => Some(OtaType::BluetrumFota),
            Self::JieLiLegacy | Self::JieLiNew => Some(OtaType::JieLiBleUnauth),
            Self::Bes => Some(OtaType::BesBle),
            Self::Zr | Self::Rd | Self::Realtek | Self::Unknown => None,
        }
    }
}

/// Form factor bitmask hint encoded in `modelID & 0x0F00`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceFormFactorHint {
    Earbuds,
    Headwear,
    Speaker,
}

pub const fn is_headwear(model_id: u16) -> bool {
    (model_id & 0x0F00) == 0x0100
}

pub const fn is_speaker(model_id: u16) -> bool {
    (model_id & 0x0F00) == 0x0300
}

pub const fn is_qcc_device(model_id: u16) -> bool {
    (model_id & 0x00F0) == 0x0010 || model_id == 0x4D17
}

pub const fn is_wq_device(model_id: u16) -> bool {
    (model_id & 0x00F0) == 0x0040 || model_id == 0x0308
}

pub const fn is_bluetrum_device(model_id: u16) -> bool {
    (model_id & 0x00F0) == 0x0050
}

pub const fn is_jl_device(model_id: u16) -> bool {
    (model_id & 0x00F0) == 0x0070 || model_id == 0x0200
}

pub const fn is_new_jl(model_id: u16) -> bool {
    (model_id & 0x8070) == 0x8070
}

pub const fn is_zr_device(model_id: u16) -> bool {
    (model_id & 0x00F0) == 0x0090
}

pub const fn is_rd_device(model_id: u16) -> bool {
    (model_id & 0x00F0) == 0x00A0
}

pub const fn is_rtk_device(model_id: u16) -> bool {
    (model_id & 0x00F0) == 0x00C0
}

pub const fn is_bes_device(model_id: u16) -> bool {
    (model_id & 0x00F0) == 0x00D0
}

pub const fn is_hq3710(model_id: u16) -> bool {
    (model_id & 0xFFF0) == 0x4BC0
}

pub const fn is_t11s(model_id: u16) -> bool {
    model_id == 0x4D17
}

/// Decodes the chipset family from a 16-bit QCY `modelID`.
pub const fn classify_chipset(model_id: u16) -> ChipsetFamily {
    if is_new_jl(model_id) {
        ChipsetFamily::JieLiNew
    } else if is_jl_device(model_id) {
        ChipsetFamily::JieLiLegacy
    } else if is_wq_device(model_id) {
        ChipsetFamily::WuQi
    } else if is_bluetrum_device(model_id) {
        ChipsetFamily::Bluetrum
    } else if is_bes_device(model_id) {
        ChipsetFamily::Bes
    } else if is_qcc_device(model_id) {
        ChipsetFamily::QualcommQcc
    } else if is_zr_device(model_id) {
        ChipsetFamily::Zr
    } else if is_rd_device(model_id) {
        ChipsetFamily::Rd
    } else if is_rtk_device(model_id) {
        ChipsetFamily::Realtek
    } else {
        ChipsetFamily::Unknown
    }
}

/// Decodes the form factor hint from `modelID & 0x0F00`.
pub const fn classify_form_factor(model_id: u16) -> DeviceFormFactorHint {
    if is_headwear(model_id) {
        DeviceFormFactorHint::Headwear
    } else if is_speaker(model_id) {
        DeviceFormFactorHint::Speaker
    } else {
        DeviceFormFactorHint::Earbuds
    }
}

/// Predicts the daily-control `ProtocolFamily` solely from `modelID`.
pub const fn classify_protocol_family(model_id: u16) -> ProtocolFamily {
    if is_new_jl(model_id) {
        ProtocolFamily::JieLiRcspNew
    } else if is_jl_device(model_id) {
        ProtocolFamily::JieLiRcspLegacy
    } else {
        ProtocolFamily::QcyUnifiedTlv
    }
}

/// Exact decision tree matching `QCYDeviceBean.isSupportMultiEQ()` (Section 4.1).
pub const fn supports_multi_eq(
    model_id: u16,
    has_legacy_eq_char: bool,
    support_flag: Option<u8>,
) -> bool {
    if let Some(flag) = support_flag {
        if (flag & 0x03) != 0 {
            return true;
        }
    }
    if matches!(model_id, 0x1972 | 0x4173 | 0x41D1) {
        return true;
    }
    if is_new_jl(model_id) {
        return true;
    }
    if is_jl_device(model_id) {
        return false;
    }
    !has_legacy_eq_char
}
