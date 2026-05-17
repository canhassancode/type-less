use std::num::NonZeroU32;
use std::path::Path;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::mpsc::{self, SyncSender};
use std::thread::JoinHandle;

use llama_cpp_2::context::LlamaContext;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;

static BACKEND: OnceLock<LlamaBackend> = OnceLock::new();

fn backend() -> &'static LlamaBackend {
    BACKEND.get_or_init(|| LlamaBackend::init().expect("llama backend init"))
}

#[derive(Debug, thiserror::Error)]
pub enum CleanupError {
    #[error("failed to load cleanup model: {0}")]
    Load(String),
    #[error("cleanup failed: {0}")]
    Cleanup(String),
    #[error("worker thread unavailable")]
    WorkerUnavailable,
}

const N_CTX: u32 = 2048;
const MAX_NEW_TOKENS: usize = 256;
const PIECE_BUFFER_BYTES: usize = 256;

enum CleanupJob {
    Cleanup {
        transcript: String,
        reply: SyncSender<Result<String, CleanupError>>,
    },
    Shutdown,
}

#[derive(Debug)]
pub struct CleanupEngine {
    sender: SyncSender<CleanupJob>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl CleanupEngine {
    pub fn load(model_path: &Path, prompt: &str) -> Result<Self, CleanupError> {
        if !model_path.exists() {
            return Err(CleanupError::Load(format!(
                "model file not found at {}",
                model_path.display()
            )));
        }
        let path_owned = model_path.to_path_buf();
        let prompt_owned = prompt.to_string();
        let (load_tx, load_rx) = mpsc::sync_channel::<Result<(), CleanupError>>(1);
        let (job_tx, job_rx) = mpsc::sync_channel::<CleanupJob>(0);

        let worker = std::thread::spawn(move || {
            run_worker(&path_owned, &prompt_owned, load_tx, job_rx);
        });

        match load_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                sender: job_tx,
                worker: Mutex::new(Some(worker)),
            }),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(CleanupError::Load("worker died during load".into())),
        }
    }

    pub fn cleanup(&self, transcript: &str) -> Result<String, CleanupError> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        self.sender
            .send(CleanupJob::Cleanup {
                transcript: transcript.to_string(),
                reply: reply_tx,
            })
            .map_err(|_| CleanupError::WorkerUnavailable)?;
        reply_rx.recv().map_err(|_| CleanupError::WorkerUnavailable)?
    }
}

impl Drop for CleanupEngine {
    fn drop(&mut self) {
        let _ = self.sender.send(CleanupJob::Shutdown);
        if let Ok(mut guard) = self.worker.lock() {
            if let Some(handle) = guard.take() {
                let _ = handle.join();
            }
        }
    }
}

fn run_worker(
    model_path: &Path,
    prompt: &str,
    load_tx: SyncSender<Result<(), CleanupError>>,
    job_rx: mpsc::Receiver<CleanupJob>,
) {
    let backend = backend();
    let model = match LlamaModel::load_from_file(backend, model_path, &LlamaModelParams::default())
    {
        Ok(m) => m,
        Err(e) => {
            let _ = load_tx.send(Err(CleanupError::Load(format!("load model: {e}"))));
            return;
        }
    };
    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(NonZeroU32::new(N_CTX))
        .with_n_seq_max(2);
    let mut ctx = match model.new_context(backend, ctx_params) {
        Ok(c) => c,
        Err(e) => {
            let _ = load_tx.send(Err(CleanupError::Load(format!("create context: {e}"))));
            return;
        }
    };

    let wrapped = format!("<|im_start|>system\n{prompt}<|im_end|>\n");
    let prompt_tokens = match model.str_to_token(&wrapped, AddBos::Never) {
        Ok(t) => t,
        Err(e) => {
            let _ = load_tx.send(Err(CleanupError::Load(format!("tokenise prompt: {e}"))));
            return;
        }
    };
    let prompt_len = prompt_tokens.len();

    let mut batch = LlamaBatch::new(prompt_len.max(64), 2);
    if let Err(e) = batch.add_sequence(&prompt_tokens, 0, false) {
        let _ = load_tx.send(Err(CleanupError::Load(format!("batch prompt: {e}"))));
        return;
    }
    if let Err(e) = ctx.decode(&mut batch) {
        let _ = load_tx.send(Err(CleanupError::Load(format!("decode prompt: {e}"))));
        return;
    }

    let _ = load_tx.send(Ok(()));
    drop(load_tx);

    while let Ok(job) = job_rx.recv() {
        match job {
            CleanupJob::Cleanup { transcript, reply } => {
                let result = run_cleanup(&model, &mut ctx, &transcript, prompt_len);
                let _ = reply.send(result);
            }
            CleanupJob::Shutdown => break,
        }
    }
}

