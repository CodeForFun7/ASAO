import React, { useEffect, useMemo, useState } from "react";
import {
  AlertTriangle,
  Ban,
  Bot,
  Check,
  ChevronDown,
  ChevronRight,
  Circle,
  CircleDot,
  Hand,
  Loader2,
  MessageSquareText,
  Wrench,
  X,
} from "lucide-react";
import type { ActivityNode, AgentRun, ConfirmationCard, PlanStep } from "../../types/nyra";

interface ActivityFeedProps {
  run: AgentRun;
  live: boolean;
  onConfirm?: (activityId: string, confirmationId: string, approved: boolean) => void;
}

function duration(ms: number): string {
  if (ms < 1000) return `${ms} ms`;
  const s = ms / 1000;
  if (s < 60) return `${s.toFixed(s < 10 ? 1 : 0)} s`;
  return `${Math.floor(s / 60)} min ${Math.round(s % 60)} s`;
}

function useNow(active: boolean): number {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    if (!active) return;
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, [active]);
  return now;
}

const StatusIcon: React.FC<{ status: ActivityNode["status"] }> = ({ status }) => {
  switch (status) {
    case "running":
      return <Loader2 className="w-3.5 h-3.5 animate-spin text-lunar-ai" />;
    case "waiting":
      return <Hand className="w-3.5 h-3.5 text-lunar-warning" />;
    case "done":
      return <Check className="w-3.5 h-3.5 text-lunar-healthy" />;
    case "error":
      return <AlertTriangle className="w-3.5 h-3.5 text-lunar-critical" />;
    case "denied":
      return <Ban className="w-3.5 h-3.5 text-lunar-text-sec" />;
    case "cancelled":
      return <X className="w-3.5 h-3.5 text-lunar-muted" />;
  }
};

const PlanView: React.FC<{ plan: PlanStep[] }> = ({ plan }) => {
  if (plan.length === 0) return null;
  const done = plan.filter((p) => p.status === "done").length;
  return (
    <div className="rounded-xl border border-lunar-border bg-lunar-surface/70 px-3.5 py-3">
      <div className="flex items-center justify-between mb-2">
        <span className="text-[11px] font-medium uppercase tracking-wide text-lunar-text-sec">Plan</span>
        <span className="text-[11px] text-lunar-muted">
          {done} of {plan.length} done
        </span>
      </div>
      <ol className="space-y-1.5">
        {plan.map((step) => (
          <li key={step.id} className="flex items-start gap-2 text-xs">
            <span className="mt-0.5">
              {step.status === "done" ? (
                <Check className="w-3.5 h-3.5 text-lunar-healthy" />
              ) : step.status === "in_progress" ? (
                <Loader2 className="w-3.5 h-3.5 animate-spin text-lunar-ai" />
              ) : step.status === "failed" ? (
                <AlertTriangle className="w-3.5 h-3.5 text-lunar-critical" />
              ) : step.status === "skipped" ? (
                <X className="w-3.5 h-3.5 text-lunar-muted" />
              ) : (
                <Circle className="w-3.5 h-3.5 text-lunar-muted" />
              )}
            </span>
            <span className={step.status === "done" || step.status === "skipped" ? "text-lunar-text-sec" : "text-lunar-text"}>
              {step.title}
              {step.owner && <span className="text-lunar-muted"> · {step.owner}</span>}
            </span>
          </li>
        ))}
      </ol>
    </div>
  );
};

