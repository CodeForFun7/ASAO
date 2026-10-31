import React from "react";
import type { BloatCategory, BloatSafety, ImpactBand } from "../../types/bloat";
import { CATEGORY_LABELS, IMPACT_LABELS, SAFETY_HELP, SAFETY_LABELS } from "../../types/bloat";

const SAFETY_STYLE: Record<BloatSafety, string> = {
  safe: "bg-lunar-healthy/10 text-lunar-healthy border-lunar-healthy/30",
  "usually-safe": "bg-lunar-healthy/5 text-lunar-healthy border-lunar-healthy/20",
  caution: "bg-lunar-warning/10 text-lunar-warning border-lunar-warning/30",
  "not-recommended": "bg-lunar-critical/10 text-lunar-critical border-lunar-critical/30",
  protected: "bg-lunar-ai/10 text-lunar-ai border-lunar-ai/30",
  "not-rated": "bg-lunar-bg text-lunar-text-sec border-lunar-border",
};

/** Safety level as text plus colour, never colour alone. */
export const SafetyBadge: React.FC<{ safety: BloatSafety }> = ({ safety }) => (
  <span
    title={SAFETY_HELP[safety]}
    className={`inline-flex items-center font-mono text-[10px] px-1.5 py-0.5 rounded border cursor-help shrink-0 ${SAFETY_STYLE[safety]}`}
  >
    {SAFETY_LABELS[safety]}
  </span>
);

const IMPACT_STYLE: Record<ImpactBand, string> = {
  high: "text-lunar-critical",
  medium: "text-lunar-warning",
  low: "text-lunar-text-sec",
};

export const ImpactScore: React.FC<{ total: number; band: ImpactBand }> = ({ total, band }) => (
  <div className="text-right" title={`${IMPACT_LABELS[band]}: ${total} of 100 points`}>
    <div className={`text-xs font-mono font-semibold ${IMPACT_STYLE[band]}`}>{total}</div>
    <div className="text-[10px] text-lunar-muted">{IMPACT_LABELS[band]}</div>
  </div>
);

export const CategoryChip: React.FC<{ category: BloatCategory }> = ({ category }) => (
  <span className="inline-flex items-center font-mono text-[10px] px-1.5 py-0.5 rounded bg-lunar-bg text-lunar-text-sec border border-lunar-border shrink-0">
    {CATEGORY_LABELS[category]}
  </span>
);
