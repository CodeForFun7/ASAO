import React from "react";
import { HardDrive, XCircle } from "lucide-react";
import type { StorageScanProgress } from "../../types/storage";

interface ScanProgressProps {
  progress: StorageScanProgress | null;
  onCancel: () => void;
}

export const ScanProgress: React.FC<ScanProgressProps> = ({
  progress,
  onCancel,
}) => {
  const pct = Math.max(2, Math.min(100, progress?.progressPercent ?? 5));
  const filesAnalyzed = progress?.filesAnalyzed ?? 0;
  const foldersAnalyzed = progress?.foldersAnalyzed ?? 0;
  const currentPath = progress?.currentPath || "Enumerating volume structure...";
  const totalSegments = 36;
  const filledSegments = Math.round((pct / 100) * totalSegments);

  return (
    <div className="rounded-lg lunar-glass-card p-6 max-w-lg mx-auto w-full border border-lunar-border">
      <div className="flex flex-col items-center text-center space-y-4">
        <div className="w-9 h-9 rounded-full bg-lunar-bg border border-lunar-border flex items-center justify-center">
          <HardDrive className="w-4 h-4 text-lunar-white animate-pulse" />
        </div>

        <div>
          <h2 className="text-sm font-semibold text-lunar-white tracking-tight">
            Scanning Storage
          </h2>
          {progress?.drive && (
            <p className="text-[11px] font-mono text-lunar-text-sec mt-0.5">
              Volume {progress.drive}
            </p>
          )}
        </div>

        {/* Segmented Progress Bar */}
        <div className="w-full space-y-2">
          <div className="w-full h-2.5 bg-lunar-bg border border-lunar-border rounded-sm p-0.5 flex items-center gap-[2px] overflow-hidden">
            {Array.from({ length: totalSegments }).map((_, idx) => (
              <div
                key={idx}
                className={`h-full flex-1 rounded-[1px] transition-colors duration-150 ${
                  idx < filledSegments
                    ? "bg-lunar-white"
                    : "bg-lunar-surface-2/80"
                }`}
              />
            ))}
          </div>

          <div className="text-[11px] font-mono text-lunar-muted truncate px-2">
            {currentPath}
          </div>
        </div>

        {/* Live Counters */}
        <div className="w-full max-w-xs bg-lunar-bg border border-lunar-border rounded p-3 space-y-1.5 text-xs font-mono">
          <div className="flex items-center justify-between">
            <span className="text-lunar-text-sec">Files analyzed</span>
            <span className="text-lunar-white font-semibold">
              {filesAnalyzed.toLocaleString()}
            </span>
          </div>
          <div className="flex items-center justify-between">
            <span className="text-lunar-text-sec">Folders analyzed</span>
            <span className="text-lunar-white font-semibold">
              {foldersAnalyzed.toLocaleString()}
            </span>
          </div>
        </div>

        {/* Cancel Action */}
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
