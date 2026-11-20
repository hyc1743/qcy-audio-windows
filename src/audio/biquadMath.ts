/**
 * Pure TypeScript IIR Biquad & Butterworth Parametric EQ Frequency Response Evaluator.
 *
 * Implements RBJ Audio EQ Cookbook formulas plus 1st-order and 4th-order (cascaded 2x2nd-order)
 * topologies covering all 9 QCY Audio IIR filter types (`0..8`) and standard PEQ filter shapes.
 */

export type BiquadFilterType =
  | "high_pass"
  | "low_pass"
  | "peaking"
  | "high_shelf"
  | "low_shelf"
  | "high_shelf_q"
  | "low_shelf_q"
  | "high_pass_advance"
  | "low_pass_advance"
  | "band_pass"
  | "notch"
  | "all_pass"
  | "first_order_low_pass"
  | "first_order_high_pass";

export interface ParametricBand {
  /** Center or cutoff frequency in Hz (typically 20..20000). */
  frequencyHz: number;
  /** Gain in dB (applicable to peaking & shelf filters; ignored by pass/notch/allpass). */
  gainDb: number;
  /** Quality factor Q (typically 0.1..18.0; default ~0.7071 for Butterworth / 1.41 for GEQ). */
  q: number;
  /** IIR filter topology. */
  type: BiquadFilterType;
  /** Whether this band contributes to the composite response (default true). */
  enabled?: boolean;
}

export interface BiquadCoefficients {
  b0: number;
  b1: number;
  b2: number;
  a0: number;
  a1: number;
  a2: number;
}

export interface FrequencyPoint {
  frequencyHz: number;
  gainDb: number;
}

export const DEFAULT_SAMPLE_RATE_HZ = 48_000;
export const MIN_EQ_FREQ_HZ = 20;
export const MAX_EQ_FREQ_HZ = 20_000;

/** QCY wire sentinel (`0xFF`) marking an unused / padding EQ band slot in `0x22`/`0x46`/`0x47`. */
export const QCY_FILTER_TYPE_PAD = 0xff;

export function isQcyPadFilter(index: number): boolean {
  return index === QCY_FILTER_TYPE_PAD;
}

/**
 * Maps QCY Audio `function_*.json` IIR filter type index (`0..8`) to `BiquadFilterType`.
 */
export function qcyFilterTypeFromIndex(index: number): BiquadFilterType {
  switch (index) {
    case 0:
      return "high_pass";
    case 1:
      return "low_pass";
    case 2:
      return "peaking";
    case 3:
      return "high_shelf";
    case 4:
      return "low_shelf";
    case 5:
      return "high_shelf_q";
    case 6:
      return "low_shelf_q";
    case 7:
      return "high_pass_advance";
    case 8:
      return "low_pass_advance";
    default:
      return "peaking";
  }
}

/**
 * Maps a `BiquadFilterType` back to QCY Audio's wire index (`0..8`), if natively representable.
 */
export function qcyFilterTypeToIndex(type: BiquadFilterType): number | null {
  switch (type) {
    case "high_pass":
      return 0;
    case "low_pass":
      return 1;
    case "peaking":
      return 2;
    case "high_shelf":
      return 3;
    case "low_shelf":
      return 4;
    case "high_shelf_q":
      return 5;
    case "low_shelf_q":
      return 6;
    case "high_pass_advance":
      return 7;
    case "low_pass_advance":
      return 8;
    default:
      return null;
  }
}

function clampFrequency(freqHz: number, sampleRateHz: number): number {
  const nyquistSafe = sampleRateHz * 0.495;
  return Math.max(1, Math.min(freqHz, nyquistSafe));
}

function clampQ(q: number): number {
  if (!Number.isFinite(q) || q <= 0) return Math.SQRT1_2;
  return Math.max(0.05, Math.min(q, 40));
}

