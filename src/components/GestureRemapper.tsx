import React, { useState } from "react";
import {
  CheckCircle2,
  Fingerprint,
  Headphones,
  MoveHorizontal,
  MoveVertical,
  PhoneCall,
  Music,
  RotateCcw,
  SlidersHorizontal,
  Volume2,
} from "lucide-react";
import {
  QCY_CARD_CLASS,
  QCY_SUBCARD_CLASS,
  TranslateFn,
  toggleTrackClass,
} from "./shared";

export type ButtonKeyId =
  | "left"
  | "right"
  | "volume_up"
  | "volume_down"
  | "mfb"
  | "mfb_anc";

export type TouchSensitivityLevel = "moderate" | "normal" | "sensitive";

export interface GestureEventSlot {
  /** QCY wire `eventID` (e.g., `1..10` for TWS L/R, `17..22` / `35..37` for Over-Ear). */
  eventId: number;
  labelKey: string;
  actionCmd: number;
}

export interface HeadControlMapping {
  enabled: boolean;
  nodAction: 1 | 2; // 1 = answer, 2 = reject
  shakeAction: 1 | 2;
  tiltLeftAction: 3 | 4; // 3 = prev, 4 = next
  tiltRightAction: 3 | 4;
}

export interface GestureCapabilities {
  buttonRemap: boolean;
  supportedButtonActions: number[];
  buttonKeys?: ButtonKeyId[];
  touchEnable?: boolean;
  touchSensitivity?: boolean;
  touchTone?: boolean;
  ancButtonCycle?: boolean;
  headControls?: boolean;
}

export interface GestureRemapperProps {
  capabilities: GestureCapabilities;
  touchEnabled: boolean;
  touchSensitivity?: TouchSensitivityLevel;
  touchToneEnabled?: boolean;
  /** Map of `eventId -> actionCmd` (`0..18`). */
  bindings: Record<number, number>;
  headControls?: HeadControlMapping;
  disabled?: boolean;
  onTouchEnableChange?: (enabled: boolean) => void;
  onTouchSensitivityChange?: (level: TouchSensitivityLevel, wireValue: number) => void;
  onTouchToneChange?: (enabled: boolean) => void;
  onBindingChange: (eventId: number, actionCmd: number) => void;
  onRestoreDefaults?: () => void;
  onOpenAncCycle?: () => void;
  onHeadControlsChange?: (next: HeadControlMapping) => void;
  t: TranslateFn;
  className?: string;
}

/** Exact QCY wire `0x33` touch sensitivity values (`0x26=38`, `0x2D=45`, `0x34=52`). */
export const TOUCH_SENSITIVITY_WIRE: Record<TouchSensitivityLevel, number> = {
  moderate: 38,
  normal: 45,
  sensitive: 52,
};

/** All 14 verified QCY assignable button action command IDs (`cmd 0..18`). */
export const ALL_QCY_BUTTON_ACTIONS: readonly number[] = [
  0, 1, 2, 3, 4, 5, 6, 7, 11, 14, 15, 16, 17, 18,
];

