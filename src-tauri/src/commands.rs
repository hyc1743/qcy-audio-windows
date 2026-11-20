//! Complete 1-to-1 Tauri v2 IPC command surface matching `src/ipc/client.ts` (`IPC_COMMANDS`).

use crate::{autostart, popup, ShellRuntime};
use qcy_core::{
    AiAncMatrix, AppCore, AppSettingsPatch, AppSnapshot, CommandOutcome, DiscoveredDeviceInfo,
    EqPresetItem, FitTestResult, OtaState, PopupTriggerPayload, ProtocolLogEntry, ResetDeviceKind,
    SetAncPayload, SetAudioEnhancersPayload, SetHeadControlsPatch, SetModesPayload,
    SetMultipointPayload, SetRgbLightingPayload, SimulatorPatch, WireParametricBand,
};
use qcy_models::DeviceModelProfile;
use qcy_transport::AdapterInfo;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, State};

fn map_err<E: ToString>(err: E) -> String {
    err.to_string()
}

// ---------------------------------------------------------------------------
// Session, Discovery & Windows Bluetooth Settings
// ---------------------------------------------------------------------------

#[tauri::command(rename_all = "snake_case")]
pub fn get_app_snapshot(core: State<'_, AppCore>) -> AppSnapshot {
    core.get_app_snapshot()
}

