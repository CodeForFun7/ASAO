import { create } from "zustand";
import type { ChatMessage, DiagnosticStep, VertexAgentConfig } from "../types/agent";
import { vertexAdkAgentService } from "../services/ai-agent/agent-service";
import { disableStartupItem, enableStartupItem } from "../services/startup";
import { openStorageLocation } from "../services/storage";
import { useProcessStore } from "./process-store";
import { useStartupStore } from "./startup-store";

interface AgentStoreState {
  messages: ChatMessage[];
  isProcessing: boolean;
  activeSteps: DiagnosticStep[];
  config: VertexAgentConfig;

  // Actions
  sendMessage: (prompt: string) => Promise<void>;
  applyFix: (fixId: string, messageId: string) => Promise<void>;
  clearChat: () => void;
  updateConfig: (patch: Partial<VertexAgentConfig>) => void;
}

const INITIAL_MESSAGES: ChatMessage[] = [
  {
    id: "welcome-1",
    sender: "assistant",
    text: `Hello! I'm your **ASAO System Diagnostics Assistant**, powered by **Gemini 3.5 Flash-Lite** on Vertex AI.

I analyze your real-time system telemetry, background processes, startup boot trace, and storage drives to identify bottlenecks and provide one-click optimizations.

What would you like to inspect today?`,
    timestamp: Date.now(),
  },
];

