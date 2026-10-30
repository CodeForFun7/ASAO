//! Integration test: a full scan against a fake root in a temp folder, with a fixture
//! inventory and history. Never touches real user folders.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, SystemTime};

use super::engine::{Engine, EngineContext, MAX_WALK_ENTRIES};
use super::exclusions::ExclusionList;
use super::history::InventoryHistory;
use super::inventory::{AppSource, InstalledApp};
use super::scoring::removed_apps;
use super::{now_ms, Confidence, OrphanKind, ResidualCategory, RootKind, ScanRoot};

/// A fresh, empty folder under the system temp directory, unique per call so tests
/// running in parallel never share (or clean up) each other's folders.
pub fn temp_dir(label: &str) -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "asao-residual-test-{}-{}-{}-{}",
        label,
        std::process::id(),
        now_ms(),
        n
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

pub fn set_mtime(path: &Path, when: SystemTime) {
    let f = fs::OpenOptions::new().write(true).open(path).unwrap();
    f.set_modified(when).unwrap();
}

fn old_file(path: &Path) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, b"settings").unwrap();
    set_mtime(path, SystemTime::now() - Duration::from_secs(120 * 86_400));
}

/// An old 2 MB file, so the folder is above the 1 MB limit for uncertain folders.
fn big_old_file(path: &Path) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, vec![0u8; 2 * 1024 * 1024]).unwrap();
    set_mtime(path, SystemTime::now() - Duration::from_secs(120 * 86_400));
}

fn app(name: &str, publisher: &str) -> InstalledApp {
    InstalledApp {
        id: format!("hkcu:{}", name.to_lowercase()),
        display_name: Some(name.into()),
        publisher: Some(publisher.into()),
        install_location: None,
        uninstall_string: None,
        display_icon: None,
        source: AppSource::RegistryUser,
        hidden: false,
        package_family_name: None,
        package_name: None,
    }
}

/// History in which "Removed Tool" and "Contoso Painter" were installed, then uninstalled.
fn history(installed_now: &[InstalledApp]) -> InventoryHistory {
    let mut h = InventoryHistory::default();
    let mut before = installed_now.to_vec();
    before.push(app("Removed Tool", "Fabrikam"));
    before.push(app("Contoso Painter", "Contoso"));
    // Pad the inventory so the "source shrank by half" guard is not triggered.
    for i in 0..6 {
        before.push(app(&format!("Filler App {}", i), "Filler"));
    }
    h.observe(&before, &[], 1_000);
    let mut after = installed_now.to_vec();
    for i in 0..6 {
        after.push(app(&format!("Filler App {}", i), "Filler"));
    }
    h.observe(&after, &[], 2_000);
    h
}

struct Fixture {
    dir: PathBuf,
    root: ScanRoot,
    installed: Vec<InstalledApp>,
}

