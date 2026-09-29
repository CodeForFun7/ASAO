import React, { useEffect, useMemo, useState } from "react";
import { AlertTriangle, RefreshCw } from "lucide-react";
import { useStartupStore } from "../stores/startup-store";
import type {
  StartupGroup,
  StartupItem,
} from "../types/startup";
import type { WidgetRecommendation } from "../types/widget";
import { StartupCategoryTabs } from "../components/startup/StartupCategoryTabs";
import { StartupFilters } from "../components/startup/StartupFilters";
import { StartupTable } from "../components/startup/StartupTable";
import { StartupDetails } from "../components/startup/StartupDetails";
import { WprBootTraceModal } from "../components/startup/WprBootTraceModal";
import { RecButtonAndModal } from "../components/common/RecommendationsModal";

export const Startup: React.FC = () => {
  const items = useStartupStore((s) => s.items);
  const wprStatus = useStartupStore((s) => s.wprStatus);
  const selectedItemId = useStartupStore((s) => s.selectedItemId);
  const loading = useStartupStore((s) => s.loading);
  const actionInProgressId = useStartupStore((s) => s.actionInProgressId);
  const error = useStartupStore((s) => s.error);

  const searchQuery = useStartupStore((s) => s.searchQuery);
  const filterImpact = useStartupStore((s) => s.filterImpact);
  const filterRecommendation = useStartupStore((s) => s.filterRecommendation);
  const filterRunning = useStartupStore((s) => s.filterRunning);

  const loadStartupData = useStartupStore((s) => s.loadStartupData);
  const selectItem = useStartupStore((s) => s.selectItem);
  const toggleItemState = useStartupStore((s) => s.toggleItemState);
  const startBootTrace = useStartupStore((s) => s.startBootTrace);
  const cancelBootTrace = useStartupStore((s) => s.cancelBootTrace);

  const setSearchQuery = useStartupStore((s) => s.setSearchQuery);
  const setFilterImpact = useStartupStore((s) => s.setFilterImpact);
  const setFilterRecommendation = useStartupStore((s) => s.setFilterRecommendation);
  const setFilterRunning = useStartupStore((s) => s.setFilterRunning);
  const resetFilters = useStartupStore((s) => s.resetFilters);

  const [activeGroup, setActiveGroup] = useState<StartupGroup | "all">("all");
  const [wprModalOpen, setWprModalOpen] = useState(false);

  // Load items on initial mount
  useEffect(() => {
    if (items.length === 0 && !loading && !error) {
      void loadStartupData();
    }
  }, [items.length, loading, error, loadStartupData]);

  // Filter and sort items (Running & Enabled first!)
  const filteredAndSortedItems = useMemo(() => {
    const q = searchQuery.trim().toLowerCase();

    const filtered = items.filter((item) => {
      // 1. Category Tab Filter
      if (activeGroup !== "all" && item.startupGroup !== activeGroup) {
        return false;
      }

      // 2. Search Query
      if (q.length > 0) {
        const matchName = item.name.toLowerCase().includes(q);
        const matchPub = (item.publisher ?? "").toLowerCase().includes(q);
        const matchPath = (item.executablePath ?? "").toLowerCase().includes(q);
        const matchDesc = (item.description ?? "").toLowerCase().includes(q);
        if (!matchName && !matchPub && !matchPath && !matchDesc) {
          return false;
        }
      }

      // 3. Impact Filter
      if (filterImpact !== "all" && item.impact !== filterImpact) {
        return false;
      }

      // 4. Recommendation Filter
      if (
        filterRecommendation !== "all" &&
        item.recommendation !== filterRecommendation
      ) {
        return false;
      }

      // 5. Running Filter
      if (filterRunning === "running" && !item.isCurrentlyRunning) {
        return false;
      }
      if (filterRunning === "stopped" && item.isCurrentlyRunning) {
        return false;
      }

      return true;
    });

    // Reorder items: Running/Enabled first, then Inactive/Disabled
    return filtered.sort((a, b) => {
      const getTier = (item: StartupItem): number => {
        if (item.isEnabled && item.isCurrentlyRunning) return 0;
        if (item.isEnabled && !item.isCurrentlyRunning) return 1;
        if (!item.isEnabled && item.isCurrentlyRunning) return 2;
        return 3;
      };

      const tierA = getTier(a);
      const tierB = getTier(b);
      if (tierA !== tierB) {
        return tierA - tierB;
      }

      // Secondary: High impact first
      const impactOrder: Record<string, number> = { high: 0, medium: 1, low: 2 };
      const impactA = impactOrder[a.impact] ?? 3;
      const impactB = impactOrder[b.impact] ?? 3;
      if (impactA !== impactB) {
        return impactA - impactB;
      }

      // Tertiary: Alphabetical
      return a.name.localeCompare(b.name);
    });
  }, [
    items,
    activeGroup,
    searchQuery,
    filterImpact,
    filterRecommendation,
    filterRunning,
  ]);

  const selectedItem = useMemo(() => {
    if (!selectedItemId) return null;
    return items.find((i) => i.id === selectedItemId) ?? null;
  }, [items, selectedItemId]);

  const runningCount = items.filter((i) => i.isCurrentlyRunning).length;

  const fallbackStartupRecommendations = useMemo<WidgetRecommendation[]>(() => {
    return items
      .filter(
        (item) =>
          item.isEnabled &&
          item.classification !== "essential" &&
          (item.impact === "high" ||
            item.impact === "medium" ||
            item.recommendation === "disable" ||
            item.recommendation === "investigate")
      )
      .slice(0, 6)
      .map((item) => {
        const delaySec = Math.max(0.2, item.bootDurationMs / 1000).toFixed(1);
        const memMb =
          item.memoryBytes > 0
            ? `${Math.round(item.memoryBytes / (1024 * 1024))} MB RAM`
            : "Background Autostart";
        return {
          id: `startup-rec-${item.id}`,
          category: "startup",
          subCategory: "Heavy Non-Core Boot Program",
          title: `Heavy Non-Core Startup: ${item.name}`,
          message: `"${item.name}" is a heavy startup program (~${delaySec}s boot delay, ${memMb}) and isn't a core Windows program or required at Windows boot. We recommend disabling it from automatic startup so it only runs when you launch it.`,
          priority: item.impact === "high" ? "important" : "interesting",
          actionLabel: "Disable from Windows Boot",
          processPid: item.pid ?? null,
          processName: item.name,
          startupItemId: item.id,
          storagePath: item.executablePath ?? null,
          metricHighlight: `+${delaySec}s Boot · ${memMb}`,
        };
      });
  }, [items]);

  // Loading State
  if (loading && items.length === 0) {
    return (
      <div className="flex-1 p-6 flex flex-col items-center justify-center text-center">
        <RefreshCw className="w-8 h-8 text-lunar-ai animate-spin mb-3 stroke-[1.5]" />
        <h2 className="text-sm font-semibold text-lunar-white">
          Scanning Windows Startup Sources...
        </h2>
        <p className="text-xs text-lunar-text-sec mt-1.5 font-mono max-w-sm">
          Discovering Registry Run keys, Startup folders, scheduled tasks, and automatic services.
        </p>
      </div>
    );
  }

  // Error State
  if (error && items.length === 0) {
    return (
      <div className="flex-1 p-6 flex flex-col items-center justify-center text-center max-w-md mx-auto">
        <AlertTriangle className="w-8 h-8 text-lunar-critical mb-3 stroke-[1.5]" />
        <h2 className="text-sm font-semibold text-lunar-white">
          Unable to scan startup entries.
        </h2>
        <p className="text-xs text-lunar-text-sec mt-1.5 leading-relaxed font-mono">
          {error}
        </p>
        <button
          type="button"
          onClick={() => void loadStartupData()}
          className="mt-4 inline-flex items-center gap-2 px-4 py-2 rounded bg-lunar-elevated hover:bg-lunar-border text-xs font-medium text-lunar-white border border-lunar-border transition-colors cursor-pointer"
        >
          <RefreshCw className="w-3.5 h-3.5" />
          <span>Retry</span>
        </button>
      </div>
    );
  }

  return (
    <div className="flex-1 flex min-h-0 overflow-hidden">
      {/* Main Startup Analyzer Column matching Processes page */}
      <div className="flex-1 flex flex-col min-w-0 min-h-0 p-6 space-y-4 overflow-hidden">
        {/* Header */}
        <div className="flex flex-col sm:flex-row sm:items-end justify-between gap-2 shrink-0">
          <div>
            <h1 className="text-lg font-semibold text-lunar-white tracking-tight">
              Startup Applications & Services
            </h1>
            <div className="flex items-center gap-3 text-xs text-lunar-text-sec mt-0.5 font-mono">
              <span>{items.length} startup items</span>
              <span className="text-lunar-border">•</span>
              <span className="text-lunar-healthy">{runningCount} active in memory</span>
              {filteredAndSortedItems.length !== items.length && (
                <>
                  <span className="text-lunar-border">•</span>
                  <span className="text-lunar-white">
                    Showing {filteredAndSortedItems.length}
                  </span>
                </>
              )}
            </div>
          </div>
        </div>

        {/* Search, REC, Filter Controls matching ProcessFilters */}
        <div className="shrink-0">
          <StartupFilters
            searchQuery={searchQuery}
            onSearchChange={setSearchQuery}
            impactFilter={filterImpact}
            onImpactChange={setFilterImpact}
            recommendationFilter={filterRecommendation}
            onRecommendationChange={setFilterRecommendation}
            runningFilter={filterRunning}
            onRunningChange={setFilterRunning}
            isLoading={loading}
            onRefresh={() => void loadStartupData()}
            onOpenWprModal={() => setWprModalOpen(true)}
            onResetAll={resetFilters}
            recSlot={
              <RecButtonAndModal
                category="startup"
                fallbackRecommendations={fallbackStartupRecommendations}
                onInspectStartup={(id) => selectItem(id)}
                onDisableStartup={(id) => {
                  const target = items.find((i) => i.id === id);
                  if (target && target.isEnabled) {
                    void toggleItemState(target);
                  }
                }}
              />
            }
          />
        </div>

        {/* Category Tabs matching ProcessCategoryTabs */}
        <div className="shrink-0">
          <StartupCategoryTabs
            items={items}
            activeGroup={activeGroup}
            onSelectGroup={setActiveGroup}
          />
        </div>

        {/* Startup Table */}
        <StartupTable
          items={filteredAndSortedItems}
          selectedId={selectedItemId}
          onSelectItem={(id) => selectItem(id)}
          onResetFilters={resetFilters}
        />
      </div>

      {/* Obsidian-Style Centered Startup Details Modal Window */}
      {selectedItem && (
        <StartupDetails
          item={selectedItem}
          isActionLoading={actionInProgressId === selectedItem.id}
          onClose={() => selectItem(null)}
          onToggleState={() => void toggleItemState(selectedItem)}
        />
      )}

      {/* WPR Boot Trace Modal */}
      <WprBootTraceModal
        status={wprStatus}
        isOpen={wprModalOpen}
        onClose={() => setWprModalOpen(false)}
        onStartTrace={startBootTrace}
        onCancelTrace={cancelBootTrace}
      />
    </div>
  );
};
