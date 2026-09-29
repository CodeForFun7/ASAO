import React, { useState, useRef, useEffect, useCallback } from "react";
import {
  ArrowUp,
  ArrowDown,
  Cpu,
  Rocket,
  HardDrive,
  Zap,
} from "lucide-react";
import { useAgentStore } from "../stores/agent-store";
import { useStartupStore } from "../stores/startup-store";
import { useStorageStore } from "../stores/storage-store";
import { ChatMessageItem } from "../components/assistant/ChatMessageItem";

const SUGGESTED_CHIPS = [
  "Why is my PC slow?",
  "Analyze my startup",
  "What's using my RAM?",
  "What's taking up my storage?",
  "How can I improve boot time?",
];

const STARTER_CARDS = [
  {
    icon: <Zap className="w-4 h-4 text-white" />,
    title: "System Performance Audit",
    prompt: "Why is my PC slow?",
    description: "Check processor strain, background load, and disk headroom.",
  },
  {
    icon: <Rocket className="w-4 h-4 text-white" />,
    title: "Startup Boot-Time Analysis",
    prompt: "Which startups are causing boot-time delay on my system?",
    description: "Rank autostart apps by delay milliseconds and safe disable options.",
  },
  {
    icon: <Cpu className="w-4 h-4 text-white" />,
    title: "Memory & CPU Breakdown",
    prompt: "Which background processes are consuming memory or CPU?",
    description: "Identify background tasks consuming sustained system resources.",
  },
  {
    icon: <HardDrive className="w-4 h-4 text-white" />,
    title: "Storage Space & Cache",
    prompt: "What is taking up space on my C drive?",
    description: "Inspect drive capacity, large files, and temporary caches.",
  },
];

