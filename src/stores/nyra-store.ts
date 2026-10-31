import { create } from "zustand";
import {
  answerConfirmation,
  deleteConversation,
  getConversation,
  getNyraHealth,
  listConversations,
  loadNyraSettings,
  renameConversation,
  saveNyraSettings,
  stopRun,
  streamChat,
  subscribeToAgentProgress,
} from "../services/nyra";
import type {
  ActivityNode,
  ConversationSummary,
  NyraMessage,
  NyraSettings,
  StreamEvent,
} from "../types/nyra";

const MAX_PROGRESS_LINES = 40;

interface LiveRun {
  runId: string | null;
  conversationId: string | null;
  controller: AbortController;
}

interface NyraState {
  settings: NyraSettings;
  conversations: ConversationSummary[];
  activeId: string | null;
  messages: NyraMessage[];
  loadingConversation: boolean;
  /** The assistant message being produced right now. */
  liveMessage: NyraMessage | null;
  live: LiveRun | null;
  service: "unknown" | "online" | "offline";
  bridge: boolean;
  error: string;

  init(): Promise<void>;
  checkHealth(): Promise<void>;
  refreshConversations(): Promise<void>;
  openConversation(id: string): Promise<void>;
  newChat(): void;
  send(text: string): Promise<void>;
  stop(): Promise<void>;
  confirm(activityId: string, confirmationId: string, approved: boolean): Promise<void>;
  removeConversation(id: string): Promise<void>;
  renameConversation(id: string, title: string): Promise<void>;
  updateSettings(settings: NyraSettings): void;
  clearError(): void;
}

let initialised = false;

function mapActivities(message: NyraMessage, fn: (nodes: ActivityNode[]) => ActivityNode[]): NyraMessage {
  const run = message.run;
  if (!run) return message;
  return { ...message, run: { ...run, activities: fn(run.activities) } };
}

function applyEvent(message: NyraMessage, event: StreamEvent): NyraMessage {
  switch (event.type) {
    case "token":
      return { ...message, content: message.content + event.text };
    case "text_reset":
      return { ...message, content: "" };
    case "activity":
      return mapActivities(message, (nodes) => [...nodes, { ...event.node, progress: event.node.progress ?? [] }]);
    case "activity_update":
      return mapActivities(message, (nodes) =>
        nodes.map((n) => (n.id === event.id ? { ...n, ...event.patch } : n))
      );
    case "activity_progress":
      return mapActivities(message, (nodes) =>
        nodes.map((n) =>
          n.id === event.id ? { ...n, progress: [...n.progress, event.message].slice(-MAX_PROGRESS_LINES) } : n
        )
      );
    case "plan":
      return message.run ? { ...message, run: { ...message.run, plan: event.items } } : message;
    default:
      return message;
  }
}

