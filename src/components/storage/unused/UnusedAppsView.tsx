import React, { useEffect, useMemo } from "react";
import {
  AlertTriangle,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  EyeOff,
  FolderOpen,
  Gamepad2,
  Hourglass,
  Info,
  Loader2,
  Package,
  RefreshCw,
  ShieldAlert,
  ShoppingBag,
  Trash2,
  X,
} from "lucide-react";
import { useAppsStore, type PendingAction } from "../../../stores/apps-store";
import type { UnusedApp } from "../../../types/apps";
import {
  KIND_LABELS,
  METHOD_LABELS,
  USAGE_SOURCE_LABELS,
  formatAgo,
  methodExplanation,
} from "../../../types/apps";
import { formatStorageBytes, formatStorageTimestamp } from "../../../services/storage";

const THRESHOLDS = [
  { days: 30, label: "30 days" },
  { days: 90, label: "3 months" },
  { days: 180, label: "6 months" },
  { days: 365, label: "1 year" },
];

interface UnusedAppsViewProps {
  onOpenInExplorer: (path: string) => void;
  onReviewLeftovers: () => void;
}

const KindIcon: React.FC<{ app: UnusedApp }> = ({ app }) => {
  const Icon =
    app.kind === "store"
      ? ShoppingBag
      : app.kind === "steam" || app.kind === "epic"
      ? Gamepad2
      : app.kind === "unregistered"
      ? FolderOpen
      : Package;
  return (
    <div className="w-7 h-7 rounded-md bg-lunar-elevated border border-lunar-border flex items-center justify-center shrink-0">
      <Icon className="w-3.5 h-3.5 text-lunar-text-sec" />
    </div>
  );
};

const PendingNote: React.FC<{
  pending: PendingAction;
  onDismiss: () => void;
  onReviewLeftovers: () => void;
}> = ({ pending, onDismiss, onReviewLeftovers }) => {
  const tone =
    pending.status === "removed" || pending.status === "done"
      ? "bg-lunar-healthy/10 border-lunar-healthy/30"
      : pending.status === "failed" || pending.status === "declined" || pending.status === "still-installed"
      ? "bg-lunar-warning/10 border-lunar-warning/30"
      : "bg-lunar-bg border-lunar-border";
  return (
    <div className={`mt-2 flex items-start gap-2.5 px-3 py-2 rounded-lg border ${tone}`}>
      {pending.status === "working" || pending.status === "waiting" ? (
        <Loader2 className="w-3.5 h-3.5 text-lunar-text-sec shrink-0 mt-0.5 animate-spin" />
      ) : pending.status === "removed" || pending.status === "done" ? (
        <CheckCircle2 className="w-3.5 h-3.5 text-lunar-healthy shrink-0 mt-0.5" />
      ) : (
        <AlertTriangle className="w-3.5 h-3.5 text-lunar-warning shrink-0 mt-0.5" />
      )}
      <p className="flex-1 text-[11px] text-lunar-text leading-relaxed">{pending.message}</p>
      {pending.status === "removed" && (
        <button
          type="button"
          onClick={onReviewLeftovers}
          className="shrink-0 px-2.5 py-1 rounded text-[11px] font-medium bg-lunar-elevated hover:bg-lunar-border text-lunar-white border border-lunar-border transition-colors cursor-pointer"
        >
          Review leftovers
        </button>
      )}
      {pending.status !== "working" && pending.status !== "waiting" && (
        <button
          type="button"
          onClick={onDismiss}
          aria-label="Dismiss"
          className="p-0.5 rounded text-lunar-muted hover:text-lunar-white transition-colors cursor-pointer"
        >
          <X className="w-3.5 h-3.5" />
        </button>
      )}
    </div>
  );
};

