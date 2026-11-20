import React, { useState } from "react";
import {
  Activity,
  Building2,
  Ear,
  Footprints,
  Gauge,
  Home,
  Mic,
  PersonStanding,
  RefreshCw,
  RotateCcw,
  Shield,
  Sliders,
  Sparkles,
  TrainFront,
  Trees,
  Volume2,
  VolumeX,
  Wind,
} from "lucide-react";
import {
  QCY_CARD_CLASS,
  QCY_SUBCARD_CLASS,
  TranslateFn,
  toggleTrackClass,
} from "./shared";

export type AncPrimaryMode = "anc" | "off" | "transparency" | "custom";

export type AncSceneId =
  | "adaptive"
  | "indoor"
  | "commute"
  | "outdoor"
  | "noisy"
  | "anti_wind"
  | "high"
  | "medium"
  | "low";

export type AiAncActivity = "detecting" | "stay" | "walk" | "run" | "off";
export type AiAncNoiseTier = "mild" | "medium" | "deep";
export type AiAncCellMode = "anc" | "off" | "transparency";

export type AiAncMatrix = Record<
  "stay" | "walk" | "run",
  Record<AiAncNoiseTier, AiAncCellMode>
>;

export type AncCycleOption =
  | "anc_adaptive"
  | "anc_indoor"
  | "anc_commute"
  | "anc_outdoor"
  | "anc_anti_wind"
  | "off"
  | "transparency"
  | "vocal_boost";

export interface AncCapabilities {
  anc: boolean;
  aiAnc?: boolean;
  ancCustomLevels?: number;
  transparencyLevels?: number;
  vocalBoost?: boolean;
  windNoise?: boolean;
  ancButtonCycle?: boolean;
  ancScenes?: AncSceneId[];
}

export interface AncState {
  mode: AncPrimaryMode;
  scene: AncSceneId;
  /** Scene depth `0..2` for `indoor` / `commute` / `outdoor` / `noisy`. */
  sceneDepth: number;
  /** Custom ANC slider level `0..(ancCustomLevels - 1)` (e.g. `0..15`). */
  customDepth: number;
  /** Transparency gain `1..transparencyLevels` (e.g. `1..6`). */
  transparencyLevel: number;
  vocalBoost: boolean;
  windNoiseReduction: boolean;
  calibrating?: boolean;
  aiAncEnabled: boolean;
  aiAncActivity?: AiAncActivity;
  aiAncNoiseTier?: AiAncNoiseTier;
  aiAncMatrix?: AiAncMatrix;
  ancCycleModes?: AncCycleOption[];
}

export interface AncControlPanelProps {
  capabilities: AncCapabilities;
  state: AncState;
  disabled?: boolean;
  onModeChange: (mode: AncPrimaryMode) => void;
  onSceneChange?: (scene: AncSceneId, depth?: number) => void;
  onCustomDepthChange?: (level: number) => void;
  onTransparencyLevelChange?: (level: number) => void;
  onVocalBoostChange?: (enabled: boolean) => void;
  onWindNoiseChange?: (enabled: boolean) => void;
  onAiAncToggle?: (enabled: boolean) => void;
  onAiAncMatrixCellChange?: (
    activity: "stay" | "walk" | "run",
    noise: AiAncNoiseTier,
    mode: AiAncCellMode
  ) => void;
  onAiAncResetMatrix?: () => void;
  onAncCycleChange?: (modes: AncCycleOption[]) => void;
  onAncCycleReset?: () => void;
  t: TranslateFn;
  className?: string;
}

export const DEFAULT_AI_ANC_MATRIX: AiAncMatrix = {
  stay: { mild: "off", medium: "anc", deep: "anc" },
  walk: { mild: "transparency", medium: "anc", deep: "anc" },
  run: { mild: "transparency", medium: "transparency", deep: "anc" },
};

