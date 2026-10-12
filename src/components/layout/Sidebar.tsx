import React from "react";
import { LayoutGrid, Cpu, HardDrive, Settings, Rocket } from "lucide-react";
import { useProcessStore } from "../../stores/process-store";
import { useStartupStore } from "../../stores/startup-store";
import type { AppRoute } from "../../types/process";

const NeuronIcon: React.FC<{ className?: string }> = ({ className }) => (
  <svg
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
    className={className}
  >
    {/* Synaptic connections / dendrites */}
    <line x1="12" y1="12" x2="5" y2="5" />
    <line x1="12" y1="12" x2="19" y2="5" />
    <line x1="12" y1="12" x2="5" y2="19" />
    <line x1="12" y1="12" x2="19" y2="19" />
    <line x1="12" y1="12" x2="12" y2="3.5" />
    <line x1="12" y1="12" x2="20.5" y2="12" />
    {/* Outer neural nodes */}
    <circle cx="5" cy="5" r="2" fill="currentColor" />
    <circle cx="19" cy="5" r="2" fill="currentColor" />
    <circle cx="5" cy="19" r="2" fill="currentColor" />
    <circle cx="19" cy="19" r="2" fill="currentColor" />
    <circle cx="12" cy="3.5" r="1.5" fill="currentColor" />
    <circle cx="20.5" cy="12" r="1.5" fill="currentColor" />
    {/* Central neuron soma */}
    <circle cx="12" cy="12" r="3.2" className="fill-lunar-bg" />
  </svg>
);

export const Sidebar: React.FC = () => {
  const currentRoute = useProcessStore((s) => s.currentRoute);
  const setRoute = useProcessStore((s) => s.setRoute);
  const systemMetrics = useProcessStore((s) => s.systemMetrics);
  const startupItemsCount = useStartupStore((s) => s.items.length);

  const totalProcs = systemMetrics?.totalProcesses ?? 0;

  const navItemClass = (route: AppRoute) =>
    `w-full flex items-center justify-between px-3 py-2.5 rounded-lg text-xs font-medium transition-colors cursor-pointer ${
      currentRoute === route
        ? "bg-lunar-elevated text-lunar-white border border-lunar-border"
        : "text-lunar-text-sec hover:text-lunar-text hover:bg-lunar-surface-2 border border-transparent"
    }`;

  return (
    <aside className="w-56 bg-lunar-surface border-r border-lunar-border flex flex-col justify-between p-3 shrink-0 select-none">
      {/* Top Navigation: Dashboard, Processes & Storage */}
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

        <button
          type="button"
          onClick={() => setRoute("startup")}
          className={navItemClass("startup")}
        >
          <span className="flex items-center gap-2.5">
            <Rocket className="w-3.5 h-3.5 text-lunar-text-sec" />
            <span>Startup</span>
          </span>

          <span className="flex items-center gap-1.5">
            {startupItemsCount > 0 && (
              <span className="font-mono text-[11px] text-lunar-muted">
                {startupItemsCount}
              </span>
            )}
          </span>
        </button>

        <button
          type="button"
          onClick={() => setRoute("storage")}
          className={navItemClass("storage")}
        >
          <span className="flex items-center gap-2.5">
            <HardDrive className="w-3.5 h-3.5 text-lunar-text-sec" />
            <span>Storage</span>
          </span>
        </button>
      </nav>

      {/* Bottom Navigation: AI Diagnostics directly above Settings */}
      <div className="pt-2 border-t border-lunar-border/60 space-y-1">
        <button
          type="button"
          onClick={() => setRoute("assistant")}
          className={navItemClass("assistant")}
        >
          <span className="flex items-center gap-2.5">
            <NeuronIcon className="w-3.5 h-3.5 text-lunar-text-sec" />
            <span>AI Diagnostics</span>
          </span>
        </button>

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
