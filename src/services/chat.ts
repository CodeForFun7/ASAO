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

export interface ChatService {
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
 * Placeholder implementation of ChatService.
 * Uses structured system context so that connecting an LLM provider later
 * requires zero UI or store changes.
 */
class PlaceholderChatService implements ChatService {
  async sendMessage(
    userPrompt: string,
    context: StructuredSystemContext
  ): Promise<ChatMessage> {
    await new Promise((resolve) => setTimeout(resolve, 280));

    const topCpu = context.topCpuProcesses[0];
    const topMem = context.topMemoryProcesses[0];

    const topCpuDesc = topCpu
      ? `${topCpu.name} (${topCpu.cpuPercent.toFixed(1)}% CPU)`
      : "none";
    const topMemDesc = topMem
      ? `${topMem.name} (${formatBytes(topMem.memoryBytes)})`
      : "none";

    const contextSummary = `Condition: ${context.systemCondition} · CPU ${context.cpuUsage.toFixed(
      0
    )}% · RAM ${context.ramUsage.toFixed(0)}% · ${
      context.processCount
    } procs`;

    const responseText = `AI analysis will be available once the AI service is connected.\n\nCaptured live context for "${userPrompt}":\n• Condition: ${context.systemCondition} (${context.conditionReason})\n• Top CPU process: ${topCpuDesc}\n• Top Memory process: ${topMemDesc}`;

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
export const chatService: ChatService = new PlaceholderChatService();
