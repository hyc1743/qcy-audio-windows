import { describe, expect, it } from "vitest";
import enUS from "./en-US.json";
import zhCN from "./zh-CN.json";
import { interpolate, normalizeLocale, translate } from "./index";

function flattenLeafStrings(
  obj: Record<string, unknown>,
  prefix = ""
): Record<string, string> {
  const result: Record<string, string> = {};
  for (const [key, value] of Object.entries(obj)) {
    const fullKey = prefix ? `${prefix}.${key}` : key;
    if (typeof value === "string") {
      result[fullKey] = value;
    } else if (value !== null && typeof value === "object" && !Array.isArray(value)) {
      Object.assign(result, flattenLeafStrings(value as Record<string, unknown>, fullKey));
    } else {
      throw new Error(`Unexpected non-string/non-object value at i18n key: ${fullKey}`);
    }
  }
  return result;
}

function extractPlaceholders(template: string): string[] {
  const matches = template.match(/\{([a-zA-Z0-9_]+)\}/g) ?? [];
  return [...new Set(matches)].sort();
}

describe("i18n dictionaries & runtime helper", () => {
  const enLeaves = flattenLeafStrings(enUS as Record<string, unknown>);
  const zhLeaves = flattenLeafStrings(zhCN as Record<string, unknown>);

  it("maintains 100% recursive key parity between zh-CN.json and en-US.json", () => {
    const enKeys = Object.keys(enLeaves).sort();
    const zhKeys = Object.keys(zhLeaves).sort();
    expect(enKeys.length).toBeGreaterThan(500);
    expect(zhKeys).toEqual(enKeys);
  });

  it("maintains 100% {placeholder} parity and zero empty values across every key", () => {
    for (const [key, enVal] of Object.entries(enLeaves)) {
      const zhVal = zhLeaves[key];
      expect(enVal.trim().length, `en-US empty key: ${key}`).toBeGreaterThan(0);
      expect(zhVal.trim().length, `zh-CN empty key: ${key}`).toBeGreaterThan(0);
      expect(
        extractPlaceholders(zhVal),
        `placeholder mismatch at key: ${key}`
      ).toEqual(extractPlaceholders(enVal));
    }
  });

  it("translates keys, interpolates placeholders, and falls back gracefully", () => {
    expect(translate("en-US", "app.name")).toBe("QCY Audio");
    expect(translate("zh-CN", "app.name")).toBe("QCY Audio");
    expect(translate("en-US", "common.minutes", { count: 30 })).toBe("30 min");
    expect(translate("zh-CN", "common.minutes", { count: 30 })).toContain("30");
    expect(translate("en-US", "nonexistent.key.path")).toBe("nonexistent.key.path");
    expect(interpolate("Hello {user}, battery {pct}%", { user: "QCY", pct: 95 })).toBe(
      "Hello QCY, battery 95%"
    );
    expect(normalizeLocale("zh-Hans")).toBe("zh-CN");
    expect(normalizeLocale("en-GB")).toBe("en-US");
  });
});
