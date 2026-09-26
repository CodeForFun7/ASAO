import React, { useMemo } from "react";
import { ArrowDown, ArrowUp, AlertTriangle, RefreshCw } from "lucide-react";
import { useProcessStore } from "../stores/process-store";
import { MetricCard } from "../components/dashboard/MetricCard";
import { SystemHealth } from "../components/dashboard/SystemHealth";
import { AIInsight } from "../components/dashboard/AIInsight";
import { ProcessAttention } from "../components/dashboard/ProcessAttention";

export const Dashboard: React.FC = () => {
  const systemMetrics = useProcessStore((s) => s.systemMetrics);
  const systemHistory = useProcessStore((s) => s.systemHistory);
  const processes = useProcessStore((s) => s.processes);
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
  const selectProcess = useProcessStore((s) => s.selectProcess);

  const attentionProcesses = useMemo(
    () => processes.filter((p) => p.status === "attention"),
    [processes]
  );

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

  const statusConfig =
    systemMetrics.systemStatus === "critical"
      ? { label: "Strained", dot: "bg-lunar-critical", sub: "Elevated system load" }
      : systemMetrics.systemStatus === "warning"
      ? { label: "Moderate Load", dot: "bg-lunar-warning", sub: "Resource thresholds active" }
      : { label: "Healthy", dot: "bg-lunar-healthy", sub: "Nominal operating parameters" };

  const handleSelectGpu = () => {
    setCategoryFilter("drivers");
    setRoute("processes");
  };

  return (
    <div className="flex-1 overflow-y-auto p-6 space-y-6">
      {/* Page Header */}
      <div className="flex flex-col sm:flex-row sm:items-end justify-between gap-2">
        <div>
          <h1 className="text-lg font-semibold text-lunar-white tracking-tight">
            Dashboard
          </h1>
          <p className="text-xs text-lunar-text-sec mt-0.5">
            System Overview — Your system is being monitored in real time.
          </p>
        </div>
        <div className="text-[11px] font-mono text-lunar-muted">
          PROTECTED PROCESSES: {systemMetrics.protectedProcesses} · HIGH LOAD:{" "}
          {systemMetrics.highResourceProcesses}
        </div>
      </div>

      {/* Top Metric Cards (CPU, MEMORY, GPU, PROCESSES, SYSTEM STATUS) */}
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
              <span>
                {Math.abs(cpuDelta).toFixed(1)}% from avg
              </span>
            </>
          }
        />

        {/* Memory Card */}
        <MetricCard
          label="MEMORY"
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
              <span>
                {Math.abs(memDelta).toFixed(1)}% from avg
              </span>
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
              <span>
                {Math.abs(gpuDelta).toFixed(1)}% from avg
              </span>
            </>
          }
        />

        {/* Processes Card */}
        <MetricCard
          label="PROCESSES"
          value={systemMetrics.totalProcesses}
          onClick={navigateToAttentionProcesses}
          actionHint="Filter processes requiring attention"
          subtitle={
            <span
              className={
                systemMetrics.attentionProcesses > 0
                  ? "text-lunar-warning"
                  : "text-lunar-text-sec"
              }
            >
              {systemMetrics.attentionProcesses} require attention
            </span>
          }
        />

        {/* System Status Card */}
        <MetricCard
          label="SYSTEM STATUS"
          value={statusConfig.label}
          statusDotColor={statusConfig.dot}
          subtitle={<span>{statusConfig.sub}</span>}
        />
      </div>

      {/* Live CPU, Memory & GPU Telemetry Charts */}
      <SystemHealth
        metrics={systemMetrics}
        history={systemHistory}
        onSelectCpu={navigateToCpuProcesses}
        onSelectMemory={navigateToMemoryProcesses}
        onSelectGpu={handleSelectGpu}
      />

      {/* Bottom Split: AI Insights + Process Attention */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        <AIInsight
          metrics={systemMetrics}
          attentionProcesses={attentionProcesses}
        />
        <ProcessAttention
          attentionProcesses={attentionProcesses}
          onViewAttentionProcesses={navigateToAttentionProcesses}
          onInspectProcess={(pid) => {
            selectProcess(pid);
            setRoute("processes");
          }}
        />
      </div>
    </div>
  );
};
