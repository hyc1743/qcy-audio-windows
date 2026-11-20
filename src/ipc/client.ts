/**
 * Typed Tauri v2 IPC Client with full In-Memory Interactive Fallback.
 *
 * - Inside Tauri WebView2: invokes Rust commands (`src-tauri`) via `@tauri-apps/api/core`
 *   and subscribes to `qcy://state-updated`, `qcy://popup-triggered`, `qcy://scan-discovered`,
 *   and `qcy://protocol-log` via `@tauri-apps/api/event`.
 * - Outside Tauri (browser `vite dev` preview / Vitest): transparently runs a full in-memory
 *   QCY simulator backend supporting multi-model switching, `popViewRules` conflict dialogs,
 *   EQ JSON import/export, live event emission, and popup preview triggers.
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  IPC_COMMANDS,
  IPC_EVENTS,
  type AdapterInfo,
  type AiAncMatrix,
  type AncPrimaryMode,
  type AncScene,
  type AppSettings,
  type AppSnapshot,
  type BatteryTelemetry,
  type CommandOutcome,
  type DeviceCapabilities,
  type DeviceModelProfile,
  type DiscoveredDevice,
  type EqPresetItem,
  type FitTestResult,
  type ListeningMode,
  type MutualExclusionPrompt,
  type OtaState,
  type PopupTriggerPayload,
  type PromptLanguage,
  type ProtocolLogEntry,
  type SpatialAudioMode,
  type WireParametricBand,
} from "../types/qcy";
import type { SpeakerRgbMode } from "../assets/DeviceIllustrations";

export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export interface SetAncPayload {
  mode: AncPrimaryMode;
  scene?: AncScene;
  scene_level?: number;
  custom_level?: number;
  transparency_level?: number;
  vocal_boost?: boolean;
  wind_noise?: boolean;
  force?: boolean;
}

export interface SetAudioEnhancersPayload {
  dynamic_eq?: boolean;
  thx_eq?: boolean;
  virtual_bass?: boolean;
  adaptive_volume?: boolean;
  channel_balance?: number;
  force?: boolean;
}

export interface SetModesPayload {
  listening_mode?: ListeningMode;
  game_mode?: boolean;
  game_mode_pro?: boolean;
  game_sync_eq?: boolean;
  spatial_audio?: SpatialAudioMode;
  ldac?: boolean;
  le_audio?: boolean;
  in_ear_mode?: number;
  in_ear_auto_anc?: boolean;
  in_ear_sensitivity?: number;
  hearing_low_vol_remind?: boolean;
  hearing_duration_remind?: boolean;
  hearing_duration_min?: number;
  privacy_mode?: boolean;
  privacy_scope?: number;
  dialogue_perception?: boolean;
  speak_to_chat_sec?: number;
  auto_power_off_min?: number;
  disconnect_power_off_min?: number;
  power_saving?: boolean;
  disconnect_warning?: boolean;
  prompt_language?: PromptLanguage;
  prompt_volume?: number;
  force?: boolean;
}

export interface SetMultipointPayload {
  dual_device?: boolean;
  multi_device?: boolean;
  connect_mac?: string;
  disconnect_mac?: string;
  enter_pairing?: boolean;
  force?: boolean;
}

export interface SetRgbLightingPayload {
  enabled?: boolean;
  mode?: SpeakerRgbMode;
  color_hex?: string;
  brightness?: number;
  speed?: number;
  tws_pair?: boolean;
}

export interface SimulatorPatch {
  lid_open?: boolean;
  latency_ms?: number;
  battery?: Partial<BatteryTelemetry>;
}

function baseCapabilities(overrides: Partial<DeviceCapabilities> = {}): DeviceCapabilities {
  return {
    anc: true,
    ai_anc: false,
    anc_custom_levels: 0,
    transparency_levels: 6,
    vocal_boost: true,
    wind_noise: true,
    anc_button_cycle: true,
    anc_scenes: ["adaptive", "indoor", "commute", "outdoor", "anti_wind"],
    parametric_eq: true,
    custom_eq: true,
    eq_bands: 10,
    max_eq_bands: 10,
    supported_filter_types: [0, 1, 2, 3, 4, 5, 6, 7, 8],
    personalized_eq: true,
    dynamic_eq: true,
    thx_eq: false,
    virtual_bass: false,
    adaptive_volume: false,
    channel_balance: true,
    spatial_audio_modes: ["off", "fixed", "head_tracking"],
    listening_modes: [],
    game_mode: true,
    game_mode_pro: true,
    ldac: true,
    le_audio: false,
    button_remap: true,
    supported_button_actions: [0, 1, 2, 3, 4, 5, 6, 7, 11],
    button_keys: ["left", "right"],
    touch_enable: true,
    touch_sensitivity: false,
    head_controls: false,
    in_ear_detection: true,
    fit_test: true,
    hearing_health: false,
    privacy_mode_pro: false,
    dialogue_perception: false,
    speak_to_chat: false,
    dual_device: true,
    multi_device: true,
    rgb_lighting: false,
    tws_speaker_pair: false,
    focus_mode_white_noise: false,
    find_earbuds: true,
    prompt_language: true,
    prompt_volume: true,
    auto_power_off: true,
    disconnect_power_off: true,
    power_saving: false,
    disconnect_warning: false,
    device_color: true,
    rename: true,
    reset: true,
    factory_reset: true,
    clear_pairing: true,
    ota: true,
    has_case_battery: true,
    ...overrides,
  };
}

function createFallbackModels(): DeviceModelProfile[] {
  const ldacConflictRules = [
    {
      target_feature: "ldac",
      target_cmd_id: 35,
      target_value: 1,
      action: "confirm_dialog" as const,
      requires_reboot: true,
      title_key: "rules.conflict_title",
      message_key: "rules.conflict_disable_features",
      reboot_notice_key: "rules.conflict_will_reboot",
      conflicts_with: [
        { feature: "dual_device", cmd_id: 36, compare: "eq" as const, value: 1, reboot: true },
        { feature: "spatial_audio", cmd_id: 91, compare: "neq" as const, value: 0, reboot: true },
      ],
    },
    {
      target_feature: "dual_device",
      target_cmd_id: 36,
      target_value: 1,
      action: "confirm_dialog" as const,
      requires_reboot: true,
      title_key: "rules.conflict_title",
      message_key: "rules.conflict_disable_features",
      reboot_notice_key: "rules.conflict_will_reboot",
      conflicts_with: [
        { feature: "ldac", cmd_id: 35, compare: "eq" as const, value: 1, reboot: true },
      ],
    },
  ];

  return [
    {
      model_id: "qcy-n70",
      numeric_model_id: 23872,
      numeric_model_id_hex: "0x5D40",
      source: "bundled_profile",
      codename: "N70",
      name_en: "QCY MeloBuds N70",
      name_zh: "QCY MeloBuds N70",
      form_factor: "tws",
      chipset_family: "wuqi",
      protocol_family: "qcy_unified_tlv",
      ota_type: 11,
      ble_name_hints: ["QCY MeloBuds N70", "QCY-N70"],
      capabilities: baseCapabilities({ head_controls: true, ai_anc: true }),
      default_eq_preset_ids: ["default", "pop", "heavy_bass", "rock", "soft", "classical"],
      pop_view_rules: ldacConflictRules,
    },
    {
      model_id: "qcy-ht08",
      numeric_model_id: 19786,
      numeric_model_id_hex: "0x4D4A",
      source: "bundled_profile",
      codename: "HT08",
      name_en: "QCY MeloBuds Pro (HT08)",
      name_zh: "QCY MeloBuds Pro (HT08)",
      form_factor: "tws",
      chipset_family: "wuqi",
      protocol_family: "qcy_unified_tlv",
      ota_type: 11,
      ble_name_hints: ["QCY MeloBuds Pro", "QCY-HT08"],
      capabilities: baseCapabilities({
        parametric_eq: false,
        personalized_eq: false,
        ai_anc: false,
        multi_device: false,
      }),
      default_eq_preset_ids: ["default", "pop", "heavy_bass", "rock", "soft", "classical"],
      pop_view_rules: ldacConflictRules,
    },
    {
      model_id: "qcy-h6",
      numeric_model_id: 33136,
      numeric_model_id_hex: "0x8170",
      source: "bundled_profile",
      codename: "H6",
      name_en: "QCY H6 Over-Ear ANC",
      name_zh: "QCY H6 头戴降噪耳机",
      form_factor: "over_ear",
      chipset_family: "jieli",
      protocol_family: "jieli_rcsp_new",
      ota_type: 9,
      ble_name_hints: ["QCY H6", "QCY-H6"],
      capabilities: baseCapabilities({
        anc_custom_levels: 16,
        hearing_health: true,
        listening_modes: ["music", "game", "movie"],
        button_keys: ["vol_up", "vol_down", "mfb"],
        supported_button_actions: [0, 1, 2, 3, 4, 5, 6, 11, 14, 15, 16, 17, 18],
        has_case_battery: false,
        in_ear_detection: false,
        fit_test: false,
      }),
      default_eq_preset_ids: ["default", "pop", "bass_boost", "rock", "soft", "classical"],
      pop_view_rules: ldacConflictRules,
    },
    {
      model_id: "qcy-ct02",
      numeric_model_id: 19791,
      numeric_model_id_hex: "0x4D4F",
      source: "bundled_profile",
      codename: "CT02",
      name_en: "QCY Crossky Open-Ear (CT02)",
      name_zh: "QCY Crossky 开放式耳机 (CT02)",
      form_factor: "open_ear",
      chipset_family: "wuqi",
      protocol_family: "qcy_unified_tlv",
      ota_type: 11,
      ble_name_hints: ["QCY Crossky", "QCY-CT02"],
      capabilities: baseCapabilities({
        anc: false,
        anc_scenes: [],
        transparency_levels: 0,
        vocal_boost: false,
        wind_noise: false,
        anc_button_cycle: false,
        focus_mode_white_noise: true,
        adaptive_volume: true,
        privacy_mode_pro: true,
      }),
      default_eq_preset_ids: ["default", "pop", "heavy_bass", "pure_vocal"],
      pop_view_rules: ldacConflictRules,
    },
    {
      model_id: "qcy-sp75",
      numeric_model_id: 776,
      numeric_model_id_hex: "0x0308",
      source: "bundled_profile",
      codename: "SP75",
      name_en: "QCY SP75 RGB Speaker",
      name_zh: "QCY SP75 蓝牙音箱",
      form_factor: "speaker",
      chipset_family: "wuqi",
      protocol_family: "qcy_unified_tlv",
      ota_type: 11,
      ble_name_hints: ["QCY SP75", "QCY-SP75"],
      capabilities: baseCapabilities({
        anc: false,
        anc_scenes: [],
        transparency_levels: 0,
        vocal_boost: false,
        wind_noise: false,
        anc_button_cycle: false,
        parametric_eq: false,
        custom_eq: false,
        personalized_eq: false,
        dynamic_eq: false,
        channel_balance: false,
        spatial_audio_modes: [],
        game_mode: false,
        game_mode_pro: false,
        ldac: false,
        button_remap: false,
        button_keys: [],
        touch_enable: false,
        in_ear_detection: false,
        fit_test: false,
        dual_device: false,
        multi_device: false,
        rgb_lighting: true,
        tws_speaker_pair: true,
        find_earbuds: false,
        has_case_battery: false,
      }),
      default_eq_preset_ids: ["speaker_normal", "speaker_outdoor", "speaker_ambient"],
      pop_view_rules: [],
    },
    {
      model_id: "qcy-generic",
      numeric_model_id: null,
      numeric_model_id_hex: null,
      source: "generic_fallback",
      codename: "GENERIC",
      name_en: "QCY Audio Device (Generic)",
      name_zh: "QCY 音频设备（通用）",
      form_factor: "tws",
      chipset_family: "generic",
      protocol_family: "generic_placeholder",
      ota_type: 0,
      ble_name_hints: ["QCY"],
      capabilities: baseCapabilities({
        parametric_eq: false,
        personalized_eq: false,
        dynamic_eq: false,
        spatial_audio_modes: [],
        ldac: false,
        dual_device: false,
        multi_device: false,
        in_ear_detection: false,
        fit_test: false,
      }),
      default_eq_preset_ids: ["default", "pop", "heavy_bass", "rock", "soft", "classical"],
      pop_view_rules: [],
    },
  ];
}

function defaultAiMatrix(): AiAncMatrix {
  return {
    stay: { low: "off", medium: "anc", high: "anc" },
    walk: { low: "transparency", medium: "anc", high: "anc" },
    run: { low: "transparency", medium: "transparency", high: "anc" },
  };
}

class InMemoryQcyBackend {
  private models: DeviceModelProfile[] = createFallbackModels();
  private snapshot: AppSnapshot;
  private logs: ProtocolLogEntry[] = [];
  private stateListeners = new Set<(snap: AppSnapshot) => void>();
  private popupListeners = new Set<(payload: PopupTriggerPayload) => void>();
  private scanListeners = new Set<(device: DiscoveredDevice) => void>();
  private logListeners = new Set<(entry: ProtocolLogEntry) => void>();

  constructor() {
    const initialModel = this.models[0];
    const battery: BatteryTelemetry = {
      left: 88,
      right: 92,
      case_level: 76,
      left_charging: false,
      right_charging: false,
      case_charging: false,
      left_in_case: false,
      right_in_case: false,
      is_tws_connected: true,
    };

    const virtualDevice: DiscoveredDevice = {
      id: `virtual:${initialModel.model_id}`,
      address: "51:43:59:00:5D:40",
      name: initialModel.name_en,
      rssi: -46,
      transport_kind: "dual",
      manufacturer_data: {},
      service_uuids: ["0000a001-0000-1000-8000-00805f9b34fb"],
      is_paired: true,
      is_connected: true,
      last_seen_ms: Date.now(),
      model_id_hint: initialModel.model_id,
      numeric_model_id: initialModel.numeric_model_id,
      is_virtual: true,
    };

    this.snapshot = {
      connection_state: "connected",
      is_virtual_session: true,
      adapter: {
        is_present: true,
        address: "00:1A:7D:DA:71:11",
        is_classic_supported: true,
        is_low_energy_supported: true,
        is_peripheral_supported: false,
        error: null,
      },
      active_device: virtualDevice,
      active_model: initialModel,
      firmware_version: "2.4.1-sim",
      device_color_index: 1,
      saved_devices: [
        {
          address: virtualDevice.address,
          name: virtualDevice.name,
          model_id: initialModel.model_id,
          color_index: 1,
          last_connected_ms: Date.now(),
          last_battery: { ...battery },
          is_virtual: true,
        },
      ],
      battery,
      anc: {
        mode: "anc",
        scene: "adaptive",
        scene_level: 2,
        custom_level: 10,
        transparency_level: 4,
        vocal_boost: false,
        wind_noise: false,
        ai_anc_enabled: false,
        ai_activity: "stay",
        ai_noise_level: "medium",
        ai_matrix: defaultAiMatrix(),
        btn_anc_cfg: ["anc_adaptive", "transparency", "off"],
      },
      eq: {
        active_preset_id: "default",
        eq_mode: "graphic",
        master_gain_db: 0,
        gains_10band_db: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        parametric_bands: [],
        active_target_curve_id: null,
        personalized_enabled: false,
        personalized_left_db: [0, 0, 0, 0, 0, 0, 0, 0],
        personalized_right_db: [0, 0, 0, 0, 0, 0, 0, 0],
        custom_presets: [],
        dynamic_eq: false,
        thx_eq: false,
        virtual_bass: false,
        adaptive_volume: false,
        channel_balance: 0,
      },
      gestures: {
        touch_enable: true,
        touch_sensitivity: 45,
        mappings: [
          { key_id: 1, event_id: 1, cmd: 0 },
          { key_id: 1, event_id: 3, cmd: 1 },
          { key_id: 1, event_id: 5, cmd: 2 },
          { key_id: 1, event_id: 9, cmd: 11 },
          { key_id: 2, event_id: 2, cmd: 0 },
          { key_id: 2, event_id: 4, cmd: 1 },
          { key_id: 2, event_id: 6, cmd: 3 },
          { key_id: 2, event_id: 10, cmd: 11 },
        ],
        head_controls: {
          enabled: false,
          nod_cmd: 1,
          shake_cmd: 2,
          tilt_left_cmd: 3,
          tilt_right_cmd: 4,
        },
      },
      modes: {
        listening_mode: "music",
        game_mode: false,
        game_mode_pro: false,
        game_sync_eq: false,
        spatial_audio: "off",
        ldac: false,
        le_audio: false,
        in_ear_mode: 1,
        in_ear_auto_anc: true,
        in_ear_sensitivity: 2,
        hearing_low_vol_remind: false,
        hearing_duration_remind: false,
        hearing_duration_min: 60,
        privacy_mode: false,
        privacy_scope: 0,
        dialogue_perception: false,
        speak_to_chat_sec: 0,
        auto_power_off_min: 0,
        disconnect_power_off_min: 10,
        power_saving: false,
        disconnect_warning: false,
        prompt_language: "en",
        prompt_volume: 6,
      },
      multipoint: {
        dual_device: false,
        multi_device: false,
        hosts: [
          {
            mac: "00:1A:7D:DA:71:11",
            name: "Windows PC (This Device)",
            is_this_device: true,
            is_connected: true,
          },
        ],
      },
      rgb_lighting: {
        enabled: true,
        mode: "solid",
        color_hex: "#FF7A2F",
        brightness: 80,
        speed: 50,
        tws_pair_enabled: false,
      },
      focus_white_noise: {
        hardware_playing: false,
        hardware_track_index: 0,
        hardware_volume: 65,
        sync_progress: null,
      },
      ota: {
        phase: "idle",
        ota_type: initialModel.ota_type,
        current_firmware: "2.4.1-sim",
        file_path: null,
        progress_pct: 0,
        message: null,
      },
      last_fit_test: null,
      simulator: {
        enabled: true,
        model_id: initialModel.model_id,
        lid_open: true,
        latency_ms: 25,
        battery: { ...battery },
      },
      settings: {
        locale: "zh-CN",
        theme: "system",
        minimize_to_tray: true,
        show_connection_popup: true,
        popup_duration_sec: 5,
        auto_start: false,
        simulator_enabled: true,
        developer_packet_log: false,
      },
    };
  }

  private cloneSnapshot(): AppSnapshot {
    return JSON.parse(JSON.stringify(this.snapshot)) as AppSnapshot;
  }

  private emitState(): AppSnapshot {
    const snap = this.cloneSnapshot();
    for (const cb of this.stateListeners) cb(snap);
    return snap;
  }

  private pushLog(direction: "tx" | "rx", summary: string, hex = "FF 04 00 01 01"): void {
    const entry: ProtocolLogEntry = {
      timestamp_ms: Date.now(),
      direction,
      transport: this.snapshot.is_virtual_session ? "virtual" : "ble_gatt",
      uuid: "00001001-0000-1000-8000-00805f9b34fb",
      hex,
      summary,
    };
    this.logs.push(entry);
    if (this.logs.length > 200) this.logs.shift();
    for (const cb of this.logListeners) cb(entry);
  }

  private evaluateConflicts(targetFeature: string): MutualExclusionPrompt | null {
    const rule = this.snapshot.active_model.pop_view_rules.find(
      (r) => r.target_feature === targetFeature
    );
    if (!rule) return null;

    const conflicting: string[] = [];
    for (const cond of rule.conflicts_with) {
      if (cond.feature === "ldac" && this.snapshot.modes.ldac) {
        conflicting.push("ldac");
      } else if (cond.feature === "dual_device" && this.snapshot.multipoint.dual_device) {
        conflicting.push("dual_device");
      } else if (
        cond.feature === "spatial_audio" &&
        this.snapshot.modes.spatial_audio !== "off"
      ) {
        conflicting.push("spatial_audio");
      }
    }

    if (conflicting.length === 0) return null;
    return {
      rule,
      conflicting_features: conflicting,
      requires_reboot: rule.requires_reboot,
      title_key: rule.title_key,
      message_key: rule.message_key,
      reboot_notice_key: rule.reboot_notice_key,
    };
  }

  getSnapshot(): AppSnapshot {
    return this.cloneSnapshot();
  }

  listModels(): DeviceModelProfile[] {
    return JSON.parse(JSON.stringify(this.models)) as DeviceModelProfile[];
  }

  getAdapterInfo(): AdapterInfo {
    return { ...this.snapshot.adapter };
  }

  startScan(): void {
    if (this.snapshot.active_device) {
      const dev = { ...this.snapshot.active_device, last_seen_ms: Date.now() };
      for (const cb of this.scanListeners) cb(dev);
    }
  }

  listPairedDevices(): DiscoveredDevice[] {
    return this.snapshot.active_device ? [{ ...this.snapshot.active_device }] : [];
  }

  connectDevice(address: string, modelId?: string): AppSnapshot {
    if (modelId) {
      this.setSimulatorModel(modelId);
    }
    this.snapshot.connection_state = "connected";
    if (this.snapshot.active_device) {
      this.snapshot.active_device.address = address;
      this.snapshot.active_device.is_connected = true;
    }
    this.pushLog("tx", `connect(${address})`);
    return this.emitState();
  }

  disconnectDevice(): AppSnapshot {
    this.snapshot.connection_state = "disconnected";
    if (this.snapshot.active_device) {
      this.snapshot.active_device.is_connected = false;
    }
    this.pushLog("tx", "disconnect()");
    return this.emitState();
  }

  removeSavedDevice(address: string): AppSnapshot {
    this.snapshot.saved_devices = this.snapshot.saved_devices.filter((d) => d.address !== address);
    return this.emitState();
  }

  setAncState(payload: SetAncPayload): CommandOutcome {
    this.snapshot.anc.mode = payload.mode;
    if (payload.scene !== undefined) this.snapshot.anc.scene = payload.scene;
    if (payload.scene_level !== undefined) this.snapshot.anc.scene_level = payload.scene_level;
    if (payload.custom_level !== undefined) {
      this.snapshot.anc.custom_level = Math.max(0, Math.min(15, payload.custom_level));
    }
    if (payload.transparency_level !== undefined) {
      this.snapshot.anc.transparency_level = Math.max(1, Math.min(6, payload.transparency_level));
    }
    if (payload.vocal_boost !== undefined) this.snapshot.anc.vocal_boost = payload.vocal_boost;
    if (payload.wind_noise !== undefined) this.snapshot.anc.wind_noise = payload.wind_noise;
    this.pushLog("tx", `set_anc_state(${payload.mode})`, "FF 04 17 02 01 05");
    return { applied: true, snapshot: this.emitState() };
  }

  setAiAnc(enabled: boolean, matrix?: AiAncMatrix): CommandOutcome {
    this.snapshot.anc.ai_anc_enabled = enabled;
    if (matrix) this.snapshot.anc.ai_matrix = matrix;
    this.pushLog("tx", `set_ai_anc(${enabled})`, "FF 03 54 01 01");
    return { applied: true, snapshot: this.emitState() };
  }

  setAncButtonCycle(modes: string[]): CommandOutcome {
    if (modes.length >= 2) {
      this.snapshot.anc.btn_anc_cfg = [...modes];
    }
    this.pushLog("tx", `set_anc_button_cycle(${modes.join(",")})`, "FF 04 65 02 01 03");
    return { applied: true, snapshot: this.emitState() };
  }

  applyEqPreset(presetId: string): CommandOutcome {
    this.snapshot.eq.active_preset_id = presetId;
    const custom = this.snapshot.eq.custom_presets.find((p) => p.id === presetId);
    if (custom) {
      this.snapshot.eq.gains_10band_db = [...custom.gains_10band_db];
      this.snapshot.eq.parametric_bands = [...custom.parametric_bands];
      this.snapshot.eq.master_gain_db = custom.master_gain_db;
    }
    this.pushLog("tx", `apply_eq_preset(${presetId})`, "FF 03 22 01 01");
    return { applied: true, snapshot: this.emitState() };
  }

  setGraphicEq(gainsDb: number[], masterGainDb?: number): CommandOutcome {
    this.snapshot.eq.eq_mode = "graphic";
    this.snapshot.eq.active_preset_id = "custom";
    this.snapshot.eq.gains_10band_db = gainsDb.map((g) => Math.max(-12, Math.min(12, g)));
    if (masterGainDb !== undefined) this.snapshot.eq.master_gain_db = masterGainDb;
    this.pushLog("tx", "set_graphic_eq(10-band)", "FF 0C 20 0A 00 00 00 00 00 00 00 00 00 00");
    return { applied: true, snapshot: this.emitState() };
  }

  setParametricEq(bands: WireParametricBand[], masterGainDb?: number): CommandOutcome {
    this.snapshot.eq.eq_mode = "parametric";
    this.snapshot.eq.active_preset_id = "custom";
    this.snapshot.eq.parametric_bands = bands.map((b) => ({ ...b }));
    if (masterGainDb !== undefined) this.snapshot.eq.master_gain_db = masterGainDb;
    this.pushLog("tx", `set_parametric_eq(${bands.length} bands)`, "FF 0A 22 08 ...");
    return { applied: true, snapshot: this.emitState() };
  }

  saveCustomEq(preset: EqPresetItem): AppSnapshot {
    const normalized: EqPresetItem = { ...preset, category: "custom" };
    const idx = this.snapshot.eq.custom_presets.findIndex((p) => p.id === normalized.id);
    if (idx >= 0) {
      this.snapshot.eq.custom_presets[idx] = normalized;
    } else {
      this.snapshot.eq.custom_presets.push(normalized);
    }
    this.snapshot.eq.active_preset_id = normalized.id;
    return this.emitState();
  }

  deleteCustomEq(presetId: string): AppSnapshot {
    this.snapshot.eq.custom_presets = this.snapshot.eq.custom_presets.filter(
      (p) => p.id !== presetId
    );
    if (this.snapshot.eq.active_preset_id === presetId) {
      this.snapshot.eq.active_preset_id = "default";
    }
    return this.emitState();
  }

  importEqJson(json: string): AppSnapshot {
    const parsed = JSON.parse(json) as EqPresetItem | EqPresetItem[];
    const items = Array.isArray(parsed) ? parsed : [parsed];
    for (const item of items) {
      if (item && typeof item.id === "string" && Array.isArray(item.gains_10band_db)) {
        this.saveCustomEq(item);
      }
    }
    return this.emitState();
  }

  exportEqJson(presetId?: string): string {
    if (presetId) {
      const found = this.snapshot.eq.custom_presets.find((p) => p.id === presetId);
      if (found) return JSON.stringify(found, null, 2);
    }
    return JSON.stringify(
      {
        id: this.snapshot.eq.active_preset_id,
        eq_index: 99,
        category: "custom",
        name_en: "Exported EQ",
        name_zh: "导出音效",
        master_gain_db: this.snapshot.eq.master_gain_db,
        gains_10band_db: this.snapshot.eq.gains_10band_db,
        parametric_bands: this.snapshot.eq.parametric_bands,
      } satisfies EqPresetItem,
      null,
      2
    );
  }

  applyPersonalizedEq(
    leftDb: number[],
    rightDb: number[],
    combinedDb: number[],
    balance?: number
  ): CommandOutcome {
    this.snapshot.eq.personalized_enabled = true;
    this.snapshot.eq.personalized_left_db = [...leftDb];
    this.snapshot.eq.personalized_right_db = [...rightDb];
    if (combinedDb.length === 10) {
      this.snapshot.eq.gains_10band_db = [...combinedDb];
    }
    if (balance !== undefined) {
      this.snapshot.eq.channel_balance = Math.max(-1, Math.min(1, balance));
    }
    this.snapshot.eq.active_preset_id = "personalized";
    this.pushLog("tx", "apply_personalized_eq(0x46/0x47)", "FF 0A 46 08 ...");
    return { applied: true, snapshot: this.emitState() };
  }

  setAudioEnhancers(payload: SetAudioEnhancersPayload): CommandOutcome {
    if (payload.dynamic_eq !== undefined) this.snapshot.eq.dynamic_eq = payload.dynamic_eq;
    if (payload.thx_eq !== undefined) this.snapshot.eq.thx_eq = payload.thx_eq;
    if (payload.virtual_bass !== undefined) this.snapshot.eq.virtual_bass = payload.virtual_bass;
    if (payload.adaptive_volume !== undefined) {
      this.snapshot.eq.adaptive_volume = payload.adaptive_volume;
    }
    if (payload.channel_balance !== undefined) {
      this.snapshot.eq.channel_balance = Math.max(-1, Math.min(1, payload.channel_balance));
    }
    this.pushLog("tx", "set_audio_enhancers()");
    return { applied: true, snapshot: this.emitState() };
  }

  setButtonAction(keyId: number, eventId: number, cmd: number): CommandOutcome {
    const idx = this.snapshot.gestures.mappings.findIndex(
      (m) => m.key_id === keyId && m.event_id === eventId
    );
    if (idx >= 0) {
      this.snapshot.gestures.mappings[idx].cmd = cmd;
    } else {
      this.snapshot.gestures.mappings.push({ key_id: keyId, event_id: eventId, cmd });
    }
    this.pushLog("tx", `set_button_action(k=${keyId},e=${eventId},cmd=${cmd})`, "FF 05 2B 03 ...");
    return { applied: true, snapshot: this.emitState() };
  }

  resetButtons(): CommandOutcome {
    this.snapshot.gestures.mappings = [
      { key_id: 1, event_id: 1, cmd: 0 },
      { key_id: 1, event_id: 3, cmd: 1 },
      { key_id: 1, event_id: 5, cmd: 2 },
      { key_id: 1, event_id: 9, cmd: 11 },
      { key_id: 2, event_id: 2, cmd: 0 },
      { key_id: 2, event_id: 4, cmd: 1 },
      { key_id: 2, event_id: 6, cmd: 3 },
      { key_id: 2, event_id: 10, cmd: 11 },
    ];
    this.pushLog("tx", "reset_buttons()");
    return { applied: true, snapshot: this.emitState() };
  }

  setTouchConfig(touchEnable?: boolean, touchSensitivity?: number): CommandOutcome {
    if (touchEnable !== undefined) this.snapshot.gestures.touch_enable = touchEnable;
    if (touchSensitivity !== undefined) this.snapshot.gestures.touch_sensitivity = touchSensitivity;
    this.pushLog("tx", "set_touch_config()");
    return { applied: true, snapshot: this.emitState() };
  }

  setHeadControls(patch: {
    enabled?: boolean;
    nod_cmd?: number;
    shake_cmd?: number;
    tilt_left_cmd?: number;
    tilt_right_cmd?: number;
  }): CommandOutcome {
    Object.assign(this.snapshot.gestures.head_controls, patch);
    this.pushLog("tx", "set_head_controls(0x50)", "FF 05 50 03 ...");
    return { applied: true, snapshot: this.emitState() };
  }

  setModes(payload: SetModesPayload): CommandOutcome {
    if (payload.ldac === true && !payload.force) {
      const conflict = this.evaluateConflicts("ldac");
      if (conflict) {
        return { applied: false, snapshot: this.cloneSnapshot(), conflict_prompt: conflict };
      }
    }
    if (payload.ldac === true && payload.force) {
      this.snapshot.multipoint.dual_device = false;
      this.snapshot.modes.spatial_audio = "off";
    }

    const { force: _force, ...rest } = payload;
    Object.assign(this.snapshot.modes, rest);
    this.pushLog("tx", "set_modes()");
    return { applied: true, snapshot: this.emitState() };
  }

  setMultipoint(payload: SetMultipointPayload): CommandOutcome {
    if (payload.dual_device === true && !payload.force) {
      const conflict = this.evaluateConflicts("dual_device");
      if (conflict) {
        return { applied: false, snapshot: this.cloneSnapshot(), conflict_prompt: conflict };
      }
    }
    if (payload.dual_device === true && payload.force) {
      this.snapshot.modes.ldac = false;
    }
    if (payload.dual_device !== undefined) this.snapshot.multipoint.dual_device = payload.dual_device;
    if (payload.multi_device !== undefined) {
      this.snapshot.multipoint.multi_device = payload.multi_device;
    }
    if (payload.disconnect_mac) {
      this.snapshot.multipoint.hosts = this.snapshot.multipoint.hosts.map((h) =>
        h.mac === payload.disconnect_mac ? { ...h, is_connected: false } : h
      );
    }
    this.pushLog("tx", "set_multipoint()");
    return { applied: true, snapshot: this.emitState() };
  }

  setRgbLighting(payload: SetRgbLightingPayload): CommandOutcome {
    if (payload.enabled !== undefined) this.snapshot.rgb_lighting.enabled = payload.enabled;
    if (payload.mode !== undefined) this.snapshot.rgb_lighting.mode = payload.mode;
    if (payload.color_hex !== undefined) this.snapshot.rgb_lighting.color_hex = payload.color_hex;
    if (payload.brightness !== undefined) {
      this.snapshot.rgb_lighting.brightness = Math.max(0, Math.min(100, payload.brightness));
    }
    if (payload.speed !== undefined) {
      this.snapshot.rgb_lighting.speed = Math.max(0, Math.min(100, payload.speed));
    }
    if (payload.tws_pair !== undefined) {
      this.snapshot.rgb_lighting.tws_pair_enabled = payload.tws_pair;
    }
    this.pushLog("tx", "set_rgb_lighting(0x36)", "FF 06 36 04 ...");
    return { applied: true, snapshot: this.emitState() };
  }

  setFocusWhiteNoise(playing?: boolean, trackIndex?: number, volume?: number): CommandOutcome {
    if (playing !== undefined) this.snapshot.focus_white_noise.hardware_playing = playing;
    if (trackIndex !== undefined) this.snapshot.focus_white_noise.hardware_track_index = trackIndex;
    if (volume !== undefined) {
      this.snapshot.focus_white_noise.hardware_volume = Math.max(0, Math.min(100, volume));
    }
    this.pushLog("tx", "set_focus_white_noise(0x39)");
    return { applied: true, snapshot: this.emitState() };
  }

  runFitTest(): FitTestResult {
    const result: FitTestResult = {
      left: "good",
      right: "good",
      tested_at_ms: Date.now(),
    };
    this.snapshot.last_fit_test = result;
    this.emitState();
    return result;
  }

  setDeviceColor(colorIndex: number): AppSnapshot {
    this.snapshot.device_color_index = colorIndex;
    return this.emitState();
  }

  renameDevice(name: string): CommandOutcome {
    const clean = name.trim();
    if (clean && this.snapshot.active_device) {
      this.snapshot.active_device.name = clean;
    }
    this.pushLog("tx", `rename_device(${clean})`, "FF ... 18 ...");
    return { applied: true, snapshot: this.emitState() };
  }

  resetDevice(kind: "soft_reset" | "clear_pairing" | "factory_reset"): CommandOutcome {
    this.snapshot.anc.mode = "off";
    this.snapshot.eq.active_preset_id = "default";
    this.snapshot.eq.gains_10band_db = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    this.pushLog("tx", `reset_device(${kind})`);
    return { applied: true, snapshot: this.emitState() };
  }

  startLocalOta(filePathOrName: string, bytes?: number[]): OtaState {
    const otaType = this.snapshot.active_model.ota_type;
    if (bytes !== undefined && bytes.length === 0) {
      this.snapshot.ota = {
        phase: "error",
        ota_type: otaType,
        current_firmware: this.snapshot.firmware_version,
        file_path: filePathOrName,
        progress_pct: 0,
        message: "Firmware image is empty",
      };
      this.emitState();
      return { ...this.snapshot.ota };
    }
    this.pushLog(
      "tx",
      `start_local_ota(${filePathOrName}${bytes !== undefined ? `, ${bytes.length}B` : ""})`
    );
    if (otaType === 9) {
      this.snapshot.ota = {
        phase: "unsupported_auth_required",
        ota_type: 9,
        current_firmware: this.snapshot.firmware_version,
        file_path: filePathOrName,
        progress_pct: 0,
        message: "ota.jieli_v2_auth_unsupported",
      };
    } else {
      this.snapshot.ota = {
        phase: "completed",
        ota_type: otaType,
        current_firmware: this.snapshot.firmware_version,
        file_path: filePathOrName,
        progress_pct: 100,
        message: null,
      };
    }
    this.emitState();
    return { ...this.snapshot.ota };
  }

  cancelOta(): OtaState {
    this.snapshot.ota.phase = "idle";
    this.snapshot.ota.progress_pct = 0;
    this.emitState();
    return { ...this.snapshot.ota };
  }

  setSimulatorEnabled(enabled: boolean): AppSnapshot {
    this.snapshot.simulator.enabled = enabled;
    this.snapshot.settings.simulator_enabled = enabled;
    this.snapshot.is_virtual_session = enabled;
    this.snapshot.connection_state = enabled ? "connected" : "disconnected";
    return this.emitState();
  }

  setSimulatorModel(modelId: string): AppSnapshot {
    const found = this.models.find((m) => m.model_id === modelId) ?? this.models[0];
    this.snapshot.active_model = found;
    this.snapshot.simulator.model_id = found.model_id;
    this.snapshot.ota.ota_type = found.ota_type;
    if (this.snapshot.active_device) {
      this.snapshot.active_device.id = `virtual:${found.model_id}`;
      this.snapshot.active_device.name = found.name_en;
      this.snapshot.active_device.model_id_hint = found.model_id;
      this.snapshot.active_device.numeric_model_id = found.numeric_model_id;
    }
    if (!found.capabilities.has_case_battery) {
      this.snapshot.battery.case_level = null;
      this.snapshot.simulator.battery.case_level = null;
    } else if (this.snapshot.battery.case_level === null) {
      this.snapshot.battery.case_level = 76;
      this.snapshot.simulator.battery.case_level = 76;
    }
    return this.emitState();
  }

  updateSimulatorState(patch: SimulatorPatch): AppSnapshot {
    if (patch.lid_open !== undefined) this.snapshot.simulator.lid_open = patch.lid_open;
    if (patch.latency_ms !== undefined) this.snapshot.simulator.latency_ms = patch.latency_ms;
    if (patch.battery) {
      Object.assign(this.snapshot.simulator.battery, patch.battery);
      Object.assign(this.snapshot.battery, patch.battery);
    }
    return this.emitState();
  }

  triggerSimulatorPopup(): PopupTriggerPayload {
    const payload: PopupTriggerPayload = {
      device_name: this.snapshot.active_device?.name ?? this.snapshot.active_model.name_en,
      model_id: this.snapshot.active_model.model_id,
      form_factor: this.snapshot.active_model.form_factor,
      color_index: this.snapshot.device_color_index,
      battery: { ...this.snapshot.battery },
      anc_mode: this.snapshot.anc.mode,
      is_virtual: this.snapshot.is_virtual_session,
      auto_dismiss_ms: this.snapshot.settings.popup_duration_sec * 1000,
    };
    for (const cb of this.popupListeners) cb(payload);
    return payload;
  }

  updateAppSettings(patch: Partial<AppSettings>): AppSnapshot {
    Object.assign(this.snapshot.settings, patch);
    return this.emitState();
  }

  getProtocolLogs(): ProtocolLogEntry[] {
    return [...this.logs];
  }

  clearProtocolLogs(): void {
    this.logs = [];
  }

  onStateUpdated(cb: (snap: AppSnapshot) => void): UnlistenFn {
    this.stateListeners.add(cb);
    return () => {
      this.stateListeners.delete(cb);
    };
  }

  onPopupTriggered(cb: (payload: PopupTriggerPayload) => void): UnlistenFn {
    this.popupListeners.add(cb);
    return () => {
      this.popupListeners.delete(cb);
    };
  }

  onScanDiscovered(cb: (device: DiscoveredDevice) => void): UnlistenFn {
    this.scanListeners.add(cb);
    return () => {
      this.scanListeners.delete(cb);
    };
  }

  onProtocolLog(cb: (entry: ProtocolLogEntry) => void): UnlistenFn {
    this.logListeners.add(cb);
    return () => {
      this.logListeners.delete(cb);
    };
  }
}

/**
 * Unified QCY Desktop IPC client supporting both live Tauri v2 runtime and in-memory fallback.
 */
