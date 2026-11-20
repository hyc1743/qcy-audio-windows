//! Clean-room Bluetooth UUIDs, company IDs, OUI prefixes, OTA route metadata,
//! and the complete QCY device command ID catalog.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Bluetooth SIG Base UUID (`00000000-0000-1000-8000-00805F9B34FB`).
const BLUETOOTH_BASE_UUID_U128: u128 = 0x0000_0000_0000_1000_8000_0080_5F9B_34FB;

/// Expands a 16-bit Bluetooth short UUID into its canonical 128-bit UUID.
pub const fn short_uuid(short_id: u16) -> Uuid {
    Uuid::from_u128(BLUETOOTH_BASE_UUID_U128 | ((short_id as u128) << 96))
}

// ---------------------------------------------------------------------------
// BLE Advertising Company IDs & Classic A2DP OUI Prefixes
// ---------------------------------------------------------------------------

/// Current QCY Bluetooth SIG Manufacturer / Company ID (`0x071E`, 1822).
pub const COMPANY_ID_QCY: u16 = 0x071E;
/// Legacy QCY Manufacturer / Company ID (`0x521C`, 21020).
pub const COMPANY_ID_QCY_OLD: u16 = 0x521C;
/// Custom / OEM QCY Manufacturer / Company ID (`0x522C`, 21036).
pub const COMPANY_ID_QCY_CUSTOM: u16 = 0x522C;

/// All recognized QCY BLE advertising manufacturer company IDs.
pub const QCY_COMPANY_IDS: [u16; 3] = [COMPANY_ID_QCY, COMPANY_ID_QCY_OLD, COMPANY_ID_QCY_CUSTOM];

/// Returns `true` if the Bluetooth SIG company ID matches a known QCY advertising ID.
pub const fn is_qcy_company_id(company_id: u16) -> bool {
    matches!(
        company_id,
        COMPANY_ID_QCY | COMPANY_ID_QCY_OLD | COMPANY_ID_QCY_CUSTOM
    )
}

/// Known Classic Bluetooth A2DP OUI prefixes assigned to QCY / Hele Electronics devices.
pub const QCY_CLASSIC_OUIS: [[u8; 3]; 3] =
    [[0x84, 0xAC, 0x60], [0x1C, 0x52, 0x16], [0xC4, 0x33, 0x96]];

/// Returns `true` if a 6-byte big-endian Bluetooth MAC matches a known QCY OUI prefix.
pub const fn is_qcy_classic_oui(mac: [u8; 6]) -> bool {
    let prefix = [mac[0], mac[1], mac[2]];
    (prefix[0] == 0x84 && prefix[1] == 0xAC && prefix[2] == 0x60)
        || (prefix[0] == 0x1C && prefix[1] == 0x52 && prefix[2] == 0x16)
        || (prefix[0] == 0xC4 && prefix[1] == 0x33 && prefix[2] == 0x96)
}

// ---------------------------------------------------------------------------
// Connection, MTU & Pacing Constants
// ---------------------------------------------------------------------------

/// Preferred BLE ATT MTU requested by QCY devices for daily control (`512` bytes).
pub const DEFAULT_BLE_MTU: usize = 512;
/// Preferred BLE ATT MTU requested during JieLi OTA (`256` bytes).
pub const JIELI_OTA_MTU: usize = 256;
/// Minimum inter-command pacing interval enforced between sequential GATT writes (`100 ms`).
pub const COMMAND_PACING_MS: u64 = 100;

// ---------------------------------------------------------------------------
// Daily Control BLE GATT UUIDs
// ---------------------------------------------------------------------------

/// QCY Unified Custom TLV primary GATT service (`0xA001`).
pub const SVC_QCY_CUSTOM: Uuid = short_uuid(0xA001);
/// QCY Unified Custom TLV host-to-device write characteristic (`0x1001`).
pub const CHAR_QCY_WRITE: Uuid = short_uuid(0x1001);
/// QCY Unified Custom TLV device-to-host notify characteristic (`0x1002`).
pub const CHAR_QCY_NOTIFY: Uuid = short_uuid(0x1002);

