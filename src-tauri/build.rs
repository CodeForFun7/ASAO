fn main() {
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
