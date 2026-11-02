import React, { useEffect } from "react";
import { Layers, Package, Power, RotateCcw, Server, ShieldCheck, Wrench, X } from "lucide-react";
import type { BloatItem, ImpactPart } from "../../types/bloat";
import { KIND_LABELS, SAFETY_LABELS } from "../../types/bloat";
import { formatStorageBytes } from "../../services/storage";
import { CategoryChip, SafetyBadge } from "./BloatPrimitives";
import { AskNyraButton } from "../common/AskNyraButton";
import { bloatPrompt } from "../../services/ask-nyra";

interface BloatDetailsProps {
  item: BloatItem;
  onClose: () => void;
}

const Section: React.FC<{ title: string; children: React.ReactNode }> = ({ title, children }) => (
  <section className="pt-5 first:pt-0 space-y-2">
    <h3 className="text-[11px] font-semibold uppercase tracking-wider text-lunar-muted">{title}</h3>
    {children}
  </section>
);

const ImpactRow: React.FC<{ label: string; part: ImpactPart }> = ({ label, part }) => (
  <div className="space-y-1">
    <div className="flex items-center justify-between text-[11px]">
      <span className="text-lunar-text-sec">{label}</span>
      <span className="font-mono text-lunar-white">
        {part.points}/{part.max}
      </span>
    </div>
    <div className="h-1.5 rounded-full bg-lunar-elevated overflow-hidden">
      <div className="h-full bg-lunar-white" style={{ width: `${part.max ? (part.points / part.max) * 100 : 0}%` }} />
    </div>
    <div className="text-[10px] text-lunar-muted leading-relaxed">{part.value}</div>
  </div>
);

function kindIcon(item: BloatItem) {
  switch (item.kind) {
    case "service":
      return <Server className="w-4 h-4 text-lunar-text-sec" />;
    case "startup-entry":
      return <Power className="w-4 h-4 text-lunar-text-sec" />;
    default:
      return <Package className="w-4 h-4 text-lunar-text-sec" />;
  }
}