fn fixture() -> Fixture {
    let dir = temp_dir("integration");
    let root_path = dir.join("Roaming");
    // 1. Folder of an installed app: must NOT be flagged
    old_file(&root_path.join("Installed Editor").join("config.ini"));
    // 2. Folder of a removed app: must be flagged High
    old_file(&root_path.join("Removed Tool").join("settings.json"));
    // 3. Publisher folder with one installed and one removed product
    old_file(&root_path.join("Contoso").join("Writer").join("prefs.xml"));
    old_file(&root_path.join("Contoso").join("Painter").join("prefs.xml"));
    // 4. Recently modified orphan: must NOT be flagged
    fs::create_dir_all(root_path.join("Fresh Orphan")).unwrap();
    fs::write(root_path.join("Fresh Orphan").join("state.db"), vec![1u8; 2 * 1024 * 1024]).unwrap();
    // 5. Excluded folder name: must NOT be flagged
    old_file(&root_path.join("NVIDIA").join("driver.cache"));
    // 6. Tiny old orphan: under 1 MB, counted but not listed
    old_file(&root_path.join("Mystery Leftover").join("data.bin"));
    // 7. Old orphan nobody claims, over 1 MB: listed as Low, unticked, under "Other app data"
    big_old_file(&root_path.join("Big Mystery").join("data.bin"));
    // 8. Empty orphan folder: hidden, offered to the one-click empty clean
    fs::create_dir_all(root_path.join("Empty Husk")).unwrap();
    // 9. Updater folders: of an installed app (hidden) and of a removed app (High)
    old_file(&root_path.join("installed-editor-updater").join("pending.json"));
    old_file(&root_path.join("removed-tool-updater").join("pending.json"));
    // 10. Developer caches: Dart is on PATH (hidden), Yarn is not (Cache)
    big_old_file(&root_path.join("Pub").join("Cache").join("pkg.tar"));
    big_old_file(&root_path.join("Yarn").join("Cache").join("pkg.tgz"));
    // 11. Game data: an Unreal-style "Saved" folder
    big_old_file(&root_path.join("Pixel Quest").join("Saved").join("slot1.sav"));

    let installed = vec![
        app("Installed Editor", "Northwind"),
        app("Contoso Writer", "Contoso"),
    ];
    Fixture {
        root: ScanRoot {
            kind: RootKind::RoamingAppData,
            path: root_path,
        },
        dir,
        installed,
    }
}

fn context(f: &Fixture, min_age_days: u32) -> EngineContext {
    let h = history(&f.installed);
    let mut installed = f.installed.clone();
    for i in 0..6 {
        installed.push(app(&format!("Filler App {}", i), "Filler"));
    }
    EngineContext {
        roots: vec![f.root.clone()],
        removed: removed_apps(&h, &installed),
        installed,
        exe_paths: vec![],
        references_available: true,
        exclusions: ExclusionList::new(vec![]),
        min_age_days,
        now_ms: now_ms(),
        onedrive: vec![],
        max_walk_entries: MAX_WALK_ENTRIES,
        path_commands: ["dart".to_string()].into_iter().collect(),
    }
}

#[test]
fn full_scan_against_fake_root() {
    let f = fixture();
    let ctx = context(&f, 30);
    let engine = Engine::new(&ctx);
    let result = engine
        .scan(&AtomicBool::new(false), &mut |_| {})
        .expect("scan runs");

    let flagged: Vec<(String, Confidence)> = result
        .candidates()
        .map(|c| (c.name.clone(), c.confidence))
        .collect();

    let find = |name: &str| flagged.iter().find(|(n, _)| n == name).map(|(_, c)| *c);

    assert_eq!(find("Installed Editor"), None, "installed app folder must not be flagged");
    assert_eq!(find("Removed Tool"), Some(Confidence::High), "removed app must be High");
    assert_eq!(find("Contoso"), None, "publisher folder itself must not be flagged");
    assert_eq!(find("Writer"), None, "installed product under publisher must not be flagged");
    assert_eq!(find("Painter"), Some(Confidence::High), "removed product under publisher must be High");
    assert_eq!(find("Fresh Orphan"), None, "recently modified orphan must not be flagged");
    assert_eq!(find("NVIDIA"), None, "excluded folder must not be flagged");
    assert_eq!(find("Mystery Leftover"), None, "uncertain folder under 1 MB is not listed");
    assert_eq!(find("Empty Husk"), None, "empty folder is not listed");
    assert_eq!(find("Big Mystery"), Some(Confidence::Low));
    assert_eq!(find("installed-editor-updater"), None, "updater of an installed app must not be flagged");
    assert_eq!(find("removed-tool-updater"), Some(Confidence::High), "updater of a removed app must be High");
    assert_eq!(find("Pub"), None, "cache of a tool on PATH must not be flagged");
    assert_eq!(find("Yarn"), Some(Confidence::Low));

    // Hidden small folders are counted, and empty ones are offered for cleaning
    assert!(result.stats.small_hidden >= 2, "{:?}", result.stats);
    assert_eq!(result.empty_folders.len(), 1);
    assert!(result.empty_folders[0].ends_with("Empty Husk"));

    // Grouping: removed apps by name, orphans by kind
    let removed_group = result.groups.iter().find(|g| g.app_name == "Removed Tool").unwrap();
    assert!(!removed_group.is_unknown);
    assert!(removed_group.folders.iter().all(|c| c.preselected));
    assert_eq!(removed_group.folders.len(), 2, "app folder and its updater");
    let group_of = |name: &str| {
        result
            .groups
            .iter()
            .find(|g| g.folders.iter().any(|c| c.name == name))
            .unwrap()
    };
    let dev = group_of("Yarn");
    assert!(dev.is_unknown);
    assert_eq!(dev.key, "unknown:developer-cache");
    assert_eq!(dev.app_name, "Developer caches");
    assert_eq!(dev.folders[0].category, ResidualCategory::Cache);
    assert_eq!(group_of("Pixel Quest").folders[0].orphan_kind, Some(OrphanKind::GameData));
    let other = group_of("Big Mystery");
    assert_eq!(other.key, "unknown:app-data");
    assert!(other.folders.iter().all(|c| !c.preselected));

    // The empty clean only accepts folders that are still empty
    let husk = f.root.path.join("Empty Husk");
    assert!(engine.verify_empty_for_deletion(&husk.to_string_lossy()).is_ok());
    let big = f.root.path.join("Big Mystery");
    assert!(engine.verify_empty_for_deletion(&big.to_string_lossy()).is_err());

    // Pre-delete verification agrees with the scan
    let installed_path = f.root.path.join("Installed Editor");
    assert!(engine.verify_for_deletion(&installed_path.to_string_lossy()).is_err());
    let removed_path = f.root.path.join("Removed Tool");
    assert!(engine.verify_for_deletion(&removed_path.to_string_lossy()).is_ok());
    let outside = f.dir.join("Elsewhere");
    fs::create_dir_all(&outside).unwrap();
    assert!(engine.verify_for_deletion(&outside.to_string_lossy()).is_err());

    let _ = fs::remove_dir_all(&f.dir);
}

