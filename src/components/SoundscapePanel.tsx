import React, { useEffect, useRef, useState } from "react";
import {
  AudioWaveform,
  CloudRain,
  Flame,
  Headphones,
  Pause,
  Play,
  Radio,
  RotateCcw,
  Sparkles,
  Timer,
  Upload,
  Volume2,
  VolumeX,
  Waves,
  Wind,
} from "lucide-react";
import {
  SOUNDSCAPE_CHANNELS,
  SOUNDSCAPE_PRESETS,
  SoundscapeChannelId,
  SoundscapeEngine,
  SoundscapePresetId,
  SoundscapeSnapshot,
} from "../audio/soundscapeEngine";
import {
  QCY_CARD_CLASS,
  QCY_SUBCARD_CLASS,
  TranslateFn,
  formatCountdown,
} from "./shared";

export interface OnboardWhiteNoiseState {
  playing: boolean;
  trackIndex: number;
  trackNames?: string[];
  volume: number; // 0..100
  syncing?: boolean;
  syncProgressPercent?: number;
}

export interface SoundscapePanelProps {
  /** Optional shared `SoundscapeEngine` instance so ambient audio persists across tab switches. */
  engine?: SoundscapeEngine;
  supportsOnboardWhiteNoise?: boolean;
  onboardState?: OnboardWhiteNoiseState;
  disabled?: boolean;
  onOnboardTogglePlay?: (playing: boolean) => void;
  onOnboardTrackChange?: (trackIndex: number) => void;
  onOnboardVolumeChange?: (volume: number) => void;
  onSyncTonePack?: (file: File) => void;
  t: TranslateFn;
  className?: string;
}

const SLEEP_TIMER_PRESETS_MIN = [15, 30, 45, 60, 90] as const;

const CHANNEL_ICON: Record<
  SoundscapeChannelId,
  React.ComponentType<{ className?: string }>
> = {
  white_noise: Radio,
  pink_noise: AudioWaveform,
  brown_noise: AudioWaveform,
  rain: CloudRain,
  ocean_waves: Waves,
  forest_wind: Wind,
  fireplace: Flame,
  deep_focus_drone: Sparkles,
};

