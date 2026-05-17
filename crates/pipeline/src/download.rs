use std::io::Read;

use crate::install::{DownloadSink, InstallError};
use crate::models::Registry;

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("url not in registry: {0}")]
    UrlNotInRegistry(String),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("unexpected http status: {0}")]
    UnexpectedStatus(u16),
    #[error("install error: {0}")]
    Install(#[from] InstallError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub fn validate_url_against_registry(url: &str, registry: &Registry) -> Result<(), DownloadError> {
    if registry.models.iter().any(|m| m.url == url) {
        Ok(())
    } else {
        Err(DownloadError::UrlNotInRegistry(url.to_string()))
    }
}

pub fn stream_to_sink<F>(
    url: &str,
    mut sink: DownloadSink,
    resume_from: u64,
    registry: &Registry,
    mut on_progress: F,
) -> Result<DownloadSink, DownloadError>
where
    F: FnMut(u64),
{
    validate_url_against_registry(url, registry)?;

    let client = reqwest::blocking::Client::new();
    let mut request = client.get(url);
    if resume_from > 0 {
        request = request.header("Range", format!("bytes={resume_from}-"));
    }

    let mut response = request.send()?;
    if !response.status().is_success() {
        return Err(DownloadError::UnexpectedStatus(response.status().as_u16()));
    }

    let mut downloaded = resume_from;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = response.read(&mut buf)?;
        if n == 0 {
            break;
        }
        sink.write_chunk(&buf[..n])?;
        downloaded += n as u64;
        on_progress(downloaded);
    }

    Ok(sink)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::DownloadSink;
    use crate::models::{Model, Purpose, Registry};
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    fn known_sha256(bytes: &[u8]) -> String {
        let mut h = Sha256::new();
        h.update(bytes);
        format!("{:x}", h.finalize())
    }

    fn registry_with(url: &str) -> Registry {
        Registry {
            models: vec![Model {
                id: "test".into(),
                purpose: Purpose::Asr,
                url: url.into(),
                sha256: "deadbeef".into(),
                size_bytes: 0,
                filename: "t.bin".into(),
            }],
        }
    }

    #[test]
    fn rejects_url_not_in_registry() {
        let registry = registry_with("https://huggingface.co/legit");

        let err = validate_url_against_registry("https://evil.example.com/x", &registry)
            .expect_err("must reject");

        assert!(matches!(err, DownloadError::UrlNotInRegistry(_)));
    }

    #[test]
    fn accepts_url_in_registry() {
        let registry = registry_with("https://huggingface.co/legit");

        validate_url_against_registry("https://huggingface.co/legit", &registry)
            .expect("must accept");
    }

    #[test]
    fn streams_full_body_to_partial_then_renames_on_sha_match() {
        let mut server = mockito::Server::new();
        let body = b"the full body bytes of the model file";
        let sha = known_sha256(body);

        let mock = server
            .mock("GET", "/model.bin")
            .with_status(200)
            .with_body(body)
            .create();

        let url = format!("{}/model.bin", server.url());
        let registry = registry_with(&url);

        let dir = TempDir::new().unwrap();
        let partial = dir.path().join("model.bin.partial");
        let final_path = dir.path().join("model.bin");

        let sink = DownloadSink::create(partial.clone()).unwrap();
        let progress = Arc::new(Mutex::new(Vec::<u64>::new()));
        let progress_clone = Arc::clone(&progress);

        stream_to_sink(&url, sink, 0, &registry, |downloaded| {
            progress_clone.lock().unwrap().push(downloaded);
        })
        .expect("download should succeed")
        .finalise_and_rename(&final_path, &sha)
        .expect("rename should succeed");

        mock.assert();
        assert_eq!(fs::read(&final_path).unwrap(), body);
        let history = progress.lock().unwrap();
        assert!(!history.is_empty(), "progress callback should fire at least once");
        assert_eq!(*history.last().unwrap(), body.len() as u64);
    }

    #[test]
    fn resume_sends_range_header_with_existing_offset() {
        let mut server = mockito::Server::new();
        let suffix = b"-resumed-tail-bytes";

        let mock = server
            .mock("GET", "/model.bin")
            .match_header("range", "bytes=10-")
            .with_status(206)
            .with_body(suffix)
            .create();

        let url = format!("{}/model.bin", server.url());
        let registry = registry_with(&url);

        let dir = TempDir::new().unwrap();
        let partial = dir.path().join("model.bin.partial");
        let prefix = b"firsttenbb";
        fs::write(&partial, prefix).unwrap();

        let (sink, offset) = DownloadSink::resume(partial.clone()).unwrap();
        assert_eq!(offset, 10);

        stream_to_sink(&url, sink, offset, &registry, |_| {})
            .expect("download should succeed");

        mock.assert();
        let mut expected = prefix.to_vec();
        expected.extend_from_slice(suffix);
        assert_eq!(fs::read(&partial).unwrap(), expected);
    }

    #[test]
    fn rejects_streaming_when_url_not_in_registry() {
        let dir = TempDir::new().unwrap();
        let partial = dir.path().join("p");
        let sink = DownloadSink::create(partial).unwrap();
        let registry = registry_with("https://huggingface.co/legit");

        let err = stream_to_sink(
            "https://evil.example.com/x",
            sink,
            0,
            &registry,
            |_| {},
        )
        .expect_err("must reject");

        assert!(matches!(err, DownloadError::UrlNotInRegistry(_)));
    }
}
