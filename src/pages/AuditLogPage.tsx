import React, { useCallback, useEffect, useState } from "react";
import { History, RefreshCw, RotateCcw } from "lucide-react";
import { AUDIT_PAGE_SIZE, listAuditEntries, restoreAuditEntry } from "../services/audit";
import { formatStorageTimestamp } from "../services/storage";
import { AUDIT_CATEGORY_LABELS, type AuditEntry } from "../types/audit";

const restoreButtonClass =
  "inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded text-[11px] font-medium border bg-lunar-elevated hover:bg-lunar-border text-lunar-white border-lunar-border transition-colors cursor-pointer disabled:opacity-45 disabled:cursor-not-allowed shrink-0";

export const AuditLogPage: React.FC = () => {
  const [entries, setEntries] = useState<AuditEntry[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [restoringId, setRestoringId] = useState<number | null>(null);
  const [messages, setMessages] = useState<Record<number, { ok: boolean; text: string }>>({});

  const load = useCallback(async (count: number = AUDIT_PAGE_SIZE) => {
    setLoading(true);
    try {
      const page = await listAuditEntries(count, 0);
      setEntries(page.entries);
      setTotal(page.total);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const loadMore = async () => {
    setLoading(true);
    try {
      const page = await listAuditEntries(AUDIT_PAGE_SIZE, entries.length);
      setEntries((prev) => [...prev, ...page.entries]);
      setTotal(page.total);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const restore = async (entry: AuditEntry) => {
    setRestoringId(entry.id);
    try {
      const result = await restoreAuditEntry(entry.id);
      setMessages((m) => ({ ...m, [entry.id]: { ok: result.ok, text: result.message } }));
      // Reload what is on screen: a restore adds a new entry at the top.
      await load(Math.max(entries.length + 1, AUDIT_PAGE_SIZE));
    } catch (e) {
      setMessages((m) => ({ ...m, [entry.id]: { ok: false, text: String(e) } }));
    } finally {
      setRestoringId(null);
    }
  };

  return (
    <div className="flex-1 flex flex-col min-h-0 p-6 gap-4 overflow-hidden">
      <header className="flex items-start justify-between gap-4 shrink-0">
        <div>
          <h1 className="text-base font-semibold text-lunar-white">Audit log</h1>
          <p className="text-xs text-lunar-text-sec mt-1">
            Every change made through ASAO, newest first. Restore puts a change back when ASAO kept what it needs.
          </p>
        </div>
        <button type="button" onClick={() => void load()} disabled={loading} className={restoreButtonClass}>
          <RefreshCw className={`w-3.5 h-3.5 ${loading ? "animate-spin" : ""}`} />
          Refresh
        </button>
      </header>

      {error && (
        <div className="text-xs text-lunar-critical border border-lunar-border rounded-lg px-3 py-2 shrink-0">
          {error}
        </div>
      )}

      <div className="flex-1 min-h-0 overflow-y-auto space-y-2 pr-1">
        {!loading && !error && entries.length === 0 && (
          <div className="flex flex-col items-center justify-center gap-2 py-16 text-lunar-muted">
            <History className="w-6 h-6" />
            <p className="text-xs">No changes yet.</p>
          </div>
        )}

        {entries.map((entry) => {
          const message = messages[entry.id];
          const restored = entry.restoredAt !== null;
          return (
            <div key={entry.id} className="lunar-glass-card p-3 flex items-start justify-between gap-4">
              <div className="min-w-0 space-y-1">
                <div className="flex items-center gap-2">
                  <span className="text-[10px] uppercase tracking-wide text-lunar-muted border border-lunar-border rounded px-1.5 py-0.5 shrink-0">
                    {AUDIT_CATEGORY_LABELS[entry.category] ?? entry.category}
                  </span>
                  <span className="text-xs font-medium text-lunar-white truncate" title={entry.title}>
                    {entry.title}
                  </span>
                </div>
                {entry.detail && (
                  <p className="text-[11px] text-lunar-text-sec break-all">{entry.detail}</p>
                )}
                <p className="text-[11px] text-lunar-muted">
                  {formatStorageTimestamp(entry.createdAt)}
                  {restored && ` · Restored ${formatStorageTimestamp(entry.restoredAt)}`}
                </p>
                {!entry.restorable && !restored && entry.undoNote && (
                  <p className="text-[11px] text-lunar-muted">{entry.undoNote}</p>
                )}
                {message && (
                  <p className={`text-[11px] ${message.ok ? "text-lunar-healthy" : "text-lunar-critical"}`}>
                    {message.text}
                  </p>
                )}
              </div>
              <button
                type="button"
                className={restoreButtonClass}
                disabled={!entry.restorable || restoringId !== null}
                title={
                  restored ? "Already restored" : entry.restorable ? "Restore this change" : entry.undoNote ?? "Cannot be restored"
                }
                onClick={() => void restore(entry)}
              >
                {restoringId === entry.id ? (
                  <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                ) : (
                  <RotateCcw className="w-3.5 h-3.5" />
                )}
                {restored ? "Restored" : "Restore"}
              </button>
            </div>
          );
        })}

        {entries.length < total && (
          <div className="flex justify-center pt-2">
            <button type="button" onClick={() => void loadMore()} disabled={loading} className={restoreButtonClass}>
              Load older ({total - entries.length} more)
            </button>
          </div>
        )}
      </div>
    </div>
  );
};
