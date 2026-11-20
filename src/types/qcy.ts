/**
 * Canonical Frontend Domain Types & Tauri IPC Contract (snake_case matching Rust serde).
 *
 * Shared source of truth between the React frontend (`src/ipc/client.ts`, `src/store/useQcyStore.ts`)
 * and the Rust desktop host (`src-tauri`, `crates/qcy-core`, `crates/qcy-models`, `crates/qcy-protocol`).
 */

import type { DeviceFormFactor, SpeakerRgbMode } from "../assets/DeviceIllustrations";
import type { SupportedLocale } from "../i18n";

export type TransportKind = "ble" | "classic_rfcomm" | "dual";
export type ConnectionState = "disconnected" | "connecting" | "connected" | "reconnecting";
export type AppTheme = "system" | "light" | "dark";

export type ProtocolFamily =
  | "qcy_unified_tlv"
  | "jieli_rcsp_new"
  | "jieli_rcsp_legacy"
  | "legacy_direct_gatt"
  | "generic_placeholder";

export interface AdapterInfo {
  is_present: boolean;
  address: string | null;
  is_classic_supported: boolean;
  is_low_energy_supported: boolean;
  is_peripheral_supported: boolean;
  error: string | null;
}

export interface DiscoveredDevice {
  id: string;
  address: string;
  name: string;
  rssi: number | null;
  transport_kind: TransportKind;
  manufacturer_data: Record<string, number[]>;
  service_uuids: string[];
  is_paired: boolean;
  is_connected: boolean;
  last_seen_ms: number;
  model_id_hint?: string | null;
  numeric_model_id?: number | null;
  is_virtual?: boolean;
}

export interface SavedDeviceRecord {
  address: string;
  name: string;
  model_id: string;
  color_index: number;
  last_connected_ms: number;
  last_battery?: BatteryTelemetry | null;
  is_virtual?: boolean;
}

export interface ConflictCondition {
  feature: string;
  cmd_id: number;
  compare: "eq" | "neq" | "gt" | "lt";
  value: number;
  reboot: boolean;
}

export interface PopViewRule {
  target_feature: string;
  target_cmd_id: number;
  target_value: number;
  action: "confirm_dialog" | "toast_blocked";
  requires_reboot: boolean;
  title_key: string;
  message_key: string;
  reboot_notice_key?: string | null;
  conflicts_with: ConflictCondition[];
}

export interface MutualExclusionPrompt {
  rule: PopViewRule;
  conflicting_features: string[];
  requires_reboot: boolean;
  title_key: string;
  message_key: string;
  reboot_notice_key?: string | null;
}

/** Matches `crates/qcy-models/data/models_catalog.json` capabilities schema (all 47 flags). */
export interface DeviceCapabilities {
  anc: boolean;
  ai_anc: boolean;
  anc_custom_levels: number;
  transparency_levels: number;
  vocal_boost: boolean;
  wind_noise: boolean;
  anc_button_cycle: boolean;
  anc_scenes: string[];
  parametric_eq: boolean;
  custom_eq: boolean;
  eq_bands: number;
  max_eq_bands: number;
  supported_filter_types: number[];
  personalized_eq: boolean;
  dynamic_eq: boolean;
  thx_eq: boolean;
  virtual_bass: boolean;
  adaptive_volume: boolean;
  channel_balance: boolean;
  spatial_audio_modes: string[];
  listening_modes: string[];
  game_mode: boolean;
  game_mode_pro: boolean;
  ldac: boolean;
  le_audio: boolean;
  button_remap: boolean;
  supported_button_actions: number[];
  button_keys: string[];
  touch_enable: boolean;
  touch_sensitivity: boolean;
  head_controls: boolean;
  in_ear_detection: boolean;
  fit_test: boolean;
  hearing_health: boolean;
  privacy_mode_pro: boolean;
  dialogue_perception: boolean;
  speak_to_chat: boolean;
  dual_device: boolean;
  multi_device: boolean;
  rgb_lighting: boolean;
  tws_speaker_pair: boolean;
  focus_mode_white_noise: boolean;
  find_earbuds: boolean;
  prompt_language: boolean;
  prompt_volume: boolean;
  auto_power_off: boolean;
  disconnect_power_off: boolean;
  power_saving: boolean;
  disconnect_warning: boolean;
  device_color: boolean;
  rename: boolean;
  reset: boolean;
  factory_reset: boolean;
  clear_pairing: boolean;
  ota: boolean;
  has_case_battery: boolean;
}

