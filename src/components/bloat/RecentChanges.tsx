import React from "react";
import { ExternalLink, History, LifeBuoy, RefreshCw, RotateCcw } from "lucide-react";
import type { HistoryBatch } from "../../types/bloat";
import { formatStorageTimestamp } from "../../services/storage";
import { OUTCOME_LABELS } from "./BloatReportDialog";

interface RecentChangesProps {
  batches: HistoryBatch[];
  undoing: Record<string, boolean>;
  messages: Record<string, { ok: boolean; message: string }>;
  onUndo: (batchId: string, actionId: string) => void;
}

export const RecentChanges: React.FC<RecentChangesProps> = ({ batches, undoing, messages, onUndo }) => {
  if (batches.length === 0) {
    return (
      <div className="rounded-lg lunar-glass-card p-10 flex flex-col items-center justify-center text-center">
        <History className="w-8 h-8 text-lunar-muted mb-3 stroke-[1.5]" />
        <p className="text-sm font-medium text-lunar-white">No changes yet.</p>
        <p className="text-xs text-lunar-text-sec mt-1 max-w-md">
          Changes ASAO makes here are listed with a way to undo each one. Previews are only written to the log.
        </p>
      </div>
    );
  }

  return (
    <div className="space-y-3">
      {batches.map((b) => (
        <div key={b.id} className="rounded-xl lunar-glass-card p-4 space-y-3">
          <div className="flex items-center justify-between gap-3">
            <div className="text-xs font-semibold text-lunar-white">{formatStorageTimestamp(b.timeMs)}</div>
            <div className="flex items-center gap-1.5 text-[11px] text-lunar-text-sec min-w-0">
              <LifeBuoy className="w-3.5 h-3.5 shrink-0" />
              <span className="truncate" title={b.restorePoint.message}>
                {b.restorePoint.status === "created"
                  ? "Restore point created"
                  : b.restorePoint.status === "recent"
                    ? "Using the day's existing restore point"
                    : "No restore point"}
              </span>
            </div>
          </div>
          <div className="space-y-1.5">
            {b.actions.map((a) => {
              const key = `${b.id}/${a.outcome.action.id}`;
              const busy = Boolean(undoing[key]);
              const msg = messages[key];
              const undone = a.undoneMs !== null;
              return (
                <div key={key} className="p-2.5 rounded-lg bg-lunar-surface-2/60 border border-lunar-border">
                  <div className="flex items-center justify-between gap-3">
                    <div className="min-w-0">
                      <div className="text-[11px] text-lunar-text leading-relaxed">{a.outcome.action.description}</div>
                      <div className="text-[10px] text-lunar-muted">
                        {undone ? "Undone" : OUTCOME_LABELS[a.outcome.status]}
                        {!a.canUndo && !a.storeUrl && !undone && ` · ${a.undoHint}`}
                      </div>
                    </div>
                    {!undone && (a.canUndo || a.storeUrl) && (
                      <button
                        type="button"
                        disabled={busy}
                        onClick={() => onUndo(b.id, a.outcome.action.id)}
                        className="inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded text-[11px] font-medium border bg-lunar-elevated hover:bg-lunar-border text-lunar-white border-lunar-border transition-colors cursor-pointer disabled:opacity-45 disabled:cursor-not-allowed shrink-0"
                      >
                        {busy ? (
                          <RefreshCw className="w-3 h-3 animate-spin" />
                        ) : a.storeUrl ? (
                          <ExternalLink className="w-3 h-3" />
                        ) : (
                          <RotateCcw className="w-3 h-3" />
                        )}
                        <span>{a.storeUrl ? "Open in Store" : "Undo"}</span>
                      </button>
                    )}
                  </div>
                  {msg && (
                    <p className={`text-[10px] mt-1 ${msg.ok ? "text-lunar-healthy" : "text-lunar-warning"}`}>{msg.message}</p>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      ))}
    </div>
  );
};
