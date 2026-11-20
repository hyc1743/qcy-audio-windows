/**
 * Pure Web Audio API Procedural Ambient Soundscape Synthesizer (Focus Mode / White Noise).
 *
 * Generates 8 continuous, seamlessly looping procedural ambient tracks in real time without
 * any copyrighted MP3/WAV samples:
 *   1. White Noise       2. Pink Noise (-3 dB/oct)   3. Brown Noise (-6 dB/oct)
 *   4. Rain              5. Ocean Waves              6. Forest Wind
 *   7. Fireplace         8. Deep Focus Drone
 *
 * Features per-channel volume & mute, master volume, 4 curated scenes, click-free exponential/
 * target gain ramping, and a sleep countdown timer with smooth fade-out.
 * Fully functional as a pure state controller in headless Node/Vitest when `AudioContext` is absent.
 */

import { createAudioContext } from "./audioUtils";

export type SoundscapeChannelId =
  | "white_noise"
  | "pink_noise"
  | "brown_noise"
  | "rain"
  | "ocean_waves"
  | "forest_wind"
  | "fireplace"
  | "deep_focus_drone";

export type SoundscapePresetId =
  | "deep_focus"
  | "rainy_night"
  | "coastal_calm"
  | "coffee_free_study";

export interface SoundscapeChannelMeta {
  id: SoundscapeChannelId;
  nameEn: string;
  nameZh: string;
  defaultVolume: number;
}

export interface SoundscapeChannelState {
  id: SoundscapeChannelId;
  volume: number; // 0..1
  muted: boolean;
}

export interface SoundscapePreset {
  id: SoundscapePresetId;
  nameEn: string;
  nameZh: string;
  channelVolumes: Partial<Record<SoundscapeChannelId, number>>;
}

export interface SleepTimerState {
  active: boolean;
  totalSeconds: number;
  remainingSeconds: number;
  fadeOutSeconds: number;
}

export interface SoundscapeSnapshot {
  isPlaying: boolean;
  masterVolume: number;
  effectiveMasterGain: number;
  activePresetId: SoundscapePresetId | null;
  channels: Record<SoundscapeChannelId, SoundscapeChannelState>;
  sleepTimer: SleepTimerState;
}

export const SOUNDSCAPE_CHANNELS: readonly SoundscapeChannelMeta[] = [
  { id: "white_noise", nameEn: "White Noise", nameZh: "白噪音", defaultVolume: 0 },
  { id: "pink_noise", nameEn: "Pink Noise", nameZh: "粉红噪音", defaultVolume: 0.35 },
  { id: "brown_noise", nameEn: "Brown Noise", nameZh: "布朗噪音", defaultVolume: 0 },
  { id: "rain", nameEn: "Rain", nameZh: "细雨", defaultVolume: 0.45 },
  { id: "ocean_waves", nameEn: "Ocean Waves", nameZh: "海浪", defaultVolume: 0 },
  { id: "forest_wind", nameEn: "Forest Wind", nameZh: "林间微风", defaultVolume: 0 },
  { id: "fireplace", nameEn: "Fireplace", nameZh: "壁炉柴火", defaultVolume: 0 },
  { id: "deep_focus_drone", nameEn: "Deep Focus Drone", nameZh: "深度专注低鸣", defaultVolume: 0 },
];

export const SOUNDSCAPE_PRESETS: readonly SoundscapePreset[] = [
  {
    id: "deep_focus",
    nameEn: "Deep Focus",
    nameZh: "深度专注",
    channelVolumes: {
      brown_noise: 0.42,
      pink_noise: 0.25,
      deep_focus_drone: 0.3,
    },
  },
  {
    id: "rainy_night",
    nameEn: "Rainy Night",
    nameZh: "雨夜助眠",
    channelVolumes: {
      rain: 0.65,
      fireplace: 0.32,
      brown_noise: 0.18,
    },
  },
  {
    id: "coastal_calm",
    nameEn: "Coastal Calm",
    nameZh: "海岸舒缓",
    channelVolumes: {
      ocean_waves: 0.68,
      forest_wind: 0.26,
      pink_noise: 0.14,
    },
  },
  {
    id: "coffee_free_study",
    nameEn: "Coffee-Free Study",
    nameZh: "清静自习",
    channelVolumes: {
      white_noise: 0.16,
      rain: 0.36,
      forest_wind: 0.28,
      deep_focus_drone: 0.2,
    },
  },
];

