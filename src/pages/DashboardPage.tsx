import React, { useMemo } from "react";
import {
  AlertCircle,
  Disc3,
  Ear,
  Gamepad2,
  Headphones,
  Link2,
  MapPin,
  Settings,
  Sliders,
  Sparkles,
  Fingerprint,
} from "lucide-react";
import {
  AncControlPanel,
  BatteryOverviewCard,
  RgbLightingControl,
  QCY_CARD_CLASS,
  QCY_SUBCARD_CLASS,
  toggleTrackClass,
  type AiAncActivity as CompAiAncActivity,
  type AiAncCellMode,
  type AiAncMatrix as CompAiAncMatrix,
  type AiAncNoiseTier,
  type AncCycleOption,
  type AncSceneId,
  type DeviceConnectionMode,
} from "../components";
import { useI18n } from "../i18n";
import { useQcyStore } from "../store/useQcyStore";
import type {
  AiAncMatrix as StoreAiAncMatrix,
  AiAncNoiseLevel,
  AncPrimaryMode,
  AncScene,
  AppTheme,
} from "../types/qcy";

export interface DashboardPageProps {
  onNavigate?: (tab: string) => void;
}

function resolveUiTheme(theme: AppTheme | undefined): "light" | "dark" {
  if (theme === "light" || theme === "dark") return theme;
  if (typeof window !== "undefined" && typeof window.matchMedia === "function") {
    return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }
  return "dark";
}

function mapNoiseToTier(level: AiAncNoiseLevel): AiAncNoiseTier {
  if (level === "low") return "mild";
  if (level === "high") return "deep";
  return "medium";
}

function mapTierToNoise(tier: AiAncNoiseTier): AiAncNoiseLevel {
  if (tier === "mild") return "low";
  if (tier === "deep") return "high";
  return "medium";
}

function toCellMode(mode: AncPrimaryMode): AiAncCellMode {
  return mode === "custom" ? "anc" : mode;
}

function toComponentAiMatrix(matrix: StoreAiAncMatrix): CompAiAncMatrix {
  return {
    stay: {
      mild: toCellMode(matrix.stay.low),
      medium: toCellMode(matrix.stay.medium),
      deep: toCellMode(matrix.stay.high),
    },
    walk: {
      mild: toCellMode(matrix.walk.low),
      medium: toCellMode(matrix.walk.medium),
      deep: toCellMode(matrix.walk.high),
    },
    run: {
      mild: toCellMode(matrix.run.low),
      medium: toCellMode(matrix.run.medium),
      deep: toCellMode(matrix.run.high),
    },
  };
}

