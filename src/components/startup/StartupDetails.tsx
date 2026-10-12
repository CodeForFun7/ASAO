import React, { useEffect, useState } from "react";
import {
  Activity,
  AlertTriangle,
  CheckCircle2,
  Clock,
  Copy,
  Cpu,
  Database,
  Flame,
  FolderSync,
  HardDrive,
  HelpCircle,
  Layers,
  Power,
  RotateCcw,
  Shield,
  ShieldAlert,
  ShieldCheck,
  Terminal,
  X,
  Zap,
} from "lucide-react";
import type { StartupGroup, StartupItem } from "../../types/startup";
import { SOURCE_LABELS } from "../../types/startup";
import { formatBytes } from "../../services/tauri";

interface StartupDetailsProps {
  item: StartupItem;
  isActionLoading: boolean;
  onClose: () => void;
  onToggleState: () => void;
}

function getGroupIcon(group: StartupGroup) {
  switch (group) {
    case "Registry Startup":
      return <Terminal className="w-4 h-4 text-lunar-text-sec" />;
    case "Startup Folder":
      return <FolderSync className="w-4 h-4 text-lunar-text-sec" />;
    case "Scheduled Tasks":
      return <Layers className="w-4 h-4 text-lunar-text-sec" />;
    case "Windows Services":
      return <Database className="w-4 h-4 text-lunar-text-sec" />;
    case "Winlogon / System Startup":
      return <ShieldCheck className="w-4 h-4 text-lunar-text-sec" />;
    case "Other Autostart Mechanisms":
    default:
      return <Cpu className="w-4 h-4 text-lunar-text-sec" />;
  }
}

