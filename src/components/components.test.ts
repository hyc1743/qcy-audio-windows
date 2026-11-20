import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import enUs from "../i18n/en-US.json";
import zhCn from "../i18n/zh-CN.json";
import {
  AncControlPanel,
  BatteryOverviewCard,
  DEFAULT_AI_ANC_MATRIX,
  GestureRemapper,
  HearingTestModal,
  MutualExclusionDialog,
  ParametricEqCanvas,
  PopViewRule,
  RgbLightingControl,
  SoundscapePanel,
  TOUCH_SENSITIVITY_WIRE,
  dbToSvgY,
  evaluateConflictRules,
  formatCountdown,
  formatWithParams,
  freqToSvgX,
  getBatteryAccentColor,
  getEventSlotsForKey,
  parseAutoEqText,
  svgXToFreq,
  svgYToDb,
  toggleAncCycleOption,
} from "./index";

function createTranslator(dict: Record<string, unknown>) {
  return (key: string, params?: Record<string, string | number>): string => {
    const parts = key.split(".");
    let cur: unknown = dict;
    for (const p of parts) {
      if (cur && typeof cur === "object" && p in (cur as Record<string, unknown>)) {
        cur = (cur as Record<string, unknown>)[p];
      } else {
        return key;
      }
    }
    return typeof cur === "string" ? formatWithParams(cur, params) : key;
  };
}

const tEn = createTranslator(enUs as Record<string, unknown>);
const tZh = createTranslator(zhCn as Record<string, unknown>);
const escapeHtml = (s: string) => s.replace(/&/g, "&amp;");

describe("QCY UI Components — pure logic & helpers", () => {
  it("maps battery accent colors at low/warning/healthy thresholds", () => {
    expect(getBatteryAccentColor(null)).toBe("#9CA3AF");
    expect(getBatteryAccentColor(10)).toBe("#FF4C4C");
    expect(getBatteryAccentColor(25)).toBe("#EAB308");
    expect(getBatteryAccentColor(85)).toBe("#22C55E");
  });

  it("enforces ANC button cycle minimum of 2 active modes", () => {
    const blocked = toggleAncCycleOption(["anc_adaptive", "off"], "off");
    expect(blocked.blockedMinTwo).toBe(true);
    expect(blocked.next).toEqual(["anc_adaptive", "off"]);

    const added = toggleAncCycleOption(["anc_adaptive", "off"], "transparency");
    expect(added.blockedMinTwo).toBe(false);
    expect(added.next).toEqual(["anc_adaptive", "off", "transparency"]);

    const removed = toggleAncCycleOption(added.next, "off");
    expect(removed.blockedMinTwo).toBe(false);
    expect(removed.next).toEqual(["anc_adaptive", "transparency"]);
  });

  it("round-trips logarithmic frequency & dB SVG coordinates accurately", () => {
    for (const freq of [20, 100, 1000, 8000, 20000]) {
      const roundTripped = svgXToFreq(freqToSvgX(freq));
      expect(Math.abs(roundTripped - freq)).toBeLessThanOrEqual(Math.max(1, freq * 0.01));
    }
    for (const db of [-12, -6, 0, 3.5, 12]) {
      expect(svgYToDb(dbToSvgY(db))).toBeCloseTo(db, 1);
    }
  });

  it("parses AutoEQ ParametricEQ .txt files cleanly", () => {
    const sample = [
      "Preamp: -4.2 dB",
      "Filter 1: ON LSC Fc 105 Hz Gain 3.5 dB Q 0.71",
      "Filter 2: ON PK Fc 2400 Hz Gain -2.1 dB Q 2.40",
      "Filter 3: OFF HSC Fc 9500 Hz Gain 1.5 dB Q 0.80",
    ].join("\n");
    const parsed = parseAutoEqText(sample);
    expect(parsed).not.toBeNull();
    expect(parsed!.masterGainDb).toBeCloseTo(-4.2, 2);
    expect(parsed!.bands).toHaveLength(3);
    expect(parsed!.bands[0]).toMatchObject({ frequencyHz: 105, gainDb: 3.5, type: "low_shelf", enabled: true });
    expect(parsed!.bands[1]).toMatchObject({ frequencyHz: 2400, gainDb: -2.1, type: "peaking", enabled: true });
    expect(parsed!.bands[2]).toMatchObject({ frequencyHz: 9500, type: "high_shelf", enabled: false });
  });

  it("evaluates popViewRules mutual exclusion and reboot rules deterministically", () => {
    const rules: PopViewRule[] = [
      {
        target_feature: "ldac",
        target_cmd_id: 35,
        target_value: 1,
        action: "confirm_dialog",
        requires_reboot: true,
        title_key: "rules.conflict_title",
        message_key: "rules.conflict_disable_features",
        reboot_notice_key: "rules.conflict_will_reboot",
        conflicts_with: [
          { feature: "dual_device", cmd_id: 36, compare: "eq", value: 1, reboot: true },
          { feature: "spatial_audio", cmd_id: 91, compare: "ne", value: 1, reboot: true },
        ],
      },
      {
        target_feature: "dynamic_eq",
        target_cmd_id: 39,
        target_value: 1,
        action: "toast",
        requires_reboot: false,
        title_key: null,
        message_key: "rules.require_default_eq",
        reboot_notice_key: null,
        conflicts_with: [
          { feature: "eq", cmd_id: 34, compare: "ne", value: 1, reboot: false },
        ],
      },
    ];

    // No conflict when dual_device=0 and spatial_audio=1 (Off)
    expect(evaluateConflictRules(rules, "ldac", 1, { dual_device: 0, spatial_audio: 1 })).toBeNull();

    // Conflict triggered when dual_device=1
    const hit = evaluateConflictRules(rules, "ldac", 1, { dual_device: 1, spatial_audio: 1 });
    expect(hit).not.toBeNull();
    expect(hit!.requiresReboot).toBe(true);
    expect(hit!.triggeredConflicts.map((c) => c.feature)).toEqual(["dual_device"]);

    // Toast rule when eq != 1
    const eqBlock = evaluateConflictRules(rules, "dynamic_eq", 1, { eq: 3 });
    expect(eqBlock?.rule.action).toBe("toast");
    expect(eqBlock?.rule.message_key).toBe("rules.require_default_eq");
  });

  it("exposes exact QCY touch sensitivity wire bytes and key slots", () => {
    expect(TOUCH_SENSITIVITY_WIRE).toEqual({ moderate: 38, normal: 45, sensitive: 52 });
    expect(getEventSlotsForKey("left", { 1: 6, 3: 1 })).toHaveLength(5);
    expect(getEventSlotsForKey("mfb", {})).toHaveLength(4);
    expect(formatCountdown(95)).toBe("01:35");
    expect(formatCountdown(3665)).toBe("1:01:05");
  });
});

