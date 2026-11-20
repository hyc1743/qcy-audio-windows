//! Strongly-typed clean-room EQ presets, frequency grids, 9 IIR biquad filter
//! metadata definitions, reference target FR curves, and 8-band personalized
//! hearing test configuration (`crates/qcy-models/data/eq_presets.json`).

use qcy_protocol::{GraphicEqSetting, IrrFilterType, ParametricEqBand, ParametricEqPreset};
use serde::{Deserialize, Serialize};

/// Top-level schema of `crates/qcy-models/data/eq_presets.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EqCatalog {
    pub schema_version: u32,
    pub description: String,
    pub frequency_grids: FrequencyGrids,
    pub filter_types: Vec<FilterTypeMeta>,
    pub presets: Vec<EqPresetEntry>,
    pub target_curves: Vec<TargetFrCurve>,
    pub hearing_test: HearingTestConfig,
}

/// Standard 5/10/20-band EQ frequency grids and parameter bounds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrequencyGrids {
    pub bands_5_hz: Vec<u16>,
    pub bands_10_hz: Vec<u16>,
    pub bands_10_peq_hz: Vec<u16>,
    pub bands_20_hz: Vec<u16>,
    pub freq_min_hz: u16,
    pub freq_max_hz: u16,
    pub gain_min_db: f32,
    pub gain_max_db: f32,
    pub q_min: f32,
    pub q_max: f32,
    pub q_default: f32,
}

/// Metadata for each of the 9 QCY IIR biquad filter types (`0..=8`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilterTypeMeta {
    pub index: u8,
    pub id: String,
    pub name_en: String,
    pub name_zh: String,
    pub description_en: String,
    pub description_zh: String,
    pub supports_gain: bool,
    pub supports_q: bool,
    pub default_q: f32,
}

impl FilterTypeMeta {
    /// Converts `index` into the corresponding wire [`IrrFilterType`].
    pub fn wire_filter_type(&self) -> Option<IrrFilterType> {
        IrrFilterType::from_u8(self.index)
    }
}

/// Single parametric filter band inside an `EqPresetEntry`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PresetParametricBand {
    pub freq_hz: u16,
    pub gain_db: f32,
    pub q: f32,
    pub filter_type: u8,
}

impl PresetParametricBand {
    /// Converts this catalog band into a wire-ready [`ParametricEqBand`].
    pub fn to_protocol_band(self) -> ParametricEqBand {
        ParametricEqBand {
            freq_hz: self.freq_hz,
            gain_db: self.gain_db,
            q: self.q,
            filter_type: IrrFilterType::from_u8(self.filter_type).unwrap_or(IrrFilterType::Peak),
        }
    }
}

/// Built-in clean-room EQ preset entry (`eq_presets.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EqPresetEntry {
    pub id: String,
    pub eq_index: Option<u8>,
    pub category: String,
    pub name_en: String,
    pub name_zh: String,
    pub subtitle_en: String,
    pub subtitle_zh: String,
    pub master_gain_db: f32,
    pub gains_10band_db: Vec<f32>,
    pub parametric_bands: Vec<PresetParametricBand>,
}

impl EqPresetEntry {
    /// Converts this preset into a wire-ready 10-band [`GraphicEqSetting`] (`0xFC` / `0x000B`).
    pub fn to_graphic_eq(&self) -> GraphicEqSetting {
        GraphicEqSetting {
            eq_type: self.eq_index.unwrap_or(0),
            gains_db: self
                .gains_10band_db
                .iter()
                .map(|g| g.round().clamp(-12.0, 12.0) as i8)
                .collect(),
            game_eq_type: None,
            current_mode: None,
        }
    }

    /// Converts this preset into a wire-ready [`ParametricEqPreset`] (`0x20` / `0x22`).
    /// If `parametric_bands` is empty (e.g., flat `default`), builds 10 peak bands from `gains_10band_db`.
    pub fn to_parametric_eq(&self, default_freqs_10: &[u16]) -> ParametricEqPreset {
        let bands = if !self.parametric_bands.is_empty() {
            self.parametric_bands
                .iter()
                .copied()
                .map(PresetParametricBand::to_protocol_band)
                .collect()
        } else {
            self.gains_10band_db
                .iter()
                .zip(default_freqs_10.iter())
                .map(|(&gain, &freq)| ParametricEqBand::peak(freq, gain, 1.41))
                .collect()
        };
        ParametricEqPreset {
            eq_type: self.eq_index.unwrap_or(0),
            master_gain_db: self.master_gain_db,
            bands,
        }
    }
}

/// Single `(freq_hz, db)` point on a reference frequency-response curve.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TargetCurvePoint {
    pub freq_hz: u16,
    pub db: f32,
}

/// Reference target FR curve overlay (`20–20000 Hz`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TargetFrCurve {
    pub id: String,
    pub name_en: String,
    pub name_zh: String,
    pub description_en: String,
    pub description_zh: String,
    pub points: Vec<TargetCurvePoint>,
}

/// 8-band personalized hearing test configuration (`hearing_test`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HearingTestConfig {
    pub eq_index: u8,
    pub bands_hz: Vec<u16>,
    pub audiometry_alt_bands_hz: Vec<u16>,
    pub default_q: f32,
    pub filter_type: u8,
    pub master_gain_db: f32,
    pub reference_sound_db: Vec<f32>,
    pub min_compensation_db: f32,
    pub max_compensation_db: f32,
    pub tone_duration_ms: u32,
    pub steps_per_band: u8,
}