const ConfirmationView: React.FC<{
  node: ActivityNode;
  card: ConfirmationCard;
  onConfirm?: ActivityFeedProps["onConfirm"];
}> = ({ node, card, onConfirm }) => {
  const [showAll, setShowAll] = useState(false);
  const pending = card.status === "pending";
  const items = showAll ? card.items : card.items.slice(0, 8);
  const high = card.danger === "high";
  return (
    <div
      className={`mt-2 rounded-xl border px-3.5 py-3 ${
        pending ? (high ? "border-lunar-critical/50 bg-lunar-critical/5" : "border-lunar-warning/40 bg-lunar-warning/5") : "border-lunar-border bg-lunar-surface/60"
      }`}
    >
      <p className="text-xs font-semibold text-lunar-white">{card.title}</p>
      {card.message && <p className="text-xs text-lunar-text-sec mt-1 leading-5">{card.message}</p>}
      {card.items.length > 0 && (
        <ul className="mt-2 max-h-56 overflow-y-auto rounded-lg border border-lunar-border/70 bg-lunar-bg/60 divide-y divide-lunar-border/50 select-text [&_*]:select-text">
          {items.map((item, i) => (
            <li key={i} className="flex items-start justify-between gap-3 px-2.5 py-1.5 text-[11px]">
              <span className="text-lunar-text break-all">{item.label}</span>
              {item.detail && <span className="text-lunar-muted shrink-0">{item.detail}</span>}
            </li>
          ))}
        </ul>
      )}
      {(card.items.length > 8 || card.totalItems > card.items.length) && (
        <button
          type="button"
          onClick={() => setShowAll((v) => !v)}
          className="mt-1.5 text-[11px] text-lunar-text-sec hover:text-lunar-white"
        >
          {showAll ? "Show fewer" : `Show all ${card.items.length}`}
          {card.totalItems > card.items.length && ` (of ${card.totalItems} in total)`}
        </button>
      )}
      <div className="flex items-center justify-end gap-2 mt-3">
        {pending ? (
          <>
            <button
              type="button"
              disabled={!onConfirm}
              onClick={() => onConfirm?.(node.id, card.id, false)}
              className="rounded-lg px-3 py-1.5 text-xs text-lunar-text-sec hover:bg-lunar-surface-2 hover:text-lunar-white"
            >
              Cancel
            </button>
            <button
              type="button"
              disabled={!onConfirm}
              onClick={() => onConfirm?.(node.id, card.id, true)}
              className={`rounded-lg px-3 py-1.5 text-xs font-medium ${
                high ? "bg-lunar-critical text-white hover:opacity-90" : "bg-lunar-white text-lunar-bg hover:opacity-85"
              }`}
            >
              {card.confirmLabel}
            </button>
          </>
        ) : (
          <span className={`text-[11px] ${card.status === "approved" ? "text-lunar-healthy" : "text-lunar-text-sec"}`}>
            {card.status === "approved" ? "You approved this" : "You declined this"}
          </span>
        )}
      </div>
    </div>
  );
};

