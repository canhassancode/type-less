use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    Asr,
    Cleanup,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Model {
    pub id: String,
    pub purpose: Purpose,
    pub url: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub filename: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Registry {
    pub models: Vec<Model>,
}

#[derive(Debug, thiserror::Error)]
pub enum ModelsError {
    #[error("invalid models.json: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("duplicate model id: {0}")]
    DuplicateId(String),
}

impl Registry {
    pub fn from_json(json: &str) -> Result<Self, ModelsError> {
        let registry: Self = serde_json::from_str(json)?;
        registry.ensure_unique_ids()?;
        Ok(registry)
    }

    pub fn find_by_id(&self, id: &str) -> Option<&Model> {
        self.models.iter().find(|m| m.id == id)
    }

    fn ensure_unique_ids(&self) -> Result<(), ModelsError> {
        let mut seen = std::collections::HashSet::new();
        for model in &self.models {
            if !seen.insert(model.id.as_str()) {
                return Err(ModelsError::DuplicateId(model.id.clone()));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_registry_json() -> &'static str {
        r#"{
            "models": [
                {
                    "id": "whisper-small-en",
                    "purpose": "asr",
                    "url": "https://huggingface.co/example/whisper-small.en.bin",
                    "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
                    "size_bytes": 488000000,
                    "filename": "ggml-small.en.bin"
                },
                {
                    "id": "qwen2.5-1.5b-instruct-q4-k-m",
                    "purpose": "cleanup",
                    "url": "https://huggingface.co/example/qwen.gguf",
                    "sha256": "1111111111111111111111111111111111111111111111111111111111111111",
                    "size_bytes": 1100000000,
                    "filename": "qwen2.5-1.5b-instruct-q4_k_m.gguf"
                }
            ]
        }"#
    }

    #[test]
    fn parses_valid_registry_with_two_models() {
        let registry = Registry::from_json(sample_registry_json()).expect("valid JSON parses");

        assert_eq!(registry.models.len(), 2);
        assert_eq!(registry.models[0].id, "whisper-small-en");
        assert_eq!(registry.models[0].purpose, Purpose::Asr);
        assert_eq!(registry.models[0].size_bytes, 488_000_000);
        assert_eq!(registry.models[1].purpose, Purpose::Cleanup);
        assert_eq!(registry.models[1].filename, "qwen2.5-1.5b-instruct-q4_k_m.gguf");
    }

    #[test]
    fn rejects_malformed_json() {
        let err = Registry::from_json("not even close to json").expect_err("must error");

        assert!(matches!(err, ModelsError::Parse(_)));
    }

    #[test]
    fn rejects_duplicate_ids() {
        let json = r#"{
            "models": [
                {
                    "id": "dup",
                    "purpose": "asr",
                    "url": "https://huggingface.co/x",
                    "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
                    "size_bytes": 1,
                    "filename": "a.bin"
                },
                {
                    "id": "dup",
                    "purpose": "cleanup",
                    "url": "https://huggingface.co/y",
                    "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
                    "size_bytes": 1,
                    "filename": "b.bin"
                }
            ]
        }"#;

        let err = Registry::from_json(json).expect_err("duplicate ids must error");

        assert!(matches!(err, ModelsError::DuplicateId(ref id) if id == "dup"));
    }

    #[test]
    fn find_by_id_returns_the_model() {
        let registry = Registry::from_json(sample_registry_json()).unwrap();

        let found = registry.find_by_id("whisper-small-en").expect("present");
        assert_eq!(found.purpose, Purpose::Asr);

        assert!(registry.find_by_id("not-there").is_none());
    }
}
