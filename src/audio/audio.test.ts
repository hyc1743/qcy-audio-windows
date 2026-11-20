import { describe, expect, it } from "vitest";
import {
  computeBandStages,
  evaluateBandMagnitudeDb,
  evaluateCompositeCurve,
  generateLogFrequencyGrid,
  graphicBandsToParametric,
  isQcyPadFilter,
  qcyFilterTypeFromIndex,
  qcyFilterTypeToIndex,
  QCY_FILTER_TYPE_PAD,
  type ParametricBand,
} from "./biquadMath";
import {
  computeHearingCompensation,
  dbHlToSafeLinearGain,
  HEARING_TEST_FREQUENCIES_HZ,
  HearingTestEngine,
  projectCurveLogFreq,
  STANDARD_10_BAND_FREQUENCIES_HZ,
} from "./hearingTestEngine";
import {
  computeSleepTimerFadeMultiplier,
  SOUNDSCAPE_CHANNELS,
  SOUNDSCAPE_PRESETS,
  SoundscapeEngine,
} from "./soundscapeEngine";
import {
  createAudioContext,
  isWebAudioSupported,
  playFindEarbudsChime,
  playFitTestProbe,
} from "./audioUtils";
import {
  computeDevicePalette,
  DEVICE_COLOR_PRESETS,
  resolveDeviceColorPreset,
} from "../assets/DeviceIllustrations";