export class QcyIpcClient {
  private readonly forceMock: boolean;
  private readonly mock = new InMemoryQcyBackend();

  constructor(options?: { forceMock?: boolean }) {
    this.forceMock = options?.forceMock ?? false;
  }

  get isTauri(): boolean {
    return !this.forceMock && isTauriRuntime();
  }

  async getAppSnapshot(): Promise<AppSnapshot> {
    if (this.isTauri) return invoke<AppSnapshot>(IPC_COMMANDS.GET_APP_SNAPSHOT);
    return this.mock.getSnapshot();
  }

  async listModels(): Promise<DeviceModelProfile[]> {
    if (this.isTauri) return invoke<DeviceModelProfile[]>(IPC_COMMANDS.LIST_MODELS);
    return this.mock.listModels();
  }

  async getAdapterInfo(): Promise<AdapterInfo> {
    if (this.isTauri) return invoke<AdapterInfo>(IPC_COMMANDS.GET_ADAPTER_INFO);
    return this.mock.getAdapterInfo();
  }

  async startScan(): Promise<void> {
    if (this.isTauri) return invoke<void>(IPC_COMMANDS.START_SCAN);
    this.mock.startScan();
  }

  async stopScan(): Promise<void> {
    if (this.isTauri) return invoke<void>(IPC_COMMANDS.STOP_SCAN);
  }

