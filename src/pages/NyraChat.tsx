import React, { useEffect, useMemo, useRef, useState } from "react";
import { AlertTriangle, ArrowUp, RefreshCw, Settings2, Sparkles, Square } from "lucide-react";
import { ActivityFeed } from "../components/nyra/ActivityFeed";
import { ConversationList } from "../components/nyra/ConversationList";
import { Markdown } from "../components/nyra/Markdown";
import { NyraSettingsDialog } from "../components/nyra/NyraSettingsDialog";
import { useNyraStore } from "../stores/nyra-store";
import type { NyraMessage } from "../types/nyra";

const suggestions = [
  { title: "Run a full system check", detail: "Processes, startup, storage, unused apps and leftovers at once" },
  { title: "What is slowing my PC down?", detail: "Heavy background processes and startup apps" },
  { title: "Free up space on C:", detail: "Temporary files, large files, duplicates and leftovers" },
  { title: "Find duplicate photos in my Pictures folder", detail: "Identical files and the space they waste" },
];

export const NyraChat: React.FC = () => {
  const s = useNyraStore();
  const [draft, setDraft] = useState("");
  const [showSettings, setShowSettings] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const stickToBottom = useRef(true);

  useEffect(() => {
    void s.init();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // An "Ask Nyra" button queued a message: send it once the page is up.
  // Without an API key, or while another run is active, leave it in the composer instead.
  useEffect(() => {
    if (!s.pendingPrompt) return;
    const store = useNyraStore.getState();
    const prompt = store.takePendingPrompt();
    if (!prompt) return;
    if (!store.settings.apiKey) {
      setDraft(prompt);
      setShowSettings(true);
      return;
    }
    if (store.live) {
      setDraft(prompt);
      return;
    }
    stickToBottom.current = true;
    void store.send(prompt);
  }, [s.pendingPrompt]);

  const busy = s.live !== null;
  const liveHere = s.liveMessage && (s.live?.conversationId ?? null) === s.activeId ? s.liveMessage : null;
  const shown: NyraMessage[] = useMemo(() => (liveHere ? [...s.messages, liveHere] : s.messages), [s.messages, liveHere]);
  const active = s.conversations.find((c) => c.id === s.activeId);

  // Follow the live feed unless the user scrolled up to read.
  useEffect(() => {
    const el = scrollRef.current;
    if (el && stickToBottom.current) el.scrollTop = el.scrollHeight;
  }, [shown]);

  const send = () => {
    const text = draft.trim();
    if (!text || busy) return;
    if (!s.settings.apiKey) {
      setShowSettings(true);
      return;
    }
    setDraft("");
    stickToBottom.current = true;
    void s.send(text);
  };

  const onKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      send();
    }
  };

  return (
    <section className="relative flex-1 min-h-0 flex bg-lunar-bg text-lunar-text">
      <ConversationList
        conversations={s.conversations}
        activeId={s.activeId}
        runningId={s.live?.conversationId ?? null}
        onOpen={(id) => void s.openConversation(id)}
        onNew={s.newChat}
        onRename={(id, t) => void s.renameConversation(id, t)}
        onDelete={(id) => void s.removeConversation(id)}
      />

      <div className="flex-1 min-w-0 flex flex-col">
        <header className="h-12 shrink-0 px-5 flex items-center justify-between border-b border-lunar-border/70">
          <div className="flex items-center gap-2 min-w-0">
            <Sparkles className="w-4 h-4 text-lunar-ai shrink-0" />
            <span className="text-sm font-medium text-lunar-white truncate">{active?.title ?? "Nyra"}</span>
          </div>
          <div className="flex items-center gap-3">
            <ServiceStatus service={s.service} bridge={s.bridge} onRetry={() => void s.checkHealth().then(s.refreshConversations)} />
            <button
              type="button"
              onClick={() => setShowSettings(true)}
              className="w-8 h-8 flex items-center justify-center rounded-md text-lunar-text-sec hover:bg-lunar-surface-2"
              title="Nyra settings"
              aria-label="Nyra settings"
            >
              <Settings2 className="w-4 h-4" />
            </button>
          </div>
        </header>

        {s.service === "offline" && (
          <div className="mx-5 mt-3 flex items-start gap-2 rounded-lg border border-lunar-warning/40 bg-lunar-warning/5 px-3 py-2 text-xs text-lunar-text-sec">
            <AlertTriangle className="w-3.5 h-3.5 text-lunar-warning mt-0.5 shrink-0" />
            <span>
              Nyra's agent service is not running. It starts with <code className="font-mono">pnpm tauri dev</code>; you can also run{" "}
              <code className="font-mono">python -m uvicorn chatbot.main:app --port 18743</code> from <code className="font-mono">src-python</code>.
            </span>
          </div>
        )}

        <div
          ref={scrollRef}
          onScroll={(e) => {
            const el = e.currentTarget;
            stickToBottom.current = el.scrollHeight - el.scrollTop - el.clientHeight < 80;
          }}
          className="flex-1 min-h-0 overflow-y-auto"
        >
          {shown.length === 0 && !s.loadingConversation ? (
            <div className="min-h-full flex flex-col items-center justify-center px-6 py-10">
              <div className="w-full max-w-[720px]">
                <div className="flex items-center gap-3 mb-7">
                  <div className="w-9 h-9 rounded-xl border border-lunar-border bg-lunar-surface-2 flex items-center justify-center">
                    <Sparkles className="w-4 h-4 text-lunar-ai" />
                  </div>
                  <div>
                    <h1 className="text-lg font-medium tracking-tight text-lunar-white">What should I take care of?</h1>
                    <p className="text-xs text-lunar-muted mt-0.5">
                      Nyra inspects, cleans and optimises this PC. You approve every change.
                    </p>
                  </div>
                </div>
                <Composer draft={draft} setDraft={setDraft} onSend={send} onStop={() => void s.stop()} onKeyDown={onKeyDown} busy={busy} />
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-2 mt-3">
                  {suggestions.map((sg) => (
                    <button
                      key={sg.title}
                      type="button"
                      onClick={() => setDraft(sg.title)}
                      className="text-left rounded-xl border border-lunar-border/80 bg-lunar-surface/60 hover:bg-lunar-surface-2 px-3.5 py-3 transition-colors"
                    >
                      <span className="block text-xs font-medium text-lunar-text">{sg.title}</span>
                      <span className="block text-[11px] leading-relaxed text-lunar-muted mt-1">{sg.detail}</span>
                    </button>
                  ))}
                </div>
                {s.error && <p className="mt-3 text-center text-xs text-lunar-critical">{s.error}</p>}
                {!s.settings.apiKey && (
                  <p className="mt-5 text-center text-[11px] text-lunar-muted">Add an API key in settings to start.</p>
                )}
              </div>
            </div>
          ) : (
            <div className="max-w-[760px] mx-auto px-6 py-8 space-y-7">
              {s.loadingConversation && <p className="text-xs text-lunar-muted">Loading conversation…</p>}
              {shown.map((m) => (
                <MessageView
                  key={m.id}
                  message={m}
                  live={m === liveHere}
                  onConfirm={(a, c, ok) => void s.confirm(a, c, ok)}
                />
              ))}
              {s.error && <p className="text-xs text-lunar-critical pl-10">{s.error}</p>}
            </div>
          )}
        </div>

        {shown.length > 0 && (
          <div className="px-6 pb-5 pt-2">
            <div className="max-w-[760px] mx-auto">
              <Composer draft={draft} setDraft={setDraft} onSend={send} onStop={() => void s.stop()} onKeyDown={onKeyDown} busy={busy} />
              <p className="text-center text-[10px] text-lunar-muted mt-2">
                Nyra can make mistakes. Check each confirmation before you approve it.
              </p>
            </div>
          </div>
        )}
      </div>

      {showSettings && (
        <NyraSettingsDialog settings={s.settings} onSave={s.updateSettings} onClose={() => setShowSettings(false)} />
      )}
    </section>
  );
};

