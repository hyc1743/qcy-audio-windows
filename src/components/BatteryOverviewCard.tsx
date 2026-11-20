import React, { useState } from "react";
import {
  Bluetooth,
  Check,
  Copy,
  Cpu,
  Edit3,
  Palette,
  Radio,
  Signal,
  WifiOff,
  X,
  Zap,
} from "lucide-react";
import {
  DEVICE_COLOR_PRESETS,
  DeviceFormFactor,
  DeviceIllustration,
  SpeakerRgbMode,
  resolveDeviceColorPreset,
} from "../assets/DeviceIllustrations";
import { QCY_CARD_CLASS, TranslateFn, UiTheme } from "./shared";

export type DeviceConnectionMode =
  | "ble_live"
  | "virtual_simulator"
  | "connecting"
  | "disconnected";

export interface BatteryOverviewCardProps {
  deviceName: string;
  modelName: string;
  modelId?: string;
  formFactor: DeviceFormFactor;
  hasCaseBattery?: boolean;
  connectionMode: DeviceConnectionMode;
  rssi?: number | null;
  macAddress?: string;
  firmwareVersion?: string;
  chipsetLabel?: string;
  /** Battery percentages `0..100`, or `null`/`undefined` when disconnected/absent. */
  leftBattery?: number | null;
  rightBattery?: number | null;
  caseBattery?: number | null;
  leftConnected?: boolean;
  rightConnected?: boolean;
  leftInCase?: boolean;
  rightInCase?: boolean;
  leftCharging?: boolean;
  rightCharging?: boolean;
  caseCharging?: boolean;
  /** Current colorway index (`0..6`) or hex override. */
  colorIndex?: number;
  deviceColorHex?: string;
  canChangeColor?: boolean;
  onColorChange?: (colorIndex: number, hex: string) => void;
  canRename?: boolean;
  onRename?: (nextName: string) => void;
  /** Optional SP75 speaker RGB state reflected directly in the hero illustration. */
  rgbEnabled?: boolean;
  rgbColor?: string;
  rgbMode?: SpeakerRgbMode;
  theme?: UiTheme;
  t: TranslateFn;
  className?: string;
}

export function getBatteryAccentColor(level: number | null | undefined): string {
  if (level === null || level === undefined) return "#9CA3AF";
  if (level <= 15) return "#FF4C4C";
  if (level <= 30) return "#EAB308";
  return "#22C55E";
}

interface BatteryRingProps {
  label: string;
  level: number | null | undefined;
  charging?: boolean;
  dimmed?: boolean;
  lowLabel: string;
  chargingLabel: string;
}

const BatteryRing: React.FC<BatteryRingProps> = ({
  label,
  level,
  charging = false,
  dimmed = false,
  lowLabel,
  chargingLabel,
}) => {
  const hasLevel = typeof level === "number" && Number.isFinite(level);
  const clamped = hasLevel ? Math.max(0, Math.min(100, Math.round(level))) : 0;
  const strokeColor = getBatteryAccentColor(hasLevel ? clamped : null);
  const radius = 24;
  const circumference = 2 * Math.PI * radius;
  const dashOffset = hasLevel
    ? circumference - (clamped / 100) * circumference
    : circumference;
  const isLow = hasLevel && clamped <= 20 && !charging;

  return (
    <div
      className={`flex flex-col items-center gap-1.5 rounded-xl border border-[#E5E5E5]/80 bg-[#F2F1F6]/80 px-3 py-2.5 transition-opacity dark:border-white/8 dark:bg-white/[0.04] ${
        dimmed ? "opacity-50" : "opacity-100"
      }`}
    >
      <div className="relative flex h-14 w-14 items-center justify-center">
        <svg className="h-14 w-14 -rotate-90" viewBox="0 0 60 60" aria-hidden="true">
          <circle
            cx="30"
            cy="30"
            r={radius}
            fill="none"
            stroke="currentColor"
            strokeWidth="5"
            className="text-black/10 dark:text-white/12"
          />
          <circle
            cx="30"
            cy="30"
            r={radius}
            fill="none"
            stroke={strokeColor}
            strokeWidth="5"
            strokeLinecap="round"
            strokeDasharray={circumference}
            strokeDashoffset={dashOffset}
            style={{ transition: "stroke-dashoffset 300ms ease, stroke 200ms ease" }}
          />
        </svg>
        <div className="absolute inset-0 flex flex-col items-center justify-center">
          <span className="text-xs font-bold tabular-nums text-[#1A1A1A] dark:text-white">
            {hasLevel ? `${clamped}%` : "--"}
          </span>
          {charging && (
            <Zap
              className="h-3 w-3 text-[#22C55E]"
              fill="#22C55E"
              aria-label={chargingLabel}
            />
          )}
        </div>
      </div>

      <span className="text-xs font-medium text-[#4D4D4D] dark:text-white/75">{label}</span>

      {charging ? (
        <span className="inline-flex items-center gap-0.5 rounded-full bg-[#22C55E]/15 px-2 py-0.5 text-[10px] font-semibold text-[#16A34A] dark:text-[#4ADE80]">
          {chargingLabel}
        </span>
      ) : isLow ? (
        <span className="inline-flex items-center rounded-full bg-[#FF4C4C]/15 px-2 py-0.5 text-[10px] font-semibold text-[#DC2626] dark:text-[#FF6B6B]">
          {lowLabel}
        </span>
      ) : null}
    </div>
  );
};

