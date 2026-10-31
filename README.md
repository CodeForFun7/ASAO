# ASAO — Autonomous System Administrator and Optimizer

ASAO is a Windows desktop app that watches CPU, RAM, GPU, startup programs, and
disk usage, then surfaces plain-language suggestions about what is slowing the
PC down. It is built as a Tauri v2 app: a native Rust backend does all the
system inspection and scoring, a React 19 frontend renders two windows (the
main dashboard and a small always-on-top companion widget), and an optional
local Python service powers an in-app chat assistant ("Nyra").

This document is the map for anyone extending or modifying ASAO. It describes
the real, current behavior of the code — not the aspirational version in
`ARCHITECTURE_AND_ENGINE.md` or `ASAO_BTech_Project_Report_NSUT.md`, which
describe some features (a kill-process command, a parallel storage walk,
Vertex AI chat) that do not exist in this codebase.
Treat those two documents as design history, not a spec.

---

## 1. Project layout

```
ASAO/
├── src-tauri/              Rust backend (Tauri v2)
│   └── src/
│       ├── lib.rs           App bootstrap, window lifecycle, background threads
│       ├── main.rs          Binary entry point
│       ├── collector/       Layer 1 — raw Win32 data collection
│       │   ├── cpu.rs        GetSystemTimes delta CPU sampling
│       │   ├── memory.rs     GlobalMemoryStatusEx RAM sampling
│       │   ├── gpu.rs        PDH GPU engine counter + compositor fallback
│       │   ├── activity.rs   Foreground window + user idle (GetLastInputInfo)
│       │   ├── processes.rs  ToolHelp32 snapshot, per-process I/O, PE metadata
│       │   └── storage.rs    Logical drive enumeration
│       ├── analyzer/        Layer 2/3 — scoring and classification
│       │   ├── classifier.rs          Name/path/publisher → ProcessCategory
│       │   ├── resource_analyzer.rs   Background impact score, status rules
│       │   ├── process_analyzer.rs    Per-tick deltas, system strain formula
│       │   ├── storage_analyzer.rs    Drive walk, category classifier, insights
│       │   └── recommendation_engine.rs  Process/startup/storage suggestions
│       ├── startup/
│       │   └── scanner.rs    Registry/folder/service/task/Winlogon enumeration,
│       │                     StartupApproved toggling, WPR boot trace control
│       ├── commands/         #[tauri::command] IPC handlers (one file per domain)
│       ├── settings/         Persisted JSON settings + widget window geometry
│       ├── tray/             System tray menu
│       └── db/                Unused SQLite schema reference (no sqlite crate wired up)
│
├── src/                     React 19 + TypeScript + Tailwind frontend
│   ├── App.tsx               Picks main shell vs. widget by URL/window label
│   ├── pages/                 Dashboard, Processes, Startup, StoragePage, NyraChat
│   ├── components/            UI split by feature folder (dashboard/, processes/,
│   │                           startup/, storage/, widget/, layout/, common/)
│   ├── stores/                 Zustand stores: process, startup, storage, widget
│   ├── services/                Thin typed wrappers over Tauri invoke/listen
│   └── types/                   Shared TS types mirroring the Rust payloads
│
├── src-python/              Nyra agent service (FastAPI + LangGraph)
│   └── chatbot/
│       ├── main.py            API: streamed chat (SSE), confirmations, stop, conversations
│       ├── graph.py           Orchestrator + 4 specialist agents (LangGraph)
│       ├── tools/              34 system tools, all calling the Rust bridge
│       ├── run_context.py      Live activity feed and confirmation cards for one turn
│       ├── history.py          Saved conversations (+ LangGraph SQLite checkpoints)
│       ├── bridge.py           Client for the authenticated Rust tool bridge
│       └── prompts.py          System prompts
│
└── scripts/dev.mjs          Dev-only launcher: starts Vite + the Python API together
```

---

## 2. How the system works end to end

### 2.1 Backend state and threads

All shared backend state lives in one `AppMonitoringState` struct
(`src-tauri/src/commands/mod.rs`), holding the process engine, storage engine,
cached startup/storage recommendations, and settings, each behind a `Mutex`.
Three background threads run for the lifetime of the app:

| Thread | Interval | Responsibility |
| --- | --- | --- |
| Telemetry loop | 1 s | Collect + analyze processes, emit events to both windows, keep the widget topmost |
| Recommendation refresh | 30 s | Re-run the PowerShell startup scan, recompute startup/storage suggestions |
| Nyra tool bridge (`bridge.rs`) | on request | Authenticated `POST /tool/<name>` on 127.0.0.1 (port 8766+) for the Nyra agent; one thread per request |

