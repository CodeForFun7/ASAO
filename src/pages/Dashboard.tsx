import React from "react";
import {
  ArrowDown,
  ArrowUp,
  AlertTriangle,
  RefreshCw,
} from "lucide-react";
import { useProcessStore } from "../stores/process-store";
import { formatBytes, formatRate } from "../services/tauri";
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
  const networkRate = systemMetrics.networkBytesPerSec ?? 0;
  const diskRate = systemMetrics.diskBytesPerSec ?? 0;

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

      {/* Top Row: 5 Metric Cards (CPU, RAM, GPU, NETWORK, DISK) */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-5 gap-4">
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
            <span>
              {formatBytes(systemMetrics.memoryUsedBytes)} ·{" "}
              {Math.abs(memDelta).toFixed(1)}% Δ
            </span>
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

        {/* Network Card */}
        <MetricCard
          label="NETWORK"
          value={formatRate(networkRate)}
          onClick={navigateToCpuProcesses}
          actionHint="Inspect active network processes"
          subtitle={<span>Live socket &amp; I/O throughput</span>}
        />

        {/* Disk Card */}
        <MetricCard
          label="DISK"
          value={formatRate(diskRate)}
          onClick={navigateToCpuProcesses}
          actionHint="Inspect active disk I/O processes"
          subtitle={<span>Read &amp; write transfer rate</span>}
        />
      </div>

      {/* Bottom Row: Left (5 Graphs: CPU, RAM, GPU, Network, Disk) | Right (System Strain & Processes Panel) */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-4 items-stretch">
        {/* Left Column: Unified Telemetry Graphs Card */}
        <div className="lg:col-span-8 flex flex-col">
          <SystemHealth
            metrics={systemMetrics}
            history={systemHistory}
            onSelectCpu={navigateToCpuProcesses}
            onSelectMemory={navigateToMemoryProcesses}
            onSelectGpu={handleSelectGpu}
            onSelectNetwork={navigateToCpuProcesses}
            onSelectDisk={navigateToCpuProcesses}
          />
        </div>

        {/* Right Column: System Strain & Processes Panel */}
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


