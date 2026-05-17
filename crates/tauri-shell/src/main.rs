#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod insertion;
mod platform;
mod session;

use std::path::PathBuf;

use pipeline::download::stream_to_sink;
use pipeline::install::{DownloadSink, InstallationState, check_installation, has_disk_space};
use pipeline::models::{Model, Registry};
use serde::Serialize;
use specta::Type;
use specta_typescript::{BigIntExportBehavior, Typescript};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, State, WindowEvent};
use tauri_plugin_positioner::{Position, WindowExt};
use tauri_specta::{Builder, Event, collect_commands, collect_events};

use session::{Session, SessionError, StopFn};

const MODELS_JSON: &str = include_str!("../../../models.json");
const DOWNLOAD_DISK_HEADROOM_BYTES: u64 = 1024 * 1024 * 1024;

fn parse_registry() -> Result<Registry, String> {
    Registry::from_json(MODELS_JSON).map_err(|err| err.to_string())
}

fn models_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app.path().app_data_dir().map_err(|err| err.to_string())?;
    let dir = base.join("models");
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    Ok(dir)
}

#[derive(Clone, Serialize, Type, Event)]
pub struct ModelDownloadProgress {
    pub model_id: String,
    pub downloaded: u64,
    pub total: u64,
}

#[derive(Clone, Serialize, Type, Event)]
pub struct ModelDownloadCompleted {
    pub model_id: String,
}

#[derive(Clone, Serialize, Type, Event)]
pub struct ModelDownloadFailed {
    pub model_id: String,
    pub message: String,
}

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

#[tauri::command]
#[specta::specta]
fn installation_status(app: AppHandle) -> Result<InstallationState, String> {
    let registry = parse_registry()?;
    let dir = models_dir(&app)?;
    Ok(check_installation(&registry, &dir))
}

#[tauri::command]
#[specta::specta]
fn list_models() -> Result<Vec<Model>, String> {
    Ok(parse_registry()?.models)
}

#[tauri::command]
#[specta::specta]
async fn download_model(app: AppHandle, model_id: String) -> Result<(), String> {
    let registry = parse_registry()?;
    let model = registry
        .find_by_id(&model_id)
        .ok_or_else(|| format!("unknown model id: {model_id}"))?
        .clone();
    let dir = models_dir(&app)?;

    if !has_disk_space(&dir, model.size_bytes + DOWNLOAD_DISK_HEADROOM_BYTES)
        .map_err(|err| err.to_string())?
    {
        return Err("insufficient disk space (model size plus 1 GB headroom required)".into());
    }

    let partial = dir.join(format!("{}.partial", model.filename));
    let final_path = dir.join(&model.filename);

    let app_for_task = app.clone();
    let model_id_for_task = model_id.clone();
    let registry_for_task = registry;
    let model_for_task = model.clone();

    let result = tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let (sink, offset) = if partial.exists() {
            DownloadSink::resume(partial.clone()).map_err(|err| err.to_string())?
        } else {
            (
                DownloadSink::create(partial.clone()).map_err(|err| err.to_string())?,
                0u64,
            )
        };

        let app_for_progress = app_for_task.clone();
        let model_id_for_progress = model_id_for_task.clone();
        let total = model_for_task.size_bytes;

        let sink = stream_to_sink(
            &model_for_task.url,
            sink,
            offset,
            &registry_for_task,
            move |downloaded| {
                let _ = ModelDownloadProgress {
                    model_id: model_id_for_progress.clone(),
                    downloaded,
                    total,
                }
                .emit(&app_for_progress);
            },
        )
        .map_err(|err| err.to_string())?;

        sink.finalise_and_rename(&final_path, &model_for_task.sha256)
            .map_err(|err| err.to_string())?;

        Ok(())
    })
    .await
    .map_err(|err| err.to_string())?;

    match result {
        Ok(()) => {
            let _ = ModelDownloadCompleted {
                model_id: model_id.clone(),
            }
            .emit(&app);
            Ok(())
        }
        Err(err) => {
            let _ = ModelDownloadFailed {
                model_id: model_id.clone(),
                message: err.clone(),
            }
            .emit(&app);
            Err(err)
        }
    }
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
    let builder = Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            show_settings,
            show_pill,
            show_overlay,
            hide_pill,
            hide_overlay,
            start_dictation_session,
            end_dictation_session,
            cancel_dictation_session,
            installation_status,
            list_models,
            download_model,
        ])
        .events(collect_events![
            ModelDownloadProgress,
            ModelDownloadCompleted,
            ModelDownloadFailed,
        ]);

    #[cfg(debug_assertions)]
    {
        let bindings_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../modules/app/src/shared/ipc/bindings.ts",
        );
        builder
            .export(
                Typescript::default().bigint(BigIntExportBehavior::Number),
                bindings_path,
            )
            .expect("failed to export typescript bindings");
    }

    if std::env::args().any(|a| a == "--export-bindings-only") {
        return;
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
