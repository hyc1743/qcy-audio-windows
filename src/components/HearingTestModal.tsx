import React, { useEffect, useMemo, useRef, useState } from "react";
import {
  Check,
  Ear,
  Play,
  RotateCcw,
  ShieldCheck,
  Volume1,
  Volume2,
  VolumeX,
  X,
} from "lucide-react";
import {
  AudiogramResult,
  EarSide,
  HEARING_TEST_FREQUENCIES_HZ,
  HearingCompensationProfile,
  HearingTestEngine,
  QCY_INDIVIDUATION_FREQUENCIES_HZ,
} from "../audio/hearingTestEngine";
import { QCY_SUBCARD_CLASS, TranslateFn, UiTheme } from "./shared";

export interface HearingTestModalProps {
  open: boolean;
  /** Optional custom frequency grid (defaults to QCY 8-band `[63..8000]` Hz or standard `[125..8000]` Hz). */
  frequenciesHz?: readonly number[];
  theme?: UiTheme;
  onApply: (profile: HearingCompensationProfile, audiogram: AudiogramResult) => void;
  onClose: () => void;
  /** Optional callback fired when entering/leaving earbud reference test mode (`0x45`). */
  onHardwareTestModeChange?: (active: boolean) => void;
  t: TranslateFn;
}

type WizardStep = "intro" | "testing" | "results";

function formatHz(freq: number): string {
  return freq >= 1000 ? `${freq / 1000} kHz` : `${freq} Hz`;
}

const CHART_W = 540;
const CHART_H = 190;
const PAD_L = 42;
const PAD_R = 16;
const PAD_T = 16;
const PAD_B = 28;
const PLOT_W = CHART_W - PAD_L - PAD_R;
const PLOT_H = CHART_H - PAD_T - PAD_B;