export const AssistantPage: React.FC = () => {
  const [input, setInput] = useState("");
  const [showScrollBottom, setShowScrollBottom] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const messages = useAgentStore((s) => s.messages);
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
  }, [startupItems.length, storageDrives.length, loadStartupData, initializeStorage]);

  // Handle scroll detection for the floating bottom button
  const handleScroll = useCallback(() => {
    if (!scrollRef.current) return;
    const { scrollTop, scrollHeight, clientHeight } = scrollRef.current;
    const distanceFromBottom = scrollHeight - scrollTop - clientHeight;
    setShowScrollBottom(distanceFromBottom > 160);
  }, []);

  // Auto-scroll on new message or live streaming updates
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages, isProcessing]);

  const scrollToBottom = () => {
    if (scrollRef.current) {
      scrollRef.current.scrollTo({
        top: scrollRef.current.scrollHeight,
        behavior: "smooth",
      });
    }
  };

  const handleSend = (textToSend?: string) => {
    const text = (textToSend || input).trim();
    if (!text || isProcessing) return;
    setInput("");
    void sendMessage(text);
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    handleSend();
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const isStarterState = messages.length <= 1;

  return (
    <div className="flex-1 flex flex-col h-full bg-[#0c0c0e] overflow-hidden select-none">
      {/* Main Conversation Stream */}
      <div
        ref={scrollRef}
        onScroll={handleScroll}
        className="flex-1 overflow-y-auto px-4 sm:px-6 py-6 min-h-0"
      >
        <div className="max-w-3xl mx-auto space-y-6">
          {messages.map((msg) => (
            <ChatMessageItem
              key={msg.id}
              message={msg}
              onApplyFix={applyFix}
            />
          ))}

          {/* Clean Starter State (Shown when chat has only the welcome message) */}
          {isStarterState && !isProcessing && (
            <div className="pt-8 pb-4">
              <div className="text-center mb-6">
                <h2 className="text-lg font-medium text-white tracking-tight">
                  What would you like to analyze?
                </h2>
                <p className="text-xs text-zinc-400 mt-1">
                  Select a diagnostic workflow or ask anything below.
                </p>
              </div>

              <div className="grid grid-cols-1 sm:grid-cols-2 gap-2.5">
                {STARTER_CARDS.map((card, idx) => (
                  <div
                    key={idx}
                    onClick={() => handleSend(card.prompt)}
                    className="p-3.5 rounded-2xl bg-white/[0.03] hover:bg-white/[0.06] border border-white/[0.07] hover:border-white/20 transition-all cursor-pointer group select-none"
                  >
                    <div className="flex items-center gap-2.5 mb-1.5">
                      <div className="p-1 rounded-md bg-white/[0.05] border border-white/10 text-white">
                        {card.icon}
                      </div>
                      <span className="font-semibold text-xs text-white group-hover:text-zinc-200 transition-colors">
                        {card.title}
                      </span>
                    </div>
                    <p className="text-[11.5px] text-zinc-400 leading-relaxed pl-0.5">
                      {card.description}
                    </p>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Processing Indicator */}
          {isProcessing && messages[messages.length - 1]?.sender === "user" && (
            <div className="flex items-center gap-2.5 text-xs text-zinc-400 py-3">
              <span className="relative flex h-2 w-2">
                <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-white opacity-75" />
                <span className="relative inline-flex rounded-full h-2 w-2 bg-white" />
              </span>
              <span className="animate-pulse">
                Analyzing system data &amp; calculating recommendations...
              </span>
            </div>
          )}
        </div>
      </div>

      {/* Floating Bottom Input Area (ChatGPT Style Capsule) */}
      <div className="relative p-4 shrink-0 bg-gradient-to-t from-[#0c0c0e] via-[#0c0c0e]/95 to-transparent">
        {/* Floating Scroll To Bottom Button */}
        {showScrollBottom && (
          <button
            type="button"
            onClick={scrollToBottom}
            className="absolute -top-6 left-1/2 -translate-x-1/2 w-7 h-7 rounded-full bg-[#262626] border border-white/[0.15] text-white flex items-center justify-center shadow-lg hover:bg-[#333] transition-all cursor-pointer z-10"
            title="Scroll to bottom"
          >
            <ArrowDown className="w-3.5 h-3.5" />
          </button>
        )}

        <div className="max-w-3xl mx-auto space-y-2.5">
          {/* Quick Prompt Suggestion Chips */}
          <div className="flex items-center gap-1.5 overflow-x-auto pb-0.5 scrollbar-none">
            {SUGGESTED_CHIPS.map((chip) => (
              <button
                key={chip}
                type="button"
                disabled={isProcessing}
                onClick={() => handleSend(chip)}
                className="px-3 py-1 rounded-full bg-white/[0.03] hover:bg-white/[0.08] border border-white/[0.07] text-[11px] text-zinc-400 hover:text-white whitespace-nowrap transition-all cursor-pointer disabled:opacity-30"
              >
                {chip}
              </button>
            ))}
          </div>

          {/* Capsule Composer */}
          <form
            onSubmit={handleSubmit}
            className="relative rounded-3xl bg-[#1e1e1e] border border-white/[0.12] p-1.5 shadow-2xl flex items-center gap-2 focus-within:border-white/30 transition-all"
          >
            {/* Input field */}
            <input
              ref={inputRef}
              type="text"
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              disabled={isProcessing}
              placeholder={
                isProcessing
                  ? "Analyzing system data..."
                  : "Ask anything about slow PC, boot delay, RAM, storage..."
              }
              className="w-full bg-transparent text-[13px] text-white placeholder:text-zinc-500 focus:outline-none pl-4 pr-2 py-1"
            />

            {/* Send Button (White circle with dark up arrow) */}
            <button
              type="submit"
              disabled={!input.trim() || isProcessing}
              className="w-8 h-8 rounded-full bg-white text-black flex items-center justify-center hover:bg-zinc-200 transition-all disabled:opacity-20 disabled:hover:bg-white shrink-0 cursor-pointer shadow-sm mr-0.5 active:scale-95"
              title="Send Message"
            >
              <ArrowUp className="w-4 h-4 text-black stroke-[2.5]" />
            </button>
          </form>
        </div>
      </div>
    </div>
  );
};
