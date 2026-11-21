import React from "react";
import {
  Bell,
  Cpu,
  Globe,
  Info,
  Monitor,
  Moon,
  ShieldCheck,
  Sparkles,
  Sun,
  Terminal,
} from "lucide-react";
import { useQcyStore } from "../store/useQcyStore";
import { useI18n, type SupportedLocale } from "../i18n";
import type { AppTheme } from "../types/qcy";

export const SettingsPage: React.FC = () => {
  const {
    snapshot,
    updateAppSettings,
    setSimulatorEnabled,
    triggerSimulatorPopup,
    client,
  } = useQcyStore();
  const { t, locale, setLocale } = useI18n();

  if (!snapshot) {
    return (
      <div className="rounded-2xl bg-white p-8 text-center text-sm text-neutral-500 dark:bg-[#1E1E1E] dark:text-neutral-400">
        {t("common.loading")}
      </div>
    );
  }

  const { settings } = snapshot;

  const handleLocaleChange = async (next: SupportedLocale) => {
    setLocale(next);
    await updateAppSettings({ locale: next });
  };

  const handleThemeChange = async (theme: AppTheme) => {
    await updateAppSettings({ theme });
  };

  const handlePreviewPopup = async () => {
    await triggerSimulatorPopup();
    await client.showPopup();
  };

  return (
    <div className="space-y-5">
      {/* Appearance & Language */}
      <section className="rounded-2xl border border-neutral-200/80 bg-white p-5 shadow-xs dark:border-white/10 dark:bg-[#1E1E1E]">
        <h2 className="mb-4 text-base font-semibold text-neutral-900 dark:text-white">
          {t("app_settings.appearance")} &amp; {t("app_settings.language")}
        </h2>

        <div className="space-y-5">
          {/* Theme */}
          <div>
            <p className="mb-2 text-xs font-medium text-neutral-500 dark:text-neutral-400">
              {t("app_settings.appearance")}
            </p>
            <div className="grid grid-cols-3 gap-2.5">
              {(
                [
                  { id: "system", icon: Monitor, key: "app_settings.theme_system" },
                  { id: "light", icon: Sun, key: "app_settings.theme_light" },
                  { id: "dark", icon: Moon, key: "app_settings.theme_dark" },
                ] as const
              ).map((opt) => {
                const Icon = opt.icon;
                const active = settings.theme === opt.id;
                return (
                  <button
                    key={opt.id}
                    type="button"
                    onClick={() => void handleThemeChange(opt.id)}
                    className={`flex items-center justify-center gap-2 rounded-xl border px-3 py-2.5 text-xs font-semibold transition ${
                      active
                        ? "border-[#FF7A2F] bg-[#FF7A2F]/12 text-[#FF7A2F]"
                        : "border-transparent bg-[#F2F1F6] text-neutral-700 hover:bg-neutral-200/70 dark:bg-white/5 dark:text-neutral-300 dark:hover:bg-white/10"
                    }`}
                  >
                    <Icon className="h-4 w-4" />
                    {t(opt.key)}
                  </button>
                );
              })}
            </div>
          </div>

          {/* Language */}
          <div>
            <p className="mb-2 flex items-center gap-1.5 text-xs font-medium text-neutral-500 dark:text-neutral-400">
              <Globe className="h-3.5 w-3.5" />
              {t("app_settings.language")}
            </p>
            <div className="grid grid-cols-2 gap-2.5">
              {(
                [
                  { id: "zh-CN", key: "app_settings.language_zh_cn" },
                  { id: "en-US", key: "app_settings.language_en_us" },
                ] as const
              ).map((lang) => {
                const active = locale === lang.id;
                return (
                  <button
                    key={lang.id}
                    type="button"
                    onClick={() => void handleLocaleChange(lang.id)}
                    className={`rounded-xl border px-3.5 py-2.5 text-xs font-semibold transition ${
                      active
                        ? "border-[#0066FF] bg-[#0066FF]/12 text-[#0066FF] dark:border-[#3399FF] dark:bg-[#3399FF]/15 dark:text-[#3399FF]"
                        : "border-transparent bg-[#F2F1F6] text-neutral-700 hover:bg-neutral-200/70 dark:bg-white/5 dark:text-neutral-300 dark:hover:bg-white/10"
                    }`}
                  >
                    {t(lang.key)}
                  </button>
                );
              })}
            </div>
          </div>
        </div>
      </section>

      {/* Windows Desktop Integration */}
      <section className="rounded-2xl border border-neutral-200/80 bg-white p-5 shadow-xs dark:border-white/10 dark:bg-[#1E1E1E]">
        <div className="mb-4 flex items-center gap-2">
          <Bell className="h-4 w-4 text-[#FF7A2F]" />
          <h2 className="text-base font-semibold text-neutral-900 dark:text-white">
            {t("app_settings.windows_integration")}
          </h2>
        </div>

        <div className="divide-y divide-neutral-200/70 dark:divide-white/10">
          {/* Minimize to Tray */}
          <div className="flex items-center justify-between py-3 first:pt-0">
            <div>
              <p className="text-sm font-medium text-neutral-900 dark:text-white">
                {t("app_settings.minimize_to_tray")}
              </p>
            </div>
            <button
              type="button"
              role="switch"
              aria-checked={settings.minimize_to_tray}
              onClick={() => void updateAppSettings({ minimize_to_tray: !settings.minimize_to_tray })}
              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                settings.minimize_to_tray ? "bg-[#FF7A2F]" : "bg-neutral-300 dark:bg-neutral-700"
              }`}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white shadow transition-transform ${
                  settings.minimize_to_tray ? "translate-x-6" : "translate-x-1"
                }`}
              />
            </button>
          </div>

          {/* Launch at Startup */}
          <div className="flex items-center justify-between py-3">
            <p className="text-sm font-medium text-neutral-900 dark:text-white">
              {t("app_settings.launch_at_startup")}
            </p>
            <button
              type="button"
              role="switch"
              aria-checked={settings.auto_start}
              onClick={() => void updateAppSettings({ auto_start: !settings.auto_start })}
              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                settings.auto_start ? "bg-[#FF7A2F]" : "bg-neutral-300 dark:bg-neutral-700"
              }`}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white shadow transition-transform ${
                  settings.auto_start ? "translate-x-6" : "translate-x-1"
                }`}
              />
            </button>
          </div>

          {/* Connection Pop-Up Overlay + Preview Button */}
          <div className="flex flex-wrap items-center justify-between gap-3 py-3">
            <div>
              <p className="text-sm font-medium text-neutral-900 dark:text-white">
                {t("app_settings.popup_on_connect")}
              </p>
            </div>
            <div className="flex items-center gap-2.5">
              <button
                type="button"
                onClick={() => void handlePreviewPopup()}
                className="rounded-xl bg-[#0066FF]/12 px-3 py-1.5 text-xs font-semibold text-[#0066FF] hover:bg-[#0066FF]/20 dark:bg-[#3399FF]/15 dark:text-[#3399FF]"
              >
                {t("simulator.trigger_popup_preview")}
              </button>
              <button
                type="button"
                role="switch"
                aria-checked={settings.show_connection_popup}
                onClick={() =>
                  void updateAppSettings({
                    show_connection_popup: !settings.show_connection_popup,
                  })
                }
                className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                  settings.show_connection_popup
                    ? "bg-[#FF7A2F]"
                    : "bg-neutral-300 dark:bg-neutral-700"
                }`}
              >
                <span
                  className={`inline-block h-4 w-4 transform rounded-full bg-white shadow transition-transform ${
                    settings.show_connection_popup ? "translate-x-6" : "translate-x-1"
                  }`}
                />
              </button>
            </div>
          </div>

          {/* Virtual QCY Earbuds Simulator Toggle */}
          <div className="flex items-center justify-between py-3">
            <div>
              <div className="flex items-center gap-2">
                <Cpu className="h-4 w-4 text-[#FF7A2F]" />
                <p className="text-sm font-medium text-neutral-900 dark:text-white">
                  {t("simulator.enable_simulator")}
                </p>
              </div>
              <p className="mt-0.5 text-xs text-neutral-500 dark:text-neutral-400">
                {t("simulator.subtitle")}
              </p>
            </div>
            <button
              type="button"
              role="switch"
              aria-checked={settings.simulator_enabled}
              onClick={() => void setSimulatorEnabled(!settings.simulator_enabled)}
              className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors ${
                settings.simulator_enabled ? "bg-[#FF7A2F]" : "bg-neutral-300 dark:bg-neutral-700"
              }`}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white shadow transition-transform ${
                  settings.simulator_enabled ? "translate-x-6" : "translate-x-1"
                }`}
              />
            </button>
          </div>

          {/* Developer Protocol Logging */}
          <div className="flex items-center justify-between pt-3">
            <div className="flex items-center gap-2">
              <Terminal className="h-4 w-4 text-neutral-400" />
              <p className="text-sm font-medium text-neutral-900 dark:text-white">
                {t("simulator.protocol_inspector")}
              </p>
            </div>
            <button
              type="button"
              role="switch"
              aria-checked={settings.developer_packet_log}
              onClick={() =>
                void updateAppSettings({
                  developer_packet_log: !settings.developer_packet_log,
                })
              }
              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                settings.developer_packet_log
                  ? "bg-[#0066FF] dark:bg-[#3399FF]"
                  : "bg-neutral-300 dark:bg-neutral-700"
              }`}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white shadow transition-transform ${
                  settings.developer_packet_log ? "translate-x-6" : "translate-x-1"
                }`}
              />
            </button>
          </div>
        </div>
      </section>

      {/* About Clean-Room Open-Source Project */}
      <section className="rounded-2xl border border-neutral-200/80 bg-white p-5 shadow-xs dark:border-white/10 dark:bg-[#1E1E1E]">
        <div className="flex items-start gap-3.5">
          <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-2xl bg-gradient-to-br from-[#FF7A2F] to-[#0066FF] text-white shadow-md">
            <Sparkles className="h-5 w-5" />
          </div>
          <div className="space-y-2">
            <div className="flex flex-wrap items-center gap-2">
              <h2 className="text-base font-semibold text-neutral-900 dark:text-white">
                {t("app_settings.about_title")}
              </h2>
              <span className="rounded-full bg-[#FF7A2F]/15 px-2.5 py-0.5 text-[11px] font-semibold text-[#FF7A2F]">
                {t("app_settings.about_version", { version: "0.1.1" })}
              </span>
              <span className="rounded-full bg-emerald-500/15 px-2.5 py-0.5 text-[11px] font-semibold text-emerald-600 dark:text-emerald-400">
                {t("app_settings.about_license")}
              </span>
            </div>

            <p className="text-xs leading-relaxed text-neutral-600 dark:text-neutral-300">
              {t("app_settings.about_desc")}
            </p>

            <div className="flex flex-wrap gap-3 pt-1 text-xs text-neutral-500 dark:text-neutral-400">
              <span className="inline-flex items-center gap-1.5">
                <ShieldCheck className="h-4 w-4 text-emerald-500" />
                {t("app_settings.about_open_source")}
              </span>
              <span className="inline-flex items-center gap-1.5">
                <Info className="h-4 w-4 text-[#0066FF] dark:text-[#3399FF]" />
                {t("app_settings.about_privacy_note")}
              </span>
            </div>
          </div>
        </div>
      </section>
    </div>
  );
};

export default SettingsPage;