export const HearingTestModal: React.FC<HearingTestModalProps> = ({
  open,
  frequenciesHz,
  theme = "dark",
  onApply,
  onClose,
  onHardwareTestModeChange,
  t,
}) => {
  const [useQcyOctaves, setUseQcyOctaves] = useState(true);
  const activeFreqs = useMemo(
    () =>
      frequenciesHz && frequenciesHz.length > 0
        ? [...frequenciesHz]
        : useQcyOctaves
          ? [...QCY_INDIVIDUATION_FREQUENCIES_HZ]
          : [...HEARING_TEST_FREQUENCIES_HZ],
    [frequenciesHz, useQcyOctaves]
  );

  const engineRef = useRef<HearingTestEngine | null>(null);
  const [step, setStep] = useState<WizardStep>("intro");
  const [side, setSide] = useState<EarSide>("left");
  const [bandIndex, setBandIndex] = useState<number>(0);
  const [currentLevelDbHl, setCurrentLevelDbHl] = useState<number>(20);
  const [isPlayingTone, setIsPlayingTone] = useState<boolean>(false);
  const [completedMask, setCompletedMask] = useState<{ left: boolean[]; right: boolean[] }>(() => ({
    left: activeFreqs.map(() => false),
    right: activeFreqs.map(() => false),
  }));
  const [resultAudiogram, setResultAudiogram] = useState<AudiogramResult | null>(null);
  const [resultProfile, setResultProfile] = useState<HearingCompensationProfile | null>(null);

  // Manage HearingTestEngine lifecycle cleanly on open/close/freq switch
  useEffect(() => {
    if (!open) return;
    const eng = new HearingTestEngine(activeFreqs);
    engineRef.current = eng;
    return () => {
      eng.dispose();
      engineRef.current = null;
    };
  }, [open, activeFreqs]);

  // Notify hardware `0x45` test mode when entering/leaving active test step
  useEffect(() => {
    if (!open) return;
    onHardwareTestModeChange?.(step === "testing");
    return () => {
      onHardwareTestModeChange?.(false);
    };
  }, [open, step, onHardwareTestModeChange]);

  // Escape key dismissal
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        engineRef.current?.stopTone();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;

  const currentFreq = activeFreqs[bandIndex] ?? activeFreqs[0];
  const totalSteps = activeFreqs.length * 2;
  const completedCount =
    completedMask.left.filter(Boolean).length + completedMask.right.filter(Boolean).length;

  const triggerTone = (targetSide: EarSide, freq: number, level: number) => {
    const eng = engineRef.current;
    if (!eng) return;
    setIsPlayingTone(true);
    eng.playTone({ frequencyHz: freq, side: targetSide, levelDbHl: level, durationMs: 700 });
    setTimeout(() => setIsPlayingTone(false), 740);
  };

  const handleStartTest = () => {
    engineRef.current?.reset();
    setCompletedMask({
      left: activeFreqs.map(() => false),
      right: activeFreqs.map(() => false),
    });
    setSide("left");
    setBandIndex(0);
    setCurrentLevelDbHl(20);
    setStep("testing");
    setTimeout(() => triggerTone("left", activeFreqs[0], 20), 120);
  };

  const handleConfirmHear = () => {
    const eng = engineRef.current;
    if (!eng) return;
    eng.stopTone();
    eng.recordThreshold(side, bandIndex, currentLevelDbHl);

    const nextMask = {
      left: [...completedMask.left],
      right: [...completedMask.right],
    };
    nextMask[side][bandIndex] = true;
    setCompletedMask(nextMask);

    if (bandIndex + 1 < activeFreqs.length) {
      const nextIdx = bandIndex + 1;
      setBandIndex(nextIdx);
      setCurrentLevelDbHl(20);
      setTimeout(() => triggerTone(side, activeFreqs[nextIdx], 20), 140);
    } else if (side === "left") {
      setSide("right");
      setBandIndex(0);
      setCurrentLevelDbHl(20);
      setTimeout(() => triggerTone("right", activeFreqs[0], 20), 180);
    } else {
      const audiogram = eng.getAudiogram(20);
      const profile = eng.computeCompensationProfile();
      setResultAudiogram(audiogram);
      setResultProfile(profile);
      setStep("results");
    }
  };

  const handleStepLevel = (deltaSteps: number) => {
    const eng = engineRef.current;
    if (!eng) return;
    const nextLevel = eng.stepThreshold(currentLevelDbHl, deltaSteps, 5);
    setCurrentLevelDbHl(nextLevel);
    triggerTone(side, currentFreq, nextLevel);
  };

  const gridStroke = theme === "dark" ? "rgba(255,255,255,0.1)" : "rgba(26,26,26,0.08)";
  const axisColor = theme === "dark" ? "rgba(255,255,255,0.55)" : "#808080";

  const buildResultPath = (values: number[]) => {
    if (values.length === 0) return "";
    return values
      .map((db, i) => {
        const x = PAD_L + (i / Math.max(1, values.length - 1)) * PLOT_W;
        const clamped = Math.max(-6, Math.min(6, db));
        const y = PAD_T + ((6 - clamped) / 12) * PLOT_H;
        return `${i === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`;
      })
      .join(" ");
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/55 p-4 backdrop-blur-xs"
      role="dialog"
      aria-modal="true"
      aria-labelledby="qcy-hearing-test-title"
      onClick={() => {
        engineRef.current?.stopTone();
        onClose();
      }}
    >
      <div
        className="w-full max-w-xl rounded-2xl border border-[#E5E5E5] bg-white p-6 shadow-2xl dark:border-white/12 dark:bg-[#1E1E1E]"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-start justify-between gap-3">
          <div className="flex items-center gap-3">
            <span className="flex h-10 w-10 items-center justify-center rounded-xl bg-[#0066FF]/12 text-[#0066FF] dark:bg-[#3399FF]/15 dark:text-[#3399FF]">
              <Ear className="h-5 w-5" />
            </span>
            <div>
              <h3
                id="qcy-hearing-test-title"
                className="text-base font-bold text-[#1A1A1A] dark:text-white"
              >
                {t("hearing_test.title")}
              </h3>
              <p className="text-xs text-[#808080] dark:text-white/55">
                {t("hearing_test.subtitle")}
              </p>
            </div>
          </div>

          <button
            type="button"
            onClick={() => {
              engineRef.current?.stopTone();
              onClose();
            }}
            className="inline-flex h-8 w-8 items-center justify-center rounded-lg text-[#808080] hover:bg-black/5 dark:text-white/60 dark:hover:bg-white/10"
            aria-label={t("common.close")}
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {/* STEP 1: INTRO */}
        {step === "intro" && (
          <div className="mt-5 space-y-4">
            <div className={`${QCY_SUBCARD_CLASS} space-y-2.5`}>
              <p className="text-sm font-bold text-[#1A1A1A] dark:text-white">
                {t("hearing_test.intro_title")}
              </p>
              <p className="text-xs leading-relaxed text-[#4D4D4D] dark:text-white/75">
                {t("hearing_test.intro_desc")}
              </p>
              <ul className="space-y-1.5 pt-1 text-xs text-[#4D4D4D] dark:text-white/75">
                <li className="flex items-center gap-2">
                  <Check className="h-3.5 w-3.5 text-[#22C55E]" />
                  {t("hearing_test.quiet_env_tip")}
                </li>
                <li className="flex items-center gap-2">
                  <Check className="h-3.5 w-3.5 text-[#22C55E]" />
                  {t("hearing_test.volume_tip")}
                </li>
              </ul>
            </div>

            {!frequenciesHz && (
              <div className="flex items-center justify-between gap-2 rounded-xl border border-[#E5E5E5] px-3.5 py-2.5 text-xs dark:border-white/10">
                <span className="font-semibold text-[#4D4D4D] dark:text-white/75">
                  8-Band Frequency Grid
                </span>
                <div className="inline-flex gap-1 rounded-lg bg-[#F2F1F6] p-0.5 dark:bg-white/6">
                  <button
                    type="button"
                    onClick={() => setUseQcyOctaves(true)}
                    className={`rounded-md px-2.5 py-1 font-semibold ${
                      useQcyOctaves
                        ? "bg-[#FF7A2F] text-white"
                        : "text-[#4D4D4D] dark:text-white/65"
                    }`}
                  >
                    63 – 8k Hz (QCY)
                  </button>
                  <button
                    type="button"
                    onClick={() => setUseQcyOctaves(false)}
                    className={`rounded-md px-2.5 py-1 font-semibold ${
                      !useQcyOctaves
                        ? "bg-[#FF7A2F] text-white"
                        : "text-[#4D4D4D] dark:text-white/65"
                    }`}
                  >
                    125 – 8k Hz
                  </button>
                </div>
              </div>
            )}

            <div className="flex items-center gap-2 rounded-xl border border-[#22C55E]/30 bg-[#22C55E]/10 px-3.5 py-2.5 text-xs text-[#15803D] dark:text-[#4ADE80]">
              <ShieldCheck className="h-4 w-4 shrink-0" />
              <span>{t("hearing_test.safety_cap_notice")}</span>
            </div>

            <div className="flex justify-end gap-2.5 pt-2">
              <button
                type="button"
                onClick={onClose}
                className="rounded-xl border border-[#E5E5E5] px-4 py-2 text-sm font-semibold text-[#4D4D4D] dark:border-white/12 dark:text-white/75"
              >
                {t("common.cancel")}
              </button>
              <button
                type="button"
                onClick={handleStartTest}
                className="inline-flex items-center gap-2 rounded-xl bg-[#FF7A2F] px-5 py-2 text-sm font-semibold text-white hover:bg-[#E8691F]"
              >
                <Play className="h-4 w-4" />
                {t("hearing_test.start_test")}
              </button>
            </div>
          </div>
        )}

        {/* STEP 2: ACTIVE AUDIOMETRY */}
        {step === "testing" && (
          <div className="mt-5 space-y-5">
            {/* Ear selector & Progress */}
            <div className="space-y-2">
              <div className="flex items-center justify-between text-xs font-semibold">
                <div className="inline-flex gap-1.5">
                  <button
                    type="button"
                    onClick={() => {
                      setSide("left");
                      setBandIndex(0);
                      setCurrentLevelDbHl(20);
                    }}
                    className={`rounded-full px-3 py-1 transition ${
                      side === "left"
                        ? "bg-[#0066FF] text-white dark:bg-[#3399FF]"
                        : "bg-[#F2F1F6] text-[#4D4D4D] dark:bg-white/8 dark:text-white/65"
                    }`}
                  >
                    {t("hearing_test.testing_left_ear")}
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      setSide("right");
                      setBandIndex(0);
                      setCurrentLevelDbHl(20);
                    }}
                    className={`rounded-full px-3 py-1 transition ${
                      side === "right"
                        ? "bg-[#FF7A2F] text-white"
                        : "bg-[#F2F1F6] text-[#4D4D4D] dark:bg-white/8 dark:text-white/65"
                    }`}
                  >
                    {t("hearing_test.testing_right_ear")}
                  </button>
                </div>

                <span className="font-mono text-[#808080] dark:text-white/60">
                  {t("hearing_test.band_progress", {
                    current: bandIndex + 1,
                    total: activeFreqs.length,
                    freq: formatHz(currentFreq),
                  })}
                </span>
              </div>

              <div className="h-2 w-full overflow-hidden rounded-full bg-[#F2F1F6] dark:bg-white/10">
                <div
                  className="h-full rounded-full bg-[#FF7A2F] transition-all duration-300"
                  style={{ width: `${Math.round(((completedCount + 0.5) / totalSteps) * 100)}%` }}
                />
              </div>
            </div>

            {/* Active Tone Pulse Display */}
            <div className="flex flex-col items-center justify-center gap-3 rounded-2xl border border-[#E5E5E5] bg-[#F2F1F6] py-7 dark:border-white/10 dark:bg-[#141418]">
              <button
                type="button"
                onClick={() => triggerTone(side, currentFreq, currentLevelDbHl)}
                className={`flex h-20 w-20 items-center justify-center rounded-full text-white shadow-lg transition ${
                  side === "left" ? "bg-[#0066FF] dark:bg-[#3399FF]" : "bg-[#FF7A2F]"
                } ${isPlayingTone ? "scale-105 ring-8 ring-[#FF7A2F]/25" : "hover:scale-105"}`}
                aria-label={t("hearing_test.replay_tone")}
              >
                <Volume2 className={`h-9 w-9 ${isPlayingTone ? "animate-pulse" : ""}`} />
              </button>

              <div className="text-center">
                <p className="text-2xl font-bold tracking-tight text-[#1A1A1A] dark:text-white">
                  {formatHz(currentFreq)}
                </p>
                <p className="mt-0.5 font-mono text-xs font-semibold text-[#808080] dark:text-white/60">
                  {currentLevelDbHl} dB HL ·{" "}
                  {side === "left" ? t("common.left") : t("common.right")}
                </p>
              </div>

              <button
                type="button"
                onClick={() => triggerTone(side, currentFreq, currentLevelDbHl)}
                className="inline-flex items-center gap-1.5 rounded-full border border-[#E5E5E5] bg-white px-3.5 py-1 text-xs font-semibold text-[#4D4D4D] hover:border-[#FF7A2F] dark:border-white/12 dark:bg-white/8 dark:text-white/80"
              >
                <Play className="h-3 w-3" />
                {t("hearing_test.replay_tone")}
              </button>
            </div>

            {/* Threshold Stepping Controls */}
            <div className="grid grid-cols-3 gap-2.5">
              <button
                type="button"
                onClick={() => handleStepLevel(-1)}
                className="flex items-center justify-center gap-1.5 rounded-xl border border-[#E5E5E5] bg-[#F2F1F6] py-3 text-xs font-semibold text-[#1A1A1A] hover:border-[#0066FF] dark:border-white/10 dark:bg-white/[0.05] dark:text-white"
              >
                <Volume1 className="h-4 w-4 text-[#0066FF] dark:text-[#3399FF]" />
                {t("hearing_test.softer")} (-5 dB)
              </button>

              <button
                type="button"
                onClick={() => handleStepLevel(1)}
                className="flex items-center justify-center gap-1.5 rounded-xl border border-[#E5E5E5] bg-[#F2F1F6] py-3 text-xs font-semibold text-[#1A1A1A] hover:border-[#FF7A2F] dark:border-white/10 dark:bg-white/[0.05] dark:text-white"
              >
                <VolumeX className="h-4 w-4 text-[#FF7A2F]" />
                {t("hearing_test.cannot_hear")} (+5 dB)
              </button>

              <button
                type="button"
                onClick={handleConfirmHear}
                className="flex items-center justify-center gap-1.5 rounded-xl bg-[#22C55E] py-3 text-xs font-bold text-white shadow-xs hover:bg-[#16A34A]"
              >
                <Check className="h-4 w-4" />
                {t("hearing_test.can_hear")}
              </button>
            </div>
          </div>
        )}

        {/* STEP 3: AUDIOGRAM RESULTS SVG */}
        {step === "results" && resultProfile && resultAudiogram && (
          <div className="mt-5 space-y-4">
            <div className="rounded-2xl border border-[#E5E5E5] bg-[#F8F8FB] p-3.5 dark:border-white/10 dark:bg-[#141418]">
              <div className="mb-2 flex flex-wrap items-center justify-between gap-2 text-xs">
                <span className="font-bold text-[#1A1A1A] dark:text-white">
                  {t("hearing_test.result_title")}
                </span>
                <div className="flex items-center gap-3">
                  <span className="inline-flex items-center gap-1.5 font-semibold text-[#0066FF] dark:text-[#3399FF]">
                    <span className="h-2 w-3.5 rounded-full bg-[#3399FF]" />
                    {t("hearing_test.left_ear_curve")}
                  </span>
                  <span className="inline-flex items-center gap-1.5 font-semibold text-[#FF7A2F]">
                    <span className="h-2 w-3.5 rounded-full bg-[#FF7A2F]" />
                    {t("hearing_test.right_ear_curve")}
                  </span>
                </div>
              </div>

              <svg
                viewBox={`0 0 ${CHART_W} ${CHART_H}`}
                className="h-44 w-full"
                role="img"
                aria-label={t("hearing_test.result_title")}
              >
                {[6, 3, 0, -3, -6].map((db) => {
                  const y = PAD_T + ((6 - db) / 12) * PLOT_H;
                  return (
                    <g key={db}>
                      <line
                        x1={PAD_L}
                        y1={y}
                        x2={CHART_W - PAD_R}
                        y2={y}
                        stroke={gridStroke}
                        strokeDasharray={db === 0 ? undefined : "3 3"}
                      />
                      <text x={PAD_L - 6} y={y + 3.5} textAnchor="end" fontSize="10" fill={axisColor}>
                        {db > 0 ? `+${db}` : db}
                      </text>
                    </g>
                  );
                })}

                {resultProfile.frequenciesHz.map((f, i) => {
                  const x =
                    PAD_L +
                    (i / Math.max(1, resultProfile.frequenciesHz.length - 1)) * PLOT_W;
                  return (
                    <text
                      key={f}
                      x={x}
                      y={CHART_H - 8}
                      textAnchor="middle"
                      fontSize="10"
                      fill={axisColor}
                    >
                      {f >= 1000 ? `${f / 1000}k` : f}
                    </text>
                  );
                })}

                <path
                  d={buildResultPath(resultProfile.leftCompensationDb)}
                  fill="none"
                  stroke="#3399FF"
                  strokeWidth="2.5"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
                <path
                  d={buildResultPath(resultProfile.rightCompensationDb)}
                  fill="none"
                  stroke="#FF7A2F"
                  strokeWidth="2.5"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              </svg>
            </div>

            <p className="text-xs text-[#4D4D4D] dark:text-white/75">
              {t("hearing_test.result_desc")}
            </p>

            <div className="flex items-center justify-between gap-2 pt-2">
              <button
                type="button"
                onClick={handleStartTest}
                className="inline-flex items-center gap-1.5 rounded-xl border border-[#E5E5E5] px-4 py-2 text-sm font-semibold text-[#4D4D4D] hover:border-[#FF7A2F] dark:border-white/12 dark:text-white/80"
              >
                <RotateCcw className="h-4 w-4" />
                {t("hearing_test.retake_test")}
              </button>

              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={onClose}
                  className="rounded-xl border border-[#E5E5E5] px-4 py-2 text-sm font-semibold text-[#4D4D4D] dark:border-white/12 dark:text-white/75"
                >
                  {t("common.close")}
                </button>
                <button
                  type="button"
                  onClick={() => {
                    onApply(resultProfile, resultAudiogram);
                    onClose();
                  }}
                  className="inline-flex items-center gap-1.5 rounded-xl bg-[#FF7A2F] px-5 py-2 text-sm font-semibold text-white hover:bg-[#E8691F]"
                >
                  <Check className="h-4 w-4" />
                  {t("hearing_test.apply_to_earbuds")}
                </button>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