#[test]
fn zero_day_override_flags_fresh_orphan_as_unknown() {
    let f = fixture();
    let ctx = context(&f, 0);
    let engine = Engine::new(&ctx);
    let result = engine.scan(&AtomicBool::new(false), &mut |_| {}).unwrap();
    let fresh = result.candidates().find(|c| c.name == "Fresh Orphan").expect("flagged with override");
    assert_eq!(fresh.confidence, Confidence::Low);
    assert!(fresh.matched_app_key.is_none());
    assert!(!fresh.preselected);
    let _ = fs::remove_dir_all(&f.dir);
}

#[test]
fn app_reinstalled_after_scan_is_refused_at_delete_time() {
    let f = fixture();
    let ctx = context(&f, 30);
    let removed_path = f.root.path.join("Removed Tool");
    assert!(Engine::new(&ctx).verify_for_deletion(&removed_path.to_string_lossy()).is_ok());

    // The user reinstalls the app between the scan and the delete.
    let mut ctx2 = context(&f, 30);
    ctx2.installed.push(app("Removed Tool", "Fabrikam"));
    ctx2.removed = removed_apps(&history(&f.installed), &ctx2.installed);
    let err = Engine::new(&ctx2)
        .verify_for_deletion(&removed_path.to_string_lossy())
        .unwrap_err();
    assert!(err.contains("Removed Tool"), "got: {}", err);
    let _ = fs::remove_dir_all(&f.dir);
}