function clamp01(val: number): number {
  if (!Number.isFinite(val)) return 0;
  return Math.max(0, Math.min(1, val));
}

/**
 * Computes the effective sleep-timer fade multiplier in `[0, 1]`.
 * Returns `1.0` when timer is inactive or outside the fade-out window, smoothly decaying
 * to `0.0` (cosine-eased) over the final `fadeOutSeconds`.
 */
export function computeSleepTimerFadeMultiplier(timer: SleepTimerState): number {
  if (!timer.active || timer.totalSeconds <= 0) return 1;
  if (timer.remainingSeconds <= 0) return 0;
  const fadeWindow = Math.max(1, Math.min(timer.fadeOutSeconds, timer.totalSeconds));
  if (timer.remainingSeconds >= fadeWindow) return 1;
  const ratio = clamp01(timer.remainingSeconds / fadeWindow);
  // Smooth half-cosine curve for natural auditory fade-out.
  return 0.5 - 0.5 * Math.cos(ratio * Math.PI);
}

interface ChannelGraph {
  gain: GainNode;
  cleanup: () => void;
}

/**
 * Generates deterministic-quality procedural PCM noise buffers for seamless looping.
 */
function createProceduralBuffer(
  ctx: AudioContext,
  kind: "white" | "pink" | "brown" | "rain_drops" | "crackle",
  durationSec: number = 4
): AudioBuffer {
  const sampleRate = ctx.sampleRate || 48_000;
  const length = Math.max(1024, Math.floor(sampleRate * durationSec));
  const buffer = ctx.createBuffer(2, length, sampleRate);

  for (let ch = 0; ch < 2; ch++) {
    const out = buffer.getChannelData(ch);

    if (kind === "white") {
      for (let i = 0; i < length; i++) {
        out[i] = (Math.random() * 2 - 1) * 0.35;
      }
    } else if (kind === "pink") {
      // Paul Kellet's 7-stage Voss-McCartney pink filter (-3 dB/octave)
      let b0 = 0;
      let b1 = 0;
      let b2 = 0;
      let b3 = 0;
      let b4 = 0;
      let b5 = 0;
      let b6 = 0;
      for (let i = 0; i < length; i++) {
        const white = Math.random() * 2 - 1;
        b0 = 0.99886 * b0 + white * 0.0555179;
        b1 = 0.99332 * b1 + white * 0.0750759;
        b2 = 0.969 * b2 + white * 0.153852;
        b3 = 0.8665 * b3 + white * 0.3104856;
        b4 = 0.55 * b4 + white * 0.5329522;
        b5 = -0.7616 * b5 - white * 0.016898;
        out[i] = (b0 + b1 + b2 + b3 + b4 + b5 + b6 + white * 0.5362) * 0.08;
        b6 = white * 0.115926;
      }
    } else if (kind === "brown") {
      let last = 0;
      for (let i = 0; i < length; i++) {
        const white = Math.random() * 2 - 1;
        last = (last + 0.02 * white) / 1.02;
        out[i] = Math.max(-1, Math.min(1, last * 2.6)) * 0.45;
      }
    } else if (kind === "rain_drops") {
      // Soft ambient drizzle bed punctuated by stochastic raindrop micro-impulses
      let prev = 0;
      for (let i = 0; i < length; i++) {
        const w = Math.random() * 2 - 1;
        prev = prev * 0.65 + w * 0.35;
        let sample = prev * 0.22;
        if (Math.random() < 0.0018) {
          sample += (Math.random() * 2 - 1) * 0.65;
        }
        out[i] = Math.max(-1, Math.min(1, sample));
      }
    } else {
      // Fireplace crackle: sparse short high-energy ember pops over warm floor
      let floor = 0;
      for (let i = 0; i < length; i++) {
        const w = Math.random() * 2 - 1;
        floor = (floor + 0.015 * w) / 1.015;
        let sample = floor * 1.4;
        if (Math.random() < 0.00055) {
          const burst = (Math.random() * 2 - 1) * (0.4 + Math.random() * 0.55);
          sample += burst;
        }
        out[i] = Math.max(-1, Math.min(1, sample * 0.45));
      }
    }

    // Apply short cross-edge taper (32 ms) to avoid loop seam clicks.
    const fadeSamples = Math.min(Math.floor(sampleRate * 0.032), Math.floor(length / 8));
    for (let i = 0; i < fadeSamples; i++) {
      const ramp = i / fadeSamples;
      out[i] *= ramp;
      out[length - 1 - i] *= ramp;
    }
  }

  return buffer;
}

