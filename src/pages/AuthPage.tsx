import React, { useState } from "react";
import { Eye, EyeOff, Loader2, Lock, Mail } from "lucide-react";
import { Topbar } from "../components/layout/Topbar";
import { useAuthStore } from "../stores/auth-store";
import { isValidEmail, MIN_PASSWORD_LENGTH } from "../services/auth";
import type { AuthMode } from "../types/auth";

interface FieldErrors {
  email?: string;
  password?: string;
  confirm?: string;
}

function validate(mode: AuthMode, email: string, password: string, confirm: string): FieldErrors {
  const errors: FieldErrors = {};
  if (!email.trim()) errors.email = "Enter your email address.";
  else if (!isValidEmail(email)) errors.email = "That doesn't look like an email address.";

  if (!password) errors.password = "Enter your password.";
  else if (mode === "sign-up" && password.length < MIN_PASSWORD_LENGTH)
    errors.password = `Use at least ${MIN_PASSWORD_LENGTH} characters.`;

  if (mode === "sign-up" && password && confirm !== password) errors.confirm = "The passwords don't match.";
  return errors;
}

const inputClass = (invalid: boolean) =>
  `w-full rounded-lg border bg-lunar-bg pl-9 pr-3 py-2.5 text-sm text-lunar-text placeholder:text-lunar-muted outline-none transition-colors ${
    invalid ? "border-lunar-critical/70 focus:border-lunar-critical" : "border-lunar-border focus:border-lunar-muted"
  }`;

const FieldError: React.FC<{ id: string; message?: string }> = ({ id, message }) =>
  message ? (
    <p id={id} className="mt-1 text-[11px] text-lunar-critical">
      {message}
    </p>
  ) : null;

