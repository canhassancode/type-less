use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pipeline::download::stream_to_sink;
use pipeline::install::{DownloadSink, verify_existing_sha256};
use pipeline::models::{Model, Registry};

const BUNDLE_ID: &str = "io.github.canhassancode.type-less";
const ZERO_SHA: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("bootstrap-models: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let models_json_path = workspace_root().join("models.json");
    let json = fs::read_to_string(&models_json_path).map_err(stringify)?;
    let mut registry = Registry::from_json(&json).map_err(stringify)?;

    let dest_dir = dirs::data_dir()
        .ok_or_else(|| "could not resolve OS data dir".to_string())?
        .join(BUNDLE_ID)
        .join("models");
    fs::create_dir_all(&dest_dir).map_err(stringify)?;

    println!("bootstrap-models: dest dir = {}", dest_dir.display());

    for model in &mut registry.models {
        let final_path = dest_dir.join(&model.filename);
        let (actual_sha, actual_size) = ensure_present(model, &registry_snapshot(model), &final_path)?;
        model.sha256 = actual_sha;
        model.size_bytes = actual_size;
    }

    let serialised = serde_json::to_string_pretty(&registry).map_err(stringify)?;
    fs::write(&models_json_path, format!("{serialised}\n")).map_err(stringify)?;
    println!("bootstrap-models: wrote {}", models_json_path.display());
    Ok(())
}

fn ensure_present(
    model: &Model,
    single_model_registry: &Registry,
    final_path: &Path,
) -> Result<(String, u64), String> {
    println!("\n[{}] {}", model.id, model.url);

    if final_path.exists() {
        let actual_size = fs::metadata(final_path).map_err(stringify)?.len();
        if model.sha256 != ZERO_SHA
            && verify_existing_sha256(final_path, &model.sha256).map_err(stringify)?
        {
            println!("  already installed (sha matches), skipping download");
            return Ok((model.sha256.clone(), actual_size));
        }
        let recorded = compute_sha_of(final_path)?;
        if model.sha256 == ZERO_SHA {
            println!("  already on disk, recording sha = {recorded}");
            return Ok((recorded, actual_size));
        }
        println!("  on-disk sha mismatch; will re-download (existing was {recorded})");
        fs::remove_file(final_path).map_err(stringify)?;
    }

    let partial_path = partial_for(final_path);
    let (sink, resume_from) = if partial_path.exists() {
        let (s, off) = DownloadSink::resume(partial_path.clone()).map_err(stringify)?;
        println!("  resuming from byte {off}");
        (s, off)
    } else {
        (DownloadSink::create(partial_path.clone()).map_err(stringify)?, 0)
    };

    let sink = stream_to_sink(&model.url, sink, resume_from, single_model_registry, |downloaded| {
        let total = model.size_bytes.max(downloaded);
        let pct = (downloaded as f64 / total as f64) * 100.0;
        print!("\r  downloaded {downloaded} bytes ({pct:.1}%)   ");
        let _ = std::io::stdout().flush();
    })
    .map_err(stringify)?;
    println!();

    let recorded_sha = sink.finalise_recording_sha(final_path).map_err(stringify)?;
    let actual_size = fs::metadata(final_path).map_err(stringify)?.len();
    println!("  done. sha = {recorded_sha}, size = {actual_size}");
    Ok((recorded_sha, actual_size))
}

fn registry_snapshot(model: &Model) -> Registry {
    Registry {
        models: vec![model.clone()],
    }
}

fn compute_sha_of(path: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let bytes = fs::read(path).map_err(stringify)?;
    let mut h = Sha256::new();
    h.update(&bytes);
    Ok(format!("{:x}", h.finalize()))
}

fn partial_for(final_path: &Path) -> PathBuf {
    let mut name = final_path
        .file_name()
        .map(|s| s.to_os_string())
        .unwrap_or_default();
    name.push(".partial");
    final_path.with_file_name(name)
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn stringify<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}