Any Tauri command that reads the snapshot (`get_full_snapshot`,
`get_processes`, etc.) reuses the cached result if it is under 750 ms old, so
UI polling never forces extra Win32 calls.

Events emitted from the backend:

| Event | Payload | Consumed by |
| --- | --- | --- |
| `PROCESS_SNAPSHOT`, `SYSTEM_METRICS` | Process list / system metrics | `process-store.ts` |
| `system:update` | Widget condition + combined recommendations | `widget-store.ts` |
| `STORAGE_SCAN_PROGRESS`, `STORAGE_SCAN_COMPLETE` | Scan progress / completion | `storage-store.ts` |
| `widget:visibility`, `settings:updated`, `asao:navigate`, `asao:navigate-inspect` | Window/tray coordination | both windows |

### 2.2 Frontend structure

One Vite bundle serves both the main window and the widget window; `App.tsx`
picks which to render by checking `?window=widget` / the Tauri window label.
There is no router — the active page is a plain string in
`useProcessStore().currentRoute`. State is split across four Zustand stores
(process, startup, storage, widget), each paired with a `services/*.ts` file
that wraps the matching `invoke`/`listen` calls.

---

## 3. Features

### Dashboard
Five metric cards (CPU, RAM, GPU, network, disk) each compared against a
rolling average, plus system health, system strain, and battery panels.
Clicking CPU/RAM cards deep-links into Processes with a matching sort/filter.
GPU usage comes from Windows' "GPU Engine" 3D counters (0% when idle, "N/A" if the
counters are missing). Network is real download plus upload over the physical adapters
(`collector/network.rs`); Windows has no simple per-process network counter, so the
Processes page shows CPU, memory and disk per process only. A process is marked
"Startup" only when it is in the real startup list (Run keys, Startup folders, automatic
services, sign-in tasks) from the startup scan. The battery panel reads Windows directly
(`collector/battery.rs`, `get_battery_status`, polled every 5 s): charge level,
state (charging, on battery, plugged in), the real charge or discharge rate in
watts from the battery driver, time left or time to full, and health
(full-charge vs. design capacity). When the driver reports no rate (usually
while plugged in) or there is no battery, the wattage is an estimate from
current load, shown with a `~`.

Core formulas (`resource_analyzer.rs`, `process_analyzer.rs`):

| Score | Formula |
| --- | --- |
| System strain | `0.40·CPU + 0.35·RAM + 0.10·GPU + 0.15·BackgroundLoad`, clamped 1–100 |
| Background load | `0.65·Σ sustained CPU(bg) + 0.35·(bg RAM share × 100)` |
| Background impact score | `(1.8·CPU + 42·MB/1200 + 3.5·diskMB/s) × durationFactor × activityFactor`, clamped 0–100 |
| Duration factor | 0.45 / 0.85 / 1.10 / 1.25 as sustained load time grows |
| Activity factor | 1.0 background · 0.20 foreground · 0.15 inactive |

### Processes
A filterable/sortable table (category, status, resource level, criticality,
free-text search) backed by an 8-category classifier (`classifier.rs`) that
checks process name, path, and publisher, with hard protection for
system-critical processes. A process counts as "foreground" if it is the
focused window, a child of it, a same-named sibling (covers multi-process
browsers), or was focused within the last 15 seconds. The detail drawer shows
a 30-second resource history built client-side. **There is no way to end a
process from the UI** — the REC modal's only process action is "Inspect".

### Startup
`StartupScanner::scan` shells out to PowerShell to read Registry Run/RunOnce
keys (HKCU/HKLM + WOW6432Node), startup folders, automatic services, boot/logon
scheduled tasks, Winlogon entries, and AppInit/BootExecute, then correlates
each entry with a live process. Classification is keyword-list based
(Essential / Optional / UserDependent → Keep / Disable / Investigate). Boot
CPU/disk/duration numbers are **fixed estimates per source type**, refined by
live memory usage when the item is running — every entry is marked
`etw_traced: true` regardless of whether any real ETW trace was captured.
Toggling writes the same `StartupApproved` binary flag Task Manager uses
(reversible, non-destructive); services use `Set-Service`, tasks use the
`*-ScheduledTask` cmdlets; Winlogon/BootExecute items are blocked from being
disabled. A WPR boot-trace panel can start/stop a real ETW boot trace, but the
resulting trace file is never parsed or read back into the app.