  async listPairedDevices(): Promise<DiscoveredDevice[]> {
    if (this.isTauri) return invoke<DiscoveredDevice[]>(IPC_COMMANDS.LIST_PAIRED_DEVICES);
    return this.mock.listPairedDevices();
  }

  async connectDevice(address: string, modelId?: string): Promise<AppSnapshot> {
    if (this.isTauri) {
      return invoke<AppSnapshot>(IPC_COMMANDS.CONNECT_DEVICE, { address, model_id: modelId });
    }
    return this.mock.connectDevice(address, modelId);
  }

  async disconnectDevice(): Promise<AppSnapshot> {
    if (this.isTauri) return invoke<AppSnapshot>(IPC_COMMANDS.DISCONNECT_DEVICE);
    return this.mock.disconnectDevice();
  }

  async removeSavedDevice(address: string): Promise<AppSnapshot> {
    if (this.isTauri) return invoke<AppSnapshot>(IPC_COMMANDS.REMOVE_SAVED_DEVICE, { address });
    return this.mock.removeSavedDevice(address);
  }

  async openWindowsBtSettings(): Promise<void> {
    if (this.isTauri) return invoke<void>(IPC_COMMANDS.OPEN_WINDOWS_BT_SETTINGS);
  }

  async setAncState(payload: SetAncPayload): Promise<CommandOutcome> {
    if (this.isTauri) return invoke<CommandOutcome>(IPC_COMMANDS.SET_ANC_STATE, { payload });
    return this.mock.setAncState(payload);
  }

