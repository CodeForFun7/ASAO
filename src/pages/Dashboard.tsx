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
import { SystemStrainPanel } from "../components/dashboard/SystemStrainPanel";

export const Dashboard: React.FC = () => {
  const systemMetrics = useProcessStore((s) => s.systemMetrics);
  const systemHistory = useProcessStore((s) => s.systemHistory);
  const monitoringStatus = useProcessStore((s) => s.monitoringStatus);
  const errorMessage = useProcessStore((s) => s.errorMessage);
  const retryMonitoring = useProcessStore((s) => s.retryMonitoring);

  const navigateToAttentionProcesses = useProcessStore(
    (s) => s.navigateToAttentionProcesses
  );
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

  return (
    <div className="flex-1 overflow-y-auto p-6 space-y-6">
      {/* Page Header */}
      <div>
        <h1 className="text-lg font-semibold text-lunar-white tracking-tight">
          Dashboard
        </h1>
        <p className="text-xs text-lunar-text-sec mt-0.5">
          System Overview — Your system is being monitored in real time.
        </p>
      </div>

      {/* Main Grid: Left (CPU/RAM/GPU Cards + Physical-Activity-style Charts) | Right (System Strain Semi-Circle Gauge + Processes) */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-4 items-stretch">
        {/* Left Column: 3 Metric Cards on top + Telemetry Charts Card below */}
        <div className="lg:col-span-8 flex flex-col gap-4">
          {/* CPU, RAM, GPU Cards */}
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
            {/* CPU Card */}
            <MetricCard
              label="CPU"
              value={`${systemMetrics.cpuUsagePercent.toFixed(0)}%`}
              onClick={navigateToCpuProcesses}
              actionHint="Open processes sorted by CPU usage"
              subtitle={
                <>
                  {cpuDelta <= 0 ? (
                    <ArrowDown className="w-3 h-3 text-lunar-healthy shrink-0" />
                  ) : (
                    <ArrowUp className="w-3 h-3 text-lunar-warning shrink-0" />
                  )}
                  <span>{Math.abs(cpuDelta).toFixed(1)}% from avg</span>
                </>
              }
            />

            {/* RAM Card */}
            <MetricCard
              label="RAM"
              value={`${systemMetrics.memoryUsagePercent.toFixed(0)}%`}
              onClick={navigateToMemoryProcesses}
              actionHint="Open processes sorted by Memory usage"
              subtitle={
                <>
                  {memDelta <= 0 ? (
                    <ArrowDown className="w-3 h-3 text-lunar-healthy shrink-0" />
                  ) : (
                    <ArrowUp className="w-3 h-3 text-lunar-warning shrink-0" />
                  )}
                  <span>{Math.abs(memDelta).toFixed(1)}% from avg</span>
                </>
              }
            />

            {/* GPU Card */}
            <MetricCard
              label="GPU"
              value={`${(systemMetrics.gpuUsagePercent ?? 0).toFixed(0)}%`}
              onClick={handleSelectGpu}
              actionHint="Inspect graphics and driver processes"
              subtitle={
                <>
                  {gpuDelta <= 0 ? (
                    <ArrowDown className="w-3 h-3 text-lunar-healthy shrink-0" />
                  ) : (
                    <ArrowUp className="w-3 h-3 text-lunar-warning shrink-0" />
                  )}
                  <span>{Math.abs(gpuDelta).toFixed(1)}% from avg</span>
                </>
              }
            />
          </div>

          {/* Below Cards: Charts of CPU, GPU, and RAM (Physical Activity structure) */}
          <SystemHealth
            metrics={systemMetrics}
            history={systemHistory}
            onSelectCpu={navigateToCpuProcesses}
            onSelectMemory={navigateToMemoryProcesses}
            onSelectGpu={handleSelectGpu}
          />
        </div>

        {/* Right Column: System Strain (Semi-Circle Gauge like Sleep Time) & Processes Card */}
        <div className="lg:col-span-4 flex flex-col">
          <SystemStrainPanel
            metrics={systemMetrics}
            onSelectProcesses={navigateToAttentionProcesses}
          />
        </div>
      </div>
    </div>
  );
};