describe("biquadMath — RBJ & QCY 9-type IIR filter response", () => {
  it("maps all 9 QCY wire filter types (0..8) and 0xFF PAD sentinel bidirectionally", () => {
    const expected = [
      "high_pass",
      "low_pass",
      "peaking",
      "high_shelf",
      "low_shelf",
      "high_shelf_q",
      "low_shelf_q",
      "high_pass_advance",
      "low_pass_advance",
    ] as const;

    expected.forEach((name, idx) => {
      expect(qcyFilterTypeFromIndex(idx)).toBe(name);
      expect(qcyFilterTypeToIndex(name)).toBe(idx);
    });
    expect(isQcyPadFilter(QCY_FILTER_TYPE_PAD)).toBe(true);
    expect(isQcyPadFilter(2)).toBe(false);
    expect(qcyFilterTypeToIndex("notch")).toBeNull();
  });

  it("evaluates peaking EQ accurately at center frequency and far asymptotes", () => {
    const band: ParametricBand = {
      frequencyHz: 1000,
      gainDb: 6.0,
      q: 1.414,
      type: "peaking",
    };

    expect(evaluateBandMagnitudeDb(band, 1000)).toBeCloseTo(6.0, 2);
    expect(Math.abs(evaluateBandMagnitudeDb(band, 25))).toBeLessThan(0.05);
    expect(Math.abs(evaluateBandMagnitudeDb(band, 18_000))).toBeLessThan(0.05);

    const cutBand: ParametricBand = { ...band, gainDb: -8.5 };
    expect(evaluateBandMagnitudeDb(cutBand, 1000)).toBeCloseTo(-8.5, 2);
  });

  it("evaluates low-shelf and high-shelf (standard & variable-Q) asymptotes", () => {
    const lowShelf: ParametricBand = {
      frequencyHz: 200,
      gainDb: 5.0,
      q: 0.707,
      type: "low_shelf",
    };
    expect(evaluateBandMagnitudeDb(lowShelf, 20)).toBeCloseTo(5.0, 1);
    expect(evaluateBandMagnitudeDb(lowShelf, 200)).toBeCloseTo(2.5, 1);
    expect(Math.abs(evaluateBandMagnitudeDb(lowShelf, 10_000))).toBeLessThan(0.05);

    const highShelf: ParametricBand = {
      frequencyHz: 4000,
      gainDb: -4.0,
      q: 0.707,
      type: "high_shelf",
    };
    expect(Math.abs(evaluateBandMagnitudeDb(highShelf, 50))).toBeLessThan(0.05);
    expect(evaluateBandMagnitudeDb(highShelf, 4000)).toBeCloseTo(-2.0, 1);
    expect(evaluateBandMagnitudeDb(highShelf, 19_000)).toBeCloseTo(-4.0, 0);

    const highShelfQ: ParametricBand = { ...highShelf, type: "high_shelf_q", q: 1.2 };
    expect(evaluateBandMagnitudeDb(highShelfQ, 19_000)).toBeCloseTo(-4.0, 0);
  });

  it("evaluates low-pass, high-pass, first-order, and 4th-order advance Butterworth rolloff", () => {
    const lp2: ParametricBand = { frequencyHz: 1000, gainDb: 0, q: Math.SQRT1_2, type: "low_pass" };
    const lp4: ParametricBand = { frequencyHz: 1000, gainDb: 0, q: Math.SQRT1_2, type: "low_pass_advance" };
    const lp1: ParametricBand = { frequencyHz: 1000, gainDb: 0, q: 0.7, type: "first_order_low_pass" };

    expect(evaluateBandMagnitudeDb(lp2, 50)).toBeCloseTo(0, 1);
    expect(evaluateBandMagnitudeDb(lp2, 1000)).toBeCloseTo(-3.01, 1);
    expect(evaluateBandMagnitudeDb(lp1, 1000)).toBeCloseTo(-3.01, 1);
    expect(evaluateBandMagnitudeDb(lp4, 1000)).toBeCloseTo(-3.01, 1);

    // 4th-order Butterworth must attenuate much more steeply than 2nd-order, and 2nd more than 1st
    const db1At4k = evaluateBandMagnitudeDb(lp1, 4000);
    const db2At4k = evaluateBandMagnitudeDb(lp2, 4000);
    const db4At4k = evaluateBandMagnitudeDb(lp4, 4000);
    expect(db4At4k).toBeLessThan(db2At4k - 10);
    expect(db2At4k).toBeLessThan(db1At4k - 6);

    const hp2: ParametricBand = { frequencyHz: 500, gainDb: 0, q: Math.SQRT1_2, type: "high_pass" };
    const hp1: ParametricBand = { frequencyHz: 500, gainDb: 0, q: 0.7, type: "first_order_high_pass" };
    const hp4: ParametricBand = { frequencyHz: 500, gainDb: 0, q: Math.SQRT1_2, type: "high_pass_advance" };
    expect(evaluateBandMagnitudeDb(hp2, 500)).toBeCloseTo(-3.01, 1);
    expect(evaluateBandMagnitudeDb(hp1, 500)).toBeCloseTo(-3.01, 1);
    expect(evaluateBandMagnitudeDb(hp4, 500)).toBeCloseTo(-3.01, 1);
    expect(evaluateBandMagnitudeDb(hp4, 125)).toBeLessThan(evaluateBandMagnitudeDb(hp2, 125) - 10);
    expect(computeBandStages(hp4)).toHaveLength(2);
  });

  it("evaluates bandpass, notch attenuation, and flat 0 dB all-pass", () => {
    const bp: ParametricBand = { frequencyHz: 1000, gainDb: 0, q: 2.0, type: "band_pass" };
    expect(evaluateBandMagnitudeDb(bp, 1000)).toBeCloseTo(0, 2);
    expect(evaluateBandMagnitudeDb(bp, 100)).toBeLessThan(-20);
    expect(evaluateBandMagnitudeDb(bp, 10_000)).toBeLessThan(-20);

    const notch: ParametricBand = { frequencyHz: 2000, gainDb: 0, q: 4.0, type: "notch" };
    expect(evaluateBandMagnitudeDb(notch, 2000)).toBeLessThan(-50);
    expect(evaluateBandMagnitudeDb(notch, 200)).toBeCloseTo(0, 1);
    expect(evaluateBandMagnitudeDb(notch, 12_000)).toBeCloseTo(0, 1);

    const ap: ParametricBand = { frequencyHz: 1500, gainDb: 0, q: 1.0, type: "all_pass" };
    for (const f of [30, 250, 1500, 6000, 18_000]) {
      expect(evaluateBandMagnitudeDb(ap, f)).toBeCloseTo(0, 3);
    }
  });

  it("cascades multiple bands + masterGainDb and skips disabled bands", () => {
    const bands: ParametricBand[] = [
      { frequencyHz: 100, gainDb: 4, q: 1.4, type: "peaking", enabled: true },
      { frequencyHz: 4000, gainDb: -3, q: 1.4, type: "peaking", enabled: true },
      { frequencyHz: 1000, gainDb: 10, q: 1.4, type: "peaking", enabled: false },
    ];
    const curve = evaluateCompositeCurve(bands, [100, 1000, 4000], -2);
    const withoutDisabled = evaluateCompositeCurve(bands.slice(0, 2), [100, 1000, 4000], -2);
    expect(curve[0].gainDb).toBeCloseTo(2.0, 1); // +4 - 2
    expect(curve[1].gainDb).toBeCloseTo(withoutDisabled[1].gainDb, 6); // disabled 1kHz band contributes 0
    expect(curve[1].gainDb).toBeCloseTo(-2.0, 0); // masterGain (-2) + tiny 4kHz skirt (-0.08 dB)
    expect(curve[2].gainDb).toBeCloseTo(-5.0, 1); // -3 - 2

    const grid = generateLogFrequencyGrid(120, 20, 20_000);
    expect(grid).toHaveLength(120);
    expect(grid[0]).toBeCloseTo(20, 3);
    expect(grid[grid.length - 1]).toBeCloseTo(20_000, 1);

    const geq = graphicBandsToParametric([31, 62, 125], [2, 0, -1]);
    expect(geq).toHaveLength(3);
    expect(geq[0].gainDb).toBe(2);
  });
});

