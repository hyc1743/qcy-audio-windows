//! Strongly-typed QCY device model catalog, 56-flag capability matrix,
//! button action catalog, and `pop_view_rules` mutual-exclusion evaluator.

use std::collections::HashMap;

use qcy_protocol::{OtaTransport, OtaType, ProtocolFamily};
use qcy_transport::TransportKind;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

fn deserialize_protocol_family<'de, D>(deserializer: D) -> Result<ProtocolFamily, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;
    match raw.as_str() {
        "qcy_unified_tlv" => Ok(ProtocolFamily::QcyUnifiedTlv),
        "jieli_rcsp_new" | "jie_li_rcsp_new" => Ok(ProtocolFamily::JieLiRcspNew),
        "jieli_rcsp_legacy" | "jie_li_rcsp_legacy" => Ok(ProtocolFamily::JieLiRcspLegacy),
        "legacy_direct_gatt" => Ok(ProtocolFamily::LegacyDirectGatt),
        "generic_placeholder" => Ok(ProtocolFamily::GenericPlaceholder),
        other => Ok(ProtocolFamily::Custom(other.to_string())),
    }
}

fn serialize_protocol_family<S>(family: &ProtocolFamily, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let s = match family {
        ProtocolFamily::QcyUnifiedTlv => "qcy_unified_tlv",
        ProtocolFamily::JieLiRcspNew => "jieli_rcsp_new",
        ProtocolFamily::JieLiRcspLegacy => "jieli_rcsp_legacy",
        ProtocolFamily::LegacyDirectGatt => "legacy_direct_gatt",
        ProtocolFamily::GenericPlaceholder => "generic_placeholder",
        ProtocolFamily::Custom(name) => name.as_str(),
    };
    serializer.serialize_str(s)
}

/// Top-level schema of `crates/qcy-models/data/models_catalog.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelsCatalog {
    pub schema_version: u32,
    pub description: String,
    pub form_factors: Vec<FormFactor>,
    pub chipset_ota_map: HashMap<String, String>,
    pub button_action_catalog: Vec<ButtonActionMeta>,
    pub models: Vec<DeviceModelProfile>,
}

/// Hardware form factor of a QCY audio product.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormFactor {
    Tws,
    OverEar,
    OpenEar,
    Speaker,
    Neckband,
}

/// Catalog entry for a button / gesture action (`cmd 0..18`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ButtonActionMeta {
    pub cmd: u8,
    pub id: String,
    pub name_en: String,
    pub name_zh: String,
}

/// Complete capability matrix matching `models_catalog.json` (`capabilities`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceCapabilities {
    pub anc: bool,
    pub ai_anc: bool,
    pub anc_custom_levels: u8,
    pub transparency_levels: u8,
    pub vocal_boost: bool,
    pub wind_noise: bool,
    pub anc_button_cycle: bool,
    pub anc_scenes: Vec<String>,
    pub parametric_eq: bool,
    pub custom_eq: bool,
    pub eq_bands: u8,
    pub max_eq_bands: u8,
    pub supported_filter_types: Vec<u8>,
    pub personalized_eq: bool,
    pub dynamic_eq: bool,
    pub thx_eq: bool,
    pub virtual_bass: bool,
    pub adaptive_volume: bool,
    pub channel_balance: bool,
    pub spatial_audio_modes: Vec<String>,
    pub listening_modes: Vec<String>,
    pub game_mode: bool,
    pub game_mode_pro: bool,
    pub ldac: bool,
    pub le_audio: bool,
    pub button_remap: bool,
    pub supported_button_actions: Vec<u8>,
    pub button_keys: Vec<String>,
    pub touch_enable: bool,
    pub touch_sensitivity: bool,
    pub head_controls: bool,
    pub in_ear_detection: bool,
    pub fit_test: bool,
    pub hearing_health: bool,
    pub privacy_mode_pro: bool,
    pub dialogue_perception: bool,
    pub speak_to_chat: bool,
    pub dual_device: bool,
    pub multi_device: bool,
    pub rgb_lighting: bool,
    pub tws_speaker_pair: bool,
    pub focus_mode_white_noise: bool,
    pub find_earbuds: bool,
    pub prompt_language: bool,
    pub prompt_volume: bool,
    pub auto_power_off: bool,
    pub disconnect_power_off: bool,
    pub power_saving: bool,
    pub disconnect_warning: bool,
    pub device_color: bool,
    pub rename: bool,
    pub reset: bool,
    pub factory_reset: bool,
    pub clear_pairing: bool,
    pub ota: bool,
    pub has_case_battery: bool,
}

/// UI presentation mode for a triggered `pop_view_rules` guard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleAction {
    ConfirmDialog,
    Toast,
}

/// Comparison operator used in `pop_view_rules[].conflicts_with`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleCompareOp {
    Eq,
    Ne,
}

/// Single conflicting feature condition inside a `PopViewRule`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleConflictCondition {
    pub feature: String,
    pub cmd_id: u16,
    pub compare: RuleCompareOp,
    pub value: i32,
    pub reboot: bool,
}