function buildChannelGraph(
  ctx: AudioContext,
  id: SoundscapeChannelId,
  destination: AudioNode
): ChannelGraph {
  const outGain = ctx.createGain();
  outGain.gain.setValueAtTime(0, ctx.currentTime);
  outGain.connect(destination);

  const stoppers: Array<() => void> = [];

  const startLoop = (
    buf: AudioBuffer,
    target: AudioNode,
    playbackRate: number = 1
  ): AudioBufferSourceNode => {
    const src = ctx.createBufferSource();
    src.buffer = buf;
    src.loop = true;
    src.playbackRate.setValueAtTime(playbackRate, ctx.currentTime);
    src.connect(target);
    src.start();
    stoppers.push(() => {
      try {
        src.stop();
        src.disconnect();
      } catch {
        // already stopped
      }
    });
    return src;
  };

  switch (id) {
    case "white_noise": {
      const shelf = ctx.createBiquadFilter();
      shelf.type = "highshelf";
      shelf.frequency.setValueAtTime(6500, ctx.currentTime);
      shelf.gain.setValueAtTime(-4, ctx.currentTime);
      shelf.connect(outGain);
      startLoop(createProceduralBuffer(ctx, "white", 4), shelf);
      break;
    }

    case "pink_noise": {
      startLoop(createProceduralBuffer(ctx, "pink", 4), outGain);
      break;
    }

    case "brown_noise": {
      const lp = ctx.createBiquadFilter();
      lp.type = "lowpass";
      lp.frequency.setValueAtTime(420, ctx.currentTime);
      lp.Q.setValueAtTime(0.7, ctx.currentTime);
      lp.connect(outGain);
      startLoop(createProceduralBuffer(ctx, "brown", 5), lp);
      break;
    }

    case "rain": {
      const bp = ctx.createBiquadFilter();
      bp.type = "bandpass";
      bp.frequency.setValueAtTime(1600, ctx.currentTime);
      bp.Q.setValueAtTime(0.55, ctx.currentTime);
      bp.connect(outGain);
      startLoop(createProceduralBuffer(ctx, "rain_drops", 5), bp);

      const sheen = ctx.createBiquadFilter();
      sheen.type = "highpass";
      sheen.frequency.setValueAtTime(3200, ctx.currentTime);
      const sheenGain = ctx.createGain();
      sheenGain.gain.setValueAtTime(0.28, ctx.currentTime);
      sheen.connect(sheenGain);
      sheenGain.connect(outGain);
      startLoop(createProceduralBuffer(ctx, "pink", 4), sheen);
      break;
    }

    case "ocean_waves": {
      const lp = ctx.createBiquadFilter();
      lp.type = "lowpass";
      lp.frequency.setValueAtTime(520, ctx.currentTime);
      lp.Q.setValueAtTime(0.8, ctx.currentTime);

      const swellGain = ctx.createGain();
      swellGain.gain.setValueAtTime(0.55, ctx.currentTime);

      // Slow wave-swell LFO modulating both amplitude and lowpass cutoff
      const lfo = ctx.createOscillator();
      lfo.type = "sine";
      lfo.frequency.setValueAtTime(0.11, ctx.currentTime);

      const lfoAmpDepth = ctx.createGain();
      lfoAmpDepth.gain.setValueAtTime(0.35, ctx.currentTime);
      lfo.connect(lfoAmpDepth);
      lfoAmpDepth.connect(swellGain.gain);

      const lfoFreqDepth = ctx.createGain();
      lfoFreqDepth.gain.setValueAtTime(280, ctx.currentTime);
      lfo.connect(lfoFreqDepth);
      lfoFreqDepth.connect(lp.frequency);

      lp.connect(swellGain);
      swellGain.connect(outGain);

      startLoop(createProceduralBuffer(ctx, "pink", 6), lp);
      lfo.start();
      stoppers.push(() => {
        try {
          lfo.stop();
          lfo.disconnect();
        } catch {
          // already stopped
        }
      });
      break;
    }

    case "forest_wind": {
      const bp = ctx.createBiquadFilter();
      bp.type = "bandpass";
      bp.frequency.setValueAtTime(640, ctx.currentTime);
      bp.Q.setValueAtTime(2.4, ctx.currentTime);

      const lfo = ctx.createOscillator();
      lfo.type = "sine";
      lfo.frequency.setValueAtTime(0.08, ctx.currentTime);
      const lfoDepth = ctx.createGain();
      lfoDepth.gain.setValueAtTime(260, ctx.currentTime);
      lfo.connect(lfoDepth);
      lfoDepth.connect(bp.frequency);

      bp.connect(outGain);
      startLoop(createProceduralBuffer(ctx, "pink", 5), bp);
      lfo.start();
      stoppers.push(() => {
        try {
          lfo.stop();
          lfo.disconnect();
        } catch {
          // already stopped
        }
      });
      break;
    }

    case "fireplace": {
      const hp = ctx.createBiquadFilter();
      hp.type = "highpass";
      hp.frequency.setValueAtTime(160, ctx.currentTime);
      hp.connect(outGain);
      startLoop(createProceduralBuffer(ctx, "crackle", 5), hp);

      const warmth = ctx.createBiquadFilter();
      warmth.type = "lowpass";
      warmth.frequency.setValueAtTime(220, ctx.currentTime);
      const warmGain = ctx.createGain();
      warmGain.gain.setValueAtTime(0.55, ctx.currentTime);
      warmth.connect(warmGain);
      warmGain.connect(outGain);
      startLoop(createProceduralBuffer(ctx, "brown", 4), warmth);
      break;
    }

    case "deep_focus_drone": {
      const lp = ctx.createBiquadFilter();
      lp.type = "lowpass";
      lp.frequency.setValueAtTime(420, ctx.currentTime);
      lp.Q.setValueAtTime(0.9, ctx.currentTime);
      lp.connect(outGain);

      // Warm harmonic fifth drone with subtle binaural shimmer (110 / 111.5 / 165 / 220 Hz)
      const partials: Array<[number, OscillatorType, number]> = [
        [110, "sine", 0.14],
        [111.5, "sine", 0.11],
        [165, "triangle", 0.06],
        [220, "sine", 0.05],
      ];

      for (const [freq, oscType, level] of partials) {
        const osc = ctx.createOscillator();
        osc.type = oscType;
        osc.frequency.setValueAtTime(freq, ctx.currentTime);
        const g = ctx.createGain();
        g.gain.setValueAtTime(level, ctx.currentTime);
        osc.connect(g);
        g.connect(lp);
        osc.start();
        stoppers.push(() => {
          try {
            osc.stop();
            osc.disconnect();
          } catch {
            // already stopped
          }
        });
      }
      break;
    }
  }

  return {
    gain: outGain,
    cleanup: () => {
      for (const stop of stoppers) stop();
      try {
        outGain.disconnect();
      } catch {
        // already disconnected
      }
    },
  };
}

