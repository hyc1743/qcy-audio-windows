import React from "react";
import { Bluetooth, ExternalLink, Radio } from "lucide-react";
import { DeviceIllustration } from "../assets/DeviceIllustrations";
import { QCY_CARD_CLASS, TranslateFn, UiTheme } from "./shared";

export interface DeviceDisconnectedStateProps {
  /** Hero variant for Dashboard (shows earbud silhouette + pairing tips + Bluetooth Settings CTA). */
  variant?: "hero" | "compact";
  /** Feature label inserted into `empty_state.device_required_desc` for compact variant. */
  featureLabel?: string;
  theme?: UiTheme;
  onScanAndConnect?: () => void;
  onGoToDevices?: () => void;
  onOpenBluetoothSettings?: () => void;
  t: TranslateFn;
}

export const DeviceDisconnectedState: React.FC<DeviceDisconnectedStateProps> = ({
  variant = "compact",
  featureLabel,
  theme = "dark",
  onScanAndConnect,
  onGoToDevices,
  onOpenBluetoothSettings,
  t,
}) => {
  if (variant === "hero") {
    return (
      <section
        className={`${QCY_CARD_CLASS} relative overflow-hidden px-6 py-10 text-center`}
        aria-label={t("empty_state.no_device_title")}
      >
        <div
          className="pointer-events-none absolute -top-28 left-1/2 h-56 w-56 -translate-x-1/2 rounded-full opacity-12 blur-3xl dark:opacity-15"
          style={{ background: "radial-gradient(circle, #FF7A2F 0%, transparent 70%)" }}
        />

        <div className="relative mx-auto flex max-w-lg flex-col items-center gap-4">
          {/* Subtle disconnected QCY earbud silhouette */}
          <div className="rounded-3xl bg-[#F2F1F6]/90 px-6 py-4 dark:bg-white/[0.04]">
            <DeviceIllustration
              formFactor="tws"
              deviceColor={theme === "dark" ? "#3A3F4B" : "#D6DBE4"}
              theme={theme}
              leftConnected={false}
              rightConnected={false}
              showCase={true}
              width={216}
              height={144}
            />
          </div>

          <div className="space-y-1.5">
            <span className="inline-flex items-center gap-1.5 rounded-full border border-[#E5E5E5] bg-[#F2F1F6] px-3 py-0.5 text-xs font-semibold text-[#6B7280] dark:border-white/10 dark:bg-white/[0.06] dark:text-white/60">
              <span className="h-2 w-2 rounded-full bg-[#9CA3AF]" />
              {t("connection.not_connected")}
            </span>
            <h2 className="text-xl font-bold tracking-tight text-[#1A1A1A] dark:text-white">
              {t("empty_state.no_device_title")}
            </h2>
            <p className="text-sm leading-relaxed text-[#4D4D4D] dark:text-white/65">
              {t("empty_state.no_device_desc")}
            </p>
          </div>

          {/* Primary & Secondary Actions */}
          <div className="mt-1 flex flex-wrap items-center justify-center gap-3">
            {onScanAndConnect && (
              <button
                type="button"
                onClick={onScanAndConnect}
                className="inline-flex items-center gap-2 rounded-xl bg-[#FF7A2F] px-5 py-2.5 text-sm font-semibold text-white shadow-sm transition hover:bg-[#E8691F]"
              >
                <Radio className="h-4 w-4" />
                {t("empty_state.scan_and_connect")}
              </button>
            )}
            {onOpenBluetoothSettings && (
              <button
                type="button"
                onClick={onOpenBluetoothSettings}
                className="inline-flex items-center gap-2 rounded-xl border border-[#E5E5E5] bg-[#F2F1F6] px-4 py-2.5 text-sm font-semibold text-[#1A1A1A] transition hover:border-[#0066FF]/50 dark:border-white/12 dark:bg-white/[0.06] dark:text-white/85 dark:hover:bg-white/10"
              >
                <ExternalLink className="h-4 w-4 text-[#0066FF] dark:text-[#3399FF]" />
                {t("connection.open_windows_bt_settings")}
              </button>
            )}
          </div>

          {/* Helpful quick tips */}
          <div className="mt-3 grid w-full gap-2.5 text-left sm:grid-cols-2">
            <div className="rounded-xl border border-[#E5E5E5]/80 bg-[#F2F1F6]/70 px-3.5 py-2.5 text-xs text-[#4D4D4D] dark:border-white/8 dark:bg-white/[0.03] dark:text-white/65">
              {t("empty_state.tip_open_case")}
            </div>
            <div className="rounded-xl border border-[#E5E5E5]/80 bg-[#F2F1F6]/70 px-3.5 py-2.5 text-xs text-[#4D4D4D] dark:border-white/8 dark:bg-white/[0.03] dark:text-white/65">
              {t("empty_state.tip_windows_pair")}
            </div>
          </div>
        </div>
      </section>
    );
  }

  return (
    <section
      className={`${QCY_CARD_CLASS} flex flex-col items-center justify-center gap-4 px-6 py-14 text-center`}
      aria-label={t("empty_state.device_required_title")}
    >
      <span className="flex h-12 w-12 items-center justify-center rounded-2xl bg-[#F2F1F6] text-[#808080] dark:bg-white/[0.06] dark:text-white/55">
        <Bluetooth className="h-6 w-6" />
      </span>
      <div className="max-w-md space-y-1.5">
        <h2 className="text-base font-bold text-[#1A1A1A] dark:text-white">
          {t("empty_state.device_required_title")}
        </h2>
        <p className="text-sm text-[#4D4D4D] dark:text-white/65">
          {t("empty_state.device_required_desc", {
            feature: featureLabel ?? t("app.nav.dashboard"),
          })}
        </p>
      </div>
      {onGoToDevices && (
        <button
          type="button"
          onClick={onGoToDevices}
          className="inline-flex items-center gap-2 rounded-xl bg-[#FF7A2F] px-5 py-2.5 text-sm font-semibold text-white shadow-xs transition hover:bg-[#E8691F]"
        >
          <Radio className="h-4 w-4" />
          {t("empty_state.go_to_devices")}
        </button>
      )}
    </section>
  );
};
