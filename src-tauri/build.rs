/// Embeds the bundled Win11Debloat registry files, so ASAO needs nothing on disk to apply them.
fn embed_win11debloat_regfiles() {
    println!("cargo:rerun-if-changed=resources/win11debloat");
    let dir = std::path::Path::new("resources/win11debloat/Regfiles");
    let mut entries: Vec<String> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().map(|x| x.eq_ignore_ascii_case("reg")).unwrap_or(false) {
                let name = e.file_name().to_string_lossy().to_string();
                let abs = std::fs::canonicalize(&p).expect("registry file path");
                entries.push(format!("    ({:?}, include_bytes!({:?})),", name, abs.to_string_lossy()));
            }
        }
    }
    entries.sort();
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("w11d_regfiles.rs");
    std::fs::write(
        out,
        format!("pub static REGFILES: &[(&str, &[u8])] = &[\n{}\n];\n", entries.join("\n")),
    )
    .expect("write w11d_regfiles.rs");
}

/// Takes the accounts API address from `.env` in the project root, so it needn't be set in
/// every terminal. It is a public web address, not a secret, so release builds bake it in
/// too. A variable set in the terminal wins.
fn load_dotenv() {
    const KEYS: [&str; 1] = ["ASAO_ACCOUNTS_URL"];
    let path = std::path::Path::new("../.env");
    println!("cargo:rerun-if-changed={}", path.display());
    let Ok(text) = std::fs::read_to_string(path) else { return };
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let key = key.trim().trim_start_matches("export ").trim();
        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
            .unwrap_or(value);
        if KEYS.contains(&key) && !value.is_empty() && std::env::var_os(key).is_none() {
            println!("cargo:rustc-env={key}={value}");
        }
    }
}

fn main() {
    embed_win11debloat_regfiles();

    // The accounts API address is baked in at build time (read with option_env!).
    println!("cargo:rerun-if-env-changed=ASAO_ACCOUNTS_URL");
    load_dotenv();

    // Debug builds (`tauri dev`) can't be launched by cargo when the exe demands elevation
    // (os error 740), so they keep the default manifest. In dev, run the terminal as
    // administrator to get the fast MFT scan; otherwise the app uses the parallel walk.
    if std::env::var("PROFILE").map(|p| p != "release").unwrap_or(true) {
        tauri_build::build();
        return;
    }

    // Release builds ask Windows for administrator rights at launch, which the NTFS MFT
    // scan needs.
    let windows = tauri_build::WindowsAttributes::new().app_manifest(
        r#"
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false" />
      </requestedPrivileges>
    </security>
  </trustInfo>
  <dependency>
    <dependentAssembly>
      <assemblyIdentity
        type="win32"
        name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0"
        processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df"
        language="*"
      />
    </dependentAssembly>
  </dependency>
</assembly>
"#,
    );
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("failed to run tauri build script");
}
