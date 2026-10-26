import React, { useState } from "react";
import {
  ArrowUp,
  ChevronDown,
  CirclePlus,
  Paperclip,
  Settings2,
  Sparkles,
} from "lucide-react";
import { sendNyraMessage } from "../services/nyra";

interface ChatMessage {
  id: number;
  role: "user" | "assistant";
  content: string;
}

const suggestions = [
  { title: "Check my system", detail: "Get an overview of system health" },
  { title: "Find what is slowing things down", detail: "Review processes and resource use" },
  { title: "Help me clean up storage", detail: "Explore space usage on this PC" },
];

export const NyraChat: React.FC = () => {
  const [draft, setDraft] = useState("");
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [apiKey, setApiKey] = useState(() => localStorage.getItem("nyra_openai_api_key") ?? "");
  const [showKeySettings, setShowKeySettings] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  const sendMessage = async () => {
    const content = draft.trim();
    if (!content || busy) return;
    if (!apiKey) { setShowKeySettings(true); return; }
    const next = [...messages, { id: Date.now(), role: "user" as const, content }];
    setMessages(next);
    setDraft("");
    setError("");
    setBusy(true);
    try {
      const reply = await sendNyraMessage(apiKey, next.map(({ role, content: text }) => ({ role, content: text })));
      setMessages([...next, { id: Date.now() + 1, role: "assistant", content: reply }]);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Nyra could not complete the request.");
    } finally { setBusy(false); }
  };

  const handleComposerKeyDown = (event: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      sendMessage();
    }
  };

  return (
    <section className="flex-1 min-h-0 flex flex-col bg-lunar-bg text-lunar-text">
      <header className="h-12 shrink-0 px-5 flex items-center justify-between border-b border-lunar-border/70">
        <div className="flex items-center gap-1">
        <button type="button" onClick={() => setShowKeySettings(true)} className="w-8 h-8 flex items-center justify-center rounded-md text-lunar-text-sec hover:bg-lunar-surface-2" title="OpenAI API key settings" aria-label="OpenAI API key settings"><Settings2 className="w-4 h-4" /></button>
        <button
          type="button"
          className="inline-flex items-center gap-1.5 rounded-md px-2 py-1.5 text-sm font-medium text-lunar-white hover:bg-lunar-surface-2 transition-colors"
          aria-label="Nyra assistant"
        >
          Nyra <ChevronDown className="w-3.5 h-3.5 text-lunar-muted" />
        </button>
        <button
          type="button"
          onClick={() => setMessages([])}
          className="inline-flex items-center gap-2 rounded-md px-2.5 py-1.5 text-xs text-lunar-text-sec hover:text-lunar-white hover:bg-lunar-surface-2 transition-colors"
          title="Start a new chat"
        >
          <CirclePlus className="w-3.5 h-3.5" />
          New chat
        </button>
        </div>
      </header>

      {showKeySettings && <div className="absolute inset-0 z-10 bg-black/50 flex items-center justify-center p-5"><div className="w-full max-w-md rounded-2xl border border-lunar-border bg-lunar-surface p-5 shadow-2xl"><h2 className="text-sm font-semibold">Connect Nyra</h2><p className="text-xs text-lunar-text-sec mt-1">Add an OpenAI API key to enable chat. It is saved in this app on this device.</p><input type="password" value={apiKey} onChange={event => setApiKey(event.target.value)} placeholder="OpenAI API key" className="mt-4 w-full rounded-lg border border-lunar-border bg-lunar-bg px-3 py-2 text-sm outline-none focus:border-lunar-muted"/><div className="flex justify-between mt-4"><button type="button" onClick={() => { setApiKey(""); localStorage.removeItem("nyra_openai_api_key"); }} className="text-xs text-lunar-text-sec">Remove key</button><div className="flex gap-2"><button type="button" onClick={() => setShowKeySettings(false)} className="rounded-lg px-3 py-2 text-xs text-lunar-text-sec hover:bg-lunar-surface-2">Cancel</button><button type="button" onClick={() => { localStorage.setItem("nyra_openai_api_key", apiKey.trim()); setApiKey(apiKey.trim()); setShowKeySettings(false); }} className="rounded-lg bg-lunar-white px-3 py-2 text-xs text-lunar-bg">Save key</button></div></div></div></div>}

      <div className="flex-1 min-h-0 overflow-y-auto">
        {messages.length === 0 ? (
          <div className="min-h-full flex flex-col items-center justify-center px-6 py-10">
            <div className="w-full max-w-[680px]">
              <div className="flex items-center gap-3 mb-7">
                <div className="w-9 h-9 rounded-xl border border-lunar-border bg-lunar-surface-2 flex items-center justify-center">
                  <Sparkles className="w-4 h-4 text-lunar-ai" />
                </div>
                <div>
                  <h1 className="text-lg font-medium tracking-tight text-lunar-white">What can I help with?</h1>
                  <p className="text-xs text-lunar-muted mt-0.5">Ask Nyra about your PC</p>
                </div>
              </div>

              <Composer
                draft={draft}
                setDraft={setDraft}
                onSend={sendMessage}
                onKeyDown={handleComposerKeyDown}
              />

              <div className="grid grid-cols-1 sm:grid-cols-3 gap-2 mt-3">
                {suggestions.map((suggestion) => (
                  <button
                    key={suggestion.title}
                    type="button"
                    onClick={() => setDraft(suggestion.title)}
                    className="text-left rounded-xl border border-lunar-border/80 bg-lunar-surface/60 hover:bg-lunar-surface-2 px-3.5 py-3 transition-colors"
                  >
                    <span className="block text-xs font-medium text-lunar-text">{suggestion.title}</span>
                    <span className="block text-[11px] leading-relaxed text-lunar-muted mt-1">{suggestion.detail}</span>
                  </button>
                ))}
              </div>
              {error && <p className="mt-3 text-center text-xs text-lunar-critical">{error}</p>}
              {!apiKey && <p className="mt-5 text-center text-[11px] text-lunar-muted">Add an OpenAI API key to start chatting.</p>}
            </div>
          </div>
        ) : (
          <div className="max-w-[680px] mx-auto px-6 py-8 space-y-7">
            {messages.map((message) => (
              <article key={message.id} className="flex gap-3">
                <div className={`w-7 h-7 shrink-0 rounded-lg flex items-center justify-center ${message.role === "assistant" ? "bg-lunar-surface-2 border border-lunar-border" : "bg-lunar-elevated"}`}>
                  {message.role === "assistant" ? <Sparkles className="w-3.5 h-3.5 text-lunar-ai" /> : <span className="text-[10px] font-semibold text-lunar-text-sec">You</span>}
                </div>
                <p className="pt-1 text-sm leading-6 text-lunar-text">{message.content}</p>
              </article>
            ))}
            {busy && <p className="text-xs text-lunar-muted pl-10">Nyra is thinking…</p>}
            {error && <p className="text-xs text-lunar-critical pl-10">{error}</p>}
          </div>
        )}
      </div>

      {messages.length > 0 && (
        <div className="px-6 pb-5 pt-2">
          <div className="max-w-[680px] mx-auto">
            <Composer
              draft={draft}
              setDraft={setDraft}
              onSend={sendMessage}
              onKeyDown={handleComposerKeyDown}
            />
            <p className="text-center text-[10px] text-lunar-muted mt-2">Nyra · Powered by OpenAI</p>
          </div>
        </div>
      )}
    </section>
  );
};

