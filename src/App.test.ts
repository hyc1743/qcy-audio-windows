import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import App, { detectWindowModeFromLocation, resolveTheme, type NavTabId } from "./App";
import { QcyIpcClient } from "./ipc/client";

describe("QCY Audio Desktop — App Shell & Multi-Window Integration", () => {
  it("detects popup vs main window mode from query string and hash", () => {
    expect(detectWindowModeFromLocation("", "")).toBe("main");
    expect(detectWindowModeFromLocation("?window=popup", "")).toBe("popup");
    expect(detectWindowModeFromLocation("?foo=1&window=POPUP", "")).toBe("popup");
    expect(detectWindowModeFromLocation("", "#popup")).toBe("popup");
    expect(detectWindowModeFromLocation("", "#/popup")).toBe("popup");
    expect(detectWindowModeFromLocation("", "#dashboard")).toBe("main");
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

  it("renders main desktop shell cleanly in both zh-CN and en-US with zero console errors", () => {
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    try {
      const client = new QcyIpcClient();

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
      expect(zhHtml).toContain("音效与 EQ");
      expect(zhHtml).toContain("专注模式");
      expect(zhHtml).toContain("应用设置");

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
      expect(enHtml).toContain("Sound &amp; EQ");
      expect(enHtml).toContain("Focus Mode");
      expect(enHtml).toContain("App Settings");

      expect(errorSpy).not.toHaveBeenCalled();
    } finally {
      errorSpy.mockRestore();
    }
  });

  it("renders frameless Connection Pop-Up overlay cleanly in popup window mode", () => {
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    try {
      const html = renderToStaticMarkup(
        React.createElement(App, {
          windowMode: "popup",
          initialLocale: "en-US",
          client: new QcyIpcClient(),
        })
      );
      expect(html).toContain('data-window-mode="popup"');
      expect(html).toContain("Connected");
      expect(html).toContain("QCY MeloBuds N70");
      expect(html).toContain("Open Controls");
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
    const client = new QcyIpcClient();
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
