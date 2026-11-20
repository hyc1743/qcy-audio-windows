//! Data-driven QCY Audio device model registry, EQ preset catalog, and
//! `pop_view_rules` mutual-exclusion evaluator loaded at compile time from
//! `data/models_catalog.json` and `data/eq_presets.json`.

use std::collections::HashMap;
use std::sync::OnceLock;

use qcy_protocol::QcyAdvertisement;

pub mod catalog;
pub mod eq;

pub use catalog::{
    ButtonActionMeta, DeviceCapabilities, DeviceModelProfile, FormFactor, ModelsCatalog,
    PopViewRule, PopViewRuleMatch, RuleAction, RuleCompareOp, RuleConflictCondition,
};
pub use eq::{
    EqCatalog, EqPresetEntry, FilterTypeMeta, FrequencyGrids, HearingTestConfig,
    PresetParametricBand, TargetCurvePoint, TargetFrCurve,
};

const MODELS_CATALOG_JSON: &str = include_str!("../data/models_catalog.json");
const EQ_PRESETS_JSON: &str = include_str!("../data/eq_presets.json");

static EMBEDDED_MODELS: OnceLock<ModelsCatalog> = OnceLock::new();
static EMBEDDED_EQ: OnceLock<EqCatalog> = OnceLock::new();

fn embedded_models_catalog() -> &'static ModelsCatalog {
    EMBEDDED_MODELS.get_or_init(|| {
        serde_json::from_str(MODELS_CATALOG_JSON)
            .expect("embedded models_catalog.json must be valid JSON")
    })
}

fn embedded_eq_catalog() -> &'static EqCatalog {
    EMBEDDED_EQ.get_or_init(|| {
        serde_json::from_str(EQ_PRESETS_JSON).expect("embedded eq_presets.json must be valid JSON")
    })
}

/// Central clean-room registry providing model lookup, BLE device matching,
/// built-in EQ presets, target FR curves, button actions, and conflict rules.
#[derive(Debug, Clone)]
pub struct ModelRegistry {
    catalog: ModelsCatalog,
    eq_catalog: EqCatalog,
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::load_embedded()
    }
}

impl ModelRegistry {
    /// Loads the embedded compile-time catalogs (`43` models, `22` EQ presets, `9` IIR filters, `5` target curves).
    pub fn load_embedded() -> Self {
        Self {
            catalog: embedded_models_catalog().clone(),
            eq_catalog: embedded_eq_catalog().clone(),
        }
    }

    /// Returns all registered device models.
    pub fn models(&self) -> &[DeviceModelProfile] {
        &self.catalog.models
    }

    /// Finds a model profile by its canonical string `model_id` (e.g., `"qcy-n70"`, `"qcy-sp75"`).
    pub fn find_by_id(&self, model_id: &str) -> Option<&DeviceModelProfile> {
        self.catalog
            .models
            .iter()
            .find(|m| m.model_id.eq_ignore_ascii_case(model_id))
    }

    /// Finds a model profile by its 16-bit numeric BLE `modelID` (e.g., `0x4D4A` -> MeloBuds Pro).
    pub fn find_by_numeric_id(&self, numeric_id: u16) -> Option<&DeviceModelProfile> {
        self.catalog
            .models
            .iter()
            .find(|m| m.numeric_model_id == Some(numeric_id))
    }

    /// Matches a discovered Bluetooth device to the best `DeviceModelProfile`:
    /// 1. Exact numeric `model_id` from parsed BLE `QcyAdvertisement` (highest priority).
    /// 2. Longest case-insensitive substring match against `ble_name_hints` (excluding generic `"QCY"` fallback hints).
    /// 3. Generic fallback (`qcy-generic`).
    pub fn match_discovered_device(
        &self,
        name: Option<&str>,
        adv: Option<&QcyAdvertisement>,
    ) -> &DeviceModelProfile {
        if let Some(a) = adv {
            if let Some(profile) = self.find_by_numeric_id(a.model_id) {
                return profile;
            }
        }

        if let Some(raw_name) = name {
            let lowered = raw_name.trim().to_ascii_lowercase();
            if !lowered.is_empty() {
                let mut best: Option<(usize, &DeviceModelProfile)> = None;
                for model in &self.catalog.models {
                    if model.model_id == "qcy-generic" {
                        continue;
                    }
                    for hint in &model.ble_name_hints {
                        let h = hint.trim().to_ascii_lowercase();
                        if !h.is_empty() && lowered.contains(&h) {
                            let score = h.len();
                            if best.map(|(s, _)| score > s).unwrap_or(true) {
                                best = Some((score, model));
                            }
                        }
                    }
                }
                if let Some((_, matched)) = best {
                    return matched;
                }
            }
        }

        self.find_by_id("qcy-generic")
            .unwrap_or(&self.catalog.models[0])
    }

