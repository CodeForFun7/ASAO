import React, { useState, useRef, useEffect } from "react";
import { ArrowUp } from "lucide-react";
import type { ChatMessage, WidgetSystemUpdate } from "../../types/widget";

interface WidgetChatProps {
  telemetry: WidgetSystemUpdate | null;
  messages: ChatMessage[];
  isSending: boolean;
  onSendMessage: (text: string) => void;
}

const QUICK_PROMPTS = [
  "Why is my PC slow?",
  "Top memory processes",
  "Is system load normal?",
];

export const WidgetChat: React.FC<WidgetChatProps> = ({
  telemetry,
  messages,
  isSending,
  onSendMessage,
}) => {
  const [input, setInput] = useState("");
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages, isSending]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!input.trim() || isSending) return;
    const prompt = input;
    setInput("");
    onSendMessage(prompt);
  };

  return (
    <div className="flex-1 flex flex-col min-h-0 p-3 space-y-2.5 overflow-hidden">
      {/* Live System Context Strip */}
      <div className="px-2.5 py-1.5 rounded-md bg-lunar-bg/70 border border-lunar-border/80 flex items-center justify-between text-[11px] shrink-0">
        <span className="text-lunar-muted uppercase tracking-wider text-[10px]">
          System
        </span>
        <div className="flex items-center gap-3 text-lunar-text-sec">
          <span>
            CPU{" "}
            <strong className="text-lunar-white font-semibold">
              {telemetry ? `${telemetry.cpuUsage.toFixed(0)}%` : "—"}
            </strong>
          </span>
          <span>
            RAM{" "}
            <strong className="text-lunar-white font-semibold">
              {telemetry ? `${telemetry.memoryUsage.toFixed(0)}%` : "—"}
            </strong>
          </span>
          <span>
            <strong className="text-lunar-white font-semibold">
              {telemetry?.processCount ?? "—"}
            </strong>{" "}
            procs
          </span>
        </div>
      </div>

      {/* Conversation History */}
      <div
        ref={scrollRef}
        className="flex-1 overflow-y-auto space-y-2.5 pr-1 min-h-0"
      >
        {messages.map((msg) => (
          <div
            key={msg.id}
            className={`rounded-lg p-2.5 text-xs leading-relaxed border ${
              msg.sender === "user"
                ? "bg-lunar-elevated/80 border-lunar-border text-lunar-white ml-4"
                : "bg-lunar-bg/70 border-lunar-border/70 text-lunar-text-sec mr-2"
            }`}
          >
            <div className="text-[10px] font-semibold uppercase tracking-wider mb-1 text-lunar-muted">
              {msg.sender === "user" ? "You" : "Asao"}
            </div>
            <div className="whitespace-pre-line">{msg.text}</div>
          </div>
        ))}

        {isSending && (
          <div className="rounded-lg p-2.5 text-xs bg-lunar-bg/70 border border-lunar-border/70 text-lunar-muted">
            I&apos;m checking your current system state and running processes...
          </div>
        )}
      </div>

      {/* Quick Prompts */}
      <div className="flex items-center gap-1.5 overflow-x-auto pb-0.5 shrink-0">
        {QUICK_PROMPTS.map((q) => (
          <button
            key={q}
            type="button"
            onClick={() => onSendMessage(q)}
            disabled={isSending}
            className="px-2 py-1 rounded bg-lunar-surface hover:bg-lunar-elevated border border-lunar-border text-[10px] text-lunar-text-sec hover:text-lunar-white whitespace-nowrap transition-colors cursor-pointer"
          >
            {q}
          </button>
        ))}
      </div>

      {/* Input Box */}
      <form onSubmit={handleSubmit} className="relative shrink-0">
        <input
          type="text"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          placeholder="Ask Asao..."
          className="w-full h-8 pl-3 pr-8 rounded-md bg-lunar-bg border border-lunar-border text-xs text-lunar-text placeholder:text-lunar-muted focus:outline-none focus:border-lunar-text-sec"
        />
        <button
          type="submit"
          disabled={!input.trim() || isSending}
          className="absolute right-1.5 top-1/2 -translate-y-1/2 w-5 h-5 rounded bg-lunar-elevated hover:bg-lunar-border text-lunar-white flex items-center justify-center disabled:opacity-40 cursor-pointer"
          title="Send"
        >
          <ArrowUp className="w-3 h-3" />
        </button>
      </form>
    </div>
  );
};