describe("hearingTestEngine — audiometry math & 10-band projection", () => {
  it("converts dB HL monotonically to safe capped linear gains", () => {
    const quiet = dbHlToSafeLinearGain(0);
    const mid = dbHlToSafeLinearGain(40);
    const loud = dbHlToSafeLinearGain(80);
    const over = dbHlToSafeLinearGain(120);

    expect(quiet).toBeGreaterThan(0);
    expect(mid).toBeGreaterThan(quiet);
    expect(loud).toBeGreaterThan(mid);
    expect(loud).toBeLessThanOrEqual(0.35);
    expect(over).toBeCloseTo(loud, 6);
  });

  it("maps flat audiogram to flat 0 dB compensation and 0 balance", () => {
    const flat = computeHearingCompensation({
      frequenciesHz: [...HEARING_TEST_FREQUENCIES_HZ],
      leftThresholdsDbHl: [20, 20, 20, 20, 20, 20, 20, 20],
      rightThresholdsDbHl: [20, 20, 20, 20, 20, 20, 20, 20],
    });

    expect(flat.combinedCompensationDb.every((g) => g === 0)).toBe(true);
    expect(flat.tenBandGainsDb).toHaveLength(10);
    expect(flat.tenBandGainsDb.every((g) => g === 0)).toBe(true);
    expect(flat.suggestedBalance).toBe(0);
  });

  it("compensates high-frequency hearing loss and computes asymmetric L/R balance", () => {
    // Classic high-frequency roll-off, with right ear 10 dB less sensitive than left
    const profile = computeHearingCompensation({
      frequenciesHz: [...HEARING_TEST_FREQUENCIES_HZ],
      leftThresholdsDbHl: [10, 10, 15, 20, 25, 40, 50, 55],
      rightThresholdsDbHl: [20, 20, 25, 30, 35, 50, 60, 65],
    });

    // High frequencies (8 kHz) should receive boost relative to low frequencies (125 Hz)
    expect(profile.combinedCompensationDb[7]).toBeGreaterThan(profile.combinedCompensationDb[0]);
    expect(profile.combinedCompensationDb[7]).toBeLessThanOrEqual(6.0);
    expect(profile.combinedCompensationDb[0]).toBeGreaterThanOrEqual(-6.0);

    // 10-band projection preserves trend from sub-bass (31 Hz) to treble (16 kHz)
    expect(profile.tenBandFrequenciesHz).toEqual([...STANDARD_10_BAND_FREQUENCIES_HZ]);
    expect(profile.tenBandGainsDb[9]).toBeGreaterThan(profile.tenBandGainsDb[0]);

    // Right ear needs more loudness -> positive balance offset
    expect(profile.suggestedBalance).toBeGreaterThan(0);
    expect(profile.suggestedBalance).toBeLessThanOrEqual(0.5);
  });

  it("projects curves in log-frequency space smoothly and supports headless engine sessions", () => {
    const projected = projectCurveLogFreq([100, 1000], [0, 6], [50, 100, 316.227766, 1000, 4000]);
    expect(projected[0]).toBe(0); // boundary clamp
    expect(projected[1]).toBe(0);
    expect(projected[2]).toBeCloseTo(3.0, 1); // exact log-midpoint of 100..1000 Hz
    expect(projected[3]).toBe(6);
    expect(projected[4]).toBe(6); // boundary clamp

    const engine = new HearingTestEngine();
    expect(engine.stepThreshold(20, 2)).toBe(30);
    expect(engine.stepThreshold(75, 4)).toBe(80);
    expect(engine.stepThreshold(-5, -3)).toBe(-10);

    engine.recordThreshold("left", 0, 15);
    engine.recordThreshold("left", 7, 45);
    engine.recordThreshold("right", 0, 15);
    engine.recordThreshold("right", 7, 45);
    expect(engine.playTone({ frequencyHz: 1000, side: "left", levelDbHl: 25 })).toBe(false);

    const comp = engine.computeCompensationProfile();
    expect(comp.combinedCompensationDb[7]).toBeGreaterThan(comp.combinedCompensationDb[0]);
    engine.dispose();
  });
});

