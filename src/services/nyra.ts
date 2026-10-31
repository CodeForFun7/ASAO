// Client for the Nyra agent service (Python, LangGraph) on 127.0.0.1:18743.

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  ConversationRecord,
  ConversationSummary,
  NyraSettings,
  StreamEvent,
} from "../types/nyra";

const BASE = "http://127.0.0.1:18743";
const HEADERS = { "Content-Type": "application/json", "X-Nyra-Client": "asao" };

/** Progress lines from long Rust operations, tagged with the activity id of the tool call. */
export const AGENT_PROGRESS_EVENT = "AGENT_TOOL_PROGRESS";

const SETTINGS_KEYS = {
  apiKey: "nyra_openai_api_key",
  model: "nyra_model",
  baseUrl: "nyra_base_url",
} as const;

export const DEFAULT_MODEL = "gpt-4o-mini";

export function loadNyraSettings(): NyraSettings {
  try {
    return {
      apiKey: localStorage.getItem(SETTINGS_KEYS.apiKey) ?? "",
      model: localStorage.getItem(SETTINGS_KEYS.model) || DEFAULT_MODEL,
      baseUrl: localStorage.getItem(SETTINGS_KEYS.baseUrl) ?? "",
    };
  } catch {
    return { apiKey: "", model: DEFAULT_MODEL, baseUrl: "" };
  }
}

export function saveNyraSettings(settings: NyraSettings): void {
  const set = (key: string, value: string) =>
    value ? localStorage.setItem(key, value) : localStorage.removeItem(key);
  set(SETTINGS_KEYS.apiKey, settings.apiKey.trim());
  set(SETTINGS_KEYS.model, settings.model.trim() === DEFAULT_MODEL ? "" : settings.model.trim());
  set(SETTINGS_KEYS.baseUrl, settings.baseUrl.trim());
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`${BASE}${path}`, { ...init, headers: { ...HEADERS, ...(init?.headers ?? {}) } });
  } catch {
    throw new Error("The Nyra agent service is not running. Start it with `pnpm tauri dev` (it runs on port 18743).");
  }
  const payload = await response.json().catch(() => ({}));
  if (!response.ok) {
    const detail = (payload as { detail?: unknown }).detail;
    throw new Error(typeof detail === "string" ? detail : `Nyra request failed (${response.status}).`);
  }
  return payload as T;
}

export function getNyraHealth(): Promise<{ ok: boolean; bridge: boolean }> {
  return request("/health");
}

export function listConversations(): Promise<ConversationSummary[]> {
  return request("/conversations");
}

export function getConversation(id: string): Promise<ConversationRecord> {
  return request(`/conversations/${encodeURIComponent(id)}`);
}

export function renameConversation(id: string, title: string): Promise<void> {
  return request(`/conversations/${encodeURIComponent(id)}`, { method: "PATCH", body: JSON.stringify({ title }) });
}

export function deleteConversation(id: string): Promise<void> {
  return request(`/conversations/${encodeURIComponent(id)}`, { method: "DELETE" });
}

export function answerConfirmation(runId: string, confirmationId: string, approved: boolean): Promise<void> {
  return request(`/chat/${encodeURIComponent(runId)}/confirm`, {
    method: "POST",
    body: JSON.stringify({ confirmation_id: confirmationId, approved }),
  });
}

export function stopRun(runId: string): Promise<void> {
  return request(`/chat/${encodeURIComponent(runId)}/stop`, { method: "POST" });
}

/** Starts one turn and calls `onEvent` for every live event until the turn ends. */
export async function streamChat(
  settings: NyraSettings,
  conversationId: string | null,
  message: string,
  onEvent: (event: StreamEvent) => void,
  signal: AbortSignal
): Promise<void> {
  let response: Response;
  try {
    response = await fetch(`${BASE}/chat/stream`, {
      method: "POST",
      headers: HEADERS,
      body: JSON.stringify({
        api_key: settings.apiKey,
        model: settings.model || DEFAULT_MODEL,
        base_url: settings.baseUrl || null,
        conversation_id: conversationId,
        message,
      }),
      signal,
    });
  } catch (err) {
    if (signal.aborted) return;
    throw new Error("The Nyra agent service is not running. Start it with `pnpm tauri dev` (it runs on port 18743).");
  }
  if (!response.ok || !response.body) {
    const payload = await response.json().catch(() => ({}));
    const detail = (payload as { detail?: unknown }).detail;
    throw new Error(
      typeof detail === "string"
        ? detail
        : Array.isArray(detail)
          ? "Nyra rejected the request. Check the settings."
          : `Nyra request failed (${response.status}).`
    );
  }

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  for (;;) {
    const { value, done } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });
    let boundary: number;
    while ((boundary = buffer.indexOf("\n\n")) >= 0) {
      const chunk = buffer.slice(0, boundary);
      buffer = buffer.slice(boundary + 2);
      for (const line of chunk.split("\n")) {
        if (!line.startsWith("data:")) continue;
        try {
          onEvent(JSON.parse(line.slice(5).trim()) as StreamEvent);
        } catch {
          // ignore a malformed line
        }
      }
    }
  }
}

export async function subscribeToAgentProgress(
  onProgress: (operationId: string, message: string) => void
): Promise<UnlistenFn> {
  try {
    return await listen<{ operationId: string; message: string }>(AGENT_PROGRESS_EVENT, (e) =>
      onProgress(e.payload.operationId, e.payload.message)
    );
  } catch {
    return () => {};
  }
}