    /// Returns a cloned copy of the generic fallback profile (`qcy-generic`).
    pub fn generic_fallback() -> DeviceModelProfile {
        let cat = embedded_models_catalog();
        cat.models
            .iter()
            .find(|m| m.model_id == "qcy-generic")
            .cloned()
            .unwrap_or_else(|| cat.models[0].clone())
    }

    /// Returns the full embedded EQ catalog (`eq_presets.json`).
    pub fn eq_catalog(&self) -> &EqCatalog {
        &self.eq_catalog
    }

    /// Returns all 22 built-in EQ presets.
    pub fn eq_presets(&self) -> &[EqPresetEntry] {
        &self.eq_catalog.presets
    }

    /// Finds a built-in EQ preset by string `id` (e.g., `"pop"`, `"heavy_bass"`).
    pub fn find_eq_preset(&self, id: &str) -> Option<&EqPresetEntry> {
        self.eq_catalog
            .presets
            .iter()
            .find(|p| p.id.eq_ignore_ascii_case(id))
    }

    /// Returns all 9 IIR biquad filter definitions (`0..=8`).
    pub fn filter_types(&self) -> &[FilterTypeMeta] {
        &self.eq_catalog.filter_types
    }

    /// Returns all 5 reference target FR curves (`20–20000 Hz`).
    pub fn target_curves(&self) -> &[TargetFrCurve] {
        &self.eq_catalog.target_curves
    }

    /// Returns the 8-band personalized hearing test configuration.
    pub fn hearing_test_config(&self) -> &HearingTestConfig {
        &self.eq_catalog.hearing_test
    }

    /// Returns the 14 button action metadata entries (`cmd 0..18`).
    pub fn button_actions(&self) -> &[ButtonActionMeta] {
        &self.catalog.button_action_catalog
    }

    /// Evaluates `pop_view_rules` for `model_id` when setting `feature_key` to `target_enabled`
    /// (`true => 1`, `false => 0`), given `current_state`.
    pub fn evaluate_pop_view_rules(
        &self,
        model_id: &str,
        feature_key: &str,
        target_enabled: bool,
        current_state: &HashMap<String, i32>,
    ) -> Vec<PopViewRuleMatch> {
        self.evaluate_pop_view_rules_value(
            model_id,
            feature_key,
            i32::from(target_enabled),
            current_state,
        )
    }

