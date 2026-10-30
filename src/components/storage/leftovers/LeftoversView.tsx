import React, { useEffect, useMemo } from "react";
import {
  AlertTriangle,
  ChevronDown,
  ChevronRight,
  FlaskConical,
  EyeOff,
  Info,
  PackageCheck,
  PackageSearch,
  RefreshCw,
  Trash2,
} from "lucide-react";
import { isSelected, selectedCandidates, useResidualStore } from "../../../stores/residual-store";
import type { ResidualCandidate, ResidualGroup } from "../../../types/residual";
import { formatStorageBytes } from "../../../services/storage";
import { openRecycleBin } from "../../../services/residual";
import { LeftoverGroupCard, totalSize } from "./LeftoverGroupCard";
import { LeftoverConfirmDialog } from "./LeftoverConfirmDialog";
import { LeftoverReportDialog } from "./LeftoverReportDialog";
import { LeftoverScanProgress } from "./LeftoverScanProgress";

interface LeftoversViewProps {
  onOpenInExplorer: (path: string) => void;
}

interface Section {
  group: ResidualGroup;
  folders: ResidualCandidate[];
}

function splitSections(groups: ResidualGroup[]): { confident: Section[]; uncertain: Section[] } {
  const confident: Section[] = [];
  const uncertain: Section[] = [];
  for (const g of groups) {
    // Each group's folders largest first
    const folders = [...g.folders].sort((a, b) => (b.sizeBytes ?? -1) - (a.sizeBytes ?? -1));
    const sure = folders.filter((f) => f.confidence !== "low");
    const unsure = folders.filter((f) => f.confidence === "low");
    if (sure.length) confident.push({ group: g, folders: sure });
    if (unsure.length) uncertain.push({ group: g, folders: unsure });
  }
  // Largest first, with the unknown group after the named apps
  const bySize = (a: Section, b: Section) =>
    Number(a.group.isUnknown) - Number(b.group.isUnknown) ||
    (totalSize(b.folders) ?? 0) - (totalSize(a.folders) ?? 0);
  confident.sort(bySize);
  uncertain.sort(bySize);
  return { confident, uncertain };
}