  async setAiAnc(enabled: boolean, matrix?: AiAncMatrix): Promise<CommandOutcome> {
    if (this.isTauri) {
      return invoke<CommandOutcome>(IPC_COMMANDS.SET_AI_ANC, { enabled, matrix });
    }
    return this.mock.setAiAnc(enabled, matrix);
  }

  async setAncButtonCycle(modes: string[]): Promise<CommandOutcome> {
    if (this.isTauri) {
      return invoke<CommandOutcome>(IPC_COMMANDS.SET_ANC_BUTTON_CYCLE, { modes });
    }
    return this.mock.setAncButtonCycle(modes);
  }

  async applyEqPreset(presetId: string): Promise<CommandOutcome> {
    if (this.isTauri) {
      return invoke<CommandOutcome>(IPC_COMMANDS.APPLY_EQ_PRESET, { preset_id: presetId });
    }
    return this.mock.applyEqPreset(presetId);
  }

  async setGraphicEq(gainsDb: number[], masterGainDb?: number): Promise<CommandOutcome> {
    if (this.isTauri) {
      return invoke<CommandOutcome>(IPC_COMMANDS.SET_GRAPHIC_EQ, {
        gains_db: gainsDb,
        master_gain_db: masterGainDb,
      });
    }
    return this.mock.setGraphicEq(gainsDb, masterGainDb);
  }