    /// Evaluates `pop_view_rules` for an arbitrary integer `target_value` (e.g. `10` for Spatial Audio EQ).
    pub fn evaluate_pop_view_rules_value(
        &self,
        model_id: &str,
        feature_key: &str,
        target_value: i32,
        current_state: &HashMap<String, i32>,
    ) -> Vec<PopViewRuleMatch> {
        let Some(model) = self.find_by_id(model_id) else {
            return Vec::new();
        };
        model.evaluate_pop_view_rules(feature_key, target_value, current_state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qcy_protocol::{
        BatteryLevel, IrrFilterType, OtaType, ProtocolFamily, QcyAdvertisement, COMPANY_ID_QCY,
    };

    #[test]
    fn embedded_catalog_and_eq_presets_parse_with_zero_errors() {
        let reg = ModelRegistry::default();
        assert_eq!(reg.models().len(), 43);
        assert_eq!(reg.button_actions().len(), 14);
        assert_eq!(reg.eq_presets().len(), 22);
        assert_eq!(reg.filter_types().len(), 9);
        assert_eq!(reg.target_curves().len(), 5);

        let generic = reg.find_by_id("qcy-generic").expect("generic profile");
        assert_eq!(generic.capabilities.eq_bands, 10);
        assert_eq!(generic.protocol_family, ProtocolFamily::GenericPlaceholder);

        // Verify every embedded model resolves cleanly to valid ProtocolFamily and optional OtaType
        for m in reg.models() {
            assert!(!m.model_id.is_empty());
            assert!(!m.name_en.is_empty());
            assert!(!m.name_zh.is_empty());
            if m.ota_type != 0 {
                assert!(
                    m.resolved_ota_type().is_some(),
                    "model {} has unrecognized ota_type {}",
                    m.model_id,
                    m.ota_type
                );
            }
            // Confirm serde round-trip on every model
            let json = serde_json::to_string(m).expect("serialize model");
            let parsed: DeviceModelProfile =
                serde_json::from_str(&json).expect("deserialize model");
            assert_eq!(&parsed, m);
        }

        // Verify all 9 filter types map 1:1 to wire IrrFilterType 0..=8
        for ft in reg.filter_types() {
            let wire = ft.wire_filter_type().expect("valid wire filter type");
            assert_eq!(wire.as_u8(), ft.index);
        }
        assert_eq!(IrrFilterType::Peak.as_u8(), 2);

        // Verify all 22 presets convert cleanly to wire graphic & parametric payloads
        for preset in reg.eq_presets() {
            let geq = preset.to_graphic_eq();
            assert_eq!(geq.gains_db.len(), 10);
            let peq = preset.to_parametric_eq(&reg.eq_catalog().frequency_grids.bands_10_hz);
            assert!(!peq.bands.is_empty());
        }
    }

    #[test]
    fn match_discovered_device_prefers_numeric_id_then_name_then_fallback() {
        let reg = ModelRegistry::default();

        // 1. Numeric modelID match via BLE advertisement (0x5D40 -> QCY MeloBuds N70)
        let adv_n70 = QcyAdvertisement {
            company_id: COMPANY_ID_QCY,
            model_id: 0x5D40,
            reserved_byte2: 0,
            is_left: true,
            color_index: 0,
            is_tws: true,
            is_pop: false,
            raw_flag_byte4: 0x40,
            left_battery: BatteryLevel {
                percent: Some(90),
                charging: false,
            },
            right_battery: BatteryLevel {
                percent: Some(85),
                charging: false,
            },
            case_battery: BatteryLevel {
                percent: Some(70),
                charging: false,
            },
            reserved_8_10: [0; 3],
            class_mac: None,
            pop_num: None,
            other_mac: None,
        };
        let matched = reg.match_discovered_device(Some("Unknown Name"), Some(&adv_n70));
        assert_eq!(matched.model_id, "qcy-n70");
        assert_eq!(matched.resolved_ota_type(), Some(OtaType::WuQiBleTone));
        assert_eq!(matched.protocol_family, ProtocolFamily::QcyUnifiedTlv);

        // 2. Name-hint match when advertisement is absent
        let by_name = reg.match_discovered_device(Some("QCY SP75 Speaker"), None);
        assert_eq!(by_name.model_id, "qcy-sp75");
        assert_eq!(by_name.form_factor, FormFactor::Speaker);
        assert!(by_name.capabilities.rgb_lighting);

        let h3 = reg.find_by_numeric_id(0x417C).expect("H3");
        assert_eq!(h3.model_id, "qcy-h3");
        assert_eq!(h3.protocol_family, ProtocolFamily::JieLiRcspLegacy);

        // 3. Fallback when unrecognized
        let fallback = reg.match_discovered_device(Some("Generic Earbuds"), None);
        assert_eq!(fallback.model_id, "qcy-generic");
    }

    #[test]
    fn pop_view_rules_detect_ldac_dual_device_conflict_and_reboot() {
        let reg = ModelRegistry::default();
        let mut state = HashMap::new();
        state.insert("dual_device".to_string(), 1);
        state.insert("ldac".to_string(), 0);

        let hits = reg.evaluate_pop_view_rules("qcy-ht08", "ldac", true, &state);
        assert!(
            !hits.is_empty(),
            "enabling LDAC while dual_device=1 should trigger a conflict rule"
        );
        assert!(hits.iter().any(|h| h.requires_reboot));
        assert!(hits
            .iter()
            .any(|h| h.conflicting_features.iter().any(|f| f == "dual_device")));
    }
}
