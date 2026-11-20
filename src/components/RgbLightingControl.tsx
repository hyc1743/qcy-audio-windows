import React from "react";
import {
  Gauge,
  Link2,
  Music2,
  Palette,
  Sparkles,
  Sun,
  Waves,
} from "lucide-react";
import {
  DeviceIllustration,
  SpeakerRgbMode,
} from "../assets/DeviceIllustrations";
import {
  QCY_CARD_CLASS,
  QCY_SUBCARD_CLASS,
  TranslateFn,
  UiTheme,
  toggleTrackClass,
} from "./shared";

export interface RgbLightingState {
  enabled: boolean;
  mode: SpeakerRgbMode;
  colorHex: string;
  /** `0..100` */
  brightness: number;
  /** `1..100` */
  speed: number;
  twsSpeakerPairEnabled?: boolean;
}

export interface RgbLightingControlProps {
  state: RgbLightingState;
  supportsTwsPair?: boolean;
  deviceShellColorHex?: string;
  connected?: boolean;
  disabled?: boolean;
  theme?: UiTheme;
  onChange: (patch: Partial<RgbLightingState>) => void;
  onTwsPairChange?: (enabled: boolean) => void;
  t: TranslateFn;
  className?: string;
}

/** Exact SP75 preset colors observed in `function_sp75.json` + clean accent swatches. */
export const SP75_COLOR_PRESETS: readonly string[] = [
  "#FF7F00",
  "#FF0000",
  "#00FF00",
  "#FF00FF",
  "#00FFFF",
  "#FF7A2F",
  "#3399FF",
  "#FFFFFF",
];

const RGB_MODES: ReadonlyArray<{
  id: SpeakerRgbMode;
  effectIndex: number;
  labelKey: string;
  supportsCustomColor: boolean;
  Icon: React.ComponentType<{ className?: string }>;
}> = [
  { id: "solid", effectIndex: 1, labelKey: "lighting.mode_solid", supportsCustomColor: true, Icon: Sun },
  { id: "single_breathing", effectIndex: 2, labelKey: "lighting.mode_single_breathing", supportsCustomColor: true, Icon: Waves },
  { id: "gradient_breathing", effectIndex: 3, labelKey: "lighting.mode_gradient_breathing", supportsCustomColor: false, Icon: Sparkles },
  { id: "rhythm", effectIndex: 4, labelKey: "lighting.mode_rhythm", supportsCustomColor: false, Icon: Music2 },
];

