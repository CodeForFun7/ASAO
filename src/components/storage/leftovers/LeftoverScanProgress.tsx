import React from "react";
import { PackageSearch, XCircle } from "lucide-react";
import type { ResidualScanProgress } from "../../../types/residual";

interface LeftoverScanProgressProps {
  progress: ResidualScanProgress | null;
  onCancel: () => void;
}

const SEGMENTS = 36;

/** Same look as the Storage scan progress. The total is unknown, so the bar runs as a sweep. */
export const LeftoverScanProgress: React.FC<LeftoverScanProgressProps> = ({ progress, onCancel }) => {
  const phaseText =
    progress?.phase === "matching"
      ? "Checking folders against installed apps"
      : "Reading installed apps, services and scheduled tasks";
  const head = (progress?.foldersChecked ?? 0) % SEGMENTS;

  return (
    <div className="rounded-lg lunar-glass-card p-6 max-w-lg mx-auto w-full border border-lunar-border">
      <div className="flex flex-col items-center text-center space-y-4">
        <div className="w-9 h-9 rounded-full bg-lunar-bg border border-lunar-border flex items-center justify-center">
          <PackageSearch className="w-4 h-4 text-lunar-white animate-pulse" />
        </div>

        <div>
          <h2 className="text-sm font-semibold text-lunar-white tracking-tight">Looking for leftovers</h2>
          <p className="text-[11px] text-lunar-text-sec mt-0.5">{phaseText}</p>
        </div>

        <div className="w-full space-y-2">
          <div className="w-full h-2.5 bg-lunar-bg border border-lunar-border rounded-sm p-0.5 flex items-center gap-[2px] overflow-hidden">
            {Array.from({ length: SEGMENTS }).map((_, idx) => {
              const distance = (head - idx + SEGMENTS) % SEGMENTS;
              return (
                <div
                  key={idx}
                  className={`h-full flex-1 rounded-[1px] transition-colors duration-150 ${
                    distance < 6 ? "bg-lunar-white" : "bg-lunar-surface-2/80"
                  }`}
                />
              );
            })}
          </div>
          <div className="text-[11px] font-mono text-lunar-muted truncate px-2">
            {progress?.phase === "matching" && progress.currentPath
              ? progress.currentPath
              : "This takes a few seconds"}
          </div>
        </div>

        <div className="w-full max-w-xs bg-lunar-bg border border-lunar-border rounded p-3 space-y-1.5 text-xs font-mono">
          <div className="flex items-center justify-between">
            <span className="text-lunar-text-sec">Folders checked</span>
            <span className="text-lunar-white font-semibold">{(progress?.foldersChecked ?? 0).toLocaleString()}</span>
          </div>
          <div className="flex items-center justify-between">
            <span className="text-lunar-text-sec">Leftovers found</span>
            <span className="text-lunar-white font-semibold">{(progress?.candidatesFound ?? 0).toLocaleString()}</span>
          </div>
        </div>

        <button
          type="button"
          onClick={onCancel}
          className="inline-flex items-center gap-1.5 px-4 py-1.5 rounded bg-lunar-surface hover:bg-lunar-elevated text-xs font-mono text-lunar-text-sec hover:text-lunar-white border border-lunar-border transition-colors cursor-pointer"
        >
          <XCircle className="w-3.5 h-3.5" />
          <span>Cancel</span>
        </button>
      </div>
    </div>
  );
};
