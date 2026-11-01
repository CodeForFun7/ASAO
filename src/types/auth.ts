export type AuthMode = "sign-in" | "sign-up";

export interface AuthUser {
  email: string;
}

export interface AuthSession {
  user: AuthUser;
  /** Random session token from the backend. The database stores only its hash. */
  token: string;
}

export interface AuthCredentials {
  email: string;
  password: string;
}
