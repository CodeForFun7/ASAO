import React, { useState } from "react";
import { Rocket, Clock, ChevronDown, ChevronUp, ArrowUpRight } from "lucide-react";
import type { DiagnosticVisualData } from "../../types/agent";
import { useProcessStore } from "../../stores/process-store";

interface StartupImpactChartProps {
  data: DiagnosticVisualData;
  onDisableItem?: (id: string, name: string) => void;
}

export const StartupImpactChart: React.FC<StartupImpactChartProps> = ({
  data,
}) => {
  const startup = data.startup;
  const setRoute = useProcessStore((s) => s.setRoute);
  const [showAllSampled, setShowAllSampled] = useState(false);

  if (!startup) return null;

  const highImpactItems = startup.highImpactItems || [];
  const displayItems = showAllSampled
    ? highImpactItems
    : highImpactItems.slice(0, 5);

  const maxBootMs = Math.max(
    1000,
    ...highImpactItems.map((i) => i.bootDurationMs || 500)
  );

  return (
    <div className="rounded-2xl bg-lunar-surface border border-lunar-border p-4 my-3 text-xs shadow-xs">
      <div className="flex items-center justify-between mb-3.5">
        <div className="flex items-center gap-2 font-medium text-lunar-white text-[12.5px]">
          <Rocket className="w-4 h-4 text-lunar-white" />
          <span>Startup Impact Ranking</span>
        </div>
        <div className="text-[11px] text-lunar-text-sec flex items-center gap-1.5 font-mono">
          <Clock className="w-3 h-3 text-lunar-muted" />
          <span>~{(startup.estimatedBootDelayMs / 1000).toFixed(1)}s Boot Delay</span>
        </div>
      </div>

      <div className="space-y-2.5">
        {displayItems.length > 0 ? (
          displayItems.map((item) => {
            const barWidthPercent = Math.min(
              100,
              Math.max(10, (item.bootDurationMs / maxBootMs) * 100)
            );
            const isHigh = item.impact === "high";

            return (
              <div
                key={item.id}
                className="p-3 rounded-xl bg-lunar-surface-2/60 border border-lunar-border hover:bg-lunar-surface-2 transition-all"
              >
                <div className="flex items-center justify-between mb-2">
                  <div className="flex items-center gap-2 min-w-0">
                    <span className="font-semibold text-lunar-white truncate text-[12.5px]">
                      {item.name}
                    </span>

                    {/* Impact badge */}
                    <span
                      className={`text-[9.5px] font-mono uppercase px-1.5 py-0.5 rounded border ${
                        isHigh
                          ? "bg-lunar-white text-lunar-bg font-semibold border-lunar-white"
                          : "bg-lunar-elevated text-lunar-text border-lunar-border"
                      }`}
                    >
                      {item.impact}
                    </span>

                    {item.isCurrentlyRunning && (
                      <span className="text-[9.5px] text-lunar-text-sec font-mono bg-lunar-elevated border border-lunar-border px-1.5 py-0.5 rounded">
                        Running
                      </span>
                    )}
                  </div>

                  <div className="flex items-center gap-2 text-right shrink-0">
                    <span className="font-mono font-bold text-[12px] text-lunar-white">
                      {item.bootDurationMs.toLocaleString()} ms
                    </span>
                    <span className="font-mono text-[10px] text-lunar-muted">
                      ({item.bootCpuMs} ms CPU)
                    </span>
                  </div>
                </div>

                {/* Horizontal Bar */}
                <div className="h-1.5 w-full rounded-full bg-lunar-border overflow-hidden mb-1.5">
                  <div
                    style={{ width: `${barWidthPercent}%` }}
                    className={`h-full rounded-full transition-all duration-500 ${
                      isHigh ? "bg-lunar-white" : "bg-lunar-muted"
                    }`}
                  />
                </div>

                <div className="flex items-center justify-between text-[10.5px] text-lunar-text-sec mt-1">
                  <span className="truncate max-w-[220px]">
                    Source: {item.source || "Registry"}
                  </span>
                  <span className="text-lunar-text">
                    {item.recommendation === "disable"
                      ? "Recommended to disable"
                      : "User preference"}
                  </span>
                </div>
              </div>
            );
          })
        ) : (
          <div className="p-3 rounded-xl bg-lunar-surface-2 text-center text-lunar-muted text-[11px]">
            No high-impact startup delays detected.
          </div>
        )}
      </div>

      {/* Footer controls: Show All / View in Startup page */}
      <div className="mt-3.5 pt-3 border-t border-lunar-border flex items-center justify-between text-[11px]">
        {highImpactItems.length > 5 ? (
          <button
            type="button"
            onClick={() => setShowAllSampled(!showAllSampled)}
            className="text-lunar-text-sec hover:text-lunar-white flex items-center gap-1 transition-colors cursor-pointer"
          >
            <span>{showAllSampled ? "Show Top 5" : `Show all ${highImpactItems.length} heavy apps`}</span>
            {showAllSampled ? <ChevronUp className="w-3 h-3" /> : <ChevronDown className="w-3 h-3" />}
          </button>
        ) : (
          <span className="text-lunar-muted text-[10px]">
            {startup.totalStartupItems} total autostart items
          </span>
        )}

        <button
          type="button"
          onClick={() => setRoute("startup")}
          className="text-lunar-white hover:text-lunar-text-sec flex items-center gap-1 font-medium transition-colors cursor-pointer ml-auto"
        >
          <span>Startup Manager</span>
          <ArrowUpRight className="w-3 h-3" />
        </button>
      </div>
    </div>
  );
};