const DEPTH_SUPPORTED_SCENES: ReadonlySet<AncSceneId> = new Set([
  "indoor",
  "commute",
  "outdoor",
  "noisy",
]);

const DEFAULT_SCENES: AncSceneId[] = [
  "adaptive",
  "indoor",
  "commute",
  "outdoor",
  "anti_wind",
];

const CYCLE_OPTIONS: Array<{ id: AncCycleOption; labelKey: string }> = [
  { id: "anc_adaptive", labelKey: "anc.scene_adaptive" },
  { id: "anc_indoor", labelKey: "anc.scene_indoor" },
  { id: "anc_commute", labelKey: "anc.scene_commute" },
  { id: "anc_outdoor", labelKey: "anc.scene_outdoor" },
  { id: "anc_anti_wind", labelKey: "anc.scene_anti_wind" },
  { id: "off", labelKey: "anc.mode_normal" },
  { id: "transparency", labelKey: "anc.mode_transparency" },
  { id: "vocal_boost", labelKey: "anc.vocal_boost" },
];

/** Pure helper enforcing QCY `btnAncCfg` rule: at least 2 modes must remain selected. */
export function toggleAncCycleOption(
  current: readonly AncCycleOption[],
  option: AncCycleOption
): { next: AncCycleOption[]; blockedMinTwo: boolean } {
  const exists = current.includes(option);
  if (exists) {
    if (current.length <= 2) {
      return { next: [...current], blockedMinTwo: true };
    }
    return { next: current.filter((m) => m !== option), blockedMinTwo: false };
  }
  return { next: [...current, option], blockedMinTwo: false };
}

function getSceneMeta(scene: AncSceneId): {
  labelKey: string;
  descKey: string;
  Icon: React.ComponentType<{ className?: string }>;
} {
  switch (scene) {
    case "adaptive":
      return { labelKey: "anc.scene_adaptive", descKey: "anc.scene_adaptive_desc", Icon: Sparkles };
    case "indoor":
      return { labelKey: "anc.scene_indoor", descKey: "anc.scene_indoor_desc", Icon: Home };
    case "commute":
      return { labelKey: "anc.scene_commute", descKey: "anc.scene_commute_desc", Icon: TrainFront };
    case "outdoor":
      return { labelKey: "anc.scene_outdoor", descKey: "anc.scene_outdoor_desc", Icon: Trees };
    case "noisy":
      return { labelKey: "anc.scene_noisy", descKey: "anc.scene_noisy_desc", Icon: Building2 };
    case "anti_wind":
      return { labelKey: "anc.scene_anti_wind", descKey: "anc.scene_anti_wind_desc", Icon: Wind };
    case "high":
      return { labelKey: "anc.scene_high", descKey: "anc.scene_noisy_desc", Icon: Shield };
    case "medium":
      return { labelKey: "anc.scene_medium", descKey: "anc.scene_outdoor_desc", Icon: Gauge };
    case "low":
      return { labelKey: "anc.scene_low", descKey: "anc.scene_indoor_desc", Icon: Volume2 };
  }
}

