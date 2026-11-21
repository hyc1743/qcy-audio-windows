import React, { useMemo, useState } from "react";
import {
  DEFAULT_BUILTIN_PRESETS,
  DEFAULT_TARGET_CURVES,
  DeviceDisconnectedState,
  HearingTestModal,
  ParametricEqCanvas,
  QCY_CARD_CLASS,
  type EqPresetOption,
} from "../components";
import {
  qcyFilterTypeFromIndex,
  qcyFilterTypeToIndex,
  type ParametricBand,
} from "../audio/biquadMath";
import { useI18n } from "../i18n";
import { useQcyStore } from "../store/useQcyStore";
import type { AppTheme, EqPresetItem, WireParametricBand } from "../types/qcy";

const SPEAKER_PRESETS: readonly EqPresetOption[] = [
  { id: "speaker_normal", eqIndex: 1, category: "speaker", name: "Normal", masterGainDb: 0, gains10BandDb: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0] },
  { id: "speaker_outdoor", eqIndex: 2, category: "speaker", name: "Outdoor", masterGainDb: -1.5, gains10BandDb: [-1, 0.5, 1.5, 1.5, 2, 2.5, 3, 3, 2, 1] },
  { id: "speaker_ambient", eqIndex: 3, category: "speaker", name: "Ambient", masterGainDb: -2, gains10BandDb: [3.5, 4, 2.5, 1, 0, -0.5, 0.5, 1, 1.5, 1] },
];

const EXTRA_OFFICIAL_PRESETS: readonly EqPresetOption[] = [
  { id: "adaptive_eq", eqIndex: 9, category: "official", name: "Adaptive EQ", masterGainDb: -1, gains10BandDb: [1.5, 1.5, 0.5, 0, 0, 0.5, 1, 1.5, 1.5, 1] },
  { id: "spatial", eqIndex: 10, category: "official", name: "Spatial Sound", masterGainDb: -2, gains10BandDb: [2, 1.5, 0, -1, -0.5, 0.5, 2, 3, 3.5, 2.5] },
  { id: "comfort_soundstage", eqIndex: 14, category: "official", name: "Comfort Soundstage", masterGainDb: -1, gains10BandDb: [1, 1.5, 0.5, 0, 0, 0.5, 0, -1.5, -3, -2] },
  { id: "soft_immersion", eqIndex: 15, category: "official", name: "Soft Immersion", masterGainDb: -1.5, gains10BandDb: [2, 2.5, 2, 1, 0, -0.5, 0, 0.5, -0.5, -1] },
];

function resolveUiTheme(theme: AppTheme | undefined): "light" | "dark" {
  if (theme === "light" || theme === "dark") return theme;
  if (typeof window !== "undefined" && typeof window.matchMedia === "function") {
    return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }
  return "dark";
}

function wireToParametric(bands: readonly WireParametricBand[]): ParametricBand[] {
  return bands.map((b) => ({
    frequencyHz: b.freq_hz,
    gainDb: b.gain_db,
    q: b.q,
    type: qcyFilterTypeFromIndex(b.irr_type),
    enabled: b.enabled,
  }));
}

function parametricToWire(bands: readonly ParametricBand[]): WireParametricBand[] {
  return bands.map((b) => ({
    freq_hz: b.frequencyHz,
    gain_db: b.gainDb,
    q: b.q,
    irr_type: qcyFilterTypeToIndex(b.type) ?? 2,
    enabled: b.enabled !== false,
  }));
}

export interface SoundEqPageProps {
  onNavigate?: (tab: string) => void;
}

