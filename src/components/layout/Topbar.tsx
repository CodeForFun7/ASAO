import React from "react";
import { Search, Pause, Play, Radio, X } from "lucide-react";
import { useProcessStore } from "../../stores/process-store";

export const Topbar: React.FC = () => {
  const searchQuery = useProcessStore((s) => s.searchQuery);
  const setSearchQuery = useProcessStore((s) => s.setSearchQuery);
  const currentRoute = useProcessStore((s) => s.currentRoute);
  const setRoute = useProcessStore((s) => s.setRoute);
  const monitoringStatus = useProcessStore((s) => s.monitoringStatus);
  const togglePauseMonitoring = useProcessStore((s) => s.togglePauseMonitoring);
  const systemMetrics = useProcessStore((s) => s.systemMetrics);

  const handleGlobalSearchChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = e.target.value;
    setSearchQuery(val);
    if (val.trim().length > 0 && currentRoute !== "processes") {
      setRoute("processes");
    }
  };

  return (
    <header className="h-12 bg-lunar-surface border-b border-lunar-border px-4 flex items-center justify-between shrink-0 select-none">
      {/* Left: Brand Identity */}
      <div className="flex items-center gap-3 w-56">
        <div className="flex items-center gap-2.5">
          <span className="relative flex h-2.5 w-2.5 items-center justify-center">
            <span className="h-2.5 w-2.5 rounded-full border border-lunar-white bg-lunar-white/20" />
            <span className="absolute h-1 w-1 rounded-full bg-lunar-white" />
          </span>
          <span className="font-mono text-xs font-semibold tracking-[0.2em] text-lunar-white">
            SYSTEMA
          </span>
        </div>
        <span className="text-[10px] font-mono uppercase tracking-wider px-1.5 py-0.5 rounded bg-lunar-surface-2 text-lunar-muted border border-lunar-border">
          INSTRUMENT
        </span>
      </div>

      {/* Center: Global Search Input */}
      <div className="flex-1 max-w-md mx-4">
        <div className="relative flex items-center">
          <Search className="w-3.5 h-3.5 text-lunar-muted absolute left-3 pointer-events-none" />
          <input
            type="text"
            value={searchQuery}
            onChange={handleGlobalSearchChange}
            placeholder="Search processes, publishers, paths, or categories..."
            className="w-full h-8 pl-8 pr-7 bg-lunar-bg border border-lunar-border rounded-md text-xs text-lunar-text placeholder:text-lunar-muted focus:outline-none focus:border-lunar-text-sec transition-colors"
          />
          {searchQuery && (
            <button
              type="button"
              onClick={() => setSearchQuery("")}
              className="absolute right-2 text-lunar-muted hover:text-lunar-text p-0.5"
              title="Clear search"
            >
              <X className="w-3 h-3" />
            </button>
          )}
        </div>
      </div>

      {/* Right: Telemetry Stream Controls & Instrumentation Readout */}
      <div className="flex items-center gap-3">
        {systemMetrics && (
          <div className="hidden sm:flex items-center gap-3 text-[11px] font-mono text-lunar-text-sec border-r border-lunar-border pr-3">
            <span>
              CPU{" "}
              <strong className="text-lunar-text font-normal">
                {systemMetrics.cpuUsagePercent.toFixed(0)}%
              </strong>
            </span>
            <span>
              MEM{" "}
              <strong className="text-lunar-text font-normal">
                {systemMetrics.memoryUsagePercent.toFixed(0)}%
              </strong>
            </span>
          </div>
        )}

        <button
          type="button"
          onClick={() => void togglePauseMonitoring()}
          className="flex items-center gap-1.5 h-7 px-2.5 rounded bg-lunar-surface-2 hover:bg-lunar-elevated border border-lunar-border text-[11px] font-mono text-lunar-text-sec hover:text-lunar-text transition-colors cursor-pointer"
          title={
            monitoringStatus === "paused"
              ? "Resume 1Hz telemetry stream"
              : "Pause live telemetry stream"
          }
        >
          {monitoringStatus === "paused" ? (
            <>
              <Play className="w-3 h-3 text-lunar-warning" />
              <span>PAUSED</span>
            </>
          ) : (
            <>
              <Radio className="w-3 h-3 text-lunar-healthy animate-pulse" />
              <span>1.0Hz LIVE</span>
              <Pause className="w-2.5 h-2.5 ml-0.5 text-lunar-muted" />
            </>
          )}
        </button>
      </div>
    </header>
  );
};
