import React, { useCallback, useEffect, useMemo, useState } from "react";
import {
  AlertCircle,
  Battery,
  Bluetooth,
  CloudRain,
  Cpu,
  Fingerprint,
  Globe,
  Headphones,
  LayoutDashboard,
  MapPin,
  Moon,
  Settings,
  Sliders,
  Sparkles,
  Sun,
  X,
} from "lucide-react";
import {
  MutualExclusionDialog,
  type EvaluatedConflict,
  type PopViewRule as DialogPopViewRule,
} from "./components";
import {
  I18nProvider,
  useI18n,
  type SupportedLocale,
} from "./i18n";
import { isTauriRuntime, type QcyIpcClient } from "./ipc/client";
import { ControlsPage } from "./pages/ControlsPage";
import { DashboardPage } from "./pages/DashboardPage";
import { DevicesOtaPage } from "./pages/DevicesOtaPage";
import { FindEarbudsPage } from "./pages/FindEarbudsPage";
import { ModesLabPage } from "./pages/ModesLabPage";
import { SettingsPage } from "./pages/SettingsPage";
import { SimulatorDrawer } from "./pages/SimulatorDrawer";
import { SoundEqPage } from "./pages/SoundEqPage";
import { SoundscapesPage } from "./pages/SoundscapesPage";
import { ConnectionPopup } from "./popup/ConnectionPopup";
import { QcyStoreProvider, useQcyStore } from "./store/useQcyStore";
import type {
  AppTheme,
  MutualExclusionPrompt,
  PopupTriggerPayload,
} from "./types/qcy";

export type WindowMode = "main" | "popup";

export type NavTabId =
  | "dashboard"
  | "eq"
  | "gestures"
  | "modes"
  | "focus"
  | "find"
  | "devices"
  | "app_settings";

export interface AppProps {
  /** Optional explicit override (useful for tests & preview harnesses). */
  windowMode?: WindowMode;
  /** Optional initial locale override (defaults to store/system locale). */
  initialLocale?: SupportedLocale;
  /** Optional initial navigation tab for main shell. */
  initialTab?: NavTabId;
  /** Optional custom IPC client injection (useful for tests). */
  client?: QcyIpcClient;
}

/**
 * Pure detector checking URL query (`?window=popup`) or hash (`#popup` / `#/popup`).
 */
export function detectWindowModeFromLocation(
  search: string = typeof window !== "undefined" ? window.location.search : "",
  hash: string = typeof window !== "undefined" ? window.location.hash : ""
): WindowMode {
  try {
    const params = new URLSearchParams(search);
    if (params.get("window")?.toLowerCase() === "popup") {
      return "popup";
    }
  } catch {
    // ignore malformed query string
  }
  const cleanHash = hash.trim().toLowerCase();
  if (cleanHash === "#popup" || cleanHash === "#/popup" || cleanHash.startsWith("#popup?")) {
    return "popup";
  }
  return "main";
}

export function resolveTheme(theme: AppTheme | undefined): "light" | "dark" {
  if (theme === "light" || theme === "dark") return theme;
  if (typeof window !== "undefined" && typeof window.matchMedia === "function") {
    return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }
  return "dark";
}

function toDialogConflict(prompt: MutualExclusionPrompt): EvaluatedConflict {
  const mappedConflicts = prompt.rule.conflicts_with.map((c) => ({
    feature: c.feature,
    cmd_id: c.cmd_id,
    compare: (c.compare === "neq" ? "ne" : c.compare) as "eq" | "ne" | "gt" | "lt",
    value: c.value,
    reboot: c.reboot,
  }));

  const dialogRule: DialogPopViewRule = {
    target_feature: prompt.rule.target_feature,
    target_cmd_id: prompt.rule.target_cmd_id,
    target_value: prompt.rule.target_value,
    action: prompt.rule.action === "toast_blocked" ? "toast" : "confirm_dialog",
    requires_reboot: prompt.requires_reboot,
    title_key: prompt.title_key || null,
    message_key: prompt.message_key,
    reboot_notice_key: prompt.reboot_notice_key ?? null,
    conflicts_with: mappedConflicts,
  };

  const triggered = mappedConflicts.filter((c) =>
    prompt.conflicting_features.includes(c.feature)
  );

  return {
    rule: dialogRule,
    triggeredConflicts:
      triggered.length > 0
        ? triggered
        : prompt.conflicting_features.map((feat) => ({
            feature: feat,
            cmd_id: null,
            compare: "eq",
            value: 1,
            reboot: prompt.requires_reboot,
          })),
    requiresReboot: prompt.requires_reboot,
  };
}

