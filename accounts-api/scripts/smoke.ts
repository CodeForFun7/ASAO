// End-to-end check of a running accounts API.
//   npm run smoke                                  → tests http://localhost:3000
//   npm run smoke -- https://<project>.vercel.app  → tests the deployed API
// Creates a throwaway asao-test-…@example.com account. With DATABASE_URL in .env.local it is
// deleted afterwards; otherwise it stays (harmless, and it can't be signed in to by anyone).
import { randomUUID } from "node:crypto";
import { db } from "../lib/db";

const BASE = (process.argv[2] ?? "http://localhost:3000").replace(/\/+$/, "");

async function call(method: string, path: string, opts: { body?: unknown; token?: string } = {}) {
  const res = await fetch(`${BASE}/api/${path}`, {
    method,
    headers: {
      ...(opts.body ? { "Content-Type": "application/json" } : {}),
      ...(opts.token ? { Authorization: `Bearer ${opts.token}` } : {}),
    },
    body: opts.body ? JSON.stringify(opts.body) : undefined,
  });
  const text = await res.text();
  return { status: res.status, data: text ? JSON.parse(text) : null };
}

function expect(label: string, cond: boolean, detail?: unknown) {
  console.log(`${cond ? "ok  " : "FAIL"} ${label}`);
  if (!cond) {
    console.log("     got:", detail);
    process.exitCode = 1;
  }
}

async function main() {
  console.log(`Testing ${BASE}\n`);
  const email = `asao-test-${randomUUID()}@example.com`;
  const password = "test password 1";

  const health = await call("GET", "health");
  expect("database reachable (/api/health)", health.status === 200, health);
  if (health.status !== 200) return;

  const up = await call("POST", "sign-up", { body: { email, password } });
  expect("sign up", up.status === 200 && up.data?.token?.length === 64, up);
  const dup = await call("POST", "sign-up", { body: { email, password } });
  expect("same email again is refused", dup.status === 409, dup);
  const short = await call("POST", "sign-up", { body: { email: `x${email}`, password: "short" } });
  expect("short password is refused", short.status === 400, short);
  const wrong = await call("POST", "sign-in", { body: { email, password: "wrong password" } });
  expect("wrong password is refused", wrong.status === 401, wrong);
  const none = await call("POST", "sign-in", { body: { email: `nobody-${email}`, password } });
  expect("unknown email gets the same answer", none.status === 401 && none.data?.error === wrong.data?.error, none);
  const inn = await call("POST", "sign-in", { body: { email: email.toUpperCase(), password } });
  expect("sign in (email is case-insensitive)", inn.status === 200 && inn.data?.user?.email === email, inn);
  const sess = await call("GET", "session", { token: up.data?.token });
  expect("saved session is recognised", sess.status === 200 && sess.data?.user?.email === email, sess);
  // Chats saved to the account
  const t = up.data?.token;
  const chat = { id: "smokechat1", title: "Smoke chat", createdAt: 1000, updatedAt: 2000 };
  const msg = (id: string, seq: number, role: string) => ({
    id,
    seq,
    role,
    createdAt: 1000 + seq,
    payload: JSON.stringify({ id, role, content: `message ${seq}`, createdAt: 1000 + seq }),
  });
  const noAuth = await call("GET", "chats");
  expect("chats need a session", noAuth.status === 401, noAuth);
  const s1 = await call("POST", "chat-sync", { token: t, body: { chat, messages: [msg("m1", 0, "user"), msg("m2", 1, "assistant")] } });
  expect("save a chat", s1.status === 200, s1);
  const s2 = await call("POST", "chat-sync", {
    token: t,
    body: { chat: { ...chat, title: "Renamed", updatedAt: 3000 }, messages: [msg("m2", 1, "assistant"), msg("m3", 2, "user")] },
  });
  expect("add to it (re-sending a message doesn't duplicate it)", s2.status === 200, s2);
  const list = await call("GET", "chats", { token: t });
  expect(
    "list shows it, renamed",
    list.status === 200 && list.data?.length === 1 && list.data[0].title === "Renamed" && list.data[0].updatedAt === 3000,
    list,
  );
  const one = await call("GET", "chat?id=smokechat1", { token: t });
  expect("open it: 3 messages in order", one.status === 200 && one.data?.messages?.map((m: { id: string }) => m.id).join() === "m1,m2,m3", one);
  const other = await call("POST", "sign-up", { body: { email: `other-${email}`, password } });
  const theirs = await call("GET", "chat?id=smokechat1", { token: other.data?.token });
  expect("another account can't see it", theirs.status === 404, theirs);
  const big = await call("POST", "chat-sync", {
    token: t,
    body: { chat, messages: [{ ...msg("m4", 3, "user"), payload: "x".repeat(300 * 1024) }] },
  });
  expect("oversized message is refused", big.status === 413, big);
  const del = await call("POST", "chat-delete", { token: t, body: { id: "smokechat1" } });
  expect("delete it", del.status === 204, del);
  const marked = await call("GET", "chats", { token: t });
  expect("list marks it deleted", marked.data?.[0]?.deleted === true, marked);
  const late = await call("POST", "chat-sync", { token: t, body: { chat, messages: [msg("m5", 4, "user")] } });
  expect("saving to a deleted chat answers 410", late.status === 410, late);
  await call("POST", "chat-sync", { token: t, body: { chat: { ...chat, id: "smokechat2" }, messages: [msg("m6", 0, "user")] } });
  const wipe = await call("POST", "chats-delete-all", { token: t });
  const empty = await call("GET", "chats", { token: t });
  expect("delete everything online", wipe.status === 204 && empty.data?.length === 0, empty);

  const out = await call("POST", "sign-out", { token: inn.data?.token });
  expect("sign out", out.status === 204, out);
  const gone = await call("GET", "session", { token: inn.data?.token });
  expect("signed-out session is refused", gone.status === 401, gone);

  if (process.env.DATABASE_URL) {
    const pool = await db();
    await pool.query("DELETE FROM asao_users WHERE email IN ($1, $2)", [email, `other-${email}`]);
    await pool.query("DELETE FROM asao_auth_attempts WHERE key IN ($1, $2)", [email, `nobody-${email}`]);
    await pool.end();
    console.log("\nTest account removed.");
  }
}

main().catch((e) => {
  console.error(e);
  process.exitCode = 1;
});