fn run_cleanup(
    model: &LlamaModel,
    ctx: &mut LlamaContext,
    transcript: &str,
    prompt_len: usize,
) -> Result<String, CleanupError> {
    let user_prefix =
        format!("<|im_start|>user\n{transcript}<|im_end|>\n<|im_start|>assistant\n");
    let user_tokens = model
        .str_to_token(&user_prefix, AddBos::Never)
        .map_err(|e| CleanupError::Cleanup(format!("tokenise user: {e}")))?;
    if user_tokens.is_empty() {
        return Ok(String::new());
    }

    ctx.copy_kv_cache_seq(0, 1, None, None)
        .map_err(|e| CleanupError::Cleanup(format!("copy_kv_cache_seq: {e}")))?;

    let mut batch = LlamaBatch::new(user_tokens.len().max(8), 2);
    let last_idx = user_tokens.len() - 1;
    for (i, &tok) in user_tokens.iter().enumerate() {
        let pos = (prompt_len + i) as i32;
        let logits = i == last_idx;
        batch
            .add(tok, pos, &[1], logits)
            .map_err(|e| CleanupError::Cleanup(format!("batch user: {e}")))?;
    }
    ctx.decode(&mut batch)
        .map_err(|e| CleanupError::Cleanup(format!("decode user: {e}")))?;

    let mut sampler = LlamaSampler::chain_simple([LlamaSampler::greedy()]);
    let mut output_bytes: Vec<u8> = Vec::new();
    let start_pos = (prompt_len + user_tokens.len()) as i32;

    for offset in 0..MAX_NEW_TOKENS as i32 {
        let token = sampler.sample(ctx, batch.n_tokens() - 1);
        if model.is_eog_token(token) {
            break;
        }
        sampler.accept(token);

        let piece = model
            .token_to_piece_bytes(token, PIECE_BUFFER_BYTES, false, None)
            .map_err(|e| CleanupError::Cleanup(format!("token_to_piece_bytes: {e}")))?;
        output_bytes.extend_from_slice(&piece);

        batch.clear();
        batch
            .add(token, start_pos + offset, &[1], true)
            .map_err(|e| CleanupError::Cleanup(format!("batch next: {e}")))?;
        ctx.decode(&mut batch)
            .map_err(|e| CleanupError::Cleanup(format!("decode next: {e}")))?;
    }

    ctx.clear_kv_cache_seq(Some(1), None, None)
        .map_err(|e| CleanupError::Cleanup(format!("clear_kv_cache_seq: {e}")))?;

    let raw = String::from_utf8_lossy(&output_bytes).trim().to_string();
    let unfenced = strip_code_fence(&raw);
    let unwrapped = strip_outer_quotes(unfenced);
    if !unwrapped.chars().any(|c| c.is_alphanumeric()) {
        return Ok(String::new());
    }
    Ok(unwrapped.to_string())
}

fn strip_outer_quotes(s: &str) -> &str {
    let stripped = s
        .strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'));
    match stripped {
        Some(inner) if !inner.contains('"') => inner,
        _ => s,
    }
}

