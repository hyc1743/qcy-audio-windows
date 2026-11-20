//! Clean-room, zero-I/O Rust Bluetooth protocol engine for QCY audio devices.
//!
//! Implements:
//! - [`uuids`]: BLE Advertising Company IDs, Classic A2DP OUI prefixes, GATT & RFCOMM UUIDs,
//!   all 9 active OTA routes, and the complete 89-entry [`CmdId`] catalog.
//! - [`adv`]: 24-byte BLE Manufacturer-Specific Data parser/encoder (with swapped Classic MAC
//!   handling) and 16-bit `modelID` chipset/form-factor classifiers.
//! - [`types`]: Strongly-typed domain models for ANC/AI-ANC, all 4 EQ dialects & 9 biquad filter
//!   types, touch/button & IMU head gestures, multipoint, wear detection, RGB LEDs, white noise,
//!   clock/alarms, and unified [`DeviceCommand`] / [`DeviceEvent`].
//! - [`tlv`]: QCY `[0xFF, PayloadLen, CmdID, VLen, Value...]` TLV framing, multi-TLV splitting,
//!   `0xFE` read requests, MTU chunking, and stream reassembly.
//! - [`rcsp`]: JieLi `FE DC BA ... EF` RCSP framing, stream accumulation, `isNewJL` TLV tunneling
//!   (`0xFF`), and legacy JieLi native RCSP codecs (`0 -> 0x7F` button map).
//! - [`driver`]: Unified [`ProtocolDriver`] trait, [`ProtocolRouter`], and concrete drivers
//!   ([`QcyUnifiedTlvDriver`], [`JieLiNewDriver`], [`JieLiLegacyDriver`], [`LegacyDirectGattDriver`]).
//! - [`ota`]: Local firmware header validation, pre-flight battery/TWS checks, WuQi tone `.bin`
//!   envelope (`0xABCD6543`), and chunked transfer framers across all 9 `otaType` routes.

use thiserror::Error;

pub mod adv;
pub mod driver;
pub mod ota;
pub mod rcsp;
pub mod tlv;
pub mod types;
pub mod uuids;

/// Alias for [`driver`] so callers importing `qcy_protocol::router::ProtocolRouter` work seamlessly.
pub use driver as router;

pub use adv::{
    classify_chipset, classify_form_factor, classify_protocol_family, format_mac, is_bes_device,
    is_bluetrum_device, is_headwear, is_hq3710, is_jl_device, is_new_jl, is_qcc_device,
    is_rd_device, is_rtk_device, is_speaker, is_t11s, is_wq_device, is_zr_device,
    supports_multi_eq, swap_adv_mac, unswap_adv_mac, BatteryLevel, ChipsetFamily,
    DeviceFormFactorHint, QcyAdvertisement,
};
pub use driver::{
    decode_legacy_direct_char, JieLiLegacyDriver, JieLiNewDriver, LegacyDirectGattDriver,
    ProtocolDriver, ProtocolRouter, QcyUnifiedTlvDriver,
};
pub use ota::{
    actions_missing_chunk_indices, crc16_ccitt, crc32_ieee, crc32_mpeg2, parse_wuqi_tone_pack,
    validate_firmware_image, wrap_wuqi_tone_pack, FirmwareContainerKind, FirmwareImageInfo,
    OtaError, OtaPhase, OtaPreFlight, OtaResult, OtaSession,
};
pub use rcsp::{
    change_jl_action_to_qcy, change_qcy_action_to_jl, combine_jl_button_event,
    decode_jl_legacy_frame, split_jl_button_event, unwrap_qcy_tlv_from_rcsp, wrap_qcy_tlv_in_rcsp,
    RcspFrame, RcspStreamAccumulator,
};
pub use tlv::{
    chunk_payload_for_mtu, decode_tlv_frame, decode_tlv_item, encode_read_request,
    encode_single_tlv, encode_tlv_frame, TlvItem, TlvStreamReassembler, TLV_FRAME_HEADER,
};
pub use types::{
    AiAncActivity, AiAncMatrixEntry, AiAncPreviewCommand, AiAncState, AlarmCommand, AncMode,
    AncSetting, ButtonAction, ButtonBinding, DeviceAlarm, DeviceCommand, DeviceDateTime,
    DeviceEvent, EarSide, EqDialect, FirmwareVersion, FitTestResult, GameModeProConfig,
    GraphicEqSetting, HeadGestureAction, HeadGestureBinding, HeadGestureEvent, HearingHealthConfig,
    InEarMusicBehavior, IrrFilterType, LedEffectConfig, LedEffectKind, ListeningMode,
    MultipointHostAction, MultipointHostStatus, PairedHostEntry, PairedHostList, ParametricEqBand,
    ParametricEqPreset, PowerManagerState, PrivacyModeProConfig, PromptToneVolume, ProtocolFamily,
    RgbColor, SpatialAudioMode, TouchSensitivity, WearConfig, WearSensitivity, WhiteNoisePlayState,
    WhiteNoiseTrackInfo, WhiteNoiseTrackList, WireWrite, DEFAULT_EQ_FREQS_10,
    DEFAULT_EQ_FREQS_5_ZR,
};
pub use uuids::{
    is_qcy_classic_oui, is_qcy_company_id, ota_route_info, short_uuid, CmdId, OtaRouteInfo,
    OtaTransport, OtaType, CHAR_BES_OTA, CHAR_BLUETRUM_FOTA_NOTIFY, CHAR_BLUETRUM_FOTA_WRITE,
    CHAR_JIELI_AUTH_NOTIFY, CHAR_JIELI_AUTH_WRITE, CHAR_JIELI_NOTIFY, CHAR_JIELI_WRITE,
    CHAR_LEGACY_BATTERY, CHAR_LEGACY_BUTTON, CHAR_LEGACY_EQ, CHAR_LEGACY_VERSION, CHAR_QCY_NOTIFY,
    CHAR_QCY_WRITE, CHAR_WUQI_OTA_NOTIFY, CHAR_WUQI_OTA_WRITE, COMMAND_PACING_MS, COMPANY_ID_QCY,
    COMPANY_ID_QCY_CUSTOM, COMPANY_ID_QCY_OLD, DEFAULT_BLE_MTU, DESC_CCCD, JIELI_OTA_MTU,
    QCY_CLASSIC_OUIS, QCY_COMPANY_IDS, SVC_ACTIONS_OTA_SPP, SVC_BES_OTA, SVC_BLUETRUM_FOTA,
    SVC_JIELI_AUTH_OTA, SVC_JIELI_RCSP, SVC_QCY_CUSTOM, SVC_QUALCOMM_GAIA, SVC_SPP_STANDARD,
    SVC_WUQI_OTA,
};

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("malformed packet: {0}")]
    MalformedPacket(String),
    #[error("checksum mismatch: expected {expected:#04x}, got {actual:#04x}")]
    ChecksumMismatch { expected: u8, actual: u8 },
    #[error("unsupported command for protocol family")]
    UnsupportedCommand,
}