#[tauri::command(rename_all = "snake_case")]
pub fn list_models(core: State<'_, AppCore>) -> Vec<DeviceModelProfile> {
    core.list_models()
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_adapter_info(core: State<'_, AppCore>) -> Result<AdapterInfo, String> {
    Ok(core.get_adapter_info().await)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn start_scan(core: State<'_, AppCore>) -> Result<(), String> {
    core.start_scan().await.map_err(map_err)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn stop_scan(core: State<'_, AppCore>) -> Result<(), String> {
    core.stop_scan().await.map_err(map_err)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn list_paired_devices(
    core: State<'_, AppCore>,
) -> Result<Vec<DiscoveredDeviceInfo>, String> {
    core.list_paired_devices().await.map_err(map_err)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn connect_device(
    core: State<'_, AppCore>,
    address: String,
    model_id: Option<String>,
) -> Result<AppSnapshot, String> {
    core.connect_device(&address, model_id.as_deref())
        .await
        .map_err(map_err)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn disconnect_device(core: State<'_, AppCore>) -> Result<AppSnapshot, String> {
    core.disconnect_device().await.map_err(map_err)
}

#[tauri::command(rename_all = "snake_case")]
pub fn remove_saved_device(
    core: State<'_, AppCore>,
    address: String,
) -> Result<AppSnapshot, String> {
    core.remove_saved_device(&address).map_err(map_err)
}

#[tauri::command(rename_all = "snake_case")]
pub fn open_windows_bt_settings() -> Result<(), String> {
    AppCore::open_windows_bt_settings().map_err(map_err)
}

// ---------------------------------------------------------------------------
// ANC & Noise Control
// ---------------------------------------------------------------------------

#[tauri::command(rename_all = "snake_case")]
pub async fn set_anc_state(
    core: State<'_, AppCore>,
    payload: SetAncPayload,
) -> Result<CommandOutcome, String> {
    core.set_anc_state(payload).await.map_err(map_err)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn set_ai_anc(
    core: State<'_, AppCore>,
    enabled: bool,
    matrix: Option<AiAncMatrix>,
) -> Result<CommandOutcome, String> {
    core.set_ai_anc(enabled, matrix).await.map_err(map_err)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn set_anc_button_cycle(
    core: State<'_, AppCore>,
    modes: Vec<String>,
) -> Result<CommandOutcome, String> {
    core.set_anc_button_cycle(modes).await.map_err(map_err)
}

// ---------------------------------------------------------------------------
// Equalizer & Audio Enhancers
// ---------------------------------------------------------------------------

#[tauri::command(rename_all = "snake_case")]
pub async fn apply_eq_preset(
    core: State<'_, AppCore>,
    preset_id: String,
) -> Result<CommandOutcome, String> {
    core.apply_eq_preset(&preset_id).await.map_err(map_err)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn set_graphic_eq(
    core: State<'_, AppCore>,
    gains_db: Vec<f32>,
    master_gain_db: Option<f32>,
) -> Result<CommandOutcome, String> {
    core.set_graphic_eq(gains_db, master_gain_db)
        .await
        .map_err(map_err)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn set_parametric_eq(
    core: State<'_, AppCore>,
    bands: Vec<WireParametricBand>,
    master_gain_db: Option<f32>,
) -> Result<CommandOutcome, String> {
    core.set_parametric_eq(bands, master_gain_db)
        .await
        .map_err(map_err)
}

#[tauri::command(rename_all = "snake_case")]
pub fn save_custom_eq(
    core: State<'_, AppCore>,
    preset: EqPresetItem,
) -> Result<AppSnapshot, String> {
    core.save_custom_eq(preset).map_err(map_err)
}

#[tauri::command(rename_all = "snake_case")]
pub fn delete_custom_eq(
    core: State<'_, AppCore>,
    preset_id: String,
) -> Result<AppSnapshot, String> {
    core.delete_custom_eq(&preset_id).map_err(map_err)
}

#[tauri::command(rename_all = "snake_case")]
pub fn import_eq_json(core: State<'_, AppCore>, json: String) -> Result<AppSnapshot, String> {
    core.import_eq_json(&json).map_err(map_err)
}

#[tauri::command(rename_all = "snake_case")]
pub fn export_eq_json(
    core: State<'_, AppCore>,
    preset_id: Option<String>,
) -> Result<String, String> {
    core.export_eq_json(preset_id.as_deref()).map_err(map_err)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn apply_personalized_eq(
    core: State<'_, AppCore>,
    left_gains_db: Vec<f32>,
    right_gains_db: Vec<f32>,
    combined_gains_db: Vec<f32>,
    balance: Option<f32>,
) -> Result<CommandOutcome, String> {
    core.apply_personalized_eq(left_gains_db, right_gains_db, combined_gains_db, balance)
        .await
        .map_err(map_err)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn set_audio_enhancers(
    core: State<'_, AppCore>,
    payload: SetAudioEnhancersPayload,
) -> Result<CommandOutcome, String> {
    core.set_audio_enhancers(payload).await.map_err(map_err)
}

// ---------------------------------------------------------------------------
// Gestures & IMU Head Controls
// ---------------------------------------------------------------------------

#[tauri::command(rename_all = "snake_case")]
pub async fn set_button_action(
    core: State<'_, AppCore>,
    key_id: u16,
    event_id: u16,
    cmd: u8,
) -> Result<CommandOutcome, String> {
    core.set_button_action(key_id, event_id, cmd)
        .await
        .map_err(map_err)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn reset_buttons(core: State<'_, AppCore>) -> Result<CommandOutcome, String> {
    core.reset_buttons().await.map_err(map_err)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn set_touch_config(
    core: State<'_, AppCore>,
    touch_enable: Option<bool>,
    touch_sensitivity: Option<u8>,
) -> Result<CommandOutcome, String> {
    core.set_touch_config(touch_enable, touch_sensitivity)
        .await
        .map_err(map_err)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn set_head_controls(
    core: State<'_, AppCore>,
    patch: SetHeadControlsPatch,
) -> Result<CommandOutcome, String> {
    core.set_head_controls(patch).await.map_err(map_err)
}

// ---------------------------------------------------------------------------
// Modes, Connectivity, RGB, Focus Mode, Fit Test, Maintenance & Local OTA
// ---------------------------------------------------------------------------

#[tauri::command(rename_all = "snake_case")]
pub async fn set_modes(
    core: State<'_, AppCore>,
    payload: SetModesPayload,
) -> Result<CommandOutcome, String> {
    core.set_modes(payload).await.map_err(map_err)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn set_multipoint(
    core: State<'_, AppCore>,
    payload: SetMultipointPayload,
) -> Result<CommandOutcome, String> {
    core.set_multipoint(payload).await.map_err(map_err)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn set_rgb_lighting(
    core: State<'_, AppCore>,
    payload: SetRgbLightingPayload,
) -> Result<CommandOutcome, String> {
    core.set_rgb_lighting(payload).await.map_err(map_err)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn set_focus_white_noise(
    core: State<'_, AppCore>,
    playing: Option<bool>,
    track_index: Option<u32>,
    volume: Option<u8>,
) -> Result<CommandOutcome, String> {
    core.set_focus_white_noise(playing, track_index, volume)
        .await
        .map_err(map_err)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn run_fit_test(core: State<'_, AppCore>) -> Result<FitTestResult, String> {
    core.run_fit_test().await.map_err(map_err)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn set_device_color(
    core: State<'_, AppCore>,
    color_index: u8,
) -> Result<AppSnapshot, String> {
    core.set_device_color(color_index).await.map_err(map_err)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn rename_device(
    core: State<'_, AppCore>,
    name: String,
) -> Result<CommandOutcome, String> {
    core.rename_device(&name).await.map_err(map_err)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn reset_device(
    core: State<'_, AppCore>,
    kind: ResetDeviceKind,
) -> Result<CommandOutcome, String> {
    core.reset_device(kind).await.map_err(map_err)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn start_local_ota(
    core: State<'_, AppCore>,
    file_path: Option<String>,
    file_name: Option<String>,
    bytes: Option<Vec<u8>>,
) -> Result<OtaState, String> {
    if let Some(raw_bytes) = bytes {
        let label = file_name
            .or(file_path)
            .unwrap_or_else(|| "firmware.ota".to_string());
        core.start_local_ota_bytes(&label, raw_bytes)
            .await
            .map_err(map_err)
    } else if let Some(path) = file_path {
        core.start_local_ota(&path).await.map_err(map_err)
    } else {
        Err("start_local_ota requires either `file_path` or `bytes`".to_string())
    }
}

#[tauri::command(rename_all = "snake_case")]
pub fn cancel_ota(core: State<'_, AppCore>) -> OtaState {
    core.cancel_ota()
}

// ---------------------------------------------------------------------------
// Virtual QCY Earbuds Simulator
// ---------------------------------------------------------------------------

#[tauri::command(rename_all = "snake_case")]
pub async fn set_simulator_enabled(
    core: State<'_, AppCore>,
    enabled: bool,
) -> Result<AppSnapshot, String> {
    core.set_simulator_enabled(enabled).await.map_err(map_err)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn set_simulator_model(
    core: State<'_, AppCore>,
    model_id: String,
) -> Result<AppSnapshot, String> {
    core.set_simulator_model(&model_id).await.map_err(map_err)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn update_simulator_state(
    core: State<'_, AppCore>,
    patch: SimulatorPatch,
) -> Result<AppSnapshot, String> {
    core.update_simulator_state(patch).await.map_err(map_err)
}

#[tauri::command(rename_all = "snake_case")]
pub fn trigger_simulator_popup(core: State<'_, AppCore>) -> PopupTriggerPayload {
    core.trigger_simulator_popup()
}

// ---------------------------------------------------------------------------
// App Settings, Pop-Up Overlay & Protocol Frame Logs
// ---------------------------------------------------------------------------

#[tauri::command(rename_all = "snake_case")]
pub fn update_app_settings(
    core: State<'_, AppCore>,
    shell: State<'_, ShellRuntime>,
    patch: AppSettingsPatch,
) -> Result<AppSnapshot, String> {
    if let Some(enabled) = patch.auto_start {
        autostart::set_autostart_enabled(enabled)?;
    }
    if let Some(min_tray) = patch.minimize_to_tray {
        shell.minimize_to_tray.store(min_tray, Ordering::SeqCst);
    }
    if let Some(show_pop) = patch.show_connection_popup {
        shell
            .show_connection_popup
            .store(show_pop, Ordering::SeqCst);
    }
    core.update_app_settings(patch).map_err(map_err)
}

#[tauri::command(rename_all = "snake_case")]
pub fn show_popup(app: AppHandle, core: State<'_, AppCore>) -> Result<(), String> {
    let payload = core.build_popup_payload();
    popup::show_popup_window(&app, Some(&payload))
}

#[tauri::command(rename_all = "snake_case")]
pub fn hide_popup(app: AppHandle) -> Result<(), String> {
    popup::hide_popup_window(&app)
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_protocol_logs(core: State<'_, AppCore>) -> Vec<ProtocolLogEntry> {
    core.get_protocol_logs()
}

#[tauri::command(rename_all = "snake_case")]
pub fn clear_protocol_logs(core: State<'_, AppCore>) {
    core.clear_protocol_logs();
}