export interface DeviceModelProfile {
  model_id: string;
  numeric_model_id: number | null;
  numeric_model_id_hex: string | null;
  source: string;
  codename: string;
  name_en: string;
  name_zh: string;
  form_factor: DeviceFormFactor;
  chipset_family: string;
  protocol_family: ProtocolFamily;
  ota_type: number;
  ble_name_hints: string[];
  capabilities: DeviceCapabilities;
  default_eq_preset_ids: string[];
  pop_view_rules: PopViewRule[];
}

export interface BatteryTelemetry {
  left: number | null;
  right: number | null;
  case_level: number | null;
  left_charging: boolean;
  right_charging: boolean;
  case_charging: boolean;
  left_in_case: boolean;
  right_in_case: boolean;
  is_tws_connected: boolean;
}

export type AncPrimaryMode = "off" | "anc" | "transparency" | "custom";
export type AncScene =
  | "adaptive"
  | "indoor"
  | "commute"
  | "outdoor"
  | "anti_wind"
  | "high"
  | "medium"
  | "low";

export type AiAncActivity = "detecting" | "stay" | "walk" | "run";
export type AiAncNoiseLevel = "low" | "medium" | "high";

export interface AiAncMatrix {
  stay: Record<AiAncNoiseLevel, AncPrimaryMode>;
  walk: Record<AiAncNoiseLevel, AncPrimaryMode>;
  run: Record<AiAncNoiseLevel, AncPrimaryMode>;
}

export interface AncState {
  mode: AncPrimaryMode;
  scene: AncScene;
  scene_level: number; // 0..2
  custom_level: number; // 0..15 (H6 16-level ANC)
  transparency_level: number; // 1..6
  vocal_boost: boolean;
  wind_noise: boolean;
  ai_anc_enabled: boolean;
  ai_activity: AiAncActivity;
  ai_noise_level: AiAncNoiseLevel;
  ai_matrix: AiAncMatrix;
  btn_anc_cfg: string[];
}

export interface WireParametricBand {
  freq_hz: number;
  gain_db: number;
  q: number;
  /** QCY `EQParamBean.irrType` (`0..8`, or `0xFF` PAD). */
  irr_type: number;
  enabled: boolean;
}

export interface EqPresetItem {
  id: string;
  eq_index: number;
  category: "official" | "personalized" | "speaker" | "curated" | "custom";
  name_en: string;
  name_zh: string;
  subtitle_en?: string;
  subtitle_zh?: string;
  master_gain_db: number;
  gains_10band_db: number[];
  parametric_bands: WireParametricBand[];
}

export interface TargetCurveItem {
  id: string;
  name_en: string;
  name_zh: string;
  description_en: string;
  description_zh: string;
  points: Array<{ freq_hz: number; db: number }>;
}

export interface EqState {
  active_preset_id: string;
  eq_mode: "graphic" | "parametric";
  master_gain_db: number;
  gains_10band_db: number[];
  parametric_bands: WireParametricBand[];
  active_target_curve_id: string | null;
  personalized_enabled: boolean;
  personalized_left_db: number[];
  personalized_right_db: number[];
  custom_presets: EqPresetItem[];
  dynamic_eq: boolean;
  thx_eq: boolean;
  virtual_bass: boolean;
  adaptive_volume: boolean;
  /** `-1.0` (full Left) .. `0.0` (Center) .. `+1.0` (full Right). */
  channel_balance: number;
}

export interface ButtonMappingEntry {
  key_id: number;
  event_id: number;
  cmd: number;
}

export interface HeadControlsState {
  enabled: boolean;
  nod_cmd: number; // 1=answer, 2=reject
  shake_cmd: number; // 1=answer, 2=reject
  tilt_left_cmd: number; // 3=prev, 4=next
  tilt_right_cmd: number; // 3=prev, 4=next
}