  async setParametricEq(
    bands: WireParametricBand[],
    masterGainDb?: number
  ): Promise<CommandOutcome> {
    if (this.isTauri) {
      return invoke<CommandOutcome>(IPC_COMMANDS.SET_PARAMETRIC_EQ, {
        bands,
        master_gain_db: masterGainDb,
      });
    }
    return this.mock.setParametricEq(bands, masterGainDb);
  }

  async saveCustomEq(preset: EqPresetItem): Promise<AppSnapshot> {
    if (this.isTauri) return invoke<AppSnapshot>(IPC_COMMANDS.SAVE_CUSTOM_EQ, { preset });
    return this.mock.saveCustomEq(preset);
  }

  async deleteCustomEq(presetId: string): Promise<AppSnapshot> {
    if (this.isTauri) {
      return invoke<AppSnapshot>(IPC_COMMANDS.DELETE_CUSTOM_EQ, { preset_id: presetId });
    }
    return this.mock.deleteCustomEq(presetId);
  }

  async importEqJson(json: string): Promise<AppSnapshot> {
    if (this.isTauri) return invoke<AppSnapshot>(IPC_COMMANDS.IMPORT_EQ_JSON, { json });
    return this.mock.importEqJson(json);
  }

  async exportEqJson(presetId?: string): Promise<string> {
    if (this.isTauri) {
      return invoke<string>(IPC_COMMANDS.EXPORT_EQ_JSON, { preset_id: presetId });
    }
    return this.mock.exportEqJson(presetId);
  }