impl RuleConflictCondition {
    /// Returns `true` if `current_state` satisfies this conflict condition.
    pub fn matches(&self, current_state: &HashMap<String, i32>) -> bool {
        let cur = current_state.get(&self.feature).copied().unwrap_or(0);
        match self.compare {
            RuleCompareOp::Eq => cur == self.value,
            RuleCompareOp::Ne => cur != self.value,
        }
    }
}

/// Mutual-exclusion or reboot guard rule from `models_catalog.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PopViewRule {
    pub target_feature: String,
    pub target_cmd_id: u16,
    pub target_value: Option<i32>,
    pub action: RuleAction,
    pub requires_reboot: bool,
    pub title_key: Option<String>,
    pub message_key: String,
    pub reboot_notice_key: Option<String>,
    pub conflicts_with: Vec<RuleConflictCondition>,
}

/// Result returned when a `PopViewRule` triggers for a candidate feature toggle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PopViewRuleMatch {
    pub target_feature: String,
    pub target_cmd_id: u16,
    pub action: RuleAction,
    pub requires_reboot: bool,
    pub title_key: Option<String>,
    pub message_key: String,
    pub reboot_notice_key: Option<String>,
    pub conflicting_features: Vec<String>,
}

/// Full data-driven QCY audio device profile deserialized from `models_catalog.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceModelProfile {
    pub model_id: String,
    pub numeric_model_id: Option<u16>,
    pub numeric_model_id_hex: Option<String>,
    pub source: String,
    pub codename: String,
    pub name_en: String,
    pub name_zh: String,
    pub form_factor: FormFactor,
    pub chipset_family: String,
    #[serde(
        deserialize_with = "deserialize_protocol_family",
        serialize_with = "serialize_protocol_family"
    )]
    pub protocol_family: ProtocolFamily,
    pub ota_type: u8,
    pub ble_name_hints: Vec<String>,
    pub capabilities: DeviceCapabilities,
    pub default_eq_preset_ids: Vec<String>,
    pub pop_view_rules: Vec<PopViewRule>,
}

impl DeviceModelProfile {
    /// Resolves the numeric `ota_type` field into a typed [`OtaType`], or `None` when `ota_type == 0`.
    pub fn resolved_ota_type(&self) -> Option<OtaType> {
        OtaType::from_u8(self.ota_type)
    }

    /// Returns the transport kind needed by this model (daily control is always BLE;
    /// Classic RFCOMM is additionally needed only for `otaType` 1/5/7).
    pub fn transport_kind(&self) -> TransportKind {
        match self.resolved_ota_type().map(OtaType::route_info) {
            Some(info) if info.transport == OtaTransport::ClassicRfcomm => TransportKind::Dual,
            _ => TransportKind::Ble,
        }
    }

    /// Evaluates this profile's `pop_view_rules` for changing `feature_key` to `target_value`
    /// given the current device feature state map.
    pub fn evaluate_pop_view_rules(
        &self,
        feature_key: &str,
        target_value: i32,
        current_state: &HashMap<String, i32>,
    ) -> Vec<PopViewRuleMatch> {
        let mut matches = Vec::new();

        for rule in &self.pop_view_rules {
            if rule.target_feature != feature_key {
                continue;
            }
            if let Some(expected_target) = rule.target_value {
                if expected_target != target_value {
                    continue;
                }
            }

            if rule.conflicts_with.is_empty() {
                // Pure reboot/confirmation guard (e.g., toggling LDAC or Dual-Device always reboots).
                let current_val = current_state.get(feature_key).copied();
                if current_val != Some(target_value) {
                    matches.push(PopViewRuleMatch {
                        target_feature: rule.target_feature.clone(),
                        target_cmd_id: rule.target_cmd_id,
                        action: rule.action,
                        requires_reboot: rule.requires_reboot,
                        title_key: rule.title_key.clone(),
                        message_key: rule.message_key.clone(),
                        reboot_notice_key: rule.reboot_notice_key.clone(),
                        conflicting_features: Vec::new(),
                    });
                }
            } else {
                let conflicting_features: Vec<String> = rule
                    .conflicts_with
                    .iter()
                    .filter(|cond| cond.matches(current_state))
                    .map(|cond| cond.feature.clone())
                    .collect();

                if !conflicting_features.is_empty() {
                    let any_conflict_reboots = rule
                        .conflicts_with
                        .iter()
                        .any(|c| c.reboot && c.matches(current_state));
                    matches.push(PopViewRuleMatch {
                        target_feature: rule.target_feature.clone(),
                        target_cmd_id: rule.target_cmd_id,
                        action: rule.action,
                        requires_reboot: rule.requires_reboot || any_conflict_reboots,
                        title_key: rule.title_key.clone(),
                        message_key: rule.message_key.clone(),
                        reboot_notice_key: rule.reboot_notice_key.clone(),
                        conflicting_features,
                    });
                }
            }
        }

        matches
    }
}
