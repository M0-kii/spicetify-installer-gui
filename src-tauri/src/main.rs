#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod installer;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{ipc::Channel, Manager, State};

#[tauri::command]
async fn inspect_system() -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(installer::inspect)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn install_spicetify(
    marketplace: bool,
    on_event: Channel<String>,
    state: State<'_, Arc<AtomicBool>>,
) -> Result<(), String> {
    if state
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("An installation is already running.".into());
    }
    let result = tauri::async_runtime::spawn_blocking(move || {
        installer::install(marketplace, |line| {
            let _ = on_event.send(line.to_owned());
        })
    })
    .await
    .map_err(|e| e.to_string());
    state.store(false, Ordering::SeqCst);
    result?
}

#[tauri::command]
async fn open_spotify() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(installer::open_spotify)
        .await
        .map_err(|e| e.to_string())?
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(Arc::new(AtomicBool::new(false)))
        .invoke_handler(tauri::generate_handler![
            inspect_system,
            install_spicetify,
            open_spotify
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.state::<Arc<AtomicBool>>().load(Ordering::SeqCst) {
                    api.prevent_close();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("Could not start Spicetify Installer");
}
