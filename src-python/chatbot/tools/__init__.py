"""Every tool Nyra can call, grouped by area.

Each tool is a LangChain StructuredTool that reaches the system through the Rust bridge.
Add a tool by declaring it with `nyra_tool` in the matching module and listing it there.
"""

from . import apps, leftovers, processes, startup, storage

PROCESS_TOOLS = processes.TOOLS
STARTUP_TOOLS = startup.TOOLS
STORAGE_TOOLS = storage.TOOLS
APP_TOOLS = apps.TOOLS
LEFTOVER_TOOLS = leftovers.TOOLS

ALL_TOOLS = PROCESS_TOOLS + STARTUP_TOOLS + STORAGE_TOOLS + APP_TOOLS + LEFTOVER_TOOLS

__all__ = ["ALL_TOOLS", "APP_TOOLS", "LEFTOVER_TOOLS", "PROCESS_TOOLS", "STARTUP_TOOLS", "STORAGE_TOOLS"]
