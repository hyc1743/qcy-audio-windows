import React, { useEffect } from "react";
import { AlertTriangle, RefreshCw, ShieldAlert, X } from "lucide-react";
import { TranslateFn } from "./shared";

export type RuleCompareOp = "eq" | "ne" | "gt" | "gte" | "lt" | "lte";

export interface PopViewRuleConflict {
  feature: string;
  cmd_id: number | null;
  compare: RuleCompareOp;
  value: number | null;
  reboot: boolean;
}

export interface PopViewRule {
  target_feature: string;
  target_cmd_id: number | null;
  target_value: number | null;
  action: "confirm_dialog" | "toast";
  requires_reboot: boolean;
  title_key: string | null;
  message_key: string;
  reboot_notice_key: string | null;
  conflicts_with: PopViewRuleConflict[];
}

export interface EvaluatedConflict {
  rule: PopViewRule;
  triggeredConflicts: PopViewRuleConflict[];
  requiresReboot: boolean;
}

function compareValues(actual: number, op: RuleCompareOp, expected: number): boolean {
  switch (op) {
    case "eq":
      return actual === expected;
    case "ne":
      return actual !== expected;
    case "gt":
      return actual > expected;
    case "gte":
      return actual >= expected;
    case "lt":
      return actual < expected;
    case "lte":
      return actual <= expected;
  }
}

/**
 * Pure deterministic evaluator matching QCY `popViewRules` against current feature state values.
 * Returns the first triggered mutual-exclusion or reboot rule, or `null` if safe to apply directly.
 */
export function evaluateConflictRules(
  rules: readonly PopViewRule[],
  targetFeature: string,
  targetValue: number,
  currentFeatureValues: Readonly<Record<string, number>>
): EvaluatedConflict | null {
  for (const rule of rules) {
    if (rule.target_feature !== targetFeature) continue;
    if (rule.target_value !== null && rule.target_value !== targetValue) continue;

    if (rule.conflicts_with.length === 0) {
      if (rule.requires_reboot) {
        return {
          rule,
          triggeredConflicts: [],
          requiresReboot: true,
        };
      }
      continue;
    }

    const triggered = rule.conflicts_with.filter((c) => {
      if (c.value === null) return false;
      const actual = currentFeatureValues[c.feature] ?? 0;
      return compareValues(actual, c.compare, c.value);
    });

    if (triggered.length > 0) {
      return {
        rule,
        triggeredConflicts: triggered,
        requiresReboot:
          rule.requires_reboot || triggered.some((c) => c.reboot),
      };
    }
  }
  return null;
}

export interface MutualExclusionDialogProps {
  open: boolean;
  conflict: EvaluatedConflict | null;
  onConfirm: (conflict: EvaluatedConflict) => void;
  onCancel: () => void;
  t: TranslateFn;
}

export const MutualExclusionDialog: React.FC<MutualExclusionDialogProps> = ({
  open,
  conflict,
  onConfirm,
  onCancel,
  t,
}) => {
  useEffect(() => {
    if (!open) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onCancel();
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [open, onCancel]);

  if (!open || !conflict) return null;

  const { rule, triggeredConflicts, requiresReboot } = conflict;
  const isToastBlock = rule.action === "toast";

  const targetLabel = t(`rules.feature_names.${rule.target_feature}`);
  const conflictLabels = triggeredConflicts.map((c) =>
    t(`rules.feature_names.${c.feature}`)
  );
  const titleText = rule.title_key
    ? t(rule.title_key)
    : t("rules.conflict_title");
  const messageText = t(rule.message_key, {
    target: targetLabel,
    conflicts: conflictLabels.join(", "),
  });

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4 backdrop-blur-xs"
      role="dialog"
      aria-modal="true"
      aria-labelledby="qcy-conflict-title"
      onClick={onCancel}
    >
      <div
        className="w-full max-w-md rounded-2xl border border-[#E5E5E5] bg-white p-6 shadow-2xl dark:border-white/12 dark:bg-[#1E1E1E]"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-start justify-between gap-3">
          <div className="flex items-center gap-3">
            <span
              className={`flex h-10 w-10 shrink-0 items-center justify-center rounded-xl ${
                isToastBlock
                  ? "bg-[#FF4C4C]/14 text-[#FF4C4C]"
                  : "bg-[#FF7A2F]/15 text-[#FF7A2F]"
              }`}
            >
              {isToastBlock ? (
                <ShieldAlert className="h-5 w-5" />
              ) : (
                <AlertTriangle className="h-5 w-5" />
              )}
            </span>
            <h3
              id="qcy-conflict-title"
              className="text-base font-bold text-[#1A1A1A] dark:text-white"
            >
              {titleText}
            </h3>
          </div>
          <button
            type="button"
            onClick={onCancel}
            className="inline-flex h-8 w-8 items-center justify-center rounded-lg text-[#808080] hover:bg-black/5 dark:text-white/60 dark:hover:bg-white/10"
            aria-label={t("common.close")}
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <p className="mt-4 text-sm leading-relaxed text-[#4D4D4D] dark:text-white/80">
          {messageText}
        </p>

        {/* Conflicting feature chips */}
        {conflictLabels.length > 0 && !isToastBlock && (
          <div className="mt-3 flex flex-wrap gap-1.5">
            {conflictLabels.map((label) => (
              <span
                key={label}
                className="inline-flex items-center rounded-full border border-[#FF7A2F]/35 bg-[#FF7A2F]/10 px-2.5 py-0.5 text-xs font-semibold text-[#C2410C] dark:text-[#FF9E66]"
              >
                {label}
              </span>
            ))}
          </div>
        )}

        {/* Earbuds Reboot Warning Callout */}
        {requiresReboot && !isToastBlock && (
          <div className="mt-4 flex items-start gap-2.5 rounded-xl border border-[#EAB308]/35 bg-[#EAB308]/10 p-3 text-xs text-[#854D0E] dark:text-[#FDE047]">
            <RefreshCw className="mt-0.5 h-4 w-4 shrink-0" />
            <span>
              {rule.reboot_notice_key
                ? t(rule.reboot_notice_key)
                : t("rules.conflict_will_reboot")}
            </span>
          </div>
        )}

        {/* Actions */}
        <div className="mt-6 flex items-center justify-end gap-2.5">
          {isToastBlock ? (
            <button
              type="button"
              onClick={onCancel}
              className="rounded-xl bg-[#FF7A2F] px-4 py-2 text-sm font-semibold text-white transition hover:bg-[#E8691F]"
            >
              {t("common.confirm")}
            </button>
          ) : (
            <>
              <button
                type="button"
                onClick={onCancel}
                className="rounded-xl border border-[#E5E5E5] bg-transparent px-4 py-2 text-sm font-semibold text-[#4D4D4D] transition hover:bg-black/5 dark:border-white/12 dark:text-white/80 dark:hover:bg-white/10"
              >
                {t("common.cancel")}
              </button>
              <button
                type="button"
                onClick={() => onConfirm(conflict)}
                className="inline-flex items-center gap-1.5 rounded-xl bg-[#FF7A2F] px-4 py-2 text-sm font-semibold text-white shadow-xs transition hover:bg-[#E8691F]"
              >
                {requiresReboot && <RefreshCw className="h-3.5 w-3.5" />}
                {requiresReboot
                  ? t("rules.reboot_confirm")
                  : t("rules.conflict_confirm_enable")}
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
};
