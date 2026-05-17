#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod insertion;
mod platform;

use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WindowEvent};
use tauri_plugin_positioner::{Position, WindowExt};

#[tauri::command]
fn show_settings(app: AppHandle) -> Result<(), String> {
    let window = window(&app, "settings")?;
    window.show().map_err(stringify)?;
    window.set_focus().map_err(stringify)?;
    Ok(())
}

#[tauri::command]
fn show_pill(app: AppHandle) -> Result<(), String> {
    let window = window(&app, "pill")?;
    window.move_window(Position::BottomCenter).map_err(stringify)?;
    window.show().map_err(stringify)?;
    Ok(())
}

#[tauri::command]
fn show_overlay(app: AppHandle) -> Result<(), String> {
    let window = window(&app, "overlay")?;
    window.move_window(Position::TopCenter).map_err(stringify)?;
    window.show().map_err(stringify)?;
    Ok(())
}

#[tauri::command]
fn hide_pill(app: AppHandle) -> Result<(), String> {
    window(&app, "pill")?.hide().map_err(stringify)
}

#[tauri::command]
fn hide_overlay(app: AppHandle) -> Result<(), String> {
    window(&app, "overlay")?.hide().map_err(stringify)
}

fn window(app: &AppHandle, label: &str) -> Result<tauri::WebviewWindow, String> {
    app.get_webview_window(label)
        .ok_or_else(|| format!("window not found: {label}"))
}

fn stringify<E: std::fmt::Display>(error: E) -> String {
    error.to_string()
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_positioner::init())
        .invoke_handler(tauri::generate_handler![
            show_settings,
            show_pill,
            show_overlay,
            hide_pill,
            hide_overlay
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "settings" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            if let Some(pill) = app.get_webview_window("pill") {
                pill.set_ignore_cursor_events(true)?;
            }
            if let Some(overlay) = app.get_webview_window("overlay") {
                overlay.set_ignore_cursor_events(true)?;
            }

            let icon = app
                .default_window_icon()
                .cloned()
                .ok_or("missing default window icon")?;

            TrayIconBuilder::with_id("main")
                .icon(icon)
                .icon_as_template(true)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let handle = tray.app_handle();
                        if let Some(window) = handle.get_webview_window("settings") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