export const UnusedAppsView: React.FC<UnusedAppsViewProps> = ({ onOpenInExplorer, onReviewLeftovers }) => {
  const phase = useAppsStore((s) => s.phase);
  const result = useAppsStore((s) => s.result);
  const error = useAppsStore((s) => s.error);
  const thresholdDays = useAppsStore((s) => s.thresholdDays);
  const showNoRecord = useAppsStore((s) => s.showNoRecord);
  const confirm = useAppsStore((s) => s.confirm);
  const pending = useAppsStore((s) => s.pending);
  const initialize = useAppsStore((s) => s.initialize);
  const scan = useAppsStore((s) => s.scan);
  const setThreshold = useAppsStore((s) => s.setThreshold);
  const setShowNoRecord = useAppsStore((s) => s.setShowNoRecord);
  const askRemove = useAppsStore((s) => s.askRemove);
  const cancelRemove = useAppsStore((s) => s.cancelRemove);
  const confirmRemove = useAppsStore((s) => s.confirmRemove);
  const ignore = useAppsStore((s) => s.ignore);
  const dismissPending = useAppsStore((s) => s.dismissPending);

  useEffect(() => {
    let cleanup: (() => void) | undefined;
    let mounted = true;
    void initialize().then((u) => {
      if (mounted) cleanup = u;
      else u();
    });
    return () => {
      mounted = false;
      if (cleanup) cleanup();
    };
  }, [initialize]);

  const { unused, noRecord } = useMemo(() => {
    const cutoff = Date.now() - thresholdDays * 86_400_000;
    const apps = result?.apps ?? [];
    return {
      unused: apps.filter((a) => a.lastUsedMs !== null && a.lastUsedMs <= cutoff),
      noRecord: apps.filter((a) => a.lastUsedMs === null),
    };
  }, [result, thresholdDays]);
  const unusedBytes = unused.reduce((acc, a) => acc + (a.sizeBytes ?? 0), 0);
  const thresholdLabel = THRESHOLDS.find((t) => t.days === thresholdDays)?.label ?? `${thresholdDays} days`;

  const renderRow = (app: UnusedApp) => {
    const p = pending[app.id];
    const busy = p?.status === "working" || p?.status === "waiting";
    const gone = p?.status === "removed";
    return (
      <div key={app.id} className={`px-4 py-3 rounded-xl border bg-lunar-surface/70 border-lunar-border ${gone ? "opacity-70" : ""}`}>
        <div className="flex items-center gap-3">
          <KindIcon app={app} />
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2 min-w-0">
              <span className="text-xs font-semibold text-lunar-white truncate" title={app.name}>
                {app.name}
              </span>
              <span className="font-mono text-[10px] px-1.5 py-0.5 rounded bg-lunar-bg text-lunar-text-sec border border-lunar-border shrink-0">
                {KIND_LABELS[app.kind]}
              </span>
              {app.needsAdmin && (
                <span title="Windows may ask for administrator permission" className="cursor-help shrink-0">
                  <ShieldAlert className="w-3.5 h-3.5 text-lunar-warning" />
                </span>
              )}
            </div>
            <div className="text-[11px] text-lunar-text-sec truncate mt-0.5">
              {app.publisher && <span className="mr-2">{app.publisher}</span>}
              <span
                className="cursor-help"
                title={app.lastUsedSource ? `From ${USAGE_SOURCE_LABELS[app.lastUsedSource]}` : "No program from this app was seen running or launched"}
              >
                {formatAgo(app.lastUsedMs)}
                {app.lastUsedMs ? ` (${formatStorageTimestamp(app.lastUsedMs)})` : ""}
              </span>
              {app.installedMs && <span className="ml-2 text-lunar-muted">Installed {formatStorageTimestamp(app.installedMs)}</span>}
            </div>
            {app.note && <div className="text-[10px] text-lunar-text-sec/80 mt-1">{app.note}</div>}
          </div>

          <div className="w-20 shrink-0 text-right">
            <div className="text-[10px] text-lunar-muted">Size</div>
            <div className="text-xs font-mono font-semibold text-lunar-white">
              {app.sizeBytes !== null ? formatStorageBytes(app.sizeBytes) : app.installDir ? "Measuring" : "Unknown"}
            </div>
          </div>

          <div className="flex items-center gap-1.5 shrink-0">
            {app.installDir && (
              <button
                type="button"
                onClick={() => onOpenInExplorer(app.installDir!)}
                title="Open its folder in Explorer"
                aria-label="Open its folder in Explorer"
                className="inline-flex items-center justify-center w-7 h-7 rounded-lg bg-lunar-surface hover:bg-lunar-elevated text-lunar-text-sec hover:text-lunar-white border border-lunar-border transition-colors cursor-pointer"
              >
                <FolderOpen className="w-3 h-3" />
              </button>
            )}
            {app.fallback && (
              <button
                type="button"
                disabled={busy || gone}
                onClick={() => askRemove(app, true)}
                className="inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg bg-lunar-surface hover:bg-lunar-elevated text-[11px] font-medium text-lunar-text-sec hover:text-lunar-white border border-lunar-border transition-colors cursor-pointer disabled:opacity-45 disabled:cursor-not-allowed"
              >
                {METHOD_LABELS[app.fallback]}
              </button>
            )}
            <button
              type="button"
              disabled={busy || gone}
              onClick={() => askRemove(app, false)}
              className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-lunar-elevated hover:bg-lunar-border text-[11px] font-medium text-lunar-white border border-lunar-border transition-colors cursor-pointer disabled:opacity-45 disabled:cursor-not-allowed"
            >
              <Trash2 className="w-3 h-3" />
              <span>{METHOD_LABELS[app.method]}</span>
            </button>
            <button
              type="button"
              disabled={busy}
              onClick={() => void ignore(app)}
              title="Don't suggest this app again"
              aria-label="Don't suggest this app again"
              className="inline-flex items-center justify-center w-7 h-7 rounded-lg bg-lunar-surface hover:bg-lunar-elevated text-lunar-text-sec hover:text-lunar-white border border-lunar-border transition-colors cursor-pointer disabled:opacity-45 disabled:cursor-not-allowed"
            >
              <EyeOff className="w-3 h-3" />
            </button>
          </div>
        </div>
        {p && <PendingNote pending={p} onDismiss={() => dismissPending(app.id)} onReviewLeftovers={onReviewLeftovers} />}
      </div>
    );
  };

  return (
    <div className="space-y-5">
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
        <div>
          <h2 className="text-sm font-semibold text-lunar-white tracking-tight">Apps you haven't used in a while</h2>
          <p className="text-xs text-lunar-text-sec mt-0.5">
            Remove an app with its own uninstaller. ASAO never uninstalls anything without your click.
          </p>
        </div>
        <button
          type="button"
          disabled={phase === "scanning"}
          onClick={() => void scan()}
          className={`inline-flex items-center gap-2 px-3.5 py-2 rounded text-xs font-medium border transition-colors shrink-0 ${
            phase === "scanning"
              ? "bg-lunar-surface text-lunar-muted border-lunar-border cursor-not-allowed"
              : "bg-lunar-elevated hover:bg-lunar-border text-lunar-white border-lunar-border cursor-pointer"
          }`}
        >
          <RefreshCw className={`w-3.5 h-3.5 ${phase === "scanning" ? "animate-spin" : ""}`} />
          <span>{result ? "Scan again" : "Find unused apps"}</span>
        </button>
      </div>

      {error && phase !== "error" && (
        <div className="flex items-start gap-2.5 px-3.5 py-2.5 rounded-lg bg-lunar-critical/10 border border-lunar-critical/30">
          <AlertTriangle className="w-3.5 h-3.5 text-lunar-critical shrink-0 mt-0.5" />
          <p className="text-[11px] text-lunar-text leading-relaxed">{error}</p>
        </div>
      )}

      {phase === "scanning" && (
        <div className="rounded-lg lunar-glass-card p-10 flex flex-col items-center justify-center text-center">
          <Loader2 className="w-7 h-7 text-lunar-text-sec mb-3 animate-spin" />
          <p className="text-sm font-medium text-lunar-white">Checking when each app was last used</p>
          <p className="text-xs text-lunar-text-sec mt-1">Reading installed apps and Windows' launch records. This takes a few seconds.</p>
        </div>
      )}

      {phase === "idle" && !result && (
        <div className="rounded-lg lunar-glass-card p-10 flex flex-col items-center justify-center text-center">
          <Hourglass className="w-8 h-8 text-lunar-muted mb-3 stroke-[1.5]" />
          <p className="text-sm font-medium text-lunar-white">No scan yet.</p>
          <p className="text-xs text-lunar-text-sec mt-1 max-w-md">
            ASAO checks when each installed app was last used, so you can remove the ones you no longer need.
          </p>
          <button
            type="button"
            onClick={() => void scan()}
            className="mt-4 inline-flex items-center gap-2 px-4 py-2 rounded bg-lunar-elevated hover:bg-lunar-border text-xs font-medium text-lunar-white border border-lunar-border transition-colors cursor-pointer"
          >
            Find unused apps
          </button>
        </div>
      )}

      {phase === "error" && (
        <div className="rounded-lg lunar-glass-card p-10 flex flex-col items-center justify-center text-center">
          <AlertTriangle className="w-8 h-8 text-lunar-critical mb-3 stroke-[1.5]" />
          <p className="text-sm font-medium text-lunar-white">{error || "Unable to check installed apps."}</p>
          <button
            type="button"
            onClick={() => void scan()}
            className="mt-4 inline-flex items-center gap-2 px-4 py-2 rounded bg-lunar-elevated hover:bg-lunar-border text-xs font-medium text-lunar-white border border-lunar-border transition-colors cursor-pointer"
          >
            <RefreshCw className="w-3.5 h-3.5" />
            <span>Try again</span>
          </button>
        </div>
      )}

      {result && phase !== "scanning" && (
        <>
          <div className="flex items-start gap-2.5 px-3.5 py-2.5 rounded-lg bg-lunar-surface border border-lunar-border">
            <Info className="w-3.5 h-3.5 text-lunar-text-sec shrink-0 mt-0.5" />
            <p className="text-[11px] text-lunar-text-sec leading-relaxed">
              "Last used" comes from Windows' record of programs you launched
              {result.prefetchAvailable ? ", Windows Prefetch," : ""} and the programs ASAO has seen running
              {result.trackingSinceMs ? ` since ${formatStorageTimestamp(result.trackingSinceMs)}` : ""}. Apps you only
              start from a terminal or that run in the background may show no record. Apps running right now are not listed.
              {!result.prefetchAvailable && " Running ASAO as administrator also reads Windows Prefetch for better dates."}
            </p>
          </div>

          <div className="rounded-xl lunar-glass-card p-5 flex flex-col sm:flex-row sm:items-center justify-between gap-4">
            <div>
              <div className="text-sm font-semibold text-lunar-white">
                {unused.length} {unused.length === 1 ? "app" : "apps"} not used in {thresholdLabel}
                {unusedBytes > 0 && `, ${formatStorageBytes(unusedBytes)}`}.
              </div>
              <div className="text-[11px] text-lunar-text-sec mt-1">
                Checked {result.considered.toLocaleString()} apps. {noRecord.length} have no record of use.
              </div>
            </div>
            <div className="flex items-center gap-1 p-1 rounded-lg bg-lunar-surface border border-lunar-border shrink-0">
              {THRESHOLDS.map((t) => (
                <button
                  key={t.days}
                  type="button"
                  onClick={() => setThreshold(t.days)}
                  className={`px-2.5 py-1 rounded-md text-[11px] font-medium border transition-colors cursor-pointer ${
                    thresholdDays === t.days
                      ? "bg-lunar-elevated text-lunar-white border-lunar-border"
                      : "text-lunar-text-sec hover:text-lunar-text border-transparent"
                  }`}
                >
                  {t.label}
                </button>
              ))}
            </div>
          </div>

          {unused.length === 0 ? (
            <div className="rounded-lg lunar-glass-card p-8 flex flex-col items-center justify-center text-center">
              <CheckCircle2 className="w-7 h-7 text-lunar-healthy mb-3 stroke-[1.5]" />
              <p className="text-sm font-medium text-lunar-white">Every app with a record was used in the last {thresholdLabel}.</p>
            </div>
          ) : (
            <div className="space-y-2">{unused.map(renderRow)}</div>
          )}

          {noRecord.length > 0 && (
            <div className="space-y-2">
              <button
                type="button"
                onClick={() => setShowNoRecord(!showNoRecord)}
                className="w-full flex items-center gap-2 px-1 py-1.5 text-left cursor-pointer"
              >
                {showNoRecord ? (
                  <ChevronDown className="w-4 h-4 text-lunar-white" />
                ) : (
                  <ChevronRight className="w-4 h-4 text-lunar-muted" />
                )}
                <span className="text-xs font-semibold uppercase tracking-wider text-lunar-white">No record of use</span>
                <span className="font-mono text-[10px] px-1.5 py-0.5 rounded bg-lunar-bg text-lunar-text-sec border border-lunar-border">
                  {noRecord.length}
                </span>
                <span className="text-[11px] text-lunar-text-sec truncate">
                  ASAO has not seen these run yet. Some may run in the background. Check before removing.
                </span>
              </button>
              {showNoRecord && noRecord.map(renderRow)}
            </div>
          )}
        </>
      )}

      {confirm && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-sm"
          onClick={cancelRemove}
          role="dialog"
          aria-modal="true"
          aria-labelledby="unused-confirm-title"
        >
          <div
            className="w-full max-w-lg rounded-xl bg-lunar-surface border border-lunar-border shadow-2xl flex flex-col overflow-hidden"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="px-5 py-4 border-b border-lunar-border flex items-start justify-between gap-4">
              <div>
                <h2 id="unused-confirm-title" className="text-sm font-semibold text-lunar-white tracking-tight">
                  {confirm.method === "remove-folder"
                    ? `Remove ${confirm.app.name}'s program folder?`
                    : confirm.method === "windows-settings" || confirm.method === "open-epic"
                    ? `Remove ${confirm.app.name}`
                    : `Uninstall ${confirm.app.name}?`}
                </h2>
                <p className="text-xs text-lunar-text-sec mt-1">{formatAgo(confirm.app.lastUsedMs)}.</p>
              </div>
              <button
                type="button"
                onClick={cancelRemove}
                aria-label="Close"
                className="p-1.5 rounded-md text-lunar-muted hover:text-lunar-white hover:bg-lunar-elevated transition-colors cursor-pointer"
              >
                <X className="w-4 h-4" />
              </button>
            </div>
            <div className="p-5 space-y-3">
              <p className="text-[11px] text-lunar-text leading-relaxed">{methodExplanation(confirm.method, confirm.app.name)}</p>
              {confirm.method === "remove-folder" && confirm.app.installDir && (
                <div className="font-mono text-[10px] text-lunar-muted break-all px-3 py-2 rounded-lg bg-lunar-bg border border-lunar-border">
                  {confirm.app.installDir}
                </div>
              )}
              <p className="text-[10px] text-lunar-muted leading-relaxed">
                Afterwards, the Leftovers tab shows any folders the app left behind, matched with high confidence.
              </p>
            </div>
            <div className="px-5 py-3 border-t border-lunar-border bg-lunar-surface-2/40 flex items-center justify-end gap-2">
              <button
                type="button"
                onClick={cancelRemove}
                className="px-3.5 py-2 rounded text-xs font-medium text-lunar-text-sec hover:text-lunar-white hover:bg-lunar-elevated transition-colors cursor-pointer"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={() => void confirmRemove()}
                className="inline-flex items-center gap-2 px-3.5 py-2 rounded text-xs font-medium border bg-lunar-white text-lunar-bg border-lunar-white hover:bg-lunar-text transition-colors cursor-pointer"
              >
                <Trash2 className="w-3.5 h-3.5" />
                <span>{METHOD_LABELS[confirm.method]}</span>
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