/// Legacy direct GATT characteristic for 3-byte firmware version (`0x0007`).
pub const CHAR_LEGACY_VERSION: Uuid = short_uuid(0x0007);
/// Legacy direct GATT characteristic for 3-byte battery level (`0x0008`).
pub const CHAR_LEGACY_BATTERY: Uuid = short_uuid(0x0008);
/// Legacy direct GATT characteristic for 10-band / 5-band graphic EQ (`0x000B`).
pub const CHAR_LEGACY_EQ: Uuid = short_uuid(0x000B);
/// Legacy direct GATT characteristic for button remapping (`0x000D`).
pub const CHAR_LEGACY_BUTTON: Uuid = short_uuid(0x000D);

/// QCY-customized JieLi RCSP primary GATT service (`0xA002`).
pub const SVC_JIELI_RCSP: Uuid = short_uuid(0xA002);
/// QCY-customized JieLi RCSP command write characteristic (`0x0001`).
pub const CHAR_JIELI_WRITE: Uuid = short_uuid(0x0001);
/// QCY-customized JieLi RCSP notification characteristic (`0x0002`).
pub const CHAR_JIELI_NOTIFY: Uuid = short_uuid(0x0002);

/// Standard GATT Client Characteristic Configuration Descriptor (`0x2902`).
pub const DESC_CCCD: Uuid = short_uuid(0x2902);

// ---------------------------------------------------------------------------
// Firmware OTA GATT & Classic RFCOMM UUIDs (All 9 Active otaType Routes)
// ---------------------------------------------------------------------------

/// Standard Bluetooth Classic Serial Port Profile (SPP) UUID (`0x1101`, PixArt/AWOTA `otaType=1` & fallback).
pub const SVC_SPP_STANDARD: Uuid = short_uuid(0x1101);

/// Qualcomm GAIA Classic RFCOMM service UUID (`otaType=5`).
pub const SVC_QUALCOMM_GAIA: Uuid = Uuid::from_u128(0x0000_1107_D102_11E1_9B23_0002_5B00_A5A5);

/// WuQi BLE OTA GATT service UUID (`0x7033`, `otaType=6` & `otaType=11`).
pub const SVC_WUQI_OTA: Uuid = short_uuid(0x7033);
/// WuQi BLE OTA write characteristic (`0x2001`).
pub const CHAR_WUQI_OTA_WRITE: Uuid = short_uuid(0x2001);
/// WuQi BLE OTA notify characteristic (`0x2002`).
pub const CHAR_WUQI_OTA_NOTIFY: Uuid = short_uuid(0x2002);

/// Actions Classic SPP OTA service UUID (`0xE003`, `otaType=7`).
pub const SVC_ACTIONS_OTA_SPP: Uuid = short_uuid(0xE003);

/// BES (Bestechnic) BLE OTA GATT service UUID (`otaType=8`).
pub const SVC_BES_OTA: Uuid = Uuid::from_u128(0x6666_6666_6666_6666_6666_6666_6666_6666);
/// BES (Bestechnic) BLE OTA combined write+notify characteristic UUID (`otaType=8`).
pub const CHAR_BES_OTA: Uuid = Uuid::from_u128(0x7777_7777_7777_7777_7777_7777_7777_7777);

/// Stock JieLi Authenticated BLE OTA GATT service UUID (`0xAE00`, `otaType=9`).
pub const SVC_JIELI_AUTH_OTA: Uuid = short_uuid(0xAE00);
/// Stock JieLi Authenticated BLE OTA write characteristic (`0xAE01`).
pub const CHAR_JIELI_AUTH_WRITE: Uuid = short_uuid(0xAE01);
/// Stock JieLi Authenticated BLE OTA notify characteristic (`0xAE02`).
pub const CHAR_JIELI_AUTH_NOTIFY: Uuid = short_uuid(0xAE02);

/// Bluetrum FOTA BLE GATT service UUID (`0xFF12`, `otaType=10`).
pub const SVC_BLUETRUM_FOTA: Uuid = short_uuid(0xFF12);
/// Bluetrum FOTA notify (device-to-host) characteristic (`0xFF14`).
pub const CHAR_BLUETRUM_FOTA_NOTIFY: Uuid = short_uuid(0xFF14);
/// Bluetrum FOTA write (host-to-device) characteristic (`0xFF15`).
pub const CHAR_BLUETRUM_FOTA_WRITE: Uuid = short_uuid(0xFF15);

// ---------------------------------------------------------------------------
// OTA Route Enumeration & Metadata
// ---------------------------------------------------------------------------

/// Transport medium required for a specific OTA route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OtaTransport {
    BleGatt,
    ClassicRfcomm,
}