export type SoundscapeListener = (snapshot: SoundscapeSnapshot) => void;

/**
 * Multi-track procedural ambient soundscape engine with mixer, presets, and sleep timer.
 */
export class SoundscapeEngine {
  private ctx: AudioContext | null = null;
  private masterGainNode: GainNode | null = null;
  private graphs: Map<SoundscapeChannelId, ChannelGraph> = new Map();
  private timerInterval: ReturnType<typeof setInterval> | null = null;
  private listeners: Set<SoundscapeListener> = new Set();

  private isPlaying = false;
  private masterVolume = 0.75;
  private activePresetId: SoundscapePresetId | null = null;
  private channels: Record<SoundscapeChannelId, SoundscapeChannelState>;
  private sleepTimer: SleepTimerState = {
    active: false,
    totalSeconds: 0,
    remainingSeconds: 0,
    fadeOutSeconds: 30,
  };

  constructor() {
    this.channels = {} as Record<SoundscapeChannelId, SoundscapeChannelState>;
    for (const meta of SOUNDSCAPE_CHANNELS) {
      this.channels[meta.id] = {
        id: meta.id,
        volume: meta.defaultVolume,
        muted: false,
      };
    }
  }

  subscribe(listener: SoundscapeListener): () => void {
    this.listeners.add(listener);
    listener(this.getSnapshot());
    return () => {
      this.listeners.delete(listener);
    };
  }

