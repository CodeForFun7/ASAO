import { useEffect } from "react";
import { Database, ShieldCheck, Sparkles, Sliders } from "lucide-react";
import { AppShell } from "./components/layout/AppShell";
import { Dashboard } from "./pages/Dashboard";
import { Processes } from "./pages/Processes";
import { useProcessStore } from "./stores/process-store";

function SettingsPlaceholder() {
  return (
    <div className="flex-1 overflow-y-auto p-6 space-y-6">
      <div>
        <h1 className="text-lg font-semibold text-lunar-white tracking-tight">
          System Architecture &amp; Instrumentation Settings
        </h1>
        <p className="text-xs text-lunar-text-sec mt-0.5">
          ASAO v0.1 — Read-only telemetry analyzer configuration and subsystem readiness.
        </p>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div className="rounded-lg bg-lunar-surface border border-lunar-border p-5 space-y-2">
          <div className="flex items-center gap-2 text-xs font-mono uppercase tracking-wider text-lunar-white">
            <Sliders className="w-3.5 h-3.5 text-lunar-text-sec" />
            <span>Win32 Telemetry Engine</span>
          </div>
          <p className="text-xs text-lunar-text-sec leading-relaxed">
            Sampling interval locked at 1.0Hz (1000ms) using native Windows ToolHelp32 snapshots,
            GetSystemTimes, GlobalMemoryStatusEx, and PE Version Info caching.
          </p>
          <div className="text-[11px] font-mono text-lunar-healthy pt-1">
            ● ACTIVE — NORMAL USER PERMISSIONS
          </div>
        </div>

        <div className="rounded-lg bg-lunar-surface border border-lunar-border p-5 space-y-2">
          <div className="flex items-center gap-2 text-xs font-mono uppercase tracking-wider text-lunar-ai">
            <Sparkles className="w-3.5 h-3.5 text-lunar-ai" />
            <span>Future LLM Service Boundary</span>
          </div>
          <p className="text-xs text-lunar-text-sec leading-relaxed">
            ProcessAnalysisService abstraction is active with MockProcessAnalysisService.
            Ready to connect OpenAI or local inference without UI changes.
          </p>
          <div className="text-[11px] font-mono text-lunar-ai pt-1">
            ● INTERFACE READY — STANDBY
          </div>
        </div>

        <div className="rounded-lg bg-lunar-surface border border-lunar-border p-5 space-y-2">
          <div className="flex items-center gap-2 text-xs font-mono uppercase tracking-wider text-lunar-white">
            <Database className="w-3.5 h-3.5 text-lunar-text-sec" />
            <span>SQLite Historical Layer</span>
          </div>
          <p className="text-xs text-lunar-text-sec leading-relaxed">
            Schema prepared for applications, process_samples, application_usage,
            recommendations, and optimization_history. Live metrics stream in-memory.
          </p>
          <div className="text-[11px] font-mono text-lunar-text-sec pt-1">
            ● IN-MEMORY RING BUFFER (30S WINDOW)
          </div>
        </div>

        <div className="rounded-lg bg-lunar-surface border border-lunar-border p-5 space-y-2">
          <div className="flex items-center gap-2 text-xs font-mono uppercase tracking-wider text-lunar-healthy">
            <ShieldCheck className="w-3.5 h-3.5 text-lunar-healthy" />
            <span>Safety Policy</span>
          </div>
          <p className="text-xs text-lunar-text-sec leading-relaxed">
            Analyzer-only mode enforced. Destructive actions (kill process, disable service,
            registry modification) are disabled until the backup &amp; rollback engine is active.
          </p>
          <div className="text-[11px] font-mono text-lunar-healthy pt-1">
            ● READ-ONLY GUARDRAILS ENFORCED
          </div>
        </div>
      </div>
    </div>
  );
}

function App() {
  const currentRoute = useProcessStore((s) => s.currentRoute);
  const initializeMonitoring = useProcessStore((s) => s.initializeMonitoring);

  useEffect(() => {
    let cleanup: (() => void) | undefined;
    let mounted = true;

    void initializeMonitoring().then((unlisten) => {
      if (mounted) {
        cleanup = unlisten;
      } else {
        unlisten();
      }
    });

    return () => {
      mounted = false;
      if (cleanup) {
        cleanup();
      }
    };
  }, [initializeMonitoring]);

  return (
    <AppShell>
      {currentRoute === "dashboard" && <Dashboard />}
      {currentRoute === "processes" && <Processes />}
      {currentRoute === "settings" && <SettingsPlaceholder />}
    </AppShell>
  );
}

export default App;