export const AncControlPanel: React.FC<AncControlPanelProps> = ({
  capabilities,
  state,
  disabled = false,
  onModeChange,
  onSceneChange,
  onCustomDepthChange,
  onTransparencyLevelChange,
  onVocalBoostChange,
  onWindNoiseChange,
  onAiAncToggle,
  onAiAncMatrixCellChange,
  onAiAncResetMatrix,
  onAncCycleChange,
  onAncCycleReset,
  t,
  className = "",
}) => {
  const [minTwoWarning, setMinTwoWarning] = useState(false);

  if (!capabilities.anc) {
    return null;
  }

  const customLevels = capabilities.ancCustomLevels ?? 0;
  const transparencyLevels = capabilities.transparencyLevels ?? 0;
  const scenes =
    capabilities.ancScenes && capabilities.ancScenes.length > 0
      ? capabilities.ancScenes
      : DEFAULT_SCENES;
  const matrix = state.aiAncMatrix ?? DEFAULT_AI_ANC_MATRIX;
  const cycleModes = state.ancCycleModes ?? ["anc_adaptive", "off", "transparency"];

  const primaryTabs: Array<{
    id: AncPrimaryMode;
    label: string;
    Icon: React.ComponentType<{ className?: string }>;
  }> = [
    { id: "anc", label: t("anc.mode_anc"), Icon: Shield },
    { id: "off", label: t("anc.mode_off"), Icon: VolumeX },
    { id: "transparency", label: t("anc.mode_transparency"), Icon: Ear },
  ];
  if (customLevels > 0) {
    primaryTabs.push({ id: "custom", label: t("anc.mode_custom"), Icon: Sliders });
  }

  const handleCycleClick = (opt: AncCycleOption) => {
    if (!onAncCycleChange || disabled) return;
    const { next, blockedMinTwo } = toggleAncCycleOption(cycleModes, opt);
    if (blockedMinTwo) {
      setMinTwoWarning(true);
      setTimeout(() => setMinTwoWarning(false), 2200);
      return;
    }
    setMinTwoWarning(false);
    onAncCycleChange(next);
  };

  const activities: Array<{
    id: "stay" | "walk" | "run";
    labelKey: string;
    Icon: React.ComponentType<{ className?: string }>;
  }> = [
    { id: "stay", labelKey: "ai_anc.activity_stay", Icon: PersonStanding },
    { id: "walk", labelKey: "ai_anc.activity_walk", Icon: Footprints },
    { id: "run", labelKey: "ai_anc.activity_run", Icon: Activity },
  ];

  const noiseTiers: Array<{ id: AiAncNoiseTier; labelKey: string }> = [
    { id: "mild", labelKey: "ai_anc.noise_mild" },
    { id: "medium", labelKey: "ai_anc.noise_medium" },
    { id: "deep", labelKey: "ai_anc.noise_deep" },
  ];

  const cycleAiCell = (mode: AiAncCellMode): AiAncCellMode => {
    if (mode === "anc") return "off";
    if (mode === "off") return "transparency";
    return "anc";
  };

  return (
    <section className={`${QCY_CARD_CLASS} space-y-5 ${className}`} aria-label={t("anc.title")}>
      {/* Header */}
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex items-center gap-2.5">
          <span className="flex h-9 w-9 items-center justify-center rounded-xl bg-[#FF7A2F]/14 text-[#FF7A2F]">
            <Shield className="h-5 w-5" />
          </span>
          <div>
            <h3 className="text-base font-bold text-[#1A1A1A] dark:text-white">
              {t("anc.title")}
            </h3>
            {state.calibrating && (
              <p className="inline-flex items-center gap-1 text-xs font-medium text-[#0066FF] dark:text-[#3399FF]">
                <RefreshCw className="h-3 w-3 animate-spin" />
                {t("anc.calibrating")}
              </p>
            )}
          </div>
        </div>
      </div>

      {/* Primary Mode Segmented Control */}
      <div
        className={`grid gap-1.5 rounded-xl bg-[#F2F1F6] p-1.5 dark:bg-white/[0.06] ${
          primaryTabs.length === 4 ? "grid-cols-2 sm:grid-cols-4" : "grid-cols-3"
        }`}
        role="tablist"
        aria-label={t("anc.title")}
      >
        {primaryTabs.map(({ id, label, Icon }) => {
          const active = state.mode === id;
          return (
            <button
              key={id}
              type="button"
              role="tab"
              aria-selected={active}
              disabled={disabled}
              onClick={() => onModeChange(id)}
              className={`flex items-center justify-center gap-2 rounded-lg px-3 py-2.5 text-sm font-semibold transition ${
                active
                  ? "bg-[#FF7A2F] text-white shadow-xs"
                  : "text-[#4D4D4D] hover:bg-black/5 dark:text-white/75 dark:hover:bg-white/8"
              } disabled:cursor-not-allowed disabled:opacity-50`}
            >
              <Icon className="h-4 w-4 shrink-0" />
              <span className="truncate">{label}</span>
            </button>
          );
        })}
      </div>

      {/* ANC Scene Cards + Scene Depth Selector */}
      {state.mode === "anc" && (
        <div className="space-y-3">
          <div className="grid grid-cols-2 gap-2.5 sm:grid-cols-3 lg:grid-cols-5">
            {scenes.map((scene) => {
              const { labelKey, descKey, Icon } = getSceneMeta(scene);
              const selected = state.scene === scene;
              return (
                <button
                  key={scene}
                  type="button"
                  disabled={disabled}
                  onClick={() => onSceneChange?.(scene, state.sceneDepth)}
                  className={`flex flex-col items-start gap-1.5 rounded-xl border p-3 text-left transition ${
                    selected
                      ? "border-[#FF7A2F] bg-[#FF7A2F]/10 dark:bg-[#FF7A2F]/15"
                      : "border-[#E5E5E5] bg-[#F2F1F6]/60 hover:border-[#FF7A2F]/45 dark:border-white/8 dark:bg-white/[0.03]"
                  } disabled:cursor-not-allowed disabled:opacity-50`}
                  aria-pressed={selected}
                >
                  <span
                    className={`flex h-7 w-7 items-center justify-center rounded-lg ${
                      selected
                        ? "bg-[#FF7A2F] text-white"
                        : "bg-black/5 text-[#4D4D4D] dark:bg-white/10 dark:text-white/70"
                    }`}
                  >
                    <Icon className="h-4 w-4" />
                  </span>
                  <span className="text-xs font-bold text-[#1A1A1A] dark:text-white">
                    {t(labelKey)}
                  </span>
                  <span className="line-clamp-2 text-[11px] leading-snug text-[#808080] dark:text-white/55">
                    {t(descKey)}
                  </span>
                </button>
              );
            })}
          </div>

          {DEPTH_SUPPORTED_SCENES.has(state.scene) && onSceneChange && (
            <div className={QCY_SUBCARD_CLASS}>
              <div className="mb-2 flex items-center justify-between text-xs">
                <span className="font-semibold text-[#1A1A1A] dark:text-white">
                  {t("anc.anc_depth")}
                </span>
                <span className="font-semibold text-[#FF7A2F]">
                  {state.sceneDepth === 0
                    ? t("anc.scene_low")
                    : state.sceneDepth === 1
                      ? t("anc.scene_medium")
                      : t("anc.scene_high")}
                </span>
              </div>
              <div className="grid grid-cols-3 gap-2">
                {[0, 1, 2].map((lvl) => (
                  <button
                    key={lvl}
                    type="button"
                    disabled={disabled}
                    onClick={() => onSceneChange(state.scene, lvl)}
                    className={`rounded-lg py-1.5 text-xs font-semibold transition ${
                      state.sceneDepth === lvl
                        ? "bg-[#0066FF] text-white dark:bg-[#3399FF]"
                        : "bg-white text-[#4D4D4D] hover:bg-black/5 dark:bg-white/8 dark:text-white/75"
                    }`}
                  >
                    {lvl === 0
                      ? t("anc.scene_low")
                      : lvl === 1
                        ? t("anc.scene_medium")
                        : t("anc.scene_high")}
                  </button>
                ))}
              </div>
            </div>
          )}
        </div>
      )}

      {/* Custom 16-Level ANC Depth Slider (e.g. QCY H6) */}
      {state.mode === "custom" && customLevels > 0 && (
        <div className={QCY_SUBCARD_CLASS}>
          <div className="mb-2 flex items-center justify-between text-xs">
            <span className="font-semibold text-[#1A1A1A] dark:text-white">
              {t("anc.custom_depth_level", {
                level: state.customDepth + 1,
                max: customLevels,
              })}
            </span>
            <span className="font-mono font-bold text-[#FF7A2F]">
              {Math.round(((state.customDepth + 1) / customLevels) * 100)}%
            </span>
          </div>
          <input
            type="range"
            min={0}
            max={customLevels - 1}
            step={1}
            disabled={disabled}
            value={state.customDepth}
            onChange={(e) => onCustomDepthChange?.(Number(e.target.value))}
            className="h-2 w-full cursor-pointer accent-[#FF7A2F]"
          />
          <div className="mt-1 flex justify-between text-[10px] text-[#808080] dark:text-white/45">
            <span>1</span>
            <span>{Math.ceil(customLevels / 2)}</span>
            <span>{customLevels}</span>
          </div>
        </div>
      )}

      {/* Transparency Level (1..6) & Vocal Boost */}
      {state.mode === "transparency" && (transparencyLevels > 0 || capabilities.vocalBoost) && (
        <div className="space-y-3">
          {transparencyLevels > 0 && (
            <div className={QCY_SUBCARD_CLASS}>
              <div className="mb-2 flex items-center justify-between text-xs">
                <span className="font-semibold text-[#1A1A1A] dark:text-white">
                  {t("anc.transparency_level", {
                    level: state.transparencyLevel,
                    max: transparencyLevels,
                  })}
                </span>
                <span className="font-mono font-bold text-[#0066FF] dark:text-[#3399FF]">
                  {state.transparencyLevel}/{transparencyLevels}
                </span>
              </div>
              <input
                type="range"
                min={1}
                max={transparencyLevels}
                step={1}
                disabled={disabled}
                value={Math.max(1, Math.min(transparencyLevels, state.transparencyLevel))}
                onChange={(e) => onTransparencyLevelChange?.(Number(e.target.value))}
                className="h-2 w-full cursor-pointer accent-[#0066FF] dark:accent-[#3399FF]"
              />
            </div>
          )}

          {capabilities.vocalBoost && (
            <div className={`${QCY_SUBCARD_CLASS} flex items-center justify-between gap-3`}>
              <div className="flex items-center gap-2.5">
                <Mic className="h-4 w-4 text-[#0066FF] dark:text-[#3399FF]" />
                <div>
                  <p className="text-sm font-semibold text-[#1A1A1A] dark:text-white">
                    {t("anc.vocal_boost")}
                  </p>
                  <p className="text-xs text-[#808080] dark:text-white/55">
                    {t("anc.vocal_boost_desc")}
                  </p>
                </div>
              </div>
              <button
                type="button"
                role="switch"
                aria-checked={state.vocalBoost}
                disabled={disabled}
                onClick={() => onVocalBoostChange?.(!state.vocalBoost)}
                className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors ${toggleTrackClass(
                  state.vocalBoost,
                  "blue"
                )}`}
              >
                <span
                  className={`inline-block h-4 w-4 rounded-full bg-white shadow transition-transform ${
                    state.vocalBoost ? "translate-x-6" : "translate-x-1"
                  }`}
                />
              </button>
            </div>
          )}
        </div>
      )}

      {/* Wind Noise Reduction Toggle */}
      {capabilities.windNoise && (
        <div className={`${QCY_SUBCARD_CLASS} flex items-center justify-between gap-3`}>
          <div className="flex items-center gap-2.5">
            <Wind className="h-4 w-4 text-[#FF7A2F]" />
            <div>
              <p className="text-sm font-semibold text-[#1A1A1A] dark:text-white">
                {t("anc.wind_noise")}
              </p>
              <p className="text-xs text-[#808080] dark:text-white/55">
                {t("anc.wind_noise_desc")}
              </p>
            </div>
          </div>
          <button
            type="button"
            role="switch"
            aria-checked={state.windNoiseReduction}
            disabled={disabled}
            onClick={() => onWindNoiseChange?.(!state.windNoiseReduction)}
            className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors ${toggleTrackClass(
              state.windNoiseReduction,
              "orange"
            )}`}
          >
            <span
              className={`inline-block h-4 w-4 rounded-full bg-white shadow transition-transform ${
                state.windNoiseReduction ? "translate-x-6" : "translate-x-1"
              }`}
            />
          </button>
        </div>
      )}

      {/* AI Scene ANC 3×3 Matrix Section */}
      {capabilities.aiAnc && (
        <div className={`${QCY_SUBCARD_CLASS} space-y-3.5`}>
          <div className="flex items-center justify-between gap-3">
            <div className="flex items-center gap-2.5">
              <Sparkles className="h-4 w-4 text-[#727AFF]" />
              <div>
                <p className="text-sm font-bold text-[#1A1A1A] dark:text-white">
                  {t("ai_anc.title")}
                </p>
                <p className="text-xs text-[#808080] dark:text-white/55">
                  {t("ai_anc.subtitle")}
                </p>
              </div>
            </div>
            <button
              type="button"
              role="switch"
              aria-checked={state.aiAncEnabled}
              disabled={disabled}
              onClick={() => onAiAncToggle?.(!state.aiAncEnabled)}
              className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors ${toggleTrackClass(
                state.aiAncEnabled,
                "orange"
              )}`}
            >
              <span
                className={`inline-block h-4 w-4 rounded-full bg-white shadow transition-transform ${
                  state.aiAncEnabled ? "translate-x-6" : "translate-x-1"
                }`}
              />
            </button>
          </div>

          {state.aiAncEnabled && (
            <div className="space-y-3 border-t border-[#E5E5E5] pt-3 dark:border-white/10">
              {/* Live Activity & Ambient Noise Monitor */}
              <div className="flex flex-wrap items-center justify-between gap-2 text-xs">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="rounded-md bg-[#727AFF]/15 px-2 py-1 font-semibold text-[#5B63E6] dark:text-[#A5AAFF]">
                    {t("ai_anc.activity_status")}:{" "}
                    {t(`ai_anc.activity_${state.aiAncActivity ?? "detecting"}`)}
                  </span>
                  <span className="rounded-md bg-[#0066FF]/12 px-2 py-1 font-semibold text-[#0066FF] dark:text-[#66B2FF]">
                    {t("ai_anc.ambient_noise_level")}:{" "}
                    {t(`ai_anc.noise_${state.aiAncNoiseTier ?? "mild"}`)}
                  </span>
                </div>
                {onAiAncResetMatrix && (
                  <button
                    type="button"
                    disabled={disabled}
                    onClick={onAiAncResetMatrix}
                    className="inline-flex items-center gap-1 rounded-lg px-2 py-1 text-xs font-medium text-[#808080] hover:bg-black/5 hover:text-[#1A1A1A] dark:text-white/60 dark:hover:bg-white/10 dark:hover:text-white"
                  >
                    <RotateCcw className="h-3 w-3" />
                    {t("common.reset")}
                  </button>
                )}
              </div>

              {/* 3×3 Grid */}
              <div className="overflow-x-auto">
                <table className="w-full border-separate border-spacing-1.5 text-xs">
                  <thead>
                    <tr>
                      <th className="py-1 text-left font-medium text-[#808080] dark:text-white/50">
                        {t("ai_anc.activity_status")}
                      </th>
                      {noiseTiers.map((tier) => (
                        <th
                          key={tier.id}
                          className="py-1 text-center font-semibold text-[#4D4D4D] dark:text-white/75"
                        >
                          {t(tier.labelKey)}
                        </th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {activities.map(({ id: act, labelKey, Icon }) => (
                      <tr key={act}>
                        <td className="py-1 pr-2 font-semibold text-[#1A1A1A] dark:text-white">
                          <span className="inline-flex items-center gap-1.5">
                            <Icon className="h-3.5 w-3.5 text-[#FF7A2F]" />
                            {t(labelKey)}
                          </span>
                        </td>
                        {noiseTiers.map(({ id: tier }) => {
                          const cellVal = matrix[act][tier];
                          const isLiveCell =
                            state.aiAncActivity === act && state.aiAncNoiseTier === tier;
                          const cellClass =
                            cellVal === "anc"
                              ? "border-[#FF7A2F]/40 bg-[#FF7A2F]/14 text-[#C2410C] dark:text-[#FF9E66]"
                              : cellVal === "transparency"
                                ? "border-[#0066FF]/40 bg-[#0066FF]/12 text-[#0066FF] dark:text-[#66B2FF]"
                                : "border-[#E5E5E5] bg-white text-[#4D4D4D] dark:border-white/10 dark:bg-white/6 dark:text-white/70";
                          return (
                            <td key={tier} className="text-center">
                              <button
                                type="button"
                                disabled={disabled}
                                onClick={() =>
                                  onAiAncMatrixCellChange?.(act, tier, cycleAiCell(cellVal))
                                }
                                className={`w-full rounded-lg border px-2 py-1.5 font-semibold transition hover:brightness-95 dark:hover:brightness-110 ${cellClass} ${
                                  isLiveCell ? "ring-2 ring-[#22C55E]" : ""
                                }`}
                              >
                                {cellVal === "anc"
                                  ? t("anc.mode_anc")
                                  : cellVal === "transparency"
                                    ? t("anc.mode_transparency")
                                    : t("anc.mode_normal")}
                              </button>
                            </td>
                          );
                        })}
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}
        </div>
      )}

      {/* Custom ANC Button Cycle (btnAncCfg, cmdID=101) */}
      {capabilities.ancButtonCycle && (
        <div className={`${QCY_SUBCARD_CLASS} space-y-2.5`}>
          <div className="flex items-center justify-between gap-2">
            <div>
              <p className="text-sm font-semibold text-[#1A1A1A] dark:text-white">
                {t("anc.anc_cycle_title")}
              </p>
              <p className="text-xs text-[#808080] dark:text-white/55">
                {t("anc.anc_cycle_desc")}
              </p>
            </div>
            {onAncCycleReset && (
              <button
                type="button"
                disabled={disabled}
                onClick={onAncCycleReset}
                className="inline-flex items-center gap-1 rounded-lg px-2 py-1 text-xs font-medium text-[#808080] hover:bg-black/5 hover:text-[#1A1A1A] dark:text-white/60 dark:hover:bg-white/10"
              >
                <RotateCcw className="h-3 w-3" />
                {t("anc.anc_cycle_reset")}
              </button>
            )}
          </div>

          <div className="flex flex-wrap gap-2">
            {CYCLE_OPTIONS.map((opt) => {
              const selected = cycleModes.includes(opt.id);
              const orderIndex = cycleModes.indexOf(opt.id);
              return (
                <button
                  key={opt.id}
                  type="button"
                  disabled={disabled}
                  onClick={() => handleCycleClick(opt.id)}
                  aria-pressed={selected}
                  className={`inline-flex items-center gap-1.5 rounded-full border px-3 py-1 text-xs font-semibold transition ${
                    selected
                      ? "border-[#FF7A2F] bg-[#FF7A2F] text-white"
                      : "border-[#E5E5E5] bg-white text-[#4D4D4D] hover:border-[#FF7A2F]/50 dark:border-white/10 dark:bg-white/6 dark:text-white/75"
                  }`}
                >
                  {selected && (
                    <span className="flex h-4 w-4 items-center justify-center rounded-full bg-white/25 text-[10px]">
                      {orderIndex + 1}
                    </span>
                  )}
                  {t(opt.labelKey)}
                </button>
              );
            })}
          </div>

          {minTwoWarning && (
            <p className="text-xs font-medium text-[#FF4C4C]" role="alert">
              {t("anc.anc_cycle_min_two")}
            </p>
          )}
        </div>
      )}
    </section>
  );
};
