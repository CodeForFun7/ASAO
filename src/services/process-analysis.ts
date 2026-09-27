import {
  CATEGORY_METADATA,
  type LoadImpactLevel,
  type ProcessActivityState,
  type ProcessAnalysis,
  type ProcessAnalysisService,
  type ProcessInfo,
  type StructuredProcessContext,
  type SystemMetrics,
} from "../types/process";
import { formatBytes } from "./tauri";

export interface LlmExplanationAdapter {
  explainProcessLoad(context: StructuredProcessContext): Promise<ProcessAnalysis>;
  explainSystemStrain(
    metrics: SystemMetrics,
    backgroundCandidates: StructuredProcessContext[]
  ): Promise<ProcessAnalysis>;
}

/**
 * Layered Process & Background Load Analysis Service.
 *
 * 1. Builds compact, structured context from the Rust Background Process Analyzer.
 * 2. Works 100% standalone without an LLM using deterministic background-load rules.
 * 3. When an optional `LlmExplanationAdapter` is connected, delegates natural-language
 *    explanation generation to the LLM using the pre-computed structured context.
 */
export class HybridProcessAnalysisService implements ProcessAnalysisService {
  private llmAdapter: LlmExplanationAdapter | null = null;

  registerLlmAdapter(adapter: LlmExplanationAdapter): void {
    this.llmAdapter = adapter;
  }

  buildProcessContext(process: ProcessInfo): StructuredProcessContext {
    const sustainedCpu = process.sustainedCpuPercent ?? process.cpuPercent;
    const activityState: ProcessActivityState =
      process.activityState ??
      (process.status === "active"
        ? "foreground"
        : process.status === "background"
        ? "inactive"
        : "background");
    const sustainedLoadSeconds = process.sustainedLoadSeconds ?? 0;
    const backgroundImpactScore = process.backgroundImpactScore ?? 0;
    const impactLevel: LoadImpactLevel =
      process.impactLevel ??
      (backgroundImpactScore >= 45
        ? "high"
        : backgroundImpactScore >= 20
        ? "moderate"
        : "low");

    return {
      pid: process.pid,
      name: process.name,
      category: process.category,
      status: process.status,
      activityState,
      cpuPercent: process.cpuPercent,
      sustainedCpuPercent: sustainedCpu,
      memoryBytes: process.memoryBytes,
      diskBytesPerSec: process.diskBytesPerSec,
      networkBytesPerSec: process.networkBytesPerSec,
      sustainedLoadSeconds,
      backgroundImpactScore,
      impactLevel,
      isSystemCritical: process.isSystemCritical,
      isStartup: process.isStartup,
      publisher: process.publisher,
      productName: process.productName,
    };
  }