export interface GestureState {
  touch_enable: boolean;
  /** 38 (moderate), 45 (normal), 52 (sensitive). */
  touch_sensitivity: number;
  mappings: ButtonMappingEntry[];
  head_controls: HeadControlsState;
}

export type ListeningMode = "music" | "game" | "movie";
export type SpatialAudioMode = "off" | "fixed" | "head_tracking";
export type PromptLanguage = "en" | "zh";

export interface ModesState {
  listening_mode: ListeningMode;
  game_mode: boolean;
  game_mode_pro: boolean;
  game_sync_eq: boolean;
  spatial_audio: SpatialAudioMode;
  ldac: boolean;
  le_audio: boolean;
  /** 0=off, 1=single pause/wear play, 2=single pause/no auto-play, 3=both pause/wear play. */
  in_ear_mode: number;
  in_ear_auto_anc: boolean;
  /** 1=moderate, 2=normal, 3=sensitive. */
  in_ear_sensitivity: number;
  hearing_low_vol_remind: boolean;
  hearing_duration_remind: boolean;
  hearing_duration_min: number;
  privacy_mode: boolean;
  /** 0=calls & music, 1=calls only. */
  privacy_scope: number;
  dialogue_perception: boolean;
  /** 0=off, or 5/10/15/20 seconds. */
  speak_to_chat_sec: number;
  /** 0 / 65535 = never, else minutes. */
  auto_power_off_min: number;
  disconnect_power_off_min: number;
  power_saving: boolean;
  disconnect_warning: boolean;
  prompt_language: PromptLanguage;
  /** 4=low, 6=medium, 8=high, 10=max. */
  prompt_volume: number;
}

export interface MultipointHostInfo {
  mac: string;
  name: string;
  is_this_device: boolean;
  is_connected: boolean;
}

export interface MultipointState {
  dual_device: boolean;
  multi_device: boolean;
  hosts: MultipointHostInfo[];
}

export interface RgbLightingState {
  enabled: boolean;
  mode: SpeakerRgbMode;
  color_hex: string;
  brightness: number; // 0..100
  speed: number; // 0..100
  tws_pair_enabled: boolean;
}

export interface FocusWhiteNoiseState {
  hardware_playing: boolean;
  hardware_track_index: number;
  hardware_volume: number; // 0..100
  sync_progress: number | null; // 0..100 when flashing .bin tone pack
}

export type OtaPhase =
  | "idle"
  | "validating"
  | "transferring"
  | "verifying"
  | "rebooting"
  | "completed"
  | "unsupported_auth_required"
  | "error";

export interface OtaState {
  phase: OtaPhase;
  ota_type: number;
  current_firmware: string;
  file_path: string | null;
  progress_pct: number;
  message: string | null;
}

export type FitTestEarStatus = "untested" | "good" | "fair" | "failed";

export interface FitTestResult {
  left: FitTestEarStatus;
  right: FitTestEarStatus;
  tested_at_ms: number;
}

export interface SimulatorControlState {
  enabled: boolean;
  model_id: string;
  lid_open: boolean;
  latency_ms: number;
  battery: BatteryTelemetry;
}

export interface AppSettings {
  locale: SupportedLocale;
  theme: AppTheme;
  minimize_to_tray: boolean;
  show_connection_popup: boolean;
  popup_duration_sec: number;
  auto_start: boolean;
  simulator_enabled: boolean;
  developer_packet_log: boolean;
}

export interface ProtocolLogEntry {
  timestamp_ms: number;
  direction: "tx" | "rx";
  transport: string;
  uuid: string;
  hex: string;
  summary: string;
}

export interface PopupTriggerPayload {
  device_name: string;
  model_id: string;
  form_factor: DeviceFormFactor;
  color_index: number;
  battery: BatteryTelemetry;
  anc_mode: AncPrimaryMode;
  is_virtual: boolean;
  auto_dismiss_ms: number;
}

