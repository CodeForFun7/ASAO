import React from "react";
import { AlertTriangle, Cloud, FlaskConical, ShieldAlert, Trash2, X } from "lucide-react";
import type { ResidualCandidate, ResidualCategory } from "../../../types/residual";
import { CATEGORY_LABELS, categoryConsequence } from "../../../types/residual";
import { formatStorageBytes } from "../../../services/storage";

interface LeftoverConfirmDialogProps {
  items: ResidualCandidate[];
  dryRun: boolean;
  permanent: boolean;
  onSetPermanent: (value: boolean) => void;
  onCancel: () => void;
  onConfirm: () => void;
}

const CATEGORY_ORDER: ResidualCategory[] = ["install-remnant", "cache", "logs", "app-data"];

export const LeftoverConfirmDialog: React.FC<LeftoverConfirmDialogProps> = ({
  items,
  dryRun,
  permanent,
  onSetPermanent,
  onCancel,
  onConfirm,
}) => {
  const total = items.reduce((acc, i) => acc + (i.sizeBytes ?? 0), 0);
  const unsized = items.some((i) => i.sizeBytes === null);
  const adminCount = items.filter((i) => i.needsAdmin).length;
  const onedriveCount = items.filter((i) => i.inOnedrive).length;

  // One consequence line per category, and per app for App data
  const lines: { key: string; label: string; text: string; count: number }[] = [];
  for (const cat of CATEGORY_ORDER) {
    const inCat = items.filter((i) => i.category === cat);
    if (inCat.length === 0) continue;
    if (cat === "app-data") {
      const byApp = new Map<string, ResidualCandidate[]>();
      for (const i of inCat) {
        const name = i.matchedAppName ?? "an unknown app";
        byApp.set(name, [...(byApp.get(name) ?? []), i]);
      }
      for (const [name, list] of byApp) {
        lines.push({
          key: `app-data-${name}`,
          label: CATEGORY_LABELS[cat],
          text: categoryConsequence(cat, name),
          count: list.length,
        });
      }
    } else {
      lines.push({ key: cat, label: CATEGORY_LABELS[cat], text: categoryConsequence(cat, null), count: inCat.length });
    }
  }

  const title = dryRun
    ? `Dry run: check ${items.length} ${items.length === 1 ? "folder" : "folders"}`
    : `Remove ${items.length} ${items.length === 1 ? "folder" : "folders"}?`;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-sm"
      onClick={onCancel}
      role="dialog"
      aria-modal="true"
      aria-labelledby="leftover-confirm-title"
    >
      <div
        className="w-full max-w-xl max-h-[82vh] rounded-xl bg-lunar-surface border border-lunar-border shadow-2xl flex flex-col overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="px-5 py-4 border-b border-lunar-border flex items-start justify-between gap-4 shrink-0">
          <div>
            <h2 id="leftover-confirm-title" className="text-sm font-semibold text-lunar-white tracking-tight">
              {title}
            </h2>
            <p className="text-xs text-lunar-text-sec mt-1">
              {dryRun ? "Would free" : "Frees"} about{" "}
              <span className="font-semibold text-lunar-white">{formatStorageBytes(total)}</span>
              {unsized && " (some sizes are still being measured)"}.
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
                Dry run is on. ASAO runs every safety check and writes what it would remove to the dry-run
                log. Nothing is deleted and no administrator prompt appears.
              </p>
            </div>
          )}

          <div className="space-y-2">
            {lines.map((l) => (
              <div key={l.key} className="p-3 rounded-lg bg-lunar-surface-2/60 border border-lunar-border">
                <div className="flex items-center justify-between gap-2">
                  <span className="text-xs font-semibold text-lunar-white">{l.label}</span>
                  <span className="font-mono text-[10px] text-lunar-muted">
                    {l.count} {l.count === 1 ? "folder" : "folders"}
                  </span>
                </div>
                <p className="text-[11px] text-lunar-text-sec mt-1 leading-relaxed">{l.text}</p>
              </div>
            ))}
          </div>

          {adminCount > 0 && (
            <div className="flex items-start gap-2.5 p-3 rounded-lg bg-lunar-warning/10 border border-lunar-warning/30">
              <ShieldAlert className="w-4 h-4 text-lunar-warning shrink-0 mt-0.5" />
              <p className="text-[11px] text-lunar-text leading-relaxed">
                {adminCount} {adminCount === 1 ? "folder is" : "folders are"} in Program Files or ProgramData.
                Windows will ask for administrator permission for {adminCount === 1 ? "it" : "those"} only.
              </p>
            </div>
          )}

          {onedriveCount > 0 && (
            <div className="flex items-start gap-2.5 p-3 rounded-lg bg-lunar-warning/10 border border-lunar-warning/30">
              <Cloud className="w-4 h-4 text-lunar-warning shrink-0 mt-0.5" />
              <p className="text-[11px] text-lunar-text leading-relaxed">
                {onedriveCount} {onedriveCount === 1 ? "folder is" : "folders are"} inside OneDrive. Removing{" "}
                {onedriveCount === 1 ? "it" : "them"} also removes {onedriveCount === 1 ? "it" : "them"} from every
                device that syncs with OneDrive.
              </p>
            </div>
          )}

          <div className="space-y-1.5">
            <div className="text-[11px] font-medium text-lunar-white">How to remove</div>
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
              <button
                type="button"
                onClick={() => onSetPermanent(false)}
                className={`p-3 rounded-lg border text-left transition-colors cursor-pointer ${
                  !permanent
                    ? "bg-lunar-elevated border-lunar-white/80"
                    : "bg-lunar-surface-2/60 border-lunar-border hover:bg-lunar-surface-2"
                }`}
              >
                <div className="text-xs font-semibold text-lunar-white">Move to Recycle Bin</div>
                <div className="text-[10px] text-lunar-text-sec mt-0.5">Recommended. You can restore them.</div>
              </button>
              <button
                type="button"
                onClick={() => onSetPermanent(true)}
                className={`p-3 rounded-lg border text-left transition-colors cursor-pointer ${
                  permanent
                    ? "bg-lunar-critical/10 border-lunar-critical/40"
                    : "bg-lunar-surface-2/60 border-lunar-border hover:bg-lunar-surface-2"
                }`}
              >
                <div className="text-xs font-semibold text-lunar-white">Delete permanently</div>
                <div className="text-[10px] text-lunar-text-sec mt-0.5">Cannot be undone.</div>
              </button>
            </div>
            {permanent && !dryRun && (
              <div className="flex items-center gap-1.5 text-[11px] text-lunar-critical pt-1">
                <AlertTriangle className="w-3.5 h-3.5" />
                <span>These folders will not go to the Recycle Bin.</span>
              </div>
            )}
          </div>

          <p className="text-[10px] text-lunar-muted leading-relaxed">
            Right before removing each folder, ASAO checks again that its app is still uninstalled and that no
            program is using it. Anything that fails is skipped and listed in the report.
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
            onClick={onConfirm}
            className={`inline-flex items-center gap-2 px-3.5 py-2 rounded text-xs font-medium border transition-colors cursor-pointer ${
              permanent && !dryRun
                ? "bg-lunar-critical/15 hover:bg-lunar-critical/25 text-lunar-critical border-lunar-critical/40"
                : "bg-lunar-white text-lunar-bg border-lunar-white hover:bg-lunar-text"
            }`}
          >
            {dryRun ? <FlaskConical className="w-3.5 h-3.5" /> : <Trash2 className="w-3.5 h-3.5" />}
            <span>{dryRun ? "Run dry run" : permanent ? "Delete permanently" : "Move to Recycle Bin"}</span>
          </button>
        </div>
      </div>
    </div>
  );
};