  async applyPersonalizedEq(
    leftGainsDb: number[],
    rightGainsDb: number[],
    combinedGainsDb: number[],
    balance?: number
  ): Promise<CommandOutcome> {
    if (this.isTauri) {
      return invoke<CommandOutcome>(IPC_COMMANDS.APPLY_PERSONALIZED_EQ, {
        left_gains_db: leftGainsDb,
        right_gains_db: rightGainsDb,
        combined_gains_db: combinedGainsDb,
        balance,
      });
    }
    return this.mock.applyPersonalizedEq(leftGainsDb, rightGainsDb, combinedGainsDb, balance);
  }

  async setAudioEnhancers(payload: SetAudioEnhancersPayload): Promise<CommandOutcome> {
    if (this.isTauri) {
      return invoke<CommandOutcome>(IPC_COMMANDS.SET_AUDIO_ENHANCERS, { payload });
    }
    return this.mock.setAudioEnhancers(payload);
  }

  async setButtonAction(keyId: number, eventId: number, cmd: number): Promise<CommandOutcome> {
    if (this.isTauri) {
      return invoke<CommandOutcome>(IPC_COMMANDS.SET_BUTTON_ACTION, {
        key_id: keyId,
        event_id: eventId,
        cmd,
      });
    }
    return this.mock.setButtonAction(keyId, eventId, cmd);
  }