export const BatteryOverviewCard: React.FC<BatteryOverviewCardProps> = ({
  deviceName,
  modelName,
  modelId,
  formFactor,
  hasCaseBattery = formFactor === "tws" || formFactor === "open_ear",
  connectionMode,
  rssi,
  macAddress,
  firmwareVersion,
  chipsetLabel,
  leftBattery,
  rightBattery,
  caseBattery,
  leftConnected = connectionMode !== "disconnected",
  rightConnected = connectionMode !== "disconnected",
  leftInCase = false,
  rightInCase = false,
  leftCharging = false,
  rightCharging = false,
  caseCharging = false,
  colorIndex = 0,
  deviceColorHex,
  canChangeColor = false,
  onColorChange,
  canRename = false,
  onRename,
  rgbEnabled = false,
  rgbColor = "#FF7A2F",
  rgbMode = "solid",
  theme = "dark",
  t,
  className = "",
}) => {
  const [editingName, setEditingName] = useState(false);
  const [draftName, setDraftName] = useState(deviceName);
  const [copiedMac, setCopiedMac] = useState(false);
  const [showColorRow, setShowColorRow] = useState(false);

  const activePreset = resolveDeviceColorPreset(colorIndex);
  const resolvedHex = deviceColorHex ?? activePreset.hex;
  const isSingleUnit = !hasCaseBattery;
  const primaryBattery = leftBattery ?? rightBattery ?? caseBattery ?? null;

  const handleSaveRename = () => {
    const trimmed = draftName.trim();
    if (trimmed && trimmed !== deviceName && onRename) {
      onRename(trimmed);
    }
    setEditingName(false);
  };

  const handleCopyMac = () => {
    if (!macAddress) return;
    if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
      void navigator.clipboard.writeText(macAddress).catch(() => {});
    }
    setCopiedMac(true);
    setTimeout(() => setCopiedMac(false), 1600);
  };

  const statusConfig = (() => {
    switch (connectionMode) {
      case "ble_live":
        return {
          label: `${t("connection.connected")} · ${t("connection.transport_ble")}`,
          dotClass: "bg-[#22C55E]",
          pillClass:
            "bg-[#22C55E]/12 text-[#15803D] border-[#22C55E]/30 dark:text-[#4ADE80]",
          Icon: Bluetooth,
        };
      case "virtual_simulator":
        return {
          label: `${t("simulator.badge")} · ${t("connection.transport_virtual")}`,
          dotClass: "bg-[#FF7A2F]",
          pillClass:
            "bg-[#FF7A2F]/14 text-[#C2410C] border-[#FF7A2F]/35 dark:text-[#FF9E66]",
          Icon: Radio,
        };
      case "connecting":
        return {
          label: t("connection.connecting"),
          dotClass: "bg-[#EAB308] animate-pulse",
          pillClass:
            "bg-[#EAB308]/14 text-[#A16207] border-[#EAB308]/35 dark:text-[#FACC15]",
          Icon: Bluetooth,
        };
      case "disconnected":
        return {
          label: t("connection.disconnected"),
          dotClass: "bg-[#9CA3AF]",
          pillClass:
            "bg-black/5 text-[#4D4D4D] border-[#E5E5E5] dark:bg-white/8 dark:text-white/60 dark:border-white/10",
          Icon: WifiOff,
        };
    }
  })();

  const StatusIcon = statusConfig.Icon;

  return (
    <section
      className={`${QCY_CARD_CLASS} relative overflow-hidden ${className}`}
      aria-label={t("dashboard.battery_status")}
    >
      {/* Subtle QCY orange ambient glow */}
      <div
        className="pointer-events-none absolute -top-24 -right-24 h-56 w-56 rounded-full opacity-15 blur-3xl dark:opacity-20"
        style={{ background: "radial-gradient(circle, #FF7A2F 0%, transparent 70%)" }}
      />

      <div className="relative flex flex-col gap-5 lg:flex-row lg:items-center lg:justify-between">
        {/* Left: Hero Device Illustration + Identity */}
        <div className="flex flex-col items-center gap-4 sm:flex-row sm:items-center">
          <div className="relative flex shrink-0 items-center justify-center rounded-2xl bg-[#F2F1F6] p-3 dark:bg-white/[0.04]">
            <DeviceIllustration
              formFactor={formFactor}
              deviceColor={resolvedHex}
              theme={theme}
              leftConnected={leftConnected}
              rightConnected={rightConnected}
              leftInCase={leftInCase}
              rightInCase={rightInCase}
              leftCharging={leftCharging}
              rightCharging={rightCharging}
              caseCharging={caseCharging}
              showCase={hasCaseBattery}
              rgbEnabled={rgbEnabled}
              rgbColor={rgbColor}
              rgbMode={rgbMode}
              width={204}
              height={136}
            />
          </div>

          <div className="flex flex-col items-center gap-2 text-center sm:items-start sm:text-left">
            {/* Connection status & RSSI pills */}
            <div className="flex flex-wrap items-center justify-center gap-2 sm:justify-start">
              <span
                className={`inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-xs font-semibold ${statusConfig.pillClass}`}
              >
                <span className={`h-1.5 w-1.5 rounded-full ${statusConfig.dotClass}`} />
                <StatusIcon className="h-3.5 w-3.5" />
                {statusConfig.label}
              </span>

              {typeof rssi === "number" && connectionMode !== "disconnected" && (
                <span className="inline-flex items-center gap-1 rounded-full border border-[#E5E5E5] bg-[#F2F1F6] px-2 py-0.5 text-[11px] font-medium text-[#4D4D4D] dark:border-white/10 dark:bg-white/[0.06] dark:text-white/70">
                  <Signal className="h-3 w-3 text-[#0066FF] dark:text-[#3399FF]" />
                  {t("connection.rssi_signal", { rssi })}
                </span>
              )}
            </div>

            {/* Device Name + Inline Rename */}
            {editingName ? (
              <div className="flex flex-col gap-1">
                <div className="flex items-center gap-1.5">
                  <input
                    type="text"
                    value={draftName}
                    onChange={(e) => setDraftName(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") handleSaveRename();
                      if (e.key === "Escape") {
                        setDraftName(deviceName);
                        setEditingName(false);
                      }
                    }}
                    placeholder={t("device_settings.device_name_placeholder")}
                    maxLength={32}
                    autoFocus
                    className="w-52 rounded-lg border border-[#FF7A2F] bg-white px-2.5 py-1 text-sm font-semibold text-[#1A1A1A] outline-none dark:bg-[#121212] dark:text-white"
                  />
                  <button
                    type="button"
                    onClick={handleSaveRename}
                    className="inline-flex h-8 w-8 items-center justify-center rounded-lg bg-[#FF7A2F] text-white hover:bg-[#E8691F]"
                    aria-label={t("common.save")}
                  >
                    <Check className="h-4 w-4" />
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      setDraftName(deviceName);
                      setEditingName(false);
                    }}
                    className="inline-flex h-8 w-8 items-center justify-center rounded-lg border border-[#E5E5E5] text-[#4D4D4D] hover:bg-black/5 dark:border-white/12 dark:text-white/70 dark:hover:bg-white/10"
                    aria-label={t("common.cancel")}
                  >
                    <X className="h-4 w-4" />
                  </button>
                </div>
                <p className="max-w-xs text-[11px] text-[#808080] dark:text-white/50">
                  {t("device_settings.device_name_repair_notice")}
                </p>
              </div>
            ) : (
              <div className="flex items-center gap-2">
                <h2 className="text-xl font-bold tracking-tight text-[#1A1A1A] dark:text-white">
                  {deviceName}
                </h2>
                {canRename && onRename && (
                  <button
                    type="button"
                    onClick={() => {
                      setDraftName(deviceName);
                      setEditingName(true);
                    }}
                    className="inline-flex h-7 w-7 items-center justify-center rounded-lg text-[#808080] transition hover:bg-black/5 hover:text-[#FF7A2F] dark:text-white/50 dark:hover:bg-white/10 dark:hover:text-[#FF7A2F]"
                    title={t("device_settings.device_name")}
                    aria-label={t("device_settings.device_name")}
                  >
                    <Edit3 className="h-3.5 w-3.5" />
                  </button>
                )}
              </div>
            )}

            <p className="text-xs font-medium text-[#808080] dark:text-white/55">
              {modelName}
              {modelId ? ` · ${modelId}` : ""}
            </p>

            {/* Hardware Metadata Badges: MAC, Firmware, Chipset, Color Switcher */}
            <div className="mt-0.5 flex flex-wrap items-center justify-center gap-1.5 sm:justify-start">
              {firmwareVersion && (
                <span className="inline-flex items-center gap-1 rounded-md bg-[#0066FF]/10 px-2 py-0.5 text-[11px] font-medium text-[#0066FF] dark:bg-[#3399FF]/15 dark:text-[#66B2FF]">
                  FW {firmwareVersion}
                </span>
              )}

              {chipsetLabel && (
                <span className="inline-flex items-center gap-1 rounded-md bg-black/5 px-2 py-0.5 text-[11px] font-medium text-[#4D4D4D] dark:bg-white/8 dark:text-white/70">
                  <Cpu className="h-3 w-3" />
                  {chipsetLabel}
                </span>
              )}

              {macAddress && (
                <button
                  type="button"
                  onClick={handleCopyMac}
                  className="inline-flex items-center gap-1 rounded-md bg-black/5 px-2 py-0.5 font-mono text-[11px] text-[#4D4D4D] transition hover:bg-black/10 dark:bg-white/8 dark:text-white/70 dark:hover:bg-white/15"
                  title={t("common.copy")}
                >
                  {macAddress}
                  {copiedMac ? (
                    <Check className="h-3 w-3 text-[#22C55E]" />
                  ) : (
                    <Copy className="h-3 w-3 opacity-60" />
                  )}
                </button>
              )}

              {canChangeColor && onColorChange && (
                <button
                  type="button"
                  onClick={() => setShowColorRow((v) => !v)}
                  className={`inline-flex items-center gap-1 rounded-md border px-2 py-0.5 text-[11px] font-medium transition ${
                    showColorRow
                      ? "border-[#FF7A2F] bg-[#FF7A2F]/12 text-[#FF7A2F]"
                      : "border-[#E5E5E5] bg-transparent text-[#4D4D4D] hover:border-[#FF7A2F]/50 dark:border-white/12 dark:text-white/70"
                  }`}
                >
                  <Palette className="h-3 w-3" />
                  {t("device_settings.device_color")}
                </button>
              )}
            </div>
          </div>
        </div>

        {/* Right: Battery Rings */}
        <div
          className={`grid gap-2.5 ${
            isSingleUnit ? "grid-cols-1 sm:min-w-[112px]" : "grid-cols-3"
          }`}
        >
          {isSingleUnit ? (
            <BatteryRing
              label={t("dashboard.battery_status")}
              level={primaryBattery}
              charging={leftCharging || rightCharging || caseCharging}
              dimmed={connectionMode === "disconnected"}
              lowLabel={t("dashboard.low_battery")}
              chargingLabel={t("dashboard.charging")}
            />
          ) : (
            <>
              <BatteryRing
                label={t("dashboard.left_earbud")}
                level={leftBattery}
                charging={leftCharging}
                dimmed={!leftConnected || leftInCase}
                lowLabel={t("dashboard.low_battery")}
                chargingLabel={t("dashboard.charging")}
              />
              <BatteryRing
                label={t("dashboard.right_earbud")}
                level={rightBattery}
                charging={rightCharging}
                dimmed={!rightConnected || rightInCase}
                lowLabel={t("dashboard.low_battery")}
                chargingLabel={t("dashboard.charging")}
              />
              <BatteryRing
                label={t("dashboard.charging_case")}
                level={caseBattery}
                charging={caseCharging}
                dimmed={caseBattery === null || caseBattery === undefined}
                lowLabel={t("dashboard.low_battery")}
                chargingLabel={t("dashboard.charging")}
              />
            </>
          )}
        </div>
      </div>

      {/* Expandable Dynamic Device Colorway Swatches */}
      {canChangeColor && onColorChange && showColorRow && (
        <div className="mt-4 flex flex-wrap items-center gap-2 border-t border-[#E5E5E5] pt-3.5 dark:border-white/10">
          <span className="mr-1 text-xs font-medium text-[#808080] dark:text-white/55">
            {t("device_settings.device_color")}:
          </span>
          {DEVICE_COLOR_PRESETS.map((preset) => {
            const selected = preset.index === colorIndex;
            return (
              <button
                key={preset.id}
                type="button"
                onClick={() => onColorChange(preset.index, preset.hex)}
                className={`inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-xs font-medium transition ${
                  selected
                    ? "border-[#FF7A2F] bg-[#FF7A2F]/12 text-[#1A1A1A] dark:text-white"
                    : "border-[#E5E5E5] bg-[#F2F1F6] text-[#4D4D4D] hover:border-[#FF7A2F]/50 dark:border-white/10 dark:bg-white/[0.04] dark:text-white/75"
                }`}
                aria-pressed={selected}
              >
                <span
                  className="h-3.5 w-3.5 rounded-full border border-black/20 dark:border-white/25"
                  style={{ backgroundColor: preset.hex }}
                />
                <span>{preset.nameEn}</span>
              </button>
            );
          })}
        </div>
      )}
    </section>
  );
};
