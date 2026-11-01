import { invoke } from "@tauri-apps/api/core";
import type { AuthCredentials, AuthSession, AuthUser } from "../types/auth";

/*
 * Accounts live in YugabyteDB behind the ASAO accounts API (accounts-api/, on Vercel).
 * The Rust side (src-tauri/src/auth) calls that API; the frontend only ever sees the
 * session token.
 */

const SESSION_KEY = "asao-session";

// Keep in step with MIN_PASSWORD_LENGTH in accounts-api/lib/accounts.ts.
export const MIN_PASSWORD_LENGTH = 8;

const EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

export function isValidEmail(email: string): boolean {
  return EMAIL_PATTERN.test(email.trim());
}

export function signIn({ email, password }: AuthCredentials): Promise<AuthSession> {
  return invoke<AuthSession>("auth_sign_in", { email, password });
}

export function signUp({ email, password }: AuthCredentials): Promise<AuthSession> {
  return invoke<AuthSession>("auth_sign_up", { email, password });
}

/** The account for a saved token, or null when it expired or was signed out elsewhere. Throws when it can't be checked. */
export function restoreSession(token: string): Promise<AuthUser | null> {
  return invoke<AuthUser | null>("auth_restore_session", { token });
}

export function signOut(token: string): Promise<void> {
  return invoke<void>("auth_sign_out", { token });
}

// Only the email and token are kept; the password is never stored.
export function loadSession(): AuthSession | null {
  try {
    const raw = localStorage.getItem(SESSION_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as AuthSession;
    // Sessions saved by the UI-only version had no token; they must sign in again.
    return parsed?.user?.email && parsed.token ? parsed : null;
  } catch {
    return null;
  }
}

export function saveSession(session: AuthSession | null) {
  try {
    if (session) localStorage.setItem(SESSION_KEY, JSON.stringify(session));
    else localStorage.removeItem(SESSION_KEY);
  } catch {
    // Ignore unavailable browser storage; the session then lasts until the app closes.
  }
}
