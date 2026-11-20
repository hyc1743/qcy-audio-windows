import React, { useMemo } from "react";
import { ArrowRight, Shield } from "lucide-react";
import {
  GestureRemapper,
  QCY_CARD_CLASS,
  type ButtonKeyId,
  type TouchSensitivityLevel,
} from "../components";
import { useI18n } from "../i18n";
import { useQcyStore } from "../store/useQcyStore";

export interface ControlsPageProps {
  onNavigate?: (tab: string) => void;
}

function resolveKeyIdFromEvent(eventId: number): number {
  if (eventId >= 1 && eventId <= 10) {
    return eventId % 2 === 1 ? 1 : 2;
  }
  if (eventId === 18) return 3; // Volume +
  if (eventId === 17) return 4; // Volume -
  if (eventId >= 19 && eventId <= 22) return 5; // MFB
  if (eventId >= 35 && eventId <= 37) return 6; // MFB / ANC
  return 1;
}

function wireToSensitivityLevel(val: number): TouchSensitivityLevel {
  if (val <= 40) return "moderate";
  if (val >= 50) return "sensitive";
  return "normal";
}

export const ControlsPage: React.FC<ControlsPageProps> = ({ onNavigate }) => {
  const {
    snapshot,
    loading,
    setButtonAction,
    resetButtons,
    setTouchConfig,
    setHeadControls,
  } = useQcyStore();
  const { t } = useI18n();

  const bindingsRecord = useMemo(() => {
    const rec: Record<number, number> = {};
    if (!snapshot) return rec;
    for (const entry of snapshot.gestures.mappings) {
      rec[entry.event_id] = entry.cmd;
    }
    return rec;
  }, [snapshot]);

  if (loading && !snapshot) {
    return (
      <div className={`${QCY_CARD_CLASS} py-12 text-center text-sm text-[#808080] dark:text-white/55`}>
        {t("common.loading")}
      </div>
    );
  }

  if (!snapshot) {
    return (
      <div className={`${QCY_CARD_CLASS} py-12 text-center text-sm text-[#808080] dark:text-white/55`}>
        {t("dashboard.no_active_device")}
      </div>
    );
  }

  const { active_model: model, gestures, connection_state } = snapshot;
  const caps = model.capabilities;
  const isDisconnected = connection_state === "disconnected";

  if (!caps.button_remap && !caps.head_controls) {
    return (
      <div className={`${QCY_CARD_CLASS} py-12 text-center text-sm text-[#808080] dark:text-white/55`}>
        {t("dashboard.feature_unavailable", { model: model.name_en })}
      </div>
    );
  }

  return (
    <div className="space-y-5">
      <GestureRemapper
        capabilities={{
          buttonRemap: caps.button_remap,
          supportedButtonActions: caps.supported_button_actions,
          buttonKeys: caps.button_keys as ButtonKeyId[],
          touchEnable: caps.touch_enable,
          touchSensitivity: caps.touch_sensitivity,
          ancButtonCycle: caps.anc_button_cycle,
          headControls: caps.head_controls,
        }}
        touchEnabled={gestures.touch_enable}
        touchSensitivity={wireToSensitivityLevel(gestures.touch_sensitivity)}
        bindings={bindingsRecord}
        headControls={{
          enabled: gestures.head_controls.enabled,
          nodAction: gestures.head_controls.nod_cmd === 2 ? 2 : 1,
          shakeAction: gestures.head_controls.shake_cmd === 1 ? 1 : 2,
          tiltLeftAction: gestures.head_controls.tilt_left_cmd === 4 ? 4 : 3,
          tiltRightAction: gestures.head_controls.tilt_right_cmd === 3 ? 3 : 4,
        }}
        disabled={isDisconnected}
        onTouchEnableChange={(enabled) => void setTouchConfig(enabled, undefined)}
        onTouchSensitivityChange={(_, wireVal) => void setTouchConfig(undefined, wireVal)}
        onBindingChange={(eventId, cmd) =>
          void setButtonAction(resolveKeyIdFromEvent(eventId), eventId, cmd)
        }
        onRestoreDefaults={() => void resetButtons()}
        onOpenAncCycle={onNavigate ? () => onNavigate("dashboard") : undefined}
        onHeadControlsChange={(next) =>
          void setHeadControls({
            enabled: next.enabled,
            nod_cmd: next.nodAction,
            shake_cmd: next.shakeAction,
            tilt_left_cmd: next.tiltLeftAction,
            tilt_right_cmd: next.tiltRightAction,
          })
        }
        t={t}
      />

      {/* Quick Jump Banner to ANC Button Switch Order */}
      {caps.anc_button_cycle && onNavigate && (
        <div className={`${QCY_CARD_CLASS} flex flex-wrap items-center justify-between gap-3`}>
          <div className="flex items-center gap-3">
            <span className="flex h-9 w-9 items-center justify-center rounded-xl bg-[#0066FF]/12 text-[#0066FF] dark:bg-[#3399FF]/15 dark:text-[#3399FF]">
              <Shield className="h-4 w-4" />
            </span>
            <div>
              <p className="text-sm font-bold text-[#1A1A1A] dark:text-white">
                {t("anc.anc_cycle_title")}
              </p>
              <p className="text-xs text-[#808080] dark:text-white/55">
                {t("anc.anc_cycle_desc")}
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={() => onNavigate("dashboard")}
            className="inline-flex items-center gap-1.5 rounded-xl bg-[#0066FF] px-3.5 py-2 text-xs font-semibold text-white transition hover:bg-[#0052CC] dark:bg-[#3399FF]"
          >
            {t("app.nav.dashboard")}
            <ArrowRight className="h-3.5 w-3.5" />
          </button>
        </div>
      )}
    </div>
  );
};
