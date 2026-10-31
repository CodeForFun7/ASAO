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

fn main() {
    embed_win11debloat_regfiles();

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
