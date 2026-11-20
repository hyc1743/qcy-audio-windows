import React, { useEffect, useRef, useState } from "react";
import {
  AlertTriangle,
  BookOpen,
  Clock,
  Headphones,
  Info,
  Volume2,
  VolumeX,
} from "lucide-react";
import { useQcyStore } from "../store/useQcyStore";
import { useI18n } from "../i18n";
import {
  playFindEarbudsChime,
  type PlaybackHandle,
  type StereoSide,
} from "../audio/audioUtils";
import { DeviceIllustration, resolveDeviceColorPreset } from "../assets/DeviceIllustrations";

export const FindEarbudsPage: React.FC = () => {
  const { snapshot } = useQcyStore();
  const { t, locale } = useI18n();
  const [ringingSide, setRingingSide] = useState<StereoSide | null>(null);
  const handleRef = useRef<PlaybackHandle | null>(null);

  const stopChime = () => {
    handleRef.current?.stop();
    handleRef.current = null;
    setRingingSide(null);
  };

  useEffect(() => {
    return () => {
      handleRef.current?.stop();
    };
  }, []);

  const startChime = (side: StereoSide) => {
    stopChime();
    setRingingSide(side);
    const handle = playFindEarbudsChime({ side, pulses: 8, maxGain: 0.32 });
    handleRef.current = handle;
    // Clear UI state once scheduled pulses finish (~4.6 s)
    setTimeout(() => {
      if (handleRef.current === handle) {
        handleRef.current = null;
        setRingingSide(null);
      }
    }, 4700);
  };

  if (!snapshot) {
    return (
      <div className="rounded-2xl bg-white p-8 text-center text-sm text-neutral-500 dark:bg-[#1E1E1E] dark:text-neutral-400">
        {t("common.loading")}
      </div>
    );
  }

  const colorHex = resolveDeviceColorPreset(snapshot.device_color_index).hex;
  const isDark = snapshot.settings.theme !== "light";

  return (
    <div className="space-y-5">
      {/* Hearing Safety Warning Banner */}
      <div className="flex items-start gap-3 rounded-2xl border border-rose-500/30 bg-[#FFE6E6] p-4 text-rose-800 dark:bg-[#561515] dark:text-rose-200">
        <AlertTriangle className="mt-0.5 h-5 w-5 shrink-0 text-[#FF4C4C]" />
        <div className="space-y-1 text-xs">
          <p className="font-semibold">{t("find_earbuds.warning_remove_before_ring")}</p>
          <p className="opacity-85">
            {t("find_earbuds.a2dp_required_notice")} · {t("find_earbuds.volume_ramp_notice")}
          </p>
        </div>
      </div>

      {/* Interactive Acoustic Locator Card */}
      <section className="rounded-2xl border border-neutral-200/80 bg-white p-6 shadow-xs dark:border-white/10 dark:bg-[#1E1E1E]">
        <div className="flex flex-col items-center gap-6 md:flex-row">
          <div className="relative flex flex-col items-center">
            <DeviceIllustration
              formFactor={snapshot.active_model.form_factor}
              deviceColor={colorHex}
              theme={isDark ? "dark" : "light"}
              leftConnected={snapshot.battery.left !== null}
              rightConnected={snapshot.battery.right !== null}
              leftCharging={snapshot.battery.left_charging}
              rightCharging={snapshot.battery.right_charging}
              caseCharging={snapshot.battery.case_charging}
              width={210}
              height={140}
            />
            {ringingSide && (
              <span className="mt-1 inline-flex animate-pulse items-center gap-1.5 rounded-full bg-[#FF7A2F]/15 px-3 py-1 text-xs font-semibold text-[#FF7A2F]">
                <Volume2 className="h-3.5 w-3.5" />
                {t("find_earbuds.ringing_active")} ({ringingSide.toUpperCase()})
              </span>
            )}
          </div>

          <div className="flex-1 space-y-4">
            <div>
              <h2 className="text-lg font-semibold text-neutral-900 dark:text-white">
                {t("find_earbuds.title")}
              </h2>
              <p className="mt-1 text-xs text-neutral-500 dark:text-neutral-400">
                {t("find_earbuds.subtitle")}
              </p>
            </div>

            <div className="grid gap-2.5 sm:grid-cols-3">
              <button
                type="button"
                onClick={() => startChime("left")}
                className={`flex items-center justify-center gap-2 rounded-xl px-4 py-3 text-xs font-semibold transition ${
                  ringingSide === "left"
                    ? "bg-[#FF7A2F] text-white shadow-md shadow-[#FF7A2F]/30"
                    : "bg-[#F2F1F6] text-neutral-800 hover:bg-neutral-200 dark:bg-white/10 dark:text-white dark:hover:bg-white/15"
                }`}
              >
                <Volume2 className="h-4 w-4" />
                {t("find_earbuds.ring_left")}
              </button>

              <button
                type="button"
                onClick={() => startChime("right")}
                className={`flex items-center justify-center gap-2 rounded-xl px-4 py-3 text-xs font-semibold transition ${
                  ringingSide === "right"
                    ? "bg-[#FF7A2F] text-white shadow-md shadow-[#FF7A2F]/30"
                    : "bg-[#F2F1F6] text-neutral-800 hover:bg-neutral-200 dark:bg-white/10 dark:text-white dark:hover:bg-white/15"
                }`}
              >
                <Volume2 className="h-4 w-4" />
                {t("find_earbuds.ring_right")}
              </button>

              <button
                type="button"
                onClick={() => startChime("both")}
                className={`flex items-center justify-center gap-2 rounded-xl px-4 py-3 text-xs font-semibold transition ${
                  ringingSide === "both"
                    ? "bg-[#0066FF] text-white shadow-md shadow-[#0066FF]/30"
                    : "bg-[#0066FF]/12 text-[#0066FF] hover:bg-[#0066FF]/20 dark:bg-[#3399FF]/15 dark:text-[#3399FF]"
                }`}
              >
                <Headphones className="h-4 w-4" />
                {t("find_earbuds.ring_both")}
              </button>
            </div>

            {ringingSide && (
              <button
                type="button"
                onClick={stopChime}
                className="inline-flex items-center gap-2 rounded-xl bg-rose-600 px-4 py-2 text-xs font-semibold text-white hover:bg-rose-700"
              >
                <VolumeX className="h-4 w-4" />
                {t("find_earbuds.stop_ringing")}
              </button>
            )}
          </div>
        </div>
      </section>

      {/* Last-Connected / Last-Seen Log */}
      <section className="rounded-2xl border border-neutral-200/80 bg-white p-5 shadow-xs dark:border-white/10 dark:bg-[#1E1E1E]">
        <div className="mb-3 flex items-center gap-2">
          <Clock className="h-4 w-4 text-[#0066FF] dark:text-[#3399FF]" />
          <h3 className="text-sm font-semibold text-neutral-900 dark:text-white">
            {t("find_earbuds.last_seen_title")}
          </h3>
        </div>

        {snapshot.saved_devices.length === 0 ? (
          <p className="text-xs text-neutral-500 dark:text-neutral-400">
            {t("find_earbuds.last_seen_empty")}
          </p>
        ) : (
          <div className="space-y-2">
            {snapshot.saved_devices.map((dev) => (
              <div
                key={dev.address}
                className="flex flex-wrap items-center justify-between gap-2 rounded-xl bg-[#F2F1F6] px-4 py-3 text-xs dark:bg-white/5"
              >
                <div>
                  <p className="font-semibold text-neutral-900 dark:text-white">{dev.name}</p>
                  <p className="font-mono text-[11px] text-neutral-500">{dev.address}</p>
                </div>
                <div className="text-right">
                  <p className="text-neutral-700 dark:text-neutral-300">
                    {t("find_earbuds.last_connected_at", {
                      time: new Date(dev.last_connected_ms).toLocaleString(),
                    })}
                  </p>
                  {dev.last_battery && (
                    <p className="font-mono text-[11px] text-neutral-500">
                      L {dev.last_battery.left ?? "—"}% · R {dev.last_battery.right ?? "—"}%
                      {dev.last_battery.case_level !== null
                        ? ` · ${t("common.case")} ${dev.last_battery.case_level}%`
                        : ""}
                    </p>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </section>

      {/* Clean Offline Quick-Start & Troubleshooting Guide */}
      <section className="rounded-2xl border border-neutral-200/80 bg-white p-5 shadow-xs dark:border-white/10 dark:bg-[#1E1E1E]">
        <div className="mb-4 flex items-center gap-2">
          <BookOpen className="h-4 w-4 text-[#FF7A2F]" />
          <div>
            <h3 className="text-sm font-semibold text-neutral-900 dark:text-white">
              {t("app_settings.guides_title")}
            </h3>
            <p className="text-xs text-neutral-500 dark:text-neutral-400">
              {t("app_settings.guides_subtitle")}
            </p>
          </div>
        </div>

        <div className="grid gap-3 md:grid-cols-3">
          <div className="rounded-xl bg-[#F2F1F6] p-3.5 dark:bg-white/5">
            <p className="mb-1.5 text-xs font-semibold text-neutral-900 dark:text-white">
              1. {t("app_settings.guide_pairing_title")}
            </p>
            <p className="text-xs leading-relaxed text-neutral-600 dark:text-neutral-400">
              {locale === "zh-CN"
                ? "将双耳放入充电盒并开盖，长按充电盒功能键约 3 秒直至指示灯白/蓝闪烁，即可在 Windows 蓝牙设置或本软件扫描页发现设备。"
                : "Place both earbuds inside the charging case with lid open, then hold the case button for ~3 seconds until the status LED flashes to enter pairing mode."}
            </p>
          </div>

          <div className="rounded-xl bg-[#F2F1F6] p-3.5 dark:bg-white/5">
            <p className="mb-1.5 text-xs font-semibold text-neutral-900 dark:text-white">
              2. {t("app_settings.guide_reset_title")}
            </p>
            <p className="text-xs leading-relaxed text-neutral-600 dark:text-neutral-400">
              {locale === "zh-CN"
                ? "耳机放回充电盒并保持开盖，长按充电盒按键约 10 秒直至指示灯红白快闪 5 次，随后在 Windows 中删除旧配对记录并重新配对。"
                : "Keep both earbuds in the open case and press-hold the case button for ~10 seconds until the LED flashes 5 times. Forget the old entry in Windows Bluetooth settings and re-pair."}
            </p>
          </div>

          <div className="rounded-xl bg-[#F2F1F6] p-3.5 dark:bg-white/5">
            <p className="mb-1.5 text-xs font-semibold text-neutral-900 dark:text-white">
              3. {t("app_settings.guide_controls_title")}
            </p>
            <p className="text-xs leading-relaxed text-neutral-600 dark:text-neutral-400">
              {locale === "zh-CN"
                ? "默认双击播放/暂停，三击左耳上一曲/右耳下一曲，长按切换降噪/通透模式；可在“手势”页面按需自定义每项触控功能。"
                : "Default: double-tap to Play/Pause, triple-tap L/R for Previous/Next track, long-press to cycle ANC/Transparency. Customize every gesture freely in the Gestures tab."}
            </p>
          </div>
        </div>

        <div className="mt-3 flex items-center gap-2 text-[11px] text-neutral-500 dark:text-neutral-400">
          <Info className="h-3.5 w-3.5 shrink-0" />
          <span>{t("app_settings.about_privacy_note")}</span>
        </div>
      </section>
    </div>
  );
};

export default FindEarbudsPage;