const ServiceStatus: React.FC<{ service: string; bridge: boolean; onRetry: () => void }> = ({ service, bridge, onRetry }) => {
  const ok = service === "online" && bridge;
  const text =
    service === "offline" ? "Agent offline" : service === "unknown" ? "Connecting…" : bridge ? "Connected" : "ASAO bridge unavailable";
  return (
    <button
      type="button"
      onClick={onRetry}
      className="flex items-center gap-1.5 text-[11px] text-lunar-text-sec hover:text-lunar-white"
      title={ok ? "Nyra and ASAO are connected" : "Check again"}
    >
      <span className={`w-1.5 h-1.5 rounded-full ${ok ? "bg-lunar-healthy" : service === "unknown" ? "bg-lunar-muted" : "bg-lunar-warning"}`} />
      {text}
      {!ok && <RefreshCw className="w-3 h-3" />}
    </button>
  );
};

const MessageView: React.FC<{
  message: NyraMessage;
  live: boolean;
  onConfirm: (activityId: string, confirmationId: string, approved: boolean) => void;
}> = ({ message, live, onConfirm }) => {
  if (message.role === "user") {
    return (
      <article className="flex justify-end">
        <div className="max-w-[85%] rounded-2xl rounded-br-md bg-lunar-elevated px-4 py-2.5 text-sm leading-6 text-lunar-text whitespace-pre-wrap break-words select-text">
          {message.content}
        </div>
      </article>
    );
  }
  const status = message.run?.status;
  return (
    <article className="flex gap-3">
      <div className="w-7 h-7 shrink-0 rounded-lg flex items-center justify-center bg-lunar-surface-2 border border-lunar-border">
        <Sparkles className="w-3.5 h-3.5 text-lunar-ai" />
      </div>
      <div className="flex-1 min-w-0 space-y-3 pt-0.5">
        {message.run && <ActivityFeed run={message.run} live={live} onConfirm={onConfirm} />}
        {message.content && <Markdown text={message.content} />}
        {live && message.content && <span className="inline-block w-1.5 h-4 bg-lunar-ai/70 animate-pulse align-middle" />}
        {status === "cancelled" && <p className="text-[11px] text-lunar-muted">Stopped.</p>}
        {message.error && <p className="text-xs text-lunar-critical">{message.error}</p>}
      </div>
    </article>
  );
};

