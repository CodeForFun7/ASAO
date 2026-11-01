import { db } from "./db";
import { AuthError } from "./accounts";

/*
 * Nyra chats saved to a user's account, so they follow the user to other PCs.
 *
 * ASAO keeps its own copy and pushes new messages here; other PCs pull them. A deleted
 * chat keeps an empty row with deleted_at set, so other PCs learn to delete their copy.
 * Payloads are the chat messages exactly as Nyra stores them (JSON text).
 */

export const MAX_PAYLOAD_BYTES = 256 * 1024;
const MAX_USER_BYTES = 50 * 1024 * 1024;
const MAX_MESSAGES_PER_SYNC = 500;
const ID_PATTERN = /^[A-Za-z0-9_-]{1,64}$/;

export interface ChatSummary {
  id: string;
  title: string;
  createdAt: number;
  updatedAt: number;
  deleted: boolean;
}

export interface ChatMessage {
  id: string;
  seq: number;
  role: "user" | "assistant";
  createdAt: number;
  payload: string;
}

const badRequest = (msg: string) => new AuthError(400, msg);

function checkId(value: unknown, what: string): string {
  if (typeof value !== "string" || !ID_PATTERN.test(value)) throw badRequest(`Invalid ${what} id.`);
  return value;
}

function checkTime(value: unknown, what: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) throw badRequest(`Invalid ${what}.`);
  return value;
}

export async function listChats(userId: string): Promise<ChatSummary[]> {
  const pool = await db();
  const r = await pool.query<{ id: string; title: string; created_at: string; updated_at: string; deleted: boolean }>(
    `SELECT id, title, created_at, updated_at, deleted_at IS NOT NULL AS deleted
     FROM asao_chats WHERE user_id = $1 ORDER BY updated_at DESC LIMIT 2000`,
    [userId],
  );
  return r.rows.map((c) => ({
    id: c.id,
    title: c.title,
    createdAt: Number(c.created_at),
    updatedAt: Number(c.updated_at),
    deleted: c.deleted,
  }));
}

export async function getChat(userId: string, rawId: unknown): Promise<{ chat: ChatSummary; messages: ChatMessage[] }> {
  const id = checkId(rawId, "chat");
  const pool = await db();
  const c = await pool.query<{ title: string; created_at: string; updated_at: string; deleted: boolean }>(
    "SELECT title, created_at, updated_at, deleted_at IS NOT NULL AS deleted FROM asao_chats WHERE user_id = $1 AND id = $2",
    [userId, id],
  );
  const row = c.rows[0];
  if (!row) throw new AuthError(404, "Chat not found.");
  const m = await pool.query<{ id: string; seq: number; role: "user" | "assistant"; created_at: string; payload: string }>(
    "SELECT id, seq, role, created_at, payload FROM asao_chat_messages WHERE user_id = $1 AND chat_id = $2 ORDER BY seq",
    [userId, id],
  );
  return {
    chat: { id, title: row.title, createdAt: Number(row.created_at), updatedAt: Number(row.updated_at), deleted: row.deleted },
    messages: m.rows.map((r) => ({ id: r.id, seq: r.seq, role: r.role, createdAt: Number(r.created_at), payload: r.payload })),
  };
}

/**
 * Creates or updates a chat and adds or replaces the given messages (by id).
 * Answers 410 when the chat was deleted on another PC, so the caller deletes its copy.
 */
