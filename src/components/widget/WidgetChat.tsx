import React, { useState, useRef, useEffect } from "react";
import { ArrowUp } from "lucide-react";
import type { ChatMessage, WidgetSystemUpdate } from "../../types/widget";
import { useAgentStore } from "../../stores/agent-store";
import { useStartupStore } from "../../stores/startup-store";
import { useStorageStore } from "../../stores/storage-store";
import { ChatMessageItem } from "../assistant/ChatMessageItem";

interface WidgetChatProps {
  telemetry: WidgetSystemUpdate | null;
  messages?: ChatMessage[];
  isSending?: boolean;
  onSendMessage?: (text: string) => void;
}

const QUICK_PROMPTS = [
  "Why is my PC slow?",
  "Analyze my startup",
  "What's using my RAM?",
  "What's taking up my storage?",
  "How can I improve boot time?",
];

export const WidgetChat: React.FC<WidgetChatProps> = ({ telemetry }) => {
  const [input, setInput] = useState("");
  const scrollRef = useRef<HTMLDivElement>(null);

  const agentMessages = useAgentStore((s) => s.messages);
  const isProcessing = useAgentStore((s) => s.isProcessing);
  const sendMessage = useAgentStore((s) => s.sendMessage);
  const applyFix = useAgentStore((s) => s.applyFix);

  const loadStartupData = useStartupStore((s) => s.loadStartupData);
  const startupItems = useStartupStore((s) => s.items);
  const initializeStorage = useStorageStore((s) => s.initializeStorage);
  const storageDrives = useStorageStore((s) => s.drives);

  useEffect(() => {
    if (startupItems.length === 0) void loadStartupData();
    if (storageDrives.length === 0) void initializeStorage();
  }, [
    startupItems.length,
    storageDrives.length,
    loadStartupData,
    initializeStorage,
  ]);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [agentMessages, isProcessing]);

  const handleSend = (textToSend?: string) => {
    const prompt = (textToSend ?? input).trim();
    if (!prompt || isProcessing) return;
    if (!textToSend) {
      setInput("");
    }
    void sendMessage(prompt);
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    handleSend();
  };

  return (
    <div className="flex-1 flex flex-col min-h-0 p-3 space-y-2.5 overflow-hidden">
      {/* Live System Context Strip */}
      <div className="px-2.5 py-1.5 rounded-md bg-lunar-surface/55 backdrop-blur-sm border border-lunar-border/80 flex items-center justify-between text-[11px] shrink-0">
        <span className="text-lunar-muted uppercase tracking-wider text-[10px]">
          System
        </span>
        <div className="flex items-center gap-3.5 text-lunar-text-sec">
          <span className="inline-flex items-center gap-1.5">
            <span>CPU</span>
            <strong className="text-lunar-white font-semibold">
              {telemetry ? `${telemetry.cpuUsage.toFixed(0)}%` : "—"}
            </strong>
          </span>
          <span className="inline-flex items-center gap-1.5">
            <span>RAM</span>
            <strong className="text-lunar-white font-semibold">
              {telemetry ? `${telemetry.memoryUsage.toFixed(0)}%` : "—"}
            </strong>
          </span>
          <span className="inline-flex items-center gap-1.5">
            <strong className="text-lunar-white font-semibold">
              {telemetry?.processCount ?? "—"}
            </strong>
            <span>processes</span>
          </span>
        </div>
      </div>

      {/* Conversation History (AI Diagnostics Engine) */}
      <div
        ref={scrollRef}
        className="flex-1 overflow-y-auto space-y-2.5 pr-1 min-h-0"
      >
        {agentMessages.length === 0 ? (
          <div className="rounded-lg p-3 text-xs leading-relaxed bg-lunar-surface/55 backdrop-blur-sm border border-lunar-border/80 text-lunar-text-sec">
            <div className="text-[10px] font-semibold uppercase tracking-wider mb-1 text-lunar-muted">
              Asao Diagnostics
            </div>
            <div>
              Monitoring your system in real time. Ask me about running
              processes, startup delays, memory usage, or storage capacity for a
              live multi-point diagnosis and 1-click fixes.
            </div>
          </div>
        ) : (
          agentMessages.map((msg) => (
            <ChatMessageItem
              key={msg.id}
              message={msg}
              onApplyFix={(fixId, msgId) => void applyFix(fixId, msgId)}
            />
          ))
        )}
      </div>

      {/* Quick Prompts */}
      <div className="flex items-center gap-1.5 overflow-x-auto pb-0.5 shrink-0">
        {QUICK_PROMPTS.map((q) => (
          <button
            key={q}
            type="button"
            onClick={() => handleSend(q)}
            disabled={isProcessing}
            className="px-2 py-1 rounded bg-lunar-surface/55 hover:bg-lunar-elevated/70 backdrop-blur-sm border border-lunar-border/80 text-[10px] text-lunar-text-sec hover:text-lunar-white whitespace-nowrap transition-colors disabled:opacity-40 cursor-pointer"
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
          disabled={isProcessing}
          className="w-full h-8 pl-3 pr-8 rounded-md bg-lunar-surface/55 backdrop-blur-sm border border-lunar-border/80 text-xs text-lunar-text placeholder:text-lunar-muted focus:outline-none focus:border-lunar-text-sec disabled:opacity-50"
        />
        <button
          type="submit"
          disabled={!input.trim() || isProcessing}
          className="absolute right-1.5 top-1/2 -translate-y-1/2 w-5 h-5 rounded bg-lunar-elevated hover:bg-lunar-border text-lunar-white flex items-center justify-center disabled:opacity-40 cursor-pointer"
          title="Send"
        >
          <ArrowUp className="w-3 h-3" />
        </button>
      </form>
    </div>
  );
};