### Storage
Drives are enumerated via Win32 volume APIs. `perform_drive_scan` is a
**single-threaded**, breadth-first walk (not parallel, despite the
architecture doc) seeded from user folders, AppData, Program Files, and
Windows, capped at 95,000 entries and depth 7, skipping WinSxS. Each file is
classified into System / Application / User / Cache / Temporary / Unknown by
path and extension; sizes roll up through parent folders. **On `C:`, bytes the
walk never reached are synthetically split 68% System / 32% Unknown** to fill
out the distribution chart. Results feed the largest-files/folders lists, five
insight types, and an in-memory search index (~10k items). Everything is
read-only — the only action is "open in Explorer" — and results are lost on
restart (nothing is persisted to disk).

### Recommendations (REC buttons)
Each page has a REC button opening a modal scoped to that domain:

| Domain | Qualifying rule (simplified) | Max shown |
| --- | --- | --- |
| Process | Non-core background process ≥80 MB RAM or ≥2.5% CPU or impact score ≥18; falls back to top memory users if fewer than 2 match | 6 (+2 fallback) |
| Startup | Enabled, non-essential, and (flagged Disable/Investigate, or Medium/High impact, or ≥400 ms boot delay, or ≥50 MB) | 7 |
| Storage | User files ≥10 MB idle ≥30 days; AppData/installer items with no matching installed app; caches ≥35 MB | 7 |

The widget interleaves all three lists. Before any manual storage scan,
storage suggestions come from a quick probe of user folders and AppData
rather than a full index.

### Widget, tray, and settings
A frameless, transparent, always-on-top window shows a GOOD / ELEVATED /
ATTENTION condition (a **separate** scale from the dashboard's
healthy/warning/critical status — the two are not the same thresholds), CPU
and RAM sparklines, and a rotating recommendation card that deep-links back
into the main window. Position/opacity persist to a settings JSON file under
the app data directory; closing the widget only hides it. The tray menu can
show/hide the widget, open the main window, open Settings, or quit. The
Settings page covers theme, start-with-Windows, widget enable/opacity/position,
launch-on-startup, always-on-top, show-recommendations, and notifications.

### Nyra agent
Nyra is a LangGraph agent in `src-python/chatbot` that manages the PC through
natural language.

- **Agents.** An orchestrator handles simple requests with tools directly. For
  broad requests it publishes a plan (`update_plan`) and delegates to four
  specialists exposed as tools: Process & Startup, Storage, Application, and
  Cleanup. Each specialist is its own LangGraph loop with a focused prompt and
  tool subset.
- **Parallelism.** `ToolNode` runs every tool call from one model response
  concurrently (tools and specialists alike). The Rust bridge serves each
  request on its own thread.
- **Tools.** Processes (list, inspect, end), startup (list, inspect,
  `disable_Autostart` / `enable_Autostart`, boot performance), storage (drives,
  overview, folder contents and details, search by name terms / kind /
  extension / size / date, file details, large files and folders, duplicates
  by SHA-256, temporary files and cleaning, Recycle Bin, file categories,
  unused files, remove file or folder, open in Explorer), unused apps (list,
  detail, uninstall), and leftovers (list, detail, remove). Rust
  implementations are in `src-tauri/src/commands/agent.rs` and the existing
  command modules. Routing is in `src-tauri/src/bridge.rs`.
- **Safety.** Every destructive tool shows a confirmation card in the chat and
  waits for the user. Rust refuses protected targets regardless:
  - Windows, Program Files, drive roots, profile roots, and ASAO's own data
  - Windows-critical processes

  Deletion goes to the Recycle Bin unless the user asks for permanent deletion.
- **Live feed.** `POST /chat/stream` is a Server-Sent Events stream of tokens,
  activities (agents, tools, notes), progress lines, plan updates and
  confirmations. Long Rust operations also emit `AGENT_TOOL_PROGRESS` Tauri
  events, keyed by activity id.
- **History.** Model state, including tool calls, is checkpointed per
  conversation with `AsyncSqliteSaver`. The chat transcript, including each
  turn's activity trace, is saved alongside it. Both live in
  `%APPDATA%\com.asao.app\nyra.db`.

The Python service only runs in `pnpm tauri dev` (via `scripts/dev.mjs`). It is
not started in production builds.

---

## 4. Known issues to check before building on this code

1. **Settings page currently crashes.** `src/components/widget/WidgetSettings.tsx`
   references the `Check` icon without importing it, and a theme is always
   selected, so the component throws on render. This also fails `tsc` and
   therefore `pnpm build` (which runs `tsc && vite build`):
   ```
   WidgetSettings.tsx(138,22): error TS2304: Cannot find name 'Check'.
   ```
