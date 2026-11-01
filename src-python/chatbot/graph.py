"""Nyra's LangGraph agents.

Architecture
- Orchestrator graph (checkpointed per conversation): an LLM node bound to every system
  tool, the four specialist agents (exposed as tools) and `update_plan`, plus a ToolNode.
  Simple requests are handled with the tools directly; broad ones are planned and
  delegated to specialists in parallel.
- Specialist graphs (Process & Startup, Storage, Applications, Cleanup): the same
  LLM-node + ToolNode loop with a focused prompt and tool subset. Each runs inside the
  `delegate_to_*` tool call that started it, so its activity nests under that call.
- ToolNode runs all tool calls from one model response concurrently, so independent
  tools and specialists work in parallel; dependent steps happen in later rounds.
"""

from dataclasses import dataclass
from typing import Any, Literal

from langchain_core.language_models import BaseChatModel
from langchain_core.messages import AIMessage, BaseMessage, HumanMessage, SystemMessage, ToolMessage
from langchain_core.tools import BaseTool
from langchain_openai import ChatOpenAI
from langgraph.graph import END, START, StateGraph
from langgraph.prebuilt import ToolNode
from pydantic import BaseModel, Field

from . import prompts
from .config import ORCHESTRATOR_MAX_ROUNDS, SPECIALIST_MAX_ROUNDS
from .formatting import shorten
from .run_context import RunContext, current_parent, current_run
from .state import ChatState
from .tools import APP_TOOLS, LEFTOVER_TOOLS, PROCESS_TOOLS, STARTUP_TOOLS, STORAGE_TOOLS, ALL_TOOLS
from .tools import storage as storage_tools
from .tools.base import Call, Result, nyra_tool

# Model input budget, in characters, before older tool traffic is dropped.
HISTORY_CHAR_BUDGET = 90_000


@dataclass
class LlmSettings:
    api_key: str
    model: str
    base_url: str | None = None


def build_model(settings: LlmSettings) -> ChatOpenAI:
    return ChatOpenAI(
        model=settings.model,
        api_key=settings.api_key,
        base_url=settings.base_url or None,
        max_retries=2,
        timeout=180,
    )


# ---------------------------------------------------------------------------
# Specialists
# ---------------------------------------------------------------------------


@dataclass
class Specialist:
    id: str
    name: str
    tool_name: str
    description: str
    prompt: str
    tools: list[BaseTool]


SPECIALISTS = [
    Specialist(
        "process_startup",
        "Process & Startup agent",
        "delegate_to_process_startup_agent",
        "Running processes, system load, resource-heavy or suspicious background activity, ending processes, and apps that start with Windows (inspect, recommend, disable, enable).",
        prompts.PROCESS_STARTUP_PROMPT,
        PROCESS_TOOLS + STARTUP_TOOLS,
    ),
    Specialist(
        "storage",
        "Storage Management agent",
        "delegate_to_storage_agent",
        "Drive and folder analysis, large/duplicate/old files, file search by name, type, size and date, and removing files or folders.",
        prompts.STORAGE_PROMPT,
        STORAGE_TOOLS,
    ),
    Specialist(
        "apps",
        "Application Management agent",
        "delegate_to_app_agent",
        "Unused and rarely used applications: last use, size, install location, and starting their removal.",
        prompts.APP_PROMPT,
        APP_TOOLS + [storage_tools.get_file_details, storage_tools.get_folder_details],
    ),
    Specialist(
        "cleanup",
        "Cleanup agent",
        "delegate_to_cleanup_agent",
        "Leftovers of uninstalled apps, temporary files, caches, crash dumps and the Recycle Bin, and cleaning them.",
        prompts.CLEANUP_PROMPT,
        LEFTOVER_TOOLS + storage_tools.TEMP_TOOLS + [storage_tools.get_file_details, storage_tools.remove_folder, storage_tools.remove_file],
    ),
]


# ---------------------------------------------------------------------------
# Message preparation
# ---------------------------------------------------------------------------


def text_of(message: BaseMessage) -> str:
    content = message.content
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        return "".join(part.get("text", "") if isinstance(part, dict) else str(part) for part in content)
    return str(content)