/// Active QCY `otaType` route identifiers from `QcyEarphoneManager.startOTA`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum OtaType {
    /// `otaType = 1` — PixArt / AWOTA over Classic SPP (`0x1101`).
    PixArtAwota = 1,
    /// `otaType = 2` — JieLi RCSP OTA over BLE GATT (`0xA002`, no auth).
    JieLiBleUnauth = 2,
    /// `otaType = 5` — Qualcomm QCC GAIA over Classic RFCOMM (`00001107-...`).
    QualcommGaia = 5,
    /// `otaType = 6` — WuQi BLE OTA (`0x7033`).
    WuQiBle = 6,
    /// `otaType = 7` — Actions OTA over Classic SPP (`0xE003`).
    ActionsSpp = 7,
    /// `otaType = 8` — BES (Bestechnic) BLE OTA (`66666666-...`).
    BesBle = 8,
    /// `otaType = 9` — JieLi Authenticated BLE OTA (`0xAE00`, requires `RcspAuth`).
    JieLiBleAuth = 9,
    /// `otaType = 10` — Bluetrum FOTA over BLE GATT (`0xFF12`).
    BluetrumFota = 10,
    /// `otaType = 11` — WuQi BLE OTA + Tone Pack `.bin` (`0x7033`).
    WuQiBleTone = 11,
}

impl OtaType {
    /// All 9 active `otaType` variants.
    pub const ALL: [OtaType; 9] = [
        Self::PixArtAwota,
        Self::JieLiBleUnauth,
        Self::QualcommGaia,
        Self::WuQiBle,
        Self::ActionsSpp,
        Self::BesBle,
        Self::JieLiBleAuth,
        Self::BluetrumFota,
        Self::WuQiBleTone,
    ];

    /// Converts a raw numeric `otaType` from a model profile into an `OtaType`.
    pub const fn from_u8(raw: u8) -> Option<Self> {
        match raw {
            1 => Some(Self::PixArtAwota),
            2 => Some(Self::JieLiBleUnauth),
            5 => Some(Self::QualcommGaia),
            6 => Some(Self::WuQiBle),
            7 => Some(Self::ActionsSpp),
            8 => Some(Self::BesBle),
            9 => Some(Self::JieLiBleAuth),
            10 => Some(Self::BluetrumFota),
            11 => Some(Self::WuQiBleTone),
            _ => None,
        }
    }

    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Returns static transport, UUID, MTU, and authentication metadata for this route.
    pub const fn route_info(self) -> OtaRouteInfo {
        ota_route_info(self)
    }
}

/// Static transport and UUID specification for a single OTA route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OtaRouteInfo {
    pub ota_type: OtaType,
    pub transport: OtaTransport,
    pub service_uuid: Uuid,
    pub write_char_uuid: Option<Uuid>,
    pub notify_char_uuid: Option<Uuid>,
    pub preferred_mtu: usize,
    pub requires_proprietary_auth: bool,
}

