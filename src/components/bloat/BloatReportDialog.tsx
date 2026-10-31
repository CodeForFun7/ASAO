import React from "react";
import { CheckCircle2, FlaskConical, LifeBuoy, PackageSearch, RotateCcw, X, XCircle } from "lucide-react";
import type { BloatApplyReport, OutcomeStatus } from "../../types/bloat";

interface BloatReportDialogProps {
  report: BloatApplyReport;
  onFindLeftovers: () => void;
  onRestartExplorer: () => void;
  onClose: () => void;
}

export const OUTCOME_LABELS: Record<OutcomeStatus, string> = {
  done: "Done",
  "would-do": "Would change",
  skipped: "Skipped",
  failed: "Failed",
  protected: "Protected by Windows",
  "not-confirmed": "Not confirmed",
};

const OUTCOME_STYLE: Record<OutcomeStatus, string> = {
  done: "text-lunar-healthy",
  "would-do": "text-lunar-text-sec",
  skipped: "text-lunar-muted",
  failed: "text-lunar-critical",
  protected: "text-lunar-ai",
  "not-confirmed": "text-lunar-warning",
};

export const BloatReportDialog: React.FC<BloatReportDialogProps> = ({ report, onFindLeftovers, onRestartExplorer, onClose }) => {
  const [explorerRestarted, setExplorerRestarted] = React.useState(false);
  const done = report.outcomes.filter((o) => o.status === "done").length;
  const problems = report.outcomes.filter((o) => ["failed", "protected", "not-confirmed"].includes(o.status)).length;
  const title = report.dryRun
    ? "Preview finished: nothing was changed"
    : report.error
      ? "Nothing was changed"
      : report.stopped
        ? "Stopped before changing anything"
        : problems > 0
          ? `${done} ${done === 1 ? "change" : "changes"} made, ${problems} not`
          : `${done} ${done === 1 ? "change" : "changes"} made`;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-sm"
      onClick={onClose}
      role="dialog"
      aria-modal="true"
      aria-labelledby="bloat-report-title"
    >
      <div
        className="w-full max-w-xl max-h-[82vh] rounded-xl bg-lunar-surface border border-lunar-border shadow-2xl flex flex-col overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="px-5 py-4 border-b border-lunar-border flex items-start justify-between gap-4 shrink-0">
          <div className="flex items-start gap-2.5">
            {report.dryRun ? (
              <FlaskConical className="w-4 h-4 text-lunar-text-sec mt-0.5" />
            ) : problems > 0 || report.error || report.stopped ? (
              <XCircle className="w-4 h-4 text-lunar-warning mt-0.5" />
            ) : (
              <CheckCircle2 className="w-4 h-4 text-lunar-healthy mt-0.5" />
            )}
            <div>
              <h2 id="bloat-report-title" className="text-sm font-semibold text-lunar-white tracking-tight">
                {title}
              </h2>
              {report.error && <p className="text-xs text-lunar-text-sec mt-1">{report.error}</p>}
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close"
            className="p-1.5 rounded-md text-lunar-muted hover:text-lunar-white hover:bg-lunar-elevated transition-colors cursor-pointer"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto p-5 space-y-3">
          {!report.dryRun && (
            <div className="flex items-start gap-2.5 p-3 rounded-lg bg-lunar-surface-2/60 border border-lunar-border">
              <LifeBuoy className="w-4 h-4 text-lunar-text-sec shrink-0 mt-0.5" />
              <p className="text-[11px] text-lunar-text-sec leading-relaxed">{report.restorePoint.message}</p>
            </div>
          )}

          <div className="space-y-1.5">
            {report.outcomes.map((o) => (
              <div key={o.action.id} className="p-2.5 rounded-lg bg-lunar-surface-2/60 border border-lunar-border">
                <div className="flex items-start justify-between gap-3">
                  <span className="text-[11px] text-lunar-text leading-relaxed">{o.action.description}</span>
                  <span className={`text-[10px] font-mono shrink-0 ${OUTCOME_STYLE[o.status]}`}>
                    {OUTCOME_LABELS[o.status]}
                  </span>
                </div>
                {o.message && <p className="text-[10px] text-lunar-text-sec mt-1 leading-relaxed">{o.message}</p>}
              </div>
            ))}
          </div>

          {!report.dryRun && report.settingsChanged && (
            <div className="flex items-start gap-2.5 p-3 rounded-lg bg-lunar-surface-2/60 border border-lunar-border">
              <RotateCcw className="w-4 h-4 text-lunar-text-sec shrink-0 mt-0.5" />
              <p className="text-[11px] text-lunar-text-sec leading-relaxed">
                {report.restartNeeded
                  ? "Some settings take effect after you restart the PC."
                  : "Taskbar, Start and File Explorer settings show after File Explorer restarts or you sign out."}{" "}
                {explorerRestarted && "File Explorer was restarted."}
              </p>
            </div>
          )}

          {report.rejected.length > 0 && (
            <p className="text-[11px] text-lunar-text-sec leading-relaxed">
              Left out: {report.rejected.map((r) => `${r.name} (${r.reason})`).join("; ")}
            </p>
          )}

          <p className="text-[10px] text-lunar-muted font-mono break-all">Log: {report.logPath}</p>
        </div>

        <div className="px-5 py-3 border-t border-lunar-border bg-lunar-surface-2/40 flex items-center justify-end gap-2 shrink-0">
          {!report.dryRun && report.settingsChanged && !explorerRestarted && (
            <button
              type="button"
              onClick={() => {
                onRestartExplorer();
                setExplorerRestarted(true);
              }}
              title="Open File Explorer windows will close"
              className="inline-flex items-center gap-2 px-3.5 py-2 rounded text-xs font-medium border bg-lunar-elevated hover:bg-lunar-border text-lunar-white border-lunar-border transition-colors cursor-pointer"
            >
              <RotateCcw className="w-3.5 h-3.5" />
              <span>Restart File Explorer</span>
            </button>
          )}
          {!report.dryRun && report.uninstalled.length > 0 && (
            <button
              type="button"
              onClick={onFindLeftovers}
              className="inline-flex items-center gap-2 px-3.5 py-2 rounded text-xs font-medium border bg-lunar-elevated hover:bg-lunar-border text-lunar-white border-lunar-border transition-colors cursor-pointer"
            >
              <PackageSearch className="w-3.5 h-3.5" />
              <span>Find their leftovers</span>
            </button>
          )}
          <button
            type="button"
            onClick={onClose}
            className="px-3.5 py-2 rounded text-xs font-medium bg-lunar-white text-lunar-bg border border-lunar-white hover:bg-lunar-text transition-colors cursor-pointer"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  );
};
