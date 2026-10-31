import React, { useEffect, useMemo } from "react";
import { AlertTriangle, FlaskConical, Info, ListChecks, PackageCheck, PackageSearch, RefreshCw, Trash2, X } from "lucide-react";
import { inPreset, isRecommended, selectedItems, selectedTweakIds, useBloatStore, type BloatTab } from "../stores/bloat-store";
import { TweaksView } from "../components/bloat/TweaksView";
import { restartExplorer } from "../services/bloat";
import type { BloatCategory, BloatItem, BloatSafety } from "../types/bloat";
import { CATEGORY_LABELS, SAFETY_LABELS } from "../types/bloat";
import { formatStorageBytes } from "../services/storage";
import { BloatRow } from "../components/bloat/BloatRow";
import { BloatDetails } from "../components/bloat/BloatDetails";
import { BloatConfirmDialog } from "../components/bloat/BloatConfirmDialog";
import { BloatReportDialog } from "../components/bloat/BloatReportDialog";
import { RecentChanges } from "../components/bloat/RecentChanges";
import { useProcessStore } from "../stores/process-store";
import { useStorageStore } from "../stores/storage-store";
import { useResidualStore } from "../stores/residual-store";

/** A short name for a Win11Debloat preset, and what it covers (for the tooltip). */
function presetLabel(name: string): { label: string; covers: string | null } {
  const covers = name.match(/\(([^)]*)\)/)?.[1] ?? null;
  const base = name.replace(/\s*\([^)]*\)\s*/, "").trim();
  if (/oem/i.test(base)) return { label: "PC maker apps", covers };
  return { label: base, covers };
}

const TABS: { id: BloatTab; label: string }[] = [
  { id: "recommended", label: "Recommended" },
  { id: "apps", label: "All apps" },
  { id: "services", label: "Services" },
  { id: "tweaks", label: "Windows tweaks" },
  { id: "recent", label: "Recent changes" },
];

function inTab(item: BloatItem, tab: BloatTab): boolean {
  switch (tab) {
    case "recommended":
      return isRecommended(item);
    case "apps":
      return item.kind === "store-app" || item.kind === "win32-app";
    case "services":
      return item.services.length > 0;
    case "tweaks":
    case "recent":
      return false;
  }
}

/** Background services that would stop, and startup entries that would be switched off. */
function effects(items: BloatItem[]) {
  let bytes = 0;
  let services = 0;
  let startup = 0;
  for (const i of items) {
    bytes += i.sizeBytes ?? 0;
    services += i.services.filter((s) => s.state === "Running" || s.startType === "Auto").length;
    startup += i.startup.filter((s) => s.enabled).length;
  }
  return { bytes, services, startup };
}

const selectClass =
  "h-8 px-2.5 rounded-lg bg-lunar-surface border border-lunar-border text-[11px] text-lunar-text focus:outline-none focus:border-lunar-text-sec cursor-pointer";

