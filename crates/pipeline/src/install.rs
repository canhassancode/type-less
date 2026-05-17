use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::models::Registry;

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },
    #[error("disk space check failed: {0}")]
    DiskSpace(String),
}

pub fn verify_existing_sha256(path: &Path, expected: &str) -> Result<bool, InstallError> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex_digest(hasher) == expected)
}

pub fn has_disk_space(path: &Path, needed_bytes: u64) -> Result<bool, InstallError> {
    let available = fs4::available_space(path).map_err(|e| InstallError::DiskSpace(e.to_string()))?;
    Ok(available >= needed_bytes)
}

#[derive(Debug)]
pub struct DownloadSink {
    writer: BufWriter<File>,
    hasher: Sha256,
    partial_path: PathBuf,
}

impl DownloadSink {
    pub fn create(partial_path: PathBuf) -> Result<Self, InstallError> {
        let file = File::create(&partial_path)?;
        Ok(Self {
            writer: BufWriter::new(file),
            hasher: Sha256::new(),
            partial_path,
        })
    }

    pub fn resume(partial_path: PathBuf) -> Result<(Self, u64), InstallError> {
        let mut existing = File::open(&partial_path)?;
        let mut hasher = Sha256::new();
        let mut buf = [0u8; 64 * 1024];
        let mut offset: u64 = 0;
        loop {
            let n = existing.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            offset += n as u64;
        }
        drop(existing);

        let file = OpenOptions::new().append(true).open(&partial_path)?;
        Ok((
            Self {
                writer: BufWriter::new(file),
                hasher,
                partial_path,
            },
            offset,
        ))
    }

    pub fn write_chunk(&mut self, chunk: &[u8]) -> Result<(), InstallError> {
        self.writer.write_all(chunk)?;
        self.hasher.update(chunk);
        Ok(())
    }

    pub fn finalise_and_rename(
        mut self,
        final_path: &Path,
        expected_sha256: &str,
    ) -> Result<(), InstallError> {
        self.writer.flush()?;
        drop(self.writer);
        let actual = hex_digest(self.hasher);
        if actual != expected_sha256 {
            return Err(InstallError::ChecksumMismatch {
                expected: expected_sha256.to_string(),
                actual,
            });
        }
        std::fs::rename(&self.partial_path, final_path)?;
        Ok(())
    }
}

