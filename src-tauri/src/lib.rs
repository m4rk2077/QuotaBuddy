mod codex;
mod core;
mod detection;
mod diagnostics;
mod monitor_controls;
mod popup_position;
mod spend;
mod tray_icons;
mod tray_interaction;
mod tray_presentation;
mod windows_backdrop;

#[cfg(test)]
mod accessibility;

use std::{
    fs,
    sync::{Arc, Mutex},
};

use core::{sanitize_for_frontend, ProviderId, UsageSnapshot};
use diagnostics::write_diagnostic_export;
use monitor_controls::{crossing_alerts, validate_preferences, AlertTracker, MonitorPreferences};
use spend::{bundled_pricing_table, estimate_spend, SpendEstimate, SpendScanner};
use std::path::PathBuf;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, PhysicalPosition, PhysicalSize, WindowEvent,
};
use tauri_plugin_autostart::ManagerExt as AutostartManagerExt;
use tauri_plugin_notification::NotificationExt;
use tray_interaction::{PanelAction, TrayClickTracker};
use tray_presentation::{build_tray_presentation, TrayPresentationTracker};

#[tauri::command]
async fn get_usage_snapshots(app: tauri::AppHandle) -> Result<Vec<UsageSnapshot>, String> {
    tauri::async_runtime::spawn_blocking(move || refresh_usage(&app))
        .await
        .map_err(|_| "Usage refresh task failed.".to_owned())
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
    update_tray_presentation(app, &snapshots, &preferences);
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
    backdrop: Mutex<windows_backdrop::BackdropMode>,
    last_tray_rect: Mutex<Option<popup_position::PhysicalRect>>,
    tray_presentation: Mutex<TrayPresentationTracker>,
    tray_click: Mutex<TrayClickTracker>,
    spend_scanner: Arc<Mutex<SpendScanner>>,
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

fn update_tray_presentation(
    app: &tauri::AppHandle,
    snapshots: &[UsageSnapshot],
    preferences: &MonitorPreferences,
) {
    let Some(tray) = app.tray_by_id("quotabuddy-tray") else {
        return;
    };
    let next = build_tray_presentation(snapshots, preferences);
    if let Ok(mut tracker) = app.state::<AppState>().tray_presentation.lock() {
        if tracker
            .apply_icon_if_changed(&next, |key| tray.set_icon(Some(tray_icons::image_for(key))))
            .is_err()
        {
            core::log_redacted("QuotaBuddy tray icon update failed");
        }
        if tracker
            .apply_tooltip_if_changed(&next, |tooltip| tray.set_tooltip(Some(tooltip)))
            .is_err()
        {
            core::log_redacted("QuotaBuddy tray tooltip update failed");
        }
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

fn codex_log_directory() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(|home| PathBuf::from(home).join(".codex").join("sessions"))
}

fn local_spend_estimate(scanner: &mut SpendScanner) -> Result<SpendEstimate, String> {
    let records = match codex_log_directory() {
        Some(path) => scanner
            .read_usage_records(&path)
            .map_err(|error| error.to_string())?,
        None => Vec::new(),
    };
    Ok(estimate_spend(&records, &bundled_pricing_table()))
}

#[tauri::command]
async fn get_local_spend_estimate(
    state: tauri::State<'_, AppState>,
) -> Result<SpendEstimate, String> {
    let scanner = Arc::clone(&state.spend_scanner);
    tauri::async_runtime::spawn_blocking(move || {
        let mut scanner = scanner
            .lock()
            .map_err(|_| "Spend estimate is unavailable.".to_owned())?;
        local_spend_estimate(&mut scanner)
    })
    .await
    .map_err(|_| "Spend estimate task failed.".to_owned())?
}

#[tauri::command]
async fn export_redacted_diagnostics(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let destination = directory.join("quotabuddy-diagnostics-redacted.json");
    let scanner = Arc::clone(&state.spend_scanner);
    tauri::async_runtime::spawn_blocking(move || {
        let mut scanner = scanner
            .lock()
            .map_err(|_| "Diagnostics are unavailable.".to_owned())?;
        write_diagnostic_export(&destination, local_spend_estimate(&mut scanner)?)
            .map_err(|error| error.to_string())?;
        Ok(destination.display().to_string())
    })
    .await
    .map_err(|_| "Diagnostics task failed.".to_owned())?
}

#[tauri::command]
fn get_window_backdrop(state: tauri::State<'_, AppState>) -> windows_backdrop::BackdropMode {
    state.backdrop.lock().map(|mode| *mode).unwrap_or_default()
}

#[tauri::command]
fn hide_main_window(app: tauri::AppHandle) {
    hide_main_window_handle(&app);
}

fn remember_tray_rect(app: &tauri::AppHandle, rect: tauri::Rect) -> popup_position::PhysicalRect {
    // Tauri's tray events are documented and emitted in physical coordinates.
    let position = rect.position.to_physical::<i32>(1.0);
    let size = rect.size.to_physical::<u32>(1.0);
    let rect = popup_position::PhysicalRect {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    };
    if let Ok(mut stored) = app.state::<AppState>().last_tray_rect.lock() {
        *stored = Some(rect);
    }
    rect
}

fn last_tray_rect(app: &tauri::AppHandle) -> Option<popup_position::PhysicalRect> {
    app.state::<AppState>()
        .last_tray_rect
        .lock()
        .ok()
        .and_then(|rect| *rect)
}

fn live_tray_rect(app: &tauri::AppHandle) -> Option<popup_position::PhysicalRect> {
    let tray = app.tray_by_id("quotabuddy-tray")?;
    let rect = tray.rect().ok().flatten()?;
    Some(remember_tray_rect(app, rect))
}

fn apply_window_backdrop(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    let mode = windows_backdrop::apply_to_window(window);
    if let Ok(mut stored) = app.state::<AppState>().backdrop.lock() {
        *stored = mode;
    }
    let mode_name = match mode {
        windows_backdrop::BackdropMode::DesktopAcrylic => "desktop-acrylic",
        windows_backdrop::BackdropMode::Solid => "solid",
    };
    let _ = window.eval(format!(
        "document.documentElement.dataset.backdrop = '{mode_name}'"
    ));
}

fn position_main_window(
    window: &tauri::WebviewWindow,
    tray_rect: popup_position::PhysicalRect,
) -> Result<(), String> {
    let center_x = tray_rect.x + tray_rect.width as i32 / 2;
    let center_y = tray_rect.y + tray_rect.height as i32 / 2;
    let monitor = window
        .monitor_from_point(f64::from(center_x), f64::from(center_y))
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "No monitor contains the tray icon.".to_owned())?;
    let monitor_rect = popup_position::PhysicalRect {
        x: monitor.position().x,
        y: monitor.position().y,
        width: monitor.size().width,
        height: monitor.size().height,
    };
    let work_area = monitor.work_area();
    let work_rect = popup_position::PhysicalRect {
        x: work_area.position.x,
        y: work_area.position.y,
        width: work_area.size.width,
        height: work_area.size.height,
    };
    let current_scale = window.scale_factor().map_err(|error| error.to_string())?;
    let current_size = window
        .inner_size()
        .map_err(|error| error.to_string())?
        .to_logical::<f64>(current_scale);
    let panel_size_dip = (
        current_size.width.round() as u32,
        current_size.height.round() as u32,
    );
    let scale_factor = monitor.scale_factor();
    let popup_size = popup_position::popup_size_physical(panel_size_dip, scale_factor);
    let ideal = popup_position::calculate_popup_position(
        monitor_rect,
        work_rect,
        tray_rect,
        panel_size_dip,
        scale_factor,
    );
    let position = popup_position::adjust_with_windows(ideal, tray_rect, popup_size);
    window
        .set_size(PhysicalSize::new(popup_size.0, popup_size.1))
        .map_err(|error| error.to_string())?;
    window
        .set_position(PhysicalPosition::new(position.x, position.y))
        .map_err(|error| error.to_string())
}

fn show_main_window(app: &tauri::AppHandle, tray_rect: Option<popup_position::PhysicalRect>) {
    if let Some(window) = app.get_webview_window("main") {
        if let Some(rect) = tray_rect
            .or_else(|| live_tray_rect(app))
            .or_else(|| last_tray_rect(app))
        {
            let _ = position_main_window(&window, rect);
        }
        apply_window_backdrop(app, &window);
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn main_window_is_visible(app: &tauri::AppHandle) -> bool {
    app.get_webview_window("main")
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false)
}

fn handle_left_tray_button(
    app: &tauri::AppHandle,
    button_state: MouseButtonState,
    tray_rect: popup_position::PhysicalRect,
) {
    let visible = main_window_is_visible(app);
    let state = app.state::<AppState>();
    match button_state {
        MouseButtonState::Down => {
            if let Ok(mut tracker) = state.tray_click.lock() {
                tracker.left_down(visible);
            }
        }
        MouseButtonState::Up => {
            let action = state
                .tray_click
                .lock()
                .map(|mut tracker| tracker.left_up(visible))
                .unwrap_or(if visible {
                    PanelAction::Hide
                } else {
                    PanelAction::Show
                });
            if action == PanelAction::Hide {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            } else {
                show_main_window(app, Some(tray_rect));
            }
        }
    }
}

fn hide_main_window_handle(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            codex_cache: Mutex::new(codex::SnapshotCache::default()),
            preferences: Mutex::new(MonitorPreferences::default()),
            alerts: Mutex::new(AlertTracker::default()),
            backdrop: Mutex::new(windows_backdrop::BackdropMode::Solid),
            last_tray_rect: Mutex::new(None),
            tray_presentation: Mutex::new(TrayPresentationTracker::default()),
            tray_click: Mutex::new(TrayClickTracker::default()),
            spend_scanner: Arc::new(Mutex::new(SpendScanner::default())),
        })
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let preferences = load_preferences(app.handle());
            let initial_tray_presentation = build_tray_presentation(&[], &preferences);
            if let Some(window) = app.get_webview_window("main") {
                apply_window_backdrop(app.handle(), &window);
            }
            if preferences.start_with_windows {
                let _ = app.autolaunch().enable();
            }
            if let Some(state) = app.try_state::<AppState>() {
                if let Ok(mut stored) = state.preferences.lock() {
                    *stored = preferences.clone();
                }
            }
            core::log_redacted("QuotaBuddy native shell initialized");
            let open = MenuItem::with_id(app, "open", "Open QuotaBuddy", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;

            let _tray = TrayIconBuilder::with_id("quotabuddy-tray")
                .icon(tray_icons::image_for(initial_tray_presentation.icon_key))
                .tooltip(&initial_tray_presentation.tooltip)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => show_main_window(app, None),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        rect,
                        button,
                        button_state,
                        ..
                    } = event
                    {
                        let rect = remember_tray_rect(tray.app_handle(), rect);
                        if button == MouseButton::Left {
                            handle_left_tray_button(tray.app_handle(), button_state, rect);
                        }
                    }
                })
                .build(app)?;

            if let Some(state) = app.try_state::<AppState>() {
                if let Ok(mut tracker) = state.tray_presentation.lock() {
                    tracker.confirm_applied(&initial_tray_presentation);
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_usage_snapshots,
            get_monitor_preferences,
            save_monitor_preferences,
            get_local_spend_estimate,
            export_redacted_diagnostics,
            get_window_backdrop,
            hide_main_window
        ])
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = window.hide();
            }
            WindowEvent::Focused(false) => {
                let _ = window.hide();
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running QuotaBuddy");
}