const NodeView: React.FC<{
  node: ActivityNode;
  childrenOf: Map<string | null, ActivityNode[]>;
  now: number;
  onConfirm?: ActivityFeedProps["onConfirm"];
}> = ({ node, childrenOf, now, onConfirm }) => {
  const kids = childrenOf.get(node.id) ?? [];
  const active = node.status === "running" || node.status === "waiting";
  const [open, setOpen] = useState(false);
  const [kidsOpen, setKidsOpen] = useState(true);

  if (node.kind === "note") {
    return (
      <div className="flex items-start gap-2 py-1 text-xs text-lunar-text-sec">
        <MessageSquareText className="w-3.5 h-3.5 mt-0.5 shrink-0 text-lunar-muted" />
        <span className="leading-5 select-text">{node.label}</span>
      </div>
    );
  }

  const elapsed = (node.endedAt ?? now) - node.startedAt;
  const lastProgress = node.progress[node.progress.length - 1];
  const KindIcon = node.kind === "agent" ? Bot : Wrench;

  return (
    <div className="py-0.5">
      <div className="flex items-start gap-2">
        <button
          type="button"
          onClick={() => setOpen((v) => !v)}
          className="mt-0.5 text-lunar-muted hover:text-lunar-text"
          aria-label={open ? "Hide details" : "Show details"}
        >
          {open ? <ChevronDown className="w-3.5 h-3.5" /> : <ChevronRight className="w-3.5 h-3.5" />}
        </button>
        <span className="mt-0.5">
          <StatusIcon status={node.status} />
        </span>
        <div className="min-w-0 flex-1">
          <div className="flex items-baseline gap-2">
            <KindIcon className={`w-3 h-3 shrink-0 translate-y-0.5 ${node.kind === "agent" ? "text-lunar-ai" : "text-lunar-muted"}`} />
            <span className={`text-xs ${node.kind === "agent" ? "font-medium text-lunar-white" : "text-lunar-text"} break-words`}>
              {node.label}
            </span>
            <span className="ml-auto shrink-0 text-[10px] text-lunar-muted tabular-nums">{duration(Math.max(0, elapsed))}</span>
          </div>
          {active && lastProgress && <p className="text-[11px] text-lunar-text-sec mt-0.5 truncate">{lastProgress}</p>}
          {node.status === "waiting" && <p className="text-[11px] text-lunar-warning mt-0.5">Waiting for your answer</p>}
          {!active && node.summary && <p className="text-[11px] text-lunar-text-sec mt-0.5 break-words">{node.summary}</p>}
          {node.error && <p className="text-[11px] text-lunar-critical mt-0.5 break-words">{node.error}</p>}

          {open && (
            <div className="mt-1.5 rounded-lg border border-lunar-border/70 bg-lunar-bg/60 p-2 space-y-1.5 select-text [&_*]:select-text">
              <p className="text-[10px] text-lunar-muted font-mono">{node.name}</p>
              {node.args && Object.keys(node.args).length > 0 && (
                <pre className="text-[10px] text-lunar-text-sec whitespace-pre-wrap break-all font-mono max-h-40 overflow-y-auto">
                  {JSON.stringify(node.args, null, 2)}
                </pre>
              )}
              {node.progress.length > 0 && (
                <ul className="text-[10px] text-lunar-text-sec space-y-0.5 max-h-40 overflow-y-auto">
                  {node.progress.map((p, i) => (
                    <li key={i}>· {p}</li>
                  ))}
                </ul>
              )}
            </div>
          )}

          {node.confirmation && <ConfirmationView node={node} card={node.confirmation} onConfirm={onConfirm} />}

          {kids.length > 0 && (
            <div className="mt-1">
              <button
                type="button"
                onClick={() => setKidsOpen((v) => !v)}
                className="text-[10px] text-lunar-muted hover:text-lunar-text-sec"
              >
                {kidsOpen ? "Hide" : "Show"} {kids.filter((k) => k.kind !== "note").length} steps
              </button>
              {kidsOpen && (
                <div className="mt-0.5 ml-1 pl-3 border-l border-lunar-border">
                  {kids.map((k) => (
                    <NodeView key={k.id} node={k} childrenOf={childrenOf} now={now} onConfirm={onConfirm} />
                  ))}
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

/** Live view of everything the agent does in one turn: plan, agents, tools and confirmations. */
export const ActivityFeed: React.FC<ActivityFeedProps> = ({ run, live, onConfirm }) => {
  const now = useNow(live);
  const [expanded, setExpanded] = useState(live);
  useEffect(() => {
    if (live) setExpanded(true);
  }, [live]);

  const childrenOf = useMemo(() => {
    const ids = new Set(run.activities.map((a) => a.id));
    const map = new Map<string | null, ActivityNode[]>();
    for (const a of run.activities) {
      const parent = a.parentId && ids.has(a.parentId) ? a.parentId : null;
      map.set(parent, [...(map.get(parent) ?? []), a]);
    }
    return map;
  }, [run.activities]);

  const tools = run.activities.filter((a) => a.kind === "tool").length;
  const agents = run.activities.filter((a) => a.kind === "agent").length;
  const pending = run.activities.filter((a) => a.confirmation?.status === "pending");
  const running = run.activities.filter((a) => a.status === "running");
  const failed = run.activities.filter((a) => a.status === "error").length;

  if (run.activities.length === 0 && run.plan.length === 0) {
    return live ? (
      <div className="flex items-center gap-2 text-xs text-lunar-text-sec">
        <Loader2 className="w-3.5 h-3.5 animate-spin text-lunar-ai" /> Thinking…
      </div>
    ) : null;
  }

  const elapsed = (run.endedAt ?? now) - run.startedAt;
  const headline = live
    ? pending.length > 0
      ? "Waiting for your confirmation"
      : running.length > 0
        ? `Working on ${running.length} ${running.length === 1 ? "task" : "tasks"}`
        : "Thinking…"
    : [
        tools && `${tools} ${tools === 1 ? "tool" : "tools"}`,
        agents && `${agents} ${agents === 1 ? "agent" : "agents"}`,
        failed && `${failed} failed`,
      ]
        .filter(Boolean)
        .join(" · ") || "Activity";

  return (
    <div className="space-y-2">
      <PlanView plan={run.plan} />
      <div className="rounded-xl border border-lunar-border bg-lunar-surface/50">
        <button
          type="button"
          onClick={() => setExpanded((v) => !v)}
          className="w-full flex items-center gap-2 px-3 py-2 text-left"
        >
          {live ? (
            pending.length ? <Hand className="w-3.5 h-3.5 text-lunar-warning" /> : <CircleDot className="w-3.5 h-3.5 text-lunar-ai animate-pulse" />
          ) : (
            <Check className="w-3.5 h-3.5 text-lunar-muted" />
          )}
          <span className="text-xs text-lunar-text">{headline}</span>
          <span className="ml-auto text-[10px] text-lunar-muted tabular-nums">{duration(Math.max(0, elapsed))}</span>
          {expanded ? <ChevronDown className="w-3.5 h-3.5 text-lunar-muted" /> : <ChevronRight className="w-3.5 h-3.5 text-lunar-muted" />}
        </button>
        {expanded && (
          <div className="px-3 pb-2.5 border-t border-lunar-border/60 pt-2">
            {(childrenOf.get(null) ?? []).map((n) => (
              <NodeView key={n.id} node={n} childrenOf={childrenOf} now={now} onConfirm={live ? onConfirm : undefined} />
            ))}
          </div>
        )}
      </div>
      {!expanded &&
        pending.map((n) =>
          n.confirmation ? <ConfirmationView key={n.id} node={n} card={n.confirmation} onConfirm={live ? onConfirm : undefined} /> : null
        )}
    </div>
  );
};
