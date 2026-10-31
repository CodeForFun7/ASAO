import React, { useEffect } from "react";
import { HardDrive, ArrowUpRight } from "lucide-react";
import { useStorageStore } from "../../stores/storage-store";
import { useProcessStore } from "../../stores/process-store";
import { StorageChart } from "../storage/StorageDistribution";
import { formatStorageBytes } from "../../services/storage";
import { STORAGE_CATEGORY_META } from "../../types/storage";

export const StorageComposition: React.FC = () => {
  const snapshot = useStorageStore((s) => s.snapshot);
  const drives = useStorageStore((s) => s.drives);
  const selectedDrive = useStorageStore((s) => s.selectedDrive);
  const scanState = useStorageStore((s) => s.scanState);
  const loadCachedStorage = useStorageStore((s) => s.loadCachedStorage);
  const setRoute = useProcessStore((s) => s.setRoute);

  useEffect(() => {
    void loadCachedStorage();
  }, [loadCachedStorage]);

  const drive =
    drives.find((d) => d.drive.toUpperCase() === (selectedDrive ?? "").toUpperCase()) ??
    drives[0] ??
    null;

  const slices = (snapshot?.distribution ?? []).filter((s) => s.bytes > 0);
  const scannedBytes = slices.reduce((acc, s) => acc + s.bytes, 0);
  const usedBytes = drive?.usedCapacity ?? scannedBytes;
  const totalBytes = drive?.totalCapacity ?? 0;
  const usedPercent =
    totalBytes > 0 ? Math.min(100, (usedBytes / totalBytes) * 100) : 0;
  const wholeBytes = Math.max(usedBytes, scannedBytes, 1);

  return (
    <section className="h-full min-h-0 rounded-xl lunar-glass-card p-4 flex flex-col gap-3">
      <header className="flex items-center justify-between gap-2 shrink-0">
        <div className="flex items-center gap-2 min-w-0">
          <HardDrive className="w-3.5 h-3.5 text-lunar-text-sec shrink-0" />
          <h2 className="text-[11px] font-mono uppercase tracking-[0.14em] text-lunar-text-sec truncate">
            Storage Composition{drive ? ` · ${drive.drive}` : ""}
          </h2>
        </div>
        <button
          type="button"
          onClick={() => setRoute("storage")}
          className="text-lunar-muted hover:text-lunar-white transition-colors cursor-pointer shrink-0"
          title="Open Storage"
          aria-label="Open Storage"
        >
          <ArrowUpRight className="w-3.5 h-3.5" />
        </button>
      </header>

      {slices.length === 0 ? (
        <div className="flex-1 min-h-0 flex flex-col items-center justify-center text-center gap-2">
          {drive ? (
            <>
              <p className="text-sm font-semibold font-mono text-lunar-white">
                {formatStorageBytes(drive.usedCapacity)} used of{" "}
                {formatStorageBytes(drive.totalCapacity)}
              </p>
              <p className="text-xs text-lunar-text-sec">
                {scanState === "scanning"
                  ? "Scanning the drive to break this down by file type..."
                  : "No file breakdown yet. Open Storage to scan this drive."}
              </p>
            </>
          ) : (
            <p className="text-xs text-lunar-text-sec">Reading drives...</p>
          )}
        </div>
      ) : (
        <div className="flex-1 min-h-0 flex flex-col sm:flex-row items-center gap-4 sm:gap-6">
          <div className="shrink-0">
            <StorageChart slices={slices} totalUsedBytes={wholeBytes} size={128} />
          </div>

          <div className="flex-1 w-full min-w-0 flex flex-col justify-center gap-2.5">
            {totalBytes > 0 && (
              <div className="space-y-1">
                <div className="flex items-baseline justify-between gap-2 font-mono text-xs">
                  <span className="text-lunar-white font-semibold">
                    {formatStorageBytes(usedBytes)}
                    <span className="text-lunar-muted font-normal">
                      {" "}
                      of {formatStorageBytes(totalBytes)}
                    </span>
                  </span>
                  <span className="text-lunar-text-sec">
                    {usedPercent.toFixed(0)}% used
                  </span>
                </div>
                <div className="h-1.5 rounded-full bg-lunar-bg border border-lunar-border/70 overflow-hidden">
                  <div
                    className="h-full rounded-full bg-lunar-white transition-[width] duration-500"
                    style={{ width: `${usedPercent}%` }}
                  />
                </div>
              </div>
            )}

            <ul className="grid grid-cols-1 min-[560px]:grid-cols-2 gap-x-5 gap-y-1.5">
              {slices.map((slice) => {
                const meta =
                  STORAGE_CATEGORY_META[slice.category] ?? STORAGE_CATEGORY_META.UNKNOWN;
                const share = (slice.bytes / wholeBytes) * 100;
                return (
                  <li
                    key={slice.category}
                    className="flex items-center gap-2 min-w-0 text-xs"
                  >
                    <span className={`w-2.5 h-2.5 rounded-xs shrink-0 ${meta.dotColor}`} />
                    <span className="text-lunar-text truncate flex-1">{slice.label}</span>
                    <span className="font-mono text-lunar-muted shrink-0">
                      {share.toFixed(0)}%
                    </span>
                    <span className="font-mono text-lunar-white font-semibold shrink-0 w-[4.5rem] text-right">
                      {formatStorageBytes(slice.bytes)}
                    </span>
                  </li>
                );
              })}
            </ul>
          </div>
        </div>
      )}
    </section>
  );
};