export const LeftoversView: React.FC<LeftoversViewProps> = ({ onOpenInExplorer }) => {
  const phase = useResidualStore((s) => s.phase);
  const result = useResidualStore((s) => s.result);
  const progress = useResidualStore((s) => s.progress);
  const sizing = useResidualStore((s) => s.sizing);
  const error = useResidualStore((s) => s.error);
  const selected = useResidualStore((s) => s.selected);
  const expandedGroups = useResidualStore((s) => s.expandedGroups);
  const showUncertain = useResidualStore((s) => s.showUncertain);
  const dryRun = useResidualStore((s) => s.dryRun);
  const permanent = useResidualStore((s) => s.permanent);
  const confirmOpen = useResidualStore((s) => s.confirmOpen);
  const report = useResidualStore((s) => s.report);

  const initialize = useResidualStore((s) => s.initialize);
  const scan = useResidualStore((s) => s.scan);
  const cancelScan = useResidualStore((s) => s.cancelScan);
  const toggleFolder = useResidualStore((s) => s.toggleFolder);
  const setGroupSelected = useResidualStore((s) => s.setGroupSelected);
  const toggleExpanded = useResidualStore((s) => s.toggleExpanded);
  const setShowUncertain = useResidualStore((s) => s.setShowUncertain);
  const setDryRun = useResidualStore((s) => s.setDryRun);
  const setPermanent = useResidualStore((s) => s.setPermanent);
  const openConfirm = useResidualStore((s) => s.openConfirm);
  const closeConfirm = useResidualStore((s) => s.closeConfirm);
  const clean = useResidualStore((s) => s.clean);
  const cleanEmpty = useResidualStore((s) => s.cleanEmpty);
  const closeReport = useResidualStore((s) => s.closeReport);
  const ignoreFolder = useResidualStore((s) => s.ignoreFolder);
  const ignoreApp = useResidualStore((s) => s.ignoreApp);

  useEffect(() => {
    let cleanup: (() => void) | undefined;
    let mounted = true;
    void initialize().then((unlisten) => {
      if (mounted) cleanup = unlisten;
      else unlisten();
    });
    return () => {
      mounted = false;
      if (cleanup) cleanup();
    };
  }, [initialize]);

  const { confident, uncertain } = useMemo(
    () => splitSections(result?.groups ?? []),
    [result]
  );
  const picked = useMemo(() => selectedCandidates({ result, selected }), [result, selected]);
  const pickedSize = picked.reduce((acc, f) => acc + (f.sizeBytes ?? 0), 0);
  const allFolders = (result?.groups ?? []).flatMap((g) => g.folders);
  const totalBytes = allFolders.reduce((acc, f) => acc + (f.sizeBytes ?? 0), 0);
  const appCount = (result?.groups ?? []).filter((g) => !g.isUnknown).length;
  const unknownCount = (result?.groups ?? []).filter((g) => g.isUnknown).reduce((acc, g) => acc + g.folders.length, 0);
  const uncertainCount = uncertain.reduce((acc, s) => acc + s.folders.length, 0);
  const smallHidden = result?.stats.smallHidden ?? 0;
  const emptyCount = result?.emptyFolders.length ?? 0;
  const busy = phase === "scanning" || phase === "cleaning";
  const selectedFn = (path: string) => isSelected({ selected }, path);

  const renderSection = (sections: Section[], prefix: string) =>
    sections.map(({ group, folders }) => {
      const key = `${prefix}:${group.key}`;
      return (
        <LeftoverGroupCard
          key={key}
          group={group}
          folders={folders}
          expanded={Boolean(expandedGroups[key])}
          busy={busy}
          isSelected={selectedFn}
          onToggleExpand={() => toggleExpanded(key)}
          onToggleFolder={toggleFolder}
          onSetGroup={setGroupSelected}
          onOpenFolder={onOpenInExplorer}
          onIgnoreFolder={(p) => void ignoreFolder(p)}
          onIgnoreApp={(g) => void ignoreApp(g)}
        />
      );
    });

  return (
    <div className="space-y-5">
      {/* Header: what this is, dry run toggle, scan button */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
        <div>
          <h2 className="text-sm font-semibold text-lunar-white tracking-tight">Leftovers from uninstalled apps</h2>
          <p className="text-xs text-lunar-text-sec mt-0.5">
            Folders that apps left behind after you uninstalled them. Nothing is removed until you confirm.
          </p>
        </div>
        <div className="flex items-center gap-3 shrink-0">
          <button
            type="button"
            onClick={() => setDryRun(!dryRun)}
            disabled={busy}
            title="Dry run runs every check and writes what would be removed to a log, without deleting anything"
            className={`inline-flex items-center gap-2 h-9 px-3 rounded-lg text-xs font-medium border transition-colors cursor-pointer disabled:opacity-45 disabled:cursor-not-allowed ${
              dryRun
                ? "bg-lunar-elevated text-lunar-white border-lunar-white/40"
                : "bg-lunar-surface text-lunar-text-sec border-lunar-border hover:text-lunar-white"
            }`}
          >
            <FlaskConical className="w-3.5 h-3.5" />
            <span>Dry run</span>
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
            <RefreshCw className={`w-3.5 h-3.5 ${phase === "scanning" ? "animate-spin" : ""}`} />
            <span>{result ? "Scan again" : "Scan for leftovers"}</span>
          </button>
        </div>
      </div>

      {dryRun && result && phase !== "scanning" && (
        <div className="flex items-start gap-2.5 px-3.5 py-2.5 rounded-lg bg-lunar-surface border border-lunar-border">
          <FlaskConical className="w-3.5 h-3.5 text-lunar-text-sec shrink-0 mt-0.5" />
          <p className="text-[11px] text-lunar-text-sec leading-relaxed">
            Dry run is on. Removing only writes what would be deleted to the log. Turn it off to remove folders.
          </p>
        </div>
      )}

      {error && phase !== "error" && (
        <div className="flex items-start gap-2.5 px-3.5 py-2.5 rounded-lg bg-lunar-critical/10 border border-lunar-critical/30">
          <AlertTriangle className="w-3.5 h-3.5 text-lunar-critical shrink-0 mt-0.5" />
          <p className="text-[11px] text-lunar-text leading-relaxed">{error}</p>
        </div>
      )}

      {/* Scanning */}
      {phase === "scanning" && (
        <div className="py-4">
          <LeftoverScanProgress progress={progress} onCancel={() => void cancelScan()} />
        </div>
      )}

      {/* Not scanned yet */}
      {phase === "idle" && !result && (
        <div className="rounded-lg lunar-glass-card p-10 flex flex-col items-center justify-center text-center">
          <PackageSearch className="w-8 h-8 text-lunar-muted mb-3 stroke-[1.5]" />
          <p className="text-sm font-medium text-lunar-white">No leftover scan yet.</p>
          <p className="text-xs text-lunar-text-sec mt-1 max-w-md">
            ASAO checks Program Files, ProgramData and your AppData folders for folders that no installed app owns.
          </p>
          <button
            type="button"
            onClick={() => void scan()}
            className="mt-4 inline-flex items-center gap-2 px-4 py-2 rounded bg-lunar-elevated hover:bg-lunar-border text-xs font-medium text-lunar-white border border-lunar-border transition-colors cursor-pointer"
          >
            <span>Scan for leftovers</span>
          </button>
        </div>
      )}

      {/* Error */}
      {phase === "error" && (
        <div className="rounded-lg lunar-glass-card p-10 flex flex-col items-center justify-center text-center">
          <AlertTriangle className="w-8 h-8 text-lunar-critical mb-3 stroke-[1.5]" />
          <p className="text-sm font-medium text-lunar-white">{error || "Unable to complete the leftover scan."}</p>
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

      {/* Results */}
      {result && phase !== "scanning" && (
        <>
          {!result.stats.referencesAvailable && (
            <div className="flex items-start gap-2.5 px-3.5 py-2.5 rounded-lg bg-lunar-warning/10 border border-lunar-warning/30">
              <AlertTriangle className="w-3.5 h-3.5 text-lunar-warning shrink-0 mt-0.5" />
              <p className="text-[11px] text-lunar-text leading-relaxed">
                ASAO could not read services and scheduled tasks. You can review folders, but removal is blocked
                until a scan can check them.
              </p>
            </div>
          )}

          {result.stats.removedAppsKnown > 0 &&
            result.stats.removedFromEvidence === result.stats.removedAppsKnown && (
              <div className="flex items-start gap-2.5 px-3.5 py-2.5 rounded-lg bg-lunar-surface border border-lunar-border">
                <Info className="w-3.5 h-3.5 text-lunar-text-sec shrink-0 mt-0.5" />
                <p className="text-[11px] text-lunar-text-sec leading-relaxed">
                  ASAO found {result.stats.removedFromEvidence} {result.stats.removedFromEvidence === 1 ? "app" : "apps"} that
                  were uninstalled before it started tracking, from the Windows Installer log and the record of programs
                  you have launched. Their folders are matched by name. Folders of apps you uninstall from now on are
                  matched too.
                </p>
              </div>
            )}

          {result.stats.removedAppsKnown === 0 && (
            <div className="flex items-start gap-2.5 px-3.5 py-2.5 rounded-lg bg-lunar-surface border border-lunar-border">
              <Info className="w-3.5 h-3.5 text-lunar-text-sec shrink-0 mt-0.5" />
              <p className="text-[11px] text-lunar-text-sec leading-relaxed">
                ASAO keeps a record of installed apps from now on. Folders of apps you uninstall later are matched by
                name and shown with high confidence. Anything found today has no known owner, so it starts unticked.
              </p>
            </div>
          )}

          {allFolders.length === 0 ? (
            <div className="rounded-lg lunar-glass-card p-10 flex flex-col items-center justify-center text-center">
              <PackageCheck className="w-8 h-8 text-lunar-healthy mb-3 stroke-[1.5]" />
              <p className="text-sm font-medium text-lunar-white">No leftovers found.</p>
              <p className="text-xs text-lunar-text-sec mt-1">
                Checked {result.stats.foldersChecked.toLocaleString()} folders against{" "}
                {result.stats.installedApps.toLocaleString()} installed apps.
              </p>
            </div>
          ) : (
            <>
              {/* Summary */}
              <div className="rounded-xl lunar-glass-card p-5 flex flex-col sm:flex-row sm:items-center justify-between gap-4">
                <div>
                  <div className="text-sm font-semibold text-lunar-white">
                    {appCount > 0
                      ? `Found leftovers from ${appCount} ${appCount === 1 ? "app" : "apps"}, `
                      : `Found ${allFolders.length} possible leftover ${allFolders.length === 1 ? "folder" : "folders"}, `}
                    {formatStorageBytes(totalBytes)}
                    {sizing && <span className="text-lunar-muted font-normal"> so far</span>}.
                  </div>
                  <div className="text-[11px] text-lunar-text-sec mt-1">
                    {appCount > 0 && unknownCount > 0 &&
                      `${unknownCount} more ${unknownCount === 1 ? "folder has" : "folders have"} no known owner. `}
                    {appCount === 0 && "None of them match an app ASAO saw being uninstalled. "}
                    Checked {result.stats.foldersChecked.toLocaleString()} folders against{" "}
                    {result.stats.installedApps.toLocaleString()} installed apps.
                  </div>
                </div>
                <div className="flex items-center gap-3 shrink-0">
                  <div className="text-right">
                    <div className="text-[10px] text-lunar-muted">Selected</div>
                    <div className="text-xs font-mono font-semibold text-lunar-white">
                      {picked.length} · {formatStorageBytes(pickedSize)}
                    </div>
                  </div>
                  <button
                    type="button"
                    disabled={picked.length === 0 || busy}
                    onClick={openConfirm}
                    className={`inline-flex items-center gap-2 px-3.5 py-2 rounded text-xs font-medium border transition-colors ${
                      picked.length === 0 || busy
                        ? "bg-lunar-surface text-lunar-muted border-lunar-border cursor-not-allowed"
                        : "bg-lunar-white text-lunar-bg border-lunar-white hover:bg-lunar-text cursor-pointer"
                    }`}
                  >
                    {phase === "cleaning" ? (
                      <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                    ) : dryRun ? (
                      <FlaskConical className="w-3.5 h-3.5" />
                    ) : (
                      <Trash2 className="w-3.5 h-3.5" />
                    )}
                    <span>{phase === "cleaning" ? "Working" : dryRun ? "Dry run selected" : "Remove selected"}</span>
                  </button>
                </div>
              </div>

              {/* Confident items (High and Medium) */}
              {confident.length > 0 && <div className="space-y-2">{renderSection(confident, "sure")}</div>}

              {/* Uncertain items, collapsed by default */}
              {uncertain.length > 0 && (
                <div className="space-y-2">
                  <button
                    type="button"
                    onClick={() => setShowUncertain(!showUncertain)}
                    className="w-full flex items-center gap-2 px-1 py-1.5 text-left cursor-pointer"
                  >
                    {showUncertain ? (
                      <ChevronDown className="w-4 h-4 text-lunar-white" />
                    ) : (
                      <ChevronRight className="w-4 h-4 text-lunar-muted" />
                    )}
                    <span className="text-xs font-semibold uppercase tracking-wider text-lunar-white">Uncertain items</span>
                    <span className="font-mono text-[10px] px-1.5 py-0.5 rounded bg-lunar-bg text-lunar-text-sec border border-lunar-border">
                      {uncertainCount}
                    </span>
                    <span className="text-[11px] text-lunar-text-sec truncate">
                      No uninstalled app matches these. Review each one before removing it.
                    </span>
                  </button>
                  {showUncertain && renderSection(uncertain, "unsure")}
                </div>
              )}
            </>
          )}

          {/* Tiny uncertain folders: one line, plus a one-click clean of the empty ones */}
          {smallHidden > 0 && (
            <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 px-3.5 py-2.5 rounded-lg bg-lunar-surface border border-lunar-border">
              <div className="flex items-start gap-2.5 min-w-0">
                <EyeOff className="w-3.5 h-3.5 text-lunar-text-sec shrink-0 mt-0.5" />
                <p className="text-[11px] text-lunar-text-sec leading-relaxed">
                  {smallHidden.toLocaleString()} more uncertain {smallHidden === 1 ? "folder" : "folders"} under 1 MB
                  {" "}({formatStorageBytes(result.stats.smallHiddenBytes)} in total) {smallHidden === 1 ? "is" : "are"} not listed.
                  {emptyCount > 0 &&
                    ` ${emptyCount.toLocaleString()} of them ${emptyCount === 1 ? "is" : "are"} empty.`}
                </p>
              </div>
              {emptyCount > 0 && (
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => void cleanEmpty()}
                  title="Each folder is checked again and must still hold no files. They go to the Recycle Bin."
                  className={`inline-flex items-center gap-2 px-3 py-1.5 rounded text-xs font-medium border transition-colors shrink-0 ${
                    busy
                      ? "bg-lunar-surface text-lunar-muted border-lunar-border cursor-not-allowed"
                      : "bg-lunar-elevated hover:bg-lunar-border text-lunar-white border-lunar-border cursor-pointer"
                  }`}
                >
                  {dryRun ? <FlaskConical className="w-3.5 h-3.5" /> : <Trash2 className="w-3.5 h-3.5" />}
                  <span>
                    {dryRun ? "Dry run" : "Clean"} {emptyCount} empty {emptyCount === 1 ? "folder" : "folders"}
                  </span>
                </button>
              )}
            </div>
          )}
        </>
      )}

      {confirmOpen && (
        <LeftoverConfirmDialog
          items={picked}
          dryRun={dryRun}
          permanent={permanent}
          onSetPermanent={setPermanent}
          onCancel={closeConfirm}
          onConfirm={() => void clean()}
        />
      )}

      {report && (
        <LeftoverReportDialog
          report={report}
          onOpenRecycleBin={() => void openRecycleBin()}
          onClose={closeReport}
        />
      )}
    </div>
  );
};