fn strip_code_fence(s: &str) -> &str {
    let trimmed = s.trim();
    let Some(after_open) = trimmed.strip_prefix("```") else {
        return trimmed;
    };
    let Some(closing) = trimmed.strip_suffix("```") else {
        return trimmed;
    };
    let _ = closing;
    let body = match after_open.split_once('\n') {
        Some((_lang, rest)) => rest,
        None => after_open,
    };
    body.trim_end_matches('`').trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUNDLE_ID: &str = "io.github.canhassancode.type-less";
    const QWEN_FILENAME: &str = "qwen2.5-1.5b-instruct-q4_k_m.gguf";
    const CLEANUP_PROMPT: &str = include_str!("../prompts/cleanup_v1.txt");

    fn assert_send_sync<T: Send + Sync>() {}

    fn qwen_model_path() -> Option<std::path::PathBuf> {
        let path = dirs::data_dir()?
            .join(BUNDLE_ID)
            .join("models")
            .join(QWEN_FILENAME);
        if path.exists() { Some(path) } else { None }
    }

    fn load_engine() -> Option<CleanupEngine> {
        let path = qwen_model_path()?;
        Some(CleanupEngine::load(&path, CLEANUP_PROMPT).expect("load qwen cleanup engine"))
    }

    #[test]
    fn lock_engine_handle_is_send_sync() {
        assert_send_sync::<CleanupEngine>();
    }

    #[test]
    fn lock_load_returns_err_for_missing_path() {
        let err = CleanupEngine::load(Path::new("/nonexistent/path/to/qwen.gguf"), "prompt")
            .expect_err("loading from missing path must error");

        assert!(
            matches!(err, CleanupError::Load(_)),
            "expected CleanupError::Load, got {err:?}",
        );
    }

    #[test]
    #[ignore = "requires real qwen model on disk; run with --ignored"]
    fn lock_e1_hello_world() {
        let Some(engine) = load_engine() else {
            eprintln!("skipping: qwen model not on disk; run `pnpm bootstrap:models`");
            return;
        };

        let out = engine.cleanup("hello world").expect("cleanup hello world");

        eprintln!("E1#1 hello world → {out:?}");
        let lower = out.to_lowercase();
        assert!(
            lower.contains("hello") && lower.contains("world"),
            "expected 'Hello, world.' shape, got: {out:?}",
        );
        assert!(
            out.starts_with('H'),
            "must capitalise sentence start, got: {out:?}",
        );
    }

    #[test]
    #[ignore = "requires real qwen model on disk; run with --ignored"]
    fn exploration_e3_long_form() {
        let Some(engine) = load_engine() else {
            eprintln!("skipping: qwen model not on disk; run `pnpm bootstrap:models`");
            return;
        };

        let input = "so I was thinking about the new dictation pipeline and how we should approach the cleanup stage \
                     the idea is that we run whisper for transcription and then pipe the raw transcript through \
                     a smaller language model that strips fillers and fixes basic punctuation things like commas \
                     periods and capitalisation we want the whole thing to feel instant so the latency budget \
                     is under one second from key release to paste landing anyway on a separate note I also want \
                     to make sure that we get the speechcraft levelling system right because that is the signature \
                     flourish of the product without it we are just another dictation tool with the levelling \
                     and the overlay we have something that feels like a game and that emotional reward is what \
                     keeps users coming back even when the model itself is not perfect";
        let input_words = input.split_whitespace().count();

        let out = engine.cleanup(input).expect("cleanup long-form input");
        let out_words = out.split_whitespace().count();

        eprintln!("E3 long-form ({input_words} words in / {out_words} words out) → {out:?}");
        assert!(
            out_words >= (input_words * 7 / 10) && out_words <= (input_words * 13 / 10),
            "long-form output word count {out_words} drifted outside 70-130% of input {input_words}",
        );
    }

    #[test]
    #[ignore = "requires real qwen model on disk; run with --ignored"]
    fn exploration_e4_restart_preserved() {
        let Some(engine) = load_engine() else {
            eprintln!("skipping: qwen model not on disk; run `pnpm bootstrap:models`");
            return;
        };

        let out = engine
            .cleanup("the meeting is at three actually four")
            .expect("cleanup restart input");

        eprintln!("E4#13 restart → {out:?}");
        let lower = out.to_lowercase();
        assert!(
            lower.contains("three") && lower.contains("four"),
            "restart should keep both 'three' and 'four', got: {out:?}",
        );
    }

    #[test]
    #[ignore = "requires real qwen model on disk; run with --ignored"]
    fn exploration_e5_numbers() {
        let Some(engine) = load_engine() else {
            eprintln!("skipping: qwen model not on disk; run `pnpm bootstrap:models`");
            return;
        };

        let inputs = [
            "the timeout is three hundred and forty two milliseconds",
            "lets meet on march fifteenth at two pm",
            "the file is about five hundred megabytes",
        ];
        for input in inputs {
            let out = engine.cleanup(input).expect("cleanup number input");
            eprintln!("E5 numbers input={input:?} → {out:?}");
            assert!(!out.is_empty(), "number-bearing input must produce output");
        }
    }

    #[test]
    #[ignore = "requires real qwen model on disk; run with --ignored"]
    fn exploration_e6_technical_vocab() {
        let Some(engine) = load_engine() else {
            eprintln!("skipping: qwen model not on disk; run `pnpm bootstrap:models`");
            return;
        };

        let inputs = [
            "the function check user auth returns a boolean",
            "the api endpoint is slash v one slash users",
            "import react from react",
        ];
        for input in inputs {
            let out = engine.cleanup(input).expect("cleanup technical input");
            eprintln!("E6 tech input={input:?} → {out:?}");
            assert!(!out.is_empty(), "technical input must produce output");
        }
    }

    #[test]
    #[ignore = "requires real qwen model on disk; run with --ignored"]
    fn lock_e9_no_failure_surfaces() {
        let Some(engine) = load_engine() else {
            eprintln!("skipping: qwen model not on disk; run `pnpm bootstrap:models`");
            return;
        };

        let inputs = [
            "hello world",
            "yeah you know its really good",
            "the meeting is at three actually four",
            "we use postgres on aws",
            "import react from react",
            "she said the file is read only",
        ];

        let banned_preambles = [
            "Here is",
            "Here's",
            "Sure,",
            "Output:",
            "Cleaned:",
            "I have",
            "I would",
            "The cleaned",
        ];

        for input in inputs {
            let out = engine.cleanup(input).expect("cleanup");
            eprintln!("E9 input={input:?} → {out:?}");
            for banned in banned_preambles {
                assert!(
                    !out.starts_with(banned),
                    "E9 preamble '{banned}' leaked for input {input:?}, got: {out:?}",
                );
            }
            assert!(
                !(out.starts_with('"') && out.ends_with('"')),
                "E9 quote-wrap leaked for input {input:?}, got: {out:?}",
            );
            assert!(
                !out.contains("```"),
                "E9 code fence leaked for input {input:?}, got: {out:?}",
            );
        }
    }

    #[test]
    #[ignore = "requires real qwen model on disk; run with --ignored"]
    fn lock_prefix_kv_idempotent_across_calls() {
        let Some(engine) = load_engine() else {
            eprintln!("skipping: qwen model not on disk; run `pnpm bootstrap:models`");
            return;
        };

        let input = "their going to the store there are three of them";
        let first = engine.cleanup(input).expect("first cleanup");
        let second = engine.cleanup(input).expect("second cleanup");
        let third = engine.cleanup(input).expect("third cleanup");

        eprintln!("prefix-kv calls: {first:?}, {second:?}, {third:?}");
        assert_eq!(
            first, second,
            "second call must reproduce first (seq 1 cleared, seq 0 intact)",
        );
        assert_eq!(
            second, third,
            "third call must reproduce second (cache hygiene durable)",
        );
    }

    #[test]
    #[ignore = "requires real qwen model on disk; run with --ignored"]
    fn lock_e7_i_think_preserved_as_hedging() {
        let Some(engine) = load_engine() else {
            eprintln!("skipping: qwen model not on disk; run `pnpm bootstrap:models`");
            return;
        };

        let out = engine
            .cleanup("I think we should ship it tomorrow")
            .expect("cleanup hedged input");

        eprintln!("E7#21 I think (hedging) → {out:?}");
        assert!(
            out.contains("I think"),
            "'I think' hedging must be preserved, got: {out:?}",
        );
    }

    #[test]
    #[ignore = "requires real qwen model on disk; run with --ignored"]
    fn lock_e8_proper_noun_capitalised() {
        let Some(engine) = load_engine() else {
            eprintln!("skipping: qwen model not on disk; run `pnpm bootstrap:models`");
            return;
        };

        let out = engine
            .cleanup("I met sarah from product yesterday")
            .expect("cleanup proper-noun input");

        eprintln!("E8#23 proper noun → {out:?}");
        assert!(
            out.contains("Sarah"),
            "'sarah' must be capitalised to 'Sarah', got: {out:?}",
        );
    }

    #[test]
    #[ignore = "requires real qwen model on disk; run with --ignored"]
    fn lock_e2_you_know_kept_as_literal() {
        let Some(engine) = load_engine() else {
            eprintln!("skipping: qwen model not on disk; run `pnpm bootstrap:models`");
            return;
        };

        let out = engine
            .cleanup("you know what I mean")
            .expect("cleanup literal you-know input");

        eprintln!("E2#9 you know (literal) → {out:?}");
        assert!(
            out.to_lowercase().contains("you know"),
            "'you know' as literal must be kept, got: {out:?}",
        );
    }

    #[test]
    #[ignore = "requires real qwen model on disk; run with --ignored"]
    fn lock_e2_you_know_removed_as_filler() {
        let Some(engine) = load_engine() else {
            eprintln!("skipping: qwen model not on disk; run `pnpm bootstrap:models`");
            return;
        };

        let out = engine
            .cleanup("this is you know really good")
            .expect("cleanup filler you-know input");

        eprintln!("E2#10 you know (filler) → {out:?}");
        let lower = out.to_lowercase();
        assert!(
            !lower.contains("you know"),
            "'you know' as filler must be removed, got: {out:?}",
        );
        assert!(
            lower.contains("really good"),
            "speaker's content must survive cleanup, got: {out:?}",
        );
    }

    #[test]
    #[ignore = "requires real qwen model on disk; run with --ignored"]
    fn lock_e2_like_removed_as_filler() {
        let Some(engine) = load_engine() else {
            eprintln!("skipping: qwen model not on disk; run `pnpm bootstrap:models`");
            return;
        };

        let out = engine
            .cleanup("it was like really cold")
            .expect("cleanup filler-like input");

        eprintln!("E2#8 like (filler) → {out:?}");
        let lower = out.to_lowercase();
        assert!(
            !lower.contains(" like "),
            "'like' as filler must be removed, got: {out:?}",
        );
        assert!(
            lower.contains("really cold"),
            "speaker's content must survive cleanup, got: {out:?}",
        );
    }

    #[test]
    #[ignore = "requires real qwen model on disk; run with --ignored"]
    fn lock_e2_like_kept_as_verb() {
        let Some(engine) = load_engine() else {
            eprintln!("skipping: qwen model not on disk; run `pnpm bootstrap:models`");
            return;
        };

        let out = engine
            .cleanup("I felt like running today")
            .expect("cleanup verb-like input");

        eprintln!("E2#7 like (verb) → {out:?}");
        assert!(
            out.to_lowercase().contains("like"),
            "'like' as verb must be kept, got: {out:?}",
        );
    }

    #[test]
    #[ignore = "requires real qwen model on disk; run with --ignored"]
    fn lock_e1_empty_input_returns_empty() {
        let Some(engine) = load_engine() else {
            eprintln!("skipping: qwen model not on disk; run `pnpm bootstrap:models`");
            return;
        };

        let out = engine
            .cleanup("um uh")
            .expect("cleanup filler-only input");

        eprintln!("E1#6 filler-only → {out:?}");
        assert!(
            out.trim().is_empty(),
            "filler-only input must yield empty output, got: {out:?}",
        );
    }

    #[test]
    #[ignore = "requires real qwen model on disk; run with --ignored"]
    fn lock_e1_homophones_their_there() {
        let Some(engine) = load_engine() else {
            eprintln!("skipping: qwen model not on disk; run `pnpm bootstrap:models`");
            return;
        };

        let out = engine
            .cleanup("their going to the store there are three of them")
            .expect("cleanup homophone input");

        eprintln!("E1#3 homophones → {out:?}");
        assert!(
            out.contains("They're"),
            "'their' → 'They're' (homophone fix), got: {out:?}",
        );
        assert!(
            out.contains("There"),
            "'there' kept as 'There', got: {out:?}",
        );
    }

    #[test]
    #[ignore = "requires real qwen model on disk; run with --ignored"]
    fn lock_e1_filler_um_removed() {
        let Some(engine) = load_engine() else {
            eprintln!("skipping: qwen model not on disk; run `pnpm bootstrap:models`");
            return;
        };

        let out = engine
            .cleanup("um so I was thinking we should ship the feature tomorrow")
            .expect("cleanup filler-prefixed input");

        eprintln!("E1#2 um filler → {out:?}");
        let lower = out.to_lowercase();
        assert!(
            !lower.starts_with("um") && !lower.contains(" um "),
            "'um' filler must be removed, got: {out:?}",
        );
        assert!(
            lower.contains("ship the feature"),
            "speaker's content must survive cleanup, got: {out:?}",
        );
    }
}
