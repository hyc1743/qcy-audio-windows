/**
 * Clean-room Web Audio Pure-Tone Audiometry Engine (QCY Personalized Hearing Test equivalent).
 *
 * Plays click-free pure-tone sine pulses strictly isolated to Left (`pan = -1`) or Right (`pan = +1`)
 * ear using Web Audio `StereoPannerNode`, steps dB HL hearing thresholds, and converts measured
 * audiograms into normalized 8-band & 10-band EQ compensation curves.
 */

import { createAudioContext } from "./audioUtils";

export type EarSide = "left" | "right";

/** Standard 8-band hearing test frequencies (Hz) for L/R threshold stepping. */
export const HEARING_TEST_FREQUENCIES_HZ: readonly number[] = [
  125, 250, 500, 1000, 2000, 4000, 6000, 8000,
];

/** QCY `individuation_eq` 8-octave frequencies (Hz). */
export const QCY_INDIVIDUATION_FREQUENCIES_HZ: readonly number[] = [
  63, 125, 250, 500, 1000, 2000, 4000, 8000,
];

/** Standard 10-band graphic/PEQ frequencies (Hz) used by QCY earbuds. */
export const STANDARD_10_BAND_FREQUENCIES_HZ: readonly number[] = [
  31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000,
];

export const MIN_THRESHOLD_DB_HL = -10;
export const MAX_THRESHOLD_DB_HL = 80;
export const DEFAULT_THRESHOLD_STEP_DB = 5;

export interface AudiogramResult {
  frequenciesHz: number[];
  /** Measured hearing thresholds in dB HL for the left ear (higher = less sensitive). */
  leftThresholdsDbHl: number[];
  /** Measured hearing thresholds in dB HL for the right ear (higher = less sensitive). */
  rightThresholdsDbHl: number[];
}

export interface HearingCompensationProfile {
  frequenciesHz: number[];
  leftCompensationDb: number[];
  rightCompensationDb: number[];
  /** Combined mono/average EQ compensation curve (dB) centered & clamped to hardware range. */
  combinedCompensationDb: number[];
  /** Projected 10-band EQ gains for `[31..16000]` Hz. */
  tenBandFrequenciesHz: number[];
  tenBandGainsDb: number[];
  /** Suggested L/R channel balance offset in `[-1.0, +1.0]` (negative = favor left, positive = favor right). */
  suggestedBalance: number;
}

export interface TonePulseOptions {
  frequencyHz: number;
  side: EarSide;
  /** Hearing level in dB HL (`-10..80`). Converted safely to digital full-scale amplitude with hard cap. */
  levelDbHl: number;
  /** Total pulse duration in milliseconds (default 650 ms). */
  durationMs?: number;
  /** Fade-in/out edge ramp in milliseconds to eliminate audible clicks (default 30 ms). */
  rampMs?: number;
}

/**
 * Converts a dB HL test level (`-10..80`) to a safe linear Web Audio gain in `(0, 0.35]`.
 * Applies a conservative digital ceiling (`0.35`, roughly -9 dBFS) for hearing safety.
 */
export function dbHlToSafeLinearGain(levelDbHl: number): number {
  const clamped = Math.max(MIN_THRESHOLD_DB_HL, Math.min(MAX_THRESHOLD_DB_HL, levelDbHl));
  // Reference: 80 dB HL maps to -9 dBFS (0.35), 0 dB HL maps to ~-65 dBFS.
  const dbfs = -65 + ((clamped - MIN_THRESHOLD_DB_HL) / (MAX_THRESHOLD_DB_HL - MIN_THRESHOLD_DB_HL)) * 56;
  const linear = Math.pow(10, dbfs / 20);
  return Math.max(0.0002, Math.min(0.35, linear));
}

function clamp(val: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, val));
}

function roundToTenth(val: number): number {
  return Math.round(val * 10) / 10;
}

/**
 * Interpolates a curve defined at `sourceFreqs` onto `targetFreqs` in log-frequency space.
 * Clamps smoothly at boundary frequencies to avoid wild extrapolation.
 */
export function projectCurveLogFreq(
  sourceFreqs: readonly number[],
  sourceGainsDb: readonly number[],
  targetFreqs: readonly number[]
): number[] {
  if (sourceFreqs.length === 0 || sourceGainsDb.length === 0) {
    return targetFreqs.map(() => 0);
  }
  const n = Math.min(sourceFreqs.length, sourceGainsDb.length);
  const logSrc = sourceFreqs.slice(0, n).map((f) => Math.log2(Math.max(1, f)));

  return targetFreqs.map((tf) => {
    const lt = Math.log2(Math.max(1, tf));
    if (lt <= logSrc[0]) return roundToTenth(sourceGainsDb[0]);
    if (lt >= logSrc[n - 1]) return roundToTenth(sourceGainsDb[n - 1]);

    for (let i = 0; i < n - 1; i++) {
      if (lt >= logSrc[i] && lt <= logSrc[i + 1]) {
        const span = logSrc[i + 1] - logSrc[i];
        const ratio = span > 1e-9 ? (lt - logSrc[i]) / span : 0;
        const val = sourceGainsDb[i] + ratio * (sourceGainsDb[i + 1] - sourceGainsDb[i]);
        return roundToTenth(val);
      }
    }
    return 0;
  });
}