pub type ProtocolResult<T> = Result<T, ProtocolError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_serialization_sanity() {
        let cmd = DeviceCommand::SetAncMode {
            mode: AncMode::NoiseCancellation,
            depth: Some(3),
        };
        let encoded = serde_json::to_string(&cmd).expect("serialize command");
        assert!(encoded.contains("noise_cancellation"));
    }

    #[test]
    fn uuids_and_cmd_id_catalog_completeness() {
        assert_eq!(
            SVC_QCY_CUSTOM.to_string(),
            "0000a001-0000-1000-8000-00805f9b34fb"
        );
        assert_eq!(
            CHAR_QCY_WRITE.to_string(),
            "00001001-0000-1000-8000-00805f9b34fb"
        );
        assert_eq!(
            CHAR_QCY_NOTIFY.to_string(),
            "00001002-0000-1000-8000-00805f9b34fb"
        );
        assert_eq!(
            SVC_JIELI_RCSP.to_string(),
            "0000a002-0000-1000-8000-00805f9b34fb"
        );
        assert_eq!(
            SVC_BES_OTA.to_string(),
            "66666666-6666-6666-6666-666666666666"
        );
        assert_eq!(
            SVC_QUALCOMM_GAIA.to_string(),
            "00001107-d102-11e1-9b23-00025b00a5a5"
        );

        assert!(is_qcy_company_id(0x071E));
        assert!(is_qcy_company_id(0x521C));
        assert!(is_qcy_company_id(0x522C));
        assert!(!is_qcy_company_id(0x004C));

        assert!(is_qcy_classic_oui([0x84, 0xAC, 0x60, 0x11, 0x22, 0x33]));
        assert!(is_qcy_classic_oui([0x1C, 0x52, 0x16, 0xAA, 0xBB, 0xCC]));
        assert!(is_qcy_classic_oui([0xC4, 0x33, 0x96, 0x01, 0x02, 0x03]));
        assert!(!is_qcy_classic_oui([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]));

        // Verify every CmdId round-trips through u8 cleanly and all 6 config-only active IDs exist.
        assert_eq!(CmdId::ALL.len(), 89);
        for cmd in CmdId::ALL {
            assert_eq!(CmdId::from_u8(cmd.as_u8()), Some(cmd));
        }
        for raw in [0x33u8, 0x53, 0x57, 0x5C, 0x5D, 0x63] {
            assert!(CmdId::from_u8(raw).is_some());
        }
        assert!(CmdId::FindEarphone.is_ui_only());
        assert!(!CmdId::AncSetting.is_ui_only());

        // Verify all 9 active OTA routes exist and classify BLE vs Classic accurately.
        assert_eq!(OtaType::ALL.len(), 9);
        assert_eq!(
            OtaType::PixArtAwota.route_info().transport,
            OtaTransport::ClassicRfcomm
        );
        assert_eq!(
            OtaType::QualcommGaia.route_info().transport,
            OtaTransport::ClassicRfcomm
        );
        assert_eq!(
            OtaType::ActionsSpp.route_info().transport,
            OtaTransport::ClassicRfcomm
        );
        assert_eq!(
            OtaType::WuQiBleTone.route_info().transport,
            OtaTransport::BleGatt
        );
        assert!(OtaType::JieLiBleAuth.route_info().requires_proprietary_auth);
        assert!(
            !OtaType::JieLiBleUnauth
                .route_info()
                .requires_proprietary_auth
        );
    }

    #[test]
    fn ble_24_byte_advertisement_golden_roundtrip_and_swapped_mac() {
        // Golden 24-byte QCY manufacturer data for HT08 (0x4D4A, WuQi):
        // Primary MAC: 84:AC:60:12:34:56 -> wire swapped: [0xAC, 0x84, 0x60, 0x56, 0x34, 0x12]
        // Peer MAC:    84:AC:60:AB:CD:EF -> wire swapped: [0xAC, 0x84, 0x60, 0xEF, 0xCD, 0xAB]
        let raw: [u8; 24] = [
            0x4D, 0x4A, // [0..1] modelID = 0x4D4A (HT08 MeloBuds Pro, WuQi)
            0x01, // [2] reserved
            0x05, // [3] bit0=1 (is_left), bits1..3=2 (color_index=2)
            0x60, // [4] bit6=1 (is_tws), bit5=1 (is_pop)
            0xD8, // [5] Left: charging=1 (0x80), 88% (0x58)
            0x5C, // [6] Right: charging=0, 92% (0x5C)
            0x7F, // [7] Case: absent/closed (0x7F -> None)
            0x00, 0x02, 0x03, // [8..10] reserved
            0xAC, 0x84, 0x60, 0x56, 0x34, 0x12, // [11..16] swapped primary MAC
            0x2A, // [17] pop_num = 42
            0xAC, 0x84, 0x60, 0xEF, 0xCD, 0xAB, // [18..23] swapped peer MAC
        ];

        let parsed = QcyAdvertisement::parse(COMPANY_ID_QCY, &raw).expect("parse 24B adv");
        assert_eq!(parsed.model_id, 0x4D4A);
        assert_eq!(parsed.chipset(), ChipsetFamily::WuQi);
        assert_eq!(parsed.form_factor_hint(), DeviceFormFactorHint::Earbuds);
        assert_eq!(
            parsed.expected_protocol_family(),
            ProtocolFamily::QcyUnifiedTlv
        );
        assert!(parsed.is_left);
        assert_eq!(parsed.color_index, 2);
        assert!(parsed.is_tws);
        assert!(parsed.is_pop);
        assert_eq!(
            parsed.left_battery,
            BatteryLevel {
                percent: Some(88),
                charging: true
            }
        );
        assert_eq!(
            parsed.right_battery,
            BatteryLevel {
                percent: Some(92),
                charging: false
            }
        );
        assert_eq!(
            parsed.case_battery,
            BatteryLevel {
                percent: None,
                charging: false
            }
        );
        assert_eq!(parsed.class_mac, Some([0x84, 0xAC, 0x60, 0x12, 0x34, 0x56]));
        assert_eq!(format_mac(parsed.class_mac.unwrap()), "84:AC:60:12:34:56");
        assert_eq!(parsed.pop_num, Some(42));
        assert_eq!(parsed.other_mac, Some([0x84, 0xAC, 0x60, 0xAB, 0xCD, 0xEF]));

        // Re-encode and verify byte-exact round-trip (note 0x7F absent normalizes to 0x00)
        let mut expected_reencoded = raw.to_vec();
        expected_reencoded[7] = 0x00;
        assert_eq!(parsed.encode(), expected_reencoded);
    }

    #[test]
    fn chipset_bitmask_and_multi_eq_routing() {
        assert_eq!(classify_chipset(0x0308), ChipsetFamily::WuQi);
        assert_eq!(classify_form_factor(0x0308), DeviceFormFactorHint::Speaker);
        assert_eq!(classify_chipset(0x417C), ChipsetFamily::JieLiLegacy);
        assert_eq!(classify_form_factor(0x417C), DeviceFormFactorHint::Headwear);
        assert_eq!(classify_chipset(0x8171), ChipsetFamily::JieLiNew);
        assert!(is_new_jl(0x8171));
        assert_eq!(classify_chipset(0x4D57), ChipsetFamily::Bluetrum);
        assert_eq!(classify_chipset(0x4DD0), ChipsetFamily::Bes);
        assert_eq!(classify_chipset(0x4D17), ChipsetFamily::QualcommQcc);
        assert!(is_t11s(0x4D17));
        assert!(is_hq3710(0x4BC5));

        assert!(supports_multi_eq(0x4D4A, false, None));
        assert!(!supports_multi_eq(0x4D4A, true, None));
        assert!(supports_multi_eq(0x1972, true, None)); // allowlist override
        assert!(!supports_multi_eq(0x417C, false, None)); // legacy JL
        assert!(supports_multi_eq(0x8171, false, None)); // new JL
    }

    #[test]
    fn multi_tlv_notification_splitting_and_0xfe_read_requests() {
        let read_req = encode_read_request(CmdId::Battery.as_u8());
        assert_eq!(read_req, vec![0xFF, 0x03, 0xFE, 0x01, 0x2F]);

        // Coalesced multi-TLV frame containing Version (0x30) + Battery (0x2F) + ANC (0x17)
        let frame = encode_tlv_frame(&[
            TlvItem::from_cmd(CmdId::Version, [1, 2, 5]),
            TlvItem::from_cmd(CmdId::Battery, [0xD0, 0x55, 0x64]),
            TlvItem::from_cmd(CmdId::AncSetting, [0x03, 0x02, 0x00]), // Vocal Boost
        ])
        .expect("encode multi-tlv");

        assert_eq!(frame[0], 0xFF);
        assert_eq!(frame[1], 15); // 3 * (2 + 3)

        // Feed fragmented across 2 BLE notifications into QcyUnifiedTlvDriver
        let driver = QcyUnifiedTlvDriver::default();
        let ev1 = driver
            .decode_notification(CHAR_QCY_NOTIFY, &frame[..7])
            .expect("part 1");
        assert!(ev1.is_empty());
        let ev2 = driver
            .decode_notification(CHAR_QCY_NOTIFY, &frame[7..])
            .expect("part 2");

        assert_eq!(
            ev2[0],
            DeviceEvent::FirmwareVersion(FirmwareVersion {
                major: 1,
                minor: 2,
                patch: 5
            })
        );
        assert_eq!(
            ev2[1],
            DeviceEvent::BatteryUpdated {
                left: Some(80),
                right: Some(85),
                case: Some(100),
                left_charging: true,
                right_charging: false,
                case_charging: false,
            }
        );
        assert_eq!(
            ev2[2],
            DeviceEvent::AncSettingUpdated(AncSetting::vocal_boost_transparency())
        );
        assert_eq!(
            ev2[3],
            DeviceEvent::AncModeChanged {
                mode: AncMode::VocalBoostTransparency,
                depth: None,
            }
        );
    }

    #[test]
    fn all_four_eq_dialects_and_irr_filter_types_roundtrip() {
        // 1. Dialect A: Legacy Graphic EQ (0x000B / 0xFC) including 13-byte extended RX
        let legacy_rx = [
            0x06u8,
            2,
            1,
            0,
            (-2i8) as u8,
            (-4i8) as u8,
            0,
            1,
            3,
            4,
            2,
            0x01,
            0x03,
        ];
        let parsed_legacy = tlv::decode_graphic_eq(&legacy_rx).expect("graphic eq");
        assert_eq!(parsed_legacy.eq_type, 6);
        assert_eq!(parsed_legacy.gains_db, vec![2, 1, 0, -2, -4, 0, 1, 3, 4, 2]);
        assert_eq!(parsed_legacy.game_eq_type, Some(1));
        assert_eq!(parsed_legacy.current_mode, Some(3));

        // 2. Dialect B: Parametric Multi-EQ v1 (0x20, 6B/band)
        let v1_preset = ParametricEqPreset {
            eq_type: 1,
            master_gain_db: -1.5,
            bands: vec![ParametricEqBand::peak(1000, 3.5, 1.41)],
        };
        let v1_bytes = tlv::encode_parametric_eq_v1(&v1_preset);
        assert_eq!(
            v1_bytes,
            vec![
                0x01, // eqType
                0x6A, 0xFF, // -150 = -1.50 dB (i16_LE)
                0xE8, 0x03, // 1000 Hz (u16_LE)
                0x5E, 0x01, // +350 = +3.50 dB (i16_LE)
                0x8D, 0x00, // 141 = Q 1.41 (i16_LE)
            ]
        );
        let v1_decoded = tlv::decode_parametric_eq_v1(&v1_bytes).expect("decode v1");
        assert_eq!(v1_decoded, v1_preset);

        // 3. Dialect C: Parametric Multi-EQ v2 (0x22, 7B/band) with ALL 9 IrrFilterType 0..8
        let all_9_filters = [
            IrrFilterType::HighPass,
            IrrFilterType::LowPass,
            IrrFilterType::Peak,
            IrrFilterType::HighShelf,
            IrrFilterType::LowShelf,
            IrrFilterType::HighShelfQ,
            IrrFilterType::LowShelfQ,
            IrrFilterType::AdvHighPass,
            IrrFilterType::AdvLowPass,
        ];
        let v2_bands: Vec<ParametricEqBand> = all_9_filters
            .iter()
            .enumerate()
            .map(|(i, &ft)| {
                assert_eq!(ft.as_u8(), i as u8);
                assert_eq!(IrrFilterType::from_u8(i as u8), Some(ft));
                ParametricEqBand {
                    freq_hz: 60 + (i as u16) * 500,
                    gain_db: -2.0 + (i as f32) * 0.5,
                    q: 1.0 + (i as f32) * 0.25,
                    filter_type: ft,
                }
            })
            .collect();
        let v2_preset = ParametricEqPreset {
            eq_type: 2,
            master_gain_db: 0.25,
            bands: v2_bands,
        };
        let mut v2_bytes = tlv::encode_parametric_eq_v2(&v2_preset);
        // Append firmware dummy sentinel (100 Hz, 0 dB, Q=0.01) + Pad (0xFF) to confirm RX filters them out
        v2_bytes.extend_from_slice(&[100, 0, 0, 0, 1, 0, 2]);
        v2_bytes.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0xFF]);
        let v2_decoded = tlv::decode_parametric_eq_v2(&v2_bytes).expect("decode v2");
        assert_eq!(v2_decoded, v2_preset);

        // 4. Dialect D: Independent L/R Parametric EQ (0x46 / 0x47) padding up to max_bands
        let lr_bytes = tlv::encode_side_parametric_eq(
            &ParametricEqPreset {
                eq_type: 9,
                master_gain_db: 0.0,
                bands: vec![ParametricEqBand::peak(250, 2.0, 1.0)],
            },
            512,
            4,
        );
        assert_eq!(lr_bytes.len(), 3 + 4 * 7);
        assert_eq!(lr_bytes[3 + 7 + 6], 0xFF); // 2nd slot padded with IrrFilterType::Pad
        let lr_decoded = tlv::decode_parametric_eq_v2(&lr_bytes).expect("decode L/R");
        assert_eq!(lr_decoded.bands.len(), 1);
    }

    #[test]
    fn jieli_rcsp_framing_new_jl_off_0x02_quirk_and_legacy_button_translation() {
        // Verify isNewJL wraps QCY TLV inside FE DC BA ... EF (opcode 0xFF) and sends OFF=0x02 on 0x06/0x09!
        let new_jl = ProtocolRouter::select_driver(Some(0x8171), &[SVC_JIELI_RCSP]);
        assert_eq!(new_jl.family(), ProtocolFamily::JieLiRcspNew);

        let game_off = new_jl
            .encode_command(&DeviceCommand::SetGamingMode { enabled: false })
            .expect("encode game off");
        assert_eq!(game_off[0].service, SVC_JIELI_RCSP);
        assert_eq!(game_off[0].characteristic, CHAR_JIELI_WRITE);
        let rcsp = RcspFrame::decode(&game_off[0].payload).expect("decode rcsp");
        assert_eq!(rcsp.opcode, rcsp::OP_EXTRA_CUSTOM);
        let inner = unwrap_qcy_tlv_from_rcsp(&rcsp).expect("unwrap inner tlv");
        assert_eq!(inner, &[0xFF, 0x03, 0x09, 0x01, 0x02]); // OFF == 0x02!

        let in_ear_off = new_jl
            .encode_command(&DeviceCommand::SetInEarCheck { enabled: false })
            .expect("encode in-ear off");
        let rcsp2 = RcspFrame::decode(&in_ear_off[0].payload).expect("decode rcsp2");
        let inner2 = unwrap_qcy_tlv_from_rcsp(&rcsp2).expect("unwrap inner2");
        assert_eq!(inner2, &[0xFF, 0x03, 0x06, 0x01, 0x02]); // OFF == 0x02!

        // Fragmented RCSP notification decode back into DeviceEvent::GamingModeChanged { enabled: false }
        let rsp = RcspFrame::response(rcsp::OP_EXTRA_CUSTOM, 0x00, 1, inner)
            .encode()
            .expect("encode rsp");
        let mid = rsp.len() / 2;
        assert!(new_jl
            .decode_notification(CHAR_JIELI_NOTIFY, &rsp[..mid])
            .unwrap()
            .is_empty());
        let decoded = new_jl
            .decode_notification(CHAR_JIELI_NOTIFY, &rsp[mid..])
            .unwrap();
        assert_eq!(
            decoded,
            vec![DeviceEvent::GamingModeChanged { enabled: false }]
        );

        // Verify Legacy JieLi button action 0 (None) <-> 0x7F and 16-bit over-ear event split
        assert_eq!(change_qcy_action_to_jl(0), 0x7F);
        assert_eq!(change_jl_action_to_qcy(0x7F), 0);
        assert_eq!(change_qcy_action_to_jl(11), 11);
        assert_eq!(split_jl_button_event(1), (1, 1)); // Left Single
        assert_eq!(split_jl_button_event(10), (2, 5)); // Right Long Press
        assert_eq!(split_jl_button_event(0x0702), (7, 2)); // MFB Double Press
        assert_eq!(combine_jl_button_event(7, 2), 0x0702);

        let legacy_jl = ProtocolRouter::select_driver(Some(0x417C), &[SVC_JIELI_RCSP]);
        assert_eq!(legacy_jl.family(), ProtocolFamily::JieLiRcspLegacy);
        let btn_writes = legacy_jl
            .encode_command(&DeviceCommand::SetButtonBinding(ButtonBinding {
                event_id: 0x0702,
                action: ButtonAction::None,
            }))
            .expect("legacy jl button");
        let btn_frame = RcspFrame::decode(&btn_writes[0].payload).expect("decode btn");
        assert_eq!(btn_frame.opcode, rcsp::OP_ADV_MODIFY_INFO);
        assert_eq!(btn_frame.body(), &[0x04, 0x02, 0x07, 0x02, 0x7F]);
    }

    #[test]
    fn multipoint_rgb_ai_anc_power_and_find_earbuds_codecs() {
        let driver = QcyUnifiedTlvDriver::default();

        // FindEarbuds (0x4B) must NEVER produce a BLE packet!
        let find_writes = driver
            .encode_command(&DeviceCommand::FindEarbuds {
                left: true,
                right: false,
            })
            .expect("find earbuds");
        assert!(find_writes.is_empty());

        // PhoneBeanList (0x4D): 2 hosts (1 connected status=2, 1 disconnected status=0)
        let mut phone_val = vec![2u8];
        // Entry 1: len=14 (1 status + 6 mac + 7 "Desktop")
        phone_val.push(14);
        phone_val.push(2);
        phone_val.extend_from_slice(&[0x11, 0x22, 0x33, 0x44, 0x55, 0x66]);
        phone_val.extend_from_slice(b"Desktop");
        // Entry 2: len=13 (1 status + 6 mac + 5 "Phone" + NUL)
        phone_val.push(13);
        phone_val.push(0);
        phone_val.extend_from_slice(&[0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
        phone_val.extend_from_slice(b"Phone\0");

        let parsed_phones = tlv::decode_paired_host_list(&phone_val).expect("parse 0x4D");
        assert_eq!(parsed_phones.total_count, 2);
        assert_eq!(parsed_phones.connected.len(), 1);
        assert_eq!(parsed_phones.connected[0].name, "Desktop");
        assert_eq!(parsed_phones.disconnected.len(), 1);
        assert_eq!(parsed_phones.disconnected[0].name, "Phone");

        // PowerManager (0x14): raw powerTime=0 normalizes to 0xFFFF (Disabled)
        let pm = tlv::decode_power_manager(&[0x00, 0x00, 0x1E, 0x00]).expect("parse 0x14");
        assert_eq!(pm.scheduled_minutes, PowerManagerState::DISABLED);
        assert_eq!(pm.remaining_minutes, 30);

        // SP75 RGB LED (0x36) round-trip
        let led = LedEffectConfig {
            speed: 3,
            brightness: 80,
            effect: LedEffectKind::Gradient,
            colors: vec![
                RgbColor {
                    r: 255,
                    g: 0,
                    b: 128,
                },
                RgbColor {
                    r: 0,
                    g: 200,
                    b: 255,
                },
            ],
        };
        let led_raw = tlv::encode_led_effect(&led);
        assert_eq!(tlv::decode_led_effect(&led_raw).unwrap(), led);

        // Hearing Health (0x58) round-trip (u16_LE minutes)
        let hh = HearingHealthConfig {
            alarm_volume: true,
            alarm_time: true,
            limit_minutes: 90,
        };
        let hh_raw = tlv::encode_hearing_health(hh);
        assert_eq!(hh_raw, [1, 1, 0x5A, 0x00]);
        assert_eq!(tlv::decode_hearing_health(&hh_raw).unwrap(), hh);

        // 0x29 AncWear 4-byte "+3" quirk
        let wear_ev = decode_tlv_item(&TlvItem::from_cmd(CmdId::AncWear, [0, 0, 1, 2])).unwrap();
        assert_eq!(
            wear_ev,
            vec![DeviceEvent::AncWearStatus {
                mask: ((1 + 3) as u16) | (((2 + 3) as u16) << 8)
            }]
        );
    }

    #[test]
    fn ota_validation_wuqi_tone_pack_chunking_and_jieli_auth_rejection() {
        // 1. otaType=9 MUST return explicit UnsupportedAuthRequired
        let dummy_fw = vec![0x55u8; 256];
        let auth_err = OtaSession::new(OtaType::JieLiBleAuth, dummy_fw.clone(), 256)
            .expect_err("otaType=9 must reject cleanly");
        assert!(matches!(
            auth_err,
            OtaError::UnsupportedAuthRequired {
                ota_type: OtaType::JieLiBleAuth,
                ..
            }
        ));

        // 2. WuQi Tone Pack 0xABCD6543 envelope wrap & dual-CRC verify
        let tone_raw = vec![0xABu8; 128];
        let wrapped = wrap_wuqi_tone_pack(&tone_raw).expect("wrap tone");
        assert_eq!(parse_wuqi_tone_pack(&wrapped).unwrap(), tone_raw.as_slice());

        let mut corrupted = wrapped.clone();
        corrupted[40] ^= 0xFF;
        assert!(matches!(
            parse_wuqi_tone_pack(&corrupted),
            Err(OtaError::CrcMismatch { .. })
        ));

        // 3. WuQi BLE OTA session chunking end-to-end
        let mut wq = OtaSession::new(OtaType::WuQiBleTone, wrapped, 256).expect("wq session");
        assert_eq!(
            wq.image_info().container,
            FirmwareContainerKind::WuQiTonePack
        );
        let starts = wq.build_start_packets().expect("starts");
        assert_eq!(&starts[0][0..3], &[0x70, 0x07, 0x6E]);
        let mut chunks = 0;
        while let Some(pkt) = wq.next_data_packet().expect("chunk") {
            assert_eq!(&pkt[0..3], &[0x70, 0x07, 0x6E]);
            chunks += 1;
        }
        assert!(chunks >= 1);
        assert!((wq.progress_percent() - 100.0).abs() < 0.01);
        assert_eq!(wq.build_finish_packets().expect("finish").len(), 2);

        // 4. Actions AOTA header validation + missing bitmap helper
        assert!(validate_firmware_image(OtaType::ActionsSpp, &dummy_fw).is_err());
        let mut aota = Vec::from(&b"AOTA1.2.3\0\0\0\0\0\0\0\0\0\0\0"[..]);
        aota.resize(128, 0xAA);
        let info = validate_firmware_image(OtaType::ActionsSpp, &aota).expect("aota valid");
        assert_eq!(info.container, FirmwareContainerKind::ActionsAota);
        assert_eq!(info.version_hint.as_deref(), Some("1.2.3"));
        assert_eq!(
            actions_missing_chunk_indices(&[0b1010_1101], 8),
            vec![1, 4, 6]
        );

        // 5. JieLi unauthenticated otaType=2 device-pulled block response
        let mut jl_ota = OtaSession::new(OtaType::JieLiBleUnauth, dummy_fw, 256).expect("jl ota");
        assert_eq!(jl_ota.build_start_packets().expect("jl starts").len(), 4);
        let blk = jl_ota.build_jieli_pulled_block(16, 32, 7).expect("jl pull");
        let blk_frame = RcspFrame::decode(&blk).expect("decode jl blk");
        assert_eq!(blk_frame.opcode, rcsp::OP_OTA_SEND_BLOCK);
        assert_eq!(blk_frame.body().len(), 32);

        // 6. Pre-flight check rejects low battery or missing TWS bud
        let bad_preflight = OtaPreFlight {
            left_battery: Some(25),
            right_battery: Some(80),
            is_tws: true,
            both_buds_connected: true,
            min_battery_percent: 30,
        };
        assert!(bad_preflight.validate().is_err());

        // 7. Standard CRC test vectors ("123456789")
        assert_eq!(crc32_ieee(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32_mpeg2(b"123456789"), 0x0376_E6E7);
        assert_eq!(crc16_ccitt(b"123456789"), 0x29B1);

        // 8. Bluetrum, BES, PixArt/AWOTA & Qualcomm GAIA OTA packet framers
        for (ota_type, expected_prefix) in [
            (OtaType::BluetrumFota, &ota::BLUETRUM_FOTA_MAGIC[..]),
            (OtaType::BesBle, &[0x80u8, b'B', b'E', b'S', b'T'][..]),
            (OtaType::PixArtAwota, &b"PA\x06\x30"[..]),
            (
                OtaType::QualcommGaia,
                &[0xFF, 0x01, 0x00, 0x00, 0x00, 0x0A, 0x06, 0x40][..],
            ),
        ] {
            let mut s = OtaSession::new(ota_type, vec![0x42; 512], 256).expect("ota session");
            let start_pkts = s.build_start_packets().expect("start pkts");
            assert!(start_pkts[0].starts_with(expected_prefix));
            let first_data = s.next_data_packet().expect("data").expect("chunk");
            assert!(!first_data.is_empty());
            let finish_pkts = s.build_finish_packets().expect("finish");
            assert!(!finish_pkts.is_empty());
            s.mark_completed();
            assert_eq!(*s.phase(), OtaPhase::Completed);
        }
    }

    #[test]
    fn legacy_direct_gatt_driver_and_config_only_cmd_roundtrips() {
        let legacy = ProtocolRouter::select_driver_with_characteristics(
            Some(0x4D4A),
            &[SVC_QCY_CUSTOM],
            &[
                CHAR_LEGACY_EQ,
                CHAR_LEGACY_BATTERY,
                CHAR_LEGACY_VERSION,
                CHAR_LEGACY_BUTTON,
            ],
            247,
        );
        assert_eq!(legacy.family(), ProtocolFamily::LegacyDirectGatt);

        let info_reads = legacy
            .encode_command(&DeviceCommand::RequestDeviceInfo)
            .expect("legacy reads");
        assert_eq!(info_reads.len(), 4);
        assert!(info_reads.iter().all(|w| w.is_read));
        assert_eq!(info_reads[0].characteristic, CHAR_LEGACY_VERSION);
        assert_eq!(info_reads[1].characteristic, CHAR_LEGACY_BATTERY);
        assert_eq!(info_reads[2].characteristic, CHAR_LEGACY_EQ);
        assert_eq!(info_reads[3].characteristic, CHAR_LEGACY_BUTTON);

        let ver = legacy
            .decode_notification(CHAR_LEGACY_VERSION, &[2, 0, 9])
            .expect("ver");
        assert_eq!(
            ver,
            vec![DeviceEvent::FirmwareVersion(FirmwareVersion {
                major: 2,
                minor: 0,
                patch: 9
            })]
        );

        let eq_write = legacy
            .encode_command(&DeviceCommand::SetGraphicEq(GraphicEqSetting {
                eq_type: 1,
                gains_db: vec![1, 2, 0, -1, -2, 0, 1, 2, 3, 1],
                game_eq_type: None,
                current_mode: None,
            }))
            .expect("legacy eq write");
        assert_eq!(eq_write[0].characteristic, CHAR_LEGACY_EQ);
        assert_eq!(eq_write[0].payload[0], 1);

        // Verify all 6 config-only active wire CMDIDs round-trip through QcyUnifiedTlvDriver
        let tlv_driver = QcyUnifiedTlvDriver::default();
        let config_cmds = [
            (
                DeviceCommand::SetTouchSensitivity {
                    sensitivity: TouchSensitivity::Sensitive,
                },
                DeviceEvent::TouchSensitivityUpdated {
                    sensitivity: TouchSensitivity::Sensitive,
                },
            ),
            (
                DeviceCommand::SetPowerSavingMode { enabled: true },
                DeviceEvent::PowerSavingModeChanged { enabled: true },
            ),
            (
                DeviceCommand::SetBassEnhancement { enabled: true },
                DeviceEvent::BassEnhancementChanged { enabled: true },
            ),
            (
                DeviceCommand::SetVoiceDetectionDelay { seconds: 10 },
                DeviceEvent::VoiceDetectionDelayUpdated { seconds: 10 },
            ),
            (
                DeviceCommand::SetDisconnectWarning { enabled: true },
                DeviceEvent::DisconnectWarningChanged { enabled: true },
            ),
            (
                DeviceCommand::SetThxEq { enabled: true },
                DeviceEvent::ThxEqChanged { enabled: true },
            ),
        ];
        for (cmd, expected_ev) in config_cmds {
            let writes = tlv_driver.encode_command(&cmd).expect("encode config cmd");
            let events = tlv_driver
                .decode_notification(CHAR_QCY_NOTIFY, &writes[0].payload)
                .expect("decode config cmd");
            assert_eq!(events, vec![expected_ev]);
        }

        // Verify AI-ANC matrix, IMU head gestures, wear config, alarms, white noise & MTU chunking
        let chunks = chunk_payload_for_mtu(&[0xAA; 50], 23);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].len(), 20);
        assert_eq!(chunks[2].len(), 10);

        let ai_matrix_raw: Vec<u8> = (0..9u8)
            .flat_map(|i| [1 + (i / 3), 1 + (i % 3), 1, 2, i % 3])
            .collect();
        let ai_entries = tlv::decode_ai_anc_matrix(&ai_matrix_raw).expect("ai matrix");
        assert_eq!(ai_entries.len(), 9);
        assert_eq!(ai_entries[0].activity, AiAncActivity::Stay);
        assert_eq!(ai_entries[8].activity, AiAncActivity::Run);

        let wear_cfg = WearConfig {
            enabled: true,
            music_behavior: InEarMusicBehavior::DualPauseAutoPlay,
            control_anc: true,
        };
        let wear_w = tlv_driver
            .encode_command(&DeviceCommand::SetWearConfig(wear_cfg))
            .unwrap();
        assert_eq!(
            tlv_driver
                .decode_notification(CHAR_QCY_NOTIFY, &wear_w[0].payload)
                .unwrap(),
            vec![DeviceEvent::WearConfigUpdated(wear_cfg)]
        );

        let imu_w = tlv_driver
            .encode_command(&DeviceCommand::SetHeadGestureBinding(HeadGestureBinding {
                event: HeadGestureEvent::Nod,
                action: HeadGestureAction::AnswerCall,
            }))
            .unwrap();
        assert_eq!(
            tlv_driver
                .decode_notification(CHAR_QCY_NOTIFY, &imu_w[0].payload)
                .unwrap(),
            vec![DeviceEvent::HeadGestureBindingsUpdated {
                bindings: vec![HeadGestureBinding {
                    event: HeadGestureEvent::Nod,
                    action: HeadGestureAction::AnswerCall,
                }]
            }]
        );

        let alarms_rx =
            encode_single_tlv(CmdId::Alarm.as_u8(), &[0x01, 0x01, 0x01, 7, 30, 0x3E, 0x00])
                .unwrap();
        let alarm_ev = tlv_driver
            .decode_notification(CHAR_QCY_NOTIFY, &alarms_rx)
            .unwrap();
        assert_eq!(
            alarm_ev,
            vec![DeviceEvent::AlarmsUpdated {
                header: 1,
                alarms: vec![DeviceAlarm {
                    alarm_id: 1,
                    enabled: true,
                    hour: 7,
                    minute: 30,
                    cycle_mask: 0x3E,
                }]
            }]
        );
    }
}