export const DashboardPage: React.FC<DashboardPageProps> = ({ onNavigate }) => {
  const {
    snapshot,
    loading,
    setAncState,
    setAiAnc,
    setAncButtonCycle,
    setModes,
    setMultipoint,
    setRgbLighting,
    setDeviceColor,
    renameDevice,
    openWindowsBtSettings,
  } = useQcyStore();
  const { locale, t } = useI18n();

  const uiTheme = resolveUiTheme(snapshot?.settings.theme);

  const connectionMode: DeviceConnectionMode = useMemo(() => {
    if (!snapshot) return "disconnected";
    if (snapshot.connection_state === "connecting" || snapshot.connection_state === "reconnecting") {
      return "connecting";
    }
    if (snapshot.connection_state === "disconnected") return "disconnected";
    return snapshot.is_virtual_session ? "virtual_simulator" : "ble_live";
  }, [snapshot]);

  if (loading && !snapshot) {
    return (
      <div className={`${QCY_CARD_CLASS} py-12 text-center text-sm text-[#808080] dark:text-white/55`}>
        {t("common.loading")}
      </div>
    );
  }

  if (!snapshot) {
    return (
      <div className={`${QCY_CARD_CLASS} py-12 text-center text-sm text-[#808080] dark:text-white/55`}>
        {t("dashboard.no_active_device")}
      </div>
    );
  }

  const { active_model: model, active_device: device, battery, anc, modes, multipoint, rgb_lighting } =
    snapshot;
  const caps = model.capabilities;
  const modelDisplayName = locale === "zh-CN" ? model.name_zh : model.name_en;
  const deviceTitle = device?.name || modelDisplayName;
  const isDisconnected = snapshot.connection_state === "disconnected";

  const hasSpatialToggle = caps.spatial_audio_modes.length > 0;
  const spatialActive = modes.spatial_audio !== "off";
  const gameActive = caps.game_mode_pro ? modes.game_mode_pro : modes.game_mode;
  const hasQuickToggles =
    caps.game_mode ||
    caps.game_mode_pro ||
    hasSpatialToggle ||
    caps.dual_device ||
    caps.multi_device ||
    caps.in_ear_detection ||
    caps.ldac;

  const activeEqSubtitle = (() => {
    const activeId = snapshot.eq.active_preset_id;
    const customMatch = snapshot.eq.custom_presets.find((p) => p.id === activeId);
    if (customMatch) {
      return locale === "zh-CN"
        ? customMatch.name_zh || customMatch.name_en
        : customMatch.name_en || customMatch.name_zh;
    }
    if (!activeId || activeId === "custom") {
      return t("eq.tab_custom");
    }
    const presetKey = `eq.presets.${activeId}`;
    const translated = t(presetKey);
    return translated === presetKey ? t("eq.tab_custom") : translated;
  })();

  const quickNavTiles = [
    {
      id: "eq",
      label: t("dashboard.sound_effects"),
      sub: activeEqSubtitle,
      Icon: Sliders,
      show: true,
    },
    {
      id: "gestures",
      label: t("dashboard.gestures"),
      sub: snapshot.gestures.touch_enable ? t("common.enabled") : t("common.disabled"),
      Icon: Fingerprint,
      show: caps.button_remap || caps.head_controls,
    },
    {
      id: "find",
      label: t("dashboard.find_earbuds"),
      sub: t("find_earbuds.ring_both"),
      Icon: MapPin,
      show: caps.find_earbuds,
    },
    {
      id: "modes",
      label: t("dashboard.device_settings"),
      sub: model.chipset_family.toUpperCase(),
      Icon: Settings,
      show: true,
    },
  ].filter((item) => item.show);

  return (
    <div className="space-y-5">
      {/* Bluetooth Adapter Warning Banner (live mode only) */}
      {!snapshot.is_virtual_session && !snapshot.adapter.is_present && (
        <div className="flex flex-wrap items-center justify-between gap-3 rounded-2xl border border-[#EAB308]/35 bg-[#EAB308]/10 px-4 py-3 text-xs text-[#854D0E] dark:text-[#FDE047]">
          <span className="inline-flex items-center gap-2 font-medium">
            <AlertCircle className="h-4 w-4 shrink-0" />
            {t("connection.bt_adapter_unavailable")}
          </span>
          <button
            type="button"
            onClick={() => void openWindowsBtSettings()}
            className="rounded-lg bg-[#FF7A2F] px-3 py-1.5 font-semibold text-white hover:bg-[#E8691F]"
          >
            {t("connection.open_windows_bt_settings")}
          </button>
        </div>
      )}

      {/* Hero Battery & Device Overview Card */}
      <BatteryOverviewCard
        deviceName={deviceTitle}
        modelName={modelDisplayName}
        modelId={model.numeric_model_id_hex ?? model.model_id}
        formFactor={model.form_factor}
        hasCaseBattery={caps.has_case_battery}
        connectionMode={connectionMode}
        rssi={device?.rssi}
        macAddress={device?.address}
        firmwareVersion={snapshot.firmware_version}
        chipsetLabel={model.chipset_family}
        leftBattery={battery.left}
        rightBattery={battery.right}
        caseBattery={battery.case_level}
        leftConnected={battery.left !== null && !isDisconnected}
        rightConnected={battery.right !== null && !isDisconnected}
        leftInCase={battery.left_in_case}
        rightInCase={battery.right_in_case}
        leftCharging={battery.left_charging}
        rightCharging={battery.right_charging}
        caseCharging={battery.case_charging}
        colorIndex={snapshot.device_color_index}
        canChangeColor={caps.device_color}
        onColorChange={(idx) => void setDeviceColor(idx)}
        canRename={caps.rename}
        onRename={(next) => void renameDevice(next)}
        rgbEnabled={caps.rgb_lighting && rgb_lighting.enabled}
        rgbColor={rgb_lighting.color_hex}
        rgbMode={rgb_lighting.mode}
        theme={uiTheme}
        t={t}
      />

      {/* Quick Navigation 2x2 / 4-col Tiles */}
      {onNavigate && quickNavTiles.length > 0 && (
        <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
          {quickNavTiles.map(({ id, label, sub, Icon }) => (
            <button
              key={id}
              type="button"
              onClick={() => onNavigate(id)}
              className="group flex items-center gap-3 rounded-2xl border border-[#E5E5E5] bg-white p-4 text-left shadow-xs transition hover:border-[#FF7A2F]/60 dark:border-white/10 dark:bg-[#1E1E1E] dark:hover:border-[#FF7A2F]/60"
            >
              <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-[#FF7A2F]/12 text-[#FF7A2F] transition group-hover:bg-[#FF7A2F] group-hover:text-white">
                <Icon className="h-5 w-5" />
              </span>
              <div className="min-w-0">
                <p className="truncate text-sm font-bold text-[#1A1A1A] dark:text-white">
                  {label}
                </p>
                <p className="truncate text-xs text-[#808080] dark:text-white/55">{sub}</p>
              </div>
            </button>
          ))}
        </div>
      )}

      {/* Noise Control Panel (capability-gated) */}
      {caps.anc && (
        <AncControlPanel
          capabilities={{
            anc: caps.anc,
            aiAnc: caps.ai_anc,
            ancCustomLevels: caps.anc_custom_levels,
            transparencyLevels: caps.transparency_levels,
            vocalBoost: caps.vocal_boost,
            windNoise: caps.wind_noise,
            ancButtonCycle: caps.anc_button_cycle,
            ancScenes: caps.anc_scenes as AncSceneId[],
          }}
          state={{
            mode: anc.mode,
            scene: anc.scene as AncSceneId,
            sceneDepth: anc.scene_level,
            customDepth: anc.custom_level,
            transparencyLevel: anc.transparency_level,
            vocalBoost: anc.vocal_boost,
            windNoiseReduction: anc.wind_noise,
            aiAncEnabled: anc.ai_anc_enabled,
            aiAncActivity: anc.ai_activity as CompAiAncActivity,
            aiAncNoiseTier: mapNoiseToTier(anc.ai_noise_level),
            aiAncMatrix: toComponentAiMatrix(anc.ai_matrix),
            ancCycleModes: anc.btn_anc_cfg as AncCycleOption[],
          }}
          disabled={isDisconnected}
          onModeChange={(mode) => void setAncState({ mode })}
          onSceneChange={(scene, depth) =>
            void setAncState({
              mode: "anc",
              scene: (scene === "noisy" ? "outdoor" : scene) as AncScene,
              scene_level: depth ?? anc.scene_level,
            })
          }
          onCustomDepthChange={(level) =>
            void setAncState({ mode: "custom", custom_level: level })
          }
          onTransparencyLevelChange={(level) =>
            void setAncState({ mode: "transparency", transparency_level: level })
          }
          onVocalBoostChange={(enabled) =>
            void setAncState({ mode: anc.mode, vocal_boost: enabled })
          }
          onWindNoiseChange={(enabled) =>
            void setAncState({ mode: anc.mode, wind_noise: enabled })
          }
          onAiAncToggle={(enabled) => void setAiAnc(enabled)}
          onAiAncMatrixCellChange={(activity, tier, cellMode) => {
            const storeNoise = mapTierToNoise(tier);
            const nextMatrix: StoreAiAncMatrix = {
              stay: { ...anc.ai_matrix.stay },
              walk: { ...anc.ai_matrix.walk },
              run: { ...anc.ai_matrix.run },
            };
            nextMatrix[activity][storeNoise] = cellMode;
            void setAiAnc(anc.ai_anc_enabled, nextMatrix);
          }}
          onAiAncResetMatrix={() =>
            void setAiAnc(anc.ai_anc_enabled, {
              stay: { low: "off", medium: "anc", high: "anc" },
              walk: { low: "transparency", medium: "anc", high: "anc" },
              run: { low: "transparency", medium: "transparency", high: "anc" },
            })
          }
          onAncCycleChange={(nextModes) => void setAncButtonCycle(nextModes)}
          onAncCycleReset={() =>
            void setAncButtonCycle(["anc_adaptive", "off", "transparency"])
          }
          t={t}
        />
      )}

      {/* Quick Feature Toggle Cards (Conflict-Guarded via Store) */}
      {hasQuickToggles && (
        <section className={`${QCY_CARD_CLASS} space-y-3.5`} aria-label={t("dashboard.quick_controls")}>
          <h3 className="text-base font-bold text-[#1A1A1A] dark:text-white">
            {t("dashboard.quick_controls")}
          </h3>

          <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
            {/* Game Mode / Game Mode Pro */}
            {(caps.game_mode || caps.game_mode_pro) && (
              <div className={`${QCY_SUBCARD_CLASS} flex items-center justify-between gap-3`}>
                <div className="flex items-center gap-2.5">
                  <Gamepad2 className="h-4 w-4 shrink-0 text-[#FF7A2F]" />
                  <div>
                    <p className="text-sm font-semibold text-[#1A1A1A] dark:text-white">
                      {caps.game_mode_pro ? t("audio_modes.game_mode_pro") : t("audio_modes.game_mode")}
                    </p>
                    <p className="text-[11px] text-[#808080] dark:text-white/55">
                      {t("audio_modes.game_mode_desc")}
                    </p>
                  </div>
                </div>
                <button
                  type="button"
                  role="switch"
                  aria-checked={gameActive}
                  disabled={isDisconnected}
                  onClick={() =>
                    void setModes(
                      caps.game_mode_pro
                        ? { game_mode_pro: !modes.game_mode_pro, game_mode: !modes.game_mode_pro }
                        : { game_mode: !modes.game_mode }
                    )
                  }
                  className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors ${toggleTrackClass(
                    gameActive
                  )}`}
                >
                  <span
                    className={`inline-block h-4 w-4 rounded-full bg-white shadow transition-transform ${
                      gameActive ? "translate-x-6" : "translate-x-1"
                    }`}
                  />
                </button>
              </div>
            )}

            {/* Spatial Audio Quick Toggle */}
            {hasSpatialToggle && (
              <div className={`${QCY_SUBCARD_CLASS} flex items-center justify-between gap-3`}>
                <div className="flex items-center gap-2.5">
                  <Sparkles className="h-4 w-4 shrink-0 text-[#727AFF]" />
                  <div>
                    <p className="text-sm font-semibold text-[#1A1A1A] dark:text-white">
                      {t("audio_modes.spatial_audio")}
                    </p>
                    <p className="text-[11px] text-[#808080] dark:text-white/55">
                      {t(`audio_modes.spatial_${modes.spatial_audio}`)}
                    </p>
                  </div>
                </div>
                <button
                  type="button"
                  role="switch"
                  aria-checked={spatialActive}
                  disabled={isDisconnected}
                  onClick={() =>
                    void setModes({
                      spatial_audio: spatialActive ? "off" : "fixed",
                    })
                  }
                  className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors ${toggleTrackClass(
                    spatialActive,
                    "blue"
                  )}`}
                >
                  <span
                    className={`inline-block h-4 w-4 rounded-full bg-white shadow transition-transform ${
                      spatialActive ? "translate-x-6" : "translate-x-1"
                    }`}
                  />
                </button>
              </div>
            )}

            {/* Dual / Multi-Device Connection */}
            {(caps.dual_device || caps.multi_device) && (
              <div className={`${QCY_SUBCARD_CLASS} flex items-center justify-between gap-3`}>
                <div className="flex items-center gap-2.5">
                  <Link2 className="h-4 w-4 shrink-0 text-[#0066FF] dark:text-[#3399FF]" />
                  <div>
                    <p className="text-sm font-semibold text-[#1A1A1A] dark:text-white">
                      {caps.multi_device
                        ? t("connectivity.multi_device_title")
                        : t("connectivity.dual_device_title")}
                    </p>
                    <p className="text-[11px] text-[#808080] dark:text-white/55">
                      {t("connectivity.dual_device_desc")}
                    </p>
                  </div>
                </div>
                <button
                  type="button"
                  role="switch"
                  aria-checked={multipoint.dual_device}
                  disabled={isDisconnected}
                  onClick={() =>
                    void setMultipoint({ dual_device: !multipoint.dual_device })
                  }
                  className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors ${toggleTrackClass(
                    multipoint.dual_device,
                    "blue"
                  )}`}
                >
                  <span
                    className={`inline-block h-4 w-4 rounded-full bg-white shadow transition-transform ${
                      multipoint.dual_device
                        ? "translate-x-6"
                        : "translate-x-1"
                    }`}
                  />
                </button>
              </div>
            )}

            {/* In-Ear Wear Detection */}
            {caps.in_ear_detection && (
              <div className={`${QCY_SUBCARD_CLASS} flex items-center justify-between gap-3`}>
                <div className="flex items-center gap-2.5">
                  <Ear className="h-4 w-4 shrink-0 text-[#22C55E]" />
                  <div>
                    <p className="text-sm font-semibold text-[#1A1A1A] dark:text-white">
                      {t("wear_and_health.in_ear_title")}
                    </p>
                    <p className="text-[11px] text-[#808080] dark:text-white/55">
                      {t("wear_and_health.in_ear_desc")}
                    </p>
                  </div>
                </div>
                <button
                  type="button"
                  role="switch"
                  aria-checked={modes.in_ear_mode > 0}
                  disabled={isDisconnected}
                  onClick={() =>
                    void setModes({ in_ear_mode: modes.in_ear_mode > 0 ? 0 : 1 })
                  }
                  className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors ${toggleTrackClass(
                    modes.in_ear_mode > 0
                  )}`}
                >
                  <span
                    className={`inline-block h-4 w-4 rounded-full bg-white shadow transition-transform ${
                      modes.in_ear_mode > 0 ? "translate-x-6" : "translate-x-1"
                    }`}
                  />
                </button>
              </div>
            )}

            {/* LDAC High-Res Audio */}
            {caps.ldac && (
              <div className={`${QCY_SUBCARD_CLASS} flex items-center justify-between gap-3`}>
                <div className="flex items-center gap-2.5">
                  <Disc3 className="h-4 w-4 shrink-0 text-[#FF7A2F]" />
                  <div>
                    <p className="text-sm font-semibold text-[#1A1A1A] dark:text-white">
                      {t("audio_modes.ldac")}
                    </p>
                    <p className="text-[11px] text-[#808080] dark:text-white/55">
                      {t("audio_modes.ldac_desc")}
                    </p>
                  </div>
                </div>
                <button
                  type="button"
                  role="switch"
                  aria-checked={modes.ldac}
                  disabled={isDisconnected}
                  onClick={() => void setModes({ ldac: !modes.ldac })}
                  className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors ${toggleTrackClass(
                    modes.ldac
                  )}`}
                >
                  <span
                    className={`inline-block h-4 w-4 rounded-full bg-white shadow transition-transform ${
                      modes.ldac ? "translate-x-6" : "translate-x-1"
                    }`}
                  />
                </button>
              </div>
            )}

            {/* Listening Mode shortcut pill for Over-Ear H6/H6 Neo */}
            {caps.listening_modes.length > 0 && (
              <div className={`${QCY_SUBCARD_CLASS} flex items-center justify-between gap-2`}>
                <div className="flex items-center gap-2">
                  <Headphones className="h-4 w-4 shrink-0 text-[#0066FF] dark:text-[#3399FF]" />
                  <span className="text-sm font-semibold text-[#1A1A1A] dark:text-white">
                    {t("audio_modes.listening_mode")}
                  </span>
                </div>
                <div className="inline-flex gap-1 rounded-lg bg-white p-0.5 dark:bg-white/8">
                  {(["music", "game", "movie"] as const).map((m) => (
                    <button
                      key={m}
                      type="button"
                      disabled={isDisconnected}
                      onClick={() => void setModes({ listening_mode: m })}
                      className={`rounded-md px-2 py-1 text-[11px] font-semibold transition ${
                        modes.listening_mode === m
                          ? "bg-[#FF7A2F] text-white"
                          : "text-[#4D4D4D] dark:text-white/70"
                      }`}
                    >
                      {t(`audio_modes.listening_${m}`)}
                    </button>
                  ))}
                </div>
              </div>
            )}
          </div>
        </section>
      )}

      {/* SP75 Speaker RGB LED Lighting & TWS Pairing (embedded when supported) */}
      {caps.rgb_lighting && (
        <RgbLightingControl
          state={{
            enabled: rgb_lighting.enabled,
            mode: rgb_lighting.mode,
            colorHex: rgb_lighting.color_hex,
            brightness: rgb_lighting.brightness,
            speed: rgb_lighting.speed,
            twsSpeakerPairEnabled: rgb_lighting.tws_pair_enabled,
          }}
          supportsTwsPair={caps.tws_speaker_pair}
          connected={!isDisconnected}
          disabled={isDisconnected}
          theme={uiTheme}
          onChange={(patch) =>
            void setRgbLighting({
              enabled: patch.enabled,
              mode: patch.mode,
              color_hex: patch.colorHex,
              brightness: patch.brightness,
              speed: patch.speed,
              tws_pair: patch.twsSpeakerPairEnabled,
            })
          }
          onTwsPairChange={(enabled) => void setRgbLighting({ tws_pair: enabled })}
          t={t}
        />
      )}
    </div>
  );
};
