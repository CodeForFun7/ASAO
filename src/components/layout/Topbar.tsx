import React, { useState } from "react";
import { Search, Minus, Square, Copy, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useProcessStore } from "../../stores/process-store";
import {
  closeWindow,
  minimizeWindow,
  toggleMaximizeWindow,
} from "../../services/tauri";

export const Topbar: React.FC = () => {
  const searchQuery = useProcessStore((s) => s.searchQuery);
  const setSearchQuery = useProcessStore((s) => s.setSearchQuery);
  const currentRoute = useProcessStore((s) => s.currentRoute);
  const setRoute = useProcessStore((s) => s.setRoute);
  const [isMaximized, setIsMaximized] = useState(false);

  const handleGlobalSearchChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = e.target.value;
    setSearchQuery(val);
    if (val.trim().length > 0 && currentRoute !== "processes") {
      setRoute("processes");
    }
  };

  const handleMinimize = async () => {
    try {
      await minimizeWindow();
    } catch {
      await getCurrentWindow().minimize();
    }
  };

  const handleToggleMaximize = async () => {
    try {
      const nextMax = await toggleMaximizeWindow();
      setIsMaximized(nextMax);
    } catch {
      const win = getCurrentWindow();
      await win.toggleMaximize();
      setIsMaximized(await win.isMaximized());
    }
  };

  const handleClose = async () => {
    try {
      await closeWindow();
    } catch {
      await getCurrentWindow().close();
    }
  };

  return (
    <header
      data-tauri-drag-region
      onDoubleClick={(e) => {
        if (e.target === e.currentTarget) {
          void handleToggleMaximize();
        }
      }}
      className="h-11 bg-lunar-surface backdrop-blur-xl border-b border-lunar-border pl-5 flex items-center justify-between shrink-0 select-none"
    >
      {/* Left: Clean ASAO Brand Text Only */}
      <div
        data-tauri-drag-region
        className="flex items-center w-48 pointer-events-none"
      >
        <span className="text-sm font-semibold tracking-wider text-lunar-white">
          ASAO
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
            className="w-full h-7 pl-8 pr-7 bg-lunar-bg border border-lunar-border rounded-md text-xs text-lunar-text placeholder:text-lunar-muted focus:outline-none focus:border-lunar-text-sec transition-colors"
          />
          {searchQuery && (
            <button
              type="button"
              onClick={() => setSearchQuery("")}
              className="absolute right-2 text-lunar-muted hover:text-lunar-text p-0.5 cursor-pointer"
              title="Clear search"
            >
              <X className="w-3 h-3" />
            </button>
          )}
        </div>
      </div>

      {/* Right: Custom Window Controls (Minimize, Maximize/Restore, Close) */}
      <div className="flex items-center h-full">
        <button
          type="button"
          onClick={() => void handleMinimize()}
          className="h-full w-11 flex items-center justify-center text-lunar-text-sec hover:bg-lunar-surface-2 hover:text-lunar-white transition-colors cursor-pointer"
          title="Minimize"
        >
          <Minus className="w-3.5 h-3.5" />
        </button>

        <button
          type="button"
          onClick={() => void handleToggleMaximize()}
          className="h-full w-11 flex items-center justify-center text-lunar-text-sec hover:bg-lunar-surface-2 hover:text-lunar-white transition-colors cursor-pointer"
          title={isMaximized ? "Restore Down" : "Maximize"}
        >
          {isMaximized ? (
            <Copy className="w-3 h-3 rotate-180" />
          ) : (
            <Square className="w-3 h-3" />
          )}
        </button>

        <button
          type="button"
          onClick={() => void handleClose()}
          className="h-full w-11 flex items-center justify-center text-lunar-text-sec hover:bg-lunar-critical hover:text-lunar-white transition-colors cursor-pointer"
          title="Close"
        >
          <X className="w-4 h-4" />
        </button>
      </div>
    </header>
  );
};
