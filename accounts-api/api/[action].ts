import type { VercelRequest, VercelResponse } from "@vercel/node";
import { AuthError, health, restore, sessionUser, signIn, signOut, signUp } from "../lib/accounts";
import { deleteAllChats, deleteChat, getChat, listChats, syncChat } from "../lib/chats";
import { ConfigError } from "../lib/db";

/*
 * POST /api/sign-up   { email, password }        → 200 { user, token }
 * POST /api/sign-in   { email, password }        → 200 { user, token }
 * GET  /api/session   Authorization: Bearer <t>  → 200 { user } | 401
 * POST /api/sign-out  Authorization: Bearer <t>  → 204
 * GET  /api/health                               → 200 { ok: true } when the database answers
 *
 * Nyra chats saved to the account (all need Authorization: Bearer <token>):
 * GET  /api/chats                                 → 200 [{ id, title, createdAt, updatedAt, deleted }]
 * GET  /api/chat?id=<id>                          → 200 { chat, messages: [{ id, seq, role, createdAt, payload }] }
 * POST /api/chat-sync  { chat, messages }         → 200 { ok } | 410 when deleted on another PC
 * POST /api/chat-delete { id }                    → 204
 * POST /api/chats-delete-all                      → 204
 *
 * Errors are { error: "<message for the user>" }.
 */

function clientIp(req: VercelRequest): string {
  const forwarded = req.headers["x-forwarded-for"];
  const first = (Array.isArray(forwarded) ? forwarded[0] : forwarded)?.split(",")[0]?.trim();
  return first || req.socket?.remoteAddress || "unknown";
}

function bearer(req: VercelRequest): string | null {
  const m = /^Bearer\s+([0-9a-f]{64})$/i.exec(req.headers.authorization ?? "");
  return m ? m[1].toLowerCase() : null;
}

function body(req: VercelRequest): Record<string, unknown> {
  return req.body && typeof req.body === "object" ? (req.body as Record<string, unknown>) : {};
}

const CHAT_ACTIONS = new Set(["chats", "chat", "chat-sync", "chat-delete", "chats-delete-all"]);

export default async function handler(req: VercelRequest, res: VercelResponse) {
  const action = String(req.query.action ?? "");
  const method = req.method ?? "GET";
  res.setHeader("Cache-Control", "no-store");

  try {
    if (action === "sign-up" && method === "POST") {
      const b = body(req);
      return res.status(200).json(await signUp(b.email, b.password, clientIp(req)));
    }
    if (action === "sign-in" && method === "POST") {
      const b = body(req);
      return res.status(200).json(await signIn(b.email, b.password, clientIp(req)));
    }
    if (action === "session" && method === "GET") {
      const token = bearer(req);
      const user = token ? await restore(token) : null;
      return user ? res.status(200).json({ user }) : res.status(401).json({ error: "Your session has ended. Sign in again." });
    }
    if (action === "sign-out" && method === "POST") {
      const token = bearer(req);
      if (token) await signOut(token);
      return res.status(204).end();
    }
    if (CHAT_ACTIONS.has(action)) {
      const token = bearer(req);
      const user = token ? await sessionUser(token) : null;
      if (!user) return res.status(401).json({ error: "Your session has ended. Sign in again." });
      if (action === "chats" && method === "GET") return res.status(200).json(await listChats(user.id));
      if (action === "chat" && method === "GET") return res.status(200).json(await getChat(user.id, req.query.id));
      if (action === "chat-sync" && method === "POST") return res.status(200).json(await syncChat(user.id, body(req)));
      if (action === "chat-delete" && method === "POST") {
        await deleteChat(user.id, body(req).id);
        return res.status(204).end();
      }
      if (action === "chats-delete-all" && method === "POST") {
        await deleteAllChats(user.id);
        return res.status(204).end();
      }
    }
    if (action === "health" && method === "GET") {
      try {
        await health();
      } catch (e) {
        // Setup aid: says why the database can't be used. Never contains the password.
        const err = e as { code?: string; message?: string };
        console.error("[health]", e);
        return res.status(503).json({ ok: false, reason: [err.code, err.message].filter(Boolean).join(": ") });
      }
      return res.status(200).json({ ok: true });
    }
    return res.status(404).json({ error: "Not found." });
  } catch (e) {
    if (e instanceof AuthError) return res.status(e.status).json({ error: e.message });
    if (e instanceof ConfigError) {
      console.error("[config]", e.message);
      return res.status(500).json({ error: "The account server isn't set up yet." });
    }
    console.error("[accounts]", e);
    return res.status(503).json({ error: "The account server couldn't reach its database. Try again shortly." });
  }
}