#[test]
fn dry_run_and_permanent_delete_inside_temp_only() {
    use super::cleaner::{process_items, CleanItem, CleanOptions, CleanStatus};
    let f = fixture();
    let ctx = context(&f, 30);
    let engine = Engine::new(&ctx);
    let log = f.dir.join("log.jsonl");
    let target = f.root.path.join("Removed Tool");
    let item = CleanItem {
        path: target.to_string_lossy().to_string(),
        name: "Removed Tool".into(),
        category: Some(ResidualCategory::AppData),
        size_bytes: Some(8),
        needs_admin: false,
        in_onedrive: false,
    };

    let dry = process_items(&engine, &[item.clone()], CleanOptions { dry_run: true, permanent: false, only_empty: false }, &log, false);
    assert_eq!(dry[0].status, CleanStatus::WouldDelete);
    assert!(target.exists(), "dry run must not delete");

    // Permanent delete of a folder this test created. Never the Recycle Bin, never real data.
    let real = process_items(&engine, &[item], CleanOptions { dry_run: false, permanent: true, only_empty: false }, &log, false);
    assert_eq!(real[0].status, CleanStatus::Deleted, "{:?}", real[0].reason);
    assert!(!target.exists());
    let lines = fs::read_to_string(&log).unwrap();
    assert!(lines.contains("would-delete") && lines.contains("\"deleted\""));
    let _ = fs::remove_dir_all(&f.dir);
}

/// Read-only scan of this machine, for manual review. Never deletes anything and keeps its
/// history in a temp folder. Run with: cargo test --lib live_scan_smoke -- --ignored --nocapture
#[test]
#[ignore]
fn live_scan_smoke() {
    use super::engine::{build_live_context, LiveOptions};
    use super::store_files::ResidualFiles;
    let data = temp_dir("live");
    let files = ResidualFiles::new(&data);
    let live = build_live_context(
        &files,
        LiveOptions {
            min_age_days: 30,
            running_exes: vec![],
            protected_paths: vec![],
            update_history: true,
            only_roots: None,
        },
    )
    .expect("context");
    let engine = Engine::new(&live.ctx);
    let started = std::time::Instant::now();
    let result = engine.scan(&AtomicBool::new(false), &mut |_| {}).expect("scan");
    println!("scan took {:?}", started.elapsed());
    println!("stats: {:?}", result.stats);
    let mut low = 0;
    for g in &result.groups {
        for c in &g.folders {
            if c.confidence == Confidence::Low {
                low += 1;
                if low > 40 {
                    continue;
                }
            }
            println!("{:?}\t{}\t{:?}\t{}\t{}", c.confidence, c.score, c.category, c.path, g.app_name);
        }
    }
    println!("low total: {}", low);
    println!("empty folders offered for cleaning: {}", result.empty_folders.len());
    for p in &result.empty_folders {
        println!("EMPTY	{}", p);
    }
    let _ = fs::remove_dir_all(&data);
}

/// Prints the raw evidence of past uninstalls on this machine. Read-only.
#[test]
#[ignore]
fn live_evidence_dump() {
    for e in super::evidence::collect_past_uninstalls() {
        println!("{:?}\t{}\tpub={:?}\talt={:?}\tdirs={:?}", e.source, e.display_name, e.publisher, e.alt_names, e.install_dirs);
    }
}


/// Read-only: prints every Uncertain folder with its size and root, for analysis.
#[test]
#[ignore]
fn live_uncertain_breakdown() {
    use super::engine::{build_live_context, LiveOptions};
    use super::store_files::ResidualFiles;
    let data = temp_dir("breakdown");
    let files = ResidualFiles::new(&data);
    let live = build_live_context(&files, LiveOptions { min_age_days: 30, running_exes: vec![], protected_paths: vec![], update_history: true, only_roots: None }).expect("context");
    let engine = Engine::new(&live.ctx);
    let result = engine.scan(&AtomicBool::new(false), &mut |_| {}).expect("scan");
    let never = AtomicBool::new(false);
    for c in result.candidates().filter(|c| c.confidence == Confidence::Low) {
        let size = super::sizer::measure(Path::new(&c.path), &never).size_on_disk;
        let exes = std::fs::read_dir(&c.path).map(|rd| rd.flatten().filter(|e| e.path().extension().map(|x| x.eq_ignore_ascii_case("exe")).unwrap_or(false)).count()).unwrap_or(0);
        println!("LOW	{:?}	{}	{}	{}	{}", c.root_kind, c.depth, size, exes, c.path);
    }
    let _ = fs::remove_dir_all(&data);
}