def _repair_tool_calls(messages: list[BaseMessage]) -> list[BaseMessage]:
    """Adds results for tool calls that never finished (the user stopped the run)."""
    out: list[BaseMessage] = []
    i = 0
    while i < len(messages):
        m = messages[i]
        out.append(m)
        i += 1
        if isinstance(m, AIMessage) and m.tool_calls:
            answered: set[str] = set()
            while i < len(messages) and isinstance(messages[i], ToolMessage):
                answered.add(messages[i].tool_call_id)
                out.append(messages[i])
                i += 1
            for tc in m.tool_calls:
                if tc["id"] not in answered:
                    out.append(ToolMessage(content='{"error": "Stopped before this tool finished."}', tool_call_id=tc["id"]))
        elif isinstance(m, ToolMessage):
            out.pop()  # a result not directly after its tool call is invalid input
    return out


def _size(messages: list[BaseMessage]) -> int:
    return sum(len(text_of(m)) + sum(len(str(tc.get("args"))) for tc in getattr(m, "tool_calls", []) or []) for m in messages)


def prepare_messages(messages: list[BaseMessage]) -> list[BaseMessage]:
    """Valid, bounded model input: recent turns in full, older turns as plain conversation."""
    msgs = _repair_tool_calls(list(messages))
    if _size(msgs) <= HISTORY_CHAR_BUDGET:
        return msgs
    human_idx = [i for i, m in enumerate(msgs) if isinstance(m, HumanMessage)]
    keep_from = human_idx[-2] if len(human_idx) >= 2 else 0
    older = [
        m for m in msgs[:keep_from]
        if isinstance(m, HumanMessage) or (isinstance(m, AIMessage) and not m.tool_calls and text_of(m).strip())
    ]
    msgs = older + msgs[keep_from:]
    while _size(msgs) > HISTORY_CHAR_BUDGET and len(human_idx := [i for i, m in enumerate(msgs) if isinstance(m, HumanMessage)]) > 1:
        msgs = msgs[human_idx[1]:]
    return msgs


def _rounds_this_turn(messages: list[BaseMessage]) -> int:
    rounds = 0
    for m in reversed(messages):
        if isinstance(m, HumanMessage):
            break
        if isinstance(m, AIMessage) and m.tool_calls:
            rounds += 1
    return rounds


# ---------------------------------------------------------------------------
# Graph builder shared by the orchestrator and specialists
# ---------------------------------------------------------------------------


async def _call_model(model: Any, messages: list[BaseMessage], stream_to: RunContext | None) -> AIMessage:
    if stream_to is None:
        response = await model.ainvoke(messages)
        return AIMessage(content=response.content, tool_calls=response.tool_calls, id=response.id)
    full = None
    async for chunk in model.astream(messages):
        stream_to.check_cancelled()
        delta = chunk.content if isinstance(chunk.content, str) else text_of(chunk)
        if delta:
            stream_to.append_text(delta)
        full = chunk if full is None else full + chunk
    if full is None:
        return AIMessage(content="")
    return AIMessage(content=text_of(full), tool_calls=full.tool_calls, id=full.id)


def build_agent_graph(
    model: BaseChatModel,
    tools: list[BaseTool],
    system_prompt: str,
    max_rounds: int,
    *,
    stream_answer: bool,
    checkpointer: Any = None,
):
    with_tools = model.bind_tools(tools, parallel_tool_calls=True)

    async def agent(state: ChatState) -> dict[str, list[BaseMessage]]:
        run = current_run.get()
        if run:
            run.check_cancelled()
        history = prepare_messages(state["messages"])
        chosen: Any = with_tools
        if _rounds_this_turn(state["messages"]) >= max_rounds:
            history = history + [
                HumanMessage(
                    "You have reached the step limit for this request. Without calling tools, report what you found, "
                    "what you completed and what is still left to do."
                )
            ]
            chosen = model
        response = await _call_model(chosen, [SystemMessage(system_prompt), *history], run if (stream_answer and run) else None)
        if response.tool_calls and run:
            if stream_answer:
                run.commit_interim_text()
            elif text_of(response).strip():
                parent = current_parent.get()
                if parent:
                    run.progress(parent, shorten(text_of(response).strip(), 300) or "")
        return {"messages": [response]}

    def route(state: ChatState) -> Literal["tools", "__end__"]:
        last = state["messages"][-1]
        return "tools" if isinstance(last, AIMessage) and last.tool_calls else END

    graph = StateGraph(ChatState)
    graph.add_node("agent", agent)
    graph.add_node("tools", ToolNode(tools, handle_tool_errors=True))
    graph.add_edge(START, "agent")
    graph.add_conditional_edges("agent", route, {"tools": "tools", END: END})
    graph.add_edge("tools", "agent")
    return graph.compile(checkpointer=checkpointer)


