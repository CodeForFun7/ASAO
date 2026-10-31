"""Paths and defaults for the Nyra agent service."""

import os
from pathlib import Path

# Tauri's app data folder for identifier com.asao.app (see src-tauri/tauri.conf.json).
APP_IDENTIFIER = "com.asao.app"


def app_data_dir() -> Path:
    override = os.environ.get("ASAO_APP_DATA_DIR")
    if override:
        return Path(override)
    base = os.environ.get("APPDATA") or str(Path.home() / "AppData" / "Roaming")
    return Path(base) / APP_IDENTIFIER


def bridge_file() -> Path:
    """Written by the Rust app at launch: {"port": ..., "token": ...}."""
    return Path(os.environ.get("ASAO_BRIDGE_FILE") or app_data_dir() / "nyra-bridge.json")


def database_file() -> Path:
    """Conversation history and LangGraph checkpoints."""
    path = Path(os.environ.get("NYRA_DB_FILE") or app_data_dir() / "nyra.db")
    path.parent.mkdir(parents=True, exist_ok=True)
    return path


DEFAULT_MODEL = "gpt-4o-mini"

# Orchestrator and sub-agent tool rounds before they must answer.
ORCHESTRATOR_MAX_ROUNDS = 20
SPECIALIST_MAX_ROUNDS = 14

# Longest tool result passed back to the model, in characters.
MAX_TOOL_RESULT_CHARS = 14_000

# How long a confirmation card waits for the user before counting as declined.
CONFIRMATION_TIMEOUT_SECONDS = 15 * 60