  async resetButtons(): Promise<CommandOutcome> {
    if (this.isTauri) return invoke<CommandOutcome>(IPC_COMMANDS.RESET_BUTTONS);
    return this.mock.resetButtons();
  }

  async setTouchConfig(touchEnable?: boolean, touchSensitivity?: number): Promise<CommandOutcome> {
    if (this.isTauri) {
      return invoke<CommandOutcome>(IPC_COMMANDS.SET_TOUCH_CONFIG, {
        touch_enable: touchEnable,
        touch_sensitivity: touchSensitivity,
      });
    }
    return this.mock.setTouchConfig(touchEnable, touchSensitivity);
  }

  async setHeadControls(patch: {
    enabled?: boolean;
    nod_cmd?: number;
    shake_cmd?: number;
    tilt_left_cmd?: number;
    tilt_right_cmd?: number;
  }): Promise<CommandOutcome> {
    if (this.isTauri) return invoke<CommandOutcome>(IPC_COMMANDS.SET_HEAD_CONTROLS, { patch });
    return this.mock.setHeadControls(patch);
  }

  async setModes(payload: SetModesPayload): Promise<CommandOutcome> {
    if (this.isTauri) return invoke<CommandOutcome>(IPC_COMMANDS.SET_MODES, { payload });
    return this.mock.setModes(payload);
  }

