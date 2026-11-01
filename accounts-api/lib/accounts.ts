import { createHash, randomBytes, randomUUID } from "node:crypto";
import { hash, verify } from "@node-rs/argon2";
import { db } from "./db";

/*
 * The same scheme ASAO's desktop code used, so accounts made earlier still work:
 * Argon2id hashes (m=19456 KiB, t=2, p=1) in PHC format, and 32-byte random session
 * tokens of which only the SHA-256 is stored.
 */

export const MIN_PASSWORD_LENGTH = 8;
const MAX_PASSWORD_LENGTH = 1024;
const MAX_EMAIL_LENGTH = 254;
const SESSION_DAYS = 30;

// Abuse limits, counted in the database because function instances share no memory.
const LIMITS = {
  signInFailPerEmail: { max: 10, minutes: 15 },
  signInFailPerIp: { max: 30, minutes: 15 },
  signUpPerIp: { max: 5, minutes: 60 },
};

export class AuthError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
  }
}

const WRONG_CREDENTIALS = () => new AuthError(401, "The email or password is incorrect.");
const TOO_MANY = () => new AuthError(429, "Too many attempts. Wait a few minutes and try again.");

export interface AuthUser {
  email: string;
}

export interface AuthSession {
  user: AuthUser;
  token: string;
}

export function normaliseEmail(raw: unknown): string | null {
  if (typeof raw !== "string") return null;
  const email = raw.trim().toLowerCase();
  if (email.length > MAX_EMAIL_LENGTH || /\s/.test(email)) return null;
  const at = email.indexOf("@");
  if (at <= 0 || at !== email.lastIndexOf("@")) return null;
  const domain = email.slice(at + 1);
  if (!domain.includes(".") || domain.startsWith(".") || domain.endsWith(".")) return null;
  return email;
}

const ARGON2 = { memoryCost: 19456, timeCost: 2, parallelism: 1 };

export function hashPassword(password: string): Promise<string> {
  return hash(password, ARGON2);
}

async function verifyPassword(stored: string, password: string): Promise<boolean> {
  try {
    return await verify(stored, password);
  } catch {
    return false;
  }
}

// Checked when the email has no account, so response times don't reveal which emails exist.
let dummyHash: Promise<string> | null = null;
const getDummyHash = () => (dummyHash ??= hashPassword("asao-no-such-account"));

function tokenHash(token: string): Buffer {
  return createHash("sha256").update(token).digest();
}

// ── Rate limits ───────────────────────────────────────────────────────────────

async function attempts(kind: string, key: string, minutes: number): Promise<number> {
  const pool = await db();
  const r = await pool.query<{ n: string }>(
    "SELECT count(*) AS n FROM asao_auth_attempts WHERE kind = $1 AND key = $2 AND at > now() - $3::int * interval '1 minute'",
    [kind, key, minutes],
  );
  return Number(r.rows[0]?.n ?? 0);
}

async function record(kind: string, key: string): Promise<void> {
  const pool = await db();
  await pool.query("INSERT INTO asao_auth_attempts (id, kind, key) VALUES ($1, $2, $3)", [randomUUID(), kind, key]);
  // Now and then, drop records too old to matter.
  if (Math.random() < 0.02) {
    await pool.query("DELETE FROM asao_auth_attempts WHERE at < now() - interval '1 day'");
  }
}

// ── Accounts and sessions ─────────────────────────────────────────────────────

async function createSession(userId: string, email: string): Promise<AuthSession> {
  const token = randomBytes(32).toString("hex");
  const pool = await db();
  await pool.query(
    "INSERT INTO asao_sessions (token_hash, user_id, expires_at) VALUES ($1, $2, now() + $3::int * interval '1 day')",
    [tokenHash(token), userId, SESSION_DAYS],
  );
  return { user: { email }, token };
}

export async function signUp(rawEmail: unknown, password: unknown, ip: string): Promise<AuthSession> {
  const email = normaliseEmail(rawEmail);
  if (!email) throw new AuthError(400, "That doesn't look like an email address.");
  if (typeof password !== "string" || [...password].length < MIN_PASSWORD_LENGTH)
    throw new AuthError(400, `Use at least ${MIN_PASSWORD_LENGTH} characters for your password.`);
  if (password.length > MAX_PASSWORD_LENGTH) throw new AuthError(400, "That password is too long.");

  const { max, minutes } = LIMITS.signUpPerIp;
  if ((await attempts("sign-up", ip, minutes)) >= max) throw TOO_MANY();
  await record("sign-up", ip);

  const id = randomUUID();
  const pool = await db();
  try {
    await pool.query("INSERT INTO asao_users (id, email, password_hash) VALUES ($1, $2, $3)", [
      id,
      email,
      await hashPassword(password),
    ]);
  } catch (e) {
    if ((e as { code?: string }).code === "23505")
      throw new AuthError(409, "An account with this email already exists. Sign in instead.");
    throw e;
  }
  return createSession(id, email);
}

export async function signIn(rawEmail: unknown, password: unknown, ip: string): Promise<AuthSession> {
  const email = normaliseEmail(rawEmail);
  if (!email || typeof password !== "string" || password.length > MAX_PASSWORD_LENGTH) throw WRONG_CREDENTIALS();

  const perEmail = LIMITS.signInFailPerEmail;
  const perIp = LIMITS.signInFailPerIp;
  if (
    (await attempts("sign-in-fail-email", email, perEmail.minutes)) >= perEmail.max ||
    (await attempts("sign-in-fail-ip", ip, perIp.minutes)) >= perIp.max
  )
    throw TOO_MANY();

  const pool = await db();
  const r = await pool.query<{ id: string; password_hash: string }>(
    "SELECT id::text AS id, password_hash FROM asao_users WHERE email = $1",
    [email],
  );
  const row = r.rows[0];
  const ok = await verifyPassword(row?.password_hash ?? (await getDummyHash()), password);
  if (!row || !ok) {
    await record("sign-in-fail-email", email);
    await record("sign-in-fail-ip", ip);
    throw WRONG_CREDENTIALS();
  }

  await pool.query("DELETE FROM asao_sessions WHERE user_id = $1 AND expires_at <= now()", [row.id]);
  return createSession(row.id, email);
}

/** The account a token belongs to, or null when it expired or was signed out. */
export async function restore(token: string): Promise<AuthUser | null> {
  const pool = await db();
  const r = await pool.query<{ email: string }>(
    `SELECT u.email FROM asao_sessions s JOIN asao_users u ON u.id = s.user_id
     WHERE s.token_hash = $1 AND s.expires_at > now()`,
    [tokenHash(token)],
  );
  return r.rows[0] ? { email: r.rows[0].email } : null;
}

export async function signOut(token: string): Promise<void> {
  const pool = await db();
  await pool.query("DELETE FROM asao_sessions WHERE token_hash = $1", [tokenHash(token)]);
}

export async function health(): Promise<void> {
  const pool = await db();
  await pool.query("SELECT 1");
}