export const StartupDetails: React.FC<StartupDetailsProps> = ({
  item,
  isActionLoading,
  onClose,
  onToggleState,
}) => {
  const [copiedKey, setCopiedKey] = useState(false);
  const [copiedPath, setCopiedPath] = useState(false);

  // Close on Escape key press (Obsidian window behavior)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  const sourceLabel = SOURCE_LABELS[item.source] ?? item.source;

  const copyToClipboard = async (text: string, type: "path" | "key") => {
    try {
      await navigator.clipboard.writeText(text);
      if (type === "path") {
        setCopiedPath(true);
        setTimeout(() => setCopiedPath(false), 1800);
      } else {
        setCopiedKey(true);
        setTimeout(() => setCopiedKey(false), 1800);
      }
    } catch {
      // Fallback
    }
  };

  // Severity config
  const impactConfig = {
    high: {
      icon: Flame,
      color: "text-lunar-critical",
      badge: "bg-lunar-critical/10 text-lunar-critical border-lunar-critical/25",
      label: "High Impact",
      summary:
        "Significantly contributes to boot-time disk queue contention and user logon delay.",
    },
    medium: {
      icon: AlertTriangle,
      color: "text-lunar-warning",
      badge: "bg-lunar-warning/10 text-lunar-warning border-lunar-warning/25",
      label: "Moderate Impact",
      summary: "Consumes moderate resources during user session startup.",
    },
    low: {
      icon: Zap,
      color: "text-lunar-healthy",
      badge: "bg-lunar-healthy/10 text-lunar-healthy border-lunar-healthy/25",
      label: "Low Impact",
      summary: "Minimal boot overhead or lightweight background daemon.",
    },
  }[item.impact];

  const classConfig = {
    essential: {
      icon: ShieldCheck,
      color: "text-lunar-ai",
      badge: "bg-lunar-ai/10 text-lunar-ai border-lunar-ai/25",
      label: "Essential Component",
      explanation:
        "Required for core Windows operating system functionality, display rendering, or security subsystems.",
    },
    optional: {
      icon: ShieldAlert,
      color: "text-lunar-critical",
      badge: "bg-lunar-critical/10 text-lunar-critical border-lunar-critical/25",
      label: "Optional Application",
      explanation:
        "Safe to disable from automatic boot. Can be launched manually on demand whenever you need it.",
    },
    "user-dependent": {
      icon: HelpCircle,
      color: "text-lunar-warning",
      badge: "bg-lunar-warning/10 text-lunar-warning border-lunar-warning/25",
      label: "User-Dependent Utility",
      explanation:
        "Background utility (cloud sync, hardware controller, or notifications). Disable if not needed immediately at logon.",
    },
  }[item.classification];

  const recConfig = {
    disable: {
      icon: ShieldAlert,
      badge: "bg-lunar-critical/10 text-lunar-critical border-lunar-critical/25",
      label: "Disable Recommended",
    },
    investigate: {
      icon: HelpCircle,
      badge: "bg-lunar-warning/10 text-lunar-warning border-lunar-warning/25",
      label: "Investigate",
    },
    keep: {
      icon: CheckCircle2,
      badge: "bg-lunar-healthy/10 text-lunar-healthy border-lunar-healthy/25",
      label: "Keep / Essential",
    },
  }[item.recommendation];

  const ImpactIcon = impactConfig.icon;
  const RecIcon = recConfig.icon;
  const ClassIcon = classConfig.icon;

  return (
    <div
      className="fixed inset-0 z-50 bg-black/75 backdrop-blur-xs flex items-center justify-center p-4 sm:p-6"
      onClick={onClose}
      role="dialog"
      aria-modal="true"
      aria-labelledby="modal-startup-title"
    >
      {/* Obsidian-Style Modal Window */}
      <div
        className="w-full max-w-3xl h-[88vh] max-h-[820px] bg-lunar-surface border border-lunar-border rounded-xl shadow-2xl flex flex-col overflow-hidden animate-in fade-in zoom-in-95 duration-150"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Window Title Bar */}
        <div className="px-6 py-4 border-b border-lunar-border flex items-center justify-between bg-lunar-surface-2/60 shrink-0">
          <div className="flex items-center gap-3 min-w-0">
            <div className="w-8 h-8 rounded-lg bg-lunar-elevated border border-lunar-border flex items-center justify-center shrink-0">
              {getGroupIcon(item.startupGroup)}
            </div>

            <div className="min-w-0">
              <div className="flex items-center gap-2">
                <h2
                  id="modal-startup-title"
                  className="text-sm font-semibold text-lunar-white tracking-tight truncate"
                >
                  {item.name}
                </h2>
                {item.isCurrentlyRunning && (
                  <span
                    className="w-2 h-2 rounded-full bg-lunar-healthy animate-pulse shrink-0 cursor-help"
                    title={
                      item.pid
                        ? `PID: ${item.pid} (Running, ${formatBytes(item.memoryBytes)} RAM)`
                        : "Running Process"
                    }
                  />
                )}
              </div>
              <p className="text-[11px] text-lunar-text-sec truncate font-mono mt-0.5">
                {item.publisher ?? "Unknown Publisher"} • {sourceLabel}
              </p>
            </div>
          </div>

          <div className="flex items-center gap-2 shrink-0">
            <kbd className="hidden sm:inline-block text-[10px] font-mono px-1.5 py-0.5 rounded bg-lunar-surface-2 border border-lunar-border text-lunar-muted">
              ESC
            </kbd>
            <button
              type="button"
              onClick={onClose}
              title="Close window (Esc)"
              className="p-1.5 rounded-md hover:bg-lunar-elevated text-lunar-muted hover:text-lunar-white transition-colors cursor-pointer"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </div>

        {/* Modal Window Scrollable Content */}
        <div className="flex-1 overflow-y-auto p-6 space-y-6 divide-y divide-lunar-border/30">
          {/* Section 1: Quick Status Banner & Toggle Control */}
          <div className="space-y-4">
            <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4 p-4 rounded-xl bg-lunar-surface-2/50 border border-lunar-border/60">
              <div className="space-y-1.5">
                <div className="flex items-center gap-2 flex-wrap">
                  {/* Enabled / Disabled Tag */}
                  <span
                    className={`inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded text-xs font-mono font-medium border ${
                      item.isEnabled
                        ? "bg-lunar-healthy/15 text-lunar-healthy border-lunar-healthy/30"
                        : "bg-lunar-surface-2 text-lunar-muted border-lunar-border"
                    }`}
                  >
                    <span
                      className={`w-1.5 h-1.5 rounded-full ${
                        item.isEnabled ? "bg-lunar-healthy" : "bg-lunar-muted"
                      }`}
                    />
                    <span>
                      {item.isEnabled
                        ? "Autostart Enabled"
                        : "Autostart Disabled"}
                    </span>
                  </span>

                  {/* Running state pill */}
                  {item.isCurrentlyRunning ? (
                    <span className="inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded text-xs font-mono bg-lunar-healthy/10 text-lunar-healthy border border-lunar-healthy/20">
                      <span>Active Memory (PID {item.pid ?? "Active"})</span>
                    </span>
                  ) : (
                    <span className="inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded text-xs font-mono bg-lunar-bg text-lunar-muted border border-lunar-border/60">
                      <span>Process Inactive</span>
                    </span>
                  )}

                  {/* Boot Timeline Phase Tag */}
                  {item.bootTimelinePhase && (
                    <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-[11px] font-mono bg-lunar-bg text-lunar-text-sec border border-lunar-border">
                      <span>{item.bootTimelinePhase}</span>
                    </span>
                  )}
                </div>

                <p className="text-xs text-lunar-text-sec">
                  {item.isEnabled
                    ? "This entry launches automatically during system boot or user logon."
                    : "This entry is suppressed in Windows StartupApproved registry and will not launch at boot."}
                </p>
              </div>

              {/* Action Button */}
              <div className="shrink-0">
                {item.classification === "essential" ? (
                  <div
                    className="inline-flex items-center gap-2 px-3 py-2 rounded-lg bg-lunar-bg border border-lunar-border text-xs text-lunar-muted cursor-help"
                    title="Protected core operating system component. Disabling is restricted to maintain system stability."
                  >
                    <Shield className="w-4 h-4 text-lunar-ai shrink-0" />
                    <span>Protected System Component</span>
                  </div>
                ) : (
                  <button
                    type="button"
                    disabled={isActionLoading}
                    onClick={onToggleState}
                    className={`inline-flex items-center justify-center gap-2 px-4 py-2 rounded-lg text-xs font-semibold transition-all border cursor-pointer ${
                      item.isEnabled
                        ? "bg-lunar-surface-2 hover:bg-lunar-critical/20 text-lunar-white hover:text-lunar-critical border-lunar-border hover:border-lunar-critical/40"
                        : "bg-lunar-healthy/15 hover:bg-lunar-healthy/25 text-lunar-healthy border-lunar-healthy/30"
                    }`}
                  >
                    {isActionLoading ? (
                      <RotateCcw className="w-3.5 h-3.5 animate-spin" />
                    ) : item.isEnabled ? (
                      <>
                        <Power className="w-3.5 h-3.5" />
                        <span>Disable Autostart</span>
                      </>
                    ) : (
                      <>
                        <RotateCcw className="w-3.5 h-3.5" />
                        <span>Re-enable Autostart</span>
                      </>
                    )}
                  </button>
                )}
              </div>
            </div>
          </div>

          {/* Section 2: Boot Activity & Impact Telemetry */}
          <div className="pt-6 space-y-4">
            <div className="flex items-center justify-between">
              <div>
                <h3 className="text-xs font-mono uppercase tracking-wider text-lunar-muted flex items-center gap-1.5">
                  <Activity className="w-3.5 h-3.5 text-lunar-text-sec" />
                  <span>Boot Activity & Impact Telemetry</span>
                </h3>
                <p className="text-xs text-lunar-text-sec mt-0.5">
                  Hardware resource load measured during operating system startup
                </p>
              </div>

              <span
                title={impactConfig.summary}
                className={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded text-xs font-mono font-medium border cursor-help ${impactConfig.badge}`}
              >
                <ImpactIcon className="w-3.5 h-3.5" />
                <span>{impactConfig.label}</span>
              </span>
            </div>

            {/* 4-Tile Telemetry Grid */}
            <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
              <div className="p-3 rounded-lg bg-lunar-surface-2/40 border border-lunar-border/60">
                <div className="text-[11px] text-lunar-muted font-mono flex items-center gap-1.5">
                  <Cpu className="w-3.5 h-3.5 text-lunar-text-sec" />
                  <span>CPU Time</span>
                </div>
                <div className="text-sm font-mono font-semibold text-lunar-white mt-1">
                  {item.bootCpuMs} ms
                </div>
                <div className="text-[10px] text-lunar-muted mt-0.5">
                  Initialization CPU load
                </div>
              </div>

              <div className="p-3 rounded-lg bg-lunar-surface-2/40 border border-lunar-border/60">
                <div className="text-[11px] text-lunar-muted font-mono flex items-center gap-1.5">
                  <HardDrive className="w-3.5 h-3.5 text-lunar-text-sec" />
                  <span>Disk I/O</span>
                </div>
                <div className="text-sm font-mono font-semibold text-lunar-white mt-1">
                  {formatBytes(item.bootDiskBytes)}
                </div>
                <div className="text-[10px] text-lunar-muted mt-0.5">
                  Disk read / write bytes
                </div>
              </div>

              <div className="p-3 rounded-lg bg-lunar-surface-2/40 border border-lunar-border/60">
                <div className="text-[11px] text-lunar-muted font-mono flex items-center gap-1.5">
                  <Database className="w-3.5 h-3.5 text-lunar-text-sec" />
                  <span>Memory</span>
                </div>
                <div className="text-sm font-mono font-semibold text-lunar-white mt-1">
                  {formatBytes(item.memoryBytes)}
                </div>
                <div className="text-[10px] text-lunar-muted mt-0.5">
                  Working set footprint
                </div>
              </div>

              <div className="p-3 rounded-lg bg-lunar-surface-2/40 border border-lunar-border/60">
                <div className="text-[11px] text-lunar-muted font-mono flex items-center gap-1.5">
                  <Clock className="w-3.5 h-3.5 text-lunar-text-sec" />
                  <span>Startup Delay</span>
                </div>
                <div className="text-sm font-mono font-semibold text-lunar-white mt-1">
                  ~{item.bootDurationMs} ms
                </div>
                <div className="text-[10px] text-lunar-muted mt-0.5">
                  Added boot latency
                </div>
              </div>
            </div>

            <p className="text-xs text-lunar-text-sec leading-relaxed">
              {impactConfig.summary}
            </p>
          </div>

          {/* Section 3: Assessment & Recommendation */}
          <div className="pt-6 space-y-4">
            <div>
              <h3 className="text-xs font-mono uppercase tracking-wider text-lunar-muted">
                Assessment & Recommendation
              </h3>
              <p className="text-xs text-lunar-text-sec mt-0.5">
                Evaluation based on component necessity, stability risk, and boot impact
              </p>
            </div>

            <div className="flex items-center gap-2.5 flex-wrap">
              <span
                className={`inline-flex items-center gap-1.5 px-3 py-1 rounded-md text-xs font-medium border ${recConfig.badge}`}
              >
                <RecIcon className="w-3.5 h-3.5" />
                <span>{recConfig.label}</span>
              </span>

              <span
                className={`inline-flex items-center gap-1.5 px-3 py-1 rounded-md text-xs font-medium border ${classConfig.badge}`}
              >
                <ClassIcon className="w-3.5 h-3.5" />
                <span>{classConfig.label}</span>
              </span>
            </div>

            <p className="text-xs text-lunar-text leading-relaxed">
              {classConfig.explanation}
            </p>

            <div className="flex items-center gap-2 text-xs text-lunar-text-sec">
              <span className="text-lunar-muted font-mono">Usage Frequency:</span>
              <span className="capitalize font-mono text-lunar-text">
                {item.usageFrequency}
              </span>
            </div>
          </div>

          {/* Section 4: Configuration & Origin */}
          <div className="pt-6 space-y-4">
            <div>
              <h3 className="text-xs font-mono uppercase tracking-wider text-lunar-muted">
                Configuration & Origin
              </h3>
              <p className="text-xs text-lunar-text-sec mt-0.5">
                Autostart registration source, file path, and command arguments
              </p>
            </div>

            <div className="space-y-3">
              {/* Executable Path */}
              {item.executablePath && (
                <div className="space-y-1">
                  <div className="text-[11px] text-lunar-muted flex items-center justify-between">
                    <span>Executable Path:</span>
                    <button
                      type="button"
                      onClick={() => copyToClipboard(item.executablePath!, "path")}
                      className="inline-flex items-center gap-1 text-[11px] text-lunar-text-sec hover:text-lunar-white cursor-pointer"
                    >
                      <Copy className="w-3 h-3" />
                      <span>{copiedPath ? "Copied!" : "Copy Path"}</span>
                    </button>
                  </div>
                  <div className="p-2.5 rounded-lg bg-lunar-surface-2/60 border border-lunar-border font-mono text-xs text-lunar-text break-all select-text leading-relaxed">
                    {item.executablePath}
                  </div>
                </div>
              )}

              {/* Command Line / Arguments */}
              {item.commandLine && item.commandLine !== item.executablePath && (
                <div className="space-y-1">
                  <div className="text-[11px] text-lunar-muted">Command Line Arguments:</div>
                  <div className="p-2.5 rounded-lg bg-lunar-surface-2/60 border border-lunar-border font-mono text-xs text-lunar-text-sec break-all select-text leading-relaxed">
                    {item.commandLine}
                  </div>
                </div>
              )}

              {/* Registry Key / Task Path / Service Name */}
              {(item.registryKey || item.taskPath || item.serviceName) && (
                <div className="space-y-1">
                  <div className="text-[11px] text-lunar-muted flex items-center justify-between">
                    <span>
                      {item.registryKey
                        ? "Registry Location:"
                        : item.serviceName
                        ? "Service Identifier:"
                        : "Task Path:"}
                    </span>
                    <button
                      type="button"
                      onClick={() =>
                        copyToClipboard(
                          (item.registryKey || item.serviceName || item.taskPath)!,
                          "key"
                        )
                      }
                      className="inline-flex items-center gap-1 text-[11px] text-lunar-text-sec hover:text-lunar-white cursor-pointer"
                    >
                      <Copy className="w-3 h-3" />
                      <span>{copiedKey ? "Copied!" : "Copy"}</span>
                    </button>
                  </div>
                  <div className="p-2.5 rounded-lg bg-lunar-surface-2/60 border border-lunar-border font-mono text-xs text-lunar-text break-all select-text leading-relaxed">
                    {item.registryKey || item.serviceName || item.taskPath}
                  </div>
                </div>
              )}
            </div>
          </div>

          {/* Section 5: Optimization & Recovery Guidelines */}
          <div className="pt-6 space-y-4">
            <div>
              <h3 className="text-xs font-mono uppercase tracking-wider text-lunar-muted">
                Optimization & Recovery Guidelines
              </h3>
              <p className="text-xs text-lunar-text-sec mt-0.5">
                Technical disable mechanism, operational side-effects, and restore procedure
              </p>
            </div>

            <div className="space-y-3">
              <div className="border-l-2 border-lunar-border pl-3.5 space-y-1">
                <div className="text-[11px] font-mono uppercase tracking-wider text-lunar-muted">
                  How it is disabled
                </div>
                <div className="text-xs text-lunar-text leading-relaxed">
                  {item.disableMethod}
                </div>
              </div>

              <div className="border-l-2 border-lunar-warning/40 pl-3.5 space-y-1">
                <div className="text-[11px] font-mono uppercase tracking-wider text-lunar-warning/80">
                  Potential consequences of disabling
                </div>
                <div className="text-xs text-lunar-text-sec leading-relaxed">
                  {item.disableConsequences}
                </div>
              </div>

              <div className="border-l-2 border-lunar-healthy/40 pl-3.5 space-y-1">
                <div className="text-[11px] font-mono uppercase tracking-wider text-lunar-healthy/80">
                  Safe restore / re-enable method
                </div>
                <div className="text-xs text-lunar-healthy/90 leading-relaxed font-mono">
                  {item.restoreMethod}
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* Modal Window Footer */}
        <div className="px-6 py-3 border-t border-lunar-border flex items-center justify-between bg-lunar-surface-2/40 shrink-0">
          <span className="text-[11px] text-lunar-muted font-mono">
            Origin: {sourceLabel}
          </span>
          <button
            type="button"
            onClick={onClose}
            className="px-4 py-1.5 rounded-lg text-xs font-medium bg-lunar-surface-2 hover:bg-lunar-elevated text-lunar-white border border-lunar-border transition-colors cursor-pointer"
          >
            Done
          </button>
        </div>
      </div>
    </div>
  );
};