/// Returns the UUID and transport specification for each of the 9 active `OtaType` routes.
pub const fn ota_route_info(ota_type: OtaType) -> OtaRouteInfo {
    match ota_type {
        OtaType::PixArtAwota => OtaRouteInfo {
            ota_type,
            transport: OtaTransport::ClassicRfcomm,
            service_uuid: SVC_SPP_STANDARD,
            write_char_uuid: None,
            notify_char_uuid: None,
            preferred_mtu: 248,
            requires_proprietary_auth: false,
        },
        OtaType::JieLiBleUnauth => OtaRouteInfo {
            ota_type,
            transport: OtaTransport::BleGatt,
            service_uuid: SVC_JIELI_RCSP,
            write_char_uuid: Some(CHAR_JIELI_WRITE),
            notify_char_uuid: Some(CHAR_JIELI_NOTIFY),
            preferred_mtu: JIELI_OTA_MTU,
            requires_proprietary_auth: false,
        },
        OtaType::QualcommGaia => OtaRouteInfo {
            ota_type,
            transport: OtaTransport::ClassicRfcomm,
            service_uuid: SVC_QUALCOMM_GAIA,
            write_char_uuid: None,
            notify_char_uuid: None,
            preferred_mtu: 873,
            requires_proprietary_auth: false,
        },
        OtaType::WuQiBle => OtaRouteInfo {
            ota_type,
            transport: OtaTransport::BleGatt,
            service_uuid: SVC_WUQI_OTA,
            write_char_uuid: Some(CHAR_WUQI_OTA_WRITE),
            notify_char_uuid: Some(CHAR_WUQI_OTA_NOTIFY),
            preferred_mtu: DEFAULT_BLE_MTU,
            requires_proprietary_auth: false,
        },
        OtaType::ActionsSpp => OtaRouteInfo {
            ota_type,
            transport: OtaTransport::ClassicRfcomm,
            service_uuid: SVC_ACTIONS_OTA_SPP,
            write_char_uuid: None,
            notify_char_uuid: None,
            preferred_mtu: 256,
            requires_proprietary_auth: false,
        },
        OtaType::BesBle => OtaRouteInfo {
            ota_type,
            transport: OtaTransport::BleGatt,
            service_uuid: SVC_BES_OTA,
            write_char_uuid: Some(CHAR_BES_OTA),
            notify_char_uuid: Some(CHAR_BES_OTA),
            preferred_mtu: DEFAULT_BLE_MTU,
            requires_proprietary_auth: false,
        },
        OtaType::JieLiBleAuth => OtaRouteInfo {
            ota_type,
            transport: OtaTransport::BleGatt,
            service_uuid: SVC_JIELI_AUTH_OTA,
            write_char_uuid: Some(CHAR_JIELI_AUTH_WRITE),
            notify_char_uuid: Some(CHAR_JIELI_AUTH_NOTIFY),
            preferred_mtu: JIELI_OTA_MTU,
            requires_proprietary_auth: true,
        },
        OtaType::BluetrumFota => OtaRouteInfo {
            ota_type,
            transport: OtaTransport::BleGatt,
            service_uuid: SVC_BLUETRUM_FOTA,
            write_char_uuid: Some(CHAR_BLUETRUM_FOTA_WRITE),
            notify_char_uuid: Some(CHAR_BLUETRUM_FOTA_NOTIFY),
            preferred_mtu: 240,
            requires_proprietary_auth: false,
        },
        OtaType::WuQiBleTone => OtaRouteInfo {
            ota_type,
            transport: OtaTransport::BleGatt,
            service_uuid: SVC_WUQI_OTA,
            write_char_uuid: Some(CHAR_WUQI_OTA_WRITE),
            notify_char_uuid: Some(CHAR_WUQI_OTA_NOTIFY),
            preferred_mtu: DEFAULT_BLE_MTU,
            requires_proprietary_auth: false,
        },
    }
}

// ---------------------------------------------------------------------------
// Complete QCY Command ID Catalog (All 89 Documented IDs: 83 SDK + 6 Config)
// ---------------------------------------------------------------------------

