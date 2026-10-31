//! Local tool bridge for Nyra, the Python LangGraph agent in `src-python/chatbot`.
//!
//! The agent performs every system operation by calling `POST /tool/<name>` on
//! 127.0.0.1 with a JSON body. Each request runs on its own thread, so the agent's
//! parallel tool calls really run in parallel.
//!
//! Security:
//! - Only loopback is bound.
//! - Every request needs the `X-Asao-Token` header. The token is random per launch and is
//!   written, with the port, to `<app data>/nyra-bridge.json`, which only this user can read.
//! - Requests carrying an `Origin` header (browsers) are refused, so web pages cannot reach it.
//! - Destructive operations still go through the guards in `commands::agent`
//!   (protected paths, critical processes) and the agent asks the user first.
//!
//! Long operations report progress as `AGENT_TOOL_PROGRESS` Tauri events tagged with the
//! `X-Asao-Operation` header, so the chat shows them live next to the matching tool call.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::io::Read;

use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Listener, Manager};

use crate::commands::agent::{self, AgentProgress, DuplicateRequest, SearchFilesRequest, EVENT_AGENT_PROGRESS};
use crate::commands::{apps, process, residual, startup, storage, system, AppMonitoringState};

const FIRST_PORT: u16 = 8766;
const MAX_BODY_BYTES: u64 = 2 * 1024 * 1024;