interface ComposerProps {
  draft: string;
  setDraft: (value: string) => void;
  onSend: () => void;
  onKeyDown: (event: React.KeyboardEvent<HTMLTextAreaElement>) => void;
}

const Composer: React.FC<ComposerProps> = ({ draft, setDraft, onSend, onKeyDown }) => (
  <div className="rounded-2xl border border-lunar-border bg-lunar-surface shadow-[0_8px_30px_rgba(0,0,0,0.16)] focus-within:border-lunar-muted/70 transition-colors">
    <textarea
      value={draft}
      onChange={(event) => setDraft(event.target.value)}
      onKeyDown={onKeyDown}
      placeholder="Message Nyra"
      rows={2}
      aria-label="Message Nyra"
      className="w-full resize-none bg-transparent px-4 pt-3.5 pb-2 text-sm leading-6 text-lunar-text placeholder:text-lunar-muted outline-none"
    />
    <div className="flex items-center justify-between px-3 pb-2.5">
      <button
        type="button"
        className="w-7 h-7 rounded-lg flex items-center justify-center text-lunar-muted hover:text-lunar-text-sec hover:bg-lunar-surface-2 transition-colors"
        title="Attachments coming soon"
        aria-label="Attachments coming soon"
      >
        <Paperclip className="w-4 h-4" />
      </button>
      <div className="flex items-center gap-2">
        <span className="text-[10px] text-lunar-muted">Enter to send · Shift+Enter for new line</span>
        <button
          type="button"
          onClick={onSend}
          disabled={!draft.trim()}
          className="w-7 h-7 rounded-lg flex items-center justify-center bg-lunar-white text-lunar-bg hover:opacity-85 disabled:bg-lunar-elevated disabled:text-lunar-muted transition-colors"
          aria-label="Send message"
        >
          <ArrowUp className="w-4 h-4" />
        </button>
      </div>
    </div>
  </div>
);
