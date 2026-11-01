import { Pool } from "pg";

/*
 * One small pool per function instance. Vercel reuses warm instances, so the pool
 * (and its connections) survive between requests.
 *
 * Settings, from the Vercel project's Environment Variables:
 *   DATABASE_URL     YugabyteDB connection string (the one Aeon shows for YSQL)
 *   DATABASE_CA_CERT contents of Aeon's root.crt (paste the whole PEM text)
 */

const SCHEMA = `
CREATE TABLE IF NOT EXISTS asao_users (
    id            UUID PRIMARY KEY,
    email         TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE IF NOT EXISTS asao_sessions (
    token_hash BYTEA PRIMARY KEY,
    user_id    UUID NOT NULL REFERENCES asao_users (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS asao_sessions_user_idx ON asao_sessions (user_id);
CREATE TABLE IF NOT EXISTS asao_auth_attempts (
    id   UUID PRIMARY KEY,
    kind TEXT NOT NULL,
    key  TEXT NOT NULL,
    at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS asao_auth_attempts_key_idx ON asao_auth_attempts (kind, key, at);
`;

export class ConfigError extends Error {}

/**
 * pg would try to read `sslrootcert` as a file, which doesn't exist on Vercel, so the
 * SSL options are taken out of the URL and the certificate comes from DATABASE_CA_CERT.
 */
export function stripSslParams(url: string): string {
  const [base, query] = url.split("?", 2);
  if (!query) return url;
  const kept = query.split("&").filter((p) => p && !/^(ssl|sslmode|sslrootcert|sslcert|sslkey)=/.test(p));
  return kept.length ? `${base}?${kept.join("&")}` : base;
}

let pool: Pool | null = null;
let schemaReady: Promise<void> | null = null;

function createPool(): Pool {
  const url = process.env.DATABASE_URL?.trim();
  if (!url) throw new ConfigError("DATABASE_URL is not set.");
  const ca = process.env.DATABASE_CA_CERT?.trim();
  if (!ca) throw new ConfigError("DATABASE_CA_CERT is not set.");

  const p = new Pool({
    connectionString: stripSslParams(url),
    // Pasted PEM text sometimes arrives with literal "\n" instead of line breaks.
    ssl: { ca: ca.replace(/\\n/g, "\n"), rejectUnauthorized: true },
    max: 3,
    idleTimeoutMillis: 10_000,
    connectionTimeoutMillis: 10_000,
    application_name: "asao-accounts-api",
  });
  // A dropped idle connection must not crash the function.
  p.on("error", (e) => console.error("[db] idle connection error:", e.message));
  return p;
}

export async function db(): Promise<Pool> {
  pool ??= createPool();
  schemaReady ??= pool
    .query(SCHEMA)
    .then(() => undefined)
    .catch((e) => {
      schemaReady = null;
      throw e;
    });
  await schemaReady;
  return pool;
}