/**
 * Converts measured Left/Right dB HL hearing thresholds into normalized 8-band and 10-band
 * EQ compensation curves plus a recommended L/R channel balance adjustment.
 *
 * Uses half-gain audiometric prescription (NAL-like 0.45x relative deficit compensation) centered
 * around each user's mean threshold so overall loudness stays comfortable without clipping.
 */
export function computeHearingCompensation(
  audiogram: AudiogramResult,
  options?: {
    maxBoostDb?: number;
    maxCutDb?: number;
    compensationSlope?: number;
    tenBandFrequenciesHz?: readonly number[];
  }
): HearingCompensationProfile {
  const maxBoostDb = options?.maxBoostDb ?? 6.0;
  const maxCutDb = options?.maxCutDb ?? -6.0;
  const slope = options?.compensationSlope ?? 0.45;
  const tenBandFreqs = [...(options?.tenBandFrequenciesHz ?? STANDARD_10_BAND_FREQUENCIES_HZ)];

  const n = Math.min(
    audiogram.frequenciesHz.length,
    audiogram.leftThresholdsDbHl.length,
    audiogram.rightThresholdsDbHl.length
  );

  const freqs = audiogram.frequenciesHz.slice(0, n);
  const leftRaw = audiogram.leftThresholdsDbHl
    .slice(0, n)
    .map((v) => clamp(v, MIN_THRESHOLD_DB_HL, MAX_THRESHOLD_DB_HL));
  const rightRaw = audiogram.rightThresholdsDbHl
    .slice(0, n)
    .map((v) => clamp(v, MIN_THRESHOLD_DB_HL, MAX_THRESHOLD_DB_HL));

  if (n === 0) {
    return {
      frequenciesHz: [],
      leftCompensationDb: [],
      rightCompensationDb: [],
      combinedCompensationDb: [],
      tenBandFrequenciesHz: tenBandFreqs,
      tenBandGainsDb: tenBandFreqs.map(() => 0),
      suggestedBalance: 0,
    };
  }

  const mean = (arr: number[]) => arr.reduce((a, b) => a + b, 0) / arr.length;
  const leftMean = mean(leftRaw);
  const rightMean = mean(rightRaw);
  const overallMean = (leftMean + rightMean) / 2;

  const normalizeEar = (thresholds: number[], baseline: number): number[] =>
    thresholds.map((t) => roundToTenth(clamp((t - baseline) * slope, maxCutDb, maxBoostDb)));

  const leftCompensationDb = normalizeEar(leftRaw, leftMean);
  const rightCompensationDb = normalizeEar(rightRaw, rightMean);

  const avgThresholds = leftRaw.map((l, i) => (l + rightRaw[i]) / 2);
  const combinedCompensationDb = normalizeEar(avgThresholds, overallMean);

  const tenBandGainsDb = projectCurveLogFreq(freqs, combinedCompensationDb, tenBandFreqs).map((g) =>
    roundToTenth(clamp(g, maxCutDb, maxBoostDb))
  );

  // If right ear has higher threshold (needs more level), shift balance positive; cap to [-0.5, 0.5].
  const earDeltaDb = rightMean - leftMean;
  const suggestedBalance = roundToTenth(clamp((earDeltaDb / 40) * 0.5, -0.5, 0.5));

  return {
    frequenciesHz: freqs,
    leftCompensationDb,
    rightCompensationDb,
    combinedCompensationDb,
    tenBandFrequenciesHz: tenBandFreqs,
    tenBandGainsDb,
    suggestedBalance,
  };
}

/**
 * Interactive Web Audio pure-tone player & audiometry session controller.
 */
export class HearingTestEngine {
  private ctx: AudioContext | null = null;
  private activeOsc: OscillatorNode | null = null;
  private activeGain: GainNode | null = null;
  private stopTimeout: ReturnType<typeof setTimeout> | null = null;

  readonly frequenciesHz: number[];
  private leftThresholds: Map<number, number> = new Map();
  private rightThresholds: Map<number, number> = new Map();

  constructor(frequenciesHz: readonly number[] = HEARING_TEST_FREQUENCIES_HZ) {
    this.frequenciesHz = [...frequenciesHz];
  }