/** Default gesture event slots per QCY physical key type. */
export function getEventSlotsForKey(
  key: ButtonKeyId,
  bindings: Record<number, number>
): GestureEventSlot[] {
  switch (key) {
    case "left":
      return [
        { eventId: 1, labelKey: "gestures.event_single_tap", actionCmd: bindings[1] ?? 6 },
        { eventId: 3, labelKey: "gestures.event_double_tap", actionCmd: bindings[3] ?? 1 },
        { eventId: 5, labelKey: "gestures.event_triple_tap", actionCmd: bindings[5] ?? 4 },
        { eventId: 7, labelKey: "gestures.event_quad_tap", actionCmd: bindings[7] ?? 0 },
        { eventId: 9, labelKey: "gestures.event_long_press", actionCmd: bindings[9] ?? 2 },
      ];
    case "right":
      return [
        { eventId: 2, labelKey: "gestures.event_single_tap", actionCmd: bindings[2] ?? 5 },
        { eventId: 4, labelKey: "gestures.event_double_tap", actionCmd: bindings[4] ?? 1 },
        { eventId: 6, labelKey: "gestures.event_triple_tap", actionCmd: bindings[6] ?? 7 },
        { eventId: 8, labelKey: "gestures.event_quad_tap", actionCmd: bindings[8] ?? 0 },
        { eventId: 10, labelKey: "gestures.event_long_press", actionCmd: bindings[10] ?? 11 },
      ];
    case "volume_up":
      return [
        { eventId: 18, labelKey: "gestures.event_single_click", actionCmd: bindings[18] ?? 5 },
      ];
    case "volume_down":
      return [
        { eventId: 17, labelKey: "gestures.event_single_click", actionCmd: bindings[17] ?? 6 },
      ];
    case "mfb":
      return [
        { eventId: 19, labelKey: "gestures.event_single_click", actionCmd: bindings[19] ?? 1 },
        { eventId: 20, labelKey: "gestures.event_double_click", actionCmd: bindings[20] ?? 3 },
        { eventId: 21, labelKey: "gestures.event_triple_click", actionCmd: bindings[21] ?? 2 },
        { eventId: 22, labelKey: "gestures.event_quad_click", actionCmd: bindings[22] ?? 4 },
      ];
    case "mfb_anc":
      return [
        { eventId: 35, labelKey: "gestures.event_single_click", actionCmd: bindings[35] ?? 14 },
        { eventId: 36, labelKey: "gestures.event_double_click", actionCmd: bindings[36] ?? 15 },
        { eventId: 37, labelKey: "gestures.event_triple_click", actionCmd: bindings[37] ?? 18 },
      ];
  }
}

const KEY_LABEL_MAP: Record<ButtonKeyId, string> = {
  left: "gestures.key_left",
  right: "gestures.key_right",
  volume_up: "gestures.key_volume_up",
  volume_down: "gestures.key_volume_down",
  mfb: "gestures.key_mfb",
  mfb_anc: "gestures.key_mfb_anc",
};

