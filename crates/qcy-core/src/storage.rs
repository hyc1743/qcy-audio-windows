//! Atomic local JSON persistence (`%APPDATA%\QcyAudioDesktop\state.json`)
//! storing `AppSettings`, custom EQ presets, and saved / last-connected device records.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::types::{AppSettings, CoreError, CoreResult, EqPresetItem, SavedDeviceRecord};

/// Persisted on-disk state schema (`state.json`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PersistedState {
    #[serde(default)]
    pub settings: AppSettings,
    #[serde(default)]
    pub custom_presets: Vec<EqPresetItem>,
    #[serde(default)]
    pub saved_devices: Vec<SavedDeviceRecord>,
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
    pub fn load(&self) -> CoreResult<PersistedState> {
        if !self.path.exists() {
            return Ok(PersistedState::default());
        }
        let raw = fs::read_to_string(&self.path).map_err(|e| {
            CoreError::Storage(format!("failed to read {}: {e}", self.path.display()))
        })?;
        serde_json::from_str(&raw).map_err(|e| {
            CoreError::Storage(format!("failed to parse {}: {e}", self.path.display()))
        })
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