fn hex_digest(hasher: Sha256) -> String {
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ModelStatus {
    NotInstalled,
    Installed,
    ChecksumMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct InstallationState {
    pub items: Vec<(String, ModelStatus)>,
}

pub fn check_installation(registry: &Registry, models_dir: &Path) -> InstallationState {
    let items = registry
        .models
        .iter()
        .map(|m| {
            let path = models_dir.join(&m.filename);
            let status = if !path.exists() {
                ModelStatus::NotInstalled
            } else if verify_existing_sha256(&path, &m.sha256).unwrap_or(false) {
                ModelStatus::Installed
            } else {
                ModelStatus::ChecksumMismatch
            };
            (m.id.clone(), status)
        })
        .collect();
    InstallationState { items }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    fn known_sha256_of(bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(bytes);
        format!("{:x}", h.finalize())
    }

    #[test]
    fn verifies_existing_file_sha256() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("file.bin");
        let bytes = b"hello, world";
        fs::write(&path, bytes).unwrap();

        let expected = known_sha256_of(bytes);
        assert!(verify_existing_sha256(&path, &expected).unwrap());

        let wrong = "0".repeat(64);
        assert!(!verify_existing_sha256(&path, &wrong).unwrap());
    }

    #[test]
    fn create_write_finalise_atomically_renames_on_sha_match() {
        let dir = TempDir::new().unwrap();
        let partial = dir.path().join("model.bin.partial");
        let final_path = dir.path().join("model.bin");
        let bytes = b"some-payload-bytes-here";
        let expected = known_sha256_of(bytes);

        let mut sink = DownloadSink::create(partial.clone()).unwrap();
        sink.write_chunk(&bytes[..10]).unwrap();
        sink.write_chunk(&bytes[10..]).unwrap();
        sink.finalise_and_rename(&final_path, &expected).unwrap();

        assert!(!partial.exists(), "partial should be gone after rename");
        assert!(final_path.exists(), "final path should exist");
        assert_eq!(fs::read(&final_path).unwrap(), bytes);
    }

    #[test]
    fn finalise_with_sha_mismatch_leaves_partial_and_errors() {
        let dir = TempDir::new().unwrap();
        let partial = dir.path().join("model.bin.partial");
        let final_path = dir.path().join("model.bin");

        let mut sink = DownloadSink::create(partial.clone()).unwrap();
        sink.write_chunk(b"actual-bytes").unwrap();
        let wrong_sha = "0".repeat(64);

        let err = sink
            .finalise_and_rename(&final_path, &wrong_sha)
            .expect_err("must error on sha mismatch");

        assert!(matches!(err, InstallError::ChecksumMismatch { .. }));
        assert!(partial.exists(), "partial must survive sha mismatch for inspection");
        assert!(!final_path.exists(), "final path must not be created on mismatch");
    }

    #[test]
    fn resume_primes_hasher_with_existing_partial_bytes() {
        let dir = TempDir::new().unwrap();
        let partial = dir.path().join("model.bin.partial");
        let final_path = dir.path().join("model.bin");
        let prefix = b"first-half-of-the-file";
        let suffix = b"-second-half";
        let mut full = prefix.to_vec();
        full.extend_from_slice(suffix);
        let expected = known_sha256_of(&full);

        {
            let mut f = fs::File::create(&partial).unwrap();
            f.write_all(prefix).unwrap();
        }

        let (mut sink, resume_from) = DownloadSink::resume(partial.clone()).unwrap();
        assert_eq!(resume_from, prefix.len() as u64);

        sink.write_chunk(suffix).unwrap();
        sink.finalise_and_rename(&final_path, &expected).unwrap();

        assert_eq!(fs::read(&final_path).unwrap(), full);
    }

    #[test]
    fn has_disk_space_reports_true_when_plenty_free() {
        let dir = TempDir::new().unwrap();
        assert!(has_disk_space(dir.path(), 1).unwrap());
    }

    #[test]
    fn has_disk_space_reports_false_when_request_is_impossibly_large() {
        let dir = TempDir::new().unwrap();
        assert!(!has_disk_space(dir.path(), u64::MAX).unwrap());
    }

    fn registry_with_model(filename: &str, sha: &str) -> crate::models::Registry {
        use crate::models::{Model, Purpose, Registry};
        Registry {
            models: vec![Model {
                id: "test".into(),
                purpose: Purpose::Asr,
                url: "https://huggingface.co/x".into(),
                sha256: sha.into(),
                size_bytes: 1,
                filename: filename.into(),
            }],
        }
    }

    #[test]
    fn check_installation_reports_not_installed_when_file_absent() {
        let dir = TempDir::new().unwrap();
        let registry = registry_with_model("missing.bin", &"0".repeat(64));

        let state = check_installation(&registry, dir.path());

        assert_eq!(state.items.len(), 1);
        assert_eq!(state.items[0].0, "test");
        assert_eq!(state.items[0].1, ModelStatus::NotInstalled);
    }

    #[test]
    fn check_installation_reports_installed_when_sha_matches() {
        let dir = TempDir::new().unwrap();
        let bytes = b"installed bytes";
        let sha = known_sha256_of(bytes);
        fs::write(dir.path().join("model.bin"), bytes).unwrap();
        let registry = registry_with_model("model.bin", &sha);

        let state = check_installation(&registry, dir.path());

        assert_eq!(state.items[0].1, ModelStatus::Installed);
    }

    #[test]
    fn check_installation_reports_mismatch_when_sha_does_not_match() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("model.bin"), b"corrupted").unwrap();
        let registry = registry_with_model("model.bin", &"f".repeat(64));

        let state = check_installation(&registry, dir.path());

        assert_eq!(state.items[0].1, ModelStatus::ChecksumMismatch);
    }
}
