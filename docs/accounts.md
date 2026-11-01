# Accounts

ASAO asks for an email and password before showing the app. Accounts are stored in
YugabyteDB, behind a small API that runs as a Vercel Function.

```
ASAO (any PC)  ──HTTPS──►  accounts API (Vercel, accounts-api/)  ──►  YugabyteDB Aeon
```

ASAO only knows the API's public address. The database password lives in Vercel's
environment variables and never ships with the app.

## How it works

- `accounts-api/api/[action].ts` serves:
  - `POST /api/sign-up` and `POST /api/sign-in` with `{ email, password }`, returning `{ user, token }`
  - `GET /api/session` with `Authorization: Bearer <token>`, returning `{ user }` or 401
  - `POST /api/sign-out` with the same header
  - `GET /api/health`, which checks the database answers
- Passwords are hashed with Argon2id; only the hash is stored.
- A session is a random 32-byte token that lasts 30 days. The database stores only its
  SHA-256. ASAO keeps the token and email in local storage, never the password.
- Repeated failures are slowed down: 10 failed sign-ins per email or 30 per IP address in
  15 minutes, and 5 sign-ups per IP address an hour.
- Tables `asao_users`, `asao_sessions` and `asao_auth_attempts` are created automatically.
- At startup ASAO checks the saved token. If it expired or was signed out, the user signs in
  again; if the server can't be reached, the saved session is kept so ASAO works offline.
- On the desktop side, `src-tauri/src/auth/mod.rs` is an HTTPS client for the API, and the
  Tauri commands are `auth_sign_up`, `auth_sign_in`, `auth_restore_session`, `auth_sign_out`.

## Settings

| Where | Variable | Value |
|---|---|---|
| Vercel project | `DATABASE_URL` | Aeon's YSQL connection string (special password characters as `%XX`) |
| Vercel project | `DATABASE_CA_CERT` | The full text of Aeon's `root.crt` |
| ASAO `.env` (optional) | `ASAO_ACCOUNTS_URL` | Overrides the API address, e.g. `http://localhost:3000` |

The deployed API, `https://asao-accounts-api-two.vercel.app`, is written into
`src-tauri/src/auth/mod.rs` (`DEFAULT_ACCOUNTS_URL`), so ASAO needs no settings. It is a
public address, not a secret.

On Vercel the project's Root Directory is `accounts-api`, the framework preset is Other, and
pushing to `main` redeploys it. `GET /api/health` returns `{ "ok": false, "reason": ... }`
when the database can't be used.

Aeon's IP allow list must include `0.0.0.0/0`, because Vercel doesn't connect from fixed
addresses. The database is still protected by its password and TLS.

## Running the API locally

```powershell
cd accounts-api
npm install
copy .env.example .env.local   # then fill in DATABASE_URL and DATABASE_CA_CERT
npm run dev                    # http://localhost:3000
```

Set `ASAO_ACCOUNTS_URL=http://localhost:3000` in the root `.env` to point ASAO at it.

## Tests

```powershell
cd accounts-api
npm run typecheck
npm run smoke                                   # against http://localhost:3000
npm run smoke -- https://<project>.vercel.app   # against the deployed API
```

The smoke test creates a throwaway `asao-test-…@example.com` account and checks sign-up,
duplicate sign-up, short password, wrong password, unknown email, sign-in, session restore
and sign-out. With `DATABASE_URL` in `.env.local` it deletes the account afterwards.

`cargo test --lib auth::tests::live_round_trip -- --ignored` (in `src-tauri`) runs the same
round trip through ASAO's own client against the deployed API (or `ASAO_ACCOUNTS_URL`). It
leaves its `asao-test-…` account behind.

## Before a public release

The free Sandbox cluster pauses after 10 days without use, has no backups and allows 15
connections at once. Move to a paid Aeon cluster and update `DATABASE_URL` in Vercel; ASAO
itself doesn't change.