export const BloatDetails: React.FC<BloatDetailsProps> = ({ item, onClose }) => {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const lastUsedDays = item.lastUsedMs ? Math.floor((Date.now() - item.lastUsedMs) / 86_400_000) : null;

  return (
    <aside
      className="w-96 bg-lunar-surface border-l border-lunar-border flex flex-col h-full shrink-0 overflow-hidden"
      aria-labelledby="bloat-detail-title"
    >
        <div className="p-4 border-b border-lunar-border flex items-start justify-between gap-3 bg-lunar-surface-2/50 shrink-0">
          <div className="flex items-center gap-3 min-w-0">
            <div className="w-8 h-8 rounded-lg bg-lunar-elevated border border-lunar-border flex items-center justify-center shrink-0">
              {kindIcon(item)}
            </div>
            <div className="min-w-0">
              <h2 id="bloat-detail-title" className="text-sm font-semibold text-lunar-white tracking-tight truncate" title={item.name}>
                {item.name}
              </h2>
              <div className="mt-1">
                <SafetyBadge safety={item.safety} />
              </div>
              <p className="text-[11px] text-lunar-text-sec truncate font-mono mt-0.5">
                {item.publisher ?? "Unknown publisher"} • {KIND_LABELS[item.kind]}
                {item.sizeBytes ? ` • ${formatStorageBytes(item.sizeBytes)}` : ""}
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            title="Close details (Esc)"
            className="p-1 rounded hover:bg-lunar-elevated text-lunar-muted hover:text-lunar-white transition-colors cursor-pointer shrink-0"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto p-4 space-y-5 divide-y divide-lunar-border/30">
          <Section title="What it is">
            <p className="text-xs text-lunar-text leading-relaxed">
              {item.whatItDoes ?? "ASAO has no description for it."}
            </p>
            <div className="flex items-center gap-2 flex-wrap">
              {item.category && <CategoryChip category={item.category} />}
              {item.running && <span className="text-[11px] text-lunar-healthy">Running now</span>}
              {lastUsedDays !== null && (
                <span className="text-[11px] text-lunar-text-sec">
                  Last used {lastUsedDays === 0 ? "today" : `${lastUsedDays} ${lastUsedDays === 1 ? "day" : "days"} ago`}
                </span>
              )}
            </div>
          </Section>

          <Section title={`Impact: ${item.impact.total} of 100`}>
            <div className="space-y-2">
              <ImpactRow label="Startup" part={item.impact.startup} />
              <ImpactRow label="Background CPU" part={item.impact.cpu} />
              <ImpactRow label="Memory" part={item.impact.memory} />
              <ImpactRow label="Services and tasks" part={item.impact.autostart} />
              <ImpactRow label="Disk space" part={item.impact.disk} />
            </div>
          </Section>

          <Section title="Safety">
            <div className="flex items-start gap-2.5">
              <ShieldCheck className="w-3.5 h-3.5 text-lunar-text-sec shrink-0 mt-0.5" />
              <div className="space-y-1.5">
                <p className="text-xs text-lunar-text leading-relaxed">
                  <span className="font-semibold text-lunar-white">{SAFETY_LABELS[item.safety]}.</span> {item.safetyReason}
                </p>
                {item.safetyChecks.length > 0 && (
                  <ul className="space-y-1">
                    {item.safetyChecks.map((c) => (
                      <li key={c} className="text-[11px] text-lunar-text-sec leading-relaxed">
                        • {c}
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            </div>
          </Section>

          <Section title="What ASAO would change">
            {item.selectable ? (
              <ol className="space-y-1">
                {item.plannedChanges.map((c, i) => (
                  <li key={c} className="text-xs text-lunar-text leading-relaxed">
                    <span className="font-mono text-lunar-muted mr-2">{i + 1}.</span>
                    {c}
                  </li>
                ))}
              </ol>
            ) : (
              <p className="text-xs text-lunar-text-sec leading-relaxed">
                Nothing. ASAO only changes items rated Safe to remove or Usually safe.
              </p>
            )}
          </Section>

          {item.whatBreaks && (
            <Section title="What stops working afterwards">
              <p className="text-xs text-lunar-text leading-relaxed">{item.whatBreaks}</p>
            </Section>
          )}

          <Section title="What depends on it">
            <p className="text-xs text-lunar-text leading-relaxed">
              {item.dependents.length > 0 ? item.dependents.join(", ") : "Nothing ASAO could find."}
            </p>
          </Section>

          <Section title="Can it be undone?">
            <div className="flex items-start gap-2.5">
              <RotateCcw className="w-3.5 h-3.5 text-lunar-text-sec shrink-0 mt-0.5" />
              <p className="text-xs text-lunar-text leading-relaxed">{item.undo}</p>
            </div>
          </Section>

          {(item.services.length > 0 || item.startup.length > 0 || item.tasks.length > 0) && (
            <Section title="Background parts">
              <div className="space-y-1.5">
                {item.services.map((s) => (
                  <div key={s.name} className="flex items-start gap-2 text-[11px]">
                    <Server className="w-3 h-3 text-lunar-muted shrink-0 mt-0.5" />
                    <span className="text-lunar-text">
                      Service <span className="font-mono">{s.name}</span> ({s.displayName}): {s.startType}, {s.state}
                    </span>
                  </div>
                ))}
                {item.startup.map((s) => (
                  <div key={s.id} className="flex items-start gap-2 text-[11px]">
                    <Power className="w-3 h-3 text-lunar-muted shrink-0 mt-0.5" />
                    <span className="text-lunar-text">
                      Startup entry "{s.name}": {s.enabled ? "enabled" : "already disabled"}
                    </span>
                  </div>
                ))}
                {item.tasks.map((t) => (
                  <div key={t.path} className="flex items-start gap-2 text-[11px]">
                    <Layers className="w-3 h-3 text-lunar-muted shrink-0 mt-0.5" />
                    <span className="text-lunar-text">
                      Scheduled task <span className="font-mono">{t.path}</span>: {t.enabled ? t.triggers.join(", ") || "enabled" : "disabled"}
                    </span>
                  </div>
                ))}
              </div>
            </Section>
          )}

          <Section title="Where this rating comes from">
            <div className="flex items-start gap-2.5">
              <Wrench className="w-3.5 h-3.5 text-lunar-text-sec shrink-0 mt-0.5" />
              <p className="text-[11px] text-lunar-text-sec leading-relaxed">
                {item.source ?? "Not in ASAO's list. The impact score is measured on this PC."}
              </p>
            </div>
          </Section>
        </div>

        <div className="p-4 border-t border-lunar-border bg-lunar-surface-2/40 shrink-0">
          <AskNyraButton buildPrompt={() => bloatPrompt(item)} className="w-full" />
        </div>
    </aside>
  );
};