export const AuthPage: React.FC = () => {
  const submit = useAuthStore((s) => s.submit);
  const submitting = useAuthStore((s) => s.submitting);
  const serverError = useAuthStore((s) => s.error);
  const clearError = useAuthStore((s) => s.clearError);

  const [mode, setMode] = useState<AuthMode>("sign-in");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  // Errors show only after the first submit, then update as the user types.
  const [attempted, setAttempted] = useState(false);

  const errors = attempted ? validate(mode, email, password, confirm) : {};
  const isSignUp = mode === "sign-up";

  const switchMode = (next: AuthMode) => {
    setMode(next);
    setConfirm("");
    setAttempted(false);
    clearError();
  };

  const onSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    setAttempted(true);
    if (Object.keys(validate(mode, email, password, confirm)).length > 0) return;
    void submit(mode, { email: email.trim(), password });
  };

  return (
    <div className="h-screen w-screen flex flex-col bg-lunar-bg text-lunar-text overflow-hidden">
      <Topbar />
      <main className="flex-1 overflow-y-auto flex items-center justify-center p-6">
        <div className="w-full max-w-sm">
          <div className="text-center mb-6">
            <h1 className="text-lg font-semibold tracking-tight text-lunar-white">
              {isSignUp ? "Create your ASAO account" : "Sign in to ASAO"}
            </h1>
            <p className="text-xs text-lunar-text-sec mt-1.5">
              {isSignUp ? "Use your email address and choose a password." : "Use the email address and password for your account."}
            </p>
          </div>

          <div className="lunar-glass-card rounded-xl p-5">
            <div role="tablist" className="grid grid-cols-2 gap-1 p-1 rounded-lg bg-lunar-bg border border-lunar-border mb-5">
              {(["sign-in", "sign-up"] as const).map((m) => (
                <button
                  key={m}
                  type="button"
                  role="tab"
                  aria-selected={mode === m}
                  onClick={() => switchMode(m)}
                  className={`py-1.5 rounded-md text-xs font-medium transition-colors cursor-pointer ${
                    mode === m ? "bg-lunar-elevated text-lunar-white" : "text-lunar-text-sec hover:text-lunar-text"
                  }`}
                >
                  {m === "sign-in" ? "Sign in" : "Create account"}
                </button>
              ))}
            </div>

            <form onSubmit={onSubmit} noValidate className="space-y-4">
              <div>
                <label htmlFor="auth-email" className="block text-[11px] text-lunar-text-sec mb-1">
                  Email
                </label>
                <div className="relative">
                  <Mail className="w-3.5 h-3.5 text-lunar-muted absolute left-3 top-1/2 -translate-y-1/2 pointer-events-none" />
                  <input
                    id="auth-email"
                    type="email"
                    autoComplete="email"
                    autoFocus
                    value={email}
                    onChange={(e) => setEmail(e.target.value)}
                    placeholder="you@example.com"
                    aria-invalid={!!errors.email}
                    aria-describedby={errors.email ? "auth-email-error" : undefined}
                    className={inputClass(!!errors.email)}
                  />
                </div>
                <FieldError id="auth-email-error" message={errors.email} />
              </div>

              <div>
                <label htmlFor="auth-password" className="block text-[11px] text-lunar-text-sec mb-1">
                  Password
                </label>
                <div className="relative">
                  <Lock className="w-3.5 h-3.5 text-lunar-muted absolute left-3 top-1/2 -translate-y-1/2 pointer-events-none" />
                  <input
                    id="auth-password"
                    type={showPassword ? "text" : "password"}
                    autoComplete={isSignUp ? "new-password" : "current-password"}
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    aria-invalid={!!errors.password}
                    aria-describedby={errors.password ? "auth-password-error" : isSignUp ? "auth-password-hint" : undefined}
                    className={`${inputClass(!!errors.password)} pr-9`}
                  />
                  <button
                    type="button"
                    onClick={() => setShowPassword((v) => !v)}
                    title={showPassword ? "Hide password" : "Show password"}
                    aria-label={showPassword ? "Hide password" : "Show password"}
                    className="absolute right-2 top-1/2 -translate-y-1/2 p-1 rounded text-lunar-muted hover:text-lunar-white cursor-pointer"
                  >
                    {showPassword ? <EyeOff className="w-3.5 h-3.5" /> : <Eye className="w-3.5 h-3.5" />}
                  </button>
                </div>
                {errors.password ? (
                  <FieldError id="auth-password-error" message={errors.password} />
                ) : (
                  isSignUp && (
                    <p id="auth-password-hint" className="mt-1 text-[11px] text-lunar-muted">
                      At least {MIN_PASSWORD_LENGTH} characters.
                    </p>
                  )
                )}
              </div>

              {isSignUp && (
                <div>
                  <label htmlFor="auth-confirm" className="block text-[11px] text-lunar-text-sec mb-1">
                    Confirm password
                  </label>
                  <div className="relative">
                    <Lock className="w-3.5 h-3.5 text-lunar-muted absolute left-3 top-1/2 -translate-y-1/2 pointer-events-none" />
                    <input
                      id="auth-confirm"
                      type={showPassword ? "text" : "password"}
                      autoComplete="new-password"
                      value={confirm}
                      onChange={(e) => setConfirm(e.target.value)}
                      aria-invalid={!!errors.confirm}
                      aria-describedby={errors.confirm ? "auth-confirm-error" : undefined}
                      className={inputClass(!!errors.confirm)}
                    />
                  </div>
                  <FieldError id="auth-confirm-error" message={errors.confirm} />
                </div>
              )}

              {serverError && (
                <p role="alert" className="text-xs text-lunar-critical bg-lunar-critical/10 border border-lunar-critical/30 rounded-lg px-3 py-2">
                  {serverError}
                </p>
              )}

              <button
                type="submit"
                disabled={submitting}
                className={`w-full inline-flex items-center justify-center gap-2 px-3.5 py-2.5 rounded-lg text-xs font-medium border transition-colors ${
                  submitting
                    ? "bg-lunar-surface text-lunar-muted border-lunar-border cursor-not-allowed"
                    : "bg-lunar-white text-lunar-bg border-lunar-white hover:bg-lunar-text cursor-pointer"
                }`}
              >
                {submitting && <Loader2 className="w-3.5 h-3.5 animate-spin" />}
                <span>{isSignUp ? "Create account" : "Sign in"}</span>
              </button>
            </form>
          </div>

          <p className="text-center text-xs text-lunar-text-sec mt-4">
            {isSignUp ? "Already have an account? " : "New to ASAO? "}
            <button
              type="button"
              onClick={() => switchMode(isSignUp ? "sign-in" : "sign-up")}
              className="text-lunar-white font-medium hover:underline cursor-pointer"
            >
              {isSignUp ? "Sign in" : "Create an account"}
            </button>
          </p>
        </div>
      </main>
    </div>
  );
};
