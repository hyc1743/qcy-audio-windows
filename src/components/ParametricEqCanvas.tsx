import React, { useMemo, useRef, useState } from "react";
import {
  Activity,
  Download,
  Ear,
  Plus,
  RotateCcw,
  Save,
  Sliders,
  Sparkles,
  Trash2,
  Upload,
  Volume2,
} from "lucide-react";
import {
  BiquadFilterType,
  MAX_EQ_FREQ_HZ,
  MIN_EQ_FREQ_HZ,
  ParametricBand,
  evaluateBandMagnitudeDb,
  evaluateCompositeCurve,
  generateLogFrequencyGrid,
  graphicBandsToParametric,
  qcyFilterTypeFromIndex,
  qcyFilterTypeToIndex,
} from "../audio/biquadMath";
import {
  QCY_CARD_CLASS,
  QCY_SUBCARD_CLASS,
  TranslateFn,
  UiTheme,
  toggleTrackClass,
} from "./shared";

export interface EqPresetOption {
  id: string;
  eqIndex?: number | null;
  category: "official" | "speaker" | "personalized" | "curated" | "custom";
  name: string;
  subtitle?: string;
  masterGainDb: number;
  gains10BandDb: number[];
  parametricBands?: ParametricBand[];
}

export interface TargetCurveOption {
  id: string;
  name: string;
  points: Array<{ freqHz: number; db: number }>;
}

export interface EqCapabilities {
  parametricEq: boolean;
  customEq: boolean;
  eqBands?: number;
  maxEqBands?: number;
  supportedFilterTypes?: number[];
  personalizedEq?: boolean;
  dynamicEq?: boolean;
  thxEq?: boolean;
  virtualBass?: boolean;
  adaptiveVolume?: boolean;
  channelBalance?: boolean;
}

export interface EqAuxState {
  dynamicEq: boolean;
  thxEq: boolean;
  virtualBass: boolean;
  adaptiveVolume: boolean;
  /** `0..100` where `50` is center balance (`<50` favors Left, `>50` favors Right). */
  channelBalance: number;
  personalizedEqEnabled?: boolean;
  personalizedLeftDb?: number[];
  personalizedRightDb?: number[];
}

export interface ParametricEqCanvasProps {
  capabilities: EqCapabilities;
  activePresetId: string;
  presets?: EqPresetOption[];
  customPresets?: EqPresetOption[];
  targetCurves?: TargetCurveOption[];
  /** Current 10-band graphic EQ gains (`-12..+12` dB). */
  graphicGainsDb: number[];
  /** Current parametric IIR bands (used when `parametricEq` is supported). */
  parametricBands: ParametricBand[];
  masterGainDb?: number;
  auxState?: EqAuxState;
  disabled?: boolean;
  theme?: UiTheme;
  onSelectPreset: (preset: EqPresetOption) => void;
  onGraphicGainsChange: (gainsDb: number[]) => void;
  onParametricBandsChange: (bands: ParametricBand[], masterGainDb: number) => void;
  onSaveCustomPreset?: (preset: EqPresetOption) => void;
  onDeleteCustomPreset?: (presetId: string) => void;
  onOpenHearingTest?: () => void;
  onPersonalizedToggle?: (enabled: boolean) => void;
  onAuxChange?: (patch: Partial<EqAuxState>) => void;
  t: TranslateFn;
  className?: string;
}

export const STANDARD_10_BAND_HZ: readonly number[] = [
  31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000,
];

export const QCY_IIR_FILTERS: ReadonlyArray<{
  index: number;
  type: BiquadFilterType;
  i18nKey: string;
  supportsGain: boolean;
  supportsQ: boolean;
}> = [
  { index: 0, type: "high_pass", i18nKey: "eq.filters.high_pass", supportsGain: false, supportsQ: true },
  { index: 1, type: "low_pass", i18nKey: "eq.filters.low_pass", supportsGain: false, supportsQ: true },
  { index: 2, type: "peaking", i18nKey: "eq.filters.peak", supportsGain: true, supportsQ: true },
  { index: 3, type: "high_shelf", i18nKey: "eq.filters.high_shelf", supportsGain: true, supportsQ: false },
  { index: 4, type: "low_shelf", i18nKey: "eq.filters.low_shelf", supportsGain: true, supportsQ: false },
  { index: 5, type: "high_shelf_q", i18nKey: "eq.filters.high_shelf_q", supportsGain: true, supportsQ: true },
  { index: 6, type: "low_shelf_q", i18nKey: "eq.filters.low_shelf_q", supportsGain: true, supportsQ: true },
  { index: 7, type: "high_pass_advance", i18nKey: "eq.filters.high_pass_advance", supportsGain: false, supportsQ: true },
  { index: 8, type: "low_pass_advance", i18nKey: "eq.filters.low_pass_advance", supportsGain: false, supportsQ: true },
];

export const DEFAULT_TARGET_CURVES: readonly TargetCurveOption[] = [
  {
    id: "flat_reference",
    name: "Flat Reference (0 dB)",
    points: [
      { freqHz: 20, db: 0 },
      { freqHz: 20000, db: 0 },
    ],
  },
  {
    id: "harman_ie_2019",
    name: "In-Ear Target (Harman-Style)",
    points: [
      { freqHz: 20, db: 4.8 },
      { freqHz: 40, db: 5.0 },
      { freqHz: 80, db: 3.8 },
      { freqHz: 160, db: 1.2 },
      { freqHz: 300, db: 0.0 },
      { freqHz: 1000, db: 0.5 },
      { freqHz: 2000, db: 3.5 },
      { freqHz: 3000, db: 5.5 },
      { freqHz: 5000, db: 3.2 },
      { freqHz: 8000, db: 1.0 },
      { freqHz: 16000, db: -3.5 },
      { freqHz: 20000, db: -5.0 },
    ],
  },
  {
    id: "harman_oe_2018",
    name: "Over-Ear Target (Harman-Style)",
    points: [
      { freqHz: 20, db: 3.5 },
      { freqHz: 40, db: 4.0 },
      { freqHz: 80, db: 3.0 },
      { freqHz: 300, db: 0.0 },
      { freqHz: 1000, db: 0.0 },
      { freqHz: 2000, db: 2.2 },
      { freqHz: 3200, db: 4.2 },
      { freqHz: 5500, db: 2.0 },
      { freqHz: 8000, db: 0.5 },
      { freqHz: 16000, db: -2.5 },
      { freqHz: 20000, db: -4.0 },
    ],
  },
  {
    id: "diffuse_field_tilt",
    name: "Studio Diffuse-Field",
    points: [
      { freqHz: 20, db: 2.5 },
      { freqHz: 60, db: 2.0 },
      { freqHz: 200, db: 1.0 },
      { freqHz: 1000, db: 0.0 },
      { freqHz: 2500, db: 1.8 },
      { freqHz: 8000, db: -0.5 },
      { freqHz: 20000, db: -3.5 },
    ],
  },
  {
    id: "warm_bass_target",
    name: "Warm Bass Preference",
    points: [
      { freqHz: 20, db: 6.0 },
      { freqHz: 50, db: 6.2 },
      { freqHz: 120, db: 4.0 },
      { freqHz: 500, db: 0.0 },
      { freqHz: 2500, db: 1.8 },
      { freqHz: 8000, db: -1.5 },
      { freqHz: 20000, db: -4.5 },
    ],
  },
];