function errorText(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

export const useNyraStore = create<NyraState>((set, get) => ({
  settings: loadNyraSettings(),
  conversations: [],
  activeId: null,
  messages: [],
  loadingConversation: false,
  liveMessage: null,
  live: null,
  service: "unknown",
  bridge: false,
  error: "",

  async init() {
    if (initialised) {
      void get().checkHealth();
      return;
    }
    initialised = true;
    // Rust progress lines for tool calls of the running turn.
    void subscribeToAgentProgress((operationId, message) => {
      const live = get().liveMessage;
      if (!live?.run?.activities.some((n) => n.id === operationId)) return;
      set({ liveMessage: applyEvent(live, { type: "activity_progress", id: operationId, message }) });
    });
    await get().checkHealth();
    await get().refreshConversations();
  },

  async checkHealth() {
    try {
      const h = await getNyraHealth();
      set({ service: "online", bridge: h.bridge });
    } catch {
      set({ service: "offline", bridge: false });
    }
  },

  async refreshConversations() {
    try {
      set({ conversations: await listConversations(), service: "online" });
    } catch {
      set({ service: "offline" });
    }
  },

  async openConversation(id) {
    if (get().activeId === id) return;
    set({ activeId: id, messages: [], loadingConversation: true, error: "" });
    try {
      const record = await getConversation(id);
      if (get().activeId !== id) return;
      const liveId = get().liveMessage?.id;
      set({ messages: record.messages.filter((m) => m.id !== liveId), loadingConversation: false });
    } catch (err) {
      if (get().activeId === id) set({ loadingConversation: false, error: errorText(err) });
    }
  },

  newChat() {
    set({ activeId: null, messages: [], error: "" });
  },

  async send(text) {
    const content = text.trim();
    const state = get();
    if (!content || state.live) return;
    if (!state.settings.apiKey) {
      set({ error: "Add an API key in Nyra settings first." });
      return;
    }
    const controller = new AbortController();
    const conversationId = state.activeId;
    const optimistic: NyraMessage = { id: `pending-${Date.now()}`, role: "user", content, createdAt: Date.now() };
    set({
      messages: [...state.messages, optimistic],
      live: { runId: null, conversationId, controller },
      liveMessage: null,
      error: "",
    });

    const onEvent = (event: StreamEvent) => {
      if (event.type === "run") {
        const assistant: NyraMessage = {
          id: event.assistantMessageId,
          role: "assistant",
          content: "",
          createdAt: event.startedAt,
          run: { status: "running", startedAt: event.startedAt, activities: [], plan: [] },
        };
        const isNew = !get().conversations.some((c) => c.id === event.conversationId);
        set((s) => ({
          live: s.live ? { ...s.live, runId: event.runId, conversationId: event.conversationId } : s.live,
          liveMessage: assistant,
          activeId: s.activeId === conversationId ? event.conversationId : s.activeId,
          messages:
            s.activeId === conversationId
              ? s.messages.map((m) => (m.id === optimistic.id ? event.userMessage : m))
              : s.messages,
          conversations: isNew
            ? [
                {
                  id: event.conversationId,
                  title: event.title,
                  createdAt: event.startedAt,
                  updatedAt: event.startedAt,
                  messageCount: 1,
                  running: true,
                },
                ...s.conversations,
              ]
            : s.conversations.map((c) => (c.id === event.conversationId ? { ...c, running: true, updatedAt: event.startedAt } : c)),
        }));
        return;
      }
      if (event.type === "done") {
        set((s) => ({
          liveMessage: null,
          live: null,
          messages: s.activeId === s.live?.conversationId ? [...s.messages, event.message] : s.messages,
        }));
        void get().refreshConversations();
        return;
      }
      const live = get().liveMessage;
      if (live) set({ liveMessage: applyEvent(live, event) });
    };

    try {
      await streamChat(state.settings, conversationId, content, onEvent, controller.signal);
    } catch (err) {
      set((s) => ({
        error: errorText(err),
        messages: s.messages.filter((m) => m.id !== optimistic.id || s.live?.runId),
      }));
    } finally {
      // The stream ended without a "done" event (service stopped or window stopped listening).
      const s = get();
      if (s.live?.controller === controller) {
        const leftover = s.liveMessage;
        set({
          live: null,
          liveMessage: null,
          messages:
            leftover && s.activeId === s.live.conversationId
              ? [...s.messages, { ...leftover, run: leftover.run && { ...leftover.run, status: "error" } }]
              : s.messages,
        });
        void get().refreshConversations();
      }
    }
  },

  async stop() {
    const live = get().live;
    if (!live) return;
    if (live.runId) {
      try {
        await stopRun(live.runId);
        return; // the stream ends with a "done" event
      } catch {
        // fall through and drop the stream
      }
    }
    live.controller.abort();
  },

  async confirm(activityId, confirmationId, approved) {
    const { live, liveMessage } = get();
    if (!live?.runId || !liveMessage) return;
    // Show the answer immediately; the service confirms it with an update event.
    set({
      liveMessage: mapActivities(liveMessage, (nodes) =>
        nodes.map((n) =>
          n.id === activityId && n.confirmation
            ? { ...n, confirmation: { ...n.confirmation, status: approved ? "approved" : "denied" } }
            : n
        )
      ),
    });
    try {
      await answerConfirmation(live.runId, confirmationId, approved);
    } catch (err) {
      set({ error: errorText(err) });
    }
  },

  async removeConversation(id) {
    try {
      await deleteConversation(id);
      set((s) => ({
        conversations: s.conversations.filter((c) => c.id !== id),
        ...(s.activeId === id ? { activeId: null, messages: [] } : {}),
      }));
    } catch (err) {
      set({ error: errorText(err) });
    }
  },

  async renameConversation(id, title) {
    const clean = title.trim();
    if (!clean) return;
    try {
      await renameConversation(id, clean);
      set((s) => ({ conversations: s.conversations.map((c) => (c.id === id ? { ...c, title: clean } : c)) }));
    } catch (err) {
      set({ error: errorText(err) });
    }
  },

  updateSettings(settings) {
    saveNyraSettings(settings);
    set({ settings: loadNyraSettings(), error: "" });
  },

  clearError() {
    set({ error: "" });
  },
}));
