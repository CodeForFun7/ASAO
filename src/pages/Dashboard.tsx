import React from "react";
import {
  ArrowDown,
  ArrowUp,
  AlertTriangle,
  RefreshCw,
} from "lucide-react";
import { useProcessStore } from "../stores/process-store";
import { MetricCard } from "../components/dashboard/MetricCard";
import { SystemHealth } from "../components/dashboard/SystemHealth";
import { BatteryStatusCard } from "../components/dashboard/BatteryStatusCard";
import { RecommendationCarousel } from "../components/dashboard/RecommendationCarousel";
import { TemperaturePanel } from "../components/dashboard/TemperaturePanel";
import { StorageComposition } from "../components/dashboard/StorageComposition";
import { NyraCtaCard } from "../components/dashboard/NyraCtaCard";

const DeltaIcon: React.FC<{ delta: number }> = ({ delta }) =>
  delta <= 0 ? (
    <ArrowDown className="w-3 h-3 text-lunar-healthy shrink-0" />
  ) : (
    <ArrowUp className="w-3 h-3 text-lunar-warning shrink-0" />
  );

export const Dashboard: React.FC = () => {
  const systemMetrics = useProcessStore((s) => s.systemMetrics);
  const systemHistory = useProcessStore((s) => s.systemHistory);
  const monitoringStatus = useProcessStore((s) => s.monitoringStatus);
  const errorMessage = useProcessStore((s) => s.errorMessage);
  const retryMonitoring = useProcessStore((s) => s.retryMonitoring);

  const navigateToMemoryProcesses = useProcessStore(
    (s) => s.navigateToMemoryProcesses
  );
  const navigateToCpuProcesses = useProcessStore(
    (s) => s.navigateToCpuProcesses
  );
  const setCategoryFilter = useProcessStore((s) => s.setCategoryFilter);
  const setRoute = useProcessStore((s) => s.setRoute);

  // Loading State
  if (monitoringStatus === "loading" && !systemMetrics) {
    return (
      <div className="flex-1 p-6 flex flex-col items-center justify-center text-center">
        <div className="w-8 h-8 rounded-full border-2 border-lunar-border border-t-lunar-white animate-spin mb-4" />
        <h2 className="text-sm font-medium text-lunar-white">
          Analyzing system...
        </h2>
        <p className="text-xs text-lunar-muted mt-1 font-mono">
          Collecting running processes
        </p>
      </div>
    );
  }

  // Error State
  if (monitoringStatus === "error" && !systemMetrics) {
    return (
      <div className="flex-1 p-6 flex flex-col items-center justify-center text-center max-w-md mx-auto">
        <AlertTriangle className="w-8 h-8 text-lunar-critical mb-3 stroke-[1.5]" />
        <h2 className="text-sm font-semibold text-lunar-white">
          Unable to read process information.
        </h2>
        <p className="text-xs text-lunar-text-sec mt-1.5 leading-relaxed">
          {errorMessage ??
            "Some Windows processes may require elevated permissions."}
        </p>
        <button
          type="button"
          onClick={() => void retryMonitoring()}
          className="mt-4 inline-flex items-center gap-2 px-4 py-2 rounded bg-lunar-elevated hover:bg-lunar-border text-xs font-medium text-lunar-white border border-lunar-border transition-colors cursor-pointer"
        >
          <RefreshCw className="w-3.5 h-3.5" />
          <span>Retry</span>
        </button>
      </div>
    );
  }

  if (!systemMetrics) return null;

  const cpuDelta = systemMetrics.cpuDeltaPercent;
  const memDelta = systemMetrics.memoryDeltaPercent;
  const gpuDelta = systemMetrics.gpuDeltaPercent ?? 0;

  const handleSelectGpu = () => {
    setCategoryFilter("drivers");
    setRoute("processes");
  };

  // Desktop (lg+) fills the viewport with no scrolling. Smaller windows stack the cards and scroll.
  return (
    <div className="flex-1 min-h-0 overflow-y-auto p-4 lg:p-5">
      <div className="flex flex-col gap-3 lg:gap-4 lg:h-full lg:min-h-[600px]">
        <div className="flex items-baseline gap-3 shrink-0">
          <h1 className="text-lg font-semibold text-lunar-white tracking-tight">
            Dashboard
          </h1>
          <p className="text-xs text-lunar-text-sec truncate">
            Your system is being monitored in real time.
          </p>
        </div>

        {/* Row 1: metric cards + charts (left), recommendations (right) */}
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-3 lg:gap-4 lg:flex-[1.35_1_0] lg:min-h-0">
          <div className="order-2 lg:order-1 lg:col-span-8 flex flex-col gap-3 lg:gap-4 min-h-0">
            <div className="grid grid-cols-1 sm:grid-cols-3 gap-3 lg:gap-4 shrink-0">
              <MetricCard
                label="CPU"
                value={`${systemMetrics.cpuUsagePercent.toFixed(0)}%`}
                onClick={navigateToCpuProcesses}
                actionHint="Open processes sorted by CPU usage"
                subtitle={
                  <>
                    <DeltaIcon delta={cpuDelta} />
                    <span>{Math.abs(cpuDelta).toFixed(1)}% from avg</span>
                  </>
                }
              />
              <MetricCard
                label="RAM"
                value={`${systemMetrics.memoryUsagePercent.toFixed(0)}%`}
                onClick={navigateToMemoryProcesses}
                actionHint="Open processes sorted by Memory usage"
                subtitle={
                  <>
                    <DeltaIcon delta={memDelta} />
                    <span>{Math.abs(memDelta).toFixed(1)}% from avg</span>
                  </>
                }
              />
              <MetricCard
                label="GPU"
                value={`${(systemMetrics.gpuUsagePercent ?? 0).toFixed(0)}%`}
                onClick={handleSelectGpu}
                actionHint="Inspect graphics and driver processes"
                subtitle={
                  <>
                    <DeltaIcon delta={gpuDelta} />
                    <span>{Math.abs(gpuDelta).toFixed(1)}% from avg</span>
                  </>
                }
              />
            </div>

            <div className="lg:flex-1 lg:min-h-0">
              <SystemHealth
                metrics={systemMetrics}
                history={systemHistory}
                onSelectCpu={navigateToCpuProcesses}
                onSelectMemory={navigateToMemoryProcesses}
              />
            </div>
          </div>

          <div className="order-1 lg:order-2 lg:col-span-4 min-h-[210px] lg:min-h-0">
            <RecommendationCarousel />
          </div>
        </div>

        {/* Row 2: temperatures + storage composition */}
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-3 lg:gap-4 lg:flex-1 lg:min-h-0">
          <div className="lg:col-span-4 min-h-0">
            <TemperaturePanel />
          </div>
          <div className="lg:col-span-8 min-h-0">
            <StorageComposition />
          </div>
        </div>

        {/* Row 3: battery + Nyra */}
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-3 lg:gap-4 shrink-0">
          <div className="lg:col-span-8">
            <BatteryStatusCard metrics={systemMetrics} />
          </div>
          <div className="lg:col-span-4">
            <NyraCtaCard />
          </div>
        </div>
      </div>
    </div>
  );
};