/// Complete QCY Unified TLV Command ID enum covering every constant documented
/// in `docs/protocol-spec.md` Section 4.2 (including the 6 config-only active
/// wire IDs: `0x33`, `0x53`, `0x57`, `0x5C`, `0x5D`, `0x63`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum CmdId {
    /// `0x01` — Restore default settings (keeps Bluetooth pairings).
    ResetDefault = 0x01,
    /// `0x02` — Clear Bluetooth pairing records.
    ClearPair = 0x02,
    /// `0x03` — Full factory reset.
    FactoryReset = 0x03,
    /// `0x04` — Focus / White Noise playback trigger (`0x00` play/pause).
    MusicAction = 0x04,
    /// `0x05` — Legacy indicator light toggle.
    Light = 0x05,
    /// `0x06` — Simple In-Ear detection toggle (`OFF = 0x02` on `isNewJL`).
    InEarCheck = 0x06,
    /// `0x07` — Legacy ANC on/off toggle.
    LegacyAnc = 0x07,
    /// `0x08` — White noise / media volume `[vol, vol, flag]`.
    Volume = 0x08,
    /// `0x09` — Low-Latency / Game Mode (`OFF = 0x02` on `isNewJL`).
    GameMode = 0x09,
    /// `0x0A` — Legacy sidetone / monitoring toggle.
    Monitoring = 0x0A,
    /// `0x0C` — Legacy single-byte ANC mode enum.
    NoiseMode = 0x0C,
    /// `0x0D` — Engineering / factory test mode.
    TestMode = 0x0D,
    /// `0x10` — Sleep Mode / Touch Lock (`touchEnable`).
    SleepMode = 0x10,
    /// `0x11` — Ear-tip Fit / Seal Test (`CompactnessDataBean`).
    FitTest = 0x11,
    /// `0x12` — Legacy LED mode enum.
    LegacyLedMode = 0x12,
    /// `0x14` — Scheduled Auto Power-Off (`[minutes_u16LE, curTime_u16LE]`).
    PowerManager = 0x14,
    /// `0x16` — Left/Right audio channel balance (`0..100`, center `50`).
    ChannelBalance = 0x16,
    /// `0x17` — Modern ANC / Transparency / Adaptive / Custom depth `[mode, subScene, level]`.
    AncSetting = 0x17,
    /// `0x18` — Custom Bluetooth pairing display name (UTF-8).
    PairName = 0x18,
    /// `0x19` — Prompt voice pack / language 4-byte ASCII code (`"WQ00"`, `"JL01"`, `"LX00"`).
    VoiceName = 0x19,
    /// `0x1A` — Legacy prompt language 1-byte index.
    VoiceLanguage = 0x1A,
    /// `0x1B` — aptX / aptX Adaptive codec toggle.
    Aptx = 0x1B,
    /// `0x1D` — Prompt tone volume (TX `1B`, RX `[curVol, maxVol]`).
    ToneVolume = 0x1D,
    /// `0x1E` — Remote camera shutter gesture notification.
    TakePhoto = 0x1E,
    /// `0x1F` — Disconnect standby auto power-off (`u16_LE` minutes, `0xFFFF`=Never).
    Standby = 0x1F,
    /// `0x20` — Parametric Multi-EQ v1 (`6B/band`, no `irrType`).
    MultiEq = 0x20,
    /// `0x22` — Parametric Multi-EQ v2 (`7B/band`, with `IrrFilterType`).
    MultiEq2 = 0x22,
    /// `0x23` — LDAC High-Res codec toggle.
    Ldac = 0x23,
    /// `0x24` — Dual-device multipoint toggle (`1T2`).
    DualConnect = 0x24,
    /// `0x27` — Adaptive / Dynamic EQ toggle (`AEQ`).
    DynamicEq = 0x27,
    /// `0x28` — Adaptive ANC calibration status notification (`2`=calibrating, `1`=done).
    AncResult = 0x28,
    /// `0x29` — Adaptive ANC wear/seal status (`2B` or `4B` packed `u16`).
    AncWear = 0x29,
    /// `0x2A` — Dedicated Wind Noise Reduction toggle.
    WindNoise = 0x2A,
    /// `0x2B` — Touch / physical button remapping (`[eventID, actionCmd]`, reset `[0xFF, 0xFF]`).
    ButtonFunc = 0x2B,
    /// `0x2C` — Advanced In-Ear Wear configuration `[useWear, controlMusic, controlANC]`.
    WearConfig = 0x2C,
    /// `0x2D` — Simple 2-state Spatial Sound / 3D Audio toggle.
    SpaceSound = 0x2D,
    /// `0x2E` — Listening / Scene Mode (`1`=Music, `3`=Game, `4`=Movie).
    MusicMode = 0x2E,
    /// `0x2F` — Battery levels `[L, R, Box]` (bit 7 charging, bits 0..6 percentage).
    Battery = 0x2F,
    /// `0x30` — Firmware version `[major, minor, patch]`.
    Version = 0x30,
    /// `0x31` — Touch feedback tone toggle.
    TouchTone = 0x31,
    /// `0x32` — Ambient / Environment Adaptation toggle.
    EnvAdaptation = 0x32,
    /// `0x33` — *(Config-only active wire ID)* Touch sensor sensitivity (`0x26`/`0x2D`/`0x34`).
    TouchSensitivity = 0x33,
    /// `0x34` — Speaker TWS stereo pairing enable (SP75).
    TwsEnable = 0x34,
    /// `0x35` — Speaker RGB LED master power switch.
    LedSwitch = 0x35,
    /// `0x36` — Speaker RGB lighting effect `[speed, bright, effectIdx, N*RGB]`.
    LedEffect = 0x36,
    /// `0x37` — Local playback loop / shuffle mode.
    PlayMode = 0x37,
    /// `0x39` — Focus Mode / White Noise master switch.
    FocusMode = 0x39,
    /// `0x3A` — Hardware white-noise playback state `[musicIndex_u32LE, status, playMode]`.
    MusicState = 0x3A,
    /// `0x3B` — Hardware white-noise track list `[startToneID, N*(musicID_u32LE, total_u16LE)]`.
    MusicInfo = 0x3B,
    /// `0x3D` — Prompt / white-noise preview tone (`1`=start, `0`=stop).
    TonePlay = 0x3D,
    /// `0x3E` — Host wall-clock sync `[YY, MM, DD, hh, mm, ss, weekday_mask]`.
    SyncTime = 0x3E,
    /// `0x3F` — Hardware alarm clock (TX `7B` Add/Del/Edit; RX `[hdr, N*6B]`).
    Alarm = 0x3F,
    /// `0x40` — Adaptive Volume / Loudness normalization toggle.
    AdaptVolume = 0x40,
    /// `0x42` — Earbud colorway index.
    DeviceColor = 0x42,
    /// `0x43` — AI voice / assistant feature flag.
    Ai = 0x43,
    /// `0x44` — Supported parametric EQ band count (`10` or `20`).
    EqCount = 0x44,
    /// `0x45` — Personalized Hearing Test pure-tone reference mode (`1`=enter, `0`=exit).
    CustomEqTest = 0x45,
    /// `0x46` — Independent Left-ear 7B/band parametric EQ.
    MultiEqLeft = 0x46,
    /// `0x47` — Independent Right-ear 7B/band parametric EQ.
    MultiEqRight = 0x47,
    /// `0x48` — In-Ear wear sensor sensitivity (`1`=Mod, `2`=Norm, `3`=Sens).
    InEarSensitivity = 0x48,
    /// `0x4A` — Game Mode Pro `[enabled, followGameSound]`.
    GameConfig = 0x4A,
    /// `0x4B` — UI-only Find Earbuds sentinel (never sent over BLE).
    FindEarphone = 0x4B,
    /// `0x4D` — Multipoint paired/connected host list (`PhoneBeanList`).
    PhoneInfo = 0x4D,
    /// `0x4F` — Multipoint host control & upload delimiter (`10`=start, `11`=end).
    PhoneStatus = 0x4F,
    /// `0x50` — IMU Head-Gesture Control master switch.
    HeadAction = 0x50,
    /// `0x51` — IMU Head-Gesture mapping `[headEventID, actionCmd]`.
    SensorConfig = 0x51,
    /// `0x52` — Live IMU head-gesture test stream (`0xFA`=start, `0xFB`=stop).
    SensorTest = 0x52,
    /// `0x53` — *(Config-only active wire ID)* Power Saving Mode toggle.
    PowerSavingMode = 0x53,
    /// `0x54` — AI Scene ANC state (`AIANCSettingDataBean`).
    AiAnc = 0x54,
    /// `0x55` — AI Scene ANC factory default 3×3 matrix (`45B`).
    AiAncCfg = 0x55,
    /// `0x56` — AI Scene ANC user-customized 3×3 matrix (`TX 5B`, `RX 45B`).
    AiAncCfg2 = 0x56,
    /// `0x57` — *(Config-only active wire ID)* Virtual Bass enhancement toggle.
    BassEnhancement = 0x57,
    /// `0x58` — Hearing Health reminders `[alarmVol, alarmTime, time_u16LE]`.
    HearHealth = 0x58,
    /// `0x5A` — AI-ANC live preview (`[1,0,0,0]` / `[2,m,s,l]` / `[3,0,0,0]`).
    AiAncPreview = 0x5A,
    /// `0x5B` — 3-Mode Spatial Audio selector (`1`=Off, `2`=Fixed, `3`=Head Tracking).
    SpaceAudioMode = 0x5B,
    /// `0x5C` — *(Config-only active wire ID)* Voice Detection / Speak-to-Chat delay (`0/5/10/15/20`s).
    VoiceDetection = 0x5C,
    /// `0x5D` — *(Config-only active wire ID)* Disconnect warning tone toggle.
    DisconnectWarning = 0x5D,
    /// `0x5F` — Privacy Mode Pro `[enabled, level (0=Call+Music, 1=CallOnly)]`.
    PrivacyMode = 0x5F,
    /// `0x60` — Experimental LE Audio toggle.
    LeAudio = 0x60,
    /// `0x62` — Real-time L/R wearing status `u16_LE`.
    WearStatus = 0x62,
    /// `0x63` — *(Config-only active wire ID)* THX Certified EQ toggle.
    ThxEq = 0x63,
    /// `0x64` — Dialogue Perception / Conversation Awareness toggle.
    TalkCheck = 0x64,
    /// `0x65` — Custom ANC button-cycle list (`N*3B` tuples, reset `[0xFF, 0xFF, 0xFF]`).
    BtnAncCfg = 0x65,
    /// `0x66` — Multipoint 2nd-host pairing trigger (`1`=enter, `0`=exit).
    MultiPair = 0x66,
    /// `0xF6` — UI-only "More Settings" sentinel (never sent over BLE).
    MoreSetting = 0xF6,
    /// `0xF7` — UI-only "Device Info" sentinel (never sent over BLE).
    DeviceInfoUi = 0xF7,
    /// `0xF8` — UI-only "Theme Set" sentinel (never sent over BLE).
    ThemeSet = 0xF8,
    /// `0xFC` — Virtual legacy graphic EQ ID (routes to GATT `0x000B` or JL RCSP EQ).
    GainEq = 0xFC,
    /// `0xFE` — Universal attribute read request (`[TargetCmdID]`).
    RequestData = 0xFE,
}

