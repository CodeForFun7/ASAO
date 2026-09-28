import React, { useState, useRef, useEffect } from "react";
import {
  ArrowLeft,
  MoreHorizontal,
  X,
  ExternalLink,
  Settings,
} from "lucide-react";
import type {
  AsaoSettings,
  SystemConditionState,
  WidgetMode,
} from "../../types/widget";
import { hideWidget, openMainWindow } from "../../services/widget";

interface WidgetHeaderProps {
  mode: WidgetMode;
  condition: SystemConditionState;
  settings: AsaoSettings;
  onBackToMonitor: () => void;
  onUpdateSettings: (patch: Partial<AsaoSettings>) => void;
}

export const WidgetHeader: React.FC<WidgetHeaderProps> = ({
  mode,
  onBackToMonitor,
}) => {
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setMenuOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  return (
    <header
      data-tauri-drag-region
      className="h-9 px-3.5 border-b border-lunar-border/80 flex items-center justify-between shrink-0 select-none"
    >
      {/* Left: Brand or Back Button */}
      <div className="flex items-center gap-2">
        {mode === "chat" ? (
          <button
            type="button"
            onClick={onBackToMonitor}
            className="flex items-center gap-1.5 text-xs font-semibold tracking-wider text-lunar-white hover:text-lunar-text-sec transition-colors cursor-pointer"
            title="Return to Monitor Mode"
          >
            <ArrowLeft className="w-3.5 h-3.5" />
            <span>ASAO</span>
          </button>
        ) : (
          <div
            data-tauri-drag-region
            className="flex items-center pointer-events-none"
          >
            <span className="text-xs font-semibold tracking-[0.18em] text-lunar-white">
              ASAO
            </span>
          </div>
        )}
      </div>

      {/* Right: Overflow Menu & Hide Button */}
      <div className="flex items-center gap-1">
        <div className="relative" ref={menuRef}>
          <button
            type="button"
            onClick={() => setMenuOpen((prev) => !prev)}
            className="w-6 h-6 rounded flex items-center justify-center text-lunar-text-sec hover:text-lunar-white hover:bg-lunar-elevated/80 transition-colors cursor-pointer"
            title="Widget Options"
          >
            <MoreHorizontal className="w-3.5 h-3.5" />
          </button>

          {menuOpen && (
            <div className="absolute right-0 mt-1 w-44 rounded-lg bg-lunar-surface border border-lunar-border shadow-2xl z-50 py-1 text-xs">
              <button
                type="button"
                onClick={() => {
                  void openMainWindow("dashboard");
                  setMenuOpen(false);
                }}
                className="w-full px-3 py-1.5 text-left flex items-center gap-2 text-lunar-text hover:bg-lunar-surface-2 cursor-pointer"
              >
                <ExternalLink className="w-3 h-3 text-lunar-text-sec" />
                <span>Open Asao</span>
              </button>

              <button
                type="button"
                onClick={() => {
                  void openMainWindow("settings");
                  setMenuOpen(false);
                }}
                className="w-full px-3 py-1.5 text-left flex items-center gap-2 text-lunar-text hover:bg-lunar-surface-2 cursor-pointer"
              >
                <Settings className="w-3 h-3 text-lunar-text-sec" />
                <span>Widget Settings</span>
              </button>
            </div>
          )}
        </div>

        <button
          type="button"
          onClick={() => void hideWidget()}
          className="w-6 h-6 rounded flex items-center justify-center text-lunar-text-sec hover:text-lunar-white hover:bg-lunar-elevated/80 transition-colors cursor-pointer"
          title="Hide Widget (Asao stays running)"
        >
          <X className="w-3.5 h-3.5" />
        </button>
      </div>
    </header>
  );
};