export const useAgentStore = create<AgentStoreState>((set, get) => ({
  messages: INITIAL_MESSAGES,
  isProcessing: false,
  activeSteps: [],
  config: vertexAdkAgentService.getConfig(),

  sendMessage: async (prompt: string) => {
    const trimmed = prompt.trim();
    if (!trimmed || get().isProcessing) return;

    const userMessage: ChatMessage = {
      id: `user-${Date.now()}`,
      sender: "user",
      text: trimmed,
      timestamp: Date.now(),
    };

    const assistantMsgId = `asao-${Date.now()}`;
    const initialAssistantMessage: ChatMessage = {
      id: assistantMsgId,
      sender: "assistant",
      text: "",
      timestamp: Date.now(),
      steps: [],
      fixes: [],
      isStreaming: true,
    };

    set((state) => ({
      messages: [...state.messages, userMessage, initialAssistantMessage],
      isProcessing: true,
      activeSteps: [],
    }));

    try {
      const history = get().messages.slice(0, -1); // exclude current placeholder

      const result = await vertexAdkAgentService.processQuery(
        trimmed,
        history,
        {
          onStepAdded: (step) => {
            set((state) => {
              const updatedSteps = [...state.activeSteps, step];
              const updatedMessages = state.messages.map((m) =>
                m.id === assistantMsgId
                  ? { ...m, steps: updatedSteps }
                  : m
              );
              return { activeSteps: updatedSteps, messages: updatedMessages };
            });
          },
          onStepUpdated: (step) => {
            set((state) => {
              const updatedSteps = state.activeSteps.map((s) =>
                s.id === step.id ? { ...step } : s
              );
              const updatedMessages = state.messages.map((m) =>
                m.id === assistantMsgId
                  ? { ...m, steps: updatedSteps }
                  : m
              );
              return { activeSteps: updatedSteps, messages: updatedMessages };
            });
          },
          onFixesIdentified: (fixes) => {
            set((state) => ({
              messages: state.messages.map((m) =>
                m.id === assistantMsgId ? { ...m, fixes } : m
              ),
            }));
          },
        }
      );

      // Finalize the assistant message
      set((state) => ({
        isProcessing: false,
        activeSteps: [],
        messages: state.messages.map((m) =>
          m.id === assistantMsgId
            ? {
                ...m,
                text: result.text,
                steps: result.steps,
                fixes: result.fixes,
                diagnosticData: result.diagnosticData,
                isStreaming: false,
              }
            : m
        ),
      }));
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : "Failed to process query.";
      set((state) => ({
        isProcessing: false,
        activeSteps: [],
        messages: state.messages.map((m) =>
          m.id === assistantMsgId
            ? {
                ...m,
                text: "I encountered an error while analyzing your system. Please verify your connection or settings.",
                error: errorMsg,
                isStreaming: false,
              }
            : m
        ),
      }));
    }
  },

  applyFix: async (fixId: string, messageId: string) => {
    const { messages } = get();
    const targetMsg = messages.find((m) => m.id === messageId);
    if (!targetMsg || !targetMsg.fixes) return;

    const targetFix = targetMsg.fixes.find((f) => f.id === fixId);
    if (!targetFix || targetFix.status === "applying" || targetFix.status === "applied") return;

    // Set status to applying
    set((state) => ({
      messages: state.messages.map((m) =>
        m.id === messageId
          ? {
              ...m,
              fixes: m.fixes?.map((f) =>
                f.id === fixId ? { ...f, status: "applying" as const } : f
              ),
            }
          : m
      ),
    }));

    try {
      if (targetFix.actionType === "disable_startup") {
        const startupId = targetFix.payload.startupId;
        const source = targetFix.payload.source || "registry-run-user";

        if (startupId) {
          await disableStartupItem(
            startupId,
            source,
            targetFix.payload.registryKey,
            targetFix.payload.serviceName,
            targetFix.payload.taskPath
          );

          // Update startup store optimistically
          useStartupStore.setState((s) => ({
            items: s.items.map((i) =>
              i.id === startupId ? { ...i, isEnabled: false } : i
            ),
          }));

          // Trigger a background reload of startup data
          void useStartupStore.getState().loadStartupData();
        }

        set((state) => ({
          messages: state.messages.map((m) =>
            m.id === messageId
              ? {
                  ...m,
                  fixes: m.fixes?.map((f) =>
                    f.id === fixId
                      ? {
                          ...f,
                          status: "applied" as const,
                          feedbackMessage: "Disabled successfully",
                        }
                      : f
                  ),
                }
              : m
          ),
        }));
      } else if (targetFix.actionType === "enable_startup") {
        const startupId = targetFix.payload.startupId;
        const source = targetFix.payload.source || "registry-run-user";

        if (startupId) {
          await enableStartupItem(
            startupId,
            source,
            targetFix.payload.registryKey,
            targetFix.payload.serviceName,
            targetFix.payload.taskPath,
            targetFix.payload.commandLine
          );

          useStartupStore.setState((s) => ({
            items: s.items.map((i) =>
              i.id === startupId ? { ...i, isEnabled: true } : i
            ),
          }));
          void useStartupStore.getState().loadStartupData();
        }

        set((state) => ({
          messages: state.messages.map((m) =>
            m.id === messageId
              ? {
                  ...m,
                  fixes: m.fixes?.map((f) =>
                    f.id === fixId
                      ? {
                          ...f,
                          status: "applied" as const,
                          feedbackMessage: "Enabled successfully",
                        }
                      : f
                  ),
                }
              : m
          ),
        }));
      } else if (targetFix.actionType === "inspect_process") {
        if (targetFix.payload.pid !== undefined) {
          useProcessStore.getState().selectProcess(targetFix.payload.pid);
          useProcessStore.getState().setRoute("processes");
        }
      } else if (targetFix.actionType === "open_storage") {
        if (targetFix.payload.path) {
          await openStorageLocation(targetFix.payload.path);
          set((state) => ({
            messages: state.messages.map((m) =>
              m.id === messageId
                ? {
                    ...m,
                    fixes: m.fixes?.map((f) =>
                      f.id === fixId
                        ? {
                            ...f,
                            status: "applied" as const,
                            feedbackMessage: "Opened in File Explorer",
                          }
                        : f
                    ),
                  }
                : m
            ),
          }));
        }
      } else if (targetFix.actionType === "navigate_route") {
        if (targetFix.payload.route) {
          useProcessStore.getState().setRoute(targetFix.payload.route);
        }
      }
    } catch (err) {
      set((state) => ({
        messages: state.messages.map((m) =>
          m.id === messageId
            ? {
                ...m,
                fixes: m.fixes?.map((f) =>
                  f.id === fixId
                    ? {
                        ...f,
                        status: "failed" as const,
                        feedbackMessage: err instanceof Error ? err.message : "Failed to apply fix",
                      }
                    : f
                ),
              }
            : m
        ),
      }));
    }
  },

  clearChat: () => set({ messages: INITIAL_MESSAGES, activeSteps: [], isProcessing: false }),

  updateConfig: (patch: Partial<VertexAgentConfig>) => {
    vertexAdkAgentService.updateConfig(patch);
    set({ config: vertexAdkAgentService.getConfig() });
  },
}));
