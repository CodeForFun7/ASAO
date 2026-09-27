import React from "react";
import { AlertTriangle, RefreshCw } from "lucide-react";
import { useProcessStore } from "../stores/process-store";
import { SystemHealth } from "../components/dashboard/SystemHealth";
import { SystemStrainPanel } from "../components/dashboard/SystemStrainPanel";
import { DashboardBottomRow } from "../components/dashboard/DashboardBottomRow";

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

  const handleSelectGpu = () => {
    setCategoryFilter("drivers");
    setRoute("processes");
  };

  return (
    <div className="flex-1 overflow-y-auto p-6 space-y-4">
      {/* Page Header */}
      <div>
        <h1 className="text-lg font-semibold text-lunar-white tracking-tight">
          Dashboard
        </h1>
        <p className="text-xs text-lunar-text-sec mt-0.5">
          System Overview — Your system is being monitored in real time.
        </p>
      </div>

      {/* Upper Grid: Left Container (CPU/RAM Row + GPU/Network/Disk Row) | Right Container (System Strain Chart + Processes) */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-4 items-stretch">
        <div className="lg:col-span-8 flex flex-col">
          <SystemHealth
            metrics={systemMetrics}
            history={systemHistory}
            processes={processes}
            onSelectCpu={navigateToCpuProcesses}
            onSelectMemory={navigateToMemoryProcesses}
            onSelectGpu={handleSelectGpu}
            onSelectProcesses={() => setRoute("processes")}
          />
        </div>

        <div className="lg:col-span-4 flex flex-col">
          <SystemStrainPanel
            metrics={systemMetrics}
            onSelectProcesses={navigateToAttentionProcesses}
          />
        </div>
      </div>

      {/* Bottom Row: Left (Recommendation Here) | Right (AI Assistant Here) */}
      <DashboardBottomRow
        metrics={systemMetrics}
        processes={processes}
        onInspectProcess={(pid) => {
          selectProcess(pid);
          setRoute("processes");
        }}
        onOpenProcesses={() => setRoute("processes")}
      />
    </div>
  );
};


