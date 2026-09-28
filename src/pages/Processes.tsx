import React, { useMemo } from "react";
import { AlertTriangle, RefreshCw } from "lucide-react";
import { useProcessStore } from "../stores/process-store";
import { ProcessCategoryTabs } from "../components/processes/ProcessCategoryTabs";
import { ProcessFilters } from "../components/processes/ProcessFilters";
import { ProcessTable } from "../components/processes/ProcessTable";
import { ProcessDetails } from "../components/processes/ProcessDetails";
import { CATEGORY_METADATA, type ProcessStatus } from "../types/process";

const STATUS_PRIORITY: Record<ProcessStatus, number> = {
  attention: 0,
  "high-resource": 1,
  active: 2,
  normal: 3,
  background: 4,
  protected: 5,
};

export const Processes: React.FC = () => {
  const processes = useProcessStore((s) => s.processes);
  const systemMetrics = useProcessStore((s) => s.systemMetrics);
  const selectedProcessPid = useProcessStore((s) => s.selectedProcessPid);
  const resourceHistory = useProcessStore((s) => s.resourceHistory);

  const searchQuery = useProcessStore((s) => s.searchQuery);
  const categoryFilter = useProcessStore((s) => s.categoryFilter);
  const multiCategoryFilter = useProcessStore((s) => s.multiCategoryFilter);
  const statusFilter = useProcessStore((s) => s.statusFilter);
  const resourceFilter = useProcessStore((s) => s.resourceFilter);
  const criticalityFilter = useProcessStore((s) => s.criticalityFilter);
  const sort = useProcessStore((s) => s.sort);

  const monitoringStatus = useProcessStore((s) => s.monitoringStatus);
  const errorMessage = useProcessStore((s) => s.errorMessage);
  const retryMonitoring = useProcessStore((s) => s.retryMonitoring);

  const selectProcess = useProcessStore((s) => s.selectProcess);
  const setSearchQuery = useProcessStore((s) => s.setSearchQuery);
  const setCategoryFilter = useProcessStore((s) => s.setCategoryFilter);
  const toggleMultiCategory = useProcessStore((s) => s.toggleMultiCategory);
  const toggleStatusFilter = useProcessStore((s) => s.toggleStatusFilter);
  const setResourceFilter = useProcessStore((s) => s.setResourceFilter);
  const setCriticalityFilter = useProcessStore((s) => s.setCriticalityFilter);
  const setSort = useProcessStore((s) => s.setSort);
  const resetFilters = useProcessStore((s) => s.resetFilters);

  const filteredAndSortedProcesses = useMemo(() => {
    const query = searchQuery.trim().toLowerCase();

    const filtered = processes.filter((proc) => {
      // 1. Category tab filter
      if (categoryFilter !== "all" && proc.category !== categoryFilter) {
        return false;
      }

      // 2. Multi-category popover filter
      if (
        multiCategoryFilter.length > 0 &&
        !multiCategoryFilter.includes(proc.category)
      ) {
        return false;
      }

      // 3. Status filter
      if (statusFilter.length > 0 && !statusFilter.includes(proc.status)) {
        return false;
      }

      // 4. Resource usage filter
      if (resourceFilter === "cpu-25" && proc.cpuPercent <= 25) return false;
      if (resourceFilter === "cpu-50" && proc.cpuPercent <= 50) return false;
      if (
        resourceFilter === "ram-500" &&
        proc.memoryBytes <= 500 * 1024 * 1024
      ) {
        return false;
      }
      if (
        resourceFilter === "ram-1024" &&
        proc.memoryBytes <= 1024 * 1024 * 1024
      ) {
        return false;
      }

      // 5. System criticality filter
      if (criticalityFilter === "system-critical" && !proc.isSystemCritical) {
        return false;
      }
      if (criticalityFilter === "user-space" && proc.isSystemCritical) {
        return false;
      }

      // 6. Search query matching name, publisher, product, path, or category
      if (query.length > 0) {
        const catLabel =
          CATEGORY_METADATA[proc.category]?.label.toLowerCase() ?? "";
        const matchName = proc.name.toLowerCase().includes(query);
        const matchPub = (proc.publisher ?? "").toLowerCase().includes(query);
        const matchProd = (proc.productName ?? "").toLowerCase().includes(query);
        const matchPath = (proc.executablePath ?? "")
          .toLowerCase()
          .includes(query);
        const matchCat =
          proc.category.toLowerCase().includes(query) ||
          catLabel.includes(query);
        const matchPid = String(proc.pid).includes(query);

        if (
          !matchName &&
          !matchPub &&
          !matchProd &&
          !matchPath &&
          !matchCat &&
          !matchPid
        ) {
          return false;
        }
      }

      return true;
    });

    // Sort
    const sorted = [...filtered];
    sorted.sort((a, b) => {
      switch (sort) {
        case "cpu-desc":
          return b.cpuPercent - a.cpuPercent || b.memoryBytes - a.memoryBytes;
        case "cpu-asc":
          return a.cpuPercent - b.cpuPercent;
        case "memory-desc":
          return b.memoryBytes - a.memoryBytes || b.cpuPercent - a.cpuPercent;
        case "memory-asc":
          return a.memoryBytes - b.memoryBytes;
        case "name-asc":
          return a.name.localeCompare(b.name);
        case "name-desc":
          return b.name.localeCompare(a.name);
        case "category":
          return (
            a.category.localeCompare(b.category) ||
            b.memoryBytes - a.memoryBytes
          );
        case "status":
          return (
            (STATUS_PRIORITY[a.status] ?? 99) -
              (STATUS_PRIORITY[b.status] ?? 99) ||
            b.cpuPercent - a.cpuPercent
          );
        case "resource-desc":
        default: {
          const scoreA =
            a.cpuPercent * 25 + a.memoryBytes / (1024 * 1024);
          const scoreB =
            b.cpuPercent * 25 + b.memoryBytes / (1024 * 1024);
          return scoreB - scoreA;
        }
      }
    });

    return sorted;
  }, [
    processes,
    searchQuery,
    categoryFilter,
    multiCategoryFilter,
    statusFilter,
    resourceFilter,
    criticalityFilter,
    sort,
  ]);

  const selectedProcess = useMemo(
    () =>
      selectedProcessPid !== null
        ? processes.find((p) => p.pid === selectedProcessPid) ?? null
        : null,
    [processes, selectedProcessPid]
  );

  const selectedSamples = useMemo(
    () =>
      selectedProcessPid !== null
        ? resourceHistory[selectedProcessPid] ?? []
        : [],
    [resourceHistory, selectedProcessPid]
  );

  // Loading State
  if (monitoringStatus === "loading" && processes.length === 0) {
    return (
      <div className="flex-1 p-6 flex flex-col items-center justify-center text-center">
        <div className="w-8 h-8 rounded-full border-2 border-lunar-border border-t-lunar-white animate-spin mb-4" />
        <h2 className="text-sm font-medium text-lunar-white">
          Analyzing system...
        </h2>
        <p className="text-xs text-lunar-muted mt-1 font-mono">
          Collecting running processes
        </p>
      </div>
    );
  }

  // Error State
  if (monitoringStatus === "error" && processes.length === 0) {
    return (
      <div className="flex-1 p-6 flex flex-col items-center justify-center text-center max-w-md mx-auto">
        <AlertTriangle className="w-8 h-8 text-lunar-critical mb-3 stroke-[1.5]" />
        <h2 className="text-sm font-semibold text-lunar-white">
          Unable to read process information.
        </h2>
        <p className="text-xs text-lunar-text-sec mt-1.5 leading-relaxed">
          {errorMessage ??
            "Some Windows processes may require elevated permissions."}
        </p>
        <button
          type="button"
          onClick={() => void retryMonitoring()}
          className="mt-4 inline-flex items-center gap-2 px-4 py-2 rounded bg-lunar-elevated hover:bg-lunar-border text-xs font-medium text-lunar-white border border-lunar-border transition-colors cursor-pointer"
        >
          <RefreshCw className="w-3.5 h-3.5" />
          <span>Retry</span>
        </button>
      </div>
    );
  }

  const totalRunning = systemMetrics?.totalProcesses ?? processes.length;

  return (
    <div className="flex-1 flex min-h-0 overflow-hidden">
      {/* Main Process Analyzer Column */}
      <div className="flex-1 flex flex-col min-w-0 min-h-0 p-6 space-y-4 overflow-hidden">
        {/* Header */}
        <div className="flex flex-col sm:flex-row sm:items-end justify-between gap-2 shrink-0">
          <div>
            <h1 className="text-lg font-semibold text-lunar-white tracking-tight">
              Background Processes
            </h1>
            <div className="flex items-center gap-3 text-xs text-lunar-text-sec mt-0.5 font-mono">
              <span>{totalRunning} processes running</span>
              {filteredAndSortedProcesses.length !== processes.length && (
                <>
                  <span className="text-lunar-border">•</span>
                  <span className="text-lunar-white">
                    Showing {filteredAndSortedProcesses.length}
                  </span>
                </>
              )}
            </div>
          </div>
        </div>

        {/* Search, Filter & Sort Controls */}
        <div className="shrink-0">
          <ProcessFilters
            searchQuery={searchQuery}
            onSearchChange={setSearchQuery}
            multiCategoryFilter={multiCategoryFilter}
            onToggleCategory={toggleMultiCategory}
            statusFilter={statusFilter}
            onToggleStatus={toggleStatusFilter}
            resourceFilter={resourceFilter}
            onResourceFilterChange={setResourceFilter}
            criticalityFilter={criticalityFilter}
            onCriticalityFilterChange={setCriticalityFilter}
            sort={sort}
            onSortChange={setSort}
            onResetAll={resetFilters}
          />
        </div>

        {/* Category Tabs */}
        <div className="shrink-0">
          <ProcessCategoryTabs
            processes={processes}
            activeCategory={categoryFilter}
            onSelectCategory={setCategoryFilter}
          />
        </div>

        {/* Process Table */}
        <ProcessTable
          processes={filteredAndSortedProcesses}
          selectedPid={selectedProcessPid}
          onSelectProcess={(pid) =>
            selectProcess(selectedProcessPid === pid ? null : pid)
          }
          onResetFilters={resetFilters}
        />
      </div>

      {/* Right-Hand Process Details Side Panel */}
      {selectedProcess && (
        <ProcessDetails
          process={selectedProcess}
          samples={selectedSamples}
          onClose={() => selectProcess(null)}
        />
      )}
    </div>
  );
};
