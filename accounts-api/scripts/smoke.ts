// End-to-end check of a running accounts API.
//   npm run smoke                                  → tests http://localhost:3000
//   npm run smoke -- https://<project>.vercel.app  → tests the deployed API
// Creates a throwaway asao-test-…@example.com account. With DATABASE_URL in .env.local it is
// deleted afterwards; otherwise it stays (harmless, and it can't be signed in to by anyone).
import { randomUUID } from "node:crypto";

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
  const out = await call("POST", "sign-out", { token: inn.data?.token });
  expect("sign out", out.status === 204, out);
  const gone = await call("GET", "session", { token: inn.data?.token });
  expect("signed-out session is refused", gone.status === 401, gone);

  if (process.env.DATABASE_URL) {
    const { db } = await import("../lib/db");
    const pool = await db();
    await pool.query("DELETE FROM asao_users WHERE email = $1", [email]);
    await pool.query("DELETE FROM asao_auth_attempts WHERE key IN ($1, $2)", [email, `nobody-${email}`]);
    await pool.end();
    console.log("\nTest account removed.");
  }
}

main().catch((e) => {
  console.error(e);
  process.exitCode = 1;
});
