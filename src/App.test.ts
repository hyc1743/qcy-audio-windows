import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import App, { detectWindowModeFromLocation, resolveTheme, type NavTabId } from "./App";
import { QcyIpcClient } from "./ipc/client";
import { isLikelyQcyDevice } from "./pages/DevicesOtaPage";

describe("QCY Audio Desktop — App Shell & Multi-Window Integration", () => {
  it("detects popup vs main window mode from query string and hash", () => {
    expect(detectWindowModeFromLocation("", "")).toBe("main");
    expect(detectWindowModeFromLocation("?window=popup", "")).toBe("popup");
    expect(detectWindowModeFromLocation("?foo=1&window=POPUP", "")).toBe("popup");
    expect(detectWindowModeFromLocation("", "#popup")).toBe("popup");
    expect(detectWindowModeFromLocation("", "#/popup")).toBe("popup");
    expect(detectWindowModeFromLocation("", "#dashboard")).toBe("main");
  });

  it("identifies QCY Bluetooth devices by company ID, OUI prefix, service UUID, and name", () => {
    expect(
      isLikelyQcyDevice({
        id: "1",
        address: "84:AC:60:11:22:33",
        name: "Unknown Audio",
        rssi: -55,
        transport_kind: "ble",
        manufacturer_data: {},
        service_uuids: [],
        is_paired: false,
        is_connected: false,
        last_seen_ms: 0,
      })
    ).toBe(true);
    expect(
      isLikelyQcyDevice({
        id: "2",
        address: "11:22:33:44:55:66",
        name: "QCY MeloBuds Pro",
        rssi: -60,
        transport_kind: "dual",
        manufacturer_data: {},
        service_uuids: [],
        is_paired: true,
        is_connected: false,
        last_seen_ms: 0,
      })
    ).toBe(true);
    expect(
      isLikelyQcyDevice({
        id: "3",
        address: "AA:BB:CC:DD:EE:FF",
        name: "Generic Keyboard",
        rssi: -70,
        transport_kind: "ble",
        manufacturer_data: {},
        service_uuids: [],
        is_paired: true,
        is_connected: false,
        last_seen_ms: 0,
      })
    ).toBe(false);
  });

  it("resolves system theme against matchMedia prefers-color-scheme", () => {
    expect(resolveTheme("light")).toBe("light");
    expect(resolveTheme("dark")).toBe("dark");

    const origWindow = globalThis.window;
    try {
      Object.defineProperty(globalThis, "window", {
        configurable: true,
        value: { matchMedia: () => ({ matches: false }) },
      });
      expect(resolveTheme("system")).toBe("light");

      Object.defineProperty(globalThis, "window", {
        configurable: true,
        value: { matchMedia: () => ({ matches: true }) },
      });
      expect(resolveTheme("system")).toBe("dark");
    } finally {
      Object.defineProperty(globalThis, "window", {
        configurable: true,
        value: origWindow,
      });
    }
  });

  it("renders main desktop shell cleanly in both zh-CN and en-US with clean disconnected default", () => {
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    try {
      const client = new QcyIpcClient({ forceMock: true });

      const zhHtml = renderToStaticMarkup(
        React.createElement(App, {
          windowMode: "main",
          initialLocale: "zh-CN",
          initialTab: "dashboard",
          client,
        })
      );
      expect(zhHtml).toContain('data-window-mode="main"');
      expect(zhHtml).toContain("QCY Audio");
      expect(zhHtml).toContain("设备主页");
      expect(zhHtml).toContain("未连接设备");
      expect(zhHtml).not.toContain("QCY MeloBuds N70");
      expect(zhHtml).not.toContain("虚拟模拟器");

      const enHtml = renderToStaticMarkup(
        React.createElement(App, {
          windowMode: "main",
          initialLocale: "en-US",
          initialTab: "dashboard",
          client,
        })
      );
      expect(enHtml).toContain('data-window-mode="main"');
      expect(enHtml).toContain("Dashboard");
      expect(enHtml).toContain("No Device Connected");
      expect(enHtml).not.toContain("QCY MeloBuds N70");
      expect(enHtml).not.toContain(">Simulator<");

      expect(errorSpy).not.toHaveBeenCalled();
    } finally {
      errorSpy.mockRestore();
    }
  });

  it("renders frameless popup container without fake device data before a popup event arrives", () => {
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    try {
      const html = renderToStaticMarkup(
        React.createElement(App, {
          windowMode: "popup",
          initialLocale: "en-US",
          client: new QcyIpcClient({ forceMock: true }),
        })
      );
      expect(html).toContain('data-window-mode="popup"');
      expect(html).not.toContain("QCY MeloBuds N70");
      expect(errorSpy).not.toHaveBeenCalled();
    } finally {
      errorSpy.mockRestore();
    }
  });

  it("renders every routed desktop tab without throwing", () => {
    const tabs: NavTabId[] = [
      "dashboard",
      "eq",
      "gestures",
      "modes",
      "focus",
      "find",
      "devices",
      "app_settings",
    ];
    const client = new QcyIpcClient({ forceMock: true });
    for (const tab of tabs) {
      const html = renderToStaticMarkup(
        React.createElement(App, {
          windowMode: "main",
          initialLocale: "en-US",
          initialTab: tab,
          client,
        })
      );
      expect(html).toContain('data-window-mode="main"');
      expect(html).toContain('aria-current="page"');
    }
  });
});
