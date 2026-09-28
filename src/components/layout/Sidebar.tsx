import React from "react";
import { LayoutGrid, Cpu, Settings } from "lucide-react";
import { useProcessStore } from "../../stores/process-store";
import type { AppRoute } from "../../types/process";

export const Sidebar: React.FC = () => {
  const currentRoute = useProcessStore((s) => s.currentRoute);
  const setRoute = useProcessStore((s) => s.setRoute);
  const systemMetrics = useProcessStore((s) => s.systemMetrics);

  const totalProcs = systemMetrics?.totalProcesses ?? 0;

  const navItemClass = (route: AppRoute) =>
    `w-full flex items-center justify-between px-3 py-2.5 rounded-lg text-xs font-medium transition-colors cursor-pointer ${
      currentRoute === route
        ? "bg-lunar-elevated text-lunar-white border border-lunar-border"
        : "text-lunar-text-sec hover:text-lunar-text hover:bg-lunar-surface-2 border border-transparent"
    }`;

  return (
    <aside className="w-56 bg-lunar-surface border-r border-lunar-border flex flex-col justify-between p-3 shrink-0 select-none">
      {/* Top Navigation: Dashboard & Processes */}
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
            {totalProcs > 0 && (
              <span className="font-mono text-[11px] text-lunar-muted">
                {totalProcs}
              </span>
            )}
          </span>
        </button>
      </nav>

      {/* Bottom Navigation: Settings pinned at bottom */}
      <div className="pt-2 border-t border-lunar-border/60">
        <button
          type="button"
          onClick={() => setRoute("settings")}
          className={navItemClass("settings")}
        >
          <span className="flex items-center gap-2.5">
            <Settings className="w-3.5 h-3.5 text-lunar-text-sec" />
            <span>Settings</span>
          </span>
        </button>
      </div>
    </aside>
  );
};