export const DEFAULT_BUILTIN_PRESETS: readonly EqPresetOption[] = [
  { id: "default", eqIndex: 1, category: "official", name: "Default", masterGainDb: 0, gains10BandDb: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0] },
  { id: "pop", eqIndex: 2, category: "official", name: "Pop", masterGainDb: -1.5, gains10BandDb: [1.5, 2.5, 1.5, 0, -0.5, 0.5, 2, 2.5, 2, 1] },
  { id: "heavy_bass", eqIndex: 3, category: "official", name: "Heavy Bass", masterGainDb: -3, gains10BandDb: [5.5, 6, 4.5, 2, 0, -0.5, 0, 0.5, 1, 0.5] },
  { id: "rock", eqIndex: 4, category: "official", name: "Rock", masterGainDb: -2, gains10BandDb: [3.5, 4, 2, -0.5, -1.5, 0.5, 2.5, 3.5, 3, 1.5] },
  { id: "soft", eqIndex: 5, category: "official", name: "Soft", masterGainDb: -0.5, gains10BandDb: [-1, 0, 1, 1.5, 1, 0.5, -0.5, -1.5, -2.5, -3] },
  { id: "classical", eqIndex: 6, category: "official", name: "Classical", masterGainDb: -1.5, gains10BandDb: [2, 1.5, 0.5, -0.5, 0, 0.5, 1.5, 2.5, 3.5, 2.5] },
  { id: "bass_boost", eqIndex: 7, category: "official", name: "Bass Boost", masterGainDb: -2.5, gains10BandDb: [4.5, 5, 3, 0.5, 0, 0, 0.5, 0.5, 0.5, 0] },
  { id: "audiobook", eqIndex: 8, category: "official", name: "Audiobook", masterGainDb: -1, gains10BandDb: [-4, -3.5, -2, 0.5, 2.5, 3.5, 3, 1, -1.5, -3] },
  { id: "pure_vocal", eqIndex: 17, category: "official", name: "Pure Vocal", masterGainDb: -1.5, gains10BandDb: [-1.5, -1, -0.5, 1, 2.5, 3.5, 3, 1.5, 0, -1] },
  { id: "original_boost", eqIndex: 16, category: "official", name: "Original Acoustics", masterGainDb: -1, gains10BandDb: [0.5, 0.5, 0, 0, 0.5, 1, 1.5, 2, 2.5, 2] },
  { id: "gaming", eqIndex: 13, category: "official", name: "Gaming", masterGainDb: -2, gains10BandDb: [1, 0.5, -1.5, -1, 0.5, 1.5, 3.5, 4.5, 3, 1] },
  { id: "studio_master", eqIndex: 19, category: "official", name: "Studio Master", masterGainDb: -1, gains10BandDb: [1, 0.5, 0, -0.5, 0, 0.5, 1, 1, 1.5, 1] },
  { id: "podcast_voice", eqIndex: null, category: "curated", name: "Podcast & Meetings", masterGainDb: -1, gains10BandDb: [-3.5, -2.5, -1.5, 0, 2, 3, 2.5, 1, -1, -2] },
  { id: "late_night", eqIndex: null, category: "curated", name: "Late Night", masterGainDb: -1.5, gains10BandDb: [2.5, 2, 1, 0, -0.5, -0.5, 0, 0.5, 1.5, 1] },
];

const SVG_W = 680;
const SVG_H = 240;
const PAD_L = 42;
const PAD_R = 16;
const PAD_T = 16;
const PAD_B = 28;
const PLOT_W = SVG_W - PAD_L - PAD_R;
const PLOT_H = SVG_H - PAD_T - PAD_B;
const DB_MIN = -14;
const DB_MAX = 14;
const LOG_MIN = Math.log10(MIN_EQ_FREQ_HZ);
const LOG_MAX = Math.log10(MAX_EQ_FREQ_HZ);

export function freqToSvgX(freqHz: number): number {
  const clamped = Math.max(MIN_EQ_FREQ_HZ, Math.min(MAX_EQ_FREQ_HZ, freqHz));
  const norm = (Math.log10(clamped) - LOG_MIN) / (LOG_MAX - LOG_MIN);
  return PAD_L + norm * PLOT_W;
}

export function svgXToFreq(x: number): number {
  const norm = Math.max(0, Math.min(1, (x - PAD_L) / PLOT_W));
  return Math.round(Math.pow(10, LOG_MIN + norm * (LOG_MAX - LOG_MIN)));
}

export function dbToSvgY(db: number): number {
  const clamped = Math.max(DB_MIN, Math.min(DB_MAX, db));
  const norm = (DB_MAX - clamped) / (DB_MAX - DB_MIN);
  return PAD_T + norm * PLOT_H;
}

export function svgYToDb(y: number): number {
  const norm = Math.max(0, Math.min(1, (y - PAD_T) / PLOT_H));
  const raw = DB_MAX - norm * (DB_MAX - DB_MIN);
  return Math.round(Math.max(-12, Math.min(12, raw)) * 10) / 10;
}

function formatFreqLabel(hz: number): string {
  return hz >= 1000 ? `${hz % 1000 === 0 ? hz / 1000 : (hz / 1000).toFixed(1)}k` : `${hz}`;
}

/**
 * Pure parser for standard AutoEQ ParametricEQ `.txt` files (`Preamp:` + `Filter N: ON PK Fc ... Gain ... Q ...`).
 */
export function parseAutoEqText(content: string): {
  masterGainDb: number;
  bands: ParametricBand[];
} | null {
  const lines = content.split(/\r?\n/);
  let masterGainDb = 0;
  const bands: ParametricBand[] = [];

  for (const line of lines) {
    const preampMatch = line.match(/Preamp:\s*(-?\d+(?:\.\d+)?)\s*dB/i);
    if (preampMatch) {
      masterGainDb = Math.max(-12, Math.min(12, parseFloat(preampMatch[1])));
      continue;
    }
    const filterMatch = line.match(
      /Filter\s+\d+:\s+(ON|OFF)\s+(PK|LSC|HSC|LS|HS|LP|HP|LPQ|HPQ)\s+Fc\s+(\d+(?:\.\d+)?)\s*Hz(?:\s+Gain\s+(-?\d+(?:\.\d+)?)\s*dB)?(?:\s+Q\s+(\d+(?:\.\d+)?))?/i
    );
    if (filterMatch) {
      const enabled = filterMatch[1].toUpperCase() === "ON";
      const code = filterMatch[2].toUpperCase();
      const freqHz = Math.max(20, Math.min(20000, Math.round(parseFloat(filterMatch[3]))));
      const gainDb = filterMatch[4]
        ? Math.max(-12, Math.min(12, parseFloat(filterMatch[4])))
        : 0;
      const q = filterMatch[5] ? Math.max(0.2, Math.min(10, parseFloat(filterMatch[5]))) : 1.0;

      let type: BiquadFilterType = "peaking";
      if (code === "LSC" || code === "LS") type = "low_shelf";
      else if (code === "HSC" || code === "HS") type = "high_shelf";
      else if (code === "HP" || code === "HPQ") type = "high_pass";
      else if (code === "LP" || code === "LPQ") type = "low_pass";

      bands.push({ frequencyHz: freqHz, gainDb, q, type, enabled });
    }
  }

  return bands.length > 0 ? { masterGainDb, bands } : null;
}