fn random_token() -> String {
    let mut out = String::new();
    for i in 0..4u64 {
        let mut h = RandomState::new().build_hasher();
        h.write_u64(i);
        h.write_u128(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
        );
        h.write_u32(std::process::id());
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Binds the bridge, writes the discovery file and serves requests on a background thread.
pub fn start(app: AppHandle) {
    let token = random_token();
    let mut bound = None;
    for port in FIRST_PORT..FIRST_PORT + 10 {
        if let Ok(server) = tiny_http::Server::http(("127.0.0.1", port)) {
            bound = Some((server, port));
            break;
        }
    }
    let Some((server, port)) = bound else {
        eprintln!("Nyra tool bridge could not bind any port from {}", FIRST_PORT);
        return;
    };

    match app.path().app_data_dir() {
        Ok(dir) => {
            let _ = std::fs::create_dir_all(&dir);
            let info = json!({ "port": port, "token": token, "pid": std::process::id() });
            if let Err(e) = std::fs::write(dir.join("nyra-bridge.json"), info.to_string()) {
                eprintln!("Nyra tool bridge could not write its discovery file: {}", e);
            }
        }
        Err(e) => eprintln!("Nyra tool bridge has no app data folder: {}", e),
    }

    std::thread::spawn(move || {
        for request in server.incoming_requests() {
            let app = app.clone();
            let token = token.clone();
            std::thread::spawn(move || handle(app, &token, request));
        }
    });
}

fn header(request: &tiny_http::Request, name: &str) -> Option<String> {
    request
        .headers()
        .iter()
        .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(name))
        .map(|h| h.value.as_str().to_string())
}

fn respond(request: tiny_http::Request, status: u16, body: Value) {
    let response = tiny_http::Response::from_string(body.to_string())
        .with_status_code(tiny_http::StatusCode(status))
        .with_header(tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap());
    let _ = request.respond(response);
}

fn handle(app: AppHandle, token: &str, mut request: tiny_http::Request) {
    if header(&request, "Origin").is_some() {
        return respond(request, 403, json!({ "ok": false, "error": "Browser requests are not allowed." }));
    }
    let authorised = header(&request, "X-Asao-Token")
        .map(|t| constant_time_eq(t.as_bytes(), token.as_bytes()))
        .unwrap_or(false);
    if !authorised {
        return respond(request, 401, json!({ "ok": false, "error": "Missing or wrong bridge token." }));
    }

    let url = request.url().to_string();
    if request.method() == &tiny_http::Method::Get && url == "/health" {
        return respond(request, 200, json!({ "ok": true, "data": "ready" }));
    }
    let Some(name) = url.strip_prefix("/tool/").map(str::to_string) else {
        return respond(request, 404, json!({ "ok": false, "error": "Unknown route." }));
    };
    if request.method() != &tiny_http::Method::Post {
        return respond(request, 405, json!({ "ok": false, "error": "Use POST." }));
    }

    let mut body = String::new();
    if request.as_reader().take(MAX_BODY_BYTES).read_to_string(&mut body).is_err() {
        return respond(request, 400, json!({ "ok": false, "error": "Unreadable body." }));
    }
    let args: Value = if body.trim().is_empty() {
        json!({})
    } else {
        match serde_json::from_str(&body) {
            Ok(v) => v,
            Err(e) => return respond(request, 400, json!({ "ok": false, "error": format!("Invalid JSON: {}", e) })),
        }
    };
    let operation = header(&request, "X-Asao-Operation").filter(|s| !s.is_empty());

    match dispatch(&app, &name, args, operation) {
        Ok(data) => respond(request, 200, json!({ "ok": true, "data": data })),
        Err(Failure::Unknown) => respond(request, 404, json!({ "ok": false, "error": format!("Unknown tool '{}'.", name) })),
        Err(Failure::Message(error)) => respond(request, 200, json!({ "ok": false, "error": error })),
    }
}

enum Failure {
    Unknown,
    Message(String),
}

impl From<String> for Failure {
    fn from(s: String) -> Self {
        Failure::Message(s)
    }
}

fn parse<T: for<'de> Deserialize<'de>>(args: Value) -> Result<T, Failure> {
    serde_json::from_value(args).map_err(|e| Failure::Message(format!("Invalid arguments: {}", e)))
}

fn to_value<T: serde::Serialize>(v: T) -> Result<Value, Failure> {
    serde_json::to_value(v).map_err(|e| Failure::Message(e.to_string()))
}

fn block<F: std::future::Future>(f: F) -> F::Output {
    tauri::async_runtime::block_on(f)
}

/// Re-emits a global progress event as agent progress for one operation while `run` executes.
fn relay<T>(
    app: &AppHandle,
    operation: &Option<String>,
    event: &str,
    describe: fn(&Value) -> Option<String>,
    run: impl FnOnce() -> T,
) -> T {
    let Some(op) = operation.clone() else { return run() };
    let emitter = app.clone();
    let last = std::sync::Mutex::new(std::time::Instant::now() - std::time::Duration::from_secs(1));
    let id = app.listen(event, move |e| {
        let Ok(payload) = serde_json::from_str::<Value>(e.payload()) else { return };
        let Some(message) = describe(&payload) else { return };
        if let Ok(mut t) = last.lock() {
            if t.elapsed() < std::time::Duration::from_millis(400) {
                return;
            }
            *t = std::time::Instant::now();
        }
        let _ = emitter.emit(EVENT_AGENT_PROGRESS, AgentProgress { operation_id: op.clone(), message, processed: 0 });
    });
    let out = run();
    app.unlisten(id);
    out
}

fn describe_storage(p: &Value) -> Option<String> {
    let pct = p.get("progressPercent")?.as_f64()?;
    let files = p.get("filesAnalyzed").and_then(Value::as_u64).unwrap_or(0);
    let drive = p.get("drive").and_then(Value::as_str).unwrap_or("");
    Some(format!("Scanning {} {:.0}% ({} files read)", drive, pct, files))
}

fn describe_residual(p: &Value) -> Option<String> {
    let checked = p.get("foldersChecked").and_then(Value::as_u64).unwrap_or(0);
    let found = p.get("candidatesFound").and_then(Value::as_u64).unwrap_or(0);
    let phase = p.get("phase").and_then(Value::as_str).unwrap_or("matching");
    Some(if phase == "inventory" {
        "Listing installed and removed apps".to_string()
    } else {
        format!("Checked {} folders, {} possible leftovers", checked, found)
    })
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct DriveArgs {
    drive: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PathArgs {
    path: String,
    #[serde(default)]
    top: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TerminateArgs {
    pid: u32,
    #[serde(default)]
    expected_name: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartupArgs {
    item_id: String,
    source: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    registry_key: Option<String>,
    #[serde(default)]
    service_name: Option<String>,
    #[serde(default)]
    task_path: Option<String>,
    #[serde(default)]
    command_line: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PathsArgs {
    paths: Vec<String>,
    #[serde(default)]
    permanent: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CleanTempArgs {
    location_ids: Vec<String>,
    #[serde(default)]
    min_age_hours: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppRemoveArgs {
    id: String,
    #[serde(default)]
    use_fallback: bool,
}

fn dispatch(app: &AppHandle, name: &str, args: Value, op: Option<String>) -> Result<Value, Failure> {
    let state = || app.state::<AppMonitoringState>();
    match name {
        // Processes and system
        "processes.list" => to_value(process::get_processes(state())?),
        "system.metrics" => to_value(json!({
            "metrics": system::get_system_metrics(state())?,
            "battery": system::get_battery_status(),
        })),
        "system.recommendations" => to_value(system::get_categorized_recommendations(state())?),
        "process.terminate" => {
            let a: TerminateArgs = parse(args)?;
            to_value(block(agent::agent_terminate_process(state(), a.pid, a.expected_name))?)
        }

        // Startup
        "startup.list" => to_value(startup::get_startup_items(state())?),
        "startup.boot" => to_value(startup::get_boot_performance_summary(state())?),
        "startup.disable" => {
            let a: StartupArgs = parse(args)?;
            startup::disable_startup_item(a.item_id, a.source, a.name, a.registry_key, a.service_name, a.task_path)?;
            Ok(json!({ "disabled": true }))
        }
        "startup.enable" => {
            let a: StartupArgs = parse(args)?;
            startup::enable_startup_item(
                a.item_id, a.source, a.name, a.registry_key, a.service_name, a.task_path, a.command_line,
            )?;
            Ok(json!({ "enabled": true }))
        }

        // Storage overview (drive scan index)
        "storage.drives" => to_value(storage::get_storage_drives()?),
        "storage.snapshot" => {
            let a: DriveArgs = parse(args)?;
            to_value(storage::get_storage_snapshot(a.drive, state())?)
        }
        "storage.scan" => {
            let a: DriveArgs = parse(args)?;
            let result = relay(app, &op, "STORAGE_SCAN_PROGRESS", describe_storage, || {
                block(storage::start_storage_scan(a.drive, app.clone(), state()))
            });
            to_value(result?)
        }
        "storage.list_dir" => {
            let a: PathArgs = parse(args)?;
            to_value(block(storage::read_storage_directory(a.path, None, state()))?)
        }
        "storage.open" => {
            let a: PathArgs = parse(args)?;
            storage::open_storage_location(a.path)?;
            Ok(json!({ "opened": true }))
        }

        // Files and folders
        "fs.known_folders" => to_value(agent::agent_known_folders()),
        "fs.details" => {
            let a: PathArgs = parse(args)?;
            to_value(block(agent::agent_path_details(app.clone(), a.path))?)
        }
        "fs.summary" => {
            let a: PathArgs = parse(args)?;
            to_value(block(agent::agent_folder_summary(app.clone(), a.path, a.top, op))?)
        }
        "fs.search" => {
            let r: SearchFilesRequest = parse(args)?;
            to_value(block(agent::agent_search_files(app.clone(), r, op))?)
        }
        "fs.duplicates" => {
            let r: DuplicateRequest = parse(args)?;
            to_value(block(agent::agent_find_duplicates(app.clone(), r, op))?)
        }
        "fs.temp" => to_value(block(agent::agent_temp_locations(app.clone(), op))?),
        "fs.clean_temp" => {
            let a: CleanTempArgs = parse(args)?;
            to_value(block(agent::agent_clean_temp(app.clone(), a.location_ids, a.min_age_hours, op))?)
        }
        "fs.empty_recycle_bin" => to_value(json!({ "freedBytes": block(agent::agent_empty_recycle_bin())? })),
        "fs.delete" => {
            let a: PathsArgs = parse(args)?;
            to_value(block(agent::agent_delete_paths(app.clone(), a.paths, a.permanent, op))?)
        }

        // Unused apps
        "apps.last" => to_value(apps::apps_get_last(state())?),
        "apps.scan" => to_value(block(apps::apps_scan(app.clone(), state()))?),
        "apps.remove" => {
            let a: AppRemoveArgs = parse(args)?;
            to_value(block(apps::apps_remove(app.clone(), state(), a.id, a.use_fallback))?)
        }

        // Leftovers
        "leftovers.last" => to_value(residual::residual_get_last_result(state())?),
        "leftovers.scan" => {
            let result = relay(app, &op, "RESIDUAL_SCAN_PROGRESS", describe_residual, || {
                block(residual::residual_scan(app.clone(), state()))
            });
            to_value(result?)
        }
        "leftovers.clean" => {
            let a: PathsArgs = parse(args)?;
            to_value(block(residual::residual_clean(app.clone(), state(), a.paths, false, a.permanent))?)
        }

        _ => Err(Failure::Unknown),
    }
}
