import type {
  ProcessAnalysis,
  ProcessAnalysisService,
  ProcessInfo,
  SystemMetrics,
} from "../types/process";
import { formatBytes } from "./tauri";

/**
 * Placeholder implementation of ProcessAnalysisService.
 * Designed so that `OpenAIProcessAnalysisService` or a local LLM bridge
 * can replace this class later without modifying any React components.
 */
export class MockProcessAnalysisService implements ProcessAnalysisService {
  async analyzeProcess(process: ProcessInfo): Promise<ProcessAnalysis> {
    // Simulate async service boundary
    await new Promise((resolve) => setTimeout(resolve, 250));

    return {
      summary: "AI analysis will be available once the LLM service is connected.",
      reason: `Telemetry context prepared for ${process.name} (PID ${process.pid}): ${process.cpuPercent.toFixed(1)}% CPU, ${formatBytes(process.memoryBytes)} RAM, category '${process.category}', status '${process.status}'.`,
      recommendation: process.isSystemCritical
        ? "Protected Windows system process. Modification is restricted by safety policy."
        : "Awaiting LLM connection to evaluate background optimization candidates.",
      risk: undefined,
      confidence: undefined,
      estimatedImpact: undefined,
      isPlaceholder: true,
    };
  }

  async analyzeSystem(
    metrics: SystemMetrics,
    attentionProcesses: ProcessInfo[]
  ): Promise<ProcessAnalysis> {
    await new Promise((resolve) => setTimeout(resolve, 250));

    return {
      summary:
        "AI analysis will appear here once the LLM layer is connected.",
      reason: `Snapshot ready for inference: ${metrics.totalProcesses} active processes (${attentionProcesses.length} flagged for attention), CPU at ${metrics.cpuUsagePercent.toFixed(0)}%, Memory at ${metrics.memoryUsagePercent.toFixed(0)}%.`,
      recommendation:
        "Connect an LLM provider in the future optimization phase to generate personalized system recommendations.",
      isPlaceholder: true,
    };
  }
}

export const processAnalysisService: ProcessAnalysisService =
  new MockProcessAnalysisService();
