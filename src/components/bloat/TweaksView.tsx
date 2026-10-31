import React from "react";
import { Info, RotateCcw, ShieldAlert } from "lucide-react";
import type { TweakGroup, TweakItem, TweakSet } from "../../types/bloat";
import { LeftoverCheckbox } from "../storage/leftovers/LeftoverPrimitives";

interface TweaksViewProps {
  tweaks: TweakSet;
  selected: Record<string, boolean>;
  groupChoices: Record<string, string>;
  busy: boolean;
  onToggle: (id: string) => void;
  onChoose: (groupId: string, featureId: string | null) => void;
}

const selectClass =
  "h-8 px-2.5 rounded-lg bg-lunar-surface border border-lunar-border text-[11px] text-lunar-text focus:outline-none focus:border-lunar-text-sec cursor-pointer disabled:opacity-45 disabled:cursor-not-allowed";

const Badges: React.FC<{ needsAdmin: boolean; requiresReboot: boolean }> = ({ needsAdmin, requiresReboot }) => (
  <>
    {requiresReboot && (
      <span className="inline-flex items-center gap-1 font-mono text-[10px] px-1.5 py-0.5 rounded border bg-lunar-bg text-lunar-text-sec border-lunar-border shrink-0">
        <RotateCcw className="w-2.5 h-2.5" />
        Restart needed
      </span>
    )}
    {needsAdmin && (
      <span
        title="Changes a machine-wide setting, so Windows asks for administrator permission"
        className="inline-flex items-center gap-1 font-mono text-[10px] px-1.5 py-0.5 rounded border bg-lunar-bg text-lunar-text-sec border-lunar-border shrink-0"
      >
        <ShieldAlert className="w-2.5 h-2.5" />
        Admin
      </span>
    )}
  </>
);

const TweakRow: React.FC<{ t: TweakItem; checked: boolean; busy: boolean; onToggle: () => void }> = ({
  t,
  checked,
  busy,
  onToggle,
}) => {
  const applied = t.applied === true;
  return (
    <div className="flex items-start gap-3 py-2.5">
      <div className="pt-0.5">
        <LeftoverCheckbox
          state={applied || checked ? "checked" : "unchecked"}
          disabled={applied || busy}
          label={t.label}
          onToggle={onToggle}
        />
      </div>
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 flex-wrap">
          <span className={`text-xs ${applied ? "text-lunar-text-sec" : "text-lunar-white"}`}>{t.label}</span>
          {applied && <span className="text-[10px] font-mono text-lunar-healthy">Already set</span>}
          {t.applied === null && <span className="text-[10px] font-mono text-lunar-muted">State unknown</span>}
          <Badges needsAdmin={t.needsAdmin} requiresReboot={t.requiresReboot} />
        </div>
        {t.tooltip && <p className="text-[11px] text-lunar-text-sec mt-0.5 leading-relaxed">{t.tooltip}</p>}
      </div>
    </div>
  );
};

const GroupRow: React.FC<{
  g: TweakGroup;
  tweaks: TweakItem[];
  choice: string | undefined;
  busy: boolean;
  onChoose: (featureId: string | null) => void;
}> = ({ g, tweaks, choice, busy, onChoose }) => {
  const value = choice ?? g.current ?? "";
  const needsAdmin = g.options.some((o) => tweaks.find((t) => t.id === o.featureId)?.needsAdmin);
  const requiresReboot = g.options.some((o) => tweaks.find((t) => t.id === o.featureId)?.requiresReboot);
  return (
    <div className="flex flex-col sm:flex-row sm:items-start justify-between gap-2 py-2.5">
      <div className="min-w-0">
        <div className="flex items-center gap-2 flex-wrap">
          <span className="text-xs text-lunar-white">{g.label}</span>
          {choice && <span className="text-[10px] font-mono text-lunar-warning">Will change</span>}
          <Badges needsAdmin={needsAdmin} requiresReboot={requiresReboot} />
        </div>
        {g.tooltip && <p className="text-[11px] text-lunar-text-sec mt-0.5 leading-relaxed">{g.tooltip}</p>}
      </div>
      <select
        aria-label={g.label}
        value={value}
        disabled={busy}
        onChange={(e) => onChoose(e.target.value || null)}
        className={`${selectClass} sm:w-72 shrink-0`}
      >
        {!g.current && <option value="">Current setting (not one of these)</option>}
        {g.options.map((o) => (
          <option key={o.featureId} value={o.featureId}>
            {o.label}
            {o.featureId === g.current ? " · current" : ""}
          </option>
        ))}
      </select>
    </div>
  );
};

export const TweaksView: React.FC<TweaksViewProps> = ({ tweaks, selected, groupChoices, busy, onToggle, onChoose }) => {
  const categories: string[] = [];
  for (const t of tweaks.tweaks) if (!categories.includes(t.category)) categories.push(t.category);
  for (const g of tweaks.groups) if (!categories.includes(g.category)) categories.push(g.category);

  return (
    <div className="space-y-3">
      <div className="flex items-start gap-2.5 px-3.5 py-2.5 rounded-lg bg-lunar-surface border border-lunar-border">
        <Info className="w-3.5 h-3.5 text-lunar-text-sec shrink-0 mt-0.5" />
        <p className="text-[11px] text-lunar-text-sec leading-relaxed">
          Windows settings from Win11Debloat. Each one imports a registry file; ASAO backs up the exact values it
          replaces, so every change can be undone from Recent changes. Ticked ones are Win11Debloat's default
          selection. Windows Update and System tweaks are not offered.
        </p>
      </div>
      {categories.map((c) => {
        const single = tweaks.tweaks.filter((t) => t.category === c && !t.groupId);
        const groups = tweaks.groups.filter((g) => g.category === c);
        return (
          <div key={c} className="rounded-xl lunar-glass-card px-4 py-2">
            <div className="text-xs font-semibold text-lunar-white pt-2 pb-1">{c}</div>
            <div className="divide-y divide-lunar-border/40">
              {groups.map((g) => (
                <GroupRow
                  key={g.id}
                  g={g}
                  tweaks={tweaks.tweaks}
                  choice={groupChoices[g.id]}
                  busy={busy}
                  onChoose={(f) => onChoose(g.id, f)}
                />
              ))}
              {single.map((t) => (
                <TweakRow key={t.id} t={t} checked={Boolean(selected[t.id])} busy={busy} onToggle={() => onToggle(t.id)} />
              ))}
            </div>
          </div>
        );
      })}
      {tweaks.notOffered.length > 0 && (
        <details className="rounded-lg bg-lunar-surface border border-lunar-border px-3.5 py-2.5">
          <summary className="text-[11px] text-lunar-text-sec cursor-pointer">
            {tweaks.notOffered.length} Win11Debloat settings are not offered here
          </summary>
          <ul className="mt-2 space-y-1">
            {tweaks.notOffered.map(([label, why]) => (
              <li key={label} className="text-[11px] text-lunar-text-sec leading-relaxed">
                • <span className="text-lunar-text">{label}</span>: {why}
              </li>
            ))}
          </ul>
        </details>
      )}
    </div>
  );
};