function normalizeNavTab(tab: string): NavTabId {
  switch (tab) {
    case "dashboard":
    case "eq":
    case "gestures":
    case "modes":
    case "focus":
    case "find":
    case "devices":
    case "app_settings":
      return tab;
    case "settings":
      return "modes";
    default:
      return "dashboard";
  }
}

interface ShellInnerProps {
  windowMode: WindowMode;
  initialTab: NavTabId;
}

const QcyShellInner: React.FC<ShellInnerProps> = ({ windowMode, initialTab }) => {
  const {
    snapshot,
    popupPayload,
    pendingConflict,
    error,
    clearError,
    dismissPopup,
    setAncState,
    updateAppSettings,
  } = useQcyStore();
  const { locale, setLocale, t } = useI18n();

  const [activeTab, setActiveTab] = useState<NavTabId>(initialTab);
  const [simDrawerOpen, setSimDrawerOpen] = useState<boolean>(false);
  const [, setSystemThemeTick] = useState<number>(0);

  const resolvedTheme = resolveTheme(snapshot?.settings.theme);

  // Re-evaluate immediately when Windows system Light/Dark preference changes
  useEffect(() => {
    if (
      snapshot?.settings.theme !== "system" ||
      typeof window === "undefined" ||
      typeof window.matchMedia !== "function"
    ) {
      return;
    }
    const mql = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => setSystemThemeTick((n) => n + 1);
    mql.addEventListener?.("change", onChange);
    return () => mql.removeEventListener?.("change", onChange);
  }, [snapshot?.settings.theme]);

  // Sync locale with store settings
  useEffect(() => {
    const storeLocale = snapshot?.settings.locale;
    if (storeLocale && storeLocale !== locale) {
      setLocale(storeLocale);
    }
  }, [snapshot?.settings.locale, locale, setLocale]);

  // Sync `.dark` class, `data-theme`, `lang`, and popup transparency class on <html>
  useEffect(() => {
    if (typeof document === "undefined") return;
    const root = document.documentElement;
    root.classList.toggle("dark", resolvedTheme === "dark");
    root.setAttribute("data-theme", resolvedTheme);
    root.setAttribute("lang", locale);
    root.classList.toggle("qcy-popup-window", windowMode === "popup");
  }, [resolvedTheme, locale, windowMode]);

  const handleOpenMainFromPopup = useCallback(async () => {
    if (isTauriRuntime()) {
      try {
        const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
        const mainWin = await WebviewWindow.getByLabel("main");
        if (mainWin) {
          await mainWin.show();
          await mainWin.unminimize();
          await mainWin.setFocus();
        }
      } catch {
        // fallback no-op outside multi-window Tauri host
      }
    }
    dismissPopup();
  }, [dismissPopup]);

  const handleNavigate = useCallback((tab: string) => {
    setActiveTab(normalizeNavTab(tab));
  }, []);

  // Build fallback popup payload for dedicated popup window before first event arrives
  const effectivePopupPayload: PopupTriggerPayload | null = useMemo(() => {
    if (popupPayload) return popupPayload;
    if (windowMode !== "popup") return null;
    if (!snapshot) {
      return {
        device_name: "QCY MeloBuds N70",
        model_id: "qcy-n70",
        form_factor: "tws",
        color_index: 1,
        battery: {
          left: 88,
          right: 84,
          case_level: 65,
          left_charging: false,
          right_charging: false,
          case_charging: false,
          left_in_case: false,
          right_in_case: false,
          is_tws_connected: true,
        },
        anc_mode: "anc",
        is_virtual: true,
        auto_dismiss_ms: 5000,
      };
    }
    return {
      device_name:
        snapshot.active_device?.name ||
        (locale === "zh-CN" ? snapshot.active_model.name_zh : snapshot.active_model.name_en),
      model_id: snapshot.active_model.model_id,
      form_factor: snapshot.active_model.form_factor,
      color_index: snapshot.device_color_index,
      battery: snapshot.battery,
      anc_mode: snapshot.anc.mode,
      is_virtual: snapshot.is_virtual_session,
      auto_dismiss_ms: (snapshot.settings.popup_duration_sec || 5) * 1000,
    };
  }, [popupPayload, windowMode, snapshot, locale]);

  // DEDICATED FRAMELESS POP-UP WINDOW MODE
  if (windowMode === "popup") {
    return (
      <div
        className="flex h-screen w-screen items-center justify-center bg-transparent p-2"
        data-window-mode="popup"
      >
        {effectivePopupPayload && (
          <ConnectionPopup
            payload={effectivePopupPayload}
            theme={resolvedTheme}
            onOpenApp={() => void handleOpenMainFromPopup()}
            onDismiss={dismissPopup}
            onQuickToggleAnc={(mode) => void setAncState({ mode })}
            autoDismiss={true}
          />
        )}
      </div>
    );
  }

  // MAIN DESKTOP SHELL MODE
  const navItems: Array<{
    id: NavTabId;
    label: string;
    Icon: React.ComponentType<{ className?: string }>;
  }> = [
    { id: "dashboard", label: t("app.nav.dashboard"), Icon: LayoutDashboard },
    { id: "eq", label: t("app.nav.eq"), Icon: Sliders },
    { id: "gestures", label: t("app.nav.gestures"), Icon: Fingerprint },
    {
      id: "modes",
      label: locale === "zh-CN" ? "模式与实验室" : "Modes & Lab",
      Icon: Sparkles,
    },
    { id: "focus", label: t("app.nav.focus"), Icon: CloudRain },
    { id: "find", label: t("app.nav.find"), Icon: MapPin },
    {
      id: "devices",
      label: `${t("app.nav.devices")} · ${t("app.nav.ota")}`,
      Icon: Bluetooth,
    },
    { id: "app_settings", label: t("app.nav.app_settings"), Icon: Settings },
  ];

  const activeDeviceLabel = snapshot
    ? snapshot.active_device?.name ||
      (locale === "zh-CN" ? snapshot.active_model.name_zh : snapshot.active_model.name_en)
    : t("connection.not_connected");

  const batterySummary = snapshot
    ? snapshot.active_model.capabilities.has_case_battery
      ? `L ${snapshot.battery.left ?? "--"}% · R ${snapshot.battery.right ?? "--"}%`
      : `${snapshot.battery.left ?? snapshot.battery.right ?? "--"}%`
    : "--";

  const simulatorEnabled = Boolean(
    snapshot?.simulator.enabled || snapshot?.settings.simulator_enabled
  );

  const toggleLanguage = () => {
    const nextLocale: SupportedLocale = locale === "zh-CN" ? "en-US" : "zh-CN";
    setLocale(nextLocale);
    void updateAppSettings({ locale: nextLocale });
  };

  const toggleTheme = () => {
    const nextTheme: AppTheme = resolvedTheme === "dark" ? "light" : "dark";
    void updateAppSettings({ theme: nextTheme });
  };

  return (
    <div
      className="flex h-screen w-screen overflow-hidden bg-[#F2F1F6] text-[#1A1A1A] dark:bg-[#121212] dark:text-white"
      data-window-mode="main"
    >
      {/* Left Fluent Sidebar Navigation */}
      <aside className="flex w-64 shrink-0 flex-col border-r border-[#E5E5E5] bg-white/95 backdrop-blur-md dark:border-white/10 dark:bg-[#18181B]">
        {/* Brand Header */}
        <div className="flex items-center gap-3 px-5 pt-5 pb-4">
          <div className="flex h-10 w-10 items-center justify-center rounded-2xl bg-gradient-to-br from-[#FF7A2F] to-[#FF5500] text-white shadow-sm">
            <Headphones className="h-5 w-5" />
          </div>
          <div className="min-w-0">
            <h1 className="truncate text-base font-extrabold tracking-tight text-[#1A1A1A] dark:text-white">
              {t("app.name")}
            </h1>
            <p className="truncate text-[11px] font-medium text-[#808080] dark:text-white/50">
              {t("app.desktop_subtitle")}
            </p>
          </div>
        </div>

        {/* Active Device Mini-Pill */}
        <div className="px-4 pb-3">
          <button
            type="button"
            onClick={() => setActiveTab("devices")}
            className="w-full rounded-xl border border-[#E5E5E5] bg-[#F2F1F6]/85 p-2.5 text-left transition hover:border-[#FF7A2F]/60 dark:border-white/10 dark:bg-white/[0.04]"
          >
            <div className="flex items-center justify-between gap-2">
              <span className="inline-flex items-center gap-1.5 text-[10px] font-bold uppercase tracking-wide">
                <span
                  className={`h-2 w-2 rounded-full ${
                    snapshot?.connection_state === "connected"
                      ? snapshot.is_virtual_session
                        ? "bg-[#FF7A2F]"
                        : "bg-[#22C55E]"
                      : "bg-[#9CA3AF]"
                  }`}
                />
                <span
                  className={
                    snapshot?.is_virtual_session
                      ? "text-[#FF7A2F]"
                      : snapshot?.connection_state === "connected"
                        ? "text-[#16A34A] dark:text-[#4ADE80]"
                        : "text-[#808080]"
                  }
                >
                  {snapshot?.is_virtual_session
                    ? t("simulator.badge")
                    : snapshot?.connection_state === "connected"
                      ? t("connection.transport_ble")
                      : t("connection.disconnected")}
                </span>
              </span>
              <span className="inline-flex items-center gap-1 font-mono text-[10px] font-semibold text-[#4D4D4D] dark:text-white/70">
                <Battery className="h-3 w-3 text-[#22C55E]" />
                {batterySummary}
              </span>
            </div>
            <p className="mt-1 truncate text-xs font-bold text-[#1A1A1A] dark:text-white">
              {activeDeviceLabel}
            </p>
          </button>
        </div>

        {/* Primary Nav List */}
        <nav className="flex-1 space-y-1 overflow-y-auto px-3 py-1" aria-label="Main Navigation">
          {navItems.map(({ id, label, Icon }) => {
            const active = activeTab === id;
            return (
              <button
                key={id}
                type="button"
                onClick={() => setActiveTab(id)}
                aria-current={active ? "page" : undefined}
                className={`flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left text-sm font-semibold transition ${
                  active
                    ? "bg-[#FF7A2F] text-white shadow-xs"
                    : "text-[#4D4D4D] hover:bg-black/5 hover:text-[#1A1A1A] dark:text-white/75 dark:hover:bg-white/8 dark:hover:text-white"
                }`}
              >
                <Icon className="h-4 w-4 shrink-0" />
                <span className="truncate">{label}</span>
              </button>
            );
          })}
        </nav>

        {/* Bottom Quick Utilities */}
        <div className="space-y-2 border-t border-[#E5E5E5] p-3 dark:border-white/10">
          {simulatorEnabled && (
            <button
              type="button"
              onClick={() => setSimDrawerOpen((v) => !v)}
              className={`flex w-full items-center justify-center gap-2 rounded-xl border px-3 py-2 text-xs font-bold transition ${
                simDrawerOpen
                  ? "border-[#FF7A2F] bg-[#FF7A2F] text-white"
                  : "border-[#FF7A2F]/40 bg-[#FF7A2F]/12 text-[#C2410C] hover:bg-[#FF7A2F]/20 dark:text-[#FF9E66]"
              }`}
            >
              <Cpu className="h-3.5 w-3.5" />
              {t("app.nav.simulator")}
            </button>
          )}

          <div className="grid grid-cols-2 gap-2">
            <button
              type="button"
              onClick={toggleLanguage}
              className="inline-flex items-center justify-center gap-1.5 rounded-xl border border-[#E5E5E5] bg-[#F2F1F6] py-1.5 text-xs font-semibold text-[#4D4D4D] transition hover:border-[#FF7A2F] dark:border-white/10 dark:bg-white/[0.05] dark:text-white/80"
              title={t("app_settings.language")}
            >
              <Globe className="h-3.5 w-3.5 text-[#0066FF] dark:text-[#3399FF]" />
              {locale === "zh-CN" ? "EN" : "中文"}
            </button>

            <button
              type="button"
              onClick={toggleTheme}
              className="inline-flex items-center justify-center gap-1.5 rounded-xl border border-[#E5E5E5] bg-[#F2F1F6] py-1.5 text-xs font-semibold text-[#4D4D4D] transition hover:border-[#FF7A2F] dark:border-white/10 dark:bg-white/[0.05] dark:text-white/80"
              title={t("app_settings.appearance")}
            >
              {resolvedTheme === "dark" ? (
                <>
                  <Sun className="h-3.5 w-3.5 text-[#FF7A2F]" />
                  {t("app_settings.theme_light")}
                </>
              ) : (
                <>
                  <Moon className="h-3.5 w-3.5 text-[#727AFF]" />
                  {t("app_settings.theme_dark")}
                </>
              )}
            </button>
          </div>
        </div>
      </aside>

      {/* Main Scrollable Content Stage */}
      <main className="relative flex-1 overflow-y-auto px-6 py-6">
        <div className="mx-auto max-w-5xl space-y-5 pb-12">
          {/* Non-Intrusive Error Toast Banner */}
          {error && (
            <div
              role="alert"
              className="flex items-center justify-between gap-3 rounded-2xl border border-[#FF4C4C]/35 bg-[#FF4C4C]/12 px-4 py-3 text-xs font-semibold text-[#DC2626] dark:text-[#FF8080]"
            >
              <span className="inline-flex items-center gap-2">
                <AlertCircle className="h-4 w-4 shrink-0" />
                {error}
              </span>
              <button
                type="button"
                onClick={clearError}
                className="rounded-lg p-1 hover:bg-[#FF4C4C]/15"
                aria-label={t("common.close")}
              >
                <X className="h-3.5 w-3.5" />
              </button>
            </div>
          )}

          {/* Expandable Virtual Simulator Drawer (when opened from sidebar) */}
          {simulatorEnabled && simDrawerOpen && (
            <SimulatorDrawer defaultExpanded={true} />
          )}

          {/* Routed Active Page */}
          {activeTab === "dashboard" && <DashboardPage onNavigate={handleNavigate} />}
          {activeTab === "eq" && <SoundEqPage />}
          {activeTab === "gestures" && <ControlsPage onNavigate={handleNavigate} />}
          {activeTab === "modes" && <ModesLabPage />}
          {activeTab === "focus" && <SoundscapesPage />}
          {activeTab === "find" && <FindEarbudsPage />}
          {activeTab === "devices" && <DevicesOtaPage />}
          {activeTab === "app_settings" && <SettingsPage />}
        </div>
      </main>

      {/* Global Mutual-Exclusion & Reboot Confirmation Modal */}
      <MutualExclusionDialog
        open={Boolean(pendingConflict)}
        conflict={pendingConflict ? toDialogConflict(pendingConflict.prompt) : null}
        onConfirm={() => {
          void pendingConflict?.confirm();
        }}
        onCancel={() => pendingConflict?.cancel()}
        t={t}
      />

      {/* In-Window Connection Pop-Up Overlay Preview (browser preview only; Tauri uses native frameless popup window) */}
      {!isTauriRuntime() && popupPayload && (
        <div className="fixed right-5 bottom-5 z-40">
          <ConnectionPopup
            payload={popupPayload}
            theme={resolvedTheme}
            onOpenApp={() => {
              setActiveTab("dashboard");
              dismissPopup();
            }}
            onDismiss={dismissPopup}
            onQuickToggleAnc={(mode) => void setAncState({ mode })}
            autoDismiss={true}
          />
        </div>
      )}
    </div>
  );
};

export const App: React.FC<AppProps> = ({
  windowMode: explicitWindowMode,
  initialLocale,
  initialTab = "dashboard",
  client,
}) => {
  const [detectedMode, setDetectedMode] = useState<WindowMode>(
    () => explicitWindowMode ?? detectWindowModeFromLocation()
  );

  // Inside Tauri runtime, also inspect window label (`popup` vs `main`) safely
  useEffect(() => {
    if (explicitWindowMode) {
      setDetectedMode(explicitWindowMode);
      return;
    }
    if (!isTauriRuntime()) return;
    let cancelled = false;
    void (async () => {
      try {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        const label = getCurrentWindow().label;
        if (!cancelled && label === "popup") {
          setDetectedMode("popup");
        }
      } catch {
        // stay on location-derived mode
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [explicitWindowMode]);

  return (
    <QcyStoreProvider client={client}>
      <I18nProvider initialLocale={initialLocale}>
        <QcyShellInner windowMode={detectedMode} initialTab={initialTab} />
      </I18nProvider>
    </QcyStoreProvider>
  );
};

export default App;