export const SoundscapePanel: React.FC<SoundscapePanelProps> = ({
  engine: externalEngine,
  supportsOnboardWhiteNoise = false,
  onboardState,
  disabled = false,
  onOnboardTogglePlay,
  onOnboardTrackChange,
  onOnboardVolumeChange,
  onSyncTonePack,
  t,
  className = "",
}) => {
  const internalEngineRef = useRef<SoundscapeEngine | null>(null);
  if (!externalEngine && !internalEngineRef.current) {
    internalEngineRef.current = new SoundscapeEngine();
  }
  const activeEngine = externalEngine ?? internalEngineRef.current!;

  const [snapshot, setSnapshot] = useState<SoundscapeSnapshot>(() =>
    activeEngine.getSnapshot()
  );
  const [activeTab, setActiveTab] = useState<"desktop" | "onboard">("desktop");
  const toneFileInputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    const unsub = activeEngine.subscribe(setSnapshot);
    return () => {
      unsub();
    };
  }, [activeEngine]);

  useEffect(() => {
    return () => {
      if (!externalEngine && internalEngineRef.current) {
        internalEngineRef.current.dispose();
        internalEngineRef.current = null;
      }
    };
  }, [externalEngine]);

  const handleResetMix = () => {
    for (const ch of SOUNDSCAPE_CHANNELS) {
      activeEngine.setChannelVolume(ch.id, ch.defaultVolume);
      activeEngine.setChannelMuted(ch.id, false);
    }
    activeEngine.setMasterVolume(0.75);
  };

  const handleApplyPreset = (presetId: SoundscapePresetId) => {
    activeEngine.applyPreset(presetId);
    if (!snapshot.isPlaying) {
      activeEngine.play();
    }
  };

  const handleSelectTimer = (minutes: number | null) => {
    if (minutes === null) {
      activeEngine.cancelSleepTimer();
    } else {
      activeEngine.startSleepTimer(minutes * 60, 15);
    }
  };

  const onboardTracks = onboardState?.trackNames ?? [
    "Ocean Breeze",
    "Rainforest",
    "Deep Night",
    "Soft Static",
  ];

  return (
    <section
      className={`${QCY_CARD_CLASS} space-y-5 ${className}`}
      aria-label={t("focus_mode.title")}
    >
      {/* Header */}
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-2.5">
          <span className="flex h-9 w-9 items-center justify-center rounded-xl bg-[#727AFF]/15 text-[#727AFF]">
            <CloudRain className="h-5 w-5" />
          </span>
          <div>
            <h3 className="text-base font-bold text-[#1A1A1A] dark:text-white">
              {t("focus_mode.title")}
            </h3>
            <p className="text-xs text-[#808080] dark:text-white/55">
              {t("focus_mode.subtitle")}
            </p>
          </div>
        </div>

        {supportsOnboardWhiteNoise && (
          <div className="inline-flex rounded-xl bg-[#F2F1F6] p-1 dark:bg-white/[0.06]">
            <button
              type="button"
              onClick={() => setActiveTab("desktop")}
              className={`rounded-lg px-3 py-1 text-xs font-semibold transition ${
                activeTab === "desktop"
                  ? "bg-[#FF7A2F] text-white"
                  : "text-[#4D4D4D] dark:text-white/70"
              }`}
            >
              {t("focus_mode.tab_desktop_mixer")}
            </button>
            <button
              type="button"
              onClick={() => setActiveTab("onboard")}
              className={`rounded-lg px-3 py-1 text-xs font-semibold transition ${
                activeTab === "onboard"
                  ? "bg-[#FF7A2F] text-white"
                  : "text-[#4D4D4D] dark:text-white/70"
              }`}
            >
              {t("focus_mode.tab_device_tones")}
            </button>
          </div>
        )}
      </div>

      {/* DESKTOP PROCEDURAL SOUNDSCAPE MIXER */}
      {activeTab === "desktop" && (
        <div className="space-y-5">
          {/* Transport + Master Volume Bar */}
          <div className={`${QCY_SUBCARD_CLASS} flex flex-wrap items-center gap-4`}>
            <button
              type="button"
              onClick={() => activeEngine.togglePlay()}
              className="inline-flex items-center gap-2 rounded-xl bg-[#FF7A2F] px-4 py-2.5 text-sm font-bold text-white shadow-xs transition hover:bg-[#E8691F]"
            >
              {snapshot.isPlaying ? (
                <>
                  <Pause className="h-4 w-4" />
                  {t("focus_mode.pause_all")}
                </>
              ) : (
                <>
                  <Play className="h-4 w-4" />
                  {t("focus_mode.play_all")}
                </>
              )}
            </button>

            <div className="flex min-w-[180px] flex-1 items-center gap-3">
              <Volume2 className="h-4 w-4 shrink-0 text-[#0066FF] dark:text-[#3399FF]" />
              <div className="flex-1 space-y-1">
                <div className="flex justify-between text-[11px] font-semibold">
                  <span className="text-[#4D4D4D] dark:text-white/75">
                    {t("focus_mode.master_volume")}
                  </span>
                  <span className="font-mono text-[#0066FF] dark:text-[#3399FF]">
                    {Math.round(snapshot.masterVolume * 100)}%
                  </span>
                </div>
                <input
                  type="range"
                  min={0}
                  max={100}
                  step={1}
                  value={Math.round(snapshot.masterVolume * 100)}
                  onChange={(e) => activeEngine.setMasterVolume(Number(e.target.value) / 100)}
                  className="h-2 w-full cursor-pointer accent-[#0066FF] dark:accent-[#3399FF]"
                />
              </div>
            </div>

            <button
              type="button"
              onClick={handleResetMix}
              className="inline-flex items-center gap-1 rounded-lg border border-[#E5E5E5] bg-white px-3 py-2 text-xs font-semibold text-[#4D4D4D] hover:border-[#FF7A2F] dark:border-white/12 dark:bg-white/6 dark:text-white/75"
            >
              <RotateCcw className="h-3.5 w-3.5" />
              {t("focus_mode.reset_mix")}
            </button>
          </div>

          {/* 4 Mood Scene Presets */}
          <div className="space-y-2">
            <p className="text-xs font-bold uppercase tracking-wider text-[#808080] dark:text-white/50">
              {t("focus_mode.presets_label")}
            </p>
            <div className="grid grid-cols-2 gap-2.5 sm:grid-cols-4">
              {SOUNDSCAPE_PRESETS.map((preset) => {
                const active = snapshot.activePresetId === preset.id;
                return (
                  <button
                    key={preset.id}
                    type="button"
                    onClick={() => handleApplyPreset(preset.id)}
                    aria-pressed={active}
                    className={`rounded-xl border px-3 py-2.5 text-left text-xs font-bold transition ${
                      active
                        ? "border-[#FF7A2F] bg-[#FF7A2F]/14 text-[#C2410C] dark:text-[#FF9E66]"
                        : "border-[#E5E5E5] bg-[#F2F1F6]/70 text-[#1A1A1A] hover:border-[#FF7A2F]/50 dark:border-white/8 dark:bg-white/[0.04] dark:text-white/85"
                    }`}
                  >
                    {t(`focus_mode.preset_${preset.id}`)}
                  </button>
                );
              })}
            </div>
          </div>

          {/* 8 Mixable Procedural Channels */}
          <div className="grid gap-3 sm:grid-cols-2">
            {SOUNDSCAPE_CHANNELS.map((meta) => {
              const ch = snapshot.channels[meta.id];
              const Icon = CHANNEL_ICON[meta.id];
              const pct = Math.round(ch.volume * 100);
              const isAudible = pct > 0 && !ch.muted;
              return (
                <div
                  key={meta.id}
                  className={`flex items-center gap-3 rounded-xl border p-3 transition ${
                    isAudible
                      ? "border-[#0066FF]/30 bg-[#0066FF]/[0.04] dark:border-[#3399FF]/30 dark:bg-[#3399FF]/[0.06]"
                      : "border-[#E5E5E5]/80 bg-[#F2F1F6]/60 dark:border-white/8 dark:bg-white/[0.03]"
                  }`}
                >
                  <button
                    type="button"
                    onClick={() => activeEngine.toggleChannelMute(meta.id)}
                    aria-label={t(`focus_mode.channels.${meta.id}`)}
                    aria-pressed={ch.muted}
                    className={`flex h-8 w-8 shrink-0 items-center justify-center rounded-lg transition ${
                      ch.muted
                        ? "bg-black/8 text-[#808080] dark:bg-white/10 dark:text-white/40"
                        : isAudible
                          ? "bg-[#0066FF] text-white dark:bg-[#3399FF]"
                          : "bg-black/5 text-[#4D4D4D] dark:bg-white/10 dark:text-white/70"
                    }`}
                  >
                    {ch.muted ? <VolumeX className="h-4 w-4" /> : <Icon className="h-4 w-4" />}
                  </button>

                  <div className="min-w-0 flex-1 space-y-1">
                    <div className="flex items-center justify-between text-xs">
                      <span className="truncate font-semibold text-[#1A1A1A] dark:text-white">
                        {t(`focus_mode.channels.${meta.id}`)}
                      </span>
                      <span className="font-mono text-[11px] font-semibold text-[#808080] dark:text-white/55">
                        {ch.muted ? t("common.off") : `${pct}%`}
                      </span>
                    </div>
                    <input
                      type="range"
                      min={0}
                      max={100}
                      step={1}
                      value={pct}
                      onChange={(e) => {
                        const v = Number(e.target.value) / 100;
                        if (ch.muted && v > 0) activeEngine.setChannelMuted(meta.id, false);
                        activeEngine.setChannelVolume(meta.id, v);
                      }}
                      className="h-1.5 w-full cursor-pointer accent-[#FF7A2F]"
                    />
                  </div>
                </div>
              );
            })}
          </div>

          {/* Sleep Countdown Timer Bar */}
          <div className={`${QCY_SUBCARD_CLASS} space-y-2.5`}>
            <div className="flex flex-wrap items-center justify-between gap-2">
              <div className="flex items-center gap-2 text-xs font-bold text-[#1A1A1A] dark:text-white">
                <Timer className="h-4 w-4 text-[#FF7A2F]" />
                {t("focus_mode.sleep_timer")}
              </div>
              {snapshot.sleepTimer.active && (
                <span className="rounded-full bg-[#FF7A2F]/15 px-2.5 py-0.5 font-mono text-xs font-bold text-[#C2410C] dark:text-[#FF9E66]">
                  {t("focus_mode.timer_remaining", {
                    time: formatCountdown(snapshot.sleepTimer.remainingSeconds),
                  })}
                </span>
              )}
            </div>

            <div className="flex flex-wrap gap-1.5">
              <button
                type="button"
                onClick={() => handleSelectTimer(null)}
                className={`rounded-lg px-3 py-1.5 text-xs font-semibold transition ${
                  !snapshot.sleepTimer.active
                    ? "bg-[#FF7A2F] text-white"
                    : "bg-white text-[#4D4D4D] hover:bg-black/5 dark:bg-white/8 dark:text-white/75"
                }`}
              >
                {t("focus_mode.timer_off")}
              </button>
              {SLEEP_TIMER_PRESETS_MIN.map((mins) => {
                const active =
                  snapshot.sleepTimer.active &&
                  snapshot.sleepTimer.totalSeconds === mins * 60;
                return (
                  <button
                    key={mins}
                    type="button"
                    onClick={() => handleSelectTimer(mins)}
                    className={`rounded-lg px-3 py-1.5 text-xs font-semibold transition ${
                      active
                        ? "bg-[#FF7A2F] text-white"
                        : "bg-white text-[#4D4D4D] hover:bg-black/5 dark:bg-white/8 dark:text-white/75"
                    }`}
                  >
                    {t("common.minutes", { count: mins })}
                  </button>
                );
              })}
            </div>

            <p className="text-[11px] text-[#808080] dark:text-white/50">
              {t("focus_mode.fade_out_notice")}
            </p>
          </div>
        </div>
      )}

      {/* ONBOARD HARDWARE FOCUS MODE (CT02 / 0x39–0x3D) */}
      {activeTab === "onboard" && supportsOnboardWhiteNoise && (
        <div className="space-y-4">
          <div className={`${QCY_SUBCARD_CLASS} flex flex-wrap items-center justify-between gap-3`}>
            <div className="flex items-center gap-2.5">
              <Headphones className="h-4 w-4 text-[#FF7A2F]" />
              <span className="text-sm font-bold text-[#1A1A1A] dark:text-white">
                {t("focus_mode.device_playback")}
              </span>
            </div>
            <button
              type="button"
              disabled={disabled}
              onClick={() => onOnboardTogglePlay?.(!onboardState?.playing)}
              className="inline-flex items-center gap-1.5 rounded-xl bg-[#FF7A2F] px-4 py-2 text-xs font-bold text-white hover:bg-[#E8691F] disabled:opacity-50"
            >
              {onboardState?.playing ? (
                <>
                  <Pause className="h-3.5 w-3.5" />
                  {t("common.pause")}
                </>
              ) : (
                <>
                  <Play className="h-3.5 w-3.5" />
                  {t("common.play")}
                </>
              )}
            </button>
          </div>

          <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
            {onboardTracks.map((name, idx) => {
              const selected = (onboardState?.trackIndex ?? 0) === idx;
              return (
                <button
                  key={name}
                  type="button"
                  disabled={disabled}
                  onClick={() => onOnboardTrackChange?.(idx)}
                  className={`rounded-xl border px-3 py-2.5 text-xs font-semibold transition ${
                    selected
                      ? "border-[#FF7A2F] bg-[#FF7A2F]/12 text-[#1A1A1A] dark:text-white"
                      : "border-[#E5E5E5] bg-[#F2F1F6] text-[#4D4D4D] dark:border-white/8 dark:bg-white/[0.04] dark:text-white/75"
                  }`}
                >
                  {name}
                </button>
              );
            })}
          </div>

          <div className={`${QCY_SUBCARD_CLASS} space-y-1.5`}>
            <div className="flex justify-between text-xs font-semibold">
              <span className="text-[#1A1A1A] dark:text-white">
                {t("focus_mode.device_volume")}
              </span>
              <span className="font-mono text-[#FF7A2F]">{onboardState?.volume ?? 50}%</span>
            </div>
            <input
              type="range"
              min={0}
              max={100}
              step={1}
              disabled={disabled}
              value={onboardState?.volume ?? 50}
              onChange={(e) => onOnboardVolumeChange?.(Number(e.target.value))}
              className="h-2 w-full cursor-pointer accent-[#FF7A2F]"
            />
          </div>

          {onSyncTonePack && (
            <div className={`${QCY_SUBCARD_CLASS} space-y-2.5`}>
              <div className="flex flex-wrap items-center justify-between gap-2">
                <span className="text-xs font-bold text-[#1A1A1A] dark:text-white">
                  {t("focus_mode.sync_tone_pack")}
                </span>
                <input
                  ref={toneFileInputRef}
                  type="file"
                  accept=".bin"
                  onChange={(e) => {
                    const file = e.target.files?.[0];
                    e.target.value = "";
                    if (file) onSyncTonePack(file);
                  }}
                  className="hidden"
                />
                <button
                  type="button"
                  disabled={disabled || onboardState?.syncing}
                  onClick={() => toneFileInputRef.current?.click()}
                  className="inline-flex items-center gap-1.5 rounded-lg bg-[#0066FF] px-3 py-1.5 text-xs font-semibold text-white hover:bg-[#0052CC] disabled:opacity-50 dark:bg-[#3399FF]"
                >
                  <Upload className="h-3.5 w-3.5" />
                  {t("focus_mode.sync_select_bin")}
                </button>
              </div>

              {onboardState?.syncing && (
                <div className="space-y-1">
                  <p className="text-xs font-medium text-[#0066FF] dark:text-[#66B2FF]">
                    {t("focus_mode.sync_in_progress", {
                      percent: onboardState.syncProgressPercent ?? 0,
                    })}
                  </p>
                  <div className="h-2 w-full overflow-hidden rounded-full bg-black/10 dark:bg-white/10">
                    <div
                      className="h-full rounded-full bg-[#0066FF] transition-all dark:bg-[#3399FF]"
                      style={{ width: `${onboardState.syncProgressPercent ?? 0}%` }}
                    />
                  </div>
                </div>
              )}
            </div>
          )}
        </div>
      )}
    </section>
  );
};
