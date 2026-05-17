# Cleanup system prompt is decoded once at engine startup; per-dictation calls copy and reuse the prefix KV cache

The Cleanup engine decodes its system prompt — the ~500-token ChatML system message in `crates/pipeline/prompts/cleanup_v1.txt` — exactly once, into sequence 0 of the llama.cpp KV cache, when the engine's worker thread starts. Per-dictation cleanup calls copy seq 0 → seq 1 via `LlamaContext::copy_kv_cache_seq`, decode the transcript tokens into seq 1, sample output tokens, then clear seq 1 with `clear_kv_cache_seq` to leave seq 0 untouched for the next call.

This saves roughly 50-100ms per dictation on M-series hardware (the cost of re-decoding the ~500-token prompt), at the price of paying that cost once at startup and reserving a few KB of KV-cache memory per resident sequence. Across a typical session of dozens of dictations, the trade is overwhelmingly favourable.

The prefix-KV pattern is feasible specifically because `llama-cpp-2 0.1` wraps the underlying llama.cpp memory primitives (`llama_memory_seq_cp`, `llama_memory_seq_rm`, `llama_memory_seq_keep`) cleanly — see `LlamaContext::copy_kv_cache_seq` and `LlamaContext::clear_kv_cache_seq`. The engine's worker thread owns the `LlamaContext` exclusively (`LlamaContext` is `!Send + !Sync`); no synchronisation is needed for the seq-0/seq-1 dance because all access is single-threaded inside the worker.

## Considered alternatives

- **Re-decode the system prompt on every cleanup call** — rejected: 50-100ms penalty per dictation. The < 1.0s p95 latency budget could still absorb this (~440-775ms total vs. ~400-665ms with caching), but the headroom matters when later slices add work to the same window (Speechcraft levelling check, future telemetry-less analytics, longer cleanup prompts as #8 adds voice commands).
- **Multi-turn few-shot examples in the cached prefix** — rejected: stronger instruction adherence for Qwen 2.5, but the prefix becomes a multi-turn dialogue, complicating the prefix-KV setup (each example user/assistant turn lives at a known token position; the per-call decode must start at the correct offset). Prose-embedded examples in the single system message are weaker but trivial to cache and trivially extensible by #8.
- **Use llama.cpp's higher-level "session save/restore" file API** (`save_session_file` / `load_session_file`) — rejected: that API persists state across process restarts, which we don't need (we're in-process). The per-call sequence-copy approach is faster (memory-to-memory) and avoids disk I/O entirely, respecting ADR 0010's offline-first-and-no-disk-side-effects spirit.
- **Use a different llama wrapper** (`llama-cpp-rs`, `llm`, hand-rolled FFI) — rejected: `llama-cpp-2` is already a workspace dependency, actively maintained, wraps the exact primitives we need, and the rest of the codebase is converging on it.

## Accepted cost

App startup blocks for ~30-80ms while the Cleanup engine decodes the system prompt (after the larger ~1-3s model load). This is invisible in practice (the hotkey isn't usable until engines are ready per ADR 0007/Q6, and the user is typically not pressing it within the first few seconds of launch). Memory footprint: a handful of KB per resident sequence in the KV cache — negligible against the ~1 GB model itself.