2. **Disabling a registry/startup-folder item can target the wrong entry.**
   The frontend sends the generated item ID (e.g. `reg-run-user-discord`); the
   backend's `clean_item_name` strips the first two hyphen-separated segments
   and lowercases the rest, so the value actually written to
   `StartupApproved` may not match the real registry value name. The UI flips
   the toggle optimistically, so this can look like it worked until the next
   refresh.
3. **"Idle 30+ minutes" process recommendations don't track real idle time.**
   The displayed idle minutes come from process uptime (floored at 30), not
   from when the process last had focus.
4. **Four settings are inert:** `startWithWindows` is saved but never wired to
   an actual autostart entry; `showRecommendations` is ignored by the widget;
   `notifications` never requests browser notification permission;
   `alwaysOnTop` is forced back to `true` on every settings load.
5. **Startup scans run twice on page load** (`getStartupItems` +
   `getBootPerformanceSummary` each trigger a full PowerShell scan), and every
   open REC modal polls `get_categorized_recommendations` every 10 seconds
   even while closed.
6. **The Nyra agent service is dev-only.** Nothing starts the Python service
   in a release build yet.
7. **Dead code:** `AIInsight.tsx`, `ProcessAttention.tsx`,
   `services/process-analysis.ts`, and the SQLite schema in `db/mod.rs` are
   never imported/used (no `sqlite` crate is even in `Cargo.toml`).
8. **`ARCHITECTURE_AND_ENGINE.md` and `ASAO_BTech_Project_Report_NSUT.md` are
   aspirational**, not descriptive — several features and thresholds they
   describe (kill-process command, parallel storage walk,
   Vertex AI integration, different scoring constants) are not in this code.

---

## 5. Getting started

### Prerequisites
- Rust toolchain (stable) + `cargo`
- Node.js + `pnpm`
- Python 3.12 (only needed for the Nyra chat feature)
- Windows 10/11 — ASAO is Windows-only; most collectors are gated on
  `#[cfg(target_os = "windows")]` and no-op elsewhere

### Install dependencies
```powershell
pnpm install
```

### Optional: enable the Nyra chat backend (development only)
```powershell
python -m venv .venv
.venv\Scripts\Activate.ps1
pip install -r src-python\requirements.txt
```
`scripts/dev.mjs` uses `.venv\Scripts\python.exe` if that virtual environment
exists, otherwise it falls back to `python` on `PATH`. At launch the Rust app
starts the tool bridge on the first free port from `127.0.0.1:8766`. It writes
the port and a random per-launch token to `%APPDATA%\com.asao.app\nyra-bridge.json`.
The Python service reads that file, so ASAO must be running for Nyra's tools
to work. Add an OpenAI API key (and optionally a model or an OpenAI-compatible
endpoint) from Nyra's settings
button — it is stored in frontend `localStorage` and sent to the local Python
API with every chat request. **Do not reuse this storage approach on a
shared-device or production build.**

### Run in development
```powershell
pnpm tauri dev
```
This runs `node scripts/dev.mjs`, which starts Vite and (if the venv/deps are
present) the Nyra FastAPI service side by side.

### Build a release binary
```powershell
pnpm tauri build
```
Note that `pnpm build` (and therefore `tauri build`, which runs
`beforeBuildCommand: pnpm build`) currently fails at the `tsc` step — see
Known Issue #1 above — until `WidgetSettings.tsx` is fixed.

---

## 6. Where to look for specific changes

- **New process heuristic or threshold** → `src-tauri/src/analyzer/resource_analyzer.rs` (scoring) and `classifier.rs` (categorization)
- **New recommendation rule** → `src-tauri/src/analyzer/recommendation_engine.rs`
- **New startup source or safer toggling** → `src-tauri/src/startup/scanner.rs` and `src-tauri/src/commands/startup.rs`
- **Storage classification or scan limits** → `src-tauri/src/analyzer/storage_analyzer.rs`
- **New Tauri command** → add to the relevant `src-tauri/src/commands/*.rs` file, then register it in the `tauri::generate_handler![]` list in `lib.rs`
- **New frontend page/section** → add under `src/pages/` or `src/components/<feature>/`, wire state through a `src/stores/*.ts` store and a `src/services/*.ts` wrapper
- **Widget behavior** → `src/components/widget/`, `src/stores/widget-store.ts`, and the geometry/topmost logic in `src-tauri/src/settings/mod.rs`
