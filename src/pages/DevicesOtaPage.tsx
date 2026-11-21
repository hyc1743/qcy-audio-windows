import React, { useEffect, useMemo, useRef, useState } from "react";
import {
  AlertTriangle,
  Bluetooth,
  CheckCircle2,
  ChevronDown,
  ChevronUp,
  Cpu,
  ExternalLink,
  FolderOpen,
  Pencil,
  Radio,
  RefreshCw,
  RotateCcw,
  ShieldAlert,
  Terminal,
  Trash2,
  Upload,
  Wifi,
  XCircle,
} from "lucide-react";
import { useQcyStore } from "../store/useQcyStore";
import { useI18n } from "../i18n";
import type { DeviceModelProfile, DiscoveredDevice } from "../types/qcy";

const QCY_COMPANY_IDS = new Set(["1822", "21020", "21036", "0x071e", "0x521c", "0x522c"]);
const QCY_OUI_PREFIXES = ["84:AC:60", "1C:52:16", "C4:33:96"];

/** Returns true if a scanned/paired Bluetooth device matches known QCY identifiers or name hints. */
export function isLikelyQcyDevice(
  dev: DiscoveredDevice,
  models: readonly DeviceModelProfile[] = []
): boolean {
  if (dev.is_virtual) return true;
  if (dev.model_id_hint || (dev.numeric_model_id !== null && dev.numeric_model_id !== undefined)) {
    return true;
  }
  for (const key of Object.keys(dev.manufacturer_data ?? {})) {
    if (QCY_COMPANY_IDS.has(key.toLowerCase())) return true;
  }
  for (const uuid of dev.service_uuids ?? []) {
    const low = uuid.toLowerCase();
    if (low.includes("0000a001") || low.includes("0000a002")) return true;
  }
  const upperMac = (dev.address ?? "").toUpperCase();
  if (QCY_OUI_PREFIXES.some((oui) => upperMac.startsWith(oui))) return true;
  const lowerName = (dev.name ?? "").toLowerCase();
  if (lowerName.includes("qcy")) return true;
  for (const m of models) {
    if (m.ble_name_hints.some((hint) => hint.length >= 4 && lowerName.includes(hint.toLowerCase()))) {
      return true;
    }
  }
  return false;
}

const OTA_ROUTE_LABELS: Record<number, { name: string; transport: string }> = {
  0: { name: "Generic", transport: "N/A" },
  1: { name: "PixArt / AWOTA", transport: "Classic RFCOMM (SDP 0x1101)" },
  2: { name: "JieLi RCSP (Unauth)", transport: "BLE GATT (0xA002)" },
  5: { name: "Qualcomm GAIA", transport: "Classic RFCOMM (GAIA)" },
  6: { name: "WuQi BLE", transport: "BLE GATT (0x7033)" },
  7: { name: "Actions Semi", transport: "Classic RFCOMM (0xE003)" },
  8: { name: "BES", transport: "BLE GATT (6666/7777)" },
  9: { name: "JieLi RCSP v2 (Auth)", transport: "BLE GATT (0xAE00 · RcspAuth)" },
  10: { name: "Bluetrum FOTA", transport: "BLE GATT (0xFF12)" },
  11: { name: "WuQi BLE + Tone Pack", transport: "BLE GATT (0x7033)" },
};

/** Parses QCY 24B BLE Manufacturer Data (Company ID 0x071E / 0x521C / 0x522C) for live battery preview. */
function parseAdvBatteryPreview(
  dev: DiscoveredDevice
): { left: number | null; right: number | null; caseLevel: number | null } | null {
  const companyIds = ["1822", "21020", "21036", "0x071e", "0x521c", "0x522c"];
  for (const key of Object.keys(dev.manufacturer_data)) {
    if (companyIds.includes(key.toLowerCase())) {
      const bytes = dev.manufacturer_data[key];
      if (bytes && bytes.length >= 8) {
        const decode = (b: number) => {
          const pct = b & 0x7f;
          return pct <= 100 ? pct : null;
        };
        return {
          left: decode(bytes[5]),
          right: decode(bytes[6]),
          caseLevel: decode(bytes[7]),
        };
      }
    }
  }
  return null;
}