export const RgbLightingControl: React.FC<RgbLightingControlProps> = ({
  state,
  supportsTwsPair = true,
  deviceShellColorHex = "#2A2E35",
  connected = true,
  disabled = false,
  theme = "dark",
  onChange,
  onTwsPairChange,
  t,
  className = "",
}) => {
  const activeModeMeta = RGB_MODES.find((m) => m.id === state.mode) ?? RGB_MODES[0];

  return (
    <section
      className={`${QCY_CARD_CLASS} space-y-5 ${className}`}
      aria-label={t("lighting.title")}
    >
      {/* Header + Master RGB Switch */}
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2.5">
          <span className="flex h-9 w-9 items-center justify-center rounded-xl bg-[#FF7A2F]/14 text-[#FF7A2F]">
            <Sparkles className="h-5 w-5" />
          </span>
          <div>
            <h3 className="text-base font-bold text-[#1A1A1A] dark:text-white">
              {t("lighting.title")}
            </h3>
            <p className="text-xs text-[#808080] dark:text-white/55">
              {t("lighting.preview_label")}
            </p>
          </div>
        </div>

        <button
          type="button"
          role="switch"
          aria-checked={state.enabled}
          aria-label={t("lighting.toggle_label")}
          disabled={disabled}
          onClick={() => onChange({ enabled: !state.enabled })}
          className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors ${toggleTrackClass(
            state.enabled
          )}`}
        >
          <span
            className={`inline-block h-4 w-4 rounded-full bg-white shadow transition-transform ${
              state.enabled ? "translate-x-6" : "translate-x-1"
            }`}
          />
        </button>
      </div>

      {/* Live Preview + Controls Grid */}
      <div className="grid gap-5 lg:grid-cols-[220px_1fr]">
        {/* Live Synced SP75 Speaker Illustration */}
        <div className="flex flex-col items-center justify-center rounded-2xl border border-[#E5E5E5] bg-[#F2F1F6] p-4 dark:border-white/10 dark:bg-[#141418]">
          <DeviceIllustration
            formFactor="speaker"
            deviceColor={deviceShellColorHex}
            theme={theme}
            leftConnected={connected}
            rightConnected={connected}
            rgbEnabled={state.enabled}
            rgbColor={state.colorHex}
            rgbMode={state.mode}
            width={192}
            height={144}
          />
          <span className="mt-2 inline-flex items-center gap-1.5 rounded-full bg-black/6 px-2.5 py-0.5 text-[11px] font-semibold text-[#4D4D4D] dark:bg-white/8 dark:text-white/75">
            <span
              className="h-2.5 w-2.5 rounded-full"
              style={{
                background:
                  !state.enabled
                    ? "#9CA3AF"
                    : state.mode === "gradient_breathing" || state.mode === "rhythm"
                      ? "linear-gradient(90deg,#FF7A2F,#FF2E93,#3399FF,#22C55E)"
                      : state.colorHex,
              }}
            />
            {state.enabled ? t(activeModeMeta.labelKey) : t("common.off")}
          </span>
        </div>

        {/* Mode Picker + Color Palette + Brightness & Speed */}
        <div className={`space-y-4 ${!state.enabled ? "pointer-events-none opacity-50" : ""}`}>
          {/* 4 Lighting Modes */}
          <div className="grid grid-cols-2 gap-2">
            {RGB_MODES.map(({ id, labelKey, Icon }) => {
              const selected = state.mode === id;
              return (
                <button
                  key={id}
                  type="button"
                  disabled={disabled || !state.enabled}
                  onClick={() => onChange({ mode: id })}
                  aria-pressed={selected}
                  className={`flex items-center gap-2 rounded-xl border p-3 text-left text-xs font-semibold transition ${
                    selected
                      ? "border-[#FF7A2F] bg-[#FF7A2F]/12 text-[#1A1A1A] dark:text-white"
                      : "border-[#E5E5E5] bg-[#F2F1F6]/70 text-[#4D4D4D] hover:border-[#FF7A2F]/50 dark:border-white/8 dark:bg-white/[0.04] dark:text-white/75"
                  }`}
                >
                  <span
                    className={`flex h-7 w-7 shrink-0 items-center justify-center rounded-lg ${
                      selected
                        ? "bg-[#FF7A2F] text-white"
                        : "bg-black/5 text-[#4D4D4D] dark:bg-white/10 dark:text-white/70"
                    }`}
                  >
                    <Icon className="h-4 w-4" />
                  </span>
                  <span>{t(labelKey)}</span>
                </button>
              );
            })}
          </div>

          {/* Preset Swatches + Custom Color Picker */}
          {activeModeMeta.supportsCustomColor && (
            <div className={`${QCY_SUBCARD_CLASS} space-y-2.5`}>
              <div className="flex items-center justify-between text-xs">
                <span className="inline-flex items-center gap-1.5 font-semibold text-[#1A1A1A] dark:text-white">
                  <Palette className="h-3.5 w-3.5 text-[#FF7A2F]" />
                  {t("lighting.color_presets")}
                </span>
                <span className="font-mono text-[11px] font-bold uppercase text-[#808080] dark:text-white/60">
                  {state.colorHex}
                </span>
              </div>

              <div className="flex flex-wrap items-center gap-2">
                {SP75_COLOR_PRESETS.map((hex) => {
                  const selected = state.colorHex.toUpperCase() === hex.toUpperCase();
                  return (
                    <button
                      key={hex}
                      type="button"
                      disabled={disabled || !state.enabled}
                      onClick={() => onChange({ colorHex: hex })}
                      aria-label={hex}
                      aria-pressed={selected}
                      className={`h-7 w-7 rounded-full border-2 transition ${
                        selected
                          ? "scale-110 border-[#FF7A2F] ring-2 ring-[#FF7A2F]/40"
                          : "border-black/15 hover:scale-105 dark:border-white/25"
                      }`}
                      style={{ backgroundColor: hex }}
                    />
                  );
                })}

                <label className="ml-auto inline-flex cursor-pointer items-center gap-1.5 rounded-lg border border-[#E5E5E5] bg-white px-2.5 py-1 text-xs font-semibold text-[#4D4D4D] dark:border-white/12 dark:bg-[#1E1E1E] dark:text-white/80">
                  <span>{t("lighting.custom_color")}</span>
                  <input
                    type="color"
                    disabled={disabled || !state.enabled}
                    value={state.colorHex}
                    onChange={(e) => onChange({ colorHex: e.target.value.toUpperCase() })}
                    className="h-5 w-6 cursor-pointer border-0 bg-transparent p-0"
                  />
                </label>
              </div>
            </div>
          )}

          {/* Brightness & Animation Speed Sliders */}
          <div className="grid gap-3 sm:grid-cols-2">
            <div className={`${QCY_SUBCARD_CLASS} space-y-1.5`}>
              <div className="flex items-center justify-between text-xs">
                <span className="inline-flex items-center gap-1.5 font-semibold text-[#1A1A1A] dark:text-white">
                  <Sun className="h-3.5 w-3.5 text-[#FF7A2F]" />
                  {t("lighting.brightness")}
                </span>
                <span className="font-mono font-bold text-[#FF7A2F]">{state.brightness}%</span>
              </div>
              <input
                type="range"
                min={0}
                max={100}
                step={1}
                disabled={disabled || !state.enabled}
                value={state.brightness}
                onChange={(e) => onChange({ brightness: Number(e.target.value) })}
                className="h-2 w-full cursor-pointer accent-[#FF7A2F]"
              />
            </div>

            <div className={`${QCY_SUBCARD_CLASS} space-y-1.5`}>
              <div className="flex items-center justify-between text-xs">
                <span className="inline-flex items-center gap-1.5 font-semibold text-[#1A1A1A] dark:text-white">
                  <Gauge className="h-3.5 w-3.5 text-[#0066FF] dark:text-[#3399FF]" />
                  {t("lighting.speed")}
                </span>
                <span className="font-mono font-bold text-[#0066FF] dark:text-[#3399FF]">
                  {state.speed}%
                </span>
              </div>
              <input
                type="range"
                min={1}
                max={100}
                step={1}
                disabled={disabled || !state.enabled || state.mode === "solid"}
                value={state.speed}
                onChange={(e) => onChange({ speed: Number(e.target.value) })}
                className="h-2 w-full cursor-pointer accent-[#0066FF] dark:accent-[#3399FF]"
              />
            </div>
          </div>
        </div>
      </div>

      {/* TWS Stereo Speaker Pairing (cmdID=52) */}
      {supportsTwsPair && (
        <div className={`${QCY_SUBCARD_CLASS} flex items-center justify-between gap-3`}>
          <div className="flex items-center gap-2.5">
            <Link2 className="h-4 w-4 text-[#0066FF] dark:text-[#3399FF]" />
            <div>
              <p className="text-sm font-semibold text-[#1A1A1A] dark:text-white">
                {t("connectivity.tws_speaker_pair")}
              </p>
              <p className="text-xs text-[#808080] dark:text-white/55">
                {t("connectivity.tws_speaker_pair_desc")}
              </p>
            </div>
          </div>
          <button
            type="button"
            role="switch"
            aria-checked={Boolean(state.twsSpeakerPairEnabled)}
            disabled={disabled}
            onClick={() => {
              const next = !state.twsSpeakerPairEnabled;
              onChange({ twsSpeakerPairEnabled: next });
              onTwsPairChange?.(next);
            }}
            className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors ${toggleTrackClass(
              Boolean(state.twsSpeakerPairEnabled),
              "blue"
            )}`}
          >
            <span
              className={`inline-block h-4 w-4 rounded-full bg-white shadow transition-transform ${
                state.twsSpeakerPairEnabled ? "translate-x-6" : "translate-x-1"
              }`}
            />
          </button>
        </div>
      )}
    </section>
  );
};