export const SoundEqPage: React.FC<SoundEqPageProps> = ({ onNavigate }) => {
  const {
    snapshot,
    loading,
    applyEqPreset,
    setGraphicEq,
    setParametricEq,
    saveCustomEq,
    deleteCustomEq,
    applyPersonalizedEq,
    setAudioEnhancers,
  } = useQcyStore();
  const { locale, t } = useI18n();
  const [hearingTestOpen, setHearingTestOpen] = useState(false);

  const uiTheme = resolveUiTheme(snapshot?.settings.theme);

  const builtInPresets: EqPresetOption[] = useMemo(() => {
    const allKnown = [
      ...DEFAULT_BUILTIN_PRESETS,
      ...EXTRA_OFFICIAL_PRESETS,
      ...SPEAKER_PRESETS,
    ];
    if (!snapshot) return [...DEFAULT_BUILTIN_PRESETS];
    const modelIds = snapshot.active_model.default_eq_preset_ids;
    if (!modelIds || modelIds.length === 0) return allKnown;

    const allowed = new Set(modelIds);
    const filtered = allKnown.filter(
      (p) => p.category === "curated" || allowed.has(p.id)
    );
    return filtered.length > 0 ? filtered : [...DEFAULT_BUILTIN_PRESETS];
  }, [snapshot]);

  const customPresetOptions: EqPresetOption[] = useMemo(() => {
    if (!snapshot) return [];
    return snapshot.eq.custom_presets.map((p) => ({
      id: p.id,
      eqIndex: p.eq_index,
      category: "custom" as const,
      name: locale === "zh-CN" ? p.name_zh || p.name_en : p.name_en || p.name_zh,
      masterGainDb: p.master_gain_db,
      gains10BandDb: p.gains_10band_db,
      parametricBands: wireToParametric(p.parametric_bands),
    }));
  }, [snapshot, locale]);

  const localizedTargetCurves = useMemo(
    () =>
      DEFAULT_TARGET_CURVES.map((tc) => ({
        ...tc,
      })),
    []
  );

  if (loading && !snapshot) {
    return (
      <div className={`${QCY_CARD_CLASS} py-12 text-center text-sm text-[#808080] dark:text-white/55`}>
        {t("common.loading")}
      </div>
    );
  }

  if (!snapshot || snapshot.connection_state === "disconnected" || !snapshot.active_device) {
    return (
      <DeviceDisconnectedState
        variant="compact"
        featureLabel={t("app.nav.eq")}
        onGoToDevices={() => onNavigate?.("devices")}
        t={t}
      />
    );
  }

  const { eq, active_model: model, connection_state } = snapshot;
  const caps = model.capabilities;
  const isDisconnected = connection_state !== "connected";
  const channelBalance0To100 = Math.round(
    Math.max(0, Math.min(100, (eq.channel_balance + 1) * 50))
  );

  const handleSaveCustom = (opt: EqPresetOption) => {
    const item: EqPresetItem = {
      id: opt.id,
      eq_index: opt.eqIndex ?? 100,
      category: "custom",
      name_en: opt.name,
      name_zh: opt.name,
      master_gain_db: opt.masterGainDb,
      gains_10band_db: opt.gains10BandDb,
      parametric_bands: opt.parametricBands ? parametricToWire(opt.parametricBands) : [],
    };
    void saveCustomEq(item);
  };

  return (
    <div className="space-y-5">
      <ParametricEqCanvas
        capabilities={{
          parametricEq: caps.parametric_eq,
          customEq: caps.custom_eq,
          eqBands: caps.eq_bands,
          maxEqBands: caps.max_eq_bands,
          supportedFilterTypes: caps.supported_filter_types,
          personalizedEq: caps.personalized_eq,
          dynamicEq: caps.dynamic_eq,
          thxEq: caps.thx_eq,
          virtualBass: caps.virtual_bass,
          adaptiveVolume: caps.adaptive_volume,
          channelBalance: caps.channel_balance,
        }}
        activePresetId={eq.active_preset_id}
        presets={builtInPresets}
        customPresets={customPresetOptions}
        targetCurves={localizedTargetCurves}
        graphicGainsDb={eq.gains_10band_db}
        parametricBands={wireToParametric(eq.parametric_bands)}
        masterGainDb={eq.master_gain_db}
        auxState={{
          dynamicEq: eq.dynamic_eq,
          thxEq: eq.thx_eq,
          virtualBass: eq.virtual_bass,
          adaptiveVolume: eq.adaptive_volume,
          channelBalance: channelBalance0To100,
          personalizedEqEnabled: eq.personalized_enabled,
          personalizedLeftDb: eq.personalized_left_db,
          personalizedRightDb: eq.personalized_right_db,
        }}
        disabled={isDisconnected}
        theme={uiTheme}
        onSelectPreset={(preset) => {
          void applyEqPreset(preset.id);
        }}
        onGraphicGainsChange={(gains) => void setGraphicEq(gains, eq.master_gain_db)}
        onParametricBandsChange={(bands, master) =>
          void setParametricEq(parametricToWire(bands), master)
        }
        onSaveCustomPreset={handleSaveCustom}
        onDeleteCustomPreset={(id) => void deleteCustomEq(id)}
        onOpenHearingTest={() => setHearingTestOpen(true)}
        onPersonalizedToggle={(enabled) => {
          if (enabled && eq.personalized_left_db.length > 0) {
            void applyPersonalizedEq(
              eq.personalized_left_db,
              eq.personalized_right_db,
              eq.gains_10band_db,
              eq.channel_balance
            );
          } else {
            void applyEqPreset("default");
          }
        }}
        onAuxChange={(patch) => {
          void setAudioEnhancers({
            dynamic_eq: patch.dynamicEq,
            thx_eq: patch.thxEq,
            virtual_bass: patch.virtualBass,
            adaptive_volume: patch.adaptiveVolume,
            channel_balance:
              patch.channelBalance !== undefined
                ? Math.round(((patch.channelBalance - 50) / 50) * 100) / 100
                : undefined,
          });
        }}
        t={t}
      />

      <HearingTestModal
        open={hearingTestOpen}
        theme={uiTheme}
        onApply={(profile) => {
          void applyPersonalizedEq(
            profile.leftCompensationDb,
            profile.rightCompensationDb,
            profile.tenBandGainsDb,
            profile.suggestedBalance
          );
        }}
        onClose={() => setHearingTestOpen(false)}
        t={t}
      />
    </div>
  );
};
