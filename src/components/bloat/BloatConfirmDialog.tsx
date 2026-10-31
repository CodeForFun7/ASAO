import React from "react";
import { AlertTriangle, FlaskConical, LifeBuoy, ShieldAlert, Trash2, X } from "lucide-react";
import type { BloatAction, BloatPlan } from "../../types/bloat";
import { actionGroup } from "../../types/bloat";
import { formatStorageBytes } from "../../services/storage";
import { LeftoverCheckbox } from "../storage/leftovers/LeftoverPrimitives";

interface BloatConfirmDialogProps {
  plan: BloatPlan;
  dryRun: boolean;
  hasStoreApps: boolean;
  removeProvisioned: boolean;
  continueWithoutRestorePoint: boolean;
  onSetRemoveProvisioned: (v: boolean) => void;
  onSetContinueWithoutRestorePoint: (v: boolean) => void;
  onCancel: () => void;
  onConfirm: () => void;
}

const GROUP_ORDER = [
  "Uninstall",
  "Disable service",
  "Disable startup entry",
  "Disable scheduled task",
  "Remove for new users",
  "Change Windows setting",
];

export const BloatConfirmDialog: React.FC<BloatConfirmDialogProps> = ({
  plan,
  dryRun,
  hasStoreApps,
  removeProvisioned,
  continueWithoutRestorePoint,
  onSetRemoveProvisioned,
  onSetContinueWithoutRestorePoint,
  onCancel,
  onConfirm,
}) => {
  const groups = new Map<string, BloatAction[]>();
  for (const a of plan.actions) {
    if (a.kind.type === "restore-point") continue;
    const g = actionGroup(a.kind);
    groups.set(g, [...(groups.get(g) ?? []), a]);
  }
  const ordered = GROUP_ORDER.filter((g) => groups.has(g));
  const changeCount = plan.actions.filter((a) => a.kind.type !== "restore-point").length;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-sm"
      onClick={onCancel}
      role="dialog"
      aria-modal="true"
      aria-labelledby="bloat-confirm-title"
    >
      <div
        className="w-full max-w-xl max-h-[82vh] rounded-xl bg-lunar-surface border border-lunar-border shadow-2xl flex flex-col overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="px-5 py-4 border-b border-lunar-border flex items-start justify-between gap-4 shrink-0">
          <div>
            <h2 id="bloat-confirm-title" className="text-sm font-semibold text-lunar-white tracking-tight">
              {dryRun ? "Preview these changes?" : `Make ${changeCount} ${changeCount === 1 ? "change" : "changes"}?`}
            </h2>
            <p className="text-xs text-lunar-text-sec mt-1">
              {dryRun ? "Would free" : "Frees"} about{" "}
              <span className="font-semibold text-lunar-white">{formatStorageBytes(plan.freedBytes)}</span>
              {plan.servicesStopped > 0 &&
                ` and stops ${plan.servicesStopped} background ${plan.servicesStopped === 1 ? "service" : "services"}`}
              .
            </p>
          </div>
          <button
            type="button"
            onClick={onCancel}
            aria-label="Close"
            className="p-1.5 rounded-md text-lunar-muted hover:text-lunar-white hover:bg-lunar-elevated transition-colors cursor-pointer"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto p-5 space-y-4">
          {dryRun && (
            <div className="flex items-start gap-2.5 p-3 rounded-lg bg-lunar-bg border border-lunar-border">
              <FlaskConical className="w-4 h-4 text-lunar-text-sec shrink-0 mt-0.5" />
              <p className="text-[11px] text-lunar-text-sec leading-relaxed">
                Preview only is on. ASAO checks everything below and writes what it would change to the preview log.
                Nothing is uninstalled or disabled, and no administrator prompt appears.
              </p>
            </div>
          )}

          {ordered.map((g) => (
            <div key={g} className="p-3 rounded-lg bg-lunar-surface-2/60 border border-lunar-border space-y-1.5">
              <div className="flex items-center justify-between gap-2">
                <span className="text-xs font-semibold text-lunar-white">{g}</span>
                <span className="font-mono text-[10px] text-lunar-muted">{groups.get(g)!.length}</span>
              </div>
              <ul className="space-y-1">
                {groups.get(g)!.map((a) => (
                  <li key={a.id} className="text-[11px] text-lunar-text-sec leading-relaxed">
                    • {a.description}
                  </li>
                ))}
              </ul>
            </div>
          ))}

          {plan.actions.some((a) => a.kind.type === "apply-tweak") && (
            <p className="text-[11px] text-lunar-text-sec leading-relaxed px-1">
              Settings are changed by importing Win11Debloat's registry files. ASAO first backs up the exact values
              each one replaces, so Undo in Recent changes puts them back.
            </p>
          )}

          {plan.notUndoable.length > 0 && (
            <div className="flex items-start gap-2.5 p-3 rounded-lg bg-lunar-warning/10 border border-lunar-warning/30">
              <AlertTriangle className="w-4 h-4 text-lunar-warning shrink-0 mt-0.5" />
              <div className="text-[11px] text-lunar-text leading-relaxed">
                <span className="font-semibold">ASAO can't undo these automatically:</span> {plan.notUndoable.join(", ")}.
                Their own uninstallers run, and their windows may appear. To get them back, reinstall them from the
                publisher or go back to the restore point.
              </div>
            </div>
          )}

          {!dryRun && (
            <div className="p-3 rounded-lg bg-lunar-surface-2/60 border border-lunar-border space-y-2">
              <div className="flex items-start gap-2.5">
                <LifeBuoy className="w-4 h-4 text-lunar-text-sec shrink-0 mt-0.5" />
                <p className="text-[11px] text-lunar-text-sec leading-relaxed">
                  ASAO creates a System Restore point first. Windows makes at most one a day, so if one was made in the
                  last 24 hours, that one is your fallback. If System Protection is off, no restore point can be made.
                </p>
              </div>
              <label className="flex items-center gap-2.5 cursor-pointer">
                <LeftoverCheckbox
                  state={continueWithoutRestorePoint ? "checked" : "unchecked"}
                  label="Continue without a restore point"
                  onToggle={() => onSetContinueWithoutRestorePoint(!continueWithoutRestorePoint)}
                />
                <span className="text-[11px] text-lunar-text">
                  If no restore point can be made, continue anyway (otherwise ASAO stops before changing anything)
                </span>
              </label>
            </div>
          )}

          {hasStoreApps && (
            <label className="flex items-center gap-2.5 cursor-pointer px-1">
              <LeftoverCheckbox
                state={removeProvisioned ? "checked" : "unchecked"}
                label="Also remove for new users on this PC"
                onToggle={() => onSetRemoveProvisioned(!removeProvisioned)}
              />
              <span className="text-[11px] text-lunar-text">
                Also remove the Store apps for new users on this PC (they won't be installed for new accounts)
              </span>
            </label>
          )}

          {!dryRun && plan.needsAdmin && (
            <div className="flex items-start gap-2.5 p-3 rounded-lg bg-lunar-warning/10 border border-lunar-warning/30">
              <ShieldAlert className="w-4 h-4 text-lunar-warning shrink-0 mt-0.5" />
              <p className="text-[11px] text-lunar-text leading-relaxed">
                Windows will ask for administrator permission once. It is needed for the restore point, services,
                scheduled tasks and machine-wide startup entries.
              </p>
            </div>
          )}

          {plan.rejected.length > 0 && (
            <p className="text-[11px] text-lunar-text-sec leading-relaxed">
              Left out: {plan.rejected.map((r) => `${r.name} (${r.reason})`).join("; ")}
            </p>
          )}

          <p className="text-[10px] text-lunar-muted leading-relaxed">
            Services are stopped and set to Manual or Disabled, never deleted. Startup entries and tasks are switched
            off the same way Task Manager does it, never deleted. Every change and its original state is logged.
          </p>
        </div>

        <div className="px-5 py-3 border-t border-lunar-border bg-lunar-surface-2/40 flex items-center justify-end gap-2 shrink-0">
          <button
            type="button"
            onClick={onCancel}
            className="px-3.5 py-2 rounded text-xs font-medium text-lunar-text-sec hover:text-lunar-white hover:bg-lunar-elevated transition-colors cursor-pointer"
          >
            Cancel
          </button>
          <button
            type="button"
            disabled={changeCount === 0}
            onClick={onConfirm}
            className="inline-flex items-center gap-2 px-3.5 py-2 rounded text-xs font-medium border transition-colors cursor-pointer bg-lunar-white text-lunar-bg border-lunar-white hover:bg-lunar-text disabled:opacity-45 disabled:cursor-not-allowed"
          >
            {dryRun ? <FlaskConical className="w-3.5 h-3.5" /> : <Trash2 className="w-3.5 h-3.5" />}
            <span>{dryRun ? "Show preview" : "Make changes"}</span>
          </button>
        </div>
      </div>
    </div>
  );
};
