import React, { useEffect, useState } from "react";
import { ExternalLink, Shield, Sparkles, X, Zap } from "lucide-react";
import {
  DeviceIllustration,
  resolveDeviceColorPreset,
} from "../assets/DeviceIllustrations";
import { useI18n } from "../i18n";
import type { AncPrimaryMode, PopupTriggerPayload } from "../types/qcy";

export interface ConnectionPopupProps {
  payload: PopupTriggerPayload;
  theme?: "light" | "dark";
  /** Callback when user clicks "Open QCY Audio" (focuses main window). */
  onOpenApp?: () => void;
  /** Callback when popup closes or auto-dismisses. */
  onDismiss?: () => void;
  /** Optional quick ANC mode toggle callback right from the popup card. */
  onQuickToggleAnc?: (nextMode: AncPrimaryMode) => void;
  /** Disable auto-dismiss when rendering inline preview if desired (default true = auto-dismiss). */
  autoDismiss?: boolean;
}

function BatteryPill({
  label,
  percent,
  charging,
}: {
  label: string;
  percent: number | null;
  charging: boolean;
}) {
  if (percent === null) return null;
  const barColor =
    percent <= 20 ? "bg-[#FF4C4C]" : percent <= 40 ? "bg-amber-400" : "bg-[#22C55E]";

  return (
    <div className="flex flex-col items-center gap-1 rounded-xl bg-neutral-100/90 px-2.5 py-2 dark:bg-white/8">
      <div className="flex items-center gap-1 text-[11px] font-semibold text-neutral-700 dark:text-neutral-200">
        <span>{label}</span>
        {charging && <Zap className="h-3 w-3 text-[#22C55E]" />}
      </div>
      <div className="h-1.5 w-12 overflow-hidden rounded-full bg-neutral-300/80 dark:bg-white/15">
        <div
          className={`h-full rounded-full transition-all ${barColor}`}
          style={{ width: `${Math.max(0, Math.min(100, percent))}%` }}
        />
      </div>
      <span className="font-mono text-[11px] font-bold text-neutral-900 dark:text-white">
        {percent}%
      </span>
    </div>
  );
}

/**
 * Standalone Connection Pop-Up Overlay Card.
 * Renders cleanly inside the frameless transparent always-on-top Tauri `popup` window
 * (`index.html#popup`) and can also be previewed inline inside the main window.
 */
export const ConnectionPopup: React.FC<ConnectionPopupProps> = ({
  payload,
  theme = "dark",
  onOpenApp,
  onDismiss,
  onQuickToggleAnc,
  autoDismiss = true,
}) => {
  const { t } = useI18n();
  const [paused, setPaused] = useState(false);

  useEffect(() => {
    if (!autoDismiss || paused || !onDismiss) return;
    const ms = Math.max(1500, payload.auto_dismiss_ms || 5000);
    const timer = setTimeout(onDismiss, ms);
    return () => clearTimeout(timer);
  }, [autoDismiss, paused, onDismiss, payload.auto_dismiss_ms]);

  const colorHex = resolveDeviceColorPreset(payload.color_index).hex;
  const ancLabel =
    payload.anc_mode === "anc"
      ? t("anc.mode_anc")
      : payload.anc_mode === "transparency"
        ? t("anc.mode_transparency")
        : t("anc.mode_off");

  const nextAncMode: AncPrimaryMode =
    payload.anc_mode === "anc"
      ? "transparency"
      : payload.anc_mode === "transparency"
        ? "off"
        : "anc";

  return (
    <div
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
      className={`w-[344px] select-none rounded-3xl border p-4 shadow-2xl backdrop-blur-xl transition-all ${
        theme === "dark"
          ? "border-white/15 bg-[#1E1E1E]/95 text-white"
          : "border-neutral-200/90 bg-white/95 text-[#1A1A1A]"
      }`}
    >
      {/* Top Bar */}
      <div className="mb-2 flex items-center justify-between gap-2">
        <div className="min-w-0">
          <div className="flex items-center gap-1.5">
            <span className="inline-block h-2 w-2 rounded-full bg-[#22C55E]" />
            <span className="text-[11px] font-semibold text-[#22C55E]">
              {t("popup.connected_badge")}
            </span>
            {payload.is_virtual && (
              <span className="inline-flex items-center gap-0.5 rounded-full bg-[#FF7A2F]/15 px-1.5 py-0.5 text-[9px] font-bold text-[#FF7A2F]">
                <Sparkles className="h-2.5 w-2.5" />
                SIM
              </span>
            )}
          </div>
          <h2 className="truncate text-sm font-bold">{payload.device_name}</h2>
        </div>

        <button
          type="button"
          aria-label={t("popup.dismiss")}
          onClick={onDismiss}
          className="rounded-full p-1 text-neutral-400 transition hover:bg-neutral-500/15 hover:text-neutral-600 dark:hover:text-white"
        >
          <X className="h-4 w-4" />
        </button>
      </div>

      {/* Center Device Illustration + Battery Pills */}
      <div className="flex items-center justify-between gap-3 py-1">
        <DeviceIllustration
          formFactor={payload.form_factor}
          deviceColor={colorHex}
          theme={theme}
          leftConnected={payload.battery.left !== null}
          rightConnected={payload.battery.right !== null}
          leftInCase={payload.battery.left_in_case}
          rightInCase={payload.battery.right_in_case}
          leftCharging={payload.battery.left_charging}
          rightCharging={payload.battery.right_charging}
          caseCharging={payload.battery.case_charging}
          width={136}
          height={90}
        />

        <div className="flex flex-wrap justify-end gap-1.5">
          <BatteryPill
            label={t("popup.battery_left")}
            percent={payload.battery.left}
            charging={payload.battery.left_charging}
          />
          <BatteryPill
            label={t("popup.battery_right")}
            percent={payload.battery.right}
            charging={payload.battery.right_charging}
          />
          <BatteryPill
            label={t("popup.battery_case")}
            percent={payload.battery.case_level}
            charging={payload.battery.case_charging}
          />
        </div>
      </div>

      {/* Bottom Action Bar */}
      <div className="mt-3 flex items-center justify-between gap-2">
        <button
          type="button"
          onClick={() => onQuickToggleAnc?.(nextAncMode)}
          className="inline-flex items-center gap-1.5 rounded-xl bg-[#0066FF]/12 px-3 py-1.5 text-xs font-semibold text-[#0066FF] transition hover:bg-[#0066FF]/20 dark:bg-[#3399FF]/15 dark:text-[#3399FF]"
        >
          <Shield className="h-3.5 w-3.5" />
          {ancLabel}
        </button>

        <button
          type="button"
          onClick={onOpenApp}
          className="inline-flex items-center gap-1.5 rounded-xl bg-[#FF7A2F] px-3.5 py-1.5 text-xs font-semibold text-white shadow-xs transition hover:bg-[#e86920]"
        >
          <ExternalLink className="h-3.5 w-3.5" />
          {t("popup.open_app")}
        </button>
      </div>
    </div>
  );
};

export default ConnectionPopup;
