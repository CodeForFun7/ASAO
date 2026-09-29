import { getBootPerformanceSummary, getStartupItems } from "../startup";
import { formatBytes, getFullSnapshot, getSystemMetrics } from "../tauri";
import { formatStorageBytes, getStorageDrives, getStorageSnapshot, openStorageLocation } from "../storage";
import type { ToolCallDeclaration } from "../../types/agent";

export interface ToolExecutionResponse {
  toolName: string;
  displayLabel: string;
  summary: string;
  data: unknown;
}

export const ADK_DIAGNOSTIC_TOOLS: ToolCallDeclaration[] = [
  {
    name: "check_startup_applications",
    description:
      "Inspect all registered Windows startup items, autostart programs, services, scheduled tasks, and boot performance metrics. Use this when the user asks about boot delay, startup apps, why boot is slow, or overall slow computer startup.",
    parameters: {
      type: "OBJECT",
      properties: {
        filter: {
          type: "STRING",
          description: "Optional filter for startup items: 'all', 'high_impact', or 'running'",
          enum: ["all", "high_impact", "running"],
        },
      },
    },
  },
  {
    name: "check_background_processes",
    description:
      "Analyze current running processes on the system, identifying background processes with high CPU or memory consumption, processes needing attention, foreground vs background workload, and system strain. Use this when the user asks why the computer feels slow, what processes are using resources, or questions about RAM and CPU.",
    parameters: {
      type: "OBJECT",
      properties: {
        filter: {
          type: "STRING",
          description: "Optional filter: 'attention' for anomalous or high background load processes, 'all', or 'high_resource'",
          enum: ["attention", "high_resource", "all"],
        },
        limit: {
          type: "INTEGER",
          description: "Maximum number of processes to return (default 8)",
        },
      },
    },
  },
  {
    name: "check_storage_status",
    description:
      "Inspect storage disk capacity, free vs used space, usage percentage across drives, category breakdown (System, Application, User, Cache, Temporary), largest folders, and largest files. Use this when the user asks about disk space, what is taking up storage, storage cleanup, or low disk space.",
    parameters: {
      type: "OBJECT",
      properties: {
        drive: {
          type: "STRING",
          description: "Drive letter or identifier (e.g., 'C' or 'C:'). If omitted, checks all drives and active snapshot.",
        },
      },
    },
  },
  {
    name: "check_storage_insights",
    description:
      "Retrieve actionable storage cleanup insights such as redundant system cache, temporary files, installer cache, and high-impact storage hogs. Use this when the user asks for storage cleanup or freeing up disk space.",
    parameters: {
      type: "OBJECT",
      properties: {
        drive: {
          type: "STRING",
          description: "Drive letter (e.g. 'C').",
        },
      },
    },
  },
  {
    name: "check_system_health_overview",
    description:
      "Quickly retrieve global real-time system performance metrics: overall system strain, CPU usage, RAM usage, GPU usage, foreground session load, background session load, and OS condition reason. Essential for broad performance queries like 'My computer feels slow'.",
    parameters: {
      type: "OBJECT",
      properties: {},
    },
  },
];

/**
 * Execute a diagnosed tool call using existing application services and APIs.
 */
