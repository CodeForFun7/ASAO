use crate::analyzer::ProcessCategory;

pub struct ClassificationResult {
    pub category: ProcessCategory,
    pub is_system_critical: bool,
    pub is_startup: bool,
    pub fallback_publisher: Option<String>,
    pub fallback_product: Option<String>,
    pub description: Option<String>,
}

pub fn classify_process(
    pid: u32,
    name: &str,
    exe_path: Option<&str>,
    publisher: Option<&str>,
    product_name: Option<&str>,
) -> ClassificationResult {
    let lower_name = name.trim().to_lowercase();
    let lower_path = exe_path.unwrap_or("").to_lowercase();
    let lower_pub = publisher.unwrap_or("").to_lowercase();
    let lower_prod = product_name.unwrap_or("").to_lowercase();

    // 1. Kernel / PID 0 & 4 / Core Windows OS processes
    if pid <= 4
        || matches!(
            lower_name.as_str(),
            "system"
                | "registry"
                | "secure system"
                | "memory compression"
                | "smss.exe"
                | "csrss.exe"
                | "wininit.exe"
                | "services.exe"
                | "lsass.exe"
                | "lsm.exe"
                | "svchost.exe"
                | "winlogon.exe"
                | "dwm.exe"
                | "fontdrvhost.exe"
                | "sihost.exe"
                | "taskhostw.exe"
                | "explorer.exe"
                | "runtimebroker.exe"
                | "applicationframehost.exe"
                | "systemsettingsbroker.exe"
                | "searchhost.exe"
                | "searchindexer.exe"
                | "startmenuexperiencehost.exe"
                | "shellexperiencehost.exe"
                | "textinputhost.exe"
                | "ctfmon.exe"
                | "dllhost.exe"
                | "wmiprvse.exe"
                | "spoolsv.exe"
                | "audiodg.exe"
                | "msmpeng.exe"
                | "nissrv.exe"
                | "securityhealthservice.exe"
                | "securityhealthsystray.exe"
                | "smartscreen.exe"
                | "dashost.exe"
                | "unsecapp.exe"
                | "wuauclt.exe"
                | "tiworker.exe"
                | "trustedinstaller.exe"
                | "vds.exe"
                | "VSSVC.exe"
                | "conhost.exe"
                | "widgets.exe"
                | "widgetservice.exe"
        )
    {
        let critical = pid <= 4
            || matches!(
                lower_name.as_str(),
                "system"
                    | "registry"
                    | "secure system"
                    | "memory compression"
                    | "smss.exe"
                    | "csrss.exe"
                    | "wininit.exe"
                    | "services.exe"
                    | "lsass.exe"
                    | "svchost.exe"
                    | "winlogon.exe"
                    | "dwm.exe"
                    | "fontdrvhost.exe"
                    | "sihost.exe"
                    | "explorer.exe"
                    | "msmpeng.exe"
                    | "securityhealthservice.exe"
                    | "wmiprvse.exe"
            );

        let desc = match lower_name.as_str() {
            "svchost.exe" => "Host Process for Windows Services",
            "dwm.exe" => "Desktop Window Manager Compositor",
            "explorer.exe" => "Windows Shell & File Explorer",
            "lsass.exe" => "Local Security Authority Process",
            "csrss.exe" => "Client Server Runtime Process",
            "services.exe" => "Windows Service Control Manager",
            "msmpeng.exe" => "Microsoft Defender Antivirus Service",
            "runtimebroker.exe" => "Windows UWP Permission Broker",
            "searchhost.exe" | "searchindexer.exe" => "Windows Search Indexing Service",
            "taskhostw.exe" => "Host Process for Windows Tasks",
            "sihost.exe" => "Shell Infrastructure Host",
            "audiodg.exe" => "Windows Audio Device Graph Isolation",
            _ => "Windows Core System Component",
        };

        return ClassificationResult {
            category: ProcessCategory::WindowsCore,
            is_system_critical: critical,
            is_startup: true,
            fallback_publisher: Some("Microsoft Corporation".to_string()),
            fallback_product: Some("Microsoft® Windows® Operating System".to_string()),
            description: Some(desc.to_string()),
        };
    }

    // 2. Browser & Browser Components
    if matches!(
        lower_name.as_str(),
        "chrome.exe"
            | "msedge.exe"
            | "msedgewebview2.exe"
            | "firefox.exe"
            | "brave.exe"
            | "opera.exe"
            | "vivaldi.exe"
            | "arc.exe"
            | "googlecrashhandler.exe"
            | "googlecrashhandler64.exe"
            | "elevation_service.exe"
            | "microsoftedgeupdate.exe"
    ) || lower_prod.contains("chromium")
        || lower_prod.contains("google chrome")
        || lower_prod.contains("microsoft edge")
        || lower_prod.contains("firefox")
        || lower_prod.contains("brave browser")
    {
        let (pub_name, prod_name, desc) = if lower_name.contains("chrome") || lower_name.contains("google") {
            ("Google LLC", "Google Chrome", "Chromium Browser Process / Renderer")
        } else if lower_name.contains("msedgewebview2") {
            ("Microsoft Corporation", "Microsoft Edge WebView2", "Embedded WebView2 Runtime Component")
        } else if lower_name.contains("edge") {
            ("Microsoft Corporation", "Microsoft Edge", "Microsoft Edge Browser Process")
        } else if lower_name.contains("firefox") {
            ("Mozilla Corporation", "Mozilla Firefox", "Firefox Quantum Content Process")
        } else if lower_name.contains("brave") {
            ("Brave Software, Inc.", "Brave Browser", "Brave Chromium Browser Process")
        } else {
            ("Browser Vendor", "Web Browser Component", "Browser Engine Subprocess")
        };

        return ClassificationResult {
            category: ProcessCategory::Browser,
            is_system_critical: false,
            is_startup: lower_name.contains("update") || lower_name.contains("crashhandler"),
            fallback_publisher: Some(pub_name.to_string()),
            fallback_product: Some(prod_name.to_string()),
            description: Some(desc.to_string()),
        };
    }

    // 3. Hardware Drivers & Vendor Daemons
    if matches!(
        lower_name.as_str(),
        "nvcontainer.exe"
            | "nvdisplay.container.exe"
            | "nvidia web helper.exe"
            | "nvidia share.exe"
            | "nvsphelper64.exe"
            | "radeonsoftware.exe"
            | "amdrsserv.exe"
            | "amdow.exe"
            | "atieclxx.exe"
            | "atiesrxx.exe"
            | "igfxcuiservice.exe"
            | "igfxem.exe"
            | "intelcphdcpsvc.exe"
            | "intelcphecisvc.exe"
            | "jhi_service.exe"
            | "rtkauduservice64.exe"
            | "ravbg64.exe"
            | "syntpenh.exe"
            | "logioptionsmgr.exe"
            | "lghub_agent.exe"
            | "lghub_updater.exe"
            | "razer synapse 3.exe"
    ) || lower_path.contains("\\driverstore\\")
        || lower_pub.contains("nvidia")
        || lower_pub.contains("advanced micro devices")
        || lower_pub.contains("realtek")
        || lower_pub.contains("intel(r)")
        || lower_pub.contains("synaptics")
        || lower_pub.contains("logitech")
    {
        let pub_fallback = if lower_name.starts_with("nv") || lower_name.contains("nvidia") {
            "NVIDIA Corporation"
        } else if lower_name.contains("amd") || lower_name.contains("radeon") || lower_name.starts_with("ati") {
            "Advanced Micro Devices, Inc."
        } else if lower_name.contains("intel") || lower_name.starts_with("igfx") {
            "Intel Corporation"
        } else if lower_name.starts_with("rtk") {
            "Realtek Semiconductor Corp."
        } else {
            "Hardware Device Vendor"
        };

        return ClassificationResult {
            category: ProcessCategory::Drivers,
            is_system_critical: lower_path.contains("\\system32\\") || lower_path.contains("\\driverstore\\"),
            is_startup: true,
            fallback_publisher: Some(pub_fallback.to_string()),
            fallback_product: Some("Hardware Driver & Telemetry Container".to_string()),
            description: Some("Hardware Driver Service / Device Daemon".to_string()),
        };
    }

    // 4. Development Tools & Runtimes
    if matches!(
        lower_name.as_str(),
        "code.exe"
            | "cursor.exe"
            | "devenv.exe"
            | "node.exe"
            | "pnpm.exe"
            | "npm.exe"
            | "bun.exe"
            | "deno.exe"
            | "esbuild.exe"
            | "cargo.exe"
            | "rustc.exe"
            | "rust-analyzer.exe"
            | "git.exe"
            | "python.exe"
            | "pythonw.exe"
            | "java.exe"
            | "javaw.exe"
            | "idea64.exe"
            | "webstorm64.exe"
            | "pycharm64.exe"
            | "clion64.exe"
            | "goland64.exe"
            | "docker.exe"
            | "com.docker.backend.exe"
            | "wsl.exe"
            | "wslservice.exe"
            | "wslhost.exe"
            | "windowsterminal.exe"
            | "openconsole.exe"
            | "powershell.exe"
            | "pwsh.exe"
            | "cmd.exe"
            | "asao.exe"
    ) || lower_pub.contains("jetbrains")
        || lower_pub.contains("docker")
        || lower_path.contains("\\microsoft vs code\\")
        || lower_path.contains("\\.cargo\\")
        || lower_path.contains("\\nodejs\\")
    {
        let (pub_fallback, prod_fallback, desc) = match lower_name.as_str() {
            "code.exe" => ("Microsoft Corporation", "Visual Studio Code", "Code Editor & Extension Host"),
            "node.exe" => ("OpenJS Foundation", "Node.js Runtime", "V8 JavaScript Runtime Environment"),
            "cargo.exe" | "rustc.exe" | "rust-analyzer.exe" => ("Rust Project Developers", "Rust Toolchain", "Rust Compiler & Language Server"),
            "windowsterminal.exe" | "openconsole.exe" => ("Microsoft Corporation", "Windows Terminal", "Modern Terminal Host Application"),
            "powershell.exe" | "pwsh.exe" => ("Microsoft Corporation", "PowerShell", "Command Shell & Scripting Engine"),
            "asao.exe" => ("ASAO Instrumentation", "ASAO Control Center", "System Telemetry & Process Analyzer"),
            _ => ("Developer Toolchain", "Development Environment", "Software Development Runtime / Tool"),
        };

        return ClassificationResult {
            category: ProcessCategory::Development,
            is_system_critical: false,
            is_startup: lower_name.contains("docker") || lower_name.contains("wslservice"),
            fallback_publisher: Some(pub_fallback.to_string()),
            fallback_product: Some(prod_fallback.to_string()),
            description: Some(desc.to_string()),
        };
    }

    // 5. Communication
    if matches!(
        lower_name.as_str(),
        "discord.exe"
            | "slack.exe"
            | "teams.exe"
            | "ms-teams.exe"
            | "zoom.exe"
            | "telegram.exe"
            | "whatsapp.exe"
            | "signal.exe"
            | "skype.exe"
            | "loom.exe"
    ) || lower_pub.contains("discord")
        || lower_pub.contains("slack")
        || lower_pub.contains("zoom video")
    {
        let (pub_fallback, prod_fallback) = if lower_name.contains("discord") {
            ("Discord Inc.", "Discord")
        } else if lower_name.contains("teams") {
            ("Microsoft Corporation", "Microsoft Teams")
        } else if lower_name.contains("slack") {
            ("Slack Technologies LLC", "Slack")
        } else if lower_name.contains("zoom") {
            ("Zoom Video Communications, Inc.", "Zoom Workplace")
        } else {
            ("Messaging Provider", "Communication Client")
        };

        return ClassificationResult {
            category: ProcessCategory::Communication,
            is_system_critical: false,
            is_startup: true,
            fallback_publisher: Some(pub_fallback.to_string()),
            fallback_product: Some(prod_fallback.to_string()),
            description: Some("Real-Time Voice & Messaging Client".to_string()),
        };
    }

    // 6. Gaming
    if matches!(
        lower_name.as_str(),
        "steam.exe"
            | "steamwebhelper.exe"
            | "steamservice.exe"
            | "epicgameslauncher.exe"
            | "epicwebhelper.exe"
            | "galaxyclient.exe"
            | "battle.net.exe"
            | "riotclientservices.exe"
            | "xboxappservices.exe"
            | "gamingservices.exe"
            | "gamingservicesnet.exe"
            | "gamebar.exe"
            | "gamebarpresencewriter.exe"
            | "eaapp.exe"
            | "eabackgroundservice.exe"
    ) || lower_pub.contains("valve")
        || lower_pub.contains("epic games")
        || lower_pub.contains("riot games")
        || lower_path.contains("\\steam\\")
    {
        let (pub_fallback, prod_fallback) = if lower_name.contains("steam") {
            ("Valve Corporation", "Steam Client")
        } else if lower_name.contains("epic") {
            ("Epic Games, Inc.", "Epic Games Launcher")
        } else if lower_name.contains("gamebar") || lower_name.contains("gamingservices") || lower_name.contains("xbox") {
            ("Microsoft Corporation", "Xbox Gaming Services")
        } else {
            ("Gaming Platform", "Game Platform Runtime")
        };

        return ClassificationResult {
            category: ProcessCategory::Gaming,
            is_system_critical: false,
            is_startup: true,
            fallback_publisher: Some(pub_fallback.to_string()),
            fallback_product: Some(prod_fallback.to_string()),
            description: Some("Gaming Platform & Overlay Runtime".to_string()),
        };
    }

    // 7. Productivity
    if matches!(
        lower_name.as_str(),
        "winword.exe"
            | "excel.exe"
            | "powerpnt.exe"
            | "outlook.exe"
            | "onenote.exe"
            | "officeclicktorun.exe"
            | "onedrive.exe"
            | "filecoauth.exe"
            | "notion.exe"
            | "obsidian.exe"
            | "acrord32.exe"
            | "acrobat.exe"
            | "adobeipcbroker.exe"
            | "cclibrary.exe"
            | "ccxprocess.exe"
            | "coresync.exe"
            | "spotify.exe"
            | "figma.exe"
            | "dropbox.exe"
            | "powertoys.exe"
            | "powertoys.runner.exe"
            | "sharex.exe"
    ) || lower_pub.contains("adobe")
        || lower_pub.contains("notion")
        || lower_pub.contains("spotify")
        || lower_prod.contains("microsoft 365")
        || lower_prod.contains("microsoft office")
    {
        return ClassificationResult {
            category: ProcessCategory::Productivity,
            is_system_critical: false,
            is_startup: lower_name.contains("onedrive") || lower_name.contains("clicktorun") || lower_name.contains("adobe"),
            fallback_publisher: publisher.map(|s| s.to_string()).or_else(|| Some("Productivity Software Vendor".to_string())),
            fallback_product: product_name.map(|s| s.to_string()).or_else(|| Some("Workspace & Productivity Suite".to_string())),
            description: Some("Productivity & Workspace Application".to_string()),
        };
    }

    // 8. Check if path is inside C:\Windows\ -> classify as WindowsCore
    if lower_path.starts_with("c:\\windows\\") || lower_pub.contains("microsoft windows") {
        let is_crit = lower_path.starts_with("c:\\windows\\system32\\");
        return ClassificationResult {
            category: ProcessCategory::WindowsCore,
            is_system_critical: is_crit,
            is_startup: is_crit,
            fallback_publisher: Some("Microsoft Corporation".to_string()),
            fallback_product: Some("Microsoft® Windows® Operating System".to_string()),
            description: Some("Windows System Service / Background Host".to_string()),
        };
    }

    // 9. Unknown fallback (do not make unsafe assumptions)
    ClassificationResult {
        category: ProcessCategory::Unknown,
        is_system_critical: false,
        is_startup: false,
        fallback_publisher: None,
        fallback_product: None,
        description: Some("Unclassified User or Background Process".to_string()),
    }
}