  private emit(): void {
    const snap = this.getSnapshot();
    for (const l of this.listeners) {
      l(snap);
    }
  }

  getSnapshot(): SoundscapeSnapshot {
    const clonedChannels = {} as Record<SoundscapeChannelId, SoundscapeChannelState>;
    for (const meta of SOUNDSCAPE_CHANNELS) {
      clonedChannels[meta.id] = { ...this.channels[meta.id] };
    }
    const timerCopy = { ...this.sleepTimer };
    const effectiveMasterGain = this.isPlaying
      ? this.masterVolume * computeSleepTimerFadeMultiplier(timerCopy)
      : 0;

    return {
      isPlaying: this.isPlaying,
      masterVolume: this.masterVolume,
      effectiveMasterGain,
      activePresetId: this.activePresetId,
      channels: clonedChannels,
      sleepTimer: timerCopy,
    };
  }

  private ensureAudioGraph(): void {
    if (!this.ctx) {
      this.ctx = createAudioContext();
    }
    if (!this.ctx) return;

    if (!this.masterGainNode) {
      this.masterGainNode = this.ctx.createGain();
      this.masterGainNode.gain.setValueAtTime(0, this.ctx.currentTime);
      this.masterGainNode.connect(this.ctx.destination);
    }

    if (this.graphs.size === 0) {
      for (const meta of SOUNDSCAPE_CHANNELS) {
        this.graphs.set(meta.id, buildChannelGraph(this.ctx, meta.id, this.masterGainNode));
      }
    }
  }

  private syncAudioGains(rampTimeSec: number = 0.08): void {
    if (!this.ctx || !this.masterGainNode) return;
    const now = this.ctx.currentTime;
    const tc = Math.max(0.015, rampTimeSec);
    const fadeMult = computeSleepTimerFadeMultiplier(this.sleepTimer);
    const targetMaster = this.isPlaying ? this.masterVolume * fadeMult : 0;

    this.masterGainNode.gain.setTargetAtTime(targetMaster, now, tc);
    for (const meta of SOUNDSCAPE_CHANNELS) {
      const chState = this.channels[meta.id];
      const graph = this.graphs.get(meta.id);
      if (graph) {
        const target = chState.muted ? 0 : chState.volume;
        graph.gain.gain.setTargetAtTime(target, now, tc);
      }
    }
  }

  play(): void {
    this.ensureAudioGraph();
    if (this.ctx && this.ctx.state === "suspended") {
      void this.ctx.resume().catch(() => {});
    }
    this.isPlaying = true;
    this.syncAudioGains(0.12);
    this.ensureTimerInterval();
    this.emit();
  }

  pause(): void {
    this.isPlaying = false;
    this.stopTimerInterval();
    this.syncAudioGains(0.1);
    this.emit();
  }

  togglePlay(): boolean {
    if (this.isPlaying) {
      this.pause();
    } else {
      this.play();
    }
    return this.isPlaying;
  }