# ---------------------------------------------------------------------------
# Orchestrator-only tools
# ---------------------------------------------------------------------------


class DelegateArgs(BaseModel):
    task: str = Field(
        description="Complete, self-contained instructions: what to inspect or do, exact paths/ids, limits, and whether the user asked for changes or deletions."
    )


def delegate_tool(spec: Specialist, model: BaseChatModel) -> BaseTool:
    @nyra_tool(
        name=spec.tool_name,
        description=f"Delegate a task to the {spec.name}: {spec.description} Several delegations in one response run in parallel. Returns the agent's report.",
        args=DelegateArgs,
        kind="agent",
        label=lambda a: f"{spec.name}: {shorten(a.task, 140)}",
    )
    async def delegate(a: DelegateArgs, call: Call) -> Result:
        graph = build_agent_graph(model, spec.tools, spec.prompt, SPECIALIST_MAX_ROUNDS, stream_answer=False)
        run = current_run.get()
        task = f"Task from Nyra:\n{a.task}"
        if run:
            task += f"\n\nThe user's request, for intent only:\n{run.user_request}"
        token = current_parent.set(call.activity_id)
        try:
            result = await graph.ainvoke(
                {"messages": [HumanMessage(task)]},
                {"recursion_limit": 2 * SPECIALIST_MAX_ROUNDS + 8},
            )
        finally:
            current_parent.reset(token)
        report = text_of(result["messages"][-1]).strip() or "The agent finished without a report."
        return Result({"agent": spec.name, "report": report}, shorten(report.splitlines()[0], 160) or "Done")

    return delegate


class PlanStep(BaseModel):
    id: str
    title: str
    status: Literal["pending", "in_progress", "done", "failed", "skipped"] = "pending"
    owner: str | None = Field(None, description="Agent working on it, if any.")


class PlanArgs(BaseModel):
    steps: list[PlanStep] = Field(description="The whole plan each time (replaces the previous one).")


@nyra_tool(
    name="update_plan",
    description="Show the user your plan for a multi-step request, and update step statuses as work progresses. Always send the full list.",
    args=PlanArgs,
    hidden=True,
    label=lambda a: "Updating plan",
)
async def update_plan(a: PlanArgs, call: Call) -> Result:
    if call.run:
        call.run.set_plan([s.model_dump(exclude_none=True) for s in a.steps])
    pending = sum(1 for s in a.steps if s.status in ("pending", "in_progress"))
    return Result({"ok": True, "pendingSteps": pending}, "Plan updated")


def build_orchestrator(settings: LlmSettings, checkpointer: Any):
    model = build_model(settings)
    tools = [*ALL_TOOLS, *(delegate_tool(s, model) for s in SPECIALISTS), update_plan]
    return build_agent_graph(
        model,
        tools,
        prompts.ORCHESTRATOR_PROMPT,
        ORCHESTRATOR_MAX_ROUNDS,
        stream_answer=True,
        checkpointer=checkpointer,
    )


async def run_turn(
    run: RunContext, settings: LlmSettings, checkpointer: Any, seed: list[tuple[str, str]] | None = None
) -> str:
    """Runs one user turn on the conversation's thread and returns the final answer.

    `seed` is earlier (role, text) turns to replay first, for a conversation whose model
    state isn't on this PC (it was synced from another one)."""
    graph = build_orchestrator(settings, checkpointer)
    history: list[BaseMessage] = [HumanMessage(t) if role == "user" else AIMessage(t) for role, t in seed or []]
    result = await graph.ainvoke(
        {"messages": [*history, HumanMessage(run.user_request)]},
        {"configurable": {"thread_id": run.conversation_id}, "recursion_limit": 2 * ORCHESTRATOR_MAX_ROUNDS + 10},
    )
    return text_of(result["messages"][-1])
