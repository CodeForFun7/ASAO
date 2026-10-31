//! Tests for the bloatware review. No test touches the real machine except the ignored
//! live scan, which only reads.

use std::collections::HashMap;

use super::database::{wildcard_match, BloatwareDatabase, ConditionFacts};
use super::impact::{band_for, parse_startup_xml, score, startup_bucket, ImpactInput, ProcessSample, Samples};
use super::inventory::{describe_trigger, exe_of_command, iso_duration_secs};
use super::safety::{always_kept, apply_runtime_checks, base_verdict, preselected, RuntimeFacts};
use super::scan::ScanInputs;
use super::{
    AntivirusProduct, BloatItem, Category, DeviceContext, ImpactBand, ItemKind, Safety, ServiceInfo,
    StartupEntryInfo,
};
use crate::residual::inventory::{AppDetails, AppSource, InstalledApp};

const DAY: u64 = 86_400_000;
const NOW: u64 = 1_800_000_000_000;

fn db() -> BloatwareDatabase {
    BloatwareDatabase::built_in_combined()
}

fn entry_json(id: &str, extra: &str) -> String {
    format!(
        r#"{{"id":"{id}","match":{{"packageFamily":["Foo.Bar"]}},"displayName":"Foo","category":"promoted-app",
        "whatItDoes":"Does foo.","safety":"usually-safe","safetyReason":"Because.","whatBreaks":"Foo.",
        "howToRestore":"Store.","recommendation":"optional","source":"test"{extra}}}"#
    )
}