function computeSingleBiquad(
  type: Exclude<BiquadFilterType, "high_pass_advance" | "low_pass_advance">,
  frequencyHz: number,
  gainDb: number,
  q: number,
  sampleRateHz: number
): BiquadCoefficients {
  const f0 = clampFrequency(frequencyHz, sampleRateHz);
  const safeQ = clampQ(q);
  const w0 = (2 * Math.PI * f0) / sampleRateHz;
  const cosW0 = Math.cos(w0);
  const sinW0 = Math.sin(w0);

  switch (type) {
    case "peaking": {
      const a = Math.pow(10, gainDb / 40);
      const alpha = sinW0 / (2 * safeQ);
      return {
        b0: 1 + alpha * a,
        b1: -2 * cosW0,
        b2: 1 - alpha * a,
        a0: 1 + alpha / a,
        a1: -2 * cosW0,
        a2: 1 - alpha / a,
      };
    }

    case "low_shelf":
    case "low_shelf_q": {
      const a = Math.pow(10, gainDb / 40);
      const alpha =
        type === "low_shelf_q"
          ? sinW0 / (2 * safeQ)
          : (sinW0 / 2) * Math.sqrt((a + 1 / a) * (1 / 1.0 - 1) + 2);
      const twoSqrtAAlpha = 2 * Math.sqrt(a) * alpha;
      return {
        b0: a * (a + 1 - (a - 1) * cosW0 + twoSqrtAAlpha),
        b1: 2 * a * (a - 1 - (a + 1) * cosW0),
        b2: a * (a + 1 - (a - 1) * cosW0 - twoSqrtAAlpha),
        a0: a + 1 + (a - 1) * cosW0 + twoSqrtAAlpha,
        a1: -2 * (a - 1 + (a + 1) * cosW0),
        a2: a + 1 + (a - 1) * cosW0 - twoSqrtAAlpha,
      };
    }

    case "high_shelf":
    case "high_shelf_q": {
      const a = Math.pow(10, gainDb / 40);
      const alpha =
        type === "high_shelf_q"
          ? sinW0 / (2 * safeQ)
          : (sinW0 / 2) * Math.sqrt((a + 1 / a) * (1 / 1.0 - 1) + 2);
      const twoSqrtAAlpha = 2 * Math.sqrt(a) * alpha;
      return {
        b0: a * (a + 1 + (a - 1) * cosW0 + twoSqrtAAlpha),
        b1: -2 * a * (a - 1 + (a + 1) * cosW0),
        b2: a * (a + 1 + (a - 1) * cosW0 - twoSqrtAAlpha),
        a0: a + 1 - (a - 1) * cosW0 + twoSqrtAAlpha,
        a1: 2 * (a - 1 - (a + 1) * cosW0),
        a2: a + 1 - (a - 1) * cosW0 - twoSqrtAAlpha,
      };
    }

    case "low_pass": {
      const alpha = sinW0 / (2 * safeQ);
      return {
        b0: (1 - cosW0) / 2,
        b1: 1 - cosW0,
        b2: (1 - cosW0) / 2,
        a0: 1 + alpha,
        a1: -2 * cosW0,
        a2: 1 - alpha,
      };
    }

    case "high_pass": {
      const alpha = sinW0 / (2 * safeQ);
      return {
        b0: (1 + cosW0) / 2,
        b1: -(1 + cosW0),
        b2: (1 + cosW0) / 2,
        a0: 1 + alpha,
        a1: -2 * cosW0,
        a2: 1 - alpha,
      };
    }

    case "band_pass": {
      // Constant 0 dB peak gain bandpass (RBJ cookbook).
      const alpha = sinW0 / (2 * safeQ);
      return {
        b0: alpha,
        b1: 0,
        b2: -alpha,
        a0: 1 + alpha,
        a1: -2 * cosW0,
        a2: 1 - alpha,
      };
    }

    case "notch": {
      const alpha = sinW0 / (2 * safeQ);
      return {
        b0: 1,
        b1: -2 * cosW0,
        b2: 1,
        a0: 1 + alpha,
        a1: -2 * cosW0,
        a2: 1 - alpha,
      };
    }

    case "all_pass": {
      const alpha = sinW0 / (2 * safeQ);
      return {
        b0: 1 - alpha,
        b1: -2 * cosW0,
        b2: 1 + alpha,
        a0: 1 + alpha,
        a1: -2 * cosW0,
        a2: 1 - alpha,
      };
    }

    case "first_order_low_pass": {
      // Bilinear transform of H(s) = wc / (s + wc)
      const k = Math.tan(w0 / 2);
      return {
        b0: k,
        b1: k,
        b2: 0,
        a0: k + 1,
        a1: k - 1,
        a2: 0,
      };
    }

    case "first_order_high_pass": {
      // Bilinear transform of H(s) = s / (s + wc)
      const k = Math.tan(w0 / 2);
      return {
        b0: 1,
        b1: -1,
        b2: 0,
        a0: k + 1,
        a1: k - 1,
        a2: 0,
      };
    }
  }
}

/**
 * Computes one or two cascaded biquad stages for a parametric EQ band.
 * `high_pass_advance` / `low_pass_advance` use a 4th-order Butterworth cascade (Q1=0.5412, Q2=1.3066).
 */