interface ComposerProps {
  draft: string;
  setDraft: (value: string) => void;
  onSend: () => void;
  onStop: () => void;
  onKeyDown: (event: React.KeyboardEvent<HTMLTextAreaElement>) => void;
  busy: boolean;
}

const Composer: React.FC<ComposerProps> = ({ draft, setDraft, onSend, onStop, onKeyDown, busy }) => (
  <div className="rounded-2xl border border-lunar-border bg-lunar-surface shadow-[0_8px_30px_rgba(0,0,0,0.16)] focus-within:border-lunar-muted/70 transition-colors">
    <textarea
      value={draft}
      onChange={(event) => setDraft(event.target.value)}
      onKeyDown={onKeyDown}
      placeholder={busy ? "Nyra is working… you can type your next message" : "Ask Nyra to check, clean or optimise your PC"}
      rows={2}
      aria-label="Message Nyra"
      className="w-full resize-none bg-transparent px-4 pt-3.5 pb-2 text-sm leading-6 text-lunar-text placeholder:text-lunar-muted outline-none select-text"
    />
    <div className="flex items-center justify-end gap-2 px-3 pb-2.5">
      <span className="text-[10px] text-lunar-muted">Enter to send · Shift+Enter for new line</span>
      {busy ? (
        <button
          type="button"
          onClick={onStop}
          className="w-7 h-7 rounded-lg flex items-center justify-center bg-lunar-elevated text-lunar-white hover:bg-lunar-surface-2"
          aria-label="Stop"
          title="Stop"
        >
          <Square className="w-3.5 h-3.5 fill-current" />
        </button>
      ) : (
        <button
          type="button"
          onClick={onSend}
          disabled={!draft.trim()}
          className="w-7 h-7 rounded-lg flex items-center justify-center bg-lunar-white text-lunar-bg hover:opacity-85 disabled:bg-lunar-elevated disabled:text-lunar-muted transition-colors"
          aria-label="Send message"
        >
          <ArrowUp className="w-4 h-4" />
        </button>
      )}
    </div>
  </div>
);
