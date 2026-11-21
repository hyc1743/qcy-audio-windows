//! Atomic local JSON persistence (`%APPDATA%\QcyAudioDesktop\state.json`)
//! storing `AppSettings`, custom EQ presets, and saved / last-connected device records,
//! with automatic schema migration (`v0/v1 -> v2`).

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::types::{AppSettings, CoreError, CoreResult, EqPresetItem, SavedDeviceRecord};

/// Current on-disk persistence schema version.
///
/// - `0` / `1` (`v0.1.0`–`v0.1.1`): `simulator_enabled` defaulted to `true` and seeded a virtual device.
/// - `2` (`v0.1.2+`): `simulator_enabled` defaults to `false`; clean disconnected startup with zero virtual devices.
pub const CURRENT_SCHEMA_VERSION: u32 = 2;

/// Returns `true` if a `SavedDeviceRecord` represents a synthetic simulator device.
pub fn is_simulator_saved_device(rec: &SavedDeviceRecord) -> bool {
    if rec.is_virtual {
        return true;
    }
    let upper = rec.address.trim().to_ascii_uppercase();
    upper.starts_with("AA:BB:CC:") || upper.starts_with("51:43:59:") || upper == "84:AC:60:5D:40:01"
}

/// Persisted on-disk state schema (`state.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersistedState {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub settings: AppSettings,
    #[serde(default)]
    pub custom_presets: Vec<EqPresetItem>,
    #[serde(default)]
    pub saved_devices: Vec<SavedDeviceRecord>,
}

impl Default for PersistedState {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            settings: AppSettings::default(),
            custom_presets: Vec::new(),
            saved_devices: Vec::new(),
        }
    }
}

/// Atomic file-backed persistence store with injectable path for tests.
#[derive(Debug, Clone)]
pub struct StorageStore {
    path: PathBuf,
}

impl StorageStore {
    /// Creates a store targeting a specific JSON file path (ideal for deterministic tests).
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Resolves the default Windows `%APPDATA%\QcyAudioDesktop\state.json` (or fallback).
    pub fn default_app_path() -> PathBuf {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata)
                .join("QcyAudioDesktop")
                .join("state.json");
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join(".config")
                .join("QcyAudioDesktop")
                .join("state.json");
        }
        std::env::temp_dir()
            .join("QcyAudioDesktop")
            .join("state.json")
    }

    pub fn default_store() -> Self {
        Self::new(Self::default_app_path())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Loads persisted state from disk, returning `PersistedState::default()` if the file does not exist yet.
    ///
    /// Automatically migrates legacy v0/v1 `state.json` files (`schema_version < 2`) so users upgrading
    /// from `v0.1.0`/`v0.1.1` start with `simulator_enabled = false` and zero virtual devices in `saved_devices`.
    pub fn load(&self) -> CoreResult<PersistedState> {
        if !self.path.exists() {
            return Ok(PersistedState::default());
        }
        let raw = fs::read_to_string(&self.path).map_err(|e| {
            CoreError::Storage(format!("failed to read {}: {e}", self.path.display()))
        })?;
        let mut state: PersistedState = serde_json::from_str(&raw).map_err(|e| {
            CoreError::Storage(format!("failed to parse {}: {e}", self.path.display()))
        })?;

        if state.schema_version < CURRENT_SCHEMA_VERSION {
            state.settings.simulator_enabled = false;
            state
                .saved_devices
                .retain(|rec| !is_simulator_saved_device(rec));
            state.schema_version = CURRENT_SCHEMA_VERSION;
            let _ = self.save(&state);
        }

        Ok(state)
    }

    /// Atomically writes `state` to disk via sibling `.tmp` file + rename/replace.
    pub fn save(&self, state: &PersistedState) -> CoreResult<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                CoreError::Storage(format!("failed to create {}: {e}", parent.display()))
            })?;
        }

        let json = serde_json::to_string_pretty(state)
            .map_err(|e| CoreError::Storage(format!("failed to serialize state: {e}")))?;

        let tmp_path = self.path.with_extension("json.tmp");
        fs::write(&tmp_path, json.as_bytes()).map_err(|e| {
            CoreError::Storage(format!("failed to write {}: {e}", tmp_path.display()))
        })?;

        // On Windows, fs::rename fails if destination exists; remove destination first if present.
        if self.path.exists() {
            let _ = fs::remove_file(&self.path);
        }
        fs::rename(&tmp_path, &self.path).map_err(|e| {
            CoreError::Storage(format!(
                "failed to commit {} -> {}: {e}",
                tmp_path.display(),
                self.path.display()
            ))
        })?;

        Ok(())
    }
}