export function computeBandStages(
  band: ParametricBand,
  sampleRateHz: number = DEFAULT_SAMPLE_RATE_HZ
): BiquadCoefficients[] {
  if (band.enabled === false) {
    return [];
  }

  if (band.type === "high_pass_advance") {
    return [
      computeSingleBiquad("high_pass", band.frequencyHz, 0, 0.5411961, sampleRateHz),
      computeSingleBiquad("high_pass", band.frequencyHz, 0, 1.306563, sampleRateHz),
    ];
  }

  if (band.type === "low_pass_advance") {
    return [
      computeSingleBiquad("low_pass", band.frequencyHz, 0, 0.5411961, sampleRateHz),
      computeSingleBiquad("low_pass", band.frequencyHz, 0, 1.306563, sampleRateHz),
    ];
  }

  return [
    computeSingleBiquad(
      band.type,
      band.frequencyHz,
      band.gainDb,
      band.q,
      sampleRateHz
    ),
  ];
}

/**
 * Evaluates squared magnitude `|H(e^{jw})|^2` of a single biquad stage at `evalFreqHz`.
 */
export function evaluateBiquadMagnitudeSquared(
  c: BiquadCoefficients,
  evalFreqHz: number,
  sampleRateHz: number = DEFAULT_SAMPLE_RATE_HZ
): number {
  const w = (2 * Math.PI * evalFreqHz) / sampleRateHz;
  const cos1 = Math.cos(w);
  const sin1 = Math.sin(w);
  const cos2 = Math.cos(2 * w);
  const sin2 = Math.sin(2 * w);

  const numRe = c.b0 + c.b1 * cos1 + c.b2 * cos2;
  const numIm = -(c.b1 * sin1 + c.b2 * sin2);
  const denRe = c.a0 + c.a1 * cos1 + c.a2 * cos2;
  const denIm = -(c.a1 * sin1 + c.a2 * sin2);

  const numMagSq = numRe * numRe + numIm * numIm;
  const denMagSq = denRe * denRe + denIm * denIm;
  if (denMagSq <= 1e-30) return 0;
  return numMagSq / denMagSq;
}

/**
 * Evaluates the gain in dB of a single `ParametricBand` at `evalFreqHz`.
 */
export function evaluateBandMagnitudeDb(
  band: ParametricBand,
  evalFreqHz: number,
  sampleRateHz: number = DEFAULT_SAMPLE_RATE_HZ
): number {
  const stages = computeBandStages(band, sampleRateHz);
  if (stages.length === 0) return 0;
  let magSq = 1;
  for (const stage of stages) {
    magSq *= evaluateBiquadMagnitudeSquared(stage, evalFreqHz, sampleRateHz);
  }
  return 10 * Math.log10(Math.max(magSq, 1e-12));
}

/**
 * Generates logarithmically spaced frequencies between `minFreqHz` and `maxFreqHz`.
 */
export function generateLogFrequencyGrid(
  pointCount: number = 200,
  minFreqHz: number = MIN_EQ_FREQ_HZ,
  maxFreqHz: number = MAX_EQ_FREQ_HZ
): number[] {
  const count = Math.max(2, Math.floor(pointCount));
  const logMin = Math.log10(Math.max(1, minFreqHz));
  const logMax = Math.log10(Math.max(minFreqHz + 1, maxFreqHz));
  const freqs: number[] = new Array(count);
  for (let i = 0; i < count; i++) {
    const t = i / (count - 1);
    freqs[i] = Math.pow(10, logMin + t * (logMax - logMin));
  }
  return freqs;
}

/**
 * Evaluates the composite frequency response (in dB) of a cascade of parametric EQ bands
 * plus optional `masterGainDb` across `frequenciesHz`.
 */
export function evaluateCompositeCurve(
  bands: readonly ParametricBand[],
  frequenciesHz: readonly number[] = generateLogFrequencyGrid(200),
  masterGainDb: number = 0,
  sampleRateHz: number = DEFAULT_SAMPLE_RATE_HZ
): FrequencyPoint[] {
  const allStages: BiquadCoefficients[] = [];
  for (const band of bands) {
    const stages = computeBandStages(band, sampleRateHz);
    for (const st of stages) {
      allStages.push(st);
    }
  }

  return frequenciesHz.map((freq) => {
    let magSq = 1;
    for (const stage of allStages) {
      magSq *= evaluateBiquadMagnitudeSquared(stage, freq, sampleRateHz);
    }
    const db = 10 * Math.log10(Math.max(magSq, 1e-12)) + masterGainDb;
    return {
      frequencyHz: freq,
      gainDb: Number.isFinite(db) ? db : -120,
    };
  });
}

/**
 * Convenience builder turning a 10-band graphic EQ gain array into standard peaking `ParametricBand`s.
 */
export function graphicBandsToParametric(
  frequenciesHz: readonly number[],
  gainsDb: readonly number[],
  q: number = 1.414
): ParametricBand[] {
  return frequenciesHz.map((freq, idx) => ({
    frequencyHz: freq,
    gainDb: gainsDb[idx] ?? 0,
    q,
    type: "peaking" as const,
    enabled: true,
  }));
}
