import React, { useEffect, useState } from "react";
import {
  Activity,
  AlarmClock,
  BatteryCharging,
  Bluetooth,
  CheckCircle2,
  Ear,
  Gamepad2,
  Headphones,
  HeartPulse,
  Laptop,
  MessageSquare,
  Moon,
  Power,
  Radio,
  Shield,
  Sparkles,
  Volume2,
  Waves,
  AlertTriangle,
} from "lucide-react";
import { useQcyStore } from "../store/useQcyStore";
import { useI18n } from "../i18n";
import { playFitTestProbe } from "../audio/audioUtils";
import type { ListeningMode, PromptLanguage, SpatialAudioMode } from "../types/qcy";

interface ToggleRowProps {
  label: string;
  description?: string;
  checked: boolean;
  disabled?: boolean;
  onChange: (next: boolean) => void;
  badge?: string;
}

const ToggleRow: React.FC<ToggleRowProps> = ({
  label,
  description,
  checked,
  disabled,
  onChange,
  badge,
}) => (
  <div className="flex items-center justify-between gap-4 py-3 first:pt-0 last:pb-0">
    <div className="min-w-0">
      <div className="flex items-center gap-2">
        <span className="text-sm font-medium text-neutral-900 dark:text-white">{label}</span>
        {badge && (
          <span className="rounded-full bg-[#FF7A2F]/15 px-2 py-0.5 text-[10px] font-semibold text-[#FF7A2F]">
            {badge}
          </span>
        )}
      </div>
      {description && (
        <p className="mt-0.5 text-xs text-neutral-500 dark:text-neutral-400">{description}</p>
      )}
    </div>
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`relative inline-flex h-6 w-11 shrink-0 cursor-pointer items-center rounded-full transition-colors disabled:cursor-not-allowed disabled:opacity-40 ${
        checked ? "bg-[#FF7A2F]" : "bg-neutral-300 dark:bg-neutral-700"
      }`}
    >
      <span
        className={`inline-block h-4 w-4 transform rounded-full bg-white shadow transition-transform ${
          checked ? "translate-x-6" : "translate-x-1"
        }`}
      />
    </button>
  </div>
);

interface AlarmEntry {
  id: number;
  time: string;
  enabled: boolean;
  label: string;
}