describe("QCY UI Components — SSR bilingual smoke rendering", () => {
  it("renders all 8 components cleanly in English and Simplified Chinese", () => {
    for (const t of [tEn, tZh]) {
      const batteryHtml = renderToStaticMarkup(
        React.createElement(BatteryOverviewCard, {
          deviceName: "QCY MeloBuds N70",
          modelName: "QCY MeloBuds N70",
          modelId: "qcy-n70",
          formFactor: "tws",
          connectionMode: "virtual_simulator",
          rssi: -48,
          macAddress: "84:AC:60:11:22:33",
          firmwareVersion: "2.0.8",
          chipsetLabel: "WuQi",
          leftBattery: 88,
          rightBattery: 84,
          caseBattery: 65,
          canChangeColor: true,
          canRename: true,
          onColorChange: () => {},
          onRename: () => {},
          t,
        })
      );
      expect(batteryHtml).toContain("88%");
      expect(batteryHtml).toContain("84:AC:60:11:22:33");

      const ancHtml = renderToStaticMarkup(
        React.createElement(AncControlPanel, {
          capabilities: {
            anc: true,
            aiAnc: true,
            ancCustomLevels: 16,
            transparencyLevels: 6,
            vocalBoost: true,
            windNoise: true,
            ancButtonCycle: true,
          },
          state: {
            mode: "anc",
            scene: "adaptive",
            sceneDepth: 2,
            customDepth: 10,
            transparencyLevel: 4,
            vocalBoost: true,
            windNoiseReduction: true,
            aiAncEnabled: true,
            aiAncActivity: "walk",
            aiAncNoiseTier: "medium",
            aiAncMatrix: DEFAULT_AI_ANC_MATRIX,
            ancCycleModes: ["anc_adaptive", "off", "transparency"],
          },
          onModeChange: () => {},
          t,
        })
      );
      expect(ancHtml).toContain(t("anc.title"));
      expect(ancHtml).toContain(t("ai_anc.title"));

      const eqHtml = renderToStaticMarkup(
        React.createElement(ParametricEqCanvas, {
          capabilities: {
            parametricEq: true,
            customEq: true,
            maxEqBands: 20,
            personalizedEq: true,
            dynamicEq: true,
            thxEq: true,
            virtualBass: true,
            adaptiveVolume: true,
            channelBalance: true,
          },
          activePresetId: "default",
          graphicGainsDb: [0, 1, 2, 0, -1, 0, 1.5, 2, 1, 0],
          parametricBands: [
            { frequencyHz: 100, gainDb: 3, q: 1.0, type: "low_shelf", enabled: true },
            { frequencyHz: 3000, gainDb: 2, q: 1.4, type: "peaking", enabled: true },
          ],
          auxState: {
            dynamicEq: false,
            thxEq: false,
            virtualBass: true,
            adaptiveVolume: false,
            channelBalance: 50,
          },
          onSelectPreset: () => {},
          onGraphicGainsChange: () => {},
          onParametricBandsChange: () => {},
          onAuxChange: () => {},
          t,
        })
      );
      expect(eqHtml).toContain("<svg");
      expect(eqHtml).toContain(escapeHtml(t("eq.title")));

      const gestureHtml = renderToStaticMarkup(
        React.createElement(GestureRemapper, {
          capabilities: {
            buttonRemap: true,
            supportedButtonActions: [0, 1, 2, 3, 4, 5, 6, 7, 11],
            touchEnable: true,
            touchSensitivity: true,
            headControls: true,
          },
          touchEnabled: true,
          bindings: { 1: 6, 2: 5, 3: 1, 4: 1 },
          headControls: {
            enabled: true,
            nodAction: 1,
            shakeAction: 2,
            tiltLeftAction: 3,
            tiltRightAction: 4,
          },
          onBindingChange: () => {},
          t,
        })
      );
      expect(gestureHtml).toContain(escapeHtml(t("gestures.title")));
      expect(gestureHtml).toContain(escapeHtml(t("head_controls.title")));

      const rgbHtml = renderToStaticMarkup(
        React.createElement(RgbLightingControl, {
          state: {
            enabled: true,
            mode: "gradient_breathing",
            colorHex: "#FF7A2F",
            brightness: 80,
            speed: 55,
            twsSpeakerPairEnabled: false,
          },
          onChange: () => {},
          t,
        })
      );
      expect(rgbHtml).toContain(escapeHtml(t("lighting.title")));

      const hearingHtml = renderToStaticMarkup(
        React.createElement(HearingTestModal, {
          open: true,
          onApply: () => {},
          onClose: () => {},
          t,
        })
      );
      expect(hearingHtml).toContain(escapeHtml(t("hearing_test.title")));

      const soundscapeHtml = renderToStaticMarkup(
        React.createElement(SoundscapePanel, {
          supportsOnboardWhiteNoise: true,
          t,
        })
      );
      expect(soundscapeHtml).toContain(escapeHtml(t("focus_mode.title")));
      expect(soundscapeHtml).toContain(escapeHtml(t("focus_mode.channels.rain")));

      const conflictHtml = renderToStaticMarkup(
        React.createElement(MutualExclusionDialog, {
          open: true,
          conflict: {
            rule: {
              target_feature: "ldac",
              target_cmd_id: 35,
              target_value: 1,
              action: "confirm_dialog",
              requires_reboot: true,
              title_key: "rules.conflict_title",
              message_key: "rules.conflict_disable_features",
              reboot_notice_key: "rules.conflict_will_reboot",
              conflicts_with: [
                { feature: "dual_device", cmd_id: 36, compare: "eq", value: 1, reboot: true },
              ],
            },
            triggeredConflicts: [
              { feature: "dual_device", cmd_id: 36, compare: "eq", value: 1, reboot: true },
            ],
            requiresReboot: true,
          },
          onConfirm: () => {},
          onCancel: () => {},
          t,
        })
      );
      expect(conflictHtml).toContain(t("rules.conflict_title"));
      expect(conflictHtml).toContain(t("rules.feature_names.dual_device"));
    }
  });
});
