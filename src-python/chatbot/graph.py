from langchain_core.messages import SystemMessage, ToolMessage
from langchain_openai import ChatOpenAI
from langgraph.graph import END, START, StateGraph
from langgraph.prebuilt import ToolNode

from .prompts import SYSTEM_PROMPT
from .state import ChatState
from .tools import TOOLS


def build_graph(api_key: str):
    model = ChatOpenAI(model="gpt-4o-mini", api_key=api_key, temperature=0).bind_tools(TOOLS)

    def assistant(state: ChatState):
        messages = [SystemMessage(content=SYSTEM_PROMPT), *state["messages"]]
        return {"messages": [model.invoke(messages)]}

    def route(state: ChatState):
        last = state["messages"][-1]
        return "tools" if getattr(last, "tool_calls", None) else END

    graph = StateGraph(ChatState)
    graph.add_node("assistant", assistant)
    graph.add_node("tools", ToolNode(TOOLS))
    graph.add_edge(START, "assistant")
    graph.add_conditional_edges("assistant", route, {"tools": "tools", END: END})
    graph.add_edge("tools", "assistant")
    return graph.compile()


def run_turn(api_key: str, history: list[dict[str, str]]) -> str:
    from langchain_core.messages import AIMessage, HumanMessage

    messages = []
    for item in history:
        if item.get("role") == "user":
            messages.append(HumanMessage(content=item.get("content", "")))
        elif item.get("role") == "assistant":
            messages.append(AIMessage(content=item.get("content", "")))
    result = build_graph(api_key).invoke({"messages": messages})
    content = result["messages"][-1].content
    if isinstance(content, list):
        return "".join(part.get("text", "") for part in content if isinstance(part, dict))
    return str(content)
