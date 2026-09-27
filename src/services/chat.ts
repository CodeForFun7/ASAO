import type {
  ChatMessage,
  StructuredSystemContext,
  WidgetHistorySample,
  WidgetSystemUpdate,
} from "../types/widget";
import { formatBytes } from "./tauri";

export interface SystemContextService {
  buildContext(
    latestUpdate: WidgetSystemUpdate,
    history: WidgetHistorySample[]
  ): StructuredSystemContext;
}

export interface LlmChatProvider {
  generateReply(
    userPrompt: string,
    context: StructuredSystemContext
  ): Promise<string>;
}

export interface ChatService {
  registerLlmProvider?(provider: LlmChatProvider): void;
  sendMessage(
    userPrompt: string,
    context: StructuredSystemContext
  ): Promise<ChatMessage>;
}

class DefaultSystemContextService implements SystemContextService {
  buildContext(
    latestUpdate: WidgetSystemUpdate,
    history: WidgetHistorySample[]
  ): StructuredSystemContext {
    return {
      cpuUsage: latestUpdate.cpuUsage,
      ramUsage: latestUpdate.memoryUsage,
      memoryUsedBytes: latestUpdate.memoryUsedBytes,
      memoryTotalBytes: latestUpdate.memoryTotalBytes,
      gpuUsage: latestUpdate.gpuUsage,
      foregroundLoad: latestUpdate.foregroundLoad ?? 0,
      backgroundLoad: latestUpdate.backgroundLoad ?? 0,
      systemStrain:
        latestUpdate.systemStrain ??
        Math.round(latestUpdate.cpuUsage * 0.45 + latestUpdate.memoryUsage * 0.4),
      userActive: latestUpdate.userActive ?? true,
      foregroundProcessName: latestUpdate.foregroundProcessName ?? null,
      processCount: latestUpdate.processCount,
      attentionCount: latestUpdate.attentionCount,
      systemCondition: latestUpdate.condition,
      conditionReason: latestUpdate.conditionReason,
      topCpuProcesses: latestUpdate.topCpuProcesses,
      topMemoryProcesses: latestUpdate.topMemoryProcesses,
      recentResourceHistory: history.slice(-15),
      activeRecommendation: latestUpdate.recommendation,
    };
  }
}

/**
 * Layered Explanation & Chat Service.
 *
 * Receives pre-analyzed `StructuredSystemContext` from the Background Process Analyzer
 * and Recommendation Engine. Works fully without an LLM using deterministic explanation
 * synthesis, and seamlessly upgrades to an `LlmChatProvider` when connected.
 */
class HybridChatService implements ChatService {
  private llmProvider: LlmChatProvider | null = null;

  registerLlmProvider(provider: LlmChatProvider): void {
    this.llmProvider = provider;
  }

  async sendMessage(
    userPrompt: string,
    context: StructuredSystemContext
  ): Promise<ChatMessage> {
    const contextSummary = `Strain ${context.systemStrain.toFixed(
      0
    )}% · BG Load ${context.backgroundLoad.toFixed(
      0
    )}% · CPU ${context.cpuUsage.toFixed(0)}% · RAM ${context.ramUsage.toFixed(
      0
    )}%`;

    if (this.llmProvider) {
      const llmReply = await this.llmProvider.generateReply(userPrompt, context);
      return {
        id: `asao-${Date.now()}`,
        sender: "asao",
        text: llmReply,
        contextSummary,
        timestamp: Date.now(),
      };
    }

    await new Promise((resolve) => setTimeout(resolve, 220));

    const bgProcesses = context.topCpuProcesses.filter(
      (p) => p.activityState === "background"
    );
    const heaviestBg =
      bgProcesses[0] ?? context.topCpuProcesses[0] ?? context.topMemoryProcesses[0];
    const topMem = context.topMemoryProcesses[0];

    const lowerPrompt = userPrompt.toLowerCase();
    let responseText: string;

    if (
      lowerPrompt.includes("background") ||
      lowerPrompt.includes("heavy") ||
      lowerPrompt.includes("slow") ||
      lowerPrompt.includes("close")
    ) {
      if (heaviestBg && context.backgroundLoad >= 10) {
        responseText = `Your background load is currently ${context.backgroundLoad.toFixed(
          0
        )}% (System Strain: ${context.systemStrain.toFixed(0)}%).\n\n• Top background contributor: ${
          heaviestBg.name
        } using ${(heaviestBg.sustainedCpuPercent ?? heaviestBg.cpuPercent).toFixed(
          1
        )}% sustained CPU and ${formatBytes(
          heaviestBg.memoryBytes
        )} RAM while not in focus.\n• Recommendation: ${
          context.activeRecommendation?.message ??
          `You can close ${heaviestBg.name} if you are not actively using it.`
        }`;
      } else {
        responseText = `Background processes are currently quiet (${context.backgroundLoad.toFixed(
          0
        )}% background load, ${context.systemStrain.toFixed(
          0
        )}% overall strain). Most active resources are serving ${
          context.foregroundProcessName ?? "your foreground session"
        }.`;
      }
    } else if (lowerPrompt.includes("ram") || lowerPrompt.includes("memory")) {
      responseText = `Memory usage is at ${context.ramUsage.toFixed(0)}% (${formatBytes(
        context.memoryUsedBytes
      )} / ${formatBytes(context.memoryTotalBytes)}).\n\n• Largest memory holder: ${
        topMem ? `${topMem.name} (${formatBytes(topMem.memoryBytes)})` : "None"
      }\n• Background load share: ${context.backgroundLoad.toFixed(0)}%`;
    } else {
      responseText = `System Condition: ${context.systemCondition} (${
        context.conditionReason
      })\n\n• System Strain: ${context.systemStrain.toFixed(
        0
      )}% (Foreground: ${context.foregroundLoad.toFixed(
        0
      )}% · Background: ${context.backgroundLoad.toFixed(0)}%)\n• Active Focus: ${
        context.foregroundProcessName ?? "Desktop"
      }\n• Guidance: ${
        context.activeRecommendation?.message ??
        "All background processes are within nominal limits."
      }`;
    }

    return {
      id: `asao-${Date.now()}`,
      sender: "asao",
      text: responseText,
      contextSummary,
      timestamp: Date.now(),
    };
  }
}

export const systemContextService: SystemContextService =
  new DefaultSystemContextService();
export const chatService: ChatService = new HybridChatService();

