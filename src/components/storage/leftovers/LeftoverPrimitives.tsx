import React from "react";
import { Check, Minus } from "lucide-react";
import type { ResidualCategory, ResidualConfidence } from "../../../types/residual";
import { CATEGORY_LABELS, CONFIDENCE_LABELS } from "../../../types/residual";

export type CheckState = "checked" | "unchecked" | "mixed";

interface CheckboxProps {
  state: CheckState;
  disabled?: boolean;
  label: string;
  onToggle: () => void;
}

/** Small checkbox in the same white-on-dark style as ASAO's toggles. */
export const LeftoverCheckbox: React.FC<CheckboxProps> = ({ state, disabled, label, onToggle }) => (
  <button
    type="button"
    role="checkbox"
    aria-checked={state === "mixed" ? "mixed" : state === "checked"}
    aria-label={label}
    disabled={disabled}
    onClick={(e) => {
      e.stopPropagation();
      onToggle();
    }}
    className={`w-4 h-4 rounded border flex items-center justify-center shrink-0 transition-colors ${
      disabled ? "opacity-45 cursor-not-allowed" : "cursor-pointer"
    } ${
      state === "unchecked"
        ? "bg-lunar-bg border-lunar-border hover:border-lunar-text-sec"
        : "bg-lunar-white border-lunar-white text-lunar-bg"
    }`}
  >
    {state === "checked" && <Check className="w-3 h-3 stroke-[3]" />}
    {state === "mixed" && <Minus className="w-3 h-3 stroke-[3]" />}
  </button>
);

const CONFIDENCE_STYLE: Record<ResidualConfidence, string> = {
  high: "bg-lunar-healthy/10 text-lunar-healthy border-lunar-healthy/30",
  medium: "bg-lunar-warning/10 text-lunar-warning border-lunar-warning/30",
  low: "bg-lunar-bg text-lunar-text-sec border-lunar-border",
};

const CONFIDENCE_HELP: Record<ResidualConfidence, string> = {
  high: "High confidence: matches an app that was uninstalled from this PC",
  medium: "Medium confidence: likely a leftover, review before removing",
  low: "Uncertain: no uninstalled app matches this folder",
};

export const ConfidenceBadge: React.FC<{ confidence: ResidualConfidence }> = ({ confidence }) => (
  <span
    title={CONFIDENCE_HELP[confidence]}
    className={`inline-flex items-center font-mono text-[10px] px-1.5 py-0.5 rounded border cursor-help shrink-0 ${CONFIDENCE_STYLE[confidence]}`}
  >
    {CONFIDENCE_LABELS[confidence]}
  </span>
);

export const CategoryLabel: React.FC<{ category: ResidualCategory }> = ({ category }) => (
  <span className="inline-flex items-center font-mono text-[10px] px-1.5 py-0.5 rounded bg-lunar-bg text-lunar-text-sec border border-lunar-border shrink-0">
    {CATEGORY_LABELS[category]}
  </span>
);

export function checkStateFor(selectedCount: number, total: number): CheckState {
  if (selectedCount === 0) return "unchecked";
  if (selectedCount === total) return "checked";
  return "mixed";
}
