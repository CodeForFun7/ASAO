import React, { useState } from "react";
import { Check, Loader2, MessageSquare, Pencil, SquarePen, Trash2, X } from "lucide-react";
import type { ConversationSummary } from "../../types/nyra";

interface Props {
  conversations: ConversationSummary[];
  activeId: string | null;
  runningId: string | null;
  onOpen: (id: string) => void;
  onNew: () => void;
  onRename: (id: string, title: string) => void;
  onDelete: (id: string) => void;
}

function group(updatedAt: number): string {
  const days = Math.floor((Date.now() - updatedAt) / 86_400_000);
  if (days < 1) return "Today";
  if (days < 2) return "Yesterday";
  if (days < 7) return "Previous 7 days";
  if (days < 30) return "Previous 30 days";
  return "Older";
}

export const ConversationList: React.FC<Props> = ({ conversations, activeId, runningId, onOpen, onNew, onRename, onDelete }) => {
  const [editing, setEditing] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);

  const groups: { label: string; items: ConversationSummary[] }[] = [];
  for (const c of conversations) {
    const label = group(c.updatedAt);
    const last = groups[groups.length - 1];
    if (last?.label === label) last.items.push(c);
    else groups.push({ label, items: [c] });
  }

  return (
    <aside className="w-60 shrink-0 border-r border-lunar-border/70 bg-lunar-surface/40 flex flex-col min-h-0">
      <div className="p-2.5">
        <button
          type="button"
          onClick={onNew}
          className="w-full flex items-center gap-2 rounded-lg px-2.5 py-2 text-xs text-lunar-text hover:bg-lunar-surface-2 border border-lunar-border/70"
        >
          <SquarePen className="w-3.5 h-3.5 text-lunar-text-sec" />
          New chat
        </button>
      </div>
      <div className="flex-1 min-h-0 overflow-y-auto px-2 pb-3">
        {conversations.length === 0 && (
          <p className="px-2 py-6 text-center text-[11px] text-lunar-muted">Your conversations will appear here.</p>
        )}
        {groups.map((g) => (
          <div key={g.label} className="mt-2">
            <p className="px-2 py-1 text-[10px] uppercase tracking-wide text-lunar-muted">{g.label}</p>
            {g.items.map((c) => {
              const active = c.id === activeId;
              const running = c.id === runningId || c.running;
              if (editing === c.id) {
                return (
                  <form
                    key={c.id}
                    onSubmit={(e) => {
                      e.preventDefault();
                      onRename(c.id, draft);
                      setEditing(null);
                    }}
                    className="flex items-center gap-1 px-1.5 py-1"
                  >
                    <input
                      autoFocus
                      value={draft}
                      onChange={(e) => setDraft(e.target.value)}
                      onKeyDown={(e) => e.key === "Escape" && setEditing(null)}
                      className="flex-1 min-w-0 rounded-md border border-lunar-border bg-lunar-bg px-2 py-1 text-xs outline-none focus:border-lunar-muted"
                    />
                    <button type="submit" className="p-1 text-lunar-text-sec hover:text-lunar-white" aria-label="Save title">
                      <Check className="w-3.5 h-3.5" />
                    </button>
                  </form>
                );
              }
              return (
                <div
                  key={c.id}
                  className={`group flex items-center gap-2 rounded-lg px-2 py-1.5 cursor-pointer ${
                    active ? "bg-lunar-elevated text-lunar-white" : "text-lunar-text-sec hover:bg-lunar-surface-2 hover:text-lunar-text"
                  }`}
                  onClick={() => onOpen(c.id)}
                >
                  {running ? (
                    <Loader2 className="w-3.5 h-3.5 shrink-0 animate-spin text-lunar-ai" />
                  ) : (
                    <MessageSquare className="w-3.5 h-3.5 shrink-0 text-lunar-muted" />
                  )}
                  <span className="flex-1 min-w-0 truncate text-xs" title={c.title}>
                    {c.title}
                  </span>
                  {confirmDelete === c.id ? (
                    <span className="flex items-center gap-0.5" onClick={(e) => e.stopPropagation()}>
                      <button type="button" onClick={() => { onDelete(c.id); setConfirmDelete(null); }} className="p-0.5 text-lunar-critical" aria-label="Confirm delete">
                        <Check className="w-3.5 h-3.5" />
                      </button>
                      <button type="button" onClick={() => setConfirmDelete(null)} className="p-0.5 text-lunar-text-sec" aria-label="Keep conversation">
                        <X className="w-3.5 h-3.5" />
                      </button>
                    </span>
                  ) : (
                    <span className="hidden group-hover:flex items-center gap-0.5" onClick={(e) => e.stopPropagation()}>
                      <button
                        type="button"
                        onClick={() => { setEditing(c.id); setDraft(c.title); }}
                        className="p-0.5 text-lunar-muted hover:text-lunar-white"
                        aria-label="Rename conversation"
                      >
                        <Pencil className="w-3 h-3" />
                      </button>
                      <button
                        type="button"
                        disabled={running}
                        onClick={() => setConfirmDelete(c.id)}
                        className="p-0.5 text-lunar-muted hover:text-lunar-critical disabled:opacity-40"
                        aria-label="Delete conversation"
                      >
                        <Trash2 className="w-3 h-3" />
                      </button>
                    </span>
                  )}
                </div>
              );
            })}
          </div>
        ))}
      </div>
    </aside>
  );
};