impl CmdId {
    /// Complete array of all 89 documented `CmdId` variants (83 SDK constants + 6 config-only active wire IDs).
    pub const ALL: [CmdId; 89] = [
        Self::ResetDefault,
        Self::ClearPair,
        Self::FactoryReset,
        Self::MusicAction,
        Self::Light,
        Self::InEarCheck,
        Self::LegacyAnc,
        Self::Volume,
        Self::GameMode,
        Self::Monitoring,
        Self::NoiseMode,
        Self::TestMode,
        Self::SleepMode,
        Self::FitTest,
        Self::LegacyLedMode,
        Self::PowerManager,
        Self::ChannelBalance,
        Self::AncSetting,
        Self::PairName,
        Self::VoiceName,
        Self::VoiceLanguage,
        Self::Aptx,
        Self::ToneVolume,
        Self::TakePhoto,
        Self::Standby,
        Self::MultiEq,
        Self::MultiEq2,
        Self::Ldac,
        Self::DualConnect,
        Self::DynamicEq,
        Self::AncResult,
        Self::AncWear,
        Self::WindNoise,
        Self::ButtonFunc,
        Self::WearConfig,
        Self::SpaceSound,
        Self::MusicMode,
        Self::Battery,
        Self::Version,
        Self::TouchTone,
        Self::EnvAdaptation,
        Self::TouchSensitivity,
        Self::TwsEnable,
        Self::LedSwitch,
        Self::LedEffect,
        Self::PlayMode,
        Self::FocusMode,
        Self::MusicState,
        Self::MusicInfo,
        Self::TonePlay,
        Self::SyncTime,
        Self::Alarm,
        Self::AdaptVolume,
        Self::DeviceColor,
        Self::Ai,
        Self::EqCount,
        Self::CustomEqTest,
        Self::MultiEqLeft,
        Self::MultiEqRight,
        Self::InEarSensitivity,
        Self::GameConfig,
        Self::FindEarphone,
        Self::PhoneInfo,
        Self::PhoneStatus,
        Self::HeadAction,
        Self::SensorConfig,
        Self::SensorTest,
        Self::PowerSavingMode,
        Self::AiAnc,
        Self::AiAncCfg,
        Self::AiAncCfg2,
        Self::BassEnhancement,
        Self::HearHealth,
        Self::AiAncPreview,
        Self::SpaceAudioMode,
        Self::VoiceDetection,
        Self::DisconnectWarning,
        Self::PrivacyMode,
        Self::LeAudio,
        Self::WearStatus,
        Self::ThxEq,
        Self::TalkCheck,
        Self::BtnAncCfg,
        Self::MultiPair,
        Self::MoreSetting,
        Self::DeviceInfoUi,
        Self::ThemeSet,
        Self::GainEq,
        Self::RequestData,
    ];