  private ensureContext(): AudioContext | null {
    if (!this.ctx) {
      this.ctx = createAudioContext();
    }
    if (this.ctx && this.ctx.state === "suspended") {
      void this.ctx.resume().catch(() => {});
    }
    return this.ctx;
  }

  /** Plays a single click-free isolated L/R pure-tone pulse. Safe no-op in headless environments. */
  playTone(options: TonePulseOptions): boolean {
    this.stopTone();
    const ctx = this.ensureContext();
    if (!ctx) return false;

    const now = ctx.currentTime;
    const durationSec = Math.max(0.08, (options.durationMs ?? 650) / 1000);
    const rampSec = Math.min(durationSec * 0.35, Math.max(0.01, (options.rampMs ?? 30) / 1000));
    const peakGain = dbHlToSafeLinearGain(options.levelDbHl);

    const osc = ctx.createOscillator();
    osc.type = "sine";
    osc.frequency.setValueAtTime(Math.max(20, Math.min(20_000, options.frequencyHz)), now);

    const gain = ctx.createGain();
    gain.gain.setValueAtTime(0.00001, now);
    gain.gain.linearRampToValueAtTime(peakGain, now + rampSec);
    gain.gain.setValueAtTime(peakGain, now + durationSec - rampSec);
    gain.gain.linearRampToValueAtTime(0.00001, now + durationSec);

    if (typeof ctx.createStereoPanner === "function") {
      const panner = ctx.createStereoPanner();
      panner.pan.setValueAtTime(options.side === "left" ? -1 : 1, now);
      osc.connect(gain);
      gain.connect(panner);
      panner.connect(ctx.destination);
    } else {
      osc.connect(gain);
      gain.connect(ctx.destination);
    }

    osc.start(now);
    osc.stop(now + durationSec + 0.02);

    this.activeOsc = osc;
    this.activeGain = gain;
    this.stopTimeout = setTimeout(() => {
      this.activeOsc = null;
      this.activeGain = null;
      this.stopTimeout = null;
    }, Math.ceil((durationSec + 0.05) * 1000));

    return true;
  }

  /** Immediately silences any currently playing test tone. */
  stopTone(): void {
    if (this.stopTimeout !== null) {
      clearTimeout(this.stopTimeout);
      this.stopTimeout = null;
    }
    if (this.activeOsc) {
      try {
        this.activeOsc.stop();
        this.activeOsc.disconnect();
      } catch {
        // oscillator already stopped
      }
      this.activeOsc = null;
    }
    if (this.activeGain) {
      try {
        this.activeGain.disconnect();
      } catch {
        // already disconnected
      }
      this.activeGain = null;
    }
  }

  /** Records a measured hearing threshold (dB HL) for a given ear and band index. */
  recordThreshold(side: EarSide, bandIndex: number, thresholdDbHl: number): void {
    if (bandIndex < 0 || bandIndex >= this.frequenciesHz.length) return;
    const clamped = clamp(thresholdDbHl, MIN_THRESHOLD_DB_HL, MAX_THRESHOLD_DB_HL);
    if (side === "left") {
      this.leftThresholds.set(bandIndex, clamped);
    } else {
      this.rightThresholds.set(bandIndex, clamped);
    }
  }

  /** Steps a current threshold up or down by `stepDb` (default 5 dB HL), clamped safely. */
  stepThreshold(currentDbHl: number, deltaSteps: number, stepDb: number = DEFAULT_THRESHOLD_STEP_DB): number {
    return clamp(
      currentDbHl + deltaSteps * stepDb,
      MIN_THRESHOLD_DB_HL,
      MAX_THRESHOLD_DB_HL
    );
  }

  /** Returns the current audiogram snapshot (defaulting unmeasured bands to 20 dB HL). */
  getAudiogram(defaultThresholdDbHl: number = 20): AudiogramResult {
    const leftThresholdsDbHl = this.frequenciesHz.map(
      (_, i) => this.leftThresholds.get(i) ?? defaultThresholdDbHl
    );
    const rightThresholdsDbHl = this.frequenciesHz.map(
      (_, i) => this.rightThresholds.get(i) ?? defaultThresholdDbHl
    );
    return {
      frequenciesHz: [...this.frequenciesHz],
      leftThresholdsDbHl,
      rightThresholdsDbHl,
    };
  }

  /** Computes the EQ compensation profile from the recorded session thresholds. */
  computeCompensationProfile(): HearingCompensationProfile {
    return computeHearingCompensation(this.getAudiogram());
  }

  reset(): void {
    this.stopTone();
    this.leftThresholds.clear();
    this.rightThresholds.clear();
  }

  dispose(): void {
    this.reset();
    if (this.ctx) {
      void this.ctx.close().catch(() => {});
      this.ctx = null;
    }
  }
}