fn db_with(entries: &[String]) -> Result<BloatwareDatabase, String> {
    BloatwareDatabase::parse(&format!(r#"{{"version":1,"updated":"x","entries":[{}]}}"#, entries.join(",")))
}

// ---- Database ------------------------------------------------------------------------------

#[test]
fn built_in_database_is_valid_and_complete() {
    let d = db();
    assert!(d.entries.len() > 100);
    for e in &d.entries {
        assert!(!e.what_it_does.trim().is_empty(), "{}", e.id);
        assert!(!e.safety_reason.trim().is_empty(), "{}", e.id);
        assert!(!e.what_breaks.trim().is_empty(), "{}", e.id);
        assert!(!e.how_to_restore.trim().is_empty(), "{}", e.id);
        assert!(!e.source.trim().is_empty(), "{}", e.id);
    }
    let mut ids: Vec<&str> = d.entries.iter().map(|e| e.id.as_str()).collect();
    ids.sort();
    let before = ids.len();
    ids.dedup();
    assert_eq!(before, ids.len(), "duplicate ids");
}

#[test]
fn validation_rejects_bad_entries() {
    assert!(db_with(&[entry_json("a", "")]).is_ok());
    assert!(db_with(&[entry_json("a", ""), entry_json("a", "")]).unwrap_err().contains("duplicate"));
    let empty = entry_json("a", "").replace(r#""whatBreaks":"Foo.""#, r#""whatBreaks":"  ""#);
    assert!(db_with(&[empty]).unwrap_err().contains("whatBreaks"));
    let mismatch = entry_json("a", "").replace(r#""recommendation":"optional""#, r#""recommendation":"remove""#);
    assert!(db_with(&[mismatch]).unwrap_err().contains("recommendation"));
    let unknown = entry_json("a", r#","conditions":[{"when":"full-moon","safety":"caution"}]"#);
    assert!(db_with(&[unknown]).unwrap_err().contains("full-moon"));
    let no_match = entry_json("a", "").replace(r#""packageFamily":["Foo.Bar"]"#, "");
    assert!(db_with(&[no_match]).unwrap_err().contains("match"));
    let protected = entry_json("a", "").replace(r#""safety":"usually-safe""#, r#""safety":"protected""#);
    assert!(db_with(&[protected]).is_err());
}

#[test]
fn wildcard_patterns() {
    assert!(wildcard_match("McAfee*", "McAfee LiveSafe"));
    assert!(wildcard_match("*candy crush*", "King Candy Crush Saga"));
    assert!(wildcard_match("CDPUserSvc_*", "CDPUserSvc_1a2b3"));
    assert!(!wildcard_match("CDPUserSvc_*", "CDPUserSvc"));
    assert!(wildcard_match("Fax", "fax"));
    assert!(!wildcard_match("Fax", "Faxes"));
}

#[test]
fn matching_by_package_name_and_service() {
    let d = db();
    assert_eq!(d.match_package("Clipchamp.Clipchamp").map(|e| e.safety), Some(Safety::Safe));
    assert_eq!(d.match_package("king.com.CandyCrushSaga").map(|e| e.category), Some(Category::PromotedApp));
    // Win11Debloat rates MyASUS optional.
    assert_eq!(d.match_package("B9ECED6F.ASUSPCAssistant").map(|e| e.safety), Some(Safety::UsuallySafe));
    // Win11Debloat matches every package whose name contains the id.
    assert!(d.match_package("ActiproSoftwareLLC.562882FEEB491").is_some());
    assert_eq!(d.match_package("Microsoft.WindowsStore").map(|e| e.safety), Some(Safety::NotRecommended));
    assert!(d.match_package("Some.UnknownApp").is_none());
    // Per-user service instances match their template's entry.
    assert!(d.match_service("CDPUserSvc_1a2b3").is_some());
    assert_eq!(d.match_service("Fax").map(|e| e.safety), Some(Safety::UsuallySafe));
    assert_eq!(d.match_service("WinDefend").map(|e| e.safety), Some(Safety::NotRecommended));
}

#[test]
fn win32_name_and_publisher_must_both_match() {
    let d = db();
    assert!(d.match_win32("Norton 360", Some("Gen Digital Inc.")).is_some());
    // Same owner, different product: not Norton.
    assert!(d.match_win32("Avast Free Antivirus", Some("Gen Digital Inc.")).is_none());
    assert!(d.match_win32("Norton Commander", Some("Peter Norton")).is_none());
    assert!(d.match_win32("McAfee LiveSafe", Some("McAfee, LLC")).is_some());
}

#[test]
fn conditions_only_make_entries_more_cautious() {
    let d = db();
    let _ = d;
    let games = entry_json("g", r#","conditions":[{"when":"games-installed","safety":"caution","recommendation":"keep","note":"Games need it."}]"#);
    let d1 = db_with(&[games]).unwrap();
    assert_eq!(d1.entries[0].rate(ConditionFacts::default()).safety, Safety::UsuallySafe);
    let gamer = d1.entries[0].rate(ConditionFacts { games_installed: true, org_managed: false });
    assert_eq!(gamer.safety, Safety::Caution);
    assert!(!gamer.notes.is_empty());

    // A condition that tries to relax an entry is ignored.
    let relax = entry_json("r", r#","conditions":[{"when":"org-managed","safety":"safe","recommendation":"remove"}]"#);
    let d2 = db_with(&[relax]).unwrap();
    let r = d2.entries[0].rate(ConditionFacts { games_installed: false, org_managed: true });
    assert_eq!(r.safety, Safety::UsuallySafe);
}

#[test]
fn windows_services_are_never_safe_to_remove() {
    for e in db().entries.iter().filter(|e| e.category == Category::OptionalWindowsService) {
        assert_ne!(e.safety, Safety::Safe, "{}", e.id);
    }
}

// ---- Impact --------------------------------------------------------------------------------

#[test]
fn startup_buckets_follow_windows() {
    assert_eq!(startup_bucket(1_500, 0).1, "High");
    assert_eq!(startup_bucket(0, 4 * 1024 * 1024).1, "High");
    assert_eq!(startup_bucket(500, 0).1, "Medium");
    assert_eq!(startup_bucket(0, 400 * 1024).1, "Medium");
    assert_eq!(startup_bucket(100, 1024).1, "Low");
}

#[test]
fn impact_score_with_fixed_inputs() {
    let heavy = score(&ImpactInput {
        startup: vec![Some((1_200, 0))],
        logon_tasks: 0,
        cpu_percent: Some(3.0),
        sample_seconds: 30,
        memory_bytes: 250 * 1024 * 1024,
        auto_services: 2,
        frequent_tasks: 1,
        disk_bytes: Some(600 * 1024 * 1024),
    });
    // 30 + 18 + 15 + 10 + 10
    assert_eq!(heavy.total, 83);
    assert_eq!(heavy.band, ImpactBand::High);

    let idle = score(&ImpactInput { disk_bytes: Some(30 * 1024 * 1024), ..Default::default() });
    assert_eq!(idle.total, 3);
    assert_eq!(idle.band, ImpactBand::Low);
    assert_eq!(idle.startup.value, "Doesn't run at startup");

    let unmeasured = score(&ImpactInput { startup: vec![None], ..Default::default() });
    assert_eq!(unmeasured.startup.points, 15, "runs at startup without a measurement is Medium");
    assert_eq!(band_for(59), ImpactBand::Medium);
    assert_eq!(band_for(60), ImpactBand::High);
    assert_eq!(band_for(29), ImpactBand::Low);
}

#[test]
fn startup_info_xml_is_parsed() {
    let xml = r#"<StartupData><Process Name="C:\Program Files\Foo\foo.exe" PID="1"><StartTime>1</StartTime><CpuUsage>1234</CpuUsage><DiskUsage>5000000</DiskUsage></Process>
<Process Name="C:\Bar\bar.exe"><CpuUsage>20</CpuUsage></Process></StartupData>"#;
    let rows = parse_startup_xml(xml);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0], (r"C:\Program Files\Foo\foo.exe".to_string(), 1234, 5_000_000));
    assert_eq!(rows[1].1, 20);
    assert_eq!(rows[1].2, 0);
}

#[test]
fn task_triggers_and_durations() {
    assert_eq!(iso_duration_secs("PT1H"), Some(3_600));
    assert_eq!(iso_duration_secs("PT30M"), Some(1_800));
    assert_eq!(iso_duration_secs("P1D"), Some(86_400));
    assert_eq!(iso_duration_secs("P1DT2H"), Some(93_600));
    assert!(describe_trigger("MSFT_TaskLogonTrigger", None, None).1);
    assert!(!describe_trigger("MSFT_TaskDailyTrigger", None, Some(1)).1);
    let (text, frequent) = describe_trigger("MSFT_TaskDailyTrigger", Some("PT1H"), Some(1));
    assert!(frequent);
    assert!(text.contains("every 1 h"), "{}", text);
    assert_eq!(exe_of_command(r#""C:\A B\x.exe" /run"#).as_deref(), Some(r"C:\A B\x.exe"));
    assert_eq!(exe_of_command(r"C:\Tools\y.exe --flag").as_deref(), Some(r"C:\Tools\y.exe"));
}

// ---- Safety --------------------------------------------------------------------------------

fn item(kind: ItemKind, name: &str) -> BloatItem {
    let mut it = super::scan::tests_support::blank_item(kind, name);
    it.impact = score(&ImpactInput::default());
    it
}

fn facts<'a>(av: &'a [String], defender: bool, managed: Option<&'a str>) -> RuntimeFacts<'a> {
    RuntimeFacts {
        conditions: ConditionFacts::default(),
        active_antivirus: av,
        defender_present: defender,
        managed_reason: managed,
        now_ms: NOW,
    }
}

fn verdict_for(db_id: &str, it: &BloatItem, f: &RuntimeFacts) -> Safety {
    let d = db();
    let e = d.find(db_id);
    let mut v = base_verdict(e, f);
    apply_runtime_checks(&mut v, it, f, None, None);
    v.safety
}

#[test]
fn dependents_downgrade_to_caution() {
    let mut it = item(ItemKind::StoreApp, "Clipchamp");
    let f = facts(&[], true, None);
    assert_eq!(verdict_for("w11d-clipchamp-clipchamp", &it, &f), Safety::Safe);
    it.dependents = vec!["Other (other)".into()];
    assert_eq!(verdict_for("w11d-clipchamp-clipchamp", &it, &f), Safety::Caution);
}

#[test]
fn active_antivirus_is_protected() {
    let mut it = item(ItemKind::Win32App, "McAfee LiveSafe");
    it.publisher = Some("McAfee, LLC".into());
    let av = vec!["McAfee LiveSafe".to_string()];
    assert_eq!(verdict_for("mcafee", &it, &facts(&av, true, None)), Safety::Caution);
    assert_eq!(verdict_for("mcafee", &it, &facts(&av, false, None)), Safety::NotRecommended);
    // Not the active antivirus: the database level stands.
    assert_eq!(verdict_for("mcafee", &it, &facts(&[], true, None)), Safety::Safe);
}

#[test]
fn running_or_recently_used_downgrades() {
    let f = facts(&[], true, None);
    let mut it = item(ItemKind::StoreApp, "Clipchamp");
    it.running = true;
    assert_eq!(verdict_for("w11d-clipchamp-clipchamp", &it, &f), Safety::Caution);
    it.running = false;
    it.last_used_ms = Some(NOW - 5 * DAY);
    assert_eq!(verdict_for("w11d-clipchamp-clipchamp", &it, &f), Safety::Caution);
    it.last_used_ms = Some(NOW - 45 * DAY);
    assert_eq!(verdict_for("w11d-clipchamp-clipchamp", &it, &f), Safety::Safe);
}

#[test]
fn managed_pcs_keep_oem_tools() {
    let mut it = item(ItemKind::Win32App, "ASUS GiftBox");
    it.category = Some(Category::OemUtility);
    assert_eq!(verdict_for("asus-giftbox", &it, &facts(&[], true, None)), Safety::Safe);
    assert_eq!(verdict_for("asus-giftbox", &it, &facts(&[], true, Some("Joined to a domain"))), Safety::Caution);
}

#[test]
fn platform_pieces_are_always_kept() {
    assert!(always_kept(ItemKind::StoreApp, Some("Microsoft.WindowsStore"), "Store", false).is_some());
    assert!(always_kept(ItemKind::StoreApp, Some("Microsoft.VCLibs.140.00"), "x", false).is_some());
    assert!(always_kept(ItemKind::StoreApp, Some("Foo.Bar"), "x", true).is_some());
    assert!(always_kept(ItemKind::Win32App, None, "Microsoft Visual C++ 2015-2022 Redistributable (x64)", false).is_some());
    assert!(always_kept(ItemKind::Win32App, None, "Microsoft Edge WebView2 Runtime", false).is_some());
    assert!(always_kept(ItemKind::Win32App, None, "Intel(R) Chipset Device Software", false).is_some());
    assert!(always_kept(ItemKind::Win32App, None, "Candy Crush Saga", false).is_none());

    // Even a database that calls the Store "safe" cannot make it removable.
    let it = item(ItemKind::StoreApp, "Microsoft Store");
    let f = facts(&[], true, None);
    let mut v = base_verdict(db().match_package("Clipchamp.Clipchamp"), &f);
    let kept = always_kept(ItemKind::StoreApp, Some("Microsoft.WindowsStore"), "Store", false);
    apply_runtime_checks(&mut v, &it, &f, kept, None);
    assert_eq!(v.safety, Safety::NotRecommended);
    let mut v = base_verdict(None, &f);
    apply_runtime_checks(&mut v, &it, &f, None, Some("Windows does not allow it".into()));
    assert_eq!(v.safety, Safety::Protected);
}

#[test]
fn only_safe_and_usually_safe_are_selectable() {
    for s in [Safety::Safe, Safety::UsuallySafe] {
        assert!(s.selectable());
    }
    for s in [Safety::Caution, Safety::NotRecommended, Safety::Protected, Safety::NotRated] {
        assert!(!s.selectable(), "{:?}", s);
    }
    // Not rated stays information only even after a Caution check.
    let mut it = item(ItemKind::Win32App, "Unknown Tool");
    it.running = true;
    let f = facts(&[], true, None);
    let mut v = base_verdict(None, &f);
    apply_runtime_checks(&mut v, &it, &f, None, None);
    assert_eq!(v.safety, Safety::NotRated);
}

#[test]
fn pre_selection_rules() {
    // Exactly Win11Debloat's default selection, whatever the impact.
    let mut it = item(ItemKind::StoreApp, "x");
    it.safety = Safety::Safe;
    it.category = Some(Category::MicrosoftInboxApp);
    it.impact.band = ImpactBand::Low;
    assert!(preselected(&it, true));
    assert!(!preselected(&it, false));
    it.safety = Safety::UsuallySafe;
    assert!(preselected(&it, true), "optional apps Win11Debloat selects are ticked too");
    // ASAO's checks still decide whether it can be ticked at all.
    for s in [Safety::Caution, Safety::NotRecommended, Safety::Protected, Safety::NotRated] {
        it.safety = s;
        assert!(!preselected(&it, true), "{:?}", s);
    }
    let mut svc = item(ItemKind::Service, "Fax");
    svc.safety = Safety::UsuallySafe;
    assert!(!preselected(&svc, true), "services are never pre-ticked");
}

// ---- Full build against a fake PC ----------------------------------------------------------

fn store(name: &str, family: &str, location: &str) -> (InstalledApp, AppDetails) {
    (
        InstalledApp {
            id: format!("store:{}", family),
            display_name: Some(name.into()),
            publisher: None,
            install_location: Some(location.into()),
            uninstall_string: None,
            display_icon: None,
            source: AppSource::Store,
            hidden: false,
            package_family_name: Some(family.into()),
            package_name: Some(name.into()),
        },
        AppDetails {
            package_full_name: Some(format!("{}_1.0.0.0_x64__abc", name)),
            signature_kind: Some("Store".into()),
            ..Default::default()
        },
    )
}

fn win32(name: &str, publisher: &str, location: &str, size: u64) -> (InstalledApp, AppDetails) {
    (
        InstalledApp {
            id: format!("hklm:{}", name.to_lowercase()),
            display_name: Some(name.into()),
            publisher: Some(publisher.into()),
            install_location: Some(location.into()),
            uninstall_string: Some(format!(r#""{}\uninstall.exe""#, location)),
            display_icon: None,
            source: AppSource::RegistryMachine,
            hidden: false,
            package_family_name: None,
            package_name: None,
        },
        AppDetails { estimated_size_bytes: Some(size), ..Default::default() },
    )
}

fn service(name: &str, path: &str, start: &str, state: &str, pid: u32, windows: bool) -> ServiceInfo {
    ServiceInfo {
        name: name.into(),
        display_name: name.into(),
        description: None,
        binary_path: Some(path.into()),
        start_type: start.into(),
        delayed_auto_start: false,
        state: state.into(),
        account: None,
        process_id: pid,
        depends_on: vec![],
        dependents: vec![],
        is_windows: windows,
        planned_start_type: "Manual".into(),
    }
}

fn proc_sample(exe: &str, cpu_sum: f64, mem: u64) -> ProcessSample {
    ProcessSample {
        exe_key: Some(crate::residual::normalize::path_key(exe)),
        cpu_sum,
        memory_bytes: mem,
    }
}

#[test]
fn full_build_links_scores_and_rates() {
    let d = db();
    let pairs = vec![
        store("Clipchamp.Clipchamp", "Clipchamp.Clipchamp_yxz26nhyzhsrt", r"C:\Program Files\WindowsApps\Clipchamp_1"),
        store("king.com.CandyCrushSaga", "king.com.CandyCrushSaga_kgqvnymyfvs32", r"C:\Program Files\WindowsApps\Candy_1"),
        store("B9ECED6F.ASUSPCAssistant", "B9ECED6F.ASUSPCAssistant_qmba6cd70vzyy", r"C:\Program Files\WindowsApps\MyAsus_1"),
        win32("McAfee LiveSafe", "McAfee, LLC", r"C:\Program Files\McAfee", 900 * 1024 * 1024),
        win32("Some Editor", "Indie Dev", r"C:\Program Files\Some Editor", 50 * 1024 * 1024),
    ];
    // A web app registered against the editor's folder (as Chrome web apps are).
    let mut pwa = win32("Editor Web App", "Indie Dev", r"C:\Program Files\Some Editor", 1024);
    pwa.0.install_location = None;
    pwa.0.uninstall_string = Some(r#""C:\Program Files\Some Editor\editor.exe" --uninstall-app"#.into());
    let mut pairs = pairs;
    pairs.push(pwa);
    let apps: Vec<InstalledApp> = pairs.iter().map(|p| p.0.clone()).collect();
    let details: HashMap<String, AppDetails> = pairs.iter().map(|p| (p.0.id.clone(), p.1.clone())).collect();
    let services = vec![
        service("McAfeeSvc", r#""C:\Program Files\McAfee\svc.exe" -s"#, "Auto", "Running", 500, false),
        service("Fax", r"C:\Windows\system32\fxssvc.exe", "Manual", "Stopped", 0, true),
        service("CDPUserSvc", r"C:\Windows\system32\svchost.exe -k UnistackSvcGroup", "Auto", "Stopped", 0, true),
        service("CDPUserSvc_1a2b", r"C:\Windows\system32\svchost.exe -k UnistackSvcGroup", "Auto", "Running", 900, true),
        service("ThirdPartyHelper", r"C:\Tools\helper.exe", "Auto", "Running", 700, false),
        service("Spooler", r"C:\Windows\System32\spoolsv.exe", "Auto", "Running", 600, true),
    ];
    let startup = vec![StartupEntryInfo {
        id: "reg-run-machine-mcafee".into(),
        name: "McAfee Tray".into(),
        source: "registry-run-machine".into(),
        command: Some(r#""C:\Program Files\McAfee\tray.exe""#.into()),
        executable: Some(r"C:\Program Files\McAfee\tray.exe".into()),
        enabled: true,
        registry_key: None,
        boot_cpu_ms: None,
        boot_disk_bytes: None,
    }];
    let device = DeviceContext {
        antivirus: vec![AntivirusProduct { name: "Windows Defender".into(), enabled: true }],
        defender_present: true,
        ..Default::default()
    };
    let mut by_pid = HashMap::new();
    by_pid.insert(500, proc_sample(r"C:\Program Files\McAfee\svc.exe", 30.0, 120 * 1024 * 1024));
    by_pid.insert(501, proc_sample(r"C:\Program Files\Some Editor\editor.exe", 3.0, 80 * 1024 * 1024));
    let samples = Samples { count: 15, seconds: 30, by_pid };
    let broad = vec![
        r"c:\program files".to_string(),
        r"c:\program files\windowsapps".to_string(),
    ];
    let last_use = |_d: &[String], _f: Option<&str>| None;
    let size = |_d: &str| Some(10 * 1024 * 1024);

    let items = ScanInputs {
        db: &d,
        apps: &apps,
        details: &details,
        services: &services,
        startup: &startup,
        tasks: &[],
        device: &device,
        samples: &samples,
        startup_info: None,
        broad: &broad,
        windir: Some(r"c:\windows"),
        last_use: &last_use,
        folder_size: &size,
        now_ms: NOW,
    }
    .build();

    let find = |id: &str| items.iter().find(|i| i.id == id).unwrap_or_else(|| panic!("{} missing", id));

    let clip = find("store:Clipchamp.Clipchamp_yxz26nhyzhsrt");
    assert_eq!(clip.safety, Safety::Safe);
    assert!(clip.selectable);
    assert!(clip.preselected, "Win11Debloat selects Clipchamp by default");
    assert_eq!(clip.store_url.as_deref(), Some("ms-windows-store://pdp/?PFN=Clipchamp.Clipchamp_yxz26nhyzhsrt"));

    let candy = find("store:king.com.CandyCrushSaga_kgqvnymyfvs32");
    assert!(candy.preselected, "Win11Debloat selects Candy Crush by default");

    let asus = find("store:B9ECED6F.ASUSPCAssistant_qmba6cd70vzyy");
    assert_eq!(asus.safety, Safety::UsuallySafe, "Win11Debloat rates it optional");
    assert!(asus.selectable && !asus.preselected, "optional and not selected by default");

    // McAfee: its service and tray entry are linked to it, and it is scored on them.
    let mcafee = find(r"app:hklm:mcafee livesafe");
    assert_eq!(mcafee.services.len(), 1);
    assert_eq!(mcafee.startup.len(), 1);
    assert!(mcafee.impact.total >= 30, "{:?}", mcafee.impact);
    assert_eq!(mcafee.category, Some(Category::Trialware));
    assert!(!mcafee.running, "a background service alone is not 'running' for the user");
    assert_eq!(mcafee.safety, Safety::Safe);
    assert!(!mcafee.preselected, "only Win11Debloat's defaults are pre-ticked");
    assert!(mcafee.planned_changes.iter().any(|c| c.contains("startup entry")));
    assert!(mcafee.planned_changes.iter().any(|c| c.contains("McAfeeSvc") && c.contains("Manual")));
    assert!(mcafee.planned_changes.last().unwrap().contains("uninstaller"));
    assert!(mcafee.undo.contains("can't reinstall"));

    // An app ASAO does not know: listed, never selectable or pre-ticked, and running.
    let editor = find(r"app:hklm:some editor");
    assert_eq!(editor.safety, Safety::NotRated);
    assert!(editor.running);
    assert!(!editor.selectable && !editor.preselected);

    let pwa = find(r"app:hklm:editor web app");
    assert!(!pwa.running, "a web app is not charged for its host browser's processes");
    assert_eq!(pwa.impact.memory.points, 0);

    // Windows services: only database entries are listed; templates are skipped.
    let fax = find("svc:Fax");
    assert_eq!(fax.safety, Safety::UsuallySafe);
    assert!(!fax.preselected, "Windows services are never pre-ticked");
    assert_eq!(fax.services[0].planned_start_type, "Disabled");
    assert!(items.iter().any(|i| i.id == "svc:CDPUserSvc_1a2b"));
    assert!(!items.iter().any(|i| i.id == "svc:CDPUserSvc"), "per-user template is skipped");
    assert_eq!(find("svc:Spooler").safety, Safety::NotRecommended);
    // Third-party service with no app and no entry: listed as Not rated.
    assert_eq!(find("svc:ThirdPartyHelper").safety, Safety::NotRated);

    // Nothing that cannot be selected is ever pre-ticked.
    assert!(items.iter().all(|i| !i.preselected || i.selectable));
    assert!(items.iter().filter(|i| i.safety == Safety::NotRated).all(|i| !i.preselected));
}

/// Read-only: scans this PC (no CPU sampling) and prints every row. Changes nothing.
#[test]
#[ignore]
fn live_bloat_scan() {
    use super::inventory::{collect_device, collect_services, collect_startup, collect_tasks};
    let d = db();
    let inv = crate::residual::inventory::collect_installed_apps();
    let services = collect_services().unwrap_or_default();
    let tasks = collect_tasks().unwrap_or_default();
    let startup = collect_startup(&[]);
    let gaming = inv.apps.iter().any(|a| a.package_name.as_deref() == Some("Microsoft.GamingServices"));
    let device = collect_device(gaming);
    println!("device: {:?}", device);
    let samples = Samples::default();
    let windir = std::env::var("WINDIR").ok().map(|w| crate::residual::normalize::path_key(&w));
    let mut broad: Vec<String> = crate::residual::roots::known_roots()
        .iter()
        .map(|r| crate::residual::normalize::path_key(&r.path.to_string_lossy()))
        .collect();
    broad.push(r"c:\program files\windowsapps".into());
    broad.push(r"c:\program files\common files".into());
    let last_use = |_d: &[String], _f: Option<&str>| None;
    let size = |_d: &str| None;
    let items = ScanInputs {
        db: &d,
        apps: &inv.apps,
        details: &inv.details,
        services: &services,
        startup: &startup,
        tasks: &tasks,
        device: &device,
        samples: &samples,
        startup_info: None,
        broad: &broad,
        windir: windir.as_deref(),
        last_use: &last_use,
        folder_size: &size,
        now_ms: crate::residual::now_ms(),
    }
    .build();
    for i in &items {
        if i.safety == Safety::NotRecommended && i.kind == ItemKind::Service {
            continue;
        }
        println!(
            "{:?}\t{:?}\t{}\t{:?}\t{}\tsvc={} run={} tasks={}\t{}",
            i.safety,
            i.kind,
            i.impact.total,
            i.category,
            i.name,
            i.services.len(),
            i.startup.len(),
            i.tasks.len(),
            i.safety_checks.join(" | ")
        );
    }
    println!("items: {}", items.len());
}

// ---- Stage 2: planning, dry run, undo -------------------------------------------------------

mod changes {
    use super::*;
    use crate::bloat::executor::{
        approved_target, execute, ps_quote, split_task_path, ExecOptions, ExecReport, Original, Outcome,
        OutcomeStatus, RestorePointPolicy, RestorePointResult, RestorePointStatus,
    };
    use crate::bloat::history::{store_family, undo_for, ChangeHistory, UndoKind};
    use crate::bloat::planner::{phase, plan, ActionKind, PlanOptions};
    use crate::bloat::{effective_dry_run, TaskInfo};

    fn rated(id: &str, kind: ItemKind, safety: Safety) -> BloatItem {
        let mut it = item(kind, id);
        it.id = id.to_string();
        it.safety = safety;
        it.selectable = safety.selectable();
        it
    }

    fn full_app(id: &str, safety: Safety) -> BloatItem {
        let mut it = rated(id, ItemKind::Win32App, safety);
        it.size_bytes = Some(100);
        it.registry_subkey = Some("{GUID}".into());
        it.services.push(service("OemSvc", r"C:\Oem\svc.exe", "Auto", "Running", 10, false));
        it.startup.push(StartupEntryInfo {
            id: "s".into(),
            name: "Oem Tray".into(),
            source: "registry-run-user".into(),
            command: None,
            executable: None,
            enabled: true,
            registry_key: Some(r"HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run".into()),
            boot_cpu_ms: None,
            boot_disk_bytes: None,
        });
        it.tasks.push(TaskInfo {
            path: r"\Oem\Updater".into(),
            name: "Updater".into(),
            action: None,
            enabled: true,
            frequent: true,
            triggers: vec![],
        });
        it
    }

    #[test]
    fn plan_runs_in_the_required_order() {
        let mut store = rated("store", ItemKind::StoreApp, Safety::Safe);
        store.package_full_name = Some("Foo_1.0.0.0_x64__abc".into());
        store.package_name = Some("Foo".into());
        let items = vec![full_app("app", Safety::Safe), store];
        let p = plan(&items, &["store".into(), "app".into()], &PlanOptions { remove_provisioned: true });
        let phases: Vec<u8> = p.actions.iter().map(|a| phase(&a.kind)).collect();
        let mut sorted = phases.clone();
        sorted.sort();
        assert_eq!(phases, sorted, "restore point, startup, tasks, services, uninstalls, provisioned");
        assert!(matches!(p.actions[0].kind, ActionKind::RestorePoint));
        assert!(p.actions.iter().any(|a| matches!(a.kind, ActionKind::RemoveProvisioned { .. })));
        assert_eq!(p.not_undoable, vec!["app".to_string()]);
        assert_eq!(p.services_stopped, 1);
        assert!(p.needs_admin);
        // Provisioned removal only when asked.
        let p2 = plan(&items, &["store".into()], &PlanOptions::default());
        assert!(!p2.actions.iter().any(|a| matches!(a.kind, ActionKind::RemoveProvisioned { .. })));
    }

    #[test]
    fn unsafe_items_never_reach_a_plan() {
        let items = vec![
            full_app("caution", Safety::Caution),
            full_app("needed", Safety::NotRecommended),
            full_app("locked", Safety::Protected),
            full_app("unknown", Safety::NotRated),
        ];
        let ids: Vec<String> = items.iter().map(|i| i.id.clone()).collect();
        let p = plan(&items, &ids, &PlanOptions { remove_provisioned: true });
        assert!(p.actions.is_empty(), "{:?}", p.actions);
        assert_eq!(p.rejected.len(), 4);
        // Even with `selectable` forced on, the safety level decides.
        let mut forged = full_app("forged", Safety::NotRecommended);
        forged.selectable = true;
        let p = plan(&[forged], &["forged".into()], &PlanOptions::default());
        assert!(p.actions.is_empty());
        // Unknown ids are rejected too.
        assert_eq!(plan(&[], &["ghost".into()], &PlanOptions::default()).rejected.len(), 1);
    }

    #[test]
    fn services_already_in_place_are_left_alone() {
        let mut it = rated("svc", ItemKind::Service, Safety::UsuallySafe);
        let mut s = service("Fax", r"C:\Windows\fxssvc.exe", "Disabled", "Stopped", 0, true);
        s.planned_start_type = "Disabled".into();
        it.services.push(s);
        assert!(plan(&[it], &["svc".into()], &PlanOptions::default()).actions.is_empty());
    }

    #[test]
    fn dry_run_rules() {
        // Development build: the switch decides. Release: off unless forced.
        assert!(effective_dry_run(true, true, false));
        assert!(!effective_dry_run(false, true, false));
        assert!(!effective_dry_run(true, false, false));
        assert!(effective_dry_run(false, false, true));
    }

    #[test]
    fn dry_run_changes_nothing_and_logs_everything() {
        let mut store = rated("store", ItemKind::StoreApp, Safety::Safe);
        store.package_full_name = Some("Foo_1.0.0.0_x64__abc".into());
        store.package_name = Some("Foo".into());
        let mut win = rated("win", ItemKind::Win32App, Safety::Safe);
        win.registry_subkey = Some("{GUID}".into());
        let p = plan(&[store, win], &["store".into(), "win".into()], &PlanOptions::default());
        let dir = crate::residual::tests::temp_dir("bloat-dry");
        let log = dir.join("dry.jsonl");
        let report = execute(
            &p.actions,
            ExecOptions { dry_run: true, restore_point_policy: RestorePointPolicy::Stop, elevated: false, w11d: None, scratch: &dir },
            "b1",
            &log,
            &mut |_, _, _| {},
        );
        assert_eq!(report.restore_point.status, RestorePointStatus::Skipped);
        assert_eq!(report.outcomes.len(), 2);
        assert!(report.outcomes.iter().all(|o| o.status == OutcomeStatus::WouldDo));
        let lines = std::fs::read_to_string(&log).unwrap();
        assert_eq!(lines.lines().count(), 2);
        assert!(lines.contains("\"dryRun\":true"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn startup_switch_locations() {
        let (k, n) = approved_target("Foo", "registry-run-user", Some(r"HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run"));
        assert!(k.starts_with("HKCU:") && k.ends_with(r"StartupApproved\Run"));
        assert_eq!(n, vec!["Foo".to_string()]);
        let (k, _) = approved_target(
            "Foo",
            "registry-run-machine",
            Some(r"HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run"),
        );
        assert!(k.starts_with("HKLM:") && k.ends_with(r"StartupApproved\Run32"));
        let (k, n) = approved_target("Foo", "startup-folder-user", None);
        assert!(k.ends_with(r"StartupApproved\StartupFolder"));
        assert_eq!(n, vec!["Foo.lnk".to_string()]);
        assert_eq!(split_task_path(r"\HP\HP Analytics"), (r"\HP\".to_string(), "HP Analytics".to_string()));
        assert_eq!(ps_quote("it's"), "'it''s'");
        assert_eq!(store_family("Foo.Bar_1.2.3.0_x64__8wekyb3d8bbwe").as_deref(), Some("Foo.Bar_8wekyb3d8bbwe"));
    }

    fn outcome(kind: ActionKind, status: OutcomeStatus, original: Option<Original>) -> Outcome {
        Outcome {
            action: crate::bloat::planner::Action {
                id: format!("a#{}", phase(&kind)),
                item_id: "a".into(),
                item_name: "A".into(),
                kind,
                description: String::new(),
                needs_admin: true,
                undoable: true,
            },
            status,
            message: None,
            original,
        }
    }

    #[test]
    fn undo_restores_the_recorded_state() {
        let svc = outcome(
            ActionKind::ConfigureService {
                name: "OemSvc".into(),
                display_name: "Oem".into(),
                start_type: "Manual".into(),
                stop: true,
            },
            OutcomeStatus::Done,
            Some(Original::Service { start_type: "Auto".into(), delayed: true, running: true }),
        );
        assert_eq!(
            undo_for(&svc),
            Some(UndoKind::RestoreService { name: "OemSvc".into(), start_type: "Auto".into(), delayed: true, running: true })
        );
        let task = outcome(ActionKind::DisableTask { path: r"\Oem\T".into() }, OutcomeStatus::Done, Some(Original::Task { enabled: true }));
        assert_eq!(undo_for(&task), Some(UndoKind::EnableTask { path: r"\Oem\T".into() }));
        // A change that did not go through has nothing to undo.
        let failed = outcome(
            ActionKind::DisableTask { path: r"\Oem\T2".into() },
            OutcomeStatus::Failed,
            Some(Original::Task { enabled: true }),
        );
        assert_eq!(undo_for(&failed), None);
        let win = outcome(ActionKind::UninstallWin32 { display_name: "A".into(), registry_subkey: None }, OutcomeStatus::Done, None);
        assert_eq!(undo_for(&win), None);

        let mut h = ChangeHistory::default();
        let report = ExecReport {
            restore_point: RestorePointResult { status: RestorePointStatus::Created, message: String::new() },
            outcomes: vec![svc, task, failed, win],
            stopped: false,
            restart_needed: false,
        };
        h.record("b1", 1, &report, &|_| Some("Get it from oem.example.".into()));
        assert_eq!(h.batches[0].actions.len(), 3, "failed changes are not recorded");
        let w = h.batches[0]
            .actions
            .iter()
            .find(|a| matches!(a.outcome.action.kind, ActionKind::UninstallWin32 { .. }))
            .unwrap();
        assert!(!w.can_undo && w.undo_hint.contains("can't reinstall") && w.undo_hint.contains("oem.example"));
        h.mark_undone("b1", "a#3", 5);
        assert!(h.batches[0].actions.iter().any(|a| a.undone_ms == Some(5)));
    }
}


// ---- Win11Debloat data and registry files --------------------------------------------------

mod win11debloat {
    use crate::bloat::planner::{plan_with_tweaks, ActionKind, PlanOptions};
    use crate::bloat::regfile::{all_match, op_matches, parse, test_base64, test_base64_decode, LiveValue, RegData, RegOp};
    use crate::bloat::w11d::{TweakItem, TweakSet, Win11Debloat, EXCLUDED_CATEGORIES};

    #[test]
    fn bundled_files_load() {
        let w = Win11Debloat::built_in();
        assert_eq!(w.apps.len(), 162);
        assert!(w.presets.iter().any(|p| p.name.contains("Xbox")));
        assert!(w.presets.iter().any(|p| p.name.contains("OEM")));
        assert!(w.defaults.contains("DisableTelemetry"));
        // Every offered tweak's registry file is bundled and parses to at least one change.
        for f in &w.features {
            if w.unavailable(f, Some(26200)).is_none() {
                let (_, ops, _) = w.ops_for(&f.id).expect("registry file");
                assert!(!ops.is_empty(), "{} has no registry operations", f.id);
            }
        }
    }

    #[test]
    fn risky_categories_and_script_features_are_not_offered() {
        let w = Win11Debloat::built_in();
        let set = w.tweaks(Some(26200), false);
        let offered: Vec<&TweakItem> = set.tweaks.iter().collect();
        assert!(offered.iter().all(|t| !EXCLUDED_CATEGORIES.contains(&t.category.as_str())));
        assert!(!offered.iter().any(|t| t.id == "DisableDriverUpdates"));
        assert!(!offered.iter().any(|t| t.id == "DisableWidgets"), "done by script, not a registry file");
        assert!(offered.iter().any(|t| t.id == "DisableTelemetry"));
        // Windows 10 only tweaks are not offered on Windows 11.
        assert!(!offered.iter().any(|t| t.id == "Hide3dObjects"));
        assert!(set.not_offered.iter().any(|(_, why)| why.contains("Windows Update")));
        // Pick-one groups keep their options together.
        let search = set.groups.iter().find(|g| g.id == "SearchIcon").expect("search group");
        assert!(search.options.iter().any(|o| o.is_default));
        // Without a registry read, nothing is known to be applied and defaults are ticked.
        assert!(set.tweaks.iter().find(|t| t.id == "DisableTelemetry").unwrap().preselected);
        assert!(set.tweaks.iter().filter(|t| t.group_id.is_some()).all(|t| !t.preselected));
    }

    #[test]
    fn registry_files_parse() {
        let text = "\u{feff}Windows Registry Editor Version 5.00\r\n\r\n; comment\r\n[HKEY_CURRENT_USER\\Software\\Test]\r\n\"A\"=dword:00000001\r\n\"S\"=\"x \\\"y\\\"\"\r\n\"Gone\"=-\r\n@=\"default\"\r\n\"B\"=hex:01,02,\\\r\n  03\r\n\r\n[-HKEY_LOCAL_MACHINE\\SOFTWARE\\Old]\r\n";
        let ops = parse(text);
        assert_eq!(ops.len(), 6);
        assert_eq!(ops[0], RegOp::Set { key: r"HKEY_CURRENT_USER\Software\Test".into(), name: "A".into(), data: RegData::DWord { value: 1 } });
        assert_eq!(ops[1], RegOp::Set { key: r"HKEY_CURRENT_USER\Software\Test".into(), name: "S".into(), data: RegData::String { value: "x \"y\"".into() } });
        assert!(matches!(&ops[2], RegOp::DeleteValue { name, .. } if name == "Gone"));
        assert!(matches!(&ops[3], RegOp::Set { name, .. } if name.is_empty()));
        assert_eq!(ops[4], RegOp::Set { key: r"HKEY_CURRENT_USER\Software\Test".into(), name: "B".into(), data: RegData::Raw { kind: "Binary".into(), base64: test_base64(&[1, 2, 3]) } });
        assert_eq!(ops[5], RegOp::DeleteKey { key: r"HKEY_LOCAL_MACHINE\SOFTWARE\Old".into() });
        assert_eq!(test_base64_decode(&test_base64(b"hello world!?")), b"hello world!?");
    }

    #[test]
    fn applied_means_every_value_matches() {
        let set = RegOp::Set { key: "K".into(), name: "A".into(), data: RegData::DWord { value: 0 } };
        let del = RegOp::DeleteValue { key: "K".into(), name: "B".into() };
        let dword0 = LiveValue { key_exists: true, exists: true, kind: Some("DWord".into()), value: Some(serde_json::json!(0)) };
        let dword1 = LiveValue { value: Some(serde_json::json!(1)), ..dword0.clone() };
        let missing = LiveValue { key_exists: true, ..Default::default() };
        assert!(op_matches(&set, &dword0));
        assert!(!op_matches(&set, &dword1));
        assert!(!op_matches(&set, &missing));
        assert!(op_matches(&del, &missing));
        assert!(all_match(&[set.clone(), del.clone()], &[dword0.clone(), missing.clone()]));
        assert!(!all_match(&[set, del], &[dword1, missing]));
        assert!(!all_match(&[], &[]), "an empty file is never applied");
    }

    fn tweak(id: &str, group: Option<&str>, applied: Option<bool>) -> TweakItem {
        TweakItem {
            id: id.into(),
            label: id.into(),
            tooltip: None,
            category: "Taskbar".into(),
            group_id: group.map(|g| g.to_string()),
            applied,
            preselected: false,
            requires_reboot: false,
            needs_admin: false,
            registry_file: format!("{}.reg", id),
        }
    }

    #[test]
    fn tweak_plans() {
        let set = TweakSet {
            tweaks: vec![
                tweak("A", None, Some(false)),
                tweak("Done", None, Some(true)),
                tweak("G1", Some("G"), Some(false)),
                tweak("G2", Some("G"), Some(false)),
            ],
            groups: vec![],
            not_offered: vec![],
        };
        let ids: Vec<String> = ["A", "Done", "G1", "G2", "Unknown"].iter().map(|s| s.to_string()).collect();
        let p = plan_with_tweaks(&[], &[], &set, &ids, &PlanOptions::default());
        let applied: Vec<&str> = p
            .actions
            .iter()
            .filter_map(|a| match &a.kind {
                ActionKind::ApplyTweak { feature_id, .. } => Some(feature_id.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(applied, vec!["A", "G1"], "applied tweaks are skipped and one option per group");
        assert!(matches!(p.actions[0].kind, ActionKind::RestorePoint), "a restore point comes first");
        assert_eq!(p.rejected.len(), 1, "unknown tweak ids are rejected");
    }
}

/// Read-only: prints every offered tweak and whether it is already in effect on this PC.
#[test]
#[ignore]
fn live_tweak_state() {
    let w = crate::bloat::w11d::Win11Debloat::built_in();
    let build = crate::bloat::inventory::collect_device(false).windows_build;
    let set = w.tweaks(build, true);
    println!("build {:?}: {} offered, {} groups, {} not offered", build, set.tweaks.len(), set.groups.len(), set.not_offered.len());
    for t in &set.tweaks {
        println!("{:?}\t{}\t{}\t{}\tadmin={}", t.applied, if t.preselected { "TICKED" } else { "-" }, t.category, t.id, t.needs_admin);
    }
    for g in &set.groups {
        println!("group {} current={:?}", g.id, g.current);
    }
}