    /// The 6 active wire command IDs discovered in bundled `res/raw/function_*.json`
    /// beyond `ConstantCMDID.java`.
    pub const CONFIG_ONLY_ACTIVE_IDS: [CmdId; 6] = [
        Self::TouchSensitivity,
        Self::PowerSavingMode,
        Self::BassEnhancement,
        Self::VoiceDetection,
        Self::DisconnectWarning,
        Self::ThxEq,
    ];

    /// Converts a raw TLV command byte into a typed `CmdId`.
    pub const fn from_u8(raw: u8) -> Option<Self> {
        match raw {
            0x01 => Some(Self::ResetDefault),
            0x02 => Some(Self::ClearPair),
            0x03 => Some(Self::FactoryReset),
            0x04 => Some(Self::MusicAction),
            0x05 => Some(Self::Light),
            0x06 => Some(Self::InEarCheck),
            0x07 => Some(Self::LegacyAnc),
            0x08 => Some(Self::Volume),
            0x09 => Some(Self::GameMode),
            0x0A => Some(Self::Monitoring),
            0x0C => Some(Self::NoiseMode),
            0x0D => Some(Self::TestMode),
            0x10 => Some(Self::SleepMode),
            0x11 => Some(Self::FitTest),
            0x12 => Some(Self::LegacyLedMode),
            0x14 => Some(Self::PowerManager),
            0x16 => Some(Self::ChannelBalance),
            0x17 => Some(Self::AncSetting),
            0x18 => Some(Self::PairName),
            0x19 => Some(Self::VoiceName),
            0x1A => Some(Self::VoiceLanguage),
            0x1B => Some(Self::Aptx),
            0x1D => Some(Self::ToneVolume),
            0x1E => Some(Self::TakePhoto),
            0x1F => Some(Self::Standby),
            0x20 => Some(Self::MultiEq),
            0x22 => Some(Self::MultiEq2),
            0x23 => Some(Self::Ldac),
            0x24 => Some(Self::DualConnect),
            0x27 => Some(Self::DynamicEq),
            0x28 => Some(Self::AncResult),
            0x29 => Some(Self::AncWear),
            0x2A => Some(Self::WindNoise),
            0x2B => Some(Self::ButtonFunc),
            0x2C => Some(Self::WearConfig),
            0x2D => Some(Self::SpaceSound),
            0x2E => Some(Self::MusicMode),
            0x2F => Some(Self::Battery),
            0x30 => Some(Self::Version),
            0x31 => Some(Self::TouchTone),
            0x32 => Some(Self::EnvAdaptation),
            0x33 => Some(Self::TouchSensitivity),
            0x34 => Some(Self::TwsEnable),
            0x35 => Some(Self::LedSwitch),
            0x36 => Some(Self::LedEffect),
            0x37 => Some(Self::PlayMode),
            0x39 => Some(Self::FocusMode),
            0x3A => Some(Self::MusicState),
            0x3B => Some(Self::MusicInfo),
            0x3D => Some(Self::TonePlay),
            0x3E => Some(Self::SyncTime),
            0x3F => Some(Self::Alarm),
            0x40 => Some(Self::AdaptVolume),
            0x42 => Some(Self::DeviceColor),
            0x43 => Some(Self::Ai),
            0x44 => Some(Self::EqCount),
            0x45 => Some(Self::CustomEqTest),
            0x46 => Some(Self::MultiEqLeft),
            0x47 => Some(Self::MultiEqRight),
            0x48 => Some(Self::InEarSensitivity),
            0x4A => Some(Self::GameConfig),
            0x4B => Some(Self::FindEarphone),
            0x4D => Some(Self::PhoneInfo),
            0x4F => Some(Self::PhoneStatus),
            0x50 => Some(Self::HeadAction),
            0x51 => Some(Self::SensorConfig),
            0x52 => Some(Self::SensorTest),
            0x53 => Some(Self::PowerSavingMode),
            0x54 => Some(Self::AiAnc),
            0x55 => Some(Self::AiAncCfg),
            0x56 => Some(Self::AiAncCfg2),
            0x57 => Some(Self::BassEnhancement),
            0x58 => Some(Self::HearHealth),
            0x5A => Some(Self::AiAncPreview),
            0x5B => Some(Self::SpaceAudioMode),
            0x5C => Some(Self::VoiceDetection),
            0x5D => Some(Self::DisconnectWarning),
            0x5F => Some(Self::PrivacyMode),
            0x60 => Some(Self::LeAudio),
            0x62 => Some(Self::WearStatus),
            0x63 => Some(Self::ThxEq),
            0x64 => Some(Self::TalkCheck),
            0x65 => Some(Self::BtnAncCfg),
            0x66 => Some(Self::MultiPair),
            0xF6 => Some(Self::MoreSetting),
            0xF7 => Some(Self::DeviceInfoUi),
            0xF8 => Some(Self::ThemeSet),
            0xFC => Some(Self::GainEq),
            0xFE => Some(Self::RequestData),
            _ => None,
        }
    }

    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Returns `true` if this ID is a UI-only sentinel that must never be transmitted over BLE.
    pub const fn is_ui_only(self) -> bool {
        matches!(
            self,
            Self::FindEarphone | Self::MoreSetting | Self::DeviceInfoUi | Self::ThemeSet
        )
    }
}