export const GestureRemapper: React.FC<GestureRemapperProps> = ({
  capabilities,
  touchEnabled,
  touchSensitivity = "normal",
  touchToneEnabled = true,
  bindings,
  headControls,
  disabled = false,
  onTouchEnableChange,
  onTouchSensitivityChange,
  onTouchToneChange,
  onBindingChange,
  onRestoreDefaults,
  onOpenAncCycle,
  onHeadControlsChange,
  t,
  className = "",
}) => {
  const keys: ButtonKeyId[] =
    capabilities.buttonKeys && capabilities.buttonKeys.length > 0
      ? capabilities.buttonKeys
      : ["left", "right"];
  const [activeKey, setActiveKey] = useState<ButtonKeyId>(keys[0]);
  const [practiceStep, setPracticeStep] = useState<null | "nod" | "shake" | "tilt" | "done">(
    null
  );

  const resolvedKey = keys.includes(activeKey) ? activeKey : keys[0];
  const allowedActions =
    capabilities.supportedButtonActions.length > 0
      ? capabilities.supportedButtonActions
      : [...ALL_QCY_BUTTON_ACTIONS];
  const eventSlots = getEventSlotsForKey(resolvedKey, bindings);

  const headState: HeadControlMapping = headControls ?? {
    enabled: false,
    nodAction: 1,
    shakeAction: 2,
    tiltLeftAction: 3,
    tiltRightAction: 4,
  };

  if (!capabilities.buttonRemap && !capabilities.headControls) {
    return null;
  }

  return (
    <section
      className={`${QCY_CARD_CLASS} space-y-5 ${className}`}
      aria-label={t("gestures.title")}
    >
      {/* Header */}
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex items-center gap-2.5">
          <span className="flex h-9 w-9 items-center justify-center rounded-xl bg-[#FF7A2F]/14 text-[#FF7A2F]">
            <Fingerprint className="h-5 w-5" />
          </span>
          <h3 className="text-base font-bold text-[#1A1A1A] dark:text-white">
            {t("gestures.title")}
          </h3>
        </div>

        <div className="flex items-center gap-2">
          {capabilities.ancButtonCycle && onOpenAncCycle && (
            <button
              type="button"
              disabled={disabled}
              onClick={onOpenAncCycle}
              className="rounded-lg border border-[#0066FF]/35 bg-[#0066FF]/8 px-2.5 py-1.5 text-xs font-semibold text-[#0066FF] hover:bg-[#0066FF]/15 dark:text-[#66B2FF]"
            >
              {t("gestures.anc_switch_order")}
            </button>
          )}
          {onRestoreDefaults && (
            <button
              type="button"
              disabled={disabled}
              onClick={onRestoreDefaults}
              className="inline-flex items-center gap-1 rounded-lg border border-[#E5E5E5] px-2.5 py-1.5 text-xs font-semibold text-[#4D4D4D] hover:border-[#FF7A2F] dark:border-white/12 dark:text-white/75"
            >
              <RotateCcw className="h-3.5 w-3.5" />
              {t("common.restore_defaults")}
            </button>
          )}
        </div>
      </div>

      {/* Master Gesture Enable Toggle + Touch Sensitivity + Touch Tone */}
      <div className="grid gap-3 sm:grid-cols-2">
        {capabilities.touchEnable && onTouchEnableChange && (
          <div className={`${QCY_SUBCARD_CLASS} flex items-center justify-between gap-3 sm:col-span-2`}>
            <div>
              <p className="text-sm font-semibold text-[#1A1A1A] dark:text-white">
                {t("gestures.master_toggle")}
              </p>
              <p className="text-xs text-[#808080] dark:text-white/55">
                {t("gestures.master_toggle_desc")}
              </p>
            </div>
            <button
              type="button"
              role="switch"
              aria-checked={touchEnabled}
              disabled={disabled}
              onClick={() => onTouchEnableChange(!touchEnabled)}
              className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors ${toggleTrackClass(
                touchEnabled
              )}`}
            >
              <span
                className={`inline-block h-4 w-4 rounded-full bg-white shadow transition-transform ${
                  touchEnabled ? "translate-x-6" : "translate-x-1"
                }`}
              />
            </button>
          </div>
        )}

        {capabilities.touchSensitivity && onTouchSensitivityChange && (
          <div className={`${QCY_SUBCARD_CLASS} space-y-2`}>
            <div className="flex items-center gap-2 text-xs font-semibold text-[#1A1A1A] dark:text-white">
              <SlidersHorizontal className="h-3.5 w-3.5 text-[#FF7A2F]" />
              {t("gestures.touch_sensitivity")}
            </div>
            <div className="grid grid-cols-3 gap-1.5">
              {(["moderate", "normal", "sensitive"] as const).map((lvl) => (
                <button
                  key={lvl}
                  type="button"
                  disabled={disabled}
                  onClick={() => onTouchSensitivityChange(lvl, TOUCH_SENSITIVITY_WIRE[lvl])}
                  className={`rounded-lg py-1.5 text-xs font-semibold transition ${
                    touchSensitivity === lvl
                      ? "bg-[#FF7A2F] text-white"
                      : "bg-white text-[#4D4D4D] hover:bg-black/5 dark:bg-white/8 dark:text-white/75"
                  }`}
                >
                  {t(`gestures.sensitivity_${lvl}`)}
                </button>
              ))}
            </div>
          </div>
        )}

        {capabilities.touchTone && onTouchToneChange && (
          <div className={`${QCY_SUBCARD_CLASS} flex items-center justify-between gap-3`}>
            <div className="flex items-center gap-2">
              <Volume2 className="h-4 w-4 text-[#0066FF] dark:text-[#3399FF]" />
              <div>
                <p className="text-xs font-bold text-[#1A1A1A] dark:text-white">
                  {t("gestures.touch_tone")}
                </p>
                <p className="text-[11px] text-[#808080] dark:text-white/55">
                  {t("gestures.touch_tone_desc")}
                </p>
              </div>
            </div>
            <button
              type="button"
              role="switch"
              aria-checked={touchToneEnabled}
              disabled={disabled}
              onClick={() => onTouchToneChange(!touchToneEnabled)}
              className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors ${toggleTrackClass(
                touchToneEnabled,
                "blue"
              )}`}
            >
              <span
                className={`inline-block h-4 w-4 rounded-full bg-white shadow transition-transform ${
                  touchToneEnabled ? "translate-x-6" : "translate-x-1"
                }`}
              />
            </button>
          </div>
        )}
      </div>

      {/* Physical Key / Earbud Tabs + Gesture Action Pickers */}
      {capabilities.buttonRemap && (
        <div className={`space-y-3 ${!touchEnabled && capabilities.touchEnable ? "opacity-55" : ""}`}>
          <div
            className="flex flex-wrap gap-1.5 rounded-xl bg-[#F2F1F6] p-1.5 dark:bg-white/[0.06]"
            role="tablist"
          >
            {keys.map((keyId) => {
              const active = keyId === resolvedKey;
              return (
                <button
                  key={keyId}
                  type="button"
                  role="tab"
                  aria-selected={active}
                  onClick={() => setActiveKey(keyId)}
                  className={`flex-1 rounded-lg px-3 py-2 text-xs font-semibold transition ${
                    active
                      ? "bg-[#FF7A2F] text-white shadow-xs"
                      : "text-[#4D4D4D] hover:bg-black/5 dark:text-white/75 dark:hover:bg-white/8"
                  }`}
                >
                  {t(KEY_LABEL_MAP[keyId])}
                </button>
              );
            })}
          </div>

          <div className="divide-y divide-[#E5E5E5] rounded-xl border border-[#E5E5E5] bg-[#F2F1F6]/60 dark:divide-white/8 dark:border-white/10 dark:bg-white/[0.03]">
            {eventSlots.map((slot) => (
              <div
                key={slot.eventId}
                className="flex flex-wrap items-center justify-between gap-3 px-4 py-3"
              >
                <span className="text-sm font-semibold text-[#1A1A1A] dark:text-white">
                  {t(slot.labelKey)}
                </span>
                <select
                  disabled={disabled || (capabilities.touchEnable && !touchEnabled)}
                  value={slot.actionCmd}
                  onChange={(e) => onBindingChange(slot.eventId, Number(e.target.value))}
                  aria-label={`${t(KEY_LABEL_MAP[resolvedKey])} ${t(slot.labelKey)}`}
                  className="min-w-[180px] rounded-lg border border-[#E5E5E5] bg-white px-3 py-1.5 text-xs font-semibold text-[#1A1A1A] outline-none focus:border-[#FF7A2F] disabled:cursor-not-allowed dark:border-white/12 dark:bg-[#1E1E1E] dark:text-white"
                >
                  {allowedActions.map((cmd) => (
                    <option key={cmd} value={cmd}>
                      {t(`gestures.actions.cmd_${cmd}`)}
                    </option>
                  ))}
                </select>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* IMU Head Controls Section (HT21 series, cmdID=80) */}
      {capabilities.headControls && (
        <div className={`${QCY_SUBCARD_CLASS} space-y-4`}>
          <div className="flex items-center justify-between gap-3">
            <div className="flex items-center gap-2.5">
              <Headphones className="h-4 w-4 text-[#727AFF]" />
              <div>
                <p className="text-sm font-bold text-[#1A1A1A] dark:text-white">
                  {t("head_controls.title")}
                </p>
                <p className="text-xs text-[#808080] dark:text-white/55">
                  {t("head_controls.subtitle")}
                </p>
              </div>
            </div>
            <button
              type="button"
              role="switch"
              aria-checked={headState.enabled}
              disabled={disabled}
              onClick={() =>
                onHeadControlsChange?.({ ...headState, enabled: !headState.enabled })
              }
              className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors ${toggleTrackClass(
                headState.enabled
              )}`}
            >
              <span
                className={`inline-block h-4 w-4 rounded-full bg-white shadow transition-transform ${
                  headState.enabled ? "translate-x-6" : "translate-x-1"
                }`}
              />
            </button>
          </div>

          {headState.enabled && (
            <div className="space-y-3 border-t border-[#E5E5E5] pt-3 dark:border-white/10">
              <p className="text-xs text-[#808080] dark:text-white/55">
                {t("head_controls.both_ears_required")}
              </p>

              <div className="grid gap-3 sm:grid-cols-2">
                {/* Incoming Calls */}
                <div className="space-y-2.5 rounded-xl border border-[#E5E5E5] bg-white p-3 dark:border-white/10 dark:bg-[#1E1E1E]">
                  <p className="inline-flex items-center gap-1.5 text-xs font-bold text-[#1A1A1A] dark:text-white">
                    <PhoneCall className="h-3.5 w-3.5 text-[#22C55E]" />
                    {t("head_controls.group_calls")}
                  </p>
                  <div className="flex items-center justify-between gap-2 text-xs">
                    <span className="inline-flex items-center gap-1 text-[#4D4D4D] dark:text-white/75">
                      <MoveVertical className="h-3.5 w-3.5 text-[#FF7A2F]" />
                      {t("head_controls.gesture_nod")}
                    </span>
                    <select
                      disabled={disabled}
                      value={headState.nodAction}
                      onChange={(e) =>
                        onHeadControlsChange?.({
                          ...headState,
                          nodAction: Number(e.target.value) as 1 | 2,
                        })
                      }
                      className="rounded-lg border border-[#E5E5E5] bg-[#F2F1F6] px-2 py-1 text-xs font-semibold dark:border-white/12 dark:bg-[#121212] dark:text-white"
                    >
                      <option value={1}>{t("head_controls.action_answer_call")}</option>
                      <option value={2}>{t("head_controls.action_reject_call")}</option>
                    </select>
                  </div>
                  <div className="flex items-center justify-between gap-2 text-xs">
                    <span className="inline-flex items-center gap-1 text-[#4D4D4D] dark:text-white/75">
                      <MoveHorizontal className="h-3.5 w-3.5 text-[#FF7A2F]" />
                      {t("head_controls.gesture_shake")}
                    </span>
                    <select
                      disabled={disabled}
                      value={headState.shakeAction}
                      onChange={(e) =>
                        onHeadControlsChange?.({
                          ...headState,
                          shakeAction: Number(e.target.value) as 1 | 2,
                        })
                      }
                      className="rounded-lg border border-[#E5E5E5] bg-[#F2F1F6] px-2 py-1 text-xs font-semibold dark:border-white/12 dark:bg-[#121212] dark:text-white"
                    >
                      <option value={1}>{t("head_controls.action_answer_call")}</option>
                      <option value={2}>{t("head_controls.action_reject_call")}</option>
                    </select>
                  </div>
                </div>

                {/* Music Playback */}
                <div className="space-y-2.5 rounded-xl border border-[#E5E5E5] bg-white p-3 dark:border-white/10 dark:bg-[#1E1E1E]">
                  <p className="inline-flex items-center gap-1.5 text-xs font-bold text-[#1A1A1A] dark:text-white">
                    <Music className="h-3.5 w-3.5 text-[#0066FF] dark:text-[#3399FF]" />
                    {t("head_controls.group_music")}
                  </p>
                  <div className="flex items-center justify-between gap-2 text-xs">
                    <span className="text-[#4D4D4D] dark:text-white/75">
                      {t("head_controls.gesture_tilt_left")}
                    </span>
                    <select
                      disabled={disabled}
                      value={headState.tiltLeftAction}
                      onChange={(e) =>
                        onHeadControlsChange?.({
                          ...headState,
                          tiltLeftAction: Number(e.target.value) as 3 | 4,
                        })
                      }
                      className="rounded-lg border border-[#E5E5E5] bg-[#F2F1F6] px-2 py-1 text-xs font-semibold dark:border-white/12 dark:bg-[#121212] dark:text-white"
                    >
                      <option value={3}>{t("head_controls.action_prev_track")}</option>
                      <option value={4}>{t("head_controls.action_next_track")}</option>
                    </select>
                  </div>
                  <div className="flex items-center justify-between gap-2 text-xs">
                    <span className="text-[#4D4D4D] dark:text-white/75">
                      {t("head_controls.gesture_tilt_right")}
                    </span>
                    <select
                      disabled={disabled}
                      value={headState.tiltRightAction}
                      onChange={(e) =>
                        onHeadControlsChange?.({
                          ...headState,
                          tiltRightAction: Number(e.target.value) as 3 | 4,
                        })
                      }
                      className="rounded-lg border border-[#E5E5E5] bg-[#F2F1F6] px-2 py-1 text-xs font-semibold dark:border-white/12 dark:bg-[#121212] dark:text-white"
                    >
                      <option value={3}>{t("head_controls.action_prev_track")}</option>
                      <option value={4}>{t("head_controls.action_next_track")}</option>
                    </select>
                  </div>
                </div>
              </div>

              {/* Interactive Head Gesture Practice Guide */}
              <div className="flex flex-wrap items-center justify-between gap-2 rounded-xl border border-[#727AFF]/30 bg-[#727AFF]/10 px-3 py-2 text-xs">
                {practiceStep === null ? (
                  <>
                    <span className="font-semibold text-[#4D4D4D] dark:text-white/80">
                      {t("head_controls.practice_title")}
                    </span>
                    <button
                      type="button"
                      onClick={() => setPracticeStep("nod")}
                      className="rounded-lg bg-[#727AFF] px-3 py-1 font-semibold text-white hover:brightness-110"
                    >
                      {t("head_controls.practice_start")}
                    </button>
                  </>
                ) : practiceStep === "done" ? (
                  <>
                    <span className="inline-flex items-center gap-1.5 font-semibold text-[#16A34A] dark:text-[#4ADE80]">
                      <CheckCircle2 className="h-4 w-4" />
                      {t("head_controls.practice_complete")}
                    </span>
                    <button
                      type="button"
                      onClick={() => setPracticeStep(null)}
                      className="rounded-lg bg-white px-2.5 py-1 font-semibold text-[#4D4D4D] dark:bg-white/10 dark:text-white"
                    >
                      {t("common.done")}
                    </button>
                  </>
                ) : (
                  <>
                    <span className="font-semibold text-[#1A1A1A] dark:text-white">
                      {practiceStep === "nod"
                        ? t("head_controls.practice_nod_prompt")
                        : practiceStep === "shake"
                          ? t("head_controls.practice_shake_prompt")
                          : t("head_controls.practice_tilt_prompt")}
                    </span>
                    <button
                      type="button"
                      onClick={() =>
                        setPracticeStep(
                          practiceStep === "nod"
                            ? "shake"
                            : practiceStep === "shake"
                              ? "tilt"
                              : "done"
                        )
                      }
                      className="rounded-lg bg-[#FF7A2F] px-3 py-1 font-semibold text-white"
                    >
                      {t("common.next")}
                    </button>
                  </>
                )}
              </div>
            </div>
          )}
        </div>
      )}
    </section>
  );
};