export async function executeDiagnosticTool(
  toolName: string,
  args: Record<string, unknown>
): Promise<ToolExecutionResponse> {
  switch (toolName) {
    case "check_startup_applications": {
      const [items, summary] = await Promise.all([
        getStartupItems().catch(() => []),
        getBootPerformanceSummary().catch(() => null),
      ]);

      const filter = (args.filter as string) || "all";
      let filteredItems = items;
      if (filter === "high_impact") {
        filteredItems = items.filter((i) => i.impact === "high");
      } else if (filter === "running") {
        filteredItems = items.filter((i) => i.isCurrentlyRunning);
      }

      const highImpact = items.filter((i) => i.impact === "high");
      const disabledCount = items.filter((i) => !i.isEnabled).length;
      const runningCount = items.filter((i) => i.isCurrentlyRunning).length;

      const formattedHighImpact = highImpact.map((item) => ({
        id: item.id,
        name: item.name,
        impact: item.impact,
        bootDurationMs: item.bootDurationMs,
        bootCpuMs: item.bootCpuMs,
        bootDiskBytes: formatBytes(item.bootDiskBytes),
        memoryBytes: formatBytes(item.memoryBytes),
        isEnabled: item.isEnabled,
        isCurrentlyRunning: item.isCurrentlyRunning,
        recommendation: item.recommendation,
        classification: item.classification,
        disableMethod: item.disableMethod,
        disableConsequences: item.disableConsequences,
        source: item.source,
      }));

      const topOffenders = highImpact.slice(0, 5).map((i) => i.name).join(", ");

      return {
        toolName,
        displayLabel: "Checking startup applications",
        summary: `Analyzed ${items.length} startup items (${highImpact.length} high boot impact, ${runningCount} currently running, ${disabledCount} disabled).${topOffenders ? ` Top delays: ${topOffenders}.` : ""}`,
        data: {
          totalStartupItems: items.length,
          highImpactCount: highImpact.length,
          disabledCount,
          runningCount,
          estimatedBootDelayMs: summary?.estimatedBootDelayMs ?? (highImpact.length * 1250),
          lastBootTime: summary?.lastBootTime ?? "Recent boot",
          highImpactItems: formattedHighImpact,
          sampledItems: filteredItems.slice(0, 10).map((i) => ({
            id: i.id,
            name: i.name,
            impact: i.impact,
            isEnabled: i.isEnabled,
            recommendation: i.recommendation,
            bootDurationMs: i.bootDurationMs,
          })),
        },
      };
    }

    case "check_background_processes": {
      const snapshot = await getFullSnapshot().catch(() => null);
      const metrics = snapshot?.metrics ?? (await getSystemMetrics().catch(() => null));
      const processes = snapshot?.processes ?? [];

      const filter = (args.filter as string) || "attention";
      const limit = Number(args.limit) || 8;

      let candidateProcesses = processes;
      if (filter === "attention") {
        candidateProcesses = processes.filter(
          (p) => p.status === "attention" || p.status === "high-resource" || (p.sustainedCpuPercent ?? p.cpuPercent) > 4
        );
        if (candidateProcesses.length === 0) {
          candidateProcesses = processes.filter((p) => p.activityState === "background" || p.status === "background");
        }
      } else if (filter === "high_resource") {
        candidateProcesses = processes.filter(
          (p) => p.status === "high-resource" || p.status === "attention" || p.cpuPercent > 10 || p.memoryBytes > 500 * 1024 * 1024
        );
      }

      // Sort by sustained CPU / background impact
      candidateProcesses.sort((a, b) => {
        const aScore = (a.backgroundImpactScore ?? 0) + (a.sustainedCpuPercent ?? a.cpuPercent) * 2;
        const bScore = (b.backgroundImpactScore ?? 0) + (b.sustainedCpuPercent ?? b.cpuPercent) * 2;
        return bScore - aScore;
      });

      const topSelected = candidateProcesses.slice(0, limit).map((p) => ({
        pid: p.pid,
        name: p.name,
        category: p.category,
        status: p.status,
        activityState: p.activityState ?? "background",
        cpuPercent: Number(p.cpuPercent.toFixed(1)),
        sustainedCpuPercent: Number((p.sustainedCpuPercent ?? p.cpuPercent).toFixed(1)),
        memoryFormatted: formatBytes(p.memoryBytes),
        memoryBytes: p.memoryBytes,
        isSystemCritical: p.isSystemCritical,
        backgroundImpactScore: p.backgroundImpactScore ?? 0,
        sustainedLoadSeconds: p.sustainedLoadSeconds ?? 0,
      }));

      const topName = topSelected[0]?.name;
      const bgLoad = metrics?.backgroundLoadPercent ?? 0;

      return {
        toolName,
        displayLabel: "Checking background processes",
        summary: `Inspected ${processes.length} active processes. Background load is at ${bgLoad.toFixed(0)}%.${topName ? ` Top background consumer: ${topName}.` : " No heavy background outliers detected."}`,
        data: {
          totalProcesses: processes.length,
          backgroundLoadPercent: bgLoad,
          foregroundLoadPercent: metrics?.foregroundLoadPercent ?? 0,
          systemStrainPercent: metrics?.systemStrainPercent ?? 0,
          foregroundProcess: metrics?.foregroundProcessName ?? "Active Desktop",
          topProcesses: topSelected,
        },
      };
    }

    case "check_storage_status": {
      const [drives, snapshot] = await Promise.all([
        getStorageDrives().catch(() => []),
        getStorageSnapshot(args.drive as string | undefined).catch(() => null),
      ]);

      const targetDrive = drives.find(
        (d) => !args.drive || d.drive.toLowerCase().startsWith(String(args.drive).toLowerCase().charAt(0))
      ) ?? drives[0];

      const categorySlices = snapshot?.distribution?.map((c) => ({
        category: c.category,
        label: c.label,
        sizeFormatted: formatStorageBytes(c.bytes),
        percentage: Number(c.percentage.toFixed(1)),
      })) ?? [];

      const topLargeFiles = (snapshot?.largestFiles ?? []).slice(0, 6).map((f) => ({
        name: f.name,
        path: f.path,
        sizeFormatted: formatStorageBytes(f.size),
        category: f.category,
        importance: f.importance,
      }));

      const topFolders = (snapshot?.largestFolders ?? []).slice(0, 5).map((fold) => ({
        name: fold.name,
        path: fold.path,
        sizeFormatted: formatStorageBytes(fold.size),
        percentage: 0,
      }));

      const usedPct = targetDrive ? targetDrive.usagePercentage : (snapshot?.selectedDrive ? 75 : 0);

      return {
        toolName,
        displayLabel: "Checking storage",
        summary: targetDrive
          ? `Drive ${targetDrive.drive} is at ${usedPct.toFixed(0)}% capacity (${formatStorageBytes(targetDrive.usedCapacity)} used / ${formatStorageBytes(targetDrive.freeCapacity)} free).`
          : "Storage drives inspected.",
        data: {
          drives: drives.map((d) => ({
            drive: d.drive,
            mountPoint: d.mountPoint,
            total: formatStorageBytes(d.totalCapacity),
            used: formatStorageBytes(d.usedCapacity),
            free: formatStorageBytes(d.freeCapacity),
            usagePercentage: Number(d.usagePercentage.toFixed(1)),
          })),
          selectedDrive: targetDrive?.drive ?? "C:",
          categories: categorySlices,
          largestFiles: topLargeFiles,
          largestFolders: topFolders,
        },
      };
    }

    case "check_storage_insights": {
      const snapshot = await getStorageSnapshot(args.drive as string | undefined).catch(() => null);
      const insights = snapshot?.insights ?? [];

      const formattedInsights = insights.map((ins) => ({
        id: ins.id,
        title: ins.title,
        description: ins.description,
        reclaimableFormatted: formatStorageBytes(ins.size),
        reclaimableBytes: ins.size,
        importance: ins.importance,
        action: ins.action,
        targetPath: ins.targetPath,
        affectedItemCount: ins.affectedItems?.length ?? 0,
      }));

      const totalReclaimable = insights.reduce((acc, curr) => acc + curr.size, 0);

      return {
        toolName,
        displayLabel: "Analyzing storage cleanup opportunities",
        summary: `Found ${insights.length} storage optimization insights (~${formatStorageBytes(totalReclaimable)} potentially reclaimable space).`,
        data: {
          insightsCount: insights.length,
          totalReclaimableFormatted: formatStorageBytes(totalReclaimable),
          insights: formattedInsights,
        },
      };
    }

    case "check_system_health_overview": {
      const metrics = await getSystemMetrics().catch(() => null);

      if (!metrics) {
        return {
          toolName,
          displayLabel: "Checking system metrics",
          summary: "System metrics currently unavailable.",
          data: {},
        };
      }

      const strain = metrics.systemStrainPercent ?? Math.round(metrics.cpuUsagePercent * 0.45 + metrics.memoryUsagePercent * 0.4);
      const condition = metrics.systemStatus ?? (strain > 70 ? "critical" : strain > 40 ? "warning" : "healthy");

      return {
        toolName,
        displayLabel: "Checking system metrics & condition",
        summary: `System Strain is ${strain}% (${condition}). CPU: ${metrics.cpuUsagePercent.toFixed(0)}%, RAM: ${metrics.memoryUsagePercent.toFixed(0)}% (${formatBytes(metrics.memoryUsedBytes)} / ${formatBytes(metrics.memoryTotalBytes)}).`,
        data: {
          cpuUsagePercent: Number(metrics.cpuUsagePercent.toFixed(1)),
          memoryUsagePercent: Number(metrics.memoryUsagePercent.toFixed(1)),
          memoryUsed: formatBytes(metrics.memoryUsedBytes),
          memoryTotal: formatBytes(metrics.memoryTotalBytes),
          gpuUsagePercent: Number((metrics.gpuUsagePercent ?? 0).toFixed(1)),
          foregroundLoadPercent: Number((metrics.foregroundLoadPercent ?? 0).toFixed(1)),
          backgroundLoadPercent: Number((metrics.backgroundLoadPercent ?? 0).toFixed(1)),
          systemStrainPercent: strain,
          condition,
          conditionReason: metrics.systemStatus === "critical" ? "High resource strain detected" : "Nominal execution parameters",
          foregroundProcessName: metrics.foregroundProcessName ?? "Active App",
        },
      };
    }

    default:
      throw new Error(`Unknown diagnostic tool: ${toolName}`);
  }
}

/**
 * Execute an actionable fix directly via existing APIs.
 */
export async function executeOpenLocationFix(path: string): Promise<void> {
  return openStorageLocation(path);
}