export async function syncChat(userId: string, body: Record<string, unknown>): Promise<{ ok: true }> {
  const chat = (body.chat ?? {}) as Record<string, unknown>;
  const id = checkId(chat.id, "chat");
  const title = typeof chat.title === "string" && chat.title.trim() ? chat.title.trim().slice(0, 120) : "New chat";
  const createdAt = checkTime(chat.createdAt, "creation time");
  const updatedAt = checkTime(chat.updatedAt, "update time");

  const rawMessages = Array.isArray(body.messages) ? body.messages : [];
  if (rawMessages.length > MAX_MESSAGES_PER_SYNC) throw badRequest("Too many messages in one request.");
  let incomingBytes = 0;
  const messages = rawMessages.map((raw) => {
    const m = (raw ?? {}) as Record<string, unknown>;
    if (m.role !== "user" && m.role !== "assistant") throw badRequest("Invalid message role.");
    if (typeof m.payload !== "string") throw badRequest("Invalid message.");
    const bytes = Buffer.byteLength(m.payload);
    if (bytes > MAX_PAYLOAD_BYTES) throw new AuthError(413, "A message is too large to save online.");
    incomingBytes += bytes;
    const seq = m.seq;
    if (typeof seq !== "number" || !Number.isInteger(seq) || seq < 0 || seq > 1_000_000) throw badRequest("Invalid message order.");
    return { id: checkId(m.id, "message"), seq, role: m.role, createdAt: checkTime(m.createdAt, "message time"), payload: m.payload };
  });

  const pool = await db();
  const existing = await pool.query<{ deleted: boolean }>(
    "SELECT deleted_at IS NOT NULL AS deleted FROM asao_chats WHERE user_id = $1 AND id = $2",
    [userId, id],
  );
  if (existing.rows[0]?.deleted) throw new AuthError(410, "This chat was deleted.");

  if (incomingBytes > 0) {
    const used = await pool.query<{ n: string | null }>(
      "SELECT sum(octet_length(payload)) AS n FROM asao_chat_messages WHERE user_id = $1",
      [userId],
    );
    if (Number(used.rows[0]?.n ?? 0) + incomingBytes > MAX_USER_BYTES)
      throw new AuthError(507, "Your online chat storage is full. Delete some chats to save new ones.");
  }

  const client = await pool.connect();
  try {
    await client.query("BEGIN");
    await client.query(
      `INSERT INTO asao_chats (user_id, id, title, created_at, updated_at) VALUES ($1, $2, $3, $4, $5)
       ON CONFLICT (user_id, id) DO UPDATE
         SET title = EXCLUDED.title, updated_at = GREATEST(asao_chats.updated_at, EXCLUDED.updated_at)`,
      [userId, id, title, createdAt, updatedAt],
    );
    for (const m of messages) {
      await client.query(
        `INSERT INTO asao_chat_messages (user_id, chat_id, id, seq, role, created_at, payload)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (user_id, chat_id, id) DO UPDATE SET payload = EXCLUDED.payload, seq = EXCLUDED.seq`,
        [userId, id, m.id, m.seq, m.role, m.createdAt, m.payload],
      );
    }
    await client.query("COMMIT");
  } catch (e) {
    await client.query("ROLLBACK").catch(() => undefined);
    throw e;
  } finally {
    client.release();
  }
  return { ok: true };
}

/** Deletes a chat's messages and leaves a marker so other PCs delete their copy. */
export async function deleteChat(userId: string, rawId: unknown): Promise<void> {
  const id = checkId(rawId, "chat");
  const pool = await db();
  await pool.query("DELETE FROM asao_chat_messages WHERE user_id = $1 AND chat_id = $2", [userId, id]);
  const now = Date.now();
  await pool.query(
    `INSERT INTO asao_chats (user_id, id, title, created_at, updated_at, deleted_at) VALUES ($1, $2, '', $3, $3, now())
     ON CONFLICT (user_id, id) DO UPDATE SET title = '', updated_at = $3, deleted_at = now()`,
    [userId, id, now],
  );
}

/**
 * Removes everything the user saved online. Copies on their PCs stay; a PC that still has
 * syncing on uploads its copy again.
 */
export async function deleteAllChats(userId: string): Promise<void> {
  const pool = await db();
  await pool.query("DELETE FROM asao_chat_messages WHERE user_id = $1", [userId]);
  await pool.query("DELETE FROM asao_chats WHERE user_id = $1", [userId]);
}
