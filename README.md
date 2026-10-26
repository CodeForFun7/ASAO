# Tauri + React + Typescript

This template should help get you started developing with Tauri, React and Typescript in Vite.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
# Nyra local agent (development)

Nyra's Python API starts automatically alongside Vite when running
`pnpm tauri dev`. Install Python dependencies once from the repository root:

```powershell
python -m venv .venv
.venv\Scripts\Activate.ps1
pip install -r src-python\requirements.txt
```

The development launcher uses `.venv\Scripts\python.exe` when that virtual
environment exists, otherwise it uses `python` from `PATH`. The Rust app exposes the local `get_info` tool
bridge on `127.0.0.1:8766`; Python calls it only when the model requests that
tool. Add an OpenAI API key from Nyra's settings button. The key is kept in
frontend local storage for this initial version and is sent to the local Python
API for each chat request. Do not use this storage approach for a shared-device
or production release.