const GRID_FREQS = [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000];
const GRID_DBS = [12, 6, 0, -6, -12];

export const ParametricEqCanvas: React.FC<ParametricEqCanvasProps> = ({
  capabilities,
  activePresetId,
  presets = DEFAULT_BUILTIN_PRESETS as EqPresetOption[],
  customPresets = [],
  targetCurves = DEFAULT_TARGET_CURVES as TargetCurveOption[],
  graphicGainsDb,
  parametricBands,
  masterGainDb = 0,
  auxState,
  disabled = false,
  theme = "dark",
  onSelectPreset,
  onGraphicGainsChange,
  onParametricBandsChange,
  onSaveCustomPreset,
  onDeleteCustomPreset,
  onOpenHearingTest,
  onPersonalizedToggle,
  onAuxChange,
  t,
  className = "",
}) => {
  const [editorMode, setEditorMode] = useState<"graphic" | "parametric">(
    capabilities.parametricEq ? "parametric" : "graphic"
  );
  const [presetTab, setPresetTab] = useState<"official" | "curated" | "custom">("official");
  const [selectedBandIdx, setSelectedBandIdx] = useState<number>(0);
  const [draggingIdx, setDraggingIdx] = useState<number | null>(null);
  const [targetCurveId, setTargetCurveId] = useState<string>("none");
  const [saveName, setSaveName] = useState<string>("");
  const [showSaveBar, setShowSaveBar] = useState<boolean>(false);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);

  const svgRef = useRef<SVGSVGElement | null>(null);
  const fileInputRef = useRef<HTMLInputElement | null>(null);

  const isParametric = editorMode === "parametric" && capabilities.parametricEq;
  const maxBands = capabilities.maxEqBands ?? (capabilities.parametricEq ? 20 : 10);
  const allowedFilterIndices = useMemo(
    () =>
      new Set(
        capabilities.supportedFilterTypes && capabilities.supportedFilterTypes.length > 0
          ? capabilities.supportedFilterTypes
          : [0, 1, 2, 3, 4, 5, 6, 7, 8]
      ),
    [capabilities.supportedFilterTypes]
  );

  const normalizedGraphicGains = useMemo(
    () => STANDARD_10_BAND_HZ.map((_, i) => graphicGainsDb[i] ?? 0),
    [graphicGainsDb]
  );

  const activeBands: ParametricBand[] = useMemo(() => {
    if (isParametric) {
      return parametricBands.length > 0
        ? parametricBands
        : graphicBandsToParametric(STANDARD_10_BAND_HZ, normalizedGraphicGains, 1.41);
    }
    return graphicBandsToParametric(STANDARD_10_BAND_HZ, normalizedGraphicGains, 1.41);
  }, [isParametric, parametricBands, normalizedGraphicGains]);

  const evalFreqs = useMemo(() => generateLogFrequencyGrid(180), []);
  const compositePoints = useMemo(
    () => evaluateCompositeCurve(activeBands, evalFreqs, isParametric ? masterGainDb : 0),
    [activeBands, evalFreqs, isParametric, masterGainDb]
  );

  const safeSelectedIdx = Math.max(0, Math.min(activeBands.length - 1, selectedBandIdx));
  const selectedBand = activeBands[safeSelectedIdx];

  const selectedBandPath = useMemo(() => {
    if (!selectedBand || selectedBand.enabled === false) return "";
    return evalFreqs
      .map((f, i) => {
        const db = evaluateBandMagnitudeDb(selectedBand, f);
        return `${i === 0 ? "M" : "L"}${freqToSvgX(f).toFixed(1)},${dbToSvgY(db).toFixed(1)}`;
      })
      .join(" ");
  }, [selectedBand, evalFreqs]);

  const compositeLinePath = useMemo(
    () =>
      compositePoints
        .map(
          (pt, i) =>
            `${i === 0 ? "M" : "L"}${freqToSvgX(pt.frequencyHz).toFixed(1)},${dbToSvgY(
              pt.gainDb
            ).toFixed(1)}`
        )
        .join(" "),
    [compositePoints]
  );

  const compositeAreaPath = useMemo(() => {
    if (!compositeLinePath) return "";
    const zeroY = dbToSvgY(0).toFixed(1);
    const startX = freqToSvgX(MIN_EQ_FREQ_HZ).toFixed(1);
    const endX = freqToSvgX(MAX_EQ_FREQ_HZ).toFixed(1);
    return `${compositeLinePath} L${endX},${zeroY} L${startX},${zeroY} Z`;
  }, [compositeLinePath]);

  const selectedTargetCurve = useMemo(
    () => targetCurves.find((c) => c.id === targetCurveId) ?? null,
    [targetCurves, targetCurveId]
  );

  const targetCurvePath = useMemo(() => {
    if (!selectedTargetCurve || selectedTargetCurve.points.length < 2) return "";
    return selectedTargetCurve.points
      .map(
        (pt, i) =>
          `${i === 0 ? "M" : "L"}${freqToSvgX(pt.freqHz).toFixed(1)},${dbToSvgY(pt.db).toFixed(1)}`
      )
      .join(" ");
  }, [selectedTargetCurve]);

  const updateGraphicBand = (index: number, nextGain: number) => {
    if (disabled || !capabilities.customEq) return;
    const clamped = Math.round(Math.max(-12, Math.min(12, nextGain)) * 10) / 10;
    const next = [...normalizedGraphicGains];
    next[index] = clamped;
    onGraphicGainsChange(next);
  };

  const updateParametricBand = (index: number, patch: Partial<ParametricBand>) => {
    if (disabled || !capabilities.customEq) return;
    const next = activeBands.map((b, i) => (i === index ? { ...b, ...patch } : b));
    onParametricBandsChange(next, masterGainDb);
  };

  const handleSvgPointerMove = (e: React.PointerEvent<SVGSVGElement>) => {
    if (draggingIdx === null || !svgRef.current || disabled || !capabilities.customEq) return;
    const rect = svgRef.current.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) return;
    const svgX = ((e.clientX - rect.left) / rect.width) * SVG_W;
    const svgY = ((e.clientY - rect.top) / rect.height) * SVG_H;
    const nextGain = svgYToDb(svgY);

    if (isParametric) {
      const nextFreq = svgXToFreq(svgX);
      updateParametricBand(draggingIdx, { frequencyHz: nextFreq, gainDb: nextGain });
    } else {
      updateGraphicBand(draggingIdx, nextGain);
    }
  };

  const handleAddBand = () => {
    if (!isParametric || activeBands.length >= maxBands || disabled) return;
    const next: ParametricBand[] = [
      ...activeBands,
      { frequencyHz: 1000, gainDb: 0, q: 1.4, type: "peaking", enabled: true },
    ];
    onParametricBandsChange(next, masterGainDb);
    setSelectedBandIdx(next.length - 1);
  };

  const handleRemoveBand = (idx: number) => {
    if (!isParametric || activeBands.length <= 1 || disabled) return;
    const next = activeBands.filter((_, i) => i !== idx);
    onParametricBandsChange(next, masterGainDb);
    setSelectedBandIdx(Math.max(0, idx - 1));
  };

  const handleResetFlat = () => {
    if (disabled || !capabilities.customEq) return;
    const flat = STANDARD_10_BAND_HZ.map(() => 0);
    onGraphicGainsChange(flat);
    if (capabilities.parametricEq) {
      onParametricBandsChange(graphicBandsToParametric(STANDARD_10_BAND_HZ, flat, 1.41), 0);
    }
  };

  const handleExportJson = () => {
    const payload = {
      schema: "qcy-audio-eq-v1",
      name: saveName.trim() || activePresetId,
      mode: isParametric ? "parametric" : "graphic",
      master_gain_db: masterGainDb,
      gains_10band_db: normalizedGraphicGains,
      parametric_bands: activeBands.map((b) => ({
        freq_hz: b.frequencyHz,
        gain_db: b.gainDb,
        q: b.q,
        filter_type: qcyFilterTypeToIndex(b.type) ?? 2,
        enabled: b.enabled !== false,
      })),
    };
    const json = JSON.stringify(payload, null, 2);
    if (typeof Blob !== "undefined" && typeof URL !== "undefined" && typeof document !== "undefined") {
      const blob = new Blob([json], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `qcy-eq-${(payload.name || "custom").replace(/[^a-zA-Z0-9_-]/g, "_")}.json`;
      a.click();
      URL.revokeObjectURL(url);
    }
    setStatusMessage(t("eq.export_success"));
    setTimeout(() => setStatusMessage(null), 2500);
  };

  const handleImportFile = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    e.target.value = "";
    if (!file) return;
    try {
      const text = await file.text();
      if (file.name.toLowerCase().endsWith(".txt")) {
        const parsed = parseAutoEqText(text);
        if (!parsed) throw new Error("invalid autoeq");
        setEditorMode("parametric");
        onParametricBandsChange(parsed.bands.slice(0, maxBands), parsed.masterGainDb);
        setStatusMessage(t("eq.import_success", { name: file.name }));
      } else {
        const data = JSON.parse(text) as {
          name?: string;
          master_gain_db?: number;
          gains_10band_db?: number[];
          parametric_bands?: Array<{
            freq_hz: number;
            gain_db: number;
            q: number;
            filter_type?: number;
            enabled?: boolean;
          }>;
        };
        if (Array.isArray(data.gains_10band_db) && data.gains_10band_db.length >= 10) {
          onGraphicGainsChange(data.gains_10band_db.slice(0, 10));
        }
        if (Array.isArray(data.parametric_bands) && data.parametric_bands.length > 0) {
          const bands: ParametricBand[] = data.parametric_bands.slice(0, maxBands).map((b) => ({
            frequencyHz: b.freq_hz,
            gainDb: b.gain_db,
            q: b.q,
            type: qcyFilterTypeFromIndex(b.filter_type ?? 2),
            enabled: b.enabled !== false,
          }));
          onParametricBandsChange(bands, data.master_gain_db ?? 0);
        }
        setStatusMessage(t("eq.import_success", { name: data.name ?? file.name }));
      }
    } catch {
      setStatusMessage(t("eq.import_error"));
    }
    setTimeout(() => setStatusMessage(null), 3000);
  };

  const handleSaveCustom = () => {
    const trimmed = saveName.trim();
    if (!trimmed || !onSaveCustomPreset) return;
    onSaveCustomPreset({
      id: `custom-${Date.now()}`,
      eqIndex: null,
      category: "custom",
      name: trimmed,
      masterGainDb,
      gains10BandDb: [...normalizedGraphicGains],
      parametricBands: activeBands.map((b) => ({ ...b })),
    });
    setSaveName("");
    setShowSaveBar(false);
    setPresetTab("custom");
  };

  const visiblePresets = useMemo(() => {
    if (presetTab === "custom") return customPresets;
    if (presetTab === "curated") return presets.filter((p) => p.category === "curated");
    return presets.filter((p) => p.category === "official" || p.category === "speaker");
  }, [presetTab, presets, customPresets]);

  const curveStroke = theme === "dark" ? "#3399FF" : "#0066FF";
  const gridStroke = theme === "dark" ? "rgba(255,255,255,0.09)" : "rgba(26,26,26,0.08)";
  const zeroStroke = theme === "dark" ? "rgba(255,255,255,0.22)" : "rgba(26,26,26,0.20)";
  const axisLabelColor = theme === "dark" ? "rgba(255,255,255,0.5)" : "#808080";

  return (
    <section className={`${QCY_CARD_CLASS} space-y-5 ${className}`} aria-label={t("eq.title")}>
      {/* Header Row */}
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-2.5">
          <span className="flex h-9 w-9 items-center justify-center rounded-xl bg-[#0066FF]/12 text-[#0066FF] dark:bg-[#3399FF]/15 dark:text-[#3399FF]">
            <Sliders className="h-5 w-5" />
          </span>
          <div>
            <h3 className="text-base font-bold text-[#1A1A1A] dark:text-white">
              {t("eq.title")}
            </h3>
            <p className="text-xs text-[#808080] dark:text-white/55">
              {t("eq.freq_range_hint")} · {t("eq.gain_range_hint")}
            </p>
          </div>
        </div>

        {/* Mode Toggle + Utility Actions */}
        <div className="flex flex-wrap items-center gap-2">
          {capabilities.parametricEq && capabilities.customEq && (
            <div className="inline-flex rounded-xl bg-[#F2F1F6] p-1 dark:bg-white/[0.06]">
              <button
                type="button"
                onClick={() => setEditorMode("graphic")}
                className={`rounded-lg px-2.5 py-1 text-xs font-semibold transition ${
                  !isParametric
                    ? "bg-[#0066FF] text-white dark:bg-[#3399FF]"
                    : "text-[#4D4D4D] dark:text-white/70"
                }`}
              >
                {t("eq.mode_graphic")}
              </button>
              <button
                type="button"
                onClick={() => setEditorMode("parametric")}
                className={`rounded-lg px-2.5 py-1 text-xs font-semibold transition ${
                  isParametric
                    ? "bg-[#0066FF] text-white dark:bg-[#3399FF]"
                    : "text-[#4D4D4D] dark:text-white/70"
                }`}
              >
                {t("eq.mode_parametric")}
              </button>
            </div>
          )}

          {capabilities.customEq && (
            <>
              <input
                ref={fileInputRef}
                type="file"
                accept=".json,.txt"
                onChange={handleImportFile}
                className="hidden"
              />
              <button
                type="button"
                disabled={disabled}
                onClick={() => fileInputRef.current?.click()}
                className="inline-flex items-center gap-1 rounded-lg border border-[#E5E5E5] px-2.5 py-1.5 text-xs font-semibold text-[#4D4D4D] hover:border-[#0066FF] dark:border-white/12 dark:text-white/75"
              >
                <Upload className="h-3.5 w-3.5" />
                {t("eq.import_json")}
              </button>
              <button
                type="button"
                disabled={disabled}
                onClick={handleExportJson}
                className="inline-flex items-center gap-1 rounded-lg border border-[#E5E5E5] px-2.5 py-1.5 text-xs font-semibold text-[#4D4D4D] hover:border-[#0066FF] dark:border-white/12 dark:text-white/75"
              >
                <Download className="h-3.5 w-3.5" />
                {t("eq.export_json")}
              </button>
              <button
                type="button"
                disabled={disabled}
                onClick={handleResetFlat}
                className="inline-flex items-center gap-1 rounded-lg border border-[#E5E5E5] px-2.5 py-1.5 text-xs font-semibold text-[#4D4D4D] hover:border-[#FF7A2F] dark:border-white/12 dark:text-white/75"
              >
                <RotateCcw className="h-3.5 w-3.5" />
                {t("eq.reset_flat")}
              </button>
            </>
          )}
        </div>
      </div>

      {statusMessage && (
        <div className="rounded-xl border border-[#0066FF]/30 bg-[#0066FF]/10 px-3 py-2 text-xs font-medium text-[#0066FF] dark:text-[#66B2FF]">
          {statusMessage}
        </div>
      )}

      {/* Interactive 20 Hz – 20 kHz Logarithmic Frequency Response SVG */}
      <div className="rounded-2xl border border-[#E5E5E5] bg-[#F8F8FB] p-3 dark:border-white/10 dark:bg-[#141418]">
        <div className="mb-2 flex flex-wrap items-center justify-between gap-2 text-xs">
          <div className="flex flex-wrap items-center gap-3">
            <span className="inline-flex items-center gap-1.5 font-semibold text-[#1A1A1A] dark:text-white">
              <span className="h-2 w-4 rounded-full" style={{ backgroundColor: curveStroke }} />
              {t("eq.curve_combined")}
            </span>
            {selectedTargetCurve && (
              <span className="inline-flex items-center gap-1.5 font-medium text-[#22C55E]">
                <span className="h-0.5 w-4 border-t-2 border-dashed border-[#22C55E]" />
                {t("eq.curve_target")}
              </span>
            )}
          </div>

          <label className="inline-flex items-center gap-1.5 text-xs text-[#4D4D4D] dark:text-white/70">
            <span>{t("eq.target_curve_select")}:</span>
            <select
              value={targetCurveId}
              onChange={(e) => setTargetCurveId(e.target.value)}
              className="rounded-lg border border-[#E5E5E5] bg-white px-2 py-1 text-xs font-medium text-[#1A1A1A] outline-none dark:border-white/12 dark:bg-[#1E1E1E] dark:text-white"
            >
              <option value="none">{t("eq.target_curve_none")}</option>
              {targetCurves.map((tc) => (
                <option key={tc.id} value={tc.id}>
                  {tc.name}
                </option>
              ))}
            </select>
          </label>
        </div>

        <svg
          ref={svgRef}
          viewBox={`0 0 ${SVG_W} ${SVG_H}`}
          className="h-56 w-full touch-none select-none"
          role="img"
          aria-label={t("eq.chart_title")}
          onPointerMove={handleSvgPointerMove}
          onPointerUp={() => setDraggingIdx(null)}
          onPointerLeave={() => setDraggingIdx(null)}
        >
          <defs>
            <linearGradient id="qcy-eq-area-grad" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor={curveStroke} stopOpacity="0.28" />
              <stop offset="100%" stopColor={curveStroke} stopOpacity="0.02" />
            </linearGradient>
          </defs>

          {/* Horizontal dB gridlines */}
          {GRID_DBS.map((db) => {
            const y = dbToSvgY(db);
            return (
              <g key={db}>
                <line
                  x1={PAD_L}
                  y1={y}
                  x2={SVG_W - PAD_R}
                  y2={y}
                  stroke={db === 0 ? zeroStroke : gridStroke}
                  strokeWidth={db === 0 ? 1.3 : 1}
                  strokeDasharray={db === 0 ? undefined : "3 3"}
                />
                <text
                  x={PAD_L - 6}
                  y={y + 3.5}
                  textAnchor="end"
                  fontSize="10"
                  fill={axisLabelColor}
                >
                  {db > 0 ? `+${db}` : db}
                </text>
              </g>
            );
          })}

          {/* Vertical log-frequency gridlines */}
          {GRID_FREQS.map((freq) => {
            const x = freqToSvgX(freq);
            return (
              <g key={freq}>
                <line
                  x1={x}
                  y1={PAD_T}
                  x2={x}
                  y2={SVG_H - PAD_B}
                  stroke={gridStroke}
                  strokeWidth="1"
                />
                <text
                  x={x}
                  y={SVG_H - 8}
                  textAnchor="middle"
                  fontSize="10"
                  fill={axisLabelColor}
                >
                  {formatFreqLabel(freq)}
                </text>
              </g>
            );
          })}

          {/* Optional Target FR Curve Overlay */}
          {targetCurvePath && (
            <path
              d={targetCurvePath}
              fill="none"
              stroke="#22C55E"
              strokeWidth="1.8"
              strokeDasharray="6 4"
              opacity="0.85"
            />
          )}

          {/* Selected Individual Band Curve */}
          {selectedBandPath && (
            <path
              d={selectedBandPath}
              fill="none"
              stroke="#FF7A2F"
              strokeWidth="1.4"
              strokeDasharray="4 3"
              opacity="0.7"
            />
          )}

          {/* Composite EQ Fill & Stroke */}
          {compositeAreaPath && <path d={compositeAreaPath} fill="url(#qcy-eq-area-grad)" />}
          {compositeLinePath && (
            <path
              d={compositeLinePath}
              fill="none"
              stroke={curveStroke}
              strokeWidth="2.5"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          )}

          {/* Draggable Control Nodes */}
          {capabilities.customEq &&
            activeBands.map((band, idx) => {
              const cx = freqToSvgX(band.frequencyHz);
              const cy = dbToSvgY(band.gainDb);
              const isSelected = idx === safeSelectedIdx;
              const isMuted = band.enabled === false;
              return (
                <g
                  key={idx}
                  className={disabled ? "cursor-not-allowed" : "cursor-grab active:cursor-grabbing"}
                  onPointerDown={(e) => {
                    if (disabled) return;
                    e.currentTarget.setPointerCapture?.(e.pointerId);
                    setSelectedBandIdx(idx);
                    setDraggingIdx(idx);
                  }}
                >
                  {isSelected && (
                    <circle
                      cx={cx}
                      cy={cy}
                      r="11"
                      fill="#FF7A2F"
                      opacity="0.22"
                    />
                  )}
                  <circle
                    cx={cx}
                    cy={cy}
                    r={isSelected ? "6.5" : "5"}
                    fill={isMuted ? "#9CA3AF" : isSelected ? "#FF7A2F" : curveStroke}
                    stroke="#FFFFFF"
                    strokeWidth="1.8"
                  />
                  <text
                    x={cx}
                    y={cy - 10}
                    textAnchor="middle"
                    fontSize="9.5"
                    fontWeight="700"
                    fill={isSelected ? "#FF7A2F" : axisLabelColor}
                  >
                    {idx + 1}
                  </text>
                </g>
              );
            })}
        </svg>
      </div>

      {/* Preset Library Tabs + Pills */}
      <div className="space-y-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div className="inline-flex gap-1 rounded-xl bg-[#F2F1F6] p-1 dark:bg-white/[0.06]">
            {(
              [
                ["official", "eq.tab_official"],
                ["curated", "eq.tab_curated"],
                ["custom", "eq.tab_custom"],
              ] as const
            ).map(([tabId, labelKey]) => (
              <button
                key={tabId}
                type="button"
                onClick={() => setPresetTab(tabId)}
                className={`rounded-lg px-3 py-1 text-xs font-semibold transition ${
                  presetTab === tabId
                    ? "bg-[#FF7A2F] text-white"
                    : "text-[#4D4D4D] hover:text-[#1A1A1A] dark:text-white/70 dark:hover:text-white"
                }`}
              >
                {t(labelKey)}
                {tabId === "custom" && customPresets.length > 0 ? ` (${customPresets.length})` : ""}
              </button>
            ))}
          </div>

          {capabilities.customEq && onSaveCustomPreset && (
            <button
              type="button"
              disabled={disabled}
              onClick={() => setShowSaveBar((v) => !v)}
              className="inline-flex items-center gap-1 rounded-xl bg-[#FF7A2F] px-3 py-1.5 text-xs font-semibold text-white transition hover:bg-[#E8691F] disabled:opacity-50"
            >
              <Save className="h-3.5 w-3.5" />
              {t("eq.custom_add")}
            </button>
          )}
        </div>

        {showSaveBar && onSaveCustomPreset && (
          <div className="flex items-center gap-2 rounded-xl border border-[#FF7A2F]/40 bg-[#FF7A2F]/8 p-2.5">
            <input
              type="text"
              value={saveName}
              onChange={(e) => setSaveName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") handleSaveCustom();
              }}
              placeholder={t("eq.custom_name_placeholder")}
              maxLength={28}
              className="flex-1 rounded-lg border border-[#E5E5E5] bg-white px-2.5 py-1.5 text-xs text-[#1A1A1A] outline-none focus:border-[#FF7A2F] dark:border-white/12 dark:bg-[#121212] dark:text-white"
            />
            <button
              type="button"
              onClick={handleSaveCustom}
              className="rounded-lg bg-[#FF7A2F] px-3 py-1.5 text-xs font-semibold text-white hover:bg-[#E8691F]"
            >
              {t("common.save")}
            </button>
          </div>
        )}

        <div className="flex flex-wrap gap-2">
          {visiblePresets.map((preset) => {
            const active = preset.id === activePresetId;
            return (
              <div
                key={preset.id}
                className={`group inline-flex items-center gap-1 rounded-full border transition ${
                  active
                    ? "border-[#FF7A2F] bg-[#FF7A2F] text-white"
                    : "border-[#E5E5E5] bg-[#F2F1F6] text-[#1A1A1A] hover:border-[#FF7A2F]/50 dark:border-white/10 dark:bg-white/[0.05] dark:text-white/85"
                }`}
              >
                <button
                  type="button"
                  disabled={disabled}
                  onClick={() => onSelectPreset(preset)}
                  className="px-3 py-1.5 text-xs font-semibold"
                >
                  {preset.category === "custom" ? preset.name : t(`eq.presets.${preset.id}`)}
                </button>
                {preset.category === "custom" && onDeleteCustomPreset && (
                  <button
                    type="button"
                    disabled={disabled}
                    onClick={() => onDeleteCustomPreset(preset.id)}
                    className={`mr-1.5 inline-flex h-5 w-5 items-center justify-center rounded-full transition ${
                      active
                        ? "hover:bg-white/20"
                        : "text-[#808080] hover:bg-[#FF4C4C]/15 hover:text-[#FF4C4C]"
                    }`}
                    aria-label={t("common.delete")}
                  >
                    <Trash2 className="h-3 w-3" />
                  </button>
                )}
              </div>
            );
          })}
        </div>
      </div>

      {/* Graphic 10-Band Sliders OR Parametric IIR Band Inspector */}
      {!capabilities.customEq ? (
        <p className="rounded-xl bg-[#F2F1F6] p-3 text-xs text-[#808080] dark:bg-white/[0.04] dark:text-white/55">
          {t("eq.custom_unsupported")}
        </p>
      ) : !isParametric ? (
        <div className={QCY_SUBCARD_CLASS}>
          <div className="grid grid-cols-5 gap-3 sm:grid-cols-10">
            {STANDARD_10_BAND_HZ.map((freq, idx) => {
              const gain = normalizedGraphicGains[idx];
              const isSelected = idx === safeSelectedIdx;
              return (
                <div
                  key={freq}
                  onClick={() => setSelectedBandIdx(idx)}
                  className={`flex flex-col items-center gap-1.5 rounded-xl p-1.5 transition ${
                    isSelected ? "bg-[#0066FF]/10 dark:bg-[#3399FF]/12" : ""
                  }`}
                >
                  <span className="font-mono text-[10px] font-bold text-[#0066FF] dark:text-[#66B2FF]">
                    {gain > 0 ? `+${gain.toFixed(1)}` : gain.toFixed(1)}
                  </span>
                  <div className="flex h-28 items-center justify-center">
                    <input
                      type="range"
                      min={-12}
                      max={12}
                      step={0.5}
                      disabled={disabled}
                      value={gain}
                      onChange={(e) => updateGraphicBand(idx, Number(e.target.value))}
                      aria-label={`${freq} Hz`}
                      className="h-24 w-2 cursor-pointer accent-[#FF7A2F]"
                      style={{ writingMode: "vertical-lr", direction: "rtl" }}
                    />
                  </div>
                  <span className="text-[10px] font-semibold text-[#4D4D4D] dark:text-white/65">
                    {formatFreqLabel(freq)}
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      ) : (
        <div className={`${QCY_SUBCARD_CLASS} space-y-4`}>
          {/* Band selector chips + Add band + Master Preamp */}
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div className="flex flex-wrap items-center gap-1.5">
              {activeBands.map((b, idx) => (
                <button
                  key={idx}
                  type="button"
                  onClick={() => setSelectedBandIdx(idx)}
                  className={`rounded-lg border px-2.5 py-1 text-xs font-semibold transition ${
                    idx === safeSelectedIdx
                      ? "border-[#FF7A2F] bg-[#FF7A2F] text-white"
                      : b.enabled === false
                        ? "border-[#E5E5E5] bg-black/5 text-[#808080] line-through dark:border-white/10 dark:bg-white/5"
                        : "border-[#E5E5E5] bg-white text-[#4D4D4D] dark:border-white/10 dark:bg-white/6 dark:text-white/80"
                  }`}
                >
                  #{idx + 1} · {formatFreqLabel(b.frequencyHz)}
                </button>
              ))}
              {activeBands.length < maxBands && (
                <button
                  type="button"
                  disabled={disabled}
                  onClick={handleAddBand}
                  className="inline-flex items-center gap-1 rounded-lg border border-dashed border-[#0066FF] px-2.5 py-1 text-xs font-semibold text-[#0066FF] hover:bg-[#0066FF]/8 dark:border-[#3399FF] dark:text-[#3399FF]"
                >
                  <Plus className="h-3.5 w-3.5" />
                  {t("eq.add_band")} ({activeBands.length}/{maxBands})
                </button>
              )}
            </div>

            <label className="inline-flex items-center gap-2 text-xs font-semibold text-[#4D4D4D] dark:text-white/75">
              <span>{t("eq.master_gain")}:</span>
              <input
                type="number"
                min={-12}
                max={12}
                step={0.5}
                disabled={disabled}
                value={masterGainDb}
                onChange={(e) =>
                  onParametricBandsChange(
                    activeBands,
                    Math.max(-12, Math.min(12, Number(e.target.value) || 0))
                  )
                }
                className="w-16 rounded-lg border border-[#E5E5E5] bg-white px-2 py-1 font-mono text-xs text-[#1A1A1A] dark:border-white/12 dark:bg-[#121212] dark:text-white"
              />
              <span>dB</span>
            </label>
          </div>

          {/* Selected Parametric Band Inspector */}
          {selectedBand && (
            <div className="grid gap-3 rounded-xl border border-[#E5E5E5] bg-white p-3.5 sm:grid-cols-2 lg:grid-cols-4 dark:border-white/10 dark:bg-[#1E1E1E]">
              {/* Filter Type (All 9 QCY wire filter types 0..8) */}
              <div className="space-y-1">
                <label className="text-[11px] font-semibold text-[#808080] dark:text-white/55">
                  {t("eq.filter_type")}
                </label>
                <select
                  disabled={disabled}
                  value={qcyFilterTypeToIndex(selectedBand.type) ?? 2}
                  onChange={(e) =>
                    updateParametricBand(safeSelectedIdx, {
                      type: qcyFilterTypeFromIndex(Number(e.target.value)),
                    })
                  }
                  className="w-full rounded-lg border border-[#E5E5E5] bg-[#F2F1F6] px-2.5 py-1.5 text-xs font-semibold text-[#1A1A1A] outline-none dark:border-white/12 dark:bg-[#121212] dark:text-white"
                >
                  {QCY_IIR_FILTERS.filter((f) => allowedFilterIndices.has(f.index)).map((f) => (
                    <option key={f.index} value={f.index}>
                      {t(f.i18nKey)}
                    </option>
                  ))}
                </select>
              </div>

              {/* Frequency 20..20000 Hz */}
              <div className="space-y-1">
                <div className="flex justify-between text-[11px]">
                  <span className="font-semibold text-[#808080] dark:text-white/55">
                    {t("eq.frequency")}
                  </span>
                  <span className="font-mono font-bold text-[#1A1A1A] dark:text-white">
                    {selectedBand.frequencyHz} Hz
                  </span>
                </div>
                <input
                  type="number"
                  min={20}
                  max={20000}
                  step={1}
                  disabled={disabled}
                  value={selectedBand.frequencyHz}
                  onChange={(e) =>
                    updateParametricBand(safeSelectedIdx, {
                      frequencyHz: Math.max(20, Math.min(20000, Number(e.target.value) || 20)),
                    })
                  }
                  className="w-full rounded-lg border border-[#E5E5E5] bg-[#F2F1F6] px-2.5 py-1.5 font-mono text-xs text-[#1A1A1A] dark:border-white/12 dark:bg-[#121212] dark:text-white"
                />
              </div>

              {/* Gain -12..+12 dB */}
              <div className="space-y-1">
                <div className="flex justify-between text-[11px]">
                  <span className="font-semibold text-[#808080] dark:text-white/55">
                    {t("eq.gain")}
                  </span>
                  <span className="font-mono font-bold text-[#FF7A2F]">
                    {selectedBand.gainDb > 0 ? `+${selectedBand.gainDb}` : selectedBand.gainDb} dB
                  </span>
                </div>
                <input
                  type="range"
                  min={-12}
                  max={12}
                  step={0.1}
                  disabled={disabled}
                  value={selectedBand.gainDb}
                  onChange={(e) =>
                    updateParametricBand(safeSelectedIdx, { gainDb: Number(e.target.value) })
                  }
                  className="mt-1.5 h-2 w-full cursor-pointer accent-[#FF7A2F]"
                />
              </div>

              {/* Q Factor 0.2..10.0 + Remove/Toggle */}
              <div className="space-y-1">
                <div className="flex items-center justify-between text-[11px]">
                  <span className="font-semibold text-[#808080] dark:text-white/55">
                    {t("eq.q_factor")}:{" "}
                    <strong className="font-mono text-[#0066FF] dark:text-[#3399FF]">
                      {selectedBand.q.toFixed(2)}
                    </strong>
                  </span>
                  {activeBands.length > 1 && (
                    <button
                      type="button"
                      disabled={disabled}
                      onClick={() => handleRemoveBand(safeSelectedIdx)}
                      className="text-[#FF4C4C] hover:underline"
                    >
                      {t("eq.remove_band")}
                    </button>
                  )}
                </div>
                <input
                  type="range"
                  min={0.2}
                  max={10}
                  step={0.05}
                  disabled={disabled}
                  value={selectedBand.q}
                  onChange={(e) =>
                    updateParametricBand(safeSelectedIdx, {
                      q: Math.round(Number(e.target.value) * 100) / 100,
                    })
                  }
                  className="mt-1.5 h-2 w-full cursor-pointer accent-[#0066FF] dark:accent-[#3399FF]"
                />
              </div>
            </div>
          )}
        </div>
      )}

      {/* Personalized L/R Hearing EQ Entry */}
      {capabilities.personalizedEq && (
        <div className={`${QCY_SUBCARD_CLASS} flex flex-wrap items-center justify-between gap-3`}>
          <div className="flex items-center gap-2.5">
            <span className="flex h-9 w-9 items-center justify-center rounded-xl bg-[#727AFF]/15 text-[#727AFF]">
              <Ear className="h-4 w-4" />
            </span>
            <div>
              <p className="text-sm font-bold text-[#1A1A1A] dark:text-white">
                {t("hearing_test.title")}
              </p>
              <p className="text-xs text-[#808080] dark:text-white/55">
                {t("eq.presets.personalized_sub")}
              </p>
            </div>
          </div>

          <div className="flex items-center gap-2.5">
            {onPersonalizedToggle && (
              <button
                type="button"
                role="switch"
                aria-checked={Boolean(auxState?.personalizedEqEnabled)}
                disabled={disabled}
                onClick={() => onPersonalizedToggle(!auxState?.personalizedEqEnabled)}
                className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors ${toggleTrackClass(
                  Boolean(auxState?.personalizedEqEnabled),
                  "orange"
                )}`}
              >
                <span
                  className={`inline-block h-4 w-4 rounded-full bg-white shadow transition-transform ${
                    auxState?.personalizedEqEnabled ? "translate-x-6" : "translate-x-1"
                  }`}
                />
              </button>
            )}
            {onOpenHearingTest && (
              <button
                type="button"
                disabled={disabled}
                onClick={onOpenHearingTest}
                className="rounded-xl bg-[#0066FF] px-3 py-1.5 text-xs font-semibold text-white transition hover:bg-[#0052CC] dark:bg-[#3399FF]"
              >
                {t("hearing_test.start_test")}
              </button>
            )}
          </div>
        </div>
      )}

      {/* Auxiliary Audio Enhancers (Dynamic EQ, THX, Virtual Bass, Adaptive Volume, Channel Balance) */}
      {auxState &&
        onAuxChange &&
        (capabilities.dynamicEq ||
          capabilities.thxEq ||
          capabilities.virtualBass ||
          capabilities.adaptiveVolume ||
          capabilities.channelBalance) && (
          <div className="grid gap-3 sm:grid-cols-2">
            {capabilities.dynamicEq && (
              <div className={`${QCY_SUBCARD_CLASS} flex items-center justify-between gap-2`}>
                <div className="flex items-center gap-2">
                  <Sparkles className="h-4 w-4 text-[#FF7A2F]" />
                  <div>
                    <p className="text-xs font-bold text-[#1A1A1A] dark:text-white">
                      {t("audio_modes.dynamic_eq")}
                    </p>
                    <p className="text-[11px] text-[#808080] dark:text-white/55">
                      {t("audio_modes.dynamic_eq_desc")}
                    </p>
                  </div>
                </div>
                <button
                  type="button"
                  role="switch"
                  aria-checked={auxState.dynamicEq}
                  disabled={disabled}
                  onClick={() => onAuxChange({ dynamicEq: !auxState.dynamicEq })}
                  className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors ${toggleTrackClass(
                    auxState.dynamicEq
                  )}`}
                >
                  <span
                    className={`inline-block h-4 w-4 rounded-full bg-white shadow transition-transform ${
                      auxState.dynamicEq ? "translate-x-6" : "translate-x-1"
                    }`}
                  />
                </button>
              </div>
            )}

            {capabilities.thxEq && (
              <div className={`${QCY_SUBCARD_CLASS} flex items-center justify-between gap-2`}>
                <div className="flex items-center gap-2">
                  <Activity className="h-4 w-4 text-[#0066FF] dark:text-[#3399FF]" />
                  <div>
                    <p className="text-xs font-bold text-[#1A1A1A] dark:text-white">
                      {t("audio_modes.thx_eq")}
                    </p>
                    <p className="text-[11px] text-[#808080] dark:text-white/55">
                      {t("audio_modes.thx_eq_desc")}
                    </p>
                  </div>
                </div>
                <button
                  type="button"
                  role="switch"
                  aria-checked={auxState.thxEq}
                  disabled={disabled}
                  onClick={() => onAuxChange({ thxEq: !auxState.thxEq })}
                  className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors ${toggleTrackClass(
                    auxState.thxEq,
                    "blue"
                  )}`}
                >
                  <span
                    className={`inline-block h-4 w-4 rounded-full bg-white shadow transition-transform ${
                      auxState.thxEq ? "translate-x-6" : "translate-x-1"
                    }`}
                  />
                </button>
              </div>
            )}

            {capabilities.virtualBass && (
              <div className={`${QCY_SUBCARD_CLASS} flex items-center justify-between gap-2`}>
                <div className="flex items-center gap-2">
                  <Volume2 className="h-4 w-4 text-[#FF7A2F]" />
                  <div>
                    <p className="text-xs font-bold text-[#1A1A1A] dark:text-white">
                      {t("audio_modes.virtual_bass")}
                    </p>
                    <p className="text-[11px] text-[#808080] dark:text-white/55">
                      {t("audio_modes.virtual_bass_desc")}
                    </p>
                  </div>
                </div>
                <button
                  type="button"
                  role="switch"
                  aria-checked={auxState.virtualBass}
                  disabled={disabled}
                  onClick={() => onAuxChange({ virtualBass: !auxState.virtualBass })}
                  className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors ${toggleTrackClass(
                    auxState.virtualBass
                  )}`}
                >
                  <span
                    className={`inline-block h-4 w-4 rounded-full bg-white shadow transition-transform ${
                      auxState.virtualBass ? "translate-x-6" : "translate-x-1"
                    }`}
                  />
                </button>
              </div>
            )}

            {capabilities.adaptiveVolume && (
              <div className={`${QCY_SUBCARD_CLASS} flex items-center justify-between gap-2`}>
                <div className="flex items-center gap-2">
                  <Volume2 className="h-4 w-4 text-[#0066FF] dark:text-[#3399FF]" />
                  <div>
                    <p className="text-xs font-bold text-[#1A1A1A] dark:text-white">
                      {t("audio_modes.adaptive_volume")}
                    </p>
                    <p className="text-[11px] text-[#808080] dark:text-white/55">
                      {t("audio_modes.adaptive_volume_desc")}
                    </p>
                  </div>
                </div>
                <button
                  type="button"
                  role="switch"
                  aria-checked={auxState.adaptiveVolume}
                  disabled={disabled}
                  onClick={() => onAuxChange({ adaptiveVolume: !auxState.adaptiveVolume })}
                  className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors ${toggleTrackClass(
                    auxState.adaptiveVolume,
                    "blue"
                  )}`}
                >
                  <span
                    className={`inline-block h-4 w-4 rounded-full bg-white shadow transition-transform ${
                      auxState.adaptiveVolume ? "translate-x-6" : "translate-x-1"
                    }`}
                  />
                </button>
              </div>
            )}

            {capabilities.channelBalance && (
              <div className={`${QCY_SUBCARD_CLASS} space-y-2 sm:col-span-2`}>
                <div className="flex items-center justify-between text-xs">
                  <span className="font-bold text-[#1A1A1A] dark:text-white">
                    {t("audio_modes.channel_balance")}
                  </span>
                  <button
                    type="button"
                    disabled={disabled}
                    onClick={() => onAuxChange({ channelBalance: 50 })}
                    className="font-mono font-semibold text-[#0066FF] hover:underline dark:text-[#3399FF]"
                  >
                    {auxState.channelBalance === 50
                      ? t("audio_modes.channel_center")
                      : auxState.channelBalance < 50
                        ? `L +${50 - auxState.channelBalance}`
                        : `R +${auxState.channelBalance - 50}`}
                  </button>
                </div>
                <div className="flex items-center gap-3">
                  <span className="text-[11px] font-semibold text-[#808080]">
                    {t("audio_modes.channel_left")}
                  </span>
                  <input
                    type="range"
                    min={0}
                    max={100}
                    step={1}
                    disabled={disabled}
                    value={auxState.channelBalance}
                    onChange={(e) => onAuxChange({ channelBalance: Number(e.target.value) })}
                    className="h-2 flex-1 cursor-pointer accent-[#0066FF] dark:accent-[#3399FF]"
                  />
                  <span className="text-[11px] font-semibold text-[#808080]">
                    {t("audio_modes.channel_right")}
                  </span>
                </div>
              </div>
            )}
          </div>
        )}
    </section>
  );
};