export const ModesLabPage: React.FC = () => {
  const { snapshot, setModes, setMultipoint, runFitTest } = useQcyStore();
  const { t, locale } = useI18n();

  const [fitTesting, setFitTesting] = useState(false);
  const [leCountdown, setLeCountdown] = useState<number | null>(null);
  const [clockSyncedAt, setClockSyncedAt] = useState<string | null>(null);
  const [alarms, setAlarms] = useState<AlarmEntry[]>([
    { id: 1, time: "07:30", enabled: false, label: locale === "zh-CN" ? "晨间闹钟" : "Morning Alarm" },
    { id: 2, time: "22:30", enabled: false, label: locale === "zh-CN" ? "休息提醒" : "Wind-Down Reminder" },
  ]);

  useEffect(() => {
    if (leCountdown === null || leCountdown <= 0) return;
    const timer = setTimeout(() => setLeCountdown((v) => (v !== null ? v - 1 : null)), 1000);
    return () => clearTimeout(timer);
  }, [leCountdown]);

  if (!snapshot) {
    return (
      <div className="rounded-2xl bg-white p-8 text-center text-sm text-neutral-500 dark:bg-[#1E1E1E] dark:text-neutral-400">
        {t("common.loading")}
      </div>
    );
  }

  const caps = snapshot.active_model.capabilities;
  const { modes, multipoint, last_fit_test } = snapshot;

  const handleFitTest = async () => {
    if (fitTesting) return;
    setFitTesting(true);
    const probe = playFitTestProbe(1800);
    try {
      await new Promise((resolve) => setTimeout(resolve, 600));
      await runFitTest();
    } finally {
      probe.stop();
      setFitTesting(false);
    }
  };

  const handleToggleLeAudio = (next: boolean) => {
    if (!next) {
      void setModes({ le_audio: false });
      return;
    }
    setLeCountdown(3);
  };

  const confirmLeAudio = () => {
    setLeCountdown(null);
    void setModes({ le_audio: true });
  };

  const handleSyncClock = () => {
    setClockSyncedAt(new Date().toLocaleTimeString());
  };

  const fitBadgeClass = (status: string) => {
    if (status === "good") return "bg-emerald-500/15 text-emerald-600 dark:text-emerald-400";
    if (status === "fair") return "bg-amber-500/15 text-amber-600 dark:text-amber-400";
    return "bg-rose-500/15 text-rose-600 dark:text-rose-400";
  };

  const fitLabel = (status: string) => {
    if (status === "good") return t("wear_and_health.fit_good");
    if (status === "fair") return t("wear_and_health.fit_fair");
    return t("wear_and_health.fit_failed");
  };

  return (
    <div className="space-y-5">
      {/* Section 1: Listening Modes, Game Mode & Spatial Audio */}
      <section className="rounded-2xl border border-neutral-200/80 bg-white p-5 shadow-xs dark:border-white/10 dark:bg-[#1E1E1E]">
        <div className="mb-4 flex items-center gap-2.5">
          <div className="flex h-8 w-8 items-center justify-center rounded-xl bg-[#0066FF]/12 text-[#0066FF] dark:bg-[#3399FF]/15 dark:text-[#3399FF]">
            <Gamepad2 className="h-4 w-4" />
          </div>
          <h2 className="text-base font-semibold text-neutral-900 dark:text-white">
            {t("device_settings.section_audio")}
          </h2>
        </div>

        <div className="divide-y divide-neutral-200/70 dark:divide-white/10">
          {caps.listening_modes.length > 0 && (
            <div className="pb-4">
              <div className="mb-2 flex items-center justify-between">
                <span className="text-sm font-medium text-neutral-900 dark:text-white">
                  {t("audio_modes.listening_mode")}
                </span>
              </div>
              <div className="grid grid-cols-3 gap-2">
                {(["music", "game", "movie"] as ListeningMode[])
                  .filter((m) => caps.listening_modes.includes(m))
                  .map((m) => {
                    const active = modes.listening_mode === m;
                    return (
                      <button
                        key={m}
                        type="button"
                        onClick={() => void setModes({ listening_mode: m })}
                        className={`rounded-xl px-3 py-2 text-xs font-semibold transition-all ${
                          active
                            ? "bg-[#0066FF] text-white shadow-sm dark:bg-[#3399FF]"
                            : "bg-[#F2F1F6] text-neutral-700 hover:bg-neutral-200/80 dark:bg-white/5 dark:text-neutral-300 dark:hover:bg-white/10"
                        }`}
                      >
                        {t(`audio_modes.listening_${m}`)}
                      </button>
                    );
                  })}
              </div>
              {modes.listening_mode !== "music" && (
                <p className="mt-2 text-xs text-amber-600 dark:text-amber-400">
                  {t("audio_modes.listening_eq_locked")}
                </p>
              )}
            </div>
          )}

          {caps.game_mode && (
            <ToggleRow
              label={t("audio_modes.game_mode")}
              description={t("audio_modes.game_mode_desc")}
              checked={modes.game_mode}
              onChange={(v) => void setModes({ game_mode: v })}
            />
          )}

          {caps.game_mode_pro && (
            <div className="space-y-2 py-3">
              <ToggleRow
                label={t("audio_modes.game_mode_pro")}
                description={t("audio_modes.game_mode_desc")}
                checked={modes.game_mode_pro}
                onChange={(v) => void setModes({ game_mode_pro: v })}
                badge="PRO"
              />
              {modes.game_mode_pro && (
                <div className="ml-4 rounded-xl bg-[#F2F1F6] px-3.5 py-2.5 dark:bg-white/5">
                  <ToggleRow
                    label={t("audio_modes.game_mode_sync_eq")}
                    description={t("audio_modes.game_mode_sync_eq_desc")}
                    checked={modes.game_sync_eq}
                    onChange={(v) => void setModes({ game_sync_eq: v })}
                  />
                </div>
              )}
            </div>
          )}

          {caps.spatial_audio_modes.length > 0 && (
            <div className="pt-3">
              <div className="mb-1 flex items-center gap-2">
                <Waves className="h-4 w-4 text-[#0066FF] dark:text-[#3399FF]" />
                <span className="text-sm font-medium text-neutral-900 dark:text-white">
                  {t("audio_modes.spatial_audio")}
                </span>
              </div>
              <p className="mb-2.5 text-xs text-neutral-500 dark:text-neutral-400">
                {t("audio_modes.spatial_desc")}
              </p>
              <div className="grid grid-cols-3 gap-2">
                {(["off", "fixed", "head_tracking"] as SpatialAudioMode[])
                  .filter((mode) => caps.spatial_audio_modes.includes(mode))
                  .map((mode) => {
                    const active = modes.spatial_audio === mode;
                    const key =
                      mode === "head_tracking"
                        ? "audio_modes.spatial_head_tracking"
                        : `audio_modes.spatial_${mode}`;
                    return (
                      <button
                        key={mode}
                        type="button"
                        onClick={() => void setModes({ spatial_audio: mode })}
                        className={`rounded-xl px-3 py-2 text-xs font-semibold transition-all ${
                          active
                            ? "bg-[#FF7A2F] text-white shadow-sm"
                            : "bg-[#F2F1F6] text-neutral-700 hover:bg-neutral-200/80 dark:bg-white/5 dark:text-neutral-300 dark:hover:bg-white/10"
                        }`}
                      >
                        {t(key)}
                      </button>
                    );
                  })}
              </div>
            </div>
          )}
        </div>
      </section>

      {/* Section 2: High-Res Codecs (LDAC & Experimental LE Audio) */}
      {(caps.ldac || caps.le_audio) && (
        <section className="rounded-2xl border border-neutral-200/80 bg-white p-5 shadow-xs dark:border-white/10 dark:bg-[#1E1E1E]">
          <div className="mb-4 flex items-center gap-2.5">
            <div className="flex h-8 w-8 items-center justify-center rounded-xl bg-[#FF7A2F]/15 text-[#FF7A2F]">
              <Sparkles className="h-4 w-4" />
            </div>
            <h2 className="text-base font-semibold text-neutral-900 dark:text-white">
              {locale === "zh-CN" ? "高解析音频编码" : "High-Resolution Codecs"}
            </h2>
          </div>

          <div className="divide-y divide-neutral-200/70 dark:divide-white/10">
            {caps.ldac && (
              <ToggleRow
                label={t("audio_modes.ldac")}
                description={t("audio_modes.ldac_desc")}
                checked={modes.ldac}
                onChange={(v) => void setModes({ ldac: v })}
                badge="Hi-Res"
              />
            )}

            {caps.le_audio && (
              <ToggleRow
                label={t("audio_modes.le_audio")}
                description={t("audio_modes.le_audio_disclaimer")}
                checked={modes.le_audio}
                onChange={handleToggleLeAudio}
                badge="BETA"
              />
            )}
          </div>
        </section>
      )}

      {/* Section 3: Smart Wear, Ear Tip Fit Test, Hearing Health, Privacy & Voice */}
      {(caps.in_ear_detection ||
        caps.fit_test ||
        caps.hearing_health ||
        caps.privacy_mode_pro ||
        caps.dialogue_perception ||
        caps.speak_to_chat) && (
        <section className="rounded-2xl border border-neutral-200/80 bg-white p-5 shadow-xs dark:border-white/10 dark:bg-[#1E1E1E]">
          <div className="mb-4 flex items-center gap-2.5">
            <div className="flex h-8 w-8 items-center justify-center rounded-xl bg-emerald-500/15 text-emerald-600 dark:text-emerald-400">
              <Ear className="h-4 w-4" />
            </div>
            <h2 className="text-base font-semibold text-neutral-900 dark:text-white">
              {locale === "zh-CN" ? "智能佩戴与声学健康" : "Smart Wear & Acoustic Health"}
            </h2>
          </div>

          <div className="divide-y divide-neutral-200/70 dark:divide-white/10">
            {caps.in_ear_detection && (
              <div className="space-y-3 pb-4">
                <div>
                  <p className="text-sm font-medium text-neutral-900 dark:text-white">
                    {t("wear_and_health.in_ear_title")}
                  </p>
                  <p className="text-xs text-neutral-500 dark:text-neutral-400">
                    {t("wear_and_health.in_ear_desc")}
                  </p>
                </div>

                <div className="grid gap-2 sm:grid-cols-2">
                  {[
                    { id: 0, key: "wear_and_health.in_ear_opt_off" },
                    { id: 1, key: "wear_and_health.in_ear_opt_single_pause_play" },
                    { id: 2, key: "wear_and_health.in_ear_opt_single_pause_no_play" },
                    { id: 3, key: "wear_and_health.in_ear_opt_both_pause_play" },
                  ].map((opt) => (
                    <button
                      key={opt.id}
                      type="button"
                      onClick={() => void setModes({ in_ear_mode: opt.id })}
                      className={`rounded-xl border px-3 py-2 text-left text-xs font-medium transition-all ${
                        modes.in_ear_mode === opt.id
                          ? "border-[#FF7A2F] bg-[#FF7A2F]/10 text-[#FF7A2F]"
                          : "border-neutral-200 bg-[#F2F1F6]/70 text-neutral-700 hover:bg-neutral-200/60 dark:border-white/10 dark:bg-white/5 dark:text-neutral-300"
                      }`}
                    >
                      {t(opt.key)}
                    </button>
                  ))}
                </div>

                <ToggleRow
                  label={t("wear_and_health.in_ear_auto_anc")}
                  description={t("wear_and_health.in_ear_auto_anc_desc")}
                  checked={modes.in_ear_auto_anc}
                  onChange={(v) => void setModes({ in_ear_auto_anc: v })}
                />

                <div className="flex items-center justify-between gap-4">
                  <span className="text-xs font-medium text-neutral-600 dark:text-neutral-300">
                    {t("wear_and_health.in_ear_sensitivity")}
                  </span>
                  <div className="flex gap-1.5">
                    {[
                      { val: 1, key: "gestures.sensitivity_moderate" },
                      { val: 2, key: "gestures.sensitivity_normal" },
                      { val: 3, key: "gestures.sensitivity_sensitive" },
                    ].map((s) => (
                      <button
                        key={s.val}
                        type="button"
                        onClick={() => void setModes({ in_ear_sensitivity: s.val })}
                        className={`rounded-lg px-2.5 py-1 text-xs font-medium ${
                          modes.in_ear_sensitivity === s.val
                            ? "bg-[#0066FF] text-white dark:bg-[#3399FF]"
                            : "bg-[#F2F1F6] text-neutral-600 dark:bg-white/10 dark:text-neutral-300"
                        }`}
                      >
                        {t(s.key)}
                      </button>
                    ))}
                  </div>
                </div>
              </div>
            )}

            {caps.fit_test && (
              <div className="py-4">
                <div className="flex flex-wrap items-center justify-between gap-3">
                  <div>
                    <div className="flex items-center gap-2">
                      <Activity className="h-4 w-4 text-[#FF7A2F]" />
                      <span className="text-sm font-medium text-neutral-900 dark:text-white">
                        {t("wear_and_health.fit_test_title")}
                      </span>
                    </div>
                    <p className="mt-0.5 text-xs text-neutral-500 dark:text-neutral-400">
                      {t("wear_and_health.fit_test_desc")}
                    </p>
                  </div>
                  <button
                    type="button"
                    disabled={fitTesting}
                    onClick={() => void handleFitTest()}
                    className="rounded-xl bg-[#FF7A2F] px-4 py-2 text-xs font-semibold text-white shadow-xs transition hover:bg-[#e86920] disabled:opacity-60"
                  >
                    {fitTesting
                      ? t("wear_and_health.fit_test_testing")
                      : last_fit_test
                        ? t("wear_and_health.fit_test_retest")
                        : t("wear_and_health.fit_test_start")}
                  </button>
                </div>

                {last_fit_test && (
                  <div className="mt-3 grid grid-cols-2 gap-3 rounded-xl bg-[#F2F1F6] p-3 dark:bg-white/5">
                    <div className="flex items-center justify-between">
                      <span className="text-xs font-medium text-neutral-600 dark:text-neutral-300">
                        {t("common.left")}
                      </span>
                      <span
                        className={`inline-flex items-center gap-1 rounded-full px-2.5 py-0.5 text-xs font-semibold ${fitBadgeClass(last_fit_test.left)}`}
                      >
                        <CheckCircle2 className="h-3 w-3" />
                        {fitLabel(last_fit_test.left)}
                      </span>
                    </div>
                    <div className="flex items-center justify-between">
                      <span className="text-xs font-medium text-neutral-600 dark:text-neutral-300">
                        {t("common.right")}
                      </span>
                      <span
                        className={`inline-flex items-center gap-1 rounded-full px-2.5 py-0.5 text-xs font-semibold ${fitBadgeClass(last_fit_test.right)}`}
                      >
                        <CheckCircle2 className="h-3 w-3" />
                        {fitLabel(last_fit_test.right)}
                      </span>
                    </div>
                  </div>
                )}
              </div>
            )}

            {caps.hearing_health && (
              <div className="space-y-2 py-4">
                <div className="flex items-center gap-2 text-sm font-semibold text-neutral-900 dark:text-white">
                  <HeartPulse className="h-4 w-4 text-rose-500" />
                  {t("wear_and_health.hearing_health_title")}
                </div>
                <ToggleRow
                  label={t("wear_and_health.volume_reminder")}
                  description={t("wear_and_health.volume_reminder_desc")}
                  checked={modes.hearing_low_vol_remind}
                  onChange={(v) => void setModes({ hearing_low_vol_remind: v })}
                />
                <ToggleRow
                  label={t("wear_and_health.duration_reminder")}
                  description={t("wear_and_health.duration_reminder_desc")}
                  checked={modes.hearing_duration_remind}
                  onChange={(v) => void setModes({ hearing_duration_remind: v })}
                />
                {modes.hearing_duration_remind && (
                  <div className="flex items-center justify-between pt-1">
                    <span className="text-xs text-neutral-500 dark:text-neutral-400">
                      {t("wear_and_health.duration_threshold")}
                    </span>
                    <div className="flex gap-1.5">
                      {[30, 60, 90, 120].map((m) => (
                        <button
                          key={m}
                          type="button"
                          onClick={() => void setModes({ hearing_duration_min: m })}
                          className={`rounded-lg px-2.5 py-1 text-xs font-medium ${
                            modes.hearing_duration_min === m
                              ? "bg-[#FF7A2F] text-white"
                              : "bg-[#F2F1F6] text-neutral-600 dark:bg-white/10 dark:text-neutral-300"
                          }`}
                        >
                          {t("common.minutes", { count: m })}
                        </button>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            )}

            {caps.privacy_mode_pro && (
              <div className="space-y-2 py-4">
                <ToggleRow
                  label={t("wear_and_health.privacy_mode_title")}
                  description={t("wear_and_health.privacy_mode_desc")}
                  checked={modes.privacy_mode}
                  onChange={(v) => void setModes({ privacy_mode: v })}
                />
                {modes.privacy_mode && (
                  <div className="flex gap-2">
                    {[
                      { id: 0, key: "wear_and_health.privacy_calls_and_music" },
                      { id: 1, key: "wear_and_health.privacy_calls_only" },
                    ].map((s) => (
                      <button
                        key={s.id}
                        type="button"
                        onClick={() => void setModes({ privacy_scope: s.id })}
                        className={`rounded-xl px-3 py-1.5 text-xs font-medium ${
                          modes.privacy_scope === s.id
                            ? "bg-[#0066FF] text-white dark:bg-[#3399FF]"
                            : "bg-[#F2F1F6] text-neutral-700 dark:bg-white/10 dark:text-neutral-300"
                        }`}
                      >
                        <Shield className="mr-1 inline h-3 w-3" />
                        {t(s.key)}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            )}

            {caps.dialogue_perception && (
              <ToggleRow
                label={t("wear_and_health.dialogue_perception_title")}
                description={t("wear_and_health.dialogue_perception_desc")}
                checked={modes.dialogue_perception}
                onChange={(v) => void setModes({ dialogue_perception: v })}
              />
            )}

            {caps.speak_to_chat && (
              <div className="space-y-2 pt-4">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <MessageSquare className="h-4 w-4 text-[#0066FF] dark:text-[#3399FF]" />
                    <span className="text-sm font-medium text-neutral-900 dark:text-white">
                      {t("wear_and_health.speak_to_chat_title")}
                    </span>
                  </div>
                  <div className="flex gap-1.5">
                    {[0, 5, 10, 15, 20].map((sec) => (
                      <button
                        key={sec}
                        type="button"
                        onClick={() => void setModes({ speak_to_chat_sec: sec })}
                        className={`rounded-lg px-2.5 py-1 text-xs font-medium ${
                          modes.speak_to_chat_sec === sec
                            ? "bg-[#FF7A2F] text-white"
                            : "bg-[#F2F1F6] text-neutral-600 dark:bg-white/10 dark:text-neutral-300"
                        }`}
                      >
                        {sec === 0 ? t("common.off") : t("common.seconds", { count: sec })}
                      </button>
                    ))}
                  </div>
                </div>
                <p className="text-xs text-neutral-500 dark:text-neutral-400">
                  {t("wear_and_health.speak_to_chat_desc")}
                </p>
              </div>
            )}
          </div>
        </section>
      )}

      {/* Section 4: Dual-Device & Multi-Device Manager */}
      {(caps.dual_device || caps.multi_device) && (
        <section className="rounded-2xl border border-neutral-200/80 bg-white p-5 shadow-xs dark:border-white/10 dark:bg-[#1E1E1E]">
          <div className="mb-4 flex items-center gap-2.5">
            <div className="flex h-8 w-8 items-center justify-center rounded-xl bg-violet-500/15 text-violet-600 dark:text-violet-400">
              <Bluetooth className="h-4 w-4" />
            </div>
            <h2 className="text-base font-semibold text-neutral-900 dark:text-white">
              {caps.multi_device
                ? t("connectivity.multi_device_title")
                : t("connectivity.dual_device_title")}
            </h2>
          </div>

          <div className="space-y-4">
            <ToggleRow
              label={t("connectivity.dual_device_title")}
              description={t("connectivity.dual_device_desc")}
              checked={multipoint.dual_device}
              onChange={(v) => void setMultipoint({ dual_device: v })}
            />

            {caps.multi_device && (
              <div className="space-y-2.5 rounded-xl bg-[#F2F1F6] p-3.5 dark:bg-white/5">
                <div className="flex items-center justify-between">
                  <span className="text-xs font-semibold uppercase tracking-wide text-neutral-500 dark:text-neutral-400">
                    {t("connectivity.connected_devices")} ({multipoint.hosts.length})
                  </span>
                  <button
                    type="button"
                    onClick={() => void setMultipoint({ enter_pairing: true })}
                    className="rounded-lg bg-[#0066FF]/10 px-2.5 py-1 text-xs font-semibold text-[#0066FF] hover:bg-[#0066FF]/20 dark:bg-[#3399FF]/15 dark:text-[#3399FF]"
                  >
                    + {t("connectivity.pair_new_device")}
                  </button>
                </div>

                {multipoint.hosts.map((host) => (
                  <div
                    key={host.mac}
                    className="flex items-center justify-between rounded-xl bg-white px-3 py-2.5 dark:bg-[#1E1E1E]"
                  >
                    <div className="flex items-center gap-2.5">
                      <Laptop className="h-4 w-4 text-neutral-500" />
                      <div>
                        <div className="flex items-center gap-2">
                          <span className="text-sm font-medium text-neutral-900 dark:text-white">
                            {host.name}
                          </span>
                          {host.is_this_device && (
                            <span className="rounded-full bg-emerald-500/15 px-2 py-0.5 text-[10px] font-semibold text-emerald-600 dark:text-emerald-400">
                              {t("connectivity.this_pc")}
                            </span>
                          )}
                        </div>
                        <p className="font-mono text-[11px] text-neutral-400">{host.mac}</p>
                      </div>
                    </div>

                    {!host.is_this_device && (
                      <button
                        type="button"
                        onClick={() =>
                          void setMultipoint(
                            host.is_connected
                              ? { disconnect_mac: host.mac }
                              : { connect_mac: host.mac }
                          )
                        }
                        className="rounded-lg border border-neutral-200 px-2.5 py-1 text-xs font-medium text-neutral-700 hover:bg-neutral-100 dark:border-white/10 dark:text-neutral-300 dark:hover:bg-white/10"
                      >
                        {host.is_connected
                          ? t("connectivity.disconnect_peer")
                          : t("connectivity.connect_peer")}
                      </button>
                    )}
                  </div>
                ))}
              </div>
            )}
          </div>
        </section>
      )}

      {/* Section 5: Prompts, Power Management & Hardware Clock/Alarms (0x3E/0x3F) */}
      <section className="rounded-2xl border border-neutral-200/80 bg-white p-5 shadow-xs dark:border-white/10 dark:bg-[#1E1E1E]">
        <div className="mb-4 flex items-center gap-2.5">
          <div className="flex h-8 w-8 items-center justify-center rounded-xl bg-amber-500/15 text-amber-600 dark:text-amber-400">
            <Power className="h-4 w-4" />
          </div>
          <h2 className="text-base font-semibold text-neutral-900 dark:text-white">
            {t("device_settings.section_prompts")} &amp; {t("device_settings.section_power")}
          </h2>
        </div>

        <div className="divide-y divide-neutral-200/70 dark:divide-white/10">
          {caps.prompt_language && (
            <div className="flex items-center justify-between py-3 first:pt-0">
              <div className="flex items-center gap-2">
                <Radio className="h-4 w-4 text-neutral-400" />
                <span className="text-sm font-medium text-neutral-900 dark:text-white">
                  {t("device_settings.prompt_language")}
                </span>
              </div>
              <div className="flex gap-1.5">
                {(["en", "zh"] as PromptLanguage[]).map((lang) => (
                  <button
                    key={lang}
                    type="button"
                    onClick={() => void setModes({ prompt_language: lang })}
                    className={`rounded-lg px-3 py-1.5 text-xs font-semibold ${
                      modes.prompt_language === lang
                        ? "bg-[#FF7A2F] text-white"
                        : "bg-[#F2F1F6] text-neutral-700 dark:bg-white/10 dark:text-neutral-300"
                    }`}
                  >
                    {lang === "en"
                      ? t("device_settings.lang_english")
                      : t("device_settings.lang_chinese")}
                  </button>
                ))}
              </div>
            </div>
          )}

          {caps.prompt_volume && (
            <div className="flex items-center justify-between py-3">
              <div className="flex items-center gap-2">
                <Volume2 className="h-4 w-4 text-neutral-400" />
                <span className="text-sm font-medium text-neutral-900 dark:text-white">
                  {t("device_settings.prompt_volume")}
                </span>
              </div>
              <div className="flex gap-1.5">
                {[
                  { val: 4, key: "device_settings.vol_low" },
                  { val: 6, key: "device_settings.vol_medium" },
                  { val: 8, key: "device_settings.vol_high" },
                  { val: 10, key: "device_settings.vol_max" },
                ].map((v) => (
                  <button
                    key={v.val}
                    type="button"
                    onClick={() => void setModes({ prompt_volume: v.val })}
                    className={`rounded-lg px-2.5 py-1 text-xs font-medium ${
                      modes.prompt_volume === v.val
                        ? "bg-[#0066FF] text-white dark:bg-[#3399FF]"
                        : "bg-[#F2F1F6] text-neutral-600 dark:bg-white/10 dark:text-neutral-300"
                    }`}
                  >
                    {t(v.key)}
                  </button>
                ))}
              </div>
            </div>
          )}

          {caps.auto_power_off && (
            <div className="flex flex-wrap items-center justify-between gap-2 py-3">
              <div>
                <p className="text-sm font-medium text-neutral-900 dark:text-white">
                  {t("device_settings.auto_power_off")}
                </p>
                <p className="text-xs text-neutral-500 dark:text-neutral-400">
                  {t("device_settings.auto_power_off_desc")}
                </p>
              </div>
              <div className="flex flex-wrap gap-1.5">
                {[0, 15, 30, 60, 90].map((min) => (
                  <button
                    key={min}
                    type="button"
                    onClick={() => void setModes({ auto_power_off_min: min })}
                    className={`rounded-lg px-2.5 py-1 text-xs font-medium ${
                      modes.auto_power_off_min === min
                        ? "bg-[#FF7A2F] text-white"
                        : "bg-[#F2F1F6] text-neutral-600 dark:bg-white/10 dark:text-neutral-300"
                    }`}
                  >
                    {min === 0 ? t("device_settings.power_off_never") : t("common.minutes", { count: min })}
                  </button>
                ))}
              </div>
            </div>
          )}

          {caps.disconnect_power_off && (
            <div className="flex flex-wrap items-center justify-between gap-2 py-3">
              <div>
                <p className="text-sm font-medium text-neutral-900 dark:text-white">
                  {t("device_settings.disconnect_power_off")}
                </p>
                <p className="text-xs text-neutral-500 dark:text-neutral-400">
                  {t("device_settings.disconnect_power_off_desc")}
                </p>
              </div>
              <div className="flex flex-wrap gap-1.5">
                {[5, 10, 30, 60, 0].map((min) => (
                  <button
                    key={min}
                    type="button"
                    onClick={() => void setModes({ disconnect_power_off_min: min })}
                    className={`rounded-lg px-2.5 py-1 text-xs font-medium ${
                      modes.disconnect_power_off_min === min
                        ? "bg-[#0066FF] text-white dark:bg-[#3399FF]"
                        : "bg-[#F2F1F6] text-neutral-600 dark:bg-white/10 dark:text-neutral-300"
                    }`}
                  >
                    {min === 0 ? t("device_settings.power_off_never") : t("common.minutes", { count: min })}
                  </button>
                ))}
              </div>
            </div>
          )}

          {caps.power_saving && (
            <ToggleRow
              label={t("device_settings.power_saving")}
              description={t("device_settings.power_saving_desc")}
              checked={modes.power_saving}
              onChange={(v) => void setModes({ power_saving: v })}
            />
          )}

          {caps.disconnect_warning && (
            <ToggleRow
              label={t("device_settings.disconnect_warning")}
              description={t("device_settings.disconnect_warning_desc")}
              checked={modes.disconnect_warning}
              onChange={(v) => void setModes({ disconnect_warning: v })}
            />
          )}

          {/* Hardware Clock Sync & Alarms (0x3E / 0x3F) */}
          <div className="space-y-3 pt-4">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <AlarmClock className="h-4 w-4 text-[#FF7A2F]" />
                <div>
                  <p className="text-sm font-medium text-neutral-900 dark:text-white">
                    {locale === "zh-CN" ? "设备时钟同步与硬件闹钟" : "Device Clock Sync & Alarms"}
                  </p>
                  <p className="text-xs text-neutral-500 dark:text-neutral-400">
                    {clockSyncedAt
                      ? locale === "zh-CN"
                        ? `已同步至系统时间 (${clockSyncedAt})`
                        : `Synced to PC clock at ${clockSyncedAt}`
                      : locale === "zh-CN"
                        ? "通过 BLE 0x3E/0x3F 同步时间戳与耳机本地闹钟"
                        : "Sync PC timestamp & onboard alarms over BLE (0x3E / 0x3F)"}
                  </p>
                </div>
              </div>
              <button
                type="button"
                onClick={handleSyncClock}
                className="rounded-xl bg-[#F2F1F6] px-3 py-1.5 text-xs font-semibold text-neutral-800 hover:bg-neutral-200 dark:bg-white/10 dark:text-white dark:hover:bg-white/15"
              >
                <BatteryCharging className="mr-1 inline h-3.5 w-3.5 text-[#FF7A2F]" />
                {locale === "zh-CN" ? "同步时间" : "Sync Clock"}
              </button>
            </div>

            <div className="grid gap-2 sm:grid-cols-2">
              {alarms.map((alarm) => (
                <div
                  key={alarm.id}
                  className="flex items-center justify-between rounded-xl bg-[#F2F1F6] px-3.5 py-2.5 dark:bg-white/5"
                >
                  <div className="flex items-center gap-2.5">
                    <Moon className="h-4 w-4 text-neutral-400" />
                    <div>
                      <input
                        type="time"
                        value={alarm.time}
                        onChange={(e) =>
                          setAlarms((prev) =>
                            prev.map((a) =>
                              a.id === alarm.id ? { ...a, time: e.target.value } : a
                            )
                          )
                        }
                        className="bg-transparent font-mono text-sm font-semibold text-neutral-900 outline-none dark:text-white"
                      />
                      <p className="text-[11px] text-neutral-500">{alarm.label}</p>
                    </div>
                  </div>
                  <button
                    type="button"
                    role="switch"
                    aria-checked={alarm.enabled}
                    onClick={() =>
                      setAlarms((prev) =>
                        prev.map((a) =>
                          a.id === alarm.id ? { ...a, enabled: !a.enabled } : a
                        )
                      )
                    }
                    className={`relative inline-flex h-5 w-9 items-center rounded-full transition-colors ${
                      alarm.enabled ? "bg-[#FF7A2F]" : "bg-neutral-300 dark:bg-neutral-700"
                    }`}
                  >
                    <span
                      className={`inline-block h-3.5 w-3.5 transform rounded-full bg-white transition-transform ${
                        alarm.enabled ? "translate-x-5" : "translate-x-0.5"
                      }`}
                    />
                  </button>
                </div>
              ))}
            </div>
          </div>
        </div>
      </section>

      {/* Experimental LE Audio 3-second Disclaimer Modal */}
      {leCountdown !== null && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4 backdrop-blur-xs">
          <div className="w-full max-w-md rounded-2xl bg-white p-6 shadow-2xl dark:bg-[#1E1E1E]">
            <div className="mb-3 flex items-center gap-2.5 text-amber-500">
              <AlertTriangle className="h-5 w-5" />
              <h3 className="text-base font-semibold text-neutral-900 dark:text-white">
                {t("audio_modes.le_audio_title")}
              </h3>
            </div>
            <p className="text-sm text-neutral-600 dark:text-neutral-300">
              {t("audio_modes.le_audio_disclaimer")}
            </p>
            <div className="mt-5 flex justify-end gap-2">
              <button
                type="button"
                onClick={() => setLeCountdown(null)}
                className="rounded-xl bg-[#F2F1F6] px-4 py-2 text-xs font-semibold text-neutral-700 dark:bg-white/10 dark:text-neutral-200"
              >
                {t("common.cancel")}
              </button>
              <button
                type="button"
                disabled={leCountdown > 0}
                onClick={confirmLeAudio}
                className="rounded-xl bg-[#FF7A2F] px-4 py-2 text-xs font-semibold text-white disabled:opacity-50"
              >
                {leCountdown > 0
                  ? t("audio_modes.le_audio_countdown", { seconds: leCountdown })
                  : t("audio_modes.le_audio_enable")}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Subtle footer badge */}
      <div className="flex items-center justify-end gap-1.5 text-[11px] text-neutral-400">
        <Headphones className="h-3.5 w-3.5" />
        <span>
          {snapshot.active_model.name_en} · {snapshot.active_model.protocol_family}
        </span>
      </div>
    </div>
  );
};

export default ModesLabPage;
