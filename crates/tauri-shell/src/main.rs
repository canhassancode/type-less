#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod engines;
mod insertion;
mod platform;
mod session;

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

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

use engines::{EngineHandles, EngineState, load_asr_from, load_cleanup_from, spawn_loaders};
use session::{DictationEvent, DictationStage, ErrorStage, Session, SessionError, StopFn};

const MODELS_JSON: &str = include_str!("../../../models.json");
const DOWNLOAD_DISK_HEADROOM_BYTES: u64 = 1024 * 1024 * 1024;
const CLEANUP_PROMPT: &str = include_str!("../../pipeline/prompts/cleanup_v1.txt");
const ASR_MODEL_FILENAME: &str = "ggml-small.en.bin";
const CLEANUP_MODEL_FILENAME: &str = "qwen2.5-1.5b-instruct-q4_k_m.gguf";

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

#[derive(Clone, Serialize, Type, Event)]
pub struct DictationStateChanged {
    pub stage: DictationStage,
}

#[derive(Clone, Serialize, Type, Event)]
pub struct DictationCompleted {
    pub word_count: u32,
}

#[derive(Clone, Serialize, Type, Event)]
pub struct DictationError {
    pub stage: ErrorStage,
    pub message: String,
}

#[derive(Clone, Serialize, Type, Event)]
pub struct EngineStateChanged {
    pub state: EngineState,
}

fn emit_dictation_event(app: &AppHandle, event: DictationEvent) {
    match event {
        DictationEvent::StateChanged(stage) => {
            let _ = DictationStateChanged { stage }.emit(app);
        }
        DictationEvent::Completed { word_count } => {
            let _ = DictationCompleted { word_count }.emit(app);
        }
        DictationEvent::Error { stage, message } => {
            let _ = DictationError { stage, message }.emit(app);
        }
    }
}

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
async fn end_dictation_session(app: AppHandle) -> Result<(), String> {
    let app_for_task = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let session: State<Session> = app_for_task.state();
        if let Err(err) = session.stop() {
            eprintln!("[dictation] stop failed: {err:?}");
        }
        let _ = set_pill_visible(&app_for_task, false);
    });
    Ok(())
}

#[tauri::command]
#[specta::specta]
fn cancel_dictation_session(state: State<'_, Session>) -> Result<(), String> {
    state.cancel().map_err(|err| format!("{err:?}"))
}

#[tauri::command]
#[specta::specta]
async fn installation_status(app: AppHandle) -> Result<InstallationState, String> {
    let dir = models_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || -> Result<InstallationState, String> {
        let registry = parse_registry()?;
        Ok(check_installation(&registry, &dir))
    })
    .await
    .map_err(|err| err.to_string())?
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

fn build_session(handles: EngineHandles, app_slot: Arc<OnceLock<AppHandle>>) -> Session {
    let asr_handles = handles.clone();
    let cleanup_handles = handles;
    Session::new(
        || {
            pipeline::audio::start()
                .map(|audio| Box::new(move || audio.stop()) as StopFn)
                .map_err(|err| SessionError::Audio(err.to_string()))
        },
        move |samples| {
            let engine = asr_handles
                .try_asr()
                .ok_or_else(|| SessionError::EngineNotReady("asr".into()))?;
            engine
                .transcribe(samples)
                .map_err(|err| SessionError::Asr(err.to_string()))
        },
        move |transcript| {
            let engine = cleanup_handles
                .try_cleanup()
                .ok_or_else(|| SessionError::EngineNotReady("cleanup".into()))?;
            engine
                .cleanup(transcript)
                .map_err(|err| SessionError::Cleanup(err.to_string()))
        },
        |text| insertion::paste(text).map_err(|err| SessionError::Paste(err.to_string())),
        move |event| {
            if let Some(app) = app_slot.get() {
                emit_dictation_event(app, event);
            }
        },
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
            DictationStateChanged,
            DictationCompleted,
            DictationError,
            EngineStateChanged,
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

    let engine_handles = EngineHandles::new();
    let handles_for_loaders = engine_handles.clone();
    let app_handle_slot: Arc<OnceLock<AppHandle>> = Arc::new(OnceLock::new());
    let slot_for_setup = app_handle_slot.clone();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(engine_handles.clone())
        .manage(build_session(engine_handles, app_handle_slot))
        .invoke_handler(builder.invoke_handler())
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "settings" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(move |app| {
            builder.mount_events(app);
            let _ = slot_for_setup.set(app.handle().clone());

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
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(move |app_handle, event| {
        if matches!(event, tauri::RunEvent::Ready) {
            match models_dir(app_handle) {
                Ok(dir) => {
                    let app_for_engine_sink = app_handle.clone();
                    spawn_loaders(
                        handles_for_loaders.clone(),
                        load_asr_from(dir.join(ASR_MODEL_FILENAME)),
                        load_cleanup_from(dir.join(CLEANUP_MODEL_FILENAME), CLEANUP_PROMPT),
                        Arc::new(move |state| {
                            eprintln!("[engines] state: {state:?}");
                            let _ = EngineStateChanged { state }.emit(&app_for_engine_sink);
                        }),
                    );
                }
                Err(err) => eprintln!("[engines] could not resolve models_dir: {err}"),
            }
        }
    });
}
