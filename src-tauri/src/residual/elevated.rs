//! Deleting from Program Files or ProgramData needs administrator rights. ASAO itself runs
//! as a normal user, so those items are handed to a second copy of `asao.exe`, started with
//! the "runas" verb (Windows shows its permission prompt) and a job file.
//!
//! The helper does not trust the job: it rebuilds the inventory, re-runs every ownership
//! check and the in-use check itself, and only accepts depth 1 or 2 folders of the
//! machine-wide roots. AppData items never go through here.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::cleaner::{process_items, CleanItem, CleanOptions, CleanOutcome};
use super::engine::{build_live_context, Engine, LiveOptions};
use super::normalize::path_key;
use super::store_files::ResidualFiles;
use super::{now_ms, RootKind};

pub const ELEVATED_ARG: &str = "--asao-residual-clean";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElevatedJob {
    pub items: Vec<CleanItem>,
    pub permanent: bool,
    pub app_data_dir: String,
    pub min_age_days: u32,
    pub running_exes: Vec<String>,
    pub protected_paths: Vec<String>,
    pub result_path: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElevatedResult {
    pub outcomes: Vec<CleanOutcome>,
    pub error: Option<String>,
}

#[derive(Debug)]
pub enum ElevationError {
    Declined,
    Failed(String),
}

#[cfg(target_os = "windows")]
pub fn is_process_elevated() -> bool {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut core::ffi::c_void),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        )
        .is_ok();
        let _ = CloseHandle(token);
        ok && elevation.TokenIsElevated != 0
    }
}

#[cfg(not(target_os = "windows"))]
pub fn is_process_elevated() -> bool {
    false
}

/// Starts the administrator helper and waits for it. Windows shows its permission prompt.
#[cfg(target_os = "windows")]
pub fn run_elevated(mut job: ElevatedJob, files: &ResidualFiles) -> Result<Vec<CleanOutcome>, ElevationError> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows::Win32::System::Threading::WaitForSingleObject;
    use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    let jobs = files.jobs_dir();
    fs::create_dir_all(&jobs).map_err(|e| ElevationError::Failed(e.to_string()))?;
    let stamp = now_ms();
    let job_path = jobs.join(format!("job-{}.json", stamp));
    let result_path = jobs.join(format!("job-{}.result.json", stamp));
    job.result_path = result_path.to_string_lossy().to_string();
    let json = serde_json::to_string(&job).map_err(|e| ElevationError::Failed(e.to_string()))?;
    fs::write(&job_path, json).map_err(|e| ElevationError::Failed(e.to_string()))?;

    let exe = std::env::current_exe().map_err(|e| ElevationError::Failed(e.to_string()))?;
    let verb = wide("runas");
    let file = wide(&exe.to_string_lossy());
    let params = wide(&format!("{} \"{}\"", ELEVATED_ARG, job_path.to_string_lossy()));

    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(params.as_ptr()),
        nShow: 0, // SW_HIDE
        ..Default::default()
    };

    let cleanup = |job_path: &Path, result_path: &Path| {
        let _ = fs::remove_file(job_path);
        let _ = fs::remove_file(result_path);
    };

    unsafe {
        if let Err(e) = ShellExecuteExW(&mut info) {
            cleanup(&job_path, &result_path);
            // ERROR_CANCELLED (1223) as an HRESULT: the user said no to the prompt.
            if (e.code().0 as u32) == 0x800704C7 {
                return Err(ElevationError::Declined);
            }
            return Err(ElevationError::Failed(e.message().to_string()));
        }
        if !info.hProcess.is_invalid() {
            let waited = WaitForSingleObject(info.hProcess, 15 * 60 * 1000);
            let _ = CloseHandle(info.hProcess);
            if waited != WAIT_OBJECT_0 {
                cleanup(&job_path, &result_path);
                return Err(ElevationError::Failed("The administrator helper did not finish in time".into()));
            }
        }
    }

    let result = fs::read_to_string(&result_path)
        .ok()
        .and_then(|raw| serde_json::from_str::<ElevatedResult>(&raw).ok());
    cleanup(&job_path, &result_path);
    match result {
        Some(r) => match r.error {
            Some(e) if r.outcomes.is_empty() => Err(ElevationError::Failed(e)),
            _ => Ok(r.outcomes),
        },
        None => Err(ElevationError::Failed("The administrator helper did not report back".into())),
    }
}

#[cfg(not(target_os = "windows"))]
pub fn run_elevated(_job: ElevatedJob, _files: &ResidualFiles) -> Result<Vec<CleanOutcome>, ElevationError> {
    Err(ElevationError::Failed("Not supported".into()))
}

/// Called first thing in `main`. Returns true when this process was the helper.
pub fn maybe_run_from_args() -> bool {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 || args[1] != ELEVATED_ARG {
        return false;
    }
    let job_path = PathBuf::from(&args[2]);
    let job: Option<ElevatedJob> = fs::read_to_string(&job_path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok());
    let Some(job) = job else {
        return true;
    };
    let result = run_job(&job);
    if let Ok(json) = serde_json::to_string(&result) {
        let _ = fs::write(&job.result_path, json);
    }
    true
}

fn run_job(job: &ElevatedJob) -> ElevatedResult {
    let files = ResidualFiles::new(Path::new(&job.app_data_dir));

    // Fresh list of running executables, seen with administrator rights.
    let mut running = job.running_exes.clone();
    if let Ok(records) = crate::collector::processes::ProcessCollector::new().collect_all() {
        running.extend(
            records
                .into_iter()
                .filter_map(|r| r.executable_path)
                .map(|p| path_key(&p)),
        );
    }

    let live = build_live_context(
        &files,
        LiveOptions {
            min_age_days: job.min_age_days,
            running_exes: running,
            protected_paths: job.protected_paths.clone(),
            update_history: false,
            only_roots: Some(vec![RootKind::ProgramFiles, RootKind::ProgramFilesX86, RootKind::ProgramData]),
        },
    );
    let live = match live {
        Ok(l) => l,
        Err(e) => {
            return ElevatedResult {
                outcomes: job.items.iter().map(|i| CleanOutcome::skipped(i, e.clone())).collect(),
                error: Some(e),
            }
        }
    };
    let engine = Engine::new(&live.ctx);
    let outcomes = process_items(
        &engine,
        &job.items,
        CleanOptions {
            dry_run: false,
            permanent: job.permanent,
            only_empty: false,
        },
        &files.deletion_log(),
        true,
    );
    ElevatedResult { outcomes, error: None }
}