  setMasterVolume(volume: number): void {
    this.masterVolume = clamp01(volume);
    this.syncAudioGains(0.05);
    this.emit();
  }

  setChannelVolume(id: SoundscapeChannelId, volume: number): void {
    const ch = this.channels[id];
    if (!ch) return;
    ch.volume = clamp01(volume);
    this.activePresetId = null;
    this.syncAudioGains(0.05);
    this.emit();
  }

  setChannelMuted(id: SoundscapeChannelId, muted: boolean): void {
    const ch = this.channels[id];
    if (!ch) return;
    ch.muted = muted;
    this.syncAudioGains(0.05);
    this.emit();
  }

  toggleChannelMute(id: SoundscapeChannelId): boolean {
    const ch = this.channels[id];
    if (!ch) return false;
    ch.muted = !ch.muted;
    this.syncAudioGains(0.05);
    this.emit();
    return ch.muted;
  }

  applyPreset(presetId: SoundscapePresetId): boolean {
    const preset = SOUNDSCAPE_PRESETS.find((p) => p.id === presetId);
    if (!preset) return false;

    for (const meta of SOUNDSCAPE_CHANNELS) {
      const vol = preset.channelVolumes[meta.id] ?? 0;
      this.channels[meta.id].volume = clamp01(vol);
      this.channels[meta.id].muted = false;
    }
    this.activePresetId = presetId;
    this.syncAudioGains(0.15);
    this.emit();
    return true;
  }

  /**
   * Starts a sleep countdown timer with smooth fade-out during the final `fadeOutSeconds`.
   */
  startSleepTimer(durationSeconds: number, fadeOutSeconds: number = 30): void {
    const total = Math.max(1, Math.floor(durationSeconds));
    const fade = Math.max(1, Math.min(total, Math.floor(fadeOutSeconds)));
    this.sleepTimer = {
      active: true,
      totalSeconds: total,
      remainingSeconds: total,
      fadeOutSeconds: fade,
    };
    this.ensureTimerInterval();
    this.syncAudioGains(0.08);
    this.emit();
  }

  cancelSleepTimer(): void {
    this.stopTimerInterval();
    this.sleepTimer = {
      active: false,
      totalSeconds: 0,
      remainingSeconds: 0,
      fadeOutSeconds: this.sleepTimer.fadeOutSeconds,
    };
    this.syncAudioGains(0.08);
    this.emit();
  }

  /**
   * Advances the sleep countdown timer by `deltaSeconds` (pure deterministic step; also invoked
   * automatically every second while playing in the browser).
   */
  tickTimer(deltaSeconds: number): void {
    if (!this.sleepTimer.active || deltaSeconds <= 0) return;
    const nextRemaining = Math.max(0, this.sleepTimer.remainingSeconds - deltaSeconds);
    this.sleepTimer.remainingSeconds = nextRemaining;

    if (nextRemaining <= 0) {
      this.sleepTimer.active = false;
      this.isPlaying = false;
      this.stopTimerInterval();
      this.syncAudioGains(0.05);
    } else {
      this.syncAudioGains(0.2);
    }
    this.emit();
  }

  private ensureTimerInterval(): void {
    if (!this.isPlaying || !this.sleepTimer.active) return;
    if (this.timerInterval !== null) return;
    this.timerInterval = setInterval(() => {
      this.tickTimer(1);
    }, 1000);
  }

  private stopTimerInterval(): void {
    if (this.timerInterval !== null) {
      clearInterval(this.timerInterval);
      this.timerInterval = null;
    }
  }

  dispose(): void {
    this.stopTimerInterval();
    this.isPlaying = false;
    for (const graph of this.graphs.values()) {
      graph.cleanup();
    }
    this.graphs.clear();
    if (this.masterGainNode) {
      try {
        this.masterGainNode.disconnect();
      } catch {
        // already disconnected
      }
      this.masterGainNode = null;
    }
    if (this.ctx) {
      void this.ctx.close().catch(() => {});
      this.ctx = null;
    }
    this.listeners.clear();
  }
}
