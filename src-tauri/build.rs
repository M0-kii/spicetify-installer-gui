fn main() {
    // Explicit privileges prevent Windows from treating the installer name as a request to elevate.
    let windows = tauri_build::WindowsAttributes::new()
        .app_manifest(include_str!("windows-app-manifest.xml"));
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("Could not build Windows application resources");
}
