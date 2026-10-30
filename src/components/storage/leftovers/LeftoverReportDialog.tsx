import React from "react";
import { CheckCircle2, ExternalLink, FlaskConical, X } from "lucide-react";
import type { CleanReport } from "../../../types/residual";
import { formatStorageBytes } from "../../../services/storage";

interface LeftoverReportDialogProps {
  report: CleanReport;
  onOpenRecycleBin: () => void;
  onClose: () => void;
}

export const LeftoverReportDialog: React.FC<LeftoverReportDialogProps> = ({ report, onOpenRecycleBin, onClose }) => {
  const done = report.outcomes.filter((o) => o.status === "deleted" || o.status === "would-delete");
  const skipped = report.outcomes.filter((o) => o.status === "skipped" || o.status === "failed");
  const recycled = report.outcomes.some((o) => o.status === "deleted" && o.recycled);

  const title = report.dryRun
    ? "Dry run finished"
    : done.length > 0
    ? "Leftovers removed"
    : "Nothing was removed";

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-sm"
      onClick={onClose}
      role="dialog"
      aria-modal="true"
      aria-labelledby="leftover-report-title"
    >
      <div
        className="w-full max-w-xl max-h-[82vh] rounded-xl bg-lunar-surface border border-lunar-border shadow-2xl flex flex-col overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="px-5 py-4 border-b border-lunar-border flex items-start justify-between gap-4 shrink-0">
          <div className="flex items-start gap-3">
            <div className="w-8 h-8 rounded-lg bg-lunar-elevated border border-lunar-border flex items-center justify-center shrink-0">
              {report.dryRun ? (
                <FlaskConical className="w-4 h-4 text-lunar-text-sec" />
              ) : (
                <CheckCircle2 className={`w-4 h-4 ${done.length > 0 ? "text-lunar-healthy" : "text-lunar-text-sec"}`} />
              )}
            </div>
            <div>
              <h2 id="leftover-report-title" className="text-sm font-semibold text-lunar-white tracking-tight">
                {title}
              </h2>
              <p className="text-xs text-lunar-text-sec mt-1">
                {report.dryRun ? "Would free" : "Freed"}{" "}
                <span className="font-semibold text-lunar-white">{formatStorageBytes(report.freedBytes)}</span>
                {" "}from {done.length} {done.length === 1 ? "folder" : "folders"}
                {skipped.length > 0 && `, ${skipped.length} skipped`}.
              </p>
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

        <div className="flex-1 overflow-y-auto p-5 space-y-4">
          {done.length > 0 && (
            <div className="space-y-1.5">
              <div className="text-[11px] font-medium text-lunar-white">
                {report.dryRun ? "Would remove" : report.permanent ? "Deleted permanently" : "Moved to the Recycle Bin"}
              </div>
              {done.map((o) => (
                <div key={o.path} className="flex items-center justify-between gap-3 px-3 py-2 rounded-lg bg-lunar-surface-2/60 border border-lunar-border">
                  <div className="min-w-0">
                    <div className="text-xs text-lunar-white truncate">{o.name}</div>
                    <div className="font-mono text-[10px] text-lunar-muted truncate" title={o.path}>{o.path}</div>
                    {o.reason && <div className="text-[10px] text-lunar-text-sec mt-0.5">{o.reason}</div>}
                  </div>
                  <span className="font-mono text-[11px] text-lunar-text-sec shrink-0">
                    {o.sizeBytes !== null ? formatStorageBytes(o.sizeBytes) : ""}
                  </span>
                </div>
              ))}
            </div>
          )}

          {skipped.length > 0 && (
            <div className="space-y-1.5">
              <div className="text-[11px] font-medium text-lunar-white">Skipped</div>
              {skipped.map((o) => (
                <div key={o.path} className="px-3 py-2 rounded-lg bg-lunar-warning/10 border border-lunar-warning/25">
                  <div className="text-xs text-lunar-white truncate">{o.name}</div>
                  <div className="font-mono text-[10px] text-lunar-muted truncate" title={o.path}>{o.path}</div>
                  <div className="text-[11px] text-lunar-warning mt-0.5">{o.reason ?? "Skipped"}</div>
                </div>
              ))}
            </div>
          )}

          {report.adminDeclined && (
            <p className="text-[11px] text-lunar-text-sec">
              Administrator permission was declined, so Program Files and ProgramData folders were left in place.
            </p>
          )}

          <div className="text-[10px] text-lunar-muted">
            Logged to <span className="font-mono break-all">{report.logPath}</span>
          </div>
        </div>

        <div className="px-5 py-3 border-t border-lunar-border bg-lunar-surface-2/40 flex items-center justify-end gap-2 shrink-0">
          {recycled && (
            <button
              type="button"
              onClick={onOpenRecycleBin}
              className="inline-flex items-center gap-2 px-3.5 py-2 rounded text-xs font-medium bg-lunar-elevated hover:bg-lunar-border text-lunar-white border border-lunar-border transition-colors cursor-pointer"
            >
              <ExternalLink className="w-3.5 h-3.5" />
              <span>Open Recycle Bin</span>
            </button>
          )}
          <button
            type="button"
            onClick={onClose}
            className="px-3.5 py-2 rounded text-xs font-medium bg-lunar-white text-lunar-bg border border-lunar-white hover:bg-lunar-text transition-colors cursor-pointer"
          >
            Done
          </button>
        </div>
      </div>
    </div>
  );
};
