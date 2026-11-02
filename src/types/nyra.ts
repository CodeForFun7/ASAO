// Mirrors the Nyra agent service (src-python/chatbot/main.py, run_context.py).

export type ActivityStatus = "running" | "waiting" | "done" | "error" | "denied" | "cancelled";

export interface ConfirmationItem {
  label: string;
  detail?: string;
}

export interface ConfirmationCard {
  id: string;
  status: "pending" | "approved" | "denied";
  title: string;
  message: string;
  items: ConfirmationItem[];
  totalItems: number;
  confirmLabel: string;
  danger: "medium" | "high";
}

export interface ActivityNode {
  id: string;
  parentId: string | null;
  kind: "agent" | "tool" | "note";
  name: string;
  label: string;
  status: ActivityStatus;
  args?: Record<string, unknown> | null;
  progress: string[];
  summary?: string;
  error?: string;
  confirmation?: ConfirmationCard;
  startedAt: number;
  endedAt?: number;
  durationMs?: number;
}

export type PlanStatus = "pending" | "in_progress" | "done" | "failed" | "skipped";

export interface PlanStep {
  id: string;
  title: string;
  status: PlanStatus;
  owner?: string;
}

export interface AgentRun {
  status: "running" | "done" | "error" | "cancelled";
  startedAt: number;
  endedAt?: number;
  activities: ActivityNode[];
  plan: PlanStep[];
}

export interface NyraMessage {
  id: string;
  role: "user" | "assistant";
  content: string;
  createdAt: number;
  run?: AgentRun;
  error?: string | null;
}

export interface ConversationSummary {
  id: string;
  title: string;
  createdAt: number;
  updatedAt: number;
  messageCount: number;
  running?: boolean;
}

export interface ConversationRecord extends ConversationSummary {
  messages: NyraMessage[];
}

export interface NyraSettings {
  apiKey: string;
  model: string;
  baseUrl: string;
  /** Save chats to the ASAO account so they appear on the user's other PCs. */
  syncChats: boolean;
}

/** Syncing of chats with the ASAO account (GET /cloud/status). */
export interface CloudStatus {
  enabled: boolean;
  signedIn: boolean;
  syncing: boolean;
  lastSyncAt: number | null;
  error: string | null;
}

export type StreamEvent =
  | {
      type: "run";
      runId: string;
      conversationId: string;
      title: string;
      userMessage: NyraMessage;
      assistantMessageId: string;
      startedAt: number;
    }
  | { type: "token"; text: string }
  | { type: "text_reset" }
  | { type: "activity"; node: ActivityNode }
  | { type: "activity_update"; id: string; patch: Partial<ActivityNode> }
  | { type: "activity_progress"; id: string; message: string }
  | { type: "plan"; items: PlanStep[] }
  | { type: "done"; message: NyraMessage };