  async setMultipoint(payload: SetMultipointPayload): Promise<CommandOutcome> {
    if (this.isTauri) return invoke<CommandOutcome>(IPC_COMMANDS.SET_MULTIPOINT, { payload });
    return this.mock.setMultipoint(payload);
  }

  async setRgbLighting(payload: SetRgbLightingPayload): Promise<CommandOutcome> {
    if (this.isTauri) return invoke<CommandOutcome>(IPC_COMMANDS.SET_RGB_LIGHTING, { payload });
    return this.mock.setRgbLighting(payload);
  }

  async setFocusWhiteNoise(
    playing?: boolean,
    trackIndex?: number,
    volume?: number
  ): Promise<CommandOutcome> {
    if (this.isTauri) {
      return invoke<CommandOutcome>(IPC_COMMANDS.SET_FOCUS_WHITE_NOISE, {
        playing,
        track_index: trackIndex,
        volume,
      });
    }
    return this.mock.setFocusWhiteNoise(playing, trackIndex, volume);
  }

  async runFitTest(): Promise<FitTestResult> {
    if (this.isTauri) return invoke<FitTestResult>(IPC_COMMANDS.RUN_FIT_TEST);
    return this.mock.runFitTest();
  }

  async setDeviceColor(colorIndex: number): Promise<AppSnapshot> {
    if (this.isTauri) {
      return invoke<AppSnapshot>(IPC_COMMANDS.SET_DEVICE_COLOR, { color_index: colorIndex });
    }
    return this.mock.setDeviceColor(colorIndex);
  }

  async renameDevice(name: string): Promise<CommandOutcome> {
    if (this.isTauri) return invoke<CommandOutcome>(IPC_COMMANDS.RENAME_DEVICE, { name });
    return this.mock.renameDevice(name);
  }

  async resetDevice(
    kind: "soft_reset" | "clear_pairing" | "factory_reset"
  ): Promise<CommandOutcome> {
    if (this.isTauri) return invoke<CommandOutcome>(IPC_COMMANDS.RESET_DEVICE, { kind });
    return this.mock.resetDevice(kind);
  }

  async startLocalOta(
    filePathOrName: string,
    bytes?: number[] | Uint8Array
  ): Promise<OtaState> {
    const rawBytes =
      bytes === undefined ? undefined : Array.isArray(bytes) ? bytes : Array.from(bytes);
    if (this.isTauri) {
      if (rawBytes !== undefined) {
        return invoke<OtaState>(IPC_COMMANDS.START_LOCAL_OTA, {
          file_name: filePathOrName,
          bytes: rawBytes,
        });
      }
      return invoke<OtaState>(IPC_COMMANDS.START_LOCAL_OTA, {
        file_path: filePathOrName,
      });
    }
    return this.mock.startLocalOta(filePathOrName, rawBytes);
  }

  async cancelOta(): Promise<OtaState> {
    if (this.isTauri) return invoke<OtaState>(IPC_COMMANDS.CANCEL_OTA);
    return this.mock.cancelOta();
  }

  async setSimulatorEnabled(enabled: boolean): Promise<AppSnapshot> {
    if (this.isTauri) {
      return invoke<AppSnapshot>(IPC_COMMANDS.SET_SIMULATOR_ENABLED, { enabled });
    }
    return this.mock.setSimulatorEnabled(enabled);
  }

  async setSimulatorModel(modelId: string): Promise<AppSnapshot> {
    if (this.isTauri) {
      return invoke<AppSnapshot>(IPC_COMMANDS.SET_SIMULATOR_MODEL, { model_id: modelId });
    }
    return this.mock.setSimulatorModel(modelId);
  }

  async updateSimulatorState(patch: SimulatorPatch): Promise<AppSnapshot> {
    if (this.isTauri) {
      return invoke<AppSnapshot>(IPC_COMMANDS.UPDATE_SIMULATOR_STATE, { patch });
    }
    return this.mock.updateSimulatorState(patch);
  }

  async triggerSimulatorPopup(): Promise<PopupTriggerPayload> {
    if (this.isTauri) {
      return invoke<PopupTriggerPayload>(IPC_COMMANDS.TRIGGER_SIMULATOR_POPUP);
    }
    return this.mock.triggerSimulatorPopup();
  }

  async updateAppSettings(patch: Partial<AppSettings>): Promise<AppSnapshot> {
    if (this.isTauri) {
      return invoke<AppSnapshot>(IPC_COMMANDS.UPDATE_APP_SETTINGS, { patch });
    }
    return this.mock.updateAppSettings(patch);
  }

  async showPopup(): Promise<void> {
    if (this.isTauri) return invoke<void>(IPC_COMMANDS.SHOW_POPUP);
    this.mock.triggerSimulatorPopup();
  }

  async hidePopup(): Promise<void> {
    if (this.isTauri) return invoke<void>(IPC_COMMANDS.HIDE_POPUP);
  }

  async getProtocolLogs(): Promise<ProtocolLogEntry[]> {
    if (this.isTauri) return invoke<ProtocolLogEntry[]>(IPC_COMMANDS.GET_PROTOCOL_LOGS);
    return this.mock.getProtocolLogs();
  }

  async clearProtocolLogs(): Promise<void> {
    if (this.isTauri) return invoke<void>(IPC_COMMANDS.CLEAR_PROTOCOL_LOGS);
    this.mock.clearProtocolLogs();
  }

  async onStateUpdated(cb: (snap: AppSnapshot) => void): Promise<UnlistenFn> {
    if (this.isTauri) {
      return listen<AppSnapshot>(IPC_EVENTS.STATE_UPDATED, (e) => cb(e.payload));
    }
    return this.mock.onStateUpdated(cb);
  }

  async onPopupTriggered(cb: (payload: PopupTriggerPayload) => void): Promise<UnlistenFn> {
    if (this.isTauri) {
      return listen<PopupTriggerPayload>(IPC_EVENTS.POPUP_TRIGGERED, (e) => cb(e.payload));
    }
    return this.mock.onPopupTriggered(cb);
  }

  async onScanDiscovered(cb: (device: DiscoveredDevice) => void): Promise<UnlistenFn> {
    if (this.isTauri) {
      return listen<DiscoveredDevice>(IPC_EVENTS.SCAN_DISCOVERED, (e) => cb(e.payload));
    }
    return this.mock.onScanDiscovered(cb);
  }

  async onProtocolLog(cb: (entry: ProtocolLogEntry) => void): Promise<UnlistenFn> {
    if (this.isTauri) {
      return listen<ProtocolLogEntry>(IPC_EVENTS.PROTOCOL_LOG, (e) => cb(e.payload));
    }
    return this.mock.onProtocolLog(cb);
  }
}

export const qcyClient = new QcyIpcClient();
