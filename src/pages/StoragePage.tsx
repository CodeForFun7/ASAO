import React, { useEffect, useRef } from "react";
import { RefreshCw, HardDrive, AlertTriangle } from "lucide-react";
import { useStorageStore } from "../stores/storage-store";
import { DriveOverview } from "../components/storage/DriveOverview";
import { StorageDistribution } from "../components/storage/StorageDistribution";
import { LargestFolders } from "../components/storage/LargestFolders";
import { LargestFiles } from "../components/storage/LargestFiles";
import { FilesystemExplorer } from "../components/storage/FilesystemExplorer";
import { FileDetailsPanel } from "../components/storage/FileDetailsPanel";
import { ScanProgress } from "../components/storage/ScanProgress";

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
    void navigateToDirectory(path);
    setTimeout(() => {
      explorerSectionRef.current?.scrollIntoView({
        behavior: "smooth",
        block: "start",
      });
    }, 50);
  };

  const isScanning = scanState === "scanning";

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
