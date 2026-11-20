/**
 * Utility audio helpers for Ear-Tip Fit Test probes, local Find-My-Earbuds beacon chimes,
 * and safe Web Audio context creation (gracefully falling back in headless/Node test runtimes).
 */

export type StereoSide = "left" | "right" | "both";

export interface PlaybackHandle {
  stop: () => void;
  isActive: () => boolean;
}

/**
 * Safely instantiates a Web Audio `AudioContext`, returning `null` in non-browser / headless
 * environments (e.g., Node/Vitest) without throwing.
 */
export function createAudioContext(): AudioContext | null {
  if (typeof globalThis === "undefined") return null;
  const g = globalThis as unknown as {
    AudioContext?: typeof AudioContext;
    webkitAudioContext?: typeof AudioContext;
  };
  const Ctor = g.AudioContext ?? g.webkitAudioContext;
  if (typeof Ctor !== "function") return null;
  try {
    return new Ctor();
  } catch {
    return null;
  }
}

/** Returns whether Web Audio playback is available in the current runtime. */
export function isWebAudioSupported(): boolean {
  if (typeof globalThis === "undefined") return false;
  const g = globalThis as unknown as {
    AudioContext?: unknown;
    webkitAudioContext?: unknown;
  };
  return typeof g.AudioContext === "function" || typeof g.webkitAudioContext === "function";
}

/**
 * Plays a gentle dual-ear acoustic seal probe sweep (200 Hz -> 1600 Hz + harmonic reference)
 * used during the Ear-Tip Fit Test. Safe no-op in headless environments.
 */
export function playFitTestProbe(durationMs: number = 2200): PlaybackHandle {
  let active = false;
  const ctx = createAudioContext();
  if (!ctx) {
    return {
      stop: () => {},
      isActive: () => false,
    };
  }

  active = true;
  const now = ctx.currentTime;
  const durationSec = Math.max(0.4, durationMs / 1000);

  const sweepOsc = ctx.createOscillator();
  sweepOsc.type = "sine";
  sweepOsc.frequency.setValueAtTime(180, now);
  sweepOsc.frequency.exponentialRampToValueAtTime(1800, now + durationSec * 0.85);

  const lowRefOsc = ctx.createOscillator();
  lowRefOsc.type = "triangle";
  lowRefOsc.frequency.setValueAtTime(110, now);

  const master = ctx.createGain();
  master.gain.setValueAtTime(0.0001, now);
  master.gain.linearRampToValueAtTime(0.14, now + 0.12);
  master.gain.setValueAtTime(0.14, now + durationSec - 0.18);
  master.gain.linearRampToValueAtTime(0.0001, now + durationSec);

  const lowGain = ctx.createGain();
  lowGain.gain.setValueAtTime(0.35, now);

  sweepOsc.connect(master);
  lowRefOsc.connect(lowGain);
  lowGain.connect(master);
  master.connect(ctx.destination);

  sweepOsc.start(now);
  lowRefOsc.start(now);
  sweepOsc.stop(now + durationSec + 0.05);
  lowRefOsc.stop(now + durationSec + 0.05);

  const timer = setTimeout(() => {
    active = false;
    void ctx.close().catch(() => {});
  }, Math.ceil((durationSec + 0.1) * 1000));

  return {
    stop: () => {
      if (!active) return;
      active = false;
      clearTimeout(timer);
      try {
        sweepOsc.stop();
        lowRefOsc.stop();
      } catch {
        // already stopped
      }
      void ctx.close().catch(() => {});
    },
    isActive: () => active,
  };
}

export interface FindEarbudsChimeOptions {
  side?: StereoSide;
  /** Number of dual-beep chirp pulses (default 6). */
  pulses?: number;
  /** Safe peak digital gain with progressive ramp-up (default 0.25, capped at 0.45). */
  maxGain?: number;
}

/**
 * Plays a progressive ramp-up high-audibility locating chime (2800 Hz / 3520 Hz) panned to
 * Left, Right, or Both channels for local Find-My-Earbuds testing.
 */
export function playFindEarbudsChime(options: FindEarbudsChimeOptions = {}): PlaybackHandle {
  let active = false;
  const ctx = createAudioContext();
  if (!ctx) {
    return {
      stop: () => {},
      isActive: () => false,
    };
  }

  active = true;
  const side = options.side ?? "both";
  const pulses = Math.max(1, Math.min(20, Math.floor(options.pulses ?? 6)));
  const maxGain = Math.max(0.05, Math.min(0.45, options.maxGain ?? 0.25));
  const pulseIntervalSec = 0.55;
  const now = ctx.currentTime;
  const oscillators: OscillatorNode[] = [];

  const panNode =
    typeof ctx.createStereoPanner === "function" ? ctx.createStereoPanner() : null;
  if (panNode) {
    const pan = side === "left" ? -1 : side === "right" ? 1 : 0;
    panNode.pan.setValueAtTime(pan, now);
    panNode.connect(ctx.destination);
  }
  const dest: AudioNode = panNode ?? ctx.destination;

  for (let i = 0; i < pulses; i++) {
    const t0 = now + i * pulseIntervalSec;
    // Gentle progressive ramp across the first 4 pulses to avoid startling users.
    const levelScale = Math.min(1, 0.35 + (i / 3) * 0.65);
    const pulseGain = maxGain * levelScale;

    for (const [offset, freq] of [
      [0.0, 2800],
      [0.14, 3520],
    ] as const) {
      const osc = ctx.createOscillator();
      osc.type = "sine";
      osc.frequency.setValueAtTime(freq, t0 + offset);

      const env = ctx.createGain();
      env.gain.setValueAtTime(0.0001, t0 + offset);
      env.gain.linearRampToValueAtTime(pulseGain, t0 + offset + 0.015);
      env.gain.setValueAtTime(pulseGain, t0 + offset + 0.095);
      env.gain.linearRampToValueAtTime(0.0001, t0 + offset + 0.12);

      osc.connect(env);
      env.connect(dest);
      osc.start(t0 + offset);
      osc.stop(t0 + offset + 0.13);
      oscillators.push(osc);
    }
  }

  const totalMs = Math.ceil((pulses * pulseIntervalSec + 0.2) * 1000);
  const timer = setTimeout(() => {
    active = false;
    void ctx.close().catch(() => {});
  }, totalMs);

  return {
    stop: () => {
      if (!active) return;
      active = false;
      clearTimeout(timer);
      for (const osc of oscillators) {
        try {
          osc.stop();
        } catch {
          // already stopped
        }
      }
      void ctx.close().catch(() => {});
    },
    isActive: () => active,
  };
}
