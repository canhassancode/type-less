#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod insertion;
mod platform;
mod session;

use specta_typescript::Typescript;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, State, WindowEvent};
use tauri_plugin_positioner::{Position, WindowExt};
use tauri_specta::{Builder, collect_commands};

use session::{Session, SessionError, StopFn};

const TRACER_PASTE_TEXT: &str = "Hello, type-less!";

#[tauri::command]
#[specta::specta]
fn show_settings(app: AppHandle) -> Result<(), String> {
    let window = window(&app, "settings")?;
    window.show().map_err(stringify)?;
    window.set_focus().map_err(stringify)?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
fn show_pill(app: AppHandle) -> Result<(), String> {
    set_pill_visible(&app, true)
}

#[tauri::command]
#[specta::specta]
fn show_overlay(app: AppHandle) -> Result<(), String> {
    let window = window(&app, "overlay")?;
    window.move_window(Position::TopCenter).map_err(stringify)?;
    window.show().map_err(stringify)?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
fn hide_pill(app: AppHandle) -> Result<(), String> {
    set_pill_visible(&app, false)
}

#[tauri::command]
#[specta::specta]
fn hide_overlay(app: AppHandle) -> Result<(), String> {
    window(&app, "overlay")?.hide().map_err(stringify)
}

#[tauri::command]
#[specta::specta]
fn start_dictation_session(app: AppHandle, state: State<'_, Session>) -> Result<(), String> {
    state.start().map_err(|err| format!("{err:?}"))?;
    set_pill_visible(&app, true)
}

#[tauri::command]
#[specta::specta]
fn end_dictation_session(app: AppHandle, state: State<'_, Session>) -> Result<(), String> {
    let stop_result = state
        .stop(TRACER_PASTE_TEXT)
        .map_err(|err| format!("{err:?}"));
    let _ = set_pill_visible(&app, false);
    stop_result
}

#[tauri::command]
#[specta::specta]
fn cancel_dictation_session(state: State<'_, Session>) -> Result<(), String> {
    state.cancel().map_err(|err| format!("{err:?}"))
}

fn set_pill_visible(app: &AppHandle, visible: bool) -> Result<(), String> {
    let window = window(app, "pill")?;
    if visible {
        window.move_window(Position::BottomCenter).map_err(stringify)?;
        window.show().map_err(stringify)?;
    } else {
        window.hide().map_err(stringify)?;
    }
    Ok(())
}

fn window(app: &AppHandle, label: &str) -> Result<tauri::WebviewWindow, String> {
    app.get_webview_window(label)
        .ok_or_else(|| format!("window not found: {label}"))
}

fn stringify<E: std::fmt::Display>(error: E) -> String {
    error.to_string()
}

fn build_session() -> Session {
    Session::new(
        || {
            pipeline::audio::start()
                .map(|audio| Box::new(move || audio.stop()) as StopFn)
                .map_err(|err| SessionError::Audio(err.to_string()))
        },
        |text| insertion::paste(text).map_err(|err| SessionError::Paste(err.to_string())),
    )
}

fn main() {
    let builder = Builder::<tauri::Wry>::new().commands(collect_commands![
        show_settings,
        show_pill,
        show_overlay,
        hide_pill,
        hide_overlay,
        start_dictation_session,
        end_dictation_session,
        cancel_dictation_session,
    ]);

    #[cfg(debug_assertions)]
    {
        let bindings_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../modules/app/src/shared/ipc/bindings.ts",
        );
        builder
            .export(Typescript::default(), bindings_path)
            .expect("failed to export typescript bindings");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(build_session())
        .invoke_handler(builder.invoke_handler())
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
