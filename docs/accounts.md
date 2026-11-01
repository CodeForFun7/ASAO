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

## Nyra chats saved to the account

Nyra's chats follow the user to other PCs. The local `nyra.db` stays the source of truth;
`src-python/chatbot/cloud.py` syncs it with the account in the background.

- ASAO sends the signed-in account to Nyra with every request (`X-Asao-User`,
  `X-Asao-Session`, `X-Asao-Accounts-Url`, `X-Asao-Sync`). Each conversation belongs to the
  account signed in when it started, and only that account sees it. Chats from before
  accounts existed go to the first account that signs in on that PC.
- After every change (new message, rename, delete) Nyra pushes what isn't sent yet. On
  sign-in, and at most once a minute when the chat list is opened, it pulls chats from other
  PCs and removes chats deleted elsewhere. Offline changes stay marked and go out later.
- When a chat that came from another PC is continued, Nyra replays its last 30 turns (as
  text) to the model, since the model's own state only exists on the PC that ran it.
- Messages are capped at 256 KiB online (large activity traces are trimmed; the answer is
  kept) and each account at 50 MB.
- API endpoints, all with `Authorization: Bearer <token>`: `GET /api/chats`,
  `GET /api/chat?id=`, `POST /api/chat-sync`, `POST /api/chat-delete`,
  `POST /api/chats-delete-all`. Tables: `asao_chats` (a deleted chat keeps an empty row with
  `deleted_at`, so other PCs learn of it) and `asao_chat_messages`.
- Nyra settings has a **Save chats to my account** switch (on by default), sync status, a
  **Sync now** button, and, with syncing off, **Delete chats saved online**. That removes
  the online copy only; chats on the PC stay.

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
and sign-out, then saving, listing, opening, renaming, deleting and wiping chats, including
that another account can't see them and oversized messages are refused. With `DATABASE_URL` in `.env.local` it deletes the account afterwards.

`cargo test --lib auth::tests::live_round_trip -- --ignored` (in `src-tauri`) runs the same
round trip through ASAO's own client against the deployed API (or `ASAO_ACCOUNTS_URL`). It
leaves its `asao-test-…` account behind.

## Before a public release

The free Sandbox cluster pauses after 10 days without use, has no backups and allows 15
connections at once. Move to a paid Aeon cluster and update `DATABASE_URL` in Vercel; ASAO
itself doesn't change.