export const DebloatPage: React.FC = () => {
  const phase = useBloatStore((s) => s.phase);
  const result = useBloatStore((s) => s.result);
  const progress = useBloatStore((s) => s.progress);
  const error = useBloatStore((s) => s.error);
  const selected = useBloatStore((s) => s.selected);
  const tab = useBloatStore((s) => s.tab);
  const safetyFilter = useBloatStore((s) => s.safetyFilter);
  const categoryFilter = useBloatStore((s) => s.categoryFilter);
  const detailId = useBloatStore((s) => s.detailId);
  const initialize = useBloatStore((s) => s.initialize);
  const scan = useBloatStore((s) => s.scan);
  const cancelScan = useBloatStore((s) => s.cancelScan);
  const toggle = useBloatStore((s) => s.toggle);
  const selectedTweaks = useBloatStore((s) => s.selectedTweaks);
  const groupChoices = useBloatStore((s) => s.groupChoices);
  const toggleTweak = useBloatStore((s) => s.toggleTweak);
  const chooseGroupOption = useBloatStore((s) => s.chooseGroupOption);
  const applyPreset = useBloatStore((s) => s.applyPreset);
  const setTab = useBloatStore((s) => s.setTab);
  const setSafetyFilter = useBloatStore((s) => s.setSafetyFilter);
  const setCategoryFilter = useBloatStore((s) => s.setCategoryFilter);
  const openDetail = useBloatStore((s) => s.openDetail);
  const info = useBloatStore((s) => s.info);
  const dryRun = useBloatStore((s) => s.dryRun);
  const setDryRun = useBloatStore((s) => s.setDryRun);
  const confirmOpen = useBloatStore((s) => s.confirmOpen);
  const plan = useBloatStore((s) => s.plan);
  const removeProvisioned = useBloatStore((s) => s.removeProvisioned);
  const continueWithoutRestorePoint = useBloatStore((s) => s.continueWithoutRestorePoint);
  const applyProgress = useBloatStore((s) => s.applyProgress);
  const report = useBloatStore((s) => s.report);
  const history = useBloatStore((s) => s.history);
  const undoing = useBloatStore((s) => s.undoing);
  const undoMessages = useBloatStore((s) => s.undoMessages);
  const openConfirm = useBloatStore((s) => s.openConfirm);
  const closeConfirm = useBloatStore((s) => s.closeConfirm);
  const setRemoveProvisioned = useBloatStore((s) => s.setRemoveProvisioned);
  const setContinueWithoutRestorePoint = useBloatStore((s) => s.setContinueWithoutRestorePoint);
  const apply = useBloatStore((s) => s.apply);
  const closeReport = useBloatStore((s) => s.closeReport);
  const undo = useBloatStore((s) => s.undo);
  const setRoute = useProcessStore((s) => s.setRoute);
  const setStorageView = useStorageStore((s) => s.setActiveView);

  // Uninstalled apps now count as "recently uninstalled", so their folders show as High.
  const findLeftovers = () => {
    closeReport();
    setStorageView("leftovers");
    setRoute("storage");
    void useResidualStore.getState().scan();
  };

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

  const items = result?.items ?? [];
  const recommended = useMemo(() => items.filter(isRecommended), [items]);
  const summary = effects(recommended);
  const picked = selectedItems({ result, selected });
  const pickedEffects = effects(picked);
  const pickedTweaks = selectedTweakIds({ selectedTweaks, groupChoices }).length;
  const nothingPicked = picked.length === 0 && pickedTweaks === 0;
  const visible = useMemo(
    () =>
      items
        .filter((i) => inTab(i, tab))
        .filter((i) => safetyFilter === "all" || i.safety === safetyFilter)
        .filter((i) => categoryFilter === "all" || i.category === categoryFilter)
        .sort((a, b) => b.impact.total - a.impact.total || a.name.localeCompare(b.name)),
    [items, tab, safetyFilter, categoryFilter]
  );
  const counts = useMemo(() => {
    const c: Record<BloatTab, number> = {
      recommended: 0,
      apps: 0,
      services: 0,
      tweaks: result?.tweaks?.tweaks.filter((t) => !t.groupId).length ?? 0,
      recent: history.length,
    };
    for (const i of items) for (const t of TABS) if (inTab(i, t.id)) c[t.id] += 1;
    return c;
  }, [items, history, result]);
  const detail = items.find((i) => i.id === detailId) ?? null;
  const scanning = phase === "scanning";
  const applying = phase === "applying";
  const busy = scanning || applying;
  const dryRunLocked = Boolean(info?.dryRunForced);

  return (
    <div className="flex-1 overflow-y-auto p-6 space-y-5">
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-end justify-between gap-3">
        <div>
          <h1 className="text-lg font-semibold text-lunar-white tracking-tight">Debloat</h1>
          <p className="text-xs text-lunar-text-sec mt-0.5">
            Preinstalled apps, PC maker extras and trials, and the background services and startup entries they add.
          </p>
        </div>
        <div className="flex items-center gap-3 shrink-0">
          {(info?.dryRunToggleVisible || dryRunLocked) && (
            <button
              type="button"
              onClick={() => setDryRun(!dryRun)}
              disabled={busy || dryRunLocked}
              title={
                dryRunLocked
                  ? "Preview only is forced on by ASAO's settings or a command-line flag"
                  : "Shows what would change and writes it to a log, without changing anything on this PC"
              }
              className={`inline-flex items-center gap-2 h-9 px-3 rounded-lg text-xs font-medium border transition-colors cursor-pointer disabled:opacity-60 disabled:cursor-not-allowed ${
                dryRun
                  ? "bg-lunar-elevated text-lunar-white border-lunar-white/40"
                  : "bg-lunar-surface text-lunar-text-sec border-lunar-border hover:text-lunar-white"
              }`}
            >
              <FlaskConical className="w-3.5 h-3.5" />
              <span>Preview only</span>
              <span
                className={`w-7 h-4 rounded-full border relative transition-colors ${
                  dryRun ? "bg-lunar-white border-lunar-white" : "bg-lunar-bg border-lunar-border"
                }`}
              >
                <span
                  className={`absolute top-0.5 block w-2.5 h-2.5 rounded-full transition-transform ${
                    dryRun ? "translate-x-3.5 bg-lunar-bg" : "translate-x-0.5 bg-lunar-text-sec"
                  }`}
                />
              </span>
            </button>
          )}
          <button
            type="button"
            disabled={busy}
            onClick={() => void scan()}
            className={`inline-flex items-center gap-2 px-3.5 py-2 rounded text-xs font-medium border transition-colors shrink-0 ${
              busy
                ? "bg-lunar-surface text-lunar-muted border-lunar-border cursor-not-allowed"
                : "bg-lunar-elevated hover:bg-lunar-border text-lunar-white border-lunar-border cursor-pointer"
            }`}
          >
            <RefreshCw className={`w-3.5 h-3.5 ${scanning ? "animate-spin" : ""}`} />
            <span>{result ? "Scan again" : "Scan for bloatware"}</span>
          </button>
        </div>
      </div>

      {dryRun && result && !scanning && (
        <div className="flex items-start gap-2.5 px-3.5 py-2.5 rounded-lg bg-lunar-surface border border-lunar-border">
          <FlaskConical className="w-3.5 h-3.5 text-lunar-text-sec shrink-0 mt-0.5" />
          <p className="text-[11px] text-lunar-text-sec leading-relaxed">
            Preview only is on: nothing on this PC will be changed. Removing only shows and logs what would change.{" "}
            {dryRunLocked ? "It is forced on by ASAO's settings or a command-line flag." : "Turn it off to make real changes."}
          </p>
        </div>
      )}

      {/* Applying */}
      {applying && (
        <div className="rounded-xl lunar-glass-card p-6 space-y-3">
          <div className="text-sm font-semibold text-lunar-white">{dryRun ? "Previewing changes" : "Making changes"}</div>
          <div className="text-[11px] text-lunar-text-sec truncate">
            {applyProgress?.current ?? "Starting"}
            {!dryRun && " · An uninstaller's own window may appear. ASAO waits for it."}
          </div>
          <div className="h-1.5 rounded-full bg-lunar-elevated overflow-hidden">
            {applyProgress && applyProgress.total > 0 ? (
              <div
                className="h-full bg-lunar-white transition-all duration-300"
                style={{ width: `${(applyProgress.done / applyProgress.total) * 100}%` }}
              />
            ) : (
              <div className="h-full w-1/3 bg-lunar-white/70 animate-pulse" />
            )}
          </div>
        </div>
      )}

      {error && phase !== "error" && (
        <div className="flex items-start gap-2.5 px-3.5 py-2.5 rounded-lg bg-lunar-critical/10 border border-lunar-critical/30">
          <AlertTriangle className="w-3.5 h-3.5 text-lunar-critical shrink-0 mt-0.5" />
          <p className="text-[11px] text-lunar-text leading-relaxed">{error}</p>
        </div>
      )}

      {/* Scanning */}
      {scanning && (
        <div className="rounded-xl lunar-glass-card p-6 space-y-3">
          <div className="flex items-center justify-between gap-3">
            <div className="min-w-0">
              <div className="text-sm font-semibold text-lunar-white">Scanning this PC</div>
              <div className="text-[11px] text-lunar-text-sec mt-0.5 truncate">
                {progress?.message ?? "Starting"}
                {progress?.phase === "sampling" && " (about 30 seconds, you can keep using ASAO)"}
              </div>
            </div>
            <button
              type="button"
              onClick={() => void cancelScan()}
              className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded text-xs font-medium border bg-lunar-surface text-lunar-text-sec border-lunar-border hover:text-lunar-white cursor-pointer"
            >
              <X className="w-3.5 h-3.5" />
              <span>Cancel</span>
            </button>
          </div>
          <div className="h-1.5 rounded-full bg-lunar-elevated overflow-hidden">
            {progress?.percent != null ? (
              <div className="h-full bg-lunar-white transition-all duration-300" style={{ width: `${progress.percent}%` }} />
            ) : (
              <div className="h-full w-1/3 bg-lunar-white/70 animate-pulse" />
            )}
          </div>
        </div>
      )}

      {/* Not scanned yet */}
      {phase === "idle" && !result && (
        <div className="rounded-lg lunar-glass-card p-10 flex flex-col items-center justify-center text-center">
          <PackageSearch className="w-8 h-8 text-lunar-muted mb-3 stroke-[1.5]" />
          <p className="text-sm font-medium text-lunar-white">No bloatware scan yet.</p>
          <p className="text-xs text-lunar-text-sec mt-1 max-w-md">
            ASAO lists preinstalled apps and background services, measures how much each one costs this PC, and
            explains whether it is safe to remove. Nothing is changed by scanning.
          </p>
          <button
            type="button"
            onClick={() => void scan()}
            className="mt-4 inline-flex items-center gap-2 px-4 py-2 rounded bg-lunar-elevated hover:bg-lunar-border text-xs font-medium text-lunar-white border border-lunar-border transition-colors cursor-pointer"
          >
            <span>Scan for bloatware</span>
          </button>
        </div>
      )}

      {/* Error */}
      {phase === "error" && (
        <div className="rounded-lg lunar-glass-card p-10 flex flex-col items-center justify-center text-center">
          <AlertTriangle className="w-8 h-8 text-lunar-critical mb-3 stroke-[1.5]" />
          <p className="text-sm font-medium text-lunar-white">{error || "Unable to complete the scan."}</p>
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

      {result && !busy && (
        <>
          {/* Summary */}
          <div className="rounded-xl lunar-glass-card p-5 flex flex-col sm:flex-row sm:items-center justify-between gap-4">
            <div>
              <div className="text-sm font-semibold text-lunar-white">
                {recommended.length === 0
                  ? "Nothing to recommend on this PC."
                  : `${recommended.length} recommended ${recommended.length === 1 ? "item" : "items"}.`}
              </div>
              <div className="text-[11px] text-lunar-text-sec mt-1">
                {recommended.length > 0
                  ? `Removing them frees about ${formatStorageBytes(summary.bytes)}${
                      summary.services > 0
                        ? ` and stops ${summary.services} background ${summary.services === 1 ? "service" : "services"}`
                        : ""
                    }${summary.startup > 0 ? ` and ${summary.startup} startup ${summary.startup === 1 ? "app" : "apps"}` : ""}.`
                  : "Everything ASAO is sure about has already been removed, or is in use."}{" "}
                Checked {result.stats.apps.toLocaleString()} apps and {result.stats.services.toLocaleString()} services.
              </div>
            </div>
            <div className="flex items-center gap-3 shrink-0">
              <div className="text-right">
                <div className="text-[10px] text-lunar-muted">Selected</div>
                <div className="text-xs font-mono font-semibold text-lunar-white">
                  {picked.length} · {formatStorageBytes(pickedEffects.bytes)}
                  {pickedTweaks > 0 && ` · ${pickedTweaks} ${pickedTweaks === 1 ? "setting" : "settings"}`}
                </div>
              </div>
              <button
                type="button"
                disabled={nothingPicked || busy}
                onClick={() => void openConfirm()}
                className={`inline-flex items-center gap-2 px-3.5 py-2 rounded text-xs font-medium border transition-colors ${
                  nothingPicked || busy
                    ? "bg-lunar-surface text-lunar-muted border-lunar-border cursor-not-allowed"
                    : "bg-lunar-white text-lunar-bg border-lunar-white hover:bg-lunar-text cursor-pointer"
                }`}
              >
                {applying ? (
                  <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                ) : dryRun ? (
                  <FlaskConical className="w-3.5 h-3.5" />
                ) : (
                  <Trash2 className="w-3.5 h-3.5" />
                )}
                <span>{applying ? "Working" : dryRun ? "Preview changes" : "Apply selected"}</span>
              </button>
            </div>
          </div>

          <div className="flex items-start gap-2.5 px-3.5 py-2.5 rounded-lg bg-lunar-surface border border-lunar-border">
            <Info className="w-3.5 h-3.5 text-lunar-text-sec shrink-0 mt-0.5" />
            <p className="text-[11px] text-lunar-text-sec leading-relaxed">
              ASAO only offers items rated Safe to remove or Usually safe. Anything uncertain, needed or unknown is shown
              for information and is never changed. Windows services are never ticked for you.
              {result.device.orgManaged && ` This PC is managed by an organisation (${result.device.managedReason}), so PC maker tools are kept.`}
              {!result.device.startupMeasured && " Startup impact is estimated: run ASAO as administrator to use Windows' own measurements."}
            </p>
          </div>

          {/* Tabs and filters */}
          <div className="flex flex-col lg:flex-row lg:items-center justify-between gap-3">
            <div className="flex items-center gap-1 p-1 rounded-lg bg-lunar-surface border border-lunar-border w-fit">
              {TABS.map((t) => (
                <button
                  key={t.id}
                  type="button"
                  onClick={() => setTab(t.id)}
                  className={`inline-flex items-center gap-2 px-3 py-1.5 rounded-md text-xs font-medium transition-colors cursor-pointer ${
                    tab === t.id ? "bg-lunar-elevated text-lunar-white" : "text-lunar-text-sec hover:text-lunar-white"
                  }`}
                >
                  <span>{t.label}</span>
                  <span className="font-mono text-[10px] text-lunar-muted">{counts[t.id]}</span>
                </button>
              ))}
            </div>
            <div className={`flex items-center gap-2 ${tab === "recent" || tab === "tweaks" ? "invisible" : ""}`}>
              <select
                aria-label="Filter by safety"
                value={safetyFilter}
                onChange={(e) => setSafetyFilter(e.target.value as BloatSafety | "all")}
                className={selectClass}
              >
                <option value="all">All safety levels</option>
                {(Object.keys(SAFETY_LABELS) as BloatSafety[]).map((s) => (
                  <option key={s} value={s}>
                    {SAFETY_LABELS[s]}
                  </option>
                ))}
              </select>
              <select
                aria-label="Filter by category"
                value={categoryFilter}
                onChange={(e) => setCategoryFilter(e.target.value as BloatCategory | "all")}
                className={selectClass}
              >
                <option value="all">All categories</option>
                {(Object.keys(CATEGORY_LABELS) as BloatCategory[]).map((c) => (
                  <option key={c} value={c}>
                    {CATEGORY_LABELS[c]}
                  </option>
                ))}
              </select>
            </div>
          </div>

          {/* Quick select: Win11Debloat's app groups */}
          {(tab === "recommended" || tab === "apps") && (result.presets ?? []).length > 0 && (
            <div className="flex flex-col sm:flex-row sm:items-center gap-2 px-3.5 py-2.5 rounded-lg bg-lunar-surface border border-lunar-border">
              <div className="flex items-center gap-2 min-w-0 sm:mr-auto">
                <ListChecks className="w-3.5 h-3.5 text-lunar-text-sec shrink-0" />
                <span className="text-xs font-medium text-lunar-white shrink-0">Quick select</span>
                <span className="text-[11px] text-lunar-text-sec truncate">Tick a whole group of related apps at once</span>
              </div>
              <div className="flex items-center gap-2 flex-wrap">
                {result.presets.map((p) => {
                  const count = items.filter((i) => i.selectable && inPreset(i, p.appIds)).length;
                  const { label, covers } = presetLabel(p.name);
                  const tip =
                    count === 0
                      ? "None of these apps are on this PC, or none can be removed"
                      : `Ticks ${count} ${count === 1 ? "app" : "apps"} on this PC${covers ? ` from ${covers}` : ""}`;
                  return (
                    <button
                      key={p.name}
                      type="button"
                      disabled={busy || count === 0}
                      onClick={() => applyPreset(p.name)}
                      title={tip}
                      className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded text-[11px] font-medium border bg-lunar-bg hover:bg-lunar-elevated text-lunar-text border-lunar-border transition-colors cursor-pointer disabled:opacity-45 disabled:cursor-not-allowed"
                    >
                      <span>{label}</span>
                      <span className="font-mono text-[10px] text-lunar-muted">{count}</span>
                    </button>
                  );
                })}
              </div>
            </div>
          )}

          {/* List */}
          {tab === "tweaks" ? (
            <TweaksView
              tweaks={result.tweaks ?? { tweaks: [], groups: [], notOffered: [] }}
              selected={selectedTweaks}
              groupChoices={groupChoices}
              busy={busy}
              onToggle={toggleTweak}
              onChoose={chooseGroupOption}
            />
          ) : tab === "recent" ? (
            <RecentChanges
              batches={history}
              undoing={undoing}
              messages={undoMessages}
              onUndo={(b, a) => void undo(b, a)}
            />
          ) : visible.length === 0 ? (
            <div className="rounded-lg lunar-glass-card p-10 flex flex-col items-center justify-center text-center">
              <PackageCheck className="w-8 h-8 text-lunar-healthy mb-3 stroke-[1.5]" />
              <p className="text-sm font-medium text-lunar-white">
                {tab === "recommended" ? "Nothing to recommend." : "Nothing matches these filters."}
              </p>
              {tab === "recommended" && (
                <p className="text-xs text-lunar-text-sec mt-1 max-w-md">
                  Look under All apps, Services and Startup to see everything ASAO checked and why it was kept.
                </p>
              )}
            </div>
          ) : (
            <div className="space-y-2">
              {visible.map((i) => (
                <BloatRow
                  key={i.id}
                  item={i}
                  selected={Boolean(selected[i.id])}
                  onToggle={() => toggle(i)}
                  onOpen={() => openDetail(i.id)}
                />
              ))}
            </div>
          )}

        </>
      )}

      {detail && <BloatDetails item={detail} onClose={() => openDetail(null)} />}

      {confirmOpen && plan && (
        <BloatConfirmDialog
          plan={plan}
          dryRun={dryRun}
          hasStoreApps={picked.some((i) => i.kind === "store-app")}
          removeProvisioned={removeProvisioned}
          continueWithoutRestorePoint={continueWithoutRestorePoint}
          onSetRemoveProvisioned={(v) => void setRemoveProvisioned(v)}
          onSetContinueWithoutRestorePoint={setContinueWithoutRestorePoint}
          onCancel={closeConfirm}
          onConfirm={() => void apply()}
        />
      )}

      {report && (
        <BloatReportDialog
          report={report}
          onFindLeftovers={findLeftovers}
          onRestartExplorer={() => void restartExplorer()}
          onClose={closeReport}
        />
      )}
    </div>
  );
};
