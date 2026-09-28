import React from "react";
import { HardDrive, Usb, Disc, Server } from "lucide-react";
import type { StorageDrive } from "../../types/storage";
import {
  formatDriveCapacityPair,
  formatStorageBytes,
} from "../../services/storage";

interface StorageUsageBarProps {
  usagePercentage: number;
}

export const StorageUsageBar: React.FC<StorageUsageBarProps> = ({
  usagePercentage,
}) => {
  const clamped = Math.max(0, Math.min(100, usagePercentage));
  const totalSegments = 44;
  const filledSegments = Math.round((clamped / 100) * totalSegments);

  const barColorClass =
    clamped >= 90
      ? "bg-lunar-critical"
      : clamped >= 80
      ? "bg-lunar-warning"
      : "bg-lunar-white";

  return (
    <div className="w-full space-y-1.5">
      {/* Segmented technical meter */}
      <div
        className="w-full h-2.5 bg-lunar-bg border border-lunar-border/90 rounded-sm p-0.5 flex items-center gap-[2px] overflow-hidden"
        role="progressbar"
        aria-valuenow={clamped}
        aria-valuemin={0}
        aria-valuemax={100}
      >
        {Array.from({ length: totalSegments }).map((_, idx) => {
          const isFilled = idx < filledSegments;
          return (
            <div
              key={idx}
              className={`h-full flex-1 rounded-[1px] transition-colors duration-200 ${
                isFilled ? barColorClass : "bg-lunar-surface-2/80"
              }`}
            />
          );
        })}
      </div>
    </div>
  );
};

interface DriveSelectorProps {
  drives: StorageDrive[];
  selectedDrive: string | null;
  onSelectDrive: (drive: string) => void;
}

export const DriveSelector: React.FC<DriveSelectorProps> = ({
  drives,
  selectedDrive,
  onSelectDrive,
}) => {
  if (drives.length <= 1) return null;

  return (
    <div className="flex items-center gap-1.5 flex-wrap">
      <span className="text-[10px] font-mono uppercase tracking-[0.14em] text-lunar-muted mr-1">
        Active Volume:
      </span>
      {drives.map((drv) => {
        const isSelected =
          selectedDrive?.toUpperCase() === drv.drive.toUpperCase();
        return (
          <button
            key={drv.mountPoint}
            type="button"
            onClick={() => onSelectDrive(drv.drive)}
            className={`px-2.5 py-1 rounded text-xs font-mono transition-colors cursor-pointer flex items-center gap-1.5 border ${
              isSelected
                ? "bg-lunar-elevated text-lunar-white border-lunar-white/25"
                : "bg-lunar-surface text-lunar-text-sec hover:text-lunar-white hover:bg-lunar-surface-2 border-lunar-border"
            }`}
          >
            <span className="font-semibold">{drv.drive}</span>
            <span className="text-[11px] text-lunar-muted">
              ({drv.usagePercentage.toFixed(1)}%)
            </span>
          </button>
        );
      })}
    </div>
  );
};

function getDriveIcon(driveType: string) {
  const lower = driveType.toLowerCase();
  if (lower.includes("removable") || lower.includes("usb")) {
    return Usb;
  }
  if (lower.includes("optical") || lower.includes("cd")) {
    return Disc;
  }
  if (lower.includes("network")) {
    return Server;
  }
  return HardDrive;
}

interface DriveOverviewProps {
  drives: StorageDrive[];
  selectedDrive: string | null;
  onSelectDrive: (drive: string) => void;
}

export const DriveOverview: React.FC<DriveOverviewProps> = ({
  drives,
  selectedDrive,
  onSelectDrive,
}) => {
  if (drives.length === 0) {
    return (
      <div className="rounded-lg lunar-glass-card p-6 text-center">
        <HardDrive className="w-6 h-6 text-lunar-muted mx-auto mb-2 stroke-[1.5]" />
        <p className="text-xs font-mono text-lunar-text-sec">
          No storage devices detected.
        </p>
      </div>
    );
  }

  const gridColsClass =
    drives.length === 1
      ? "grid-cols-1"
      : drives.length === 2
      ? "grid-cols-1 lg:grid-cols-2"
      : "grid-cols-1 md:grid-cols-2 xl:grid-cols-3";

  return (
    <section className="space-y-2.5">
      {drives.length > 1 && (
        <div className="flex items-center justify-end">
          <DriveSelector
            drives={drives}
            selectedDrive={selectedDrive}
            onSelectDrive={onSelectDrive}
          />
        </div>
      )}

      <div className={`grid ${gridColsClass} gap-3`}>
        {drives.map((drv) => {
          const isSelected =
            selectedDrive?.toUpperCase() === drv.drive.toUpperCase();
          const Icon = getDriveIcon(drv.driveType);

          return (
            <div
              key={drv.mountPoint}
              onClick={() => onSelectDrive(drv.drive)}
              className={`rounded-lg p-4 transition-all cursor-pointer border ${
                isSelected
                  ? "lunar-glass-card border-lunar-white/25"
                  : "bg-lunar-surface hover:bg-lunar-surface-2/90 border-lunar-border"
              }`}
            >
              {/* Top row: Drive identifier + Name and Used / Total */}
              <div className="flex items-center justify-between gap-3 mb-3">
                <div className="flex items-center gap-2 min-w-0">
                  <Icon className="w-4 h-4 text-lunar-white shrink-0" />
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-semibold font-mono text-lunar-white">
                        {drv.drive}
                      </span>
                      <span className="text-xs text-lunar-text-sec truncate">
                        {drv.name}
                      </span>
                    </div>
                  </div>
                </div>

                <div className="text-xs font-mono font-medium text-lunar-white shrink-0">
                  {formatDriveCapacityPair(
                    drv.usedCapacity,
                    drv.totalCapacity
                  )}
                </div>
              </div>

              {/* Middle: Segmented Usage Bar */}
              <StorageUsageBar usagePercentage={drv.usagePercentage} />

              {/* Bottom row: Used / Free / Total metrics + Percentage */}
              <div className="mt-3 flex flex-wrap items-center justify-between gap-2 text-xs font-mono">
                <div className="flex items-center gap-4 text-lunar-text-sec">
                  <span>
                    <strong className="text-lunar-white font-medium">
                      {formatStorageBytes(drv.usedCapacity)}
                    </strong>{" "}
                    Used
                  </span>
                  <span>
                    <strong className="text-lunar-text font-medium">
                      {formatStorageBytes(drv.freeCapacity)}
                    </strong>{" "}
                    Free
                  </span>
                  <span className="hidden sm:inline">
                    <strong className="text-lunar-text-sec font-normal">
                      {formatStorageBytes(drv.totalCapacity)}
                    </strong>{" "}
                    Total
                  </span>
                </div>

                <span
                  className={`font-mono font-semibold ${
                    drv.usagePercentage >= 90
                      ? "text-lunar-critical"
                      : drv.usagePercentage >= 80
                      ? "text-lunar-warning"
                      : "text-lunar-white"
                  }`}
                >
                  {drv.usagePercentage.toFixed(1)}%
                </span>
              </div>
            </div>
          );
        })}
      </div>
    </section>
  );
};
