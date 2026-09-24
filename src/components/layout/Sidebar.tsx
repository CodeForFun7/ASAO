import React from "react";
import { LayoutGrid, Cpu, Settings, ShieldCheck } from "lucide-react";
import { useProcessStore } from "../../stores/process-store";
import type { AppRoute } from "../../types/process";

export const Sidebar: React.FC = () => {
  const currentRoute = useProcessStore((s) => s.currentRoute);
  const setRoute = useProcessStore((s) => s.setRoute);
  const systemMetrics = useProcessStore((s) => s.systemMetrics);
  const monitoringStatus = useProcessStore((s) => s.monitoringStatus);

  const totalProcs = systemMetrics?.totalProcesses ?? 0;
  const attentionCount = systemMetrics?.attentionProcesses ?? 0;
  const sysStatus = systemMetrics?.systemStatus ?? "healthy";

  const navItemClass = (route: AppRoute) =>
    `w-full flex items-center justify-between px-3 py-2 rounded-md text-xs font-medium transition-colors cursor-pointer ${
      currentRoute === route
        ? "bg-lunar-elevated text-lunar-white border border-lunar-border"
        : "text-lunar-text-sec hover:text-lunar-text hover:bg-lunar-surface-2 border border-transparent"
    }`;

  return (
    <aside className="w-56 bg-lunar-surface border-r border-lunar-border flex flex-col justify-between shrink-0 select-none">
      {/* Top Navigation Sections */}
      <div className="p-3 space-y-6">
        {/* OVERVIEW */}
        <div>
          <div className="px-3 mb-2 text-[10px] font-mono uppercase tracking-[0.16em] text-lunar-muted">
            Overview
          </div>
          <nav className="space-y-1">
            <button
              type="button"
              onClick={() => setRoute("dashboard")}
              className={navItemClass("dashboard")}
            >
              <span className="flex items-center gap-2.5">
                <LayoutGrid className="w-3.5 h-3.5 text-lunar-text-sec" />
                <span>Dashboard</span>
              </span>
              {currentRoute === "dashboard" && (
                <span className="w-1.5 h-1.5 rounded-full bg-lunar-white" />
              )}
            </button>

            <button
              type="button"
              onClick={() => setRoute("processes")}
              className={navItemClass("processes")}
            >
              <span className="flex items-center gap-2.5">
                <Cpu className="w-3.5 h-3.5 text-lunar-text-sec" />
                <span>Processes</span>
              </span>
              <span className="flex items-center gap-1.5">
                {attentionCount > 0 && (
                  <span
                    className="w-1.5 h-1.5 rounded-full bg-lunar-warning"
                    title={`${attentionCount} require attention`}
                  />
                )}
                {totalProcs > 0 && (
                  <span className="font-mono text-[10px] text-lunar-muted bg-lunar-bg px-1.5 py-0.5 rounded border border-lunar-border">
                    {totalProcs}
                  </span>
                )}
              </span>
            </button>
          </nav>
        </div>

        <div className="border-t border-lunar-border mx-1" />

        {/* SYSTEM */}
        <div>
          <div className="px-3 mb-2 text-[10px] font-mono uppercase tracking-[0.16em] text-lunar-muted">
            System
          </div>
          <nav className="space-y-1">
            <button
              type="button"
              onClick={() => setRoute("settings")}
              className={navItemClass("settings")}
            >
              <span className="flex items-center gap-2.5">
                <Settings className="w-3.5 h-3.5 text-lunar-text-sec" />
                <span>Settings</span>
              </span>
              <span className="text-[10px] font-mono text-lunar-muted">
                v0.1
              </span>
            </button>
          </nav>
        </div>
      </div>

      {/* Bottom System Status Footer */}
      <div className="p-3 border-t border-lunar-border bg-lunar-bg/40 space-y-2.5">
        <div className="flex items-center justify-between px-2 py-1.5 rounded bg-lunar-surface-2 border border-lunar-border">
          <div className="flex items-center gap-2">
            <span
              className={`w-2 h-2 rounded-full ${
                monitoringStatus === "error"
                  ? "bg-lunar-critical"
                  : sysStatus === "critical"
                  ? "bg-lunar-critical"
                  : sysStatus === "warning"
                  ? "bg-lunar-warning"
                  : "bg-lunar-healthy"
              }`}
            />
            <span className="text-xs font-medium text-lunar-text">
              {monitoringStatus === "error"
                ? "Sensor Error"
                : sysStatus === "critical"
                ? "System Strained"
                : sysStatus === "warning"
                ? "System Load"
                : "System OK"}
            </span>
          </div>
          <ShieldCheck className="w-3.5 h-3.5 text-lunar-muted" />
        </div>

        <div className="px-2 flex items-center justify-between text-[10px] font-mono text-lunar-muted">
          <span>MODE: ANALYZER</span>
          <span>READ-ONLY</span>
        </div>
      </div>
    </aside>
  );
};