export const DevicesOtaPage: React.FC = () => {
  const {
    snapshot,
    models,
    scannedDevices,
    protocolLogs,
    isScanning,
    startScan,
    stopScan,
    refreshPairedDevices,
    connectDevice,
    disconnectDevice,
    removeSavedDevice,
    openWindowsBtSettings,
    renameDevice,
    resetDevice,
    startLocalOta,
    cancelOta,
    clearProtocolLogs,
  } = useQcyStore();
  const { t, locale } = useI18n();

  const [showAllBluetooth, setShowAllBluetooth] = useState(false);
  const [renameInput, setRenameInput] = useState("");
  const [renameOpen, setRenameOpen] = useState(false);
  const [pendingReset, setPendingReset] = useState<
    "soft_reset" | "clear_pairing" | "factory_reset" | null
  >(null);
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const [isReadingFile, setIsReadingFile] = useState(false);
  const [inspectorOpen, setInspectorOpen] = useState(false);
  const fileInputRef = useRef<HTMLInputElement | null>(null);

  const simulatorEnabled = Boolean(snapshot?.settings.simulator_enabled);

  // Auto-populate already-paired Windows Bluetooth devices on mount when in live mode
  useEffect(() => {
    if (!simulatorEnabled) {
      void refreshPairedDevices();
    }
  }, [simulatorEnabled, refreshPairedDevices]);

  const sortedFilteredDevices = useMemo(() => {
    const annotated = scannedDevices.map((dev) => ({
      dev,
      isQcy: isLikelyQcyDevice(dev, models),
    }));
    annotated.sort((a, b) => {
      if (a.isQcy !== b.isQcy) return a.isQcy ? -1 : 1;
      if (a.dev.is_paired !== b.dev.is_paired) return a.dev.is_paired ? -1 : 1;
      const rssiA = a.dev.rssi ?? -999;
      const rssiB = b.dev.rssi ?? -999;
      return rssiB - rssiA;
    });
    return showAllBluetooth ? annotated : annotated.filter((entry) => entry.isQcy);
  }, [scannedDevices, models, showAllBluetooth]);

  if (!snapshot) {
    return (
      <div className="rounded-2xl bg-white p-8 text-center text-sm text-neutral-500 dark:bg-[#1E1E1E] dark:text-neutral-400">
        {t("common.loading")}
      </div>
    );
  }

  const { adapter, active_device, active_model, battery, saved_devices, ota } = snapshot;
  const isDeviceConnected = Boolean(active_device && snapshot.connection_state === "connected");
  const otaRoute = OTA_ROUTE_LABELS[active_model.ota_type] ?? OTA_ROUTE_LABELS[0];
  const batteryOk =
    (battery.left === null || battery.left >= 30) &&
    (battery.right === null || battery.right >= 30);
  const twsOk = !active_model.capabilities.has_case_battery || battery.is_tws_connected;
  const isFlashing =
    ota.phase === "validating" ||
    ota.phase === "transferring" ||
    ota.phase === "verifying" ||
    ota.phase === "rebooting";

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const f = e.target.files?.[0];
    if (f) {
      setSelectedFile(f);
    }
  };

  const handleStartOta = async () => {
    if (!selectedFile || isReadingFile) return;
    setIsReadingFile(true);
    try {
      const buffer = await selectedFile.arrayBuffer();
      const bytes = Array.from(new Uint8Array(buffer));
      await startLocalOta(selectedFile.name, bytes);
    } finally {
      setIsReadingFile(false);
    }
  };

  const handleRenameSubmit = async () => {
    if (!renameInput.trim()) return;
    await renameDevice(renameInput.trim());
    setRenameOpen(false);
    setRenameInput("");
  };

  const confirmReset = async () => {
    if (!pendingReset) return;
    await resetDevice(pendingReset);
    setPendingReset(null);
  };

  return (
    <div className="space-y-5">
      {/* 1. Windows Bluetooth Adapter Status Card */}
      <section className="rounded-2xl border border-neutral-200/80 bg-white p-5 shadow-xs dark:border-white/10 dark:bg-[#1E1E1E]">
        <div className="flex flex-wrap items-center justify-between gap-4">
          <div className="flex items-center gap-3">
            <div
              className={`flex h-10 w-10 items-center justify-center rounded-xl ${
                adapter.is_present
                  ? "bg-[#0066FF]/12 text-[#0066FF] dark:bg-[#3399FF]/15 dark:text-[#3399FF]"
                  : "bg-rose-500/15 text-rose-500"
              }`}
            >
              <Bluetooth className="h-5 w-5" />
            </div>
            <div>
              <div className="flex items-center gap-2">
                <h2 className="text-sm font-semibold text-neutral-900 dark:text-white">
                  {locale === "zh-CN" ? "Windows 蓝牙适配器" : "Windows Bluetooth Adapter"}
                </h2>
                <span
                  className={`rounded-full px-2 py-0.5 text-[10px] font-semibold ${
                    adapter.is_present
                      ? "bg-emerald-500/15 text-emerald-600 dark:text-emerald-400"
                      : "bg-rose-500/15 text-rose-500"
                  }`}
                >
                  {adapter.is_present ? t("common.enabled") : t("common.disabled")}
                </span>
              </div>
              <p className="mt-0.5 font-mono text-xs text-neutral-500 dark:text-neutral-400">
                {adapter.is_present
                  ? `${adapter.address ?? "—"} · BLE ${adapter.is_low_energy_supported ? "✓" : "✕"} · Classic ${adapter.is_classic_supported ? "✓" : "✕"}`
                  : adapter.error ?? t("connection.bt_adapter_unavailable")}
              </p>
            </div>
          </div>

          <button
            type="button"
            onClick={() => void openWindowsBtSettings()}
            className="inline-flex items-center gap-1.5 rounded-xl bg-[#F2F1F6] px-3.5 py-2 text-xs font-semibold text-neutral-800 hover:bg-neutral-200 dark:bg-white/10 dark:text-white dark:hover:bg-white/15"
          >
            <ExternalLink className="h-3.5 w-3.5" />
            {t("connection.open_windows_bt_settings")}
          </button>
        </div>
      </section>

      {/* 2. Active BLE Scanner & Saved Devices */}
      <div className="grid gap-5 lg:grid-cols-2">
        {/* Scanner */}
        <section className="rounded-2xl border border-neutral-200/80 bg-white p-5 shadow-xs dark:border-white/10 dark:bg-[#1E1E1E]">
          <div className="mb-4 flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Radio
                className={`h-4 w-4 ${isScanning ? "animate-pulse text-[#FF7A2F]" : "text-[#0066FF] dark:text-[#3399FF]"}`}
              />
              <h2 className="text-sm font-semibold text-neutral-900 dark:text-white">
                {t("devices.scan_title")}
              </h2>
            </div>

            <button
              type="button"
              onClick={() => void (isScanning ? stopScan() : startScan())}
              className={`inline-flex items-center gap-1.5 rounded-xl px-3.5 py-1.5 text-xs font-semibold text-white transition ${
                isScanning ? "bg-neutral-600 hover:bg-neutral-700" : "bg-[#FF7A2F] hover:bg-[#e86920]"
              }`}
            >
              <RefreshCw className={`h-3.5 w-3.5 ${isScanning ? "animate-spin" : ""}`} />
              {isScanning ? t("devices.stop_scan") : t("devices.scan_again")}
            </button>
          </div>

          {/* Filter toggle: QCY matched devices first + optional Show All Nearby Bluetooth */}
          <div className="mb-3 flex flex-wrap items-center justify-between gap-2 rounded-xl bg-[#F2F1F6]/80 px-3 py-2 text-xs dark:bg-white/[0.04]">
            <label className="inline-flex cursor-pointer items-center gap-2 font-medium text-neutral-700 dark:text-neutral-300">
              <input
                type="checkbox"
                checked={showAllBluetooth}
                onChange={(e) => setShowAllBluetooth(e.target.checked)}
                className="h-3.5 w-3.5 accent-[#FF7A2F]"
              />
              <span>{t("devices.show_all_bluetooth")}</span>
            </label>
            <span className="font-mono text-[11px] text-neutral-500 dark:text-neutral-400">
              {sortedFilteredDevices.length}/{scannedDevices.length}
            </span>
          </div>

          {sortedFilteredDevices.length === 0 ? (
            <div className="space-y-2 rounded-xl bg-[#F2F1F6] px-4 py-8 text-center text-xs text-neutral-500 dark:bg-white/5 dark:text-neutral-400">
              <p>
                {isScanning
                  ? t("devices.scanning")
                  : !showAllBluetooth && scannedDevices.length > 0
                    ? t("devices.no_qcy_devices_filtered", { total: scannedDevices.length })
                    : t("devices.no_devices_found")}
              </p>
              {!showAllBluetooth && scannedDevices.length > 0 && (
                <button
                  type="button"
                  onClick={() => setShowAllBluetooth(true)}
                  className="inline-flex items-center rounded-lg bg-[#0066FF]/12 px-3 py-1 font-semibold text-[#0066FF] hover:bg-[#0066FF]/20 dark:bg-[#3399FF]/15 dark:text-[#3399FF]"
                >
                  {t("devices.show_all_bluetooth")}
                </button>
              )}
              {scannedDevices.length === 0 && !isScanning && (
                <p className="text-[11px] opacity-80">{t("devices.qcy_filter_hint")}</p>
              )}
            </div>
          ) : (
            <div className="space-y-2.5">
              {sortedFilteredDevices.map(({ dev, isQcy }) => {
                const advBat = parseAdvBatteryPreview(dev);
                const isCurrent =
                  active_device?.address === dev.address &&
                  snapshot.connection_state === "connected";
                return (
                  <div
                    key={dev.id}
                    className={`flex items-center justify-between gap-3 rounded-xl border p-3 ${
                      isQcy
                        ? "border-[#FF7A2F]/35 bg-[#FF7A2F]/[0.05] dark:border-[#FF7A2F]/35 dark:bg-[#FF7A2F]/[0.08]"
                        : "border-transparent bg-[#F2F1F6] dark:bg-white/5"
                    }`}
                  >
                    <div className="min-w-0">
                      <div className="flex flex-wrap items-center gap-1.5">
                        <span className="truncate text-sm font-semibold text-neutral-900 dark:text-white">
                          {dev.name || dev.address}
                        </span>
                        {isQcy && !dev.is_virtual && (
                          <span className="rounded-md bg-[#FF7A2F]/15 px-1.5 py-0.5 text-[10px] font-bold text-[#FF7A2F]">
                            {t("devices.qcy_match_badge")}
                          </span>
                        )}
                        {dev.is_paired && !dev.is_virtual && (
                          <span className="rounded-md bg-emerald-500/15 px-1.5 py-0.5 text-[10px] font-semibold text-emerald-600 dark:text-emerald-400">
                            {t("devices.paired_badge")}
                          </span>
                        )}
                        <span className="rounded-md bg-[#0066FF]/12 px-1.5 py-0.5 text-[10px] font-semibold uppercase text-[#0066FF] dark:text-[#3399FF]">
                          {dev.is_virtual ? "SIM" : dev.transport_kind}
                        </span>
                      </div>
                      <div className="mt-0.5 flex flex-wrap items-center gap-2 font-mono text-[11px] text-neutral-500 dark:text-neutral-400">
                        <span>{dev.address}</span>
                        {dev.rssi !== null && (
                          <span className="inline-flex items-center gap-0.5">
                            <Wifi className="h-3 w-3" />
                            {dev.rssi} dBm
                          </span>
                        )}
                        {advBat && (
                          <span className="text-emerald-600 dark:text-emerald-400">
                            L:{advBat.left ?? "—"}% R:{advBat.right ?? "—"}% C:
                            {advBat.caseLevel ?? "—"}%
                          </span>
                        )}
                      </div>
                    </div>

                    <button
                      type="button"
                      onClick={() =>
                        void (isCurrent
                          ? disconnectDevice()
                          : connectDevice(dev.address, dev.model_id_hint ?? undefined))
                      }
                      className={`shrink-0 rounded-xl px-3 py-1.5 text-xs font-semibold ${
                        isCurrent
                          ? "bg-neutral-200 text-neutral-700 dark:bg-white/10 dark:text-neutral-200"
                          : "bg-[#FF7A2F] text-white hover:bg-[#e86920]"
                      }`}
                    >
                      {isCurrent ? t("devices.disconnect") : t("devices.connect")}
                    </button>
                  </div>
                );
              })}
            </div>
          )}
        </section>

        {/* Saved Devices */}
        <section className="rounded-2xl border border-neutral-200/80 bg-white p-5 shadow-xs dark:border-white/10 dark:bg-[#1E1E1E]">
          <h2 className="mb-4 text-sm font-semibold text-neutral-900 dark:text-white">
            {t("devices.my_devices")} ({saved_devices.length})
          </h2>

          {saved_devices.length === 0 ? (
            <div className="rounded-xl bg-[#F2F1F6] px-4 py-8 text-center text-xs text-neutral-500 dark:bg-white/5 dark:text-neutral-400">
              {t("devices.empty_list_desc")}
            </div>
          ) : (
            <div className="space-y-2.5">
              {saved_devices.map((item) => {
                const isActive = active_device?.address === item.address;
                return (
                  <div
                    key={item.address}
                    className={`flex items-center justify-between gap-3 rounded-xl border p-3 ${
                      isActive
                        ? "border-[#FF7A2F]/50 bg-[#FF7A2F]/8"
                        : "border-transparent bg-[#F2F1F6] dark:bg-white/5"
                    }`}
                  >
                    <div className="min-w-0">
                      <p className="truncate text-sm font-semibold text-neutral-900 dark:text-white">
                        {item.name}
                      </p>
                      <p className="font-mono text-[11px] text-neutral-500 dark:text-neutral-400">
                        {item.address} · {item.model_id}
                      </p>
                    </div>
                    <div className="flex items-center gap-1.5">
                      <button
                        type="button"
                        onClick={() =>
                          void (isActive && snapshot.connection_state === "connected"
                            ? disconnectDevice()
                            : connectDevice(item.address, item.model_id))
                        }
                        className="rounded-lg bg-white px-2.5 py-1.5 text-xs font-semibold text-neutral-800 shadow-2xs hover:bg-neutral-100 dark:bg-white/10 dark:text-white"
                      >
                        {isActive && snapshot.connection_state === "connected"
                          ? t("devices.disconnect")
                          : t("devices.switch_device")}
                      </button>
                      <button
                        type="button"
                        aria-label={t("common.remove")}
                        onClick={() => void removeSavedDevice(item.address)}
                        className="rounded-lg p-1.5 text-neutral-400 hover:bg-rose-500/10 hover:text-rose-500"
                      >
                        <Trash2 className="h-4 w-4" />
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </section>
      </div>

      {/* 3 & 4. Device Maintenance + Local Firmware OTA Flasher — connected device only */}
      {isDeviceConnected && (
        <>
          <section className="rounded-2xl border border-neutral-200/80 bg-white p-5 shadow-xs dark:border-white/10 dark:bg-[#1E1E1E]">
        <h2 className="mb-4 text-sm font-semibold text-neutral-900 dark:text-white">
          {t("device_settings.section_maintenance")}
        </h2>

        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
          {active_model.capabilities.rename && (
            <button
              type="button"
              onClick={() => {
                setRenameInput(active_device?.name ?? active_model.name_en);
                setRenameOpen((v) => !v);
              }}
              className="flex items-center gap-2.5 rounded-xl bg-[#F2F1F6] px-3.5 py-3 text-left text-xs font-semibold text-neutral-800 hover:bg-neutral-200/80 dark:bg-white/5 dark:text-neutral-200 dark:hover:bg-white/10"
            >
              <Pencil className="h-4 w-4 text-[#0066FF] dark:text-[#3399FF]" />
              {t("device_settings.device_name")}
            </button>
          )}

          {active_model.capabilities.reset && (
            <button
              type="button"
              onClick={() => setPendingReset("soft_reset")}
              className="flex items-center gap-2.5 rounded-xl bg-[#F2F1F6] px-3.5 py-3 text-left text-xs font-semibold text-neutral-800 hover:bg-neutral-200/80 dark:bg-white/5 dark:text-neutral-200 dark:hover:bg-white/10"
            >
              <RotateCcw className="h-4 w-4 text-amber-500" />
              {t("device_settings.restore_defaults")}
            </button>
          )}

          {active_model.capabilities.clear_pairing && (
            <button
              type="button"
              onClick={() => setPendingReset("clear_pairing")}
              className="flex items-center gap-2.5 rounded-xl bg-[#F2F1F6] px-3.5 py-3 text-left text-xs font-semibold text-neutral-800 hover:bg-neutral-200/80 dark:bg-white/5 dark:text-neutral-200 dark:hover:bg-white/10"
            >
              <Bluetooth className="h-4 w-4 text-amber-500" />
              {t("device_settings.clear_pairing")}
            </button>
          )}

          {active_model.capabilities.factory_reset && (
            <button
              type="button"
              onClick={() => setPendingReset("factory_reset")}
              className="flex items-center gap-2.5 rounded-xl bg-rose-500/10 px-3.5 py-3 text-left text-xs font-semibold text-rose-600 hover:bg-rose-500/20 dark:text-rose-400"
            >
              <ShieldAlert className="h-4 w-4" />
              {t("device_settings.factory_reset")}
            </button>
          )}
        </div>

        {renameOpen && (
          <div className="mt-4 flex flex-wrap items-center gap-2 rounded-xl bg-[#F2F1F6] p-3 dark:bg-white/5">
            <input
              type="text"
              value={renameInput}
              onChange={(e) => setRenameInput(e.target.value)}
              placeholder={t("device_settings.device_name_placeholder")}
              className="flex-1 rounded-lg border border-neutral-300 bg-white px-3 py-1.5 text-sm text-neutral-900 outline-none focus:border-[#FF7A2F] dark:border-white/15 dark:bg-[#1E1E1E] dark:text-white"
            />
            <button
              type="button"
              onClick={() => void handleRenameSubmit()}
              className="rounded-lg bg-[#FF7A2F] px-3.5 py-1.5 text-xs font-semibold text-white"
            >
              {t("common.save")}
            </button>
            <button
              type="button"
              onClick={() => setRenameOpen(false)}
              className="rounded-lg bg-neutral-200 px-3 py-1.5 text-xs font-semibold text-neutral-700 dark:bg-white/10 dark:text-neutral-300"
            >
              {t("common.cancel")}
            </button>
            <p className="w-full text-[11px] text-neutral-500 dark:text-neutral-400">
              {t("device_settings.device_name_repair_notice")}
            </p>
          </div>
        )}

        {pendingReset && (
          <div className="mt-4 flex flex-wrap items-center justify-between gap-3 rounded-xl border border-rose-500/30 bg-rose-500/10 p-3.5">
            <p className="text-xs font-medium text-rose-700 dark:text-rose-300">
              {pendingReset === "soft_reset"
                ? t("device_settings.restore_defaults_confirm", {
                    name: active_device?.name ?? active_model.name_en,
                  })
                : pendingReset === "clear_pairing"
                  ? t("device_settings.clear_pairing_confirm", {
                      name: active_device?.name ?? active_model.name_en,
                    })
                  : t("device_settings.factory_reset_confirm", {
                      name: active_device?.name ?? active_model.name_en,
                    })}
            </p>
            <div className="flex gap-2">
              <button
                type="button"
                onClick={() => setPendingReset(null)}
                className="rounded-lg bg-white px-3 py-1.5 text-xs font-semibold text-neutral-700 dark:bg-white/10 dark:text-neutral-200"
              >
                {t("common.cancel")}
              </button>
              <button
                type="button"
                onClick={() => void confirmReset()}
                className="rounded-lg bg-rose-600 px-3 py-1.5 text-xs font-semibold text-white hover:bg-rose-700"
              >
                {t("common.confirm")}
              </button>
            </div>
          </div>
        )}
      </section>

      {/* 4. Local Firmware OTA Flasher */}
      <section className="rounded-2xl border border-neutral-200/80 bg-white p-5 shadow-xs dark:border-white/10 dark:bg-[#1E1E1E]">
        <div className="mb-3 flex flex-wrap items-center justify-between gap-3">
          <div className="flex items-center gap-2.5">
            <div className="flex h-8 w-8 items-center justify-center rounded-xl bg-[#FF7A2F]/15 text-[#FF7A2F]">
              <Cpu className="h-4 w-4" />
            </div>
            <div>
              <h2 className="text-sm font-semibold text-neutral-900 dark:text-white">
                {t("ota.title")}
              </h2>
              <p className="text-xs text-neutral-500 dark:text-neutral-400">{t("ota.subtitle")}</p>
            </div>
          </div>
          <span className="rounded-full bg-[#F2F1F6] px-3 py-1 font-mono text-xs font-semibold text-neutral-700 dark:bg-white/10 dark:text-neutral-200">
            {t("ota.current_version")}: {snapshot.firmware_version}
          </span>
        </div>

        <div className="mb-4 grid gap-3 rounded-xl bg-[#F2F1F6] p-3.5 text-xs dark:bg-white/5 sm:grid-cols-3">
          <div>
            <span className="text-neutral-500 dark:text-neutral-400">{t("ota.ota_route")}</span>
            <p className="mt-0.5 font-semibold text-neutral-900 dark:text-white">
              otaType={active_model.ota_type} · {otaRoute.name}
            </p>
            <p className="text-[11px] text-neutral-500">{otaRoute.transport}</p>
          </div>
          <div className="flex items-center gap-2">
            {batteryOk ? (
              <CheckCircle2 className="h-4 w-4 shrink-0 text-emerald-500" />
            ) : (
              <XCircle className="h-4 w-4 shrink-0 text-rose-500" />
            )}
            <span className="text-neutral-700 dark:text-neutral-300">
              {t("ota.low_battery_warning")}
            </span>
          </div>
          <div className="flex items-center gap-2">
            {twsOk ? (
              <CheckCircle2 className="h-4 w-4 shrink-0 text-emerald-500" />
            ) : (
              <XCircle className="h-4 w-4 shrink-0 text-rose-500" />
            )}
            <span className="text-neutral-700 dark:text-neutral-300">
              {t("ota.both_ears_warning")}
            </span>
          </div>
        </div>

        {/* Transparent otaType=9 Auth Notice */}
        {(active_model.ota_type === 9 || ota.phase === "unsupported_auth_required") && (
          <div className="mb-4 flex items-start gap-2.5 rounded-xl border border-amber-500/35 bg-amber-500/10 p-3.5 text-xs text-amber-800 dark:text-amber-300">
            <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
            <p>{t("ota.unsupported_auth_required")}</p>
          </div>
        )}

        <input
          ref={fileInputRef}
          type="file"
          accept=".ota,.bin,.ufw,.fot"
          onChange={handleFileChange}
          className="hidden"
        />

        <div className="flex flex-wrap items-center gap-3">
          <button
            type="button"
            disabled={isFlashing}
            onClick={() => fileInputRef.current?.click()}
            className="inline-flex items-center gap-2 rounded-xl bg-[#F2F1F6] px-4 py-2 text-xs font-semibold text-neutral-800 hover:bg-neutral-200 disabled:opacity-50 dark:bg-white/10 dark:text-white"
          >
            <FolderOpen className="h-4 w-4 text-[#0066FF] dark:text-[#3399FF]" />
            {t("ota.select_file")}
          </button>

          {selectedFile && (
            <span className="font-mono text-xs text-neutral-600 dark:text-neutral-300">
              {selectedFile.name} ({Math.round(selectedFile.size / 1024)} KB)
            </span>
          )}

          <button
            type="button"
            disabled={!selectedFile || !batteryOk || !twsOk || isFlashing || isReadingFile}
            onClick={() => void handleStartOta()}
            className="ml-auto inline-flex items-center gap-2 rounded-xl bg-[#FF7A2F] px-4 py-2 text-xs font-semibold text-white shadow-xs hover:bg-[#e86920] disabled:cursor-not-allowed disabled:opacity-50"
          >
            <Upload className="h-4 w-4" />
            {t("ota.start_flash")}
          </button>

          {isFlashing && (
            <button
              type="button"
              onClick={() => void cancelOta()}
              className="rounded-xl bg-rose-600 px-3.5 py-2 text-xs font-semibold text-white"
            >
              {t("common.cancel")}
            </button>
          )}
        </div>

        {ota.phase !== "idle" && ota.phase !== "unsupported_auth_required" && (
          <div className="mt-4 space-y-2">
            <div className="flex justify-between text-xs font-medium">
              <span className="text-neutral-700 dark:text-neutral-300">
                {ota.phase === "completed"
                  ? t("ota.flash_success")
                  : ota.phase === "error"
                    ? t("ota.flash_failed", { reason: ota.message ?? "unknown" })
                    : t("ota.flashing", { percent: ota.progress_pct })}
              </span>
              <span className="font-mono">{ota.progress_pct}%</span>
            </div>
            <div className="h-2 overflow-hidden rounded-full bg-[#F2F1F6] dark:bg-white/10">
              <div
                className="h-full rounded-full bg-gradient-to-r from-[#FF7A2F] to-[#0066FF] transition-all"
                style={{ width: `${ota.progress_pct}%` }}
              />
            </div>
          </div>
        )}

        <p className="mt-3 text-[11px] text-neutral-500 dark:text-neutral-400">
          {t("ota.local_offline_disclaimer")}
        </p>
      </section>
        </>
      )}

      {/* 5. Collapsible Live Protocol Frame Inspector */}
      <section className="rounded-2xl border border-neutral-200/80 bg-white shadow-xs dark:border-white/10 dark:bg-[#1E1E1E]">
        <button
          type="button"
          onClick={() => setInspectorOpen((v) => !v)}
          className="flex w-full items-center justify-between px-5 py-4 text-left"
        >
          <div className="flex items-center gap-2.5">
            <Terminal className="h-4 w-4 text-[#0066FF] dark:text-[#3399FF]" />
            <span className="text-sm font-semibold text-neutral-900 dark:text-white">
              {t("simulator.protocol_inspector")} ({protocolLogs.length})
            </span>
          </div>
          {inspectorOpen ? (
            <ChevronUp className="h-4 w-4 text-neutral-400" />
          ) : (
            <ChevronDown className="h-4 w-4 text-neutral-400" />
          )}
        </button>

        {inspectorOpen && (
          <div className="border-t border-neutral-200/70 px-5 pb-5 pt-3 dark:border-white/10">
            <div className="mb-2 flex justify-end">
              <button
                type="button"
                onClick={() => void clearProtocolLogs()}
                className="rounded-lg bg-[#F2F1F6] px-2.5 py-1 text-xs font-medium text-neutral-700 hover:bg-neutral-200 dark:bg-white/10 dark:text-neutral-300"
              >
                {t("simulator.clear_log")}
              </button>
            </div>
            <div className="max-h-56 overflow-y-auto rounded-xl bg-neutral-950 p-3 font-mono text-[11px] text-neutral-200">
              {protocolLogs.length === 0 ? (
                <p className="text-neutral-500">—</p>
              ) : (
                protocolLogs.map((entry, idx) => (
                  <div key={`${entry.timestamp_ms}-${idx}`} className="flex gap-2 py-0.5">
                    <span className="shrink-0 text-neutral-500">
                      {new Date(entry.timestamp_ms).toLocaleTimeString()}
                    </span>
                    <span
                      className={`shrink-0 font-bold ${
                        entry.direction === "tx" ? "text-[#FF7A2F]" : "text-emerald-400"
                      }`}
                    >
                      {entry.direction.toUpperCase()}
                    </span>
                    <span className="shrink-0 text-cyan-300">{entry.hex}</span>
                    <span className="truncate text-neutral-400">{entry.summary}</span>
                  </div>
                ))
              )}
            </div>
          </div>
        )}
      </section>
    </div>
  );
};

export default DevicesOtaPage;