  async analyzeProcess(process: ProcessInfo): Promise<ProcessAnalysis> {
    const ctx = this.buildProcessContext(process);

    if (this.llmAdapter) {
      return this.llmAdapter.explainProcessLoad(ctx);
    }

    await new Promise((resolve) => setTimeout(resolve, 180));

    const catLabel =
      CATEGORY_METADATA[ctx.category]?.label ?? "Application Process";
    const memFormatted = formatBytes(ctx.memoryBytes);

    if (ctx.isSystemCritical) {
      return {
        summary: `${ctx.name} is an essential Windows core service maintaining operating system stability.`,
        reason: `Sustained CPU: ${ctx.sustainedCpuPercent.toFixed(
          1
        )}% · Working Set: ${memFormatted} · Activity: System Protected (${catLabel}).`,
        recommendation:
          "Leave running. System-critical services are excluded from background load reduction.",
        impact: "low",
        confidence: 0.98,
        estimatedImpact: "Protected OS Component",
        activityState: ctx.activityState,
        sustainedSeconds: ctx.sustainedLoadSeconds,
        isPlaceholder: false,
      };
    }

    if (ctx.activityState === "foreground") {
      return {
        summary: `${ctx.name} is actively in focus and serving your current foreground session.`,
        reason: `Foreground CPU: ${ctx.cpuPercent.toFixed(
          1
        )}% (sustained ${ctx.sustainedCpuPercent.toFixed(
          1
        )}%) · Memory: ${memFormatted} · Category: ${catLabel}.`,
        recommendation:
          "No action needed while you are actively using this application.",
        impact: "low",
        confidence: 0.94,
        estimatedImpact: "Active Foreground Workload",
        activityState: "foreground",
        sustainedSeconds: ctx.sustainedLoadSeconds,
        isPlaceholder: false,
      };
    }

    if (ctx.impactLevel === "high" || ctx.status === "attention") {
      return {
        summary: `${ctx.name} is consuming significant resources in the background while not in active focus.`,
        reason: `Sustained ${ctx.sustainedCpuPercent.toFixed(
          1
        )}% CPU and ${memFormatted} RAM in the background for ${Math.max(
          3,
          ctx.sustainedLoadSeconds
        )}s (${catLabel}).`,
        recommendation: ctx.isStartup
          ? `Close ${ctx.name} if you are not using it, or disable its startup launch to reduce background strain.`
          : `Consider closing ${ctx.name} when not in use to free approximately ${memFormatted} RAM and ${ctx.sustainedCpuPercent.toFixed(
              1
            )}% CPU.`,
        impact: "high",
        confidence: 0.91,
        estimatedImpact: `~${memFormatted} RAM / ${ctx.sustainedCpuPercent.toFixed(
          1
        )}% CPU`,
        activityState: ctx.activityState,
        sustainedSeconds: ctx.sustainedLoadSeconds,
        isPlaceholder: false,
      };
    }

    if (ctx.impactLevel === "moderate" || ctx.status === "high-resource") {
      return {
        summary: `${ctx.name} has a moderate background footprint while running out of focus.`,
        reason: `Sustained ${ctx.sustainedCpuPercent.toFixed(
          1
        )}% CPU and ${memFormatted} RAM in state '${ctx.activityState}' (${catLabel}).`,
        recommendation: `Safe to keep running if needed soon, or close to reclaim ${memFormatted} of system memory.`,
        impact: "moderate",
        confidence: 0.88,
        estimatedImpact: `~${memFormatted} RAM`,
        activityState: ctx.activityState,
        sustainedSeconds: ctx.sustainedLoadSeconds,
        isPlaceholder: false,
      };
    }

    return {
      summary: `${ctx.name} is idling quietly in the background with minimal impact on PC responsiveness.`,
      reason: `Sustained ${ctx.sustainedCpuPercent.toFixed(
        1
      )}% CPU · ${memFormatted} RAM · Activity: ${ctx.activityState} (${catLabel}).`,
      recommendation:
        "No action required. This process is not adding noticeable background strain.",
      impact: "low",
      confidence: 0.95,
      estimatedImpact: "Minimal (<1% system load)",
      activityState: ctx.activityState,
      sustainedSeconds: ctx.sustainedLoadSeconds,
      isPlaceholder: false,
    };
  }

  async analyzeSystem(
    metrics: SystemMetrics,
    attentionProcesses: ProcessInfo[]
  ): Promise<ProcessAnalysis> {
    const bgContexts = attentionProcesses
      .slice(0, 5)
      .map((p) => this.buildProcessContext(p));

    if (this.llmAdapter) {
      return this.llmAdapter.explainSystemStrain(metrics, bgContexts);
    }

    await new Promise((resolve) => setTimeout(resolve, 180));

    const bgLoad = metrics.backgroundLoadPercent ?? 0;
    const fgLoad = metrics.foregroundLoadPercent ?? 0;
    const strain =
      metrics.systemStrainPercent ??
      Math.round(
        metrics.cpuUsagePercent * 0.45 + metrics.memoryUsagePercent * 0.4
      );

    if (bgContexts.length > 0) {
      const topOffender = bgContexts[0];
      return {
        summary: `${bgContexts.length} background ${
          bgContexts.length === 1 ? "process is" : "processes are"
        } adding ${bgLoad.toFixed(
          0
        )}% background load while you are focused on ${
          metrics.foregroundProcessName ?? "your desktop"
        }.`,
        reason: `System Strain is at ${strain.toFixed(
          0
        )}% (Foreground Load: ${fgLoad.toFixed(
          0
        )}% · Background Load: ${bgLoad.toFixed(0)}%). Top background contributor: ${
          topOffender.name
        } (${topOffender.sustainedCpuPercent.toFixed(1)}% sustained CPU, ${formatBytes(
          topOffender.memoryBytes
        )} RAM).`,
        recommendation: `Review ${topOffender.name} in the Process Analyzer if you are not actively using it in the background.`,
        impact: bgLoad >= 30 ? "high" : "moderate",
        confidence: 0.92,
        isPlaceholder: false,
      };
    }

    return {
      summary: `Background load is low (${bgLoad.toFixed(
        0
      )}%). Most system resources are dedicated to ${
        metrics.foregroundProcessName ?? "active foreground work"
      }.`,
      reason: `System Strain: ${strain.toFixed(
        0
      )}% · Foreground Load: ${fgLoad.toFixed(
        0
      )}% · Background Load: ${bgLoad.toFixed(
        0
      )}% · CPU: ${metrics.cpuUsagePercent.toFixed(
        0
      )}% · RAM: ${metrics.memoryUsagePercent.toFixed(0)}%.`,
      recommendation:
        "No unnecessary background processes are weighing down your PC right now.",
      impact: "low",
      confidence: 0.95,
      isPlaceholder: false,
    };
  }
}

export const processAnalysisService: ProcessAnalysisService =
  new HybridProcessAnalysisService();

