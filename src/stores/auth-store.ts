import { create } from "zustand";
import type { AuthCredentials, AuthMode, AuthSession } from "../types/auth";
import { loadSession, restoreSession, saveSession, signIn, signOut, signUp } from "../services/auth";

interface AuthStoreState {
  session: AuthSession | null;
  submitting: boolean;
  error: string | null;
  submit: (mode: AuthMode, credentials: AuthCredentials) => Promise<void>;
  /** Checks the saved session with the server once at startup. */
  verifySession: () => Promise<void>;
  signOut: () => Promise<void>;
  clearError: () => void;
}

function errorMessage(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

export const useAuthStore = create<AuthStoreState>((set, get) => ({
  session: loadSession(),
  submitting: false,
  error: null,

  submit: async (mode, credentials) => {
    set({ submitting: true, error: null });
    try {
      const session = mode === "sign-in" ? await signIn(credentials) : await signUp(credentials);
      saveSession(session);
      set({ session, submitting: false });
    } catch (e) {
      set({ submitting: false, error: errorMessage(e) });
    }
  },

  verifySession: async () => {
    const session = get().session;
    if (!session) return;
    try {
      const user = await restoreSession(session.token);
      // Ignore the answer if the user signed out or in again meanwhile.
      if (get().session?.token !== session.token) return;
      if (user) {
        const next = { ...session, user };
        saveSession(next);
        set({ session: next });
      } else {
        saveSession(null);
        set({ session: null, error: "Your session has ended. Sign in again." });
      }
    } catch {
      // Offline or the server is down: keep the saved session so ASAO still works.
    }
  },

  signOut: async () => {
    const session = get().session;
    saveSession(null);
    set({ session: null, error: null });
    if (session) {
      try {
        await signOut(session.token);
      } catch {
        // Signed out on this device; the server session expires on its own.
      }
    }
  },

  clearError: () => set({ error: null }),
}));