describe("soundscapeEngine — 8 channels, presets & sleep fade-out timer", () => {
  it("exposes 8 procedural channels and 4 curated preset scenes", () => {
    expect(SOUNDSCAPE_CHANNELS).toHaveLength(8);
    expect(SOUNDSCAPE_PRESETS.map((p) => p.id)).toEqual([
      "deep_focus",
      "rainy_night",
      "coastal_calm",
      "coffee_free_study",
    ]);
  });

  it("manages channel volumes, mutes, presets, and sleep timer fade-out cleanly", () => {
    const engine = new SoundscapeEngine();
    expect(engine.getSnapshot().isPlaying).toBe(false);
    expect(engine.getSnapshot().effectiveMasterGain).toBe(0);

    expect(engine.applyPreset("rainy_night")).toBe(true);
    let snap = engine.getSnapshot();
    expect(snap.activePresetId).toBe("rainy_night");
    expect(snap.channels.rain.volume).toBeCloseTo(0.65, 2);
    expect(snap.channels.fireplace.volume).toBeCloseTo(0.32, 2);
    expect(snap.channels.ocean_waves.volume).toBe(0);

    engine.play();
    engine.setMasterVolume(0.8);
    expect(engine.getSnapshot().effectiveMasterGain).toBeCloseTo(0.8, 3);

    // Editing a channel detaches activePresetId and clamps to [0, 1]
    engine.setChannelVolume("rain", 1.5);
    expect(engine.getSnapshot().channels.rain.volume).toBe(1);
    expect(engine.getSnapshot().activePresetId).toBeNull();

    expect(engine.toggleChannelMute("rain")).toBe(true);
    expect(engine.getSnapshot().channels.rain.muted).toBe(true);

    // Sleep timer: 60s total with 20s smooth cosine fade-out
    engine.startSleepTimer(60, 20);
    expect(engine.getSnapshot().effectiveMasterGain).toBeCloseTo(0.8, 3);

    engine.tickTimer(40); // 20s left -> right at start of fade window
    expect(engine.getSnapshot().sleepTimer.remainingSeconds).toBe(20);
    expect(engine.getSnapshot().effectiveMasterGain).toBeCloseTo(0.8, 3);

    engine.tickTimer(10); // 10s left -> cosine midpoint = 0.5x master
    expect(engine.getSnapshot().sleepTimer.remainingSeconds).toBe(10);
    expect(engine.getSnapshot().effectiveMasterGain).toBeCloseTo(0.4, 2);

    engine.tickTimer(10); // 0s left -> auto-stops playback cleanly
    snap = engine.getSnapshot();
    expect(snap.isPlaying).toBe(false);
    expect(snap.sleepTimer.active).toBe(false);
    expect(snap.effectiveMasterGain).toBe(0);

    // Direct fade helper check
    expect(
      computeSleepTimerFadeMultiplier({
        active: true,
        totalSeconds: 30,
        remainingSeconds: 15,
        fadeOutSeconds: 30,
      })
    ).toBeCloseTo(0.5, 4);

    engine.dispose();
  });
});

describe("audioUtils & DeviceIllustrations helpers", () => {
  it("falls back gracefully in headless Node environment without AudioContext", () => {
    expect(isWebAudioSupported()).toBe(false);
    expect(createAudioContext()).toBeNull();

    const fit = playFitTestProbe(1000);
    expect(fit.isActive()).toBe(false);
    expect(() => fit.stop()).not.toThrow();

    const find = playFindEarbudsChime({ side: "left", pulses: 2 });
    expect(find.isActive()).toBe(false);
    expect(() => find.stop()).not.toThrow();
  });

  it("derives valid device illustration color palettes for light/dark finishes", () => {
    expect(DEVICE_COLOR_PRESETS.length).toBeGreaterThanOrEqual(5);
    const white = resolveDeviceColorPreset(1);
    expect(white.id).toBe("pearl_white");

    const lightPal = computeDevicePalette("#ECEFF4", "light");
    expect(lightPal.isLightBody).toBe(true);
    expect(lightPal.highlight).toMatch(/^#[0-9a-f]{6}$/);

    const darkPal = computeDevicePalette("#2A2E35", "dark");
    expect(darkPal.isLightBody).toBe(false);
    expect(darkPal.shadow).toMatch(/^#[0-9a-f]{6}$/);
  });
});