export interface AppSnapshot {
  connection_state: ConnectionState;
  is_virtual_session: boolean;
  adapter: AdapterInfo;
  active_device: DiscoveredDevice | null;
  active_model: DeviceModelProfile;
  firmware_version: string;
  device_color_index: number;
  saved_devices: SavedDeviceRecord[];
  battery: BatteryTelemetry;
  anc: AncState;
  eq: EqState;
  gestures: GestureState;
  modes: ModesState;
  multipoint: MultipointState;
  rgb_lighting: RgbLightingState;
  focus_white_noise: FocusWhiteNoiseState;
  ota: OtaState;
  last_fit_test: FitTestResult | null;
  simulator: SimulatorControlState;
  settings: AppSettings;
}

export interface CommandOutcome {
  applied: boolean;
  snapshot: AppSnapshot;
  conflict_prompt?: MutualExclusionPrompt | null;
}

/** Tauri IPC event names emitted by `src-tauri`. */
export const IPC_EVENTS = {
  STATE_UPDATED: "qcy://state-updated",
  POPUP_TRIGGERED: "qcy://popup-triggered",
  SCAN_DISCOVERED: "qcy://scan-discovered",
  PROTOCOL_LOG: "qcy://protocol-log",
} as const;

/** Tauri IPC command names implemented by `src-tauri`. */
export const IPC_COMMANDS = {
  GET_APP_SNAPSHOT: "get_app_snapshot",
  LIST_MODELS: "list_models",
  GET_ADAPTER_INFO: "get_adapter_info",
  START_SCAN: "start_scan",
  STOP_SCAN: "stop_scan",
  LIST_PAIRED_DEVICES: "list_paired_devices",
  CONNECT_DEVICE: "connect_device",
  DISCONNECT_DEVICE: "disconnect_device",
  REMOVE_SAVED_DEVICE: "remove_saved_device",
  OPEN_WINDOWS_BT_SETTINGS: "open_windows_bt_settings",

  SET_ANC_STATE: "set_anc_state",
  SET_AI_ANC: "set_ai_anc",
  SET_ANC_BUTTON_CYCLE: "set_anc_button_cycle",

  APPLY_EQ_PRESET: "apply_eq_preset",
  SET_GRAPHIC_EQ: "set_graphic_eq",
  SET_PARAMETRIC_EQ: "set_parametric_eq",
  SAVE_CUSTOM_EQ: "save_custom_eq",
  DELETE_CUSTOM_EQ: "delete_custom_eq",
  IMPORT_EQ_JSON: "import_eq_json",
  EXPORT_EQ_JSON: "export_eq_json",
  APPLY_PERSONALIZED_EQ: "apply_personalized_eq",
  SET_AUDIO_ENHANCERS: "set_audio_enhancers",

  SET_BUTTON_ACTION: "set_button_action",
  RESET_BUTTONS: "reset_buttons",
  SET_TOUCH_CONFIG: "set_touch_config",
  SET_HEAD_CONTROLS: "set_head_controls",

  SET_MODES: "set_modes",
  SET_MULTIPOINT: "set_multipoint",
  SET_RGB_LIGHTING: "set_rgb_lighting",
  SET_FOCUS_WHITE_NOISE: "set_focus_white_noise",
  RUN_FIT_TEST: "run_fit_test",
  SET_DEVICE_COLOR: "set_device_color",
  RENAME_DEVICE: "rename_device",
  RESET_DEVICE: "reset_device",
  START_LOCAL_OTA: "start_local_ota",
  CANCEL_OTA: "cancel_ota",

  SET_SIMULATOR_ENABLED: "set_simulator_enabled",
  SET_SIMULATOR_MODEL: "set_simulator_model",
  UPDATE_SIMULATOR_STATE: "update_simulator_state",
  TRIGGER_SIMULATOR_POPUP: "trigger_simulator_popup",

  UPDATE_APP_SETTINGS: "update_app_settings",
  SHOW_POPUP: "show_popup",
  HIDE_POPUP: "hide_popup",
  GET_PROTOCOL_LOGS: "get_protocol_logs",
  CLEAR_PROTOCOL_LOGS: "clear_protocol_logs",
} as const;
