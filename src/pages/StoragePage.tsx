import React, { useEffect, useMemo, useRef } from "react";
import { RefreshCw, HardDrive, AlertTriangle } from "lucide-react";
import { useStorageStore } from "../stores/storage-store";
import type { WidgetRecommendation } from "../types/widget";
import { DriveOverview } from "../components/storage/DriveOverview";
import { StorageDistribution } from "../components/storage/StorageDistribution";
import { LargestFolders } from "../components/storage/LargestFolders";
import { LargestFiles } from "../components/storage/LargestFiles";
import { FilesystemExplorer } from "../components/storage/FilesystemExplorer";
import { FileDetailsPanel } from "../components/storage/FileDetailsPanel";
import { ScanProgress } from "../components/storage/ScanProgress";
import { RecButtonAndModal } from "../components/common/RecommendationsModal";

export const StoragePage: React.FC = () => {
  const drives = useStorageStore((s) => s.drives);
  const selectedDrive = useStorageStore((s) => s.selectedDrive);
  const scanState = useStorageStore((s) => s.scanState);
  const scanProgress = useStorageStore((s) => s.scanProgress);
  const snapshot = useStorageStore((s) => s.snapshot);
  const errorMessage = useStorageStore((s) => s.errorMessage);

  const currentPath = useStorageStore((s) => s.currentPath);
  const directoryListing = useStorageStore((s) => s.directoryListing);
  const isLoadingDirectory = useStorageStore((s) => s.isLoadingDirectory);
  const searchQuery = useStorageStore((s) => s.searchQuery);
  const searchResults = useStorageStore((s) => s.searchResults);
  const isSearching = useStorageStore((s) => s.isSearching);

  const categoryFilter = useStorageStore((s) => s.categoryFilter);
  const importanceFilter = useStorageStore((s) => s.importanceFilter);
  const typeFilter = useStorageStore((s) => s.typeFilter);
  const sortField = useStorageStore((s) => s.sortField);
  const sortDirection = useStorageStore((s) => s.sortDirection);
  const largestFilesSort = useStorageStore((s) => s.largestFilesSort);

  const selectedItem = useStorageStore((s) => s.selectedItem);

  const initializeStorage = useStorageStore((s) => s.initializeStorage);
  const selectDrive = useStorageStore((s) => s.selectDrive);
  const triggerScan = useStorageStore((s) => s.triggerScan);
  const cancelScan = useStorageStore((s) => s.cancelScan);
  const navigateToDirectory = useStorageStore((s) => s.navigateToDirectory);
  const retryCurrentDirectory = useStorageStore((s) => s.retryCurrentDirectory);
  const setSearchQuery = useStorageStore((s) => s.setSearchQuery);
  const setCategoryFilter = useStorageStore((s) => s.setCategoryFilter);
  const setImportanceFilter = useStorageStore((s) => s.setImportanceFilter);
  const setTypeFilter = useStorageStore((s) => s.setTypeFilter);
  const setSort = useStorageStore((s) => s.setSort);
  const setLargestFilesSort = useStorageStore((s) => s.setLargestFilesSort);
  const selectItem = useStorageStore((s) => s.selectItem);
  const resetExplorerFilters = useStorageStore((s) => s.resetExplorerFilters);
  const openLocationInExplorer = useStorageStore(
    (s) => s.openLocationInExplorer
  );

  const explorerSectionRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let cleanup: (() => void) | undefined;
    let mounted = true;

    void initializeStorage().then((unlisten) => {
      if (mounted) {
        cleanup = unlisten;
      } else {
        unlisten();
      }
    });

    return () => {
      mounted = false;
      if (cleanup) cleanup();
    };
  }, [initializeStorage]);

  const activeDriveObj =
    drives.find(
      (d) => d.drive.toUpperCase() === (selectedDrive ?? "").toUpperCase()
    ) ??
    drives[0] ??
    null;

  const handleOpenFolderInExplorer = (path: string) => {
    const clean = path.replace(/\//g, "\\");
    // Check if path matches a known file or looks like a file with an extension
    const matchedFile = snapshot?.largestFiles.find(
      (f) => f.path.toLowerCase() === clean.toLowerCase()
    );
    const lastSlash = clean.lastIndexOf("\\");
    const lastSeg = lastSlash >= 0 ? clean.slice(lastSlash + 1) : clean;
    const isLikelyFile =
      Boolean(matchedFile && !matchedFile.isDir) ||
      (lastSeg.includes(".") && !lastSeg.startsWith("."));

    const dirToOpen =
      isLikelyFile && lastSlash > 2 ? clean.slice(0, lastSlash) : clean;

    if (matchedFile) {
      selectItem(matchedFile);
    }

    void navigateToDirectory(dirToOpen);
    setTimeout(() => {
      explorerSectionRef.current?.scrollIntoView({
        behavior: "smooth",
        block: "start",
      });
    }, 50);
  };

  const isScanning = scanState === "scanning";

  const fallbackStorageRecommendations = useMemo<WidgetRecommendation[]>(() => {
    const list: WidgetRecommendation[] = [];
    if (snapshot?.largestFiles) {
      for (const f of snapshot.largestFiles.slice(0, 4)) {
        const sizeMb = Math.round(f.size / (1024 * 1024));
        if (sizeMb < 15) continue;
        if (f.category === "USER") {
          list.push({
            id: `storage-user-${f.path}`,
            category: "storage",
            subCategory: "Dormant User File (1+ Year Inactive)",
            title: `Dormant User File: ${f.name}`,
            message: `"${f.name}" is occupying huge space (${sizeMb} MB) and there has been no activity on this file since 1 year. Because this file is user-specific (${f.path}), you should review it and archive or delete it if no longer needed.`,
            priority: sizeMb >= 250 ? "important" : "interesting",
            actionLabel: "Review User File in Explorer",
            processPid: null,
            processName: null,
            storagePath: f.path,
            metricHighlight: `${sizeMb} MB · Dormant`,
          });
        } else if (f.category === "APPLICATION") {
          list.push({
            id: `storage-app-${f.path}`,
            category: "storage",
            subCategory: "Orphaned Application File",
            title: `Uninstalled App File: ${f.name}`,
            message: `"${f.name}" (${f.path}) is an application file occupying ${sizeMb} MB, and if the corresponding application is not present on this PC and you don't need it in the future, consider deleting it.`,
            priority: "interesting",
            actionLabel: "Consider Deleting Leftover App File",
            processPid: null,
            processName: null,
            storagePath: f.path,
            metricHighlight: `${sizeMb} MB · App File`,
          });
        }
      }
    }
    return list;
  }, [snapshot]);

  return (
    <div className="flex-1 flex min-h-0 overflow-hidden">
      {/* Main Scrollable Storage Content */}
      <div className="flex-1 overflow-y-auto p-6 space-y-6">
        {/* 1. Overall Storage Page Header */}
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
          <div>
            <h1 className="text-lg font-semibold text-lunar-white tracking-tight">
              Storage
            </h1>
            <p className="text-xs text-lunar-text-sec mt-0.5">
              Analyze your disk usage and understand what's consuming your
              space.
            </p>
          </div>

          <div className="flex items-center gap-3 shrink-0">
            <RecButtonAndModal
              category="storage"
              fallbackRecommendations={fallbackStorageRecommendations}
              onOpenStorageInExplorer={(p) => void openLocationInExplorer(p)}
              onInspectStoragePath={(p) => handleOpenFolderInExplorer(p)}
            />

            <button
              type="button"
              disabled={isScanning || drives.length === 0}
              onClick={() => void triggerScan(selectedDrive ?? undefined)}
              className={`inline-flex items-center gap-2 px-3.5 py-2 rounded text-xs font-medium border transition-colors shrink-0 ${
                isScanning || drives.length === 0
                  ? "bg-lunar-surface text-lunar-muted border-lunar-border cursor-not-allowed"
                  : "bg-lunar-elevated hover:bg-lunar-border text-lunar-white border-lunar-border cursor-pointer"
              }`}
            >
              <RefreshCw
                className={`w-3.5 h-3.5 ${isScanning ? "animate-spin" : ""}`}
              />
              <span>Rescan</span>
            </button>
          </div>
        </div>

        {/* 2. Drive Overview */}
        <DriveOverview
          drives={drives}
          selectedDrive={selectedDrive}
          onSelectDrive={(drv) => void selectDrive(drv)}
        />

        {/* 3. Scanning Experience */}
        {isScanning && (
          <div className="py-4">
            <ScanProgress
              progress={scanProgress}
              onCancel={() => void cancelScan()}
            />
          </div>
        )}

        {/* 4. Empty State: No Scan Yet */}
        {!isScanning && !snapshot && scanState === "unscanned" && drives.length > 0 && (
          <div className="rounded-lg lunar-glass-card p-10 flex flex-col items-center justify-center text-center">
            <HardDrive className="w-8 h-8 text-lunar-muted mb-3 stroke-[1.5]" />
            <p className="text-sm font-medium text-lunar-white">
              Storage hasn't been analyzed yet.
            </p>
            <button
              type="button"
              onClick={() => void triggerScan(selectedDrive ?? undefined)}
              className="mt-4 inline-flex items-center gap-2 px-4 py-2 rounded bg-lunar-elevated hover:bg-lunar-border text-xs font-medium text-lunar-white border border-lunar-border transition-colors cursor-pointer"
            >
              <span>Scan Storage</span>
            </button>
          </div>
        )}

        {/* 5. Error State */}
        {!isScanning && !snapshot && scanState === "error" && (
          <div className="rounded-lg lunar-glass-card p-10 flex flex-col items-center justify-center text-center">
            <AlertTriangle className="w-8 h-8 text-lunar-critical mb-3 stroke-[1.5]" />
            <p className="text-sm font-medium text-lunar-white">
              {errorMessage || "Unable to complete storage analysis."}
            </p>
            <button
              type="button"
              onClick={() => void triggerScan(selectedDrive ?? undefined)}
              className="mt-4 inline-flex items-center gap-2 px-4 py-2 rounded bg-lunar-elevated hover:bg-lunar-border text-xs font-medium text-lunar-white border border-lunar-border transition-colors cursor-pointer"
            >
              <RefreshCw className="w-3.5 h-3.5" />
              <span>Scan Storage</span>
            </button>
          </div>
        )}

        {/* 6. Full Dynamic Analysis View */}
        {snapshot && !isScanning && (
          <>
            {/* Stretched Full-Width Storage Distribution */}
            <StorageDistribution
              distribution={snapshot.distribution}
              totalUsedBytes={
                activeDriveObj?.usedCapacity ?? snapshot.bytesAnalyzed
              }
              activeCategoryFilter={categoryFilter}
              onSelectCategory={(cat) => {
                setCategoryFilter(cat);
                if (cat !== "ALL") {
                  explorerSectionRef.current?.scrollIntoView({
                    behavior: "smooth",
                    block: "start",
                  });
                }
              }}
            />

            {/* Largest Folders & Largest Files */}
            <div className="grid grid-cols-1 xl:grid-cols-2 gap-4">
              <LargestFolders
                folders={snapshot.largestFolders}
                currentExplorerPath={currentPath}
                onOpenFolder={handleOpenFolderInExplorer}
                onSelectFolder={(folder) => selectItem(folder)}
              />

              <LargestFiles
                files={snapshot.largestFiles}
                sortField={largestFilesSort}
                onSortChange={setLargestFilesSort}
                selectedPath={selectedItem?.path ?? null}
                onSelectFile={(file) => selectItem(file)}
              />
            </div>

            {/* Dedicated Filesystem Explorer */}
            <div ref={explorerSectionRef} className="pt-2">
              <FilesystemExplorer
                currentPath={currentPath}
                directoryListing={directoryListing}
                isLoadingDirectory={isLoadingDirectory}
                searchQuery={searchQuery}
                searchResults={searchResults}
                isSearching={isSearching}
                categoryFilter={categoryFilter}
                importanceFilter={importanceFilter}
                typeFilter={typeFilter}
                discoveredExtensions={snapshot.discoveredExtensions}
                sortField={sortField}
                sortDirection={sortDirection}
                selectedItem={selectedItem}
                onNavigate={(path) => void navigateToDirectory(path)}
                onRetryDirectory={() => void retryCurrentDirectory()}
                onSearchChange={setSearchQuery}
                onCategoryChange={setCategoryFilter}
                onImportanceChange={setImportanceFilter}
                onTypeChange={setTypeFilter}
                onSortChange={setSort}
                onSelectItem={(item) =>
                  selectItem(
                    selectedItem?.path === item.path ? null : item
                  )
                }
                onResetFilters={resetExplorerFilters}
              />
            </div>
          </>
        )}
      </div>

      {/* Right-Hand File Details Inspector */}
      {selectedItem && (
        <FileDetailsPanel
          item={selectedItem}
          onClose={() => selectItem(null)}
          onOpenLocation={(path) => void openLocationInExplorer(path)}
          onBrowseFolder={(path) => {
            void navigateToDirectory(path);
            explorerSectionRef.current?.scrollIntoView({
              behavior: "smooth",
              block: "start",
            });
          }}
        />
      )}
    </div>
  );
};
