import React, { createContext, useCallback, useContext, useMemo, useState } from "react";
import enUS from "./en-US.json";
import zhCN from "./zh-CN.json";

export type SupportedLocale = "zh-CN" | "en-US";
export type TranslationParams = Record<string, string | number>;
export type TranslateFn = (key: string, params?: TranslationParams) => string;

type DictNode = string | { [key: string]: DictNode };

const DICTIONARIES: Record<SupportedLocale, DictNode> = {
  "en-US": enUS as DictNode,
  "zh-CN": zhCN as DictNode,
};

function resolvePath(root: DictNode, key: string): string | undefined {
  const parts = key.split(".");
  let current: DictNode | undefined = root;
  for (const part of parts) {
    if (current === undefined || typeof current === "string") {
      return undefined;
    }
    current = current[part];
  }
  return typeof current === "string" ? current : undefined;
}

/**
 * Interpolates `{placeholder}` tokens within a localized template string.
 */
export function interpolate(template: string, params?: TranslationParams): string {
  if (!params) return template;
  return template.replace(/\{([a-zA-Z0-9_]+)\}/g, (match, name: string) => {
    const val = params[name];
    return val !== undefined ? String(val) : match;
  });
}

/**
 * Resolves a dot-delimited translation key for the given locale with fallback to `en-US`,
 * then raw key name. Never throws.
 */
export function translate(
  locale: SupportedLocale,
  key: string,
  params?: TranslationParams
): string {
  const dict = DICTIONARIES[locale] ?? DICTIONARIES["en-US"];
  const found = resolvePath(dict, key) ?? resolvePath(DICTIONARIES["en-US"], key) ?? key;
  return interpolate(found, params);
}

/**
 * Detects the best matching default locale from the browser / Windows WebView2 navigator.
 */
export function detectSystemLocale(): SupportedLocale {
  if (typeof navigator !== "undefined" && typeof navigator.language === "string") {
    if (navigator.language.toLowerCase().startsWith("zh")) {
      return "zh-CN";
    }
  }
  return "en-US";
}

export function normalizeLocale(raw?: string | null): SupportedLocale {
  if (!raw) return detectSystemLocale();
  const lower = raw.trim().toLowerCase();
  if (lower === "zh" || lower.startsWith("zh-")) return "zh-CN";
  return "en-US";
}

export interface I18nContextValue {
  locale: SupportedLocale;
  setLocale: (locale: SupportedLocale) => void;
  t: TranslateFn;
}

const I18nContext = createContext<I18nContextValue>({
  locale: "zh-CN",
  setLocale: () => {},
  t: (key, params) => translate("zh-CN", key, params),
});

export interface I18nProviderProps {
  initialLocale?: SupportedLocale;
  locale?: SupportedLocale;
  onLocaleChange?: (locale: SupportedLocale) => void;
  children: React.ReactNode;
}

export const I18nProvider: React.FC<I18nProviderProps> = ({
  initialLocale,
  locale: controlledLocale,
  onLocaleChange,
  children,
}) => {
  const [internalLocale, setInternalLocale] = useState<SupportedLocale>(
    () => initialLocale ?? detectSystemLocale()
  );

  const activeLocale = controlledLocale ?? internalLocale;

  const setLocale = useCallback(
    (next: SupportedLocale) => {
      setInternalLocale(next);
      onLocaleChange?.(next);
    },
    [onLocaleChange]
  );

  const t = useCallback<TranslateFn>(
    (key, params) => translate(activeLocale, key, params),
    [activeLocale]
  );

  const value = useMemo<I18nContextValue>(
    () => ({ locale: activeLocale, setLocale, t }),
    [activeLocale, setLocale, t]
  );

  return React.createElement(I18nContext.Provider, { value }, children);
};

export function useI18n(): I18nContextValue {
  return useContext(I18nContext);
}

export { enUS, zhCN };
