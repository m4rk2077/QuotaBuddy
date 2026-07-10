mod codex;
mod core;
mod detection;
mod monitor_controls;

use std::{fs, sync::Mutex, thread, time::Duration};

use core::{sanitize_for_frontend, ProviderId, UsageSnapshot};
use monitor_controls::{crossing_alerts, validate_preferences, AlertTracker, MonitorPreferences};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WindowEvent,
};
use tauri_plugin_autostart::ManagerExt as AutostartManagerExt;
use tauri_plugin_notification::NotificationExt;

#[tauri::command]
fn get_usage_snapshots(app: tauri::AppHandle) -> Vec<UsageSnapshot> {
    refresh_usage(&app)
}

fn refresh_usage(app: &tauri::AppHandle) -> Vec<UsageSnapshot> {
    let state = app.state::<AppState>();
    let snapshots: Vec<_> = detection::detect_installed_clients()
        .into_iter()
        .filter(|client| client.provider == ProviderId::Codex)
        .filter_map(|client| {
            let mut cache = state.codex_cache.lock().ok()?;
            Some(sanitize_for_frontend(codex::refresh_snapshot(
                &mut cache,
                &client.executable,
            )))
        })
        .collect();

    let preferences = state
        .preferences
        .lock()
        .map(|value| value.clone())
        .unwrap_or_default();
    update_tray_tooltip(app, &snapshots, &preferences);
    if let Ok(mut tracker) = state.alerts.lock() {
        for snapshot in &snapshots {
            for (label, threshold) in
                crossing_alerts(&mut tracker, snapshot, &preferences.alert_thresholds)
            {
                let (title, body) = alert_message(&preferences, &label, threshold);
                let _ = app.notification().builder().title(title).body(body).show();
            }
        }
    }
    snapshots
}

fn alert_message(preferences: &MonitorPreferences, label: &str, threshold: u8) -> (String, String) {
    match preferences.language {
        monitor_controls::Language::En => (
            "QuotaBuddy alert".to_owned(),
            format!("{label} reached {threshold}% usage."),
        ),
        monitor_controls::Language::PtBr => (
            "Alerta do QuotaBuddy".to_owned(),
            format!("{label} atingiu {threshold}% de uso."),
        ),
    }
}

struct AppState {
    codex_cache: Mutex<codex::SnapshotCache>,
    preferences: Mutex<MonitorPreferences>,
    alerts: Mutex<AlertTracker>,
}

fn preferences_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let directory = app
        .path()
        .app_config_dir()
        .map_err(|error| error.to_string())?;
    Ok(directory.join("monitor-preferences.json"))
}

fn load_preferences(app: &tauri::AppHandle) -> MonitorPreferences {
    let Ok(path) = preferences_path(app) else {
        return MonitorPreferences::default();
    };
    let Ok(source) = fs::read_to_string(path) else {
        return MonitorPreferences::default();
    };
    serde_json::from_str(&source)
        .ok()
        .and_then(|value| validate_preferences(value).ok())
        .unwrap_or_default()
}

fn save_preferences(
    app: &tauri::AppHandle,
    preferences: &MonitorPreferences,
) -> Result<(), String> {
    let path = preferences_path(app)?;
    if let Some(directory) = path.parent() {
        fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    }
    let serialized =
        serde_json::to_string_pretty(preferences).map_err(|error| error.to_string())?;
    fs::write(path, serialized).map_err(|error| error.to_string())
}

fn update_tray_tooltip(
    app: &tauri::AppHandle,
    snapshots: &[UsageSnapshot],
    preferences: &MonitorPreferences,
) {
    let Some(snapshot) = snapshots
        .iter()
        .find(|snapshot| snapshot.provider == ProviderId::Codex)
    else {
        return;
    };
    let labels: Vec<_> = preferences
        .pinned_metrics
        .iter()
        .filter_map(|kind| snapshot.metrics.iter().find(|metric| metric.kind == *kind))
        .filter_map(|metric| {
            metric.remaining.as_deref().map(|remaining| {
                format!(
                    "{}: {remaining}",
                    localized_metric_label(preferences, metric.kind)
                )
            })
        })
        .collect();
    if let Some(tray) = app.tray_by_id("quotabuddy-tray") {
        let tooltip = if labels.is_empty() {
            match preferences.language {
                monitor_controls::Language::En => "QuotaBuddy — local usage monitor".to_owned(),
                monitor_controls::Language::PtBr => "QuotaBuddy — monitor local de uso".to_owned(),
            }
        } else {
            format!("QuotaBuddy — {}", labels.join(" | "))
        };
        let _ = tray.set_tooltip(Some(&tooltip));
    }
}

fn localized_metric_label(
    preferences: &MonitorPreferences,
    kind: core::MetricKind,
) -> &'static str {
    match (preferences.language, kind) {
        (monitor_controls::Language::PtBr, core::MetricKind::Session) => "Limite de sessão",
        (monitor_controls::Language::PtBr, core::MetricKind::Cycle) => "Limite mais longo",
        (_, core::MetricKind::Session) => "Session limit",
        (_, core::MetricKind::Cycle) => "Longer limit",
        (_, _) => "Codex usage",
    }
}

#[tauri::command]
fn get_monitor_preferences(state: tauri::State<'_, AppState>) -> MonitorPreferences {
    state
        .preferences
        .lock()
        .map(|value| value.clone())
        .unwrap_or_default()
}

#[tauri::command]
fn save_monitor_preferences(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    preferences: MonitorPreferences,
) -> Result<MonitorPreferences, String> {
    let preferences = validate_preferences(preferences)?;
    save_preferences(&app, &preferences)?;
    if preferences.start_with_windows {
        app.autolaunch()
            .enable()
            .map_err(|error| error.to_string())?;
    } else {
        app.autolaunch()
            .disable()
            .map_err(|error| error.to_string())?;
    }
    *state
        .preferences
        .lock()
        .map_err(|_| "Preferences are unavailable.".to_owned())? = preferences.clone();
    Ok(preferences)
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
            preferences: Mutex::new(MonitorPreferences::default()),
            alerts: Mutex::new(AlertTracker::default()),
        })
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let preferences = load_preferences(app.handle());
            if preferences.start_with_windows {
                let _ = app.autolaunch().enable();
            }
            if let Some(state) = app.try_state::<AppState>() {
                if let Ok(mut stored) = state.preferences.lock() {
                    *stored = preferences;
                }
            }
            let handle = app.handle().clone();
            thread::spawn(move || loop {
                let _ = refresh_usage(&handle);
                thread::sleep(Duration::from_secs(5 * 60));
            });
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
        .invoke_handler(tauri::generate_handler![
            get_usage_snapshots,
            get_monitor_preferences,
            save_monitor_preferences
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running QuotaBuddy");
}
