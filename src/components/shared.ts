/**
 * Self-contained shared types & lightweight helpers strictly internal to `src/components/**`.
 * Keeps all UI components decoupled and type-safe while backend/store modules evolve in parallel.
 */

export type TranslateFn = (
  key: string,
  params?: Record<string, string | number>
) => string;

export type UiTheme = "light" | "dark";

/** Safe fallback interpolation helper if a caller supplies a minimal translator. */
export function formatWithParams(
  template: string,
  params?: Record<string, string | number>
): string {
  if (!params) return template;
  return template.replace(/\{([a-zA-Z0-9_]+)\}/g, (match, token: string) =>
    token in params ? String(params[token]) : match
  );
}

/** Shared QCY design-system card classes (Light #FFFFFF / Dark #1E1E1E, Fluent-inspired). */
export const QCY_CARD_CLASS =
  "rounded-2xl border border-[#E5E5E5] bg-white p-5 shadow-xs transition-colors dark:border-white/10 dark:bg-[#1E1E1E]";

export const QCY_SUBCARD_CLASS =
  "rounded-xl border border-[#E5E5E5]/80 bg-[#F2F1F6] p-3.5 transition-colors dark:border-white/8 dark:bg-white/[0.04]";

/** Accessible toggle switch styling helper. */
export function toggleTrackClass(checked: boolean, accent: "orange" | "blue" = "orange"): string {
  if (!checked) {
    return "bg-[#D1D5DB] dark:bg-white/20";
  }
  return accent === "orange"
    ? "bg-[#FF7A2F]"
    : "bg-[#0066FF] dark:bg-[#3399FF]";
}

/** Format seconds into MM:SS or H:MM:SS for sleep timer display. */
export function formatCountdown(totalSeconds: number): string {
  const safe = Math.max(0, Math.floor(totalSeconds));
  const hours = Math.floor(safe / 3600);
  const mins = Math.floor((safe % 3600) / 60);
  const secs = safe % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  if (hours > 0) {
    return `${hours}:${pad(mins)}:${pad(secs)}`;
  }
  return `${pad(mins)}:${pad(secs)}`;
}
