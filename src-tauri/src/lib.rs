mod codex;
mod core;
mod detection;

use std::sync::Mutex;

use core::{sanitize_for_frontend, ProviderId, UsageSnapshot};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WindowEvent,
};

#[tauri::command]
fn get_usage_snapshots(state: tauri::State<'_, AppState>) -> Vec<UsageSnapshot> {
    detection::detect_installed_clients()
        .into_iter()
        .filter(|client| client.provider == ProviderId::Codex)
        .filter_map(|client| {
            let mut cache = state.codex_cache.lock().ok()?;
            Some(sanitize_for_frontend(codex::refresh_snapshot(
                &mut cache,
                &client.executable,
            )))
        })
        .collect()
}

struct AppState {
    codex_cache: Mutex<codex::SnapshotCache>,
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            codex_cache: Mutex::new(codex::SnapshotCache::default()),
        })
        .setup(|app| {
            core::log_redacted("QuotaBuddy native shell initialized");
            let open = MenuItem::with_id(app, "open", "Open QuotaBuddy", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;

            let _tray = TrayIconBuilder::with_id("quotabuddy-tray")
                .tooltip("QuotaBuddy — local usage monitor")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => show_main_window(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main_window(tray.app_handle());
                    }
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_usage_snapshots])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running QuotaBuddy");
}
