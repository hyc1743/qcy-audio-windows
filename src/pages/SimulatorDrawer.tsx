import React, { useState } from "react";
import {
  BatteryCharging,
  ChevronDown,
  ChevronUp,
  Cpu,
  Ear,
  Plug,
  Sparkles,
  Unplug,
  Zap,
} from "lucide-react";
import { useQcyStore } from "../store/useQcyStore";
import { useI18n } from "../i18n";

export interface SimulatorDrawerProps {
  /** Whether the drawer starts expanded (default false for compact bar, true when placed in a dedicated tab). */
  defaultExpanded?: boolean;
}

export const SimulatorDrawer: React.FC<SimulatorDrawerProps> = ({
  defaultExpanded = false,
}) => {
  const {
    snapshot,
    models,
    setSimulatorModel,
    updateSimulatorState,
    triggerSimulatorPopup,
    connectDevice,
    disconnectDevice,
    runFitTest,
  } = useQcyStore();
  const { t, locale } = useI18n();
  const [expanded, setExpanded] = useState(defaultExpanded);

  if (!snapshot || !snapshot.simulator.enabled) {
    return null;
  }

  const { simulator, battery, active_model } = snapshot;
  const isConnected = snapshot.connection_state === "connected";

  const handleDrain5 = () => {
    void updateSimulatorState({
      battery: {
        left: battery.left !== null ? Math.max(0, battery.left - 5) : null,
        right: battery.right !== null ? Math.max(0, battery.right - 5) : null,
        case_level: battery.case_level !== null ? Math.max(0, battery.case_level - 5) : null,
      },
    });
  };

  return (
    <aside className="rounded-2xl border border-[#FF7A2F]/35 bg-white shadow-sm dark:border-[#FF7A2F]/40 dark:bg-[#1E1E1E]">
      {/* Compact Header Bar */}
      <div className="flex flex-wrap items-center justify-between gap-2 px-4 py-3">
        <button
          type="button"
          onClick={() => setExpanded((v) => !v)}
          className="flex items-center gap-2.5 text-left"
        >
          <span className="inline-flex items-center gap-1 rounded-full bg-[#FF7A2F]/15 px-2.5 py-0.5 text-[10px] font-bold tracking-wider text-[#FF7A2F]">
            <Cpu className="h-3 w-3" />
            {t("simulator.badge")}
          </span>
          <span className="text-xs font-semibold text-neutral-900 dark:text-white">
            {locale === "zh-CN" ? active_model.name_zh : active_model.name_en}
          </span>
          <span className="font-mono text-[11px] text-neutral-500">
            ({active_model.protocol_family})
          </span>
          {expanded ? (
            <ChevronUp className="h-4 w-4 text-neutral-400" />
          ) : (
            <ChevronDown className="h-4 w-4 text-neutral-400" />
          )}
        </button>

        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => void triggerSimulatorPopup()}
            className="inline-flex items-center gap-1.5 rounded-xl bg-[#FF7A2F] px-3 py-1.5 text-xs font-semibold text-white shadow-xs hover:bg-[#e86920]"
          >
            <Sparkles className="h-3.5 w-3.5" />
            {t("simulator.trigger_popup_preview")}
          </button>

          <button
            type="button"
            onClick={() =>
              void (isConnected
                ? disconnectDevice()
                : connectDevice(
                    snapshot.active_device?.address ?? "51:43:59:00:5D:40",
                    active_model.model_id
                  ))
            }
            className="inline-flex items-center gap-1 rounded-xl bg-[#F2F1F6] px-3 py-1.5 text-xs font-semibold text-neutral-700 hover:bg-neutral-200 dark:bg-white/10 dark:text-neutral-200"
          >
            {isConnected ? (
              <>
                <Unplug className="h-3.5 w-3.5 text-rose-500" />
                {t("simulator.disconnect_virtual")}
              </>
            ) : (
              <>
                <Plug className="h-3.5 w-3.5 text-emerald-500" />
                {t("simulator.connect_virtual")}
              </>
            )}
          </button>
        </div>
      </div>

      {/* Expandable Simulator Controls */}
      {expanded && (
        <div className="grid gap-4 border-t border-neutral-200/70 p-4 dark:border-white/10 lg:grid-cols-3">
          {/* Column 1: Model Switcher & Latency */}
          <div className="space-y-3">
            <div>
              <label className="mb-1.5 block text-xs font-semibold text-neutral-700 dark:text-neutral-300">
                {t("simulator.select_model")}
              </label>
              <select
                value={simulator.model_id}
                onChange={(e) => void setSimulatorModel(e.target.value)}
                className="w-full rounded-xl border border-neutral-300 bg-[#F2F1F6] px-3 py-2 text-xs font-medium text-neutral-900 outline-none focus:border-[#FF7A2F] dark:border-white/15 dark:bg-white/5 dark:text-white"
              >
                {models.map((m) => (
                  <option key={m.model_id} value={m.model_id} className="dark:bg-[#1E1E1E]">
                    {locale === "zh-CN" ? m.name_zh : m.name_en} [{m.form_factor} ·{" "}
                    {m.chipset_family}]
                  </option>
                ))}
              </select>
            </div>

            <div>
              <div className="mb-1 flex items-center justify-between text-xs">
                <span className="text-neutral-600 dark:text-neutral-400">
                  {locale === "zh-CN" ? "模拟 BLE 延迟" : "Simulated BLE Latency"}
                </span>
                <span className="font-mono font-semibold text-neutral-900 dark:text-white">
                  {simulator.latency_ms} ms
                </span>
              </div>
              <input
                type="range"
                min={0}
                max={300}
                step={5}
                value={simulator.latency_ms}
                onChange={(e) =>
                  void updateSimulatorState({ latency_ms: Number(e.target.value) })
                }
                className="w-full accent-[#FF7A2F]"
              />
            </div>

            <div className="flex flex-wrap gap-2 pt-1">
              <button
                type="button"
                onClick={handleDrain5}
                className="rounded-lg bg-[#F2F1F6] px-2.5 py-1.5 text-xs font-semibold text-neutral-700 hover:bg-neutral-200 dark:bg-white/10 dark:text-neutral-200"
              >
                {t("simulator.simulate_drain")}
              </button>
              <button
                type="button"
                onClick={() => void runFitTest()}
                className="rounded-lg bg-[#0066FF]/12 px-2.5 py-1.5 text-xs font-semibold text-[#0066FF] dark:bg-[#3399FF]/15 dark:text-[#3399FF]"
              >
                {t("wear_and_health.fit_test_title")}
              </button>
            </div>
          </div>

          {/* Column 2: Battery Sliders & Charging Toggles */}
          <div className="space-y-2.5">
            <p className="flex items-center gap-1.5 text-xs font-semibold text-neutral-700 dark:text-neutral-300">
              <BatteryCharging className="h-3.5 w-3.5 text-emerald-500" />
              {t("simulator.battery_controls")}
            </p>

            {/* Left Battery */}
            <div className="space-y-1">
              <div className="flex items-center justify-between text-xs">
                <span className="text-neutral-600 dark:text-neutral-400">
                  {t("common.left")}: <strong className="font-mono">{battery.left ?? 0}%</strong>
                </span>
                <button
                  type="button"
                  onClick={() =>
                    void updateSimulatorState({
                      battery: { left_charging: !battery.left_charging },
                    })
                  }
                  className={`inline-flex items-center gap-1 rounded-md px-2 py-0.5 text-[10px] font-semibold ${
                    battery.left_charging
                      ? "bg-emerald-500/20 text-emerald-600 dark:text-emerald-400"
                      : "bg-neutral-200/70 text-neutral-500 dark:bg-white/10"
                  }`}
                >
                  <Zap className="h-2.5 w-2.5" />
                  {t("dashboard.charging")}
                </button>
              </div>
              <input
                type="range"
                min={0}
                max={100}
                value={battery.left ?? 0}
                onChange={(e) =>
                  void updateSimulatorState({ battery: { left: Number(e.target.value) } })
                }
                className="w-full accent-[#0066FF]"
              />
            </div>

            {/* Right Battery */}
            <div className="space-y-1">
              <div className="flex items-center justify-between text-xs">
                <span className="text-neutral-600 dark:text-neutral-400">
                  {t("common.right")}: <strong className="font-mono">{battery.right ?? 0}%</strong>
                </span>
                <button
                  type="button"
                  onClick={() =>
                    void updateSimulatorState({
                      battery: { right_charging: !battery.right_charging },
                    })
                  }
                  className={`inline-flex items-center gap-1 rounded-md px-2 py-0.5 text-[10px] font-semibold ${
                    battery.right_charging
                      ? "bg-emerald-500/20 text-emerald-600 dark:text-emerald-400"
                      : "bg-neutral-200/70 text-neutral-500 dark:bg-white/10"
                  }`}
                >
                  <Zap className="h-2.5 w-2.5" />
                  {t("dashboard.charging")}
                </button>
              </div>
              <input
                type="range"
                min={0}
                max={100}
                value={battery.right ?? 0}
                onChange={(e) =>
                  void updateSimulatorState({ battery: { right: Number(e.target.value) } })
                }
                className="w-full accent-[#0066FF]"
              />
            </div>

            {/* Case Battery (when supported) */}
            {active_model.capabilities.has_case_battery && (
              <div className="space-y-1">
                <div className="flex items-center justify-between text-xs">
                  <span className="text-neutral-600 dark:text-neutral-400">
                    {t("common.case")}:{" "}
                    <strong className="font-mono">{battery.case_level ?? 0}%</strong>
                  </span>
                  <button
                    type="button"
                    onClick={() =>
                      void updateSimulatorState({
                        battery: { case_charging: !battery.case_charging },
                      })
                    }
                    className={`inline-flex items-center gap-1 rounded-md px-2 py-0.5 text-[10px] font-semibold ${
                      battery.case_charging
                        ? "bg-emerald-500/20 text-emerald-600 dark:text-emerald-400"
                        : "bg-neutral-200/70 text-neutral-500 dark:bg-white/10"
                    }`}
                  >
                    <Zap className="h-2.5 w-2.5" />
                    {t("dashboard.charging")}
                  </button>
                </div>
                <input
                  type="range"
                  min={0}
                  max={100}
                  value={battery.case_level ?? 0}
                  onChange={(e) =>
                    void updateSimulatorState({ battery: { case_level: Number(e.target.value) } })
                  }
                  className="w-full accent-[#FF7A2F]"
                />
              </div>
            )}
          </div>

          {/* Column 3: Wear & Case Lid State */}
          <div className="space-y-2.5">
            <p className="flex items-center gap-1.5 text-xs font-semibold text-neutral-700 dark:text-neutral-300">
              <Ear className="h-3.5 w-3.5 text-[#0066FF] dark:text-[#3399FF]" />
              {t("simulator.wear_state")}
            </p>

            <div className="space-y-2">
              {[
                {
                  label: t("simulator.left_in_ear"),
                  checked: !battery.left_in_case,
                  toggle: () =>
                    void updateSimulatorState({
                      battery: { left_in_case: !battery.left_in_case },
                    }),
                },
                {
                  label: t("simulator.right_in_ear"),
                  checked: !battery.right_in_case,
                  toggle: () =>
                    void updateSimulatorState({
                      battery: { right_in_case: !battery.right_in_case },
                    }),
                },
                {
                  label: t("simulator.case_open"),
                  checked: simulator.lid_open,
                  toggle: () => void updateSimulatorState({ lid_open: !simulator.lid_open }),
                },
              ].map((item) => (
                <button
                  key={item.label}
                  type="button"
                  onClick={item.toggle}
                  className={`flex w-full items-center justify-between rounded-xl px-3 py-2 text-xs font-medium transition ${
                    item.checked
                      ? "bg-[#FF7A2F]/12 text-[#FF7A2F]"
                      : "bg-[#F2F1F6] text-neutral-600 dark:bg-white/5 dark:text-neutral-400"
                  }`}
                >
                  <span>{item.label}</span>
                  <span className="font-mono text-[10px] font-bold">
                    {item.checked ? t("common.on") : t("common.off")}
                  </span>
                </button>
              ))}
            </div>
          </div>
        </div>
      )}
    </aside>
  );
};

export default SimulatorDrawer;
