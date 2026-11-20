import { describe, expect, it } from "vitest";
import { QcyIpcClient } from "./client";
import type { AppSnapshot, PopupTriggerPayload } from "../types/qcy";

describe("QcyIpcClient (in-memory fallback & IPC contract)", () => {
  it("provides initial virtual snapshot, models catalog, and emits live state updates", async () => {
    const client = new QcyIpcClient({ forceMock: true });
    expect(client.isTauri).toBe(false);

    const updates: AppSnapshot[] = [];
    const unlisten = await client.onStateUpdated((s) => updates.push(s));

    const snap = await client.getAppSnapshot();
    expect(snap.connection_state).toBe("connected");
    expect(snap.is_virtual_session).toBe(true);
    expect(snap.active_model.model_id).toBe("qcy-n70");
    expect(snap.anc.btn_anc_cfg).toEqual(["anc_adaptive", "transparency", "off"]);

    const models = await client.listModels();
    expect(models.length).toBeGreaterThanOrEqual(5);

    const ancRes = await client.setAncState({
      mode: "transparency",
      transparency_level: 6,
      vocal_boost: true,
    });
    expect(ancRes.applied).toBe(true);
    expect(ancRes.snapshot.anc.mode).toBe("transparency");
    expect(ancRes.snapshot.anc.transparency_level).toBe(6);
    expect(ancRes.snapshot.anc.vocal_boost).toBe(true);
    expect(updates.length).toBeGreaterThanOrEqual(1);

    unlisten();
  });

  it("enforces popViewRules mutual exclusion between LDAC and Dual-Device with force override", async () => {
    const client = new QcyIpcClient({ forceMock: true });

    // Enable Dual-Device first
    const dualRes = await client.setMultipoint({ dual_device: true });
    expect(dualRes.applied).toBe(true);
    expect(dualRes.snapshot.multipoint.dual_device).toBe(true);

    // Attempting to enable LDAC without force must return a conflict prompt
    const blocked = await client.setModes({ ldac: true });
    expect(blocked.applied).toBe(false);
    expect(blocked.conflict_prompt).toBeTruthy();
    expect(blocked.conflict_prompt?.conflicting_features).toContain("dual_device");
    expect(blocked.conflict_prompt?.requires_reboot).toBe(true);
    expect(blocked.snapshot.modes.ldac).toBe(false);

    // Re-running with force: true applies LDAC and disables Dual-Device
    const forced = await client.setModes({ ldac: true, force: true });
    expect(forced.applied).toBe(true);
    expect(forced.snapshot.modes.ldac).toBe(true);
    expect(forced.snapshot.multipoint.dual_device).toBe(false);
  });

  it("supports custom EQ CRUD, JSON export/import roundtrip, and personalized EQ", async () => {
    const client = new QcyIpcClient({ forceMock: true });

    await client.saveCustomEq({
      id: "studio-warm",
      eq_index: 99,
      category: "custom",
      name_en: "Studio Warm",
      name_zh: "温暖监听",
      master_gain_db: -1.5,
      gains_10band_db: [2, 1.5, 1, 0, -0.5, 0, 1, 1.5, 1, 0.5],
      parametric_bands: [{ freq_hz: 100, gain_db: 2.0, q: 1.2, irr_type: 2, enabled: true }],
    });

    const exported = await client.exportEqJson("studio-warm");
    expect(exported).toContain("Studio Warm");

    const afterDelete = await client.deleteCustomEq("studio-warm");
    expect(afterDelete.eq.custom_presets.find((p) => p.id === "studio-warm")).toBeUndefined();

    const afterImport = await client.importEqJson(exported);
    expect(afterImport.eq.custom_presets.find((p) => p.id === "studio-warm")).toBeDefined();

    const pers = await client.applyPersonalizedEq(
      [0, 0, 1, 1, 2, 3, 4, 4],
      [0, 0, 1, 2, 2, 3, 4, 5],
      [0, 0, 0, 1, 1, 2, 3, 4, 4, 4],
      0.1
    );
    expect(pers.applied).toBe(true);
    expect(pers.snapshot.eq.personalized_enabled).toBe(true);
    expect(pers.snapshot.eq.channel_balance).toBeCloseTo(0.1, 2);
  });

  it("switches virtual models, triggers popup events, and flags otaType=9 auth notice", async () => {
    const client = new QcyIpcClient({ forceMock: true });

    const popups: PopupTriggerPayload[] = [];
    const unlistenPopup = await client.onPopupTriggered((p) => popups.push(p));

    // Switch to SP75 Speaker & verify RGB controls + no case battery
    const sp75Snap = await client.setSimulatorModel("qcy-sp75");
    expect(sp75Snap.active_model.form_factor).toBe("speaker");
    expect(sp75Snap.battery.case_level).toBeNull();
    const rgb = await client.setRgbLighting({
      enabled: true,
      mode: "rhythm",
      color_hex: "#00FFFF",
      tws_pair: true,
    });
    expect(rgb.snapshot.rgb_lighting.mode).toBe("rhythm");
    expect(rgb.snapshot.rgb_lighting.tws_pair_enabled).toBe(true);

    const popup = await client.triggerSimulatorPopup();
    expect(popup.form_factor).toBe("speaker");
    expect(popups).toHaveLength(1);

    // Switch to H6 (ota_type = 9 JieLi v2) and confirm transparent auth-required notice
    await client.setSimulatorModel("qcy-h6");
    const otaH6 = await client.startLocalOta("h6_test.ota", new Uint8Array([0x51, 0x43, 0x59, 0x01]));
    expect(otaH6.phase).toBe("unsupported_auth_required");
    expect(otaH6.ota_type).toBe(9);

    // Switch back to N70 and verify byte-upload OTA succeeds & empty bytes fail validation
    await client.setSimulatorModel("qcy-n70");
    const otaEmpty = await client.startLocalOta("empty.bin", []);
    expect(otaEmpty.phase).toBe("error");
    const otaBytes = await client.startLocalOta("n70_v123.bin", [0xab, 0xcd, 0x65, 0x43]);
    expect(otaBytes.phase).toBe("completed");
    expect(otaBytes.file_path).toBe("n70_v123.bin");

    unlistenPopup();
  });
});
