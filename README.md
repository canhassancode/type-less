# type-less

Local-first AI dictation. Hold a hotkey, speak, get cleaned text pasted at the cursor. Everything runs on-device — no cloud, no telemetry.

## Prerequisites

- Node 20+ and pnpm 10+
- Rust toolchain (rustup, stable)
- macOS (Linux/Windows not yet supported)
- ~2 GB free disk for models

## Getting started

```bash
pnpm install
pnpm bootstrap:models    # one-shot: ~1.6 GB download (whisper-small.en + qwen2.5-1.5b)
pnpm tauri dev
```

`bootstrap:models` downloads the ASR and Cleanup models from the URLs in `models.json` into the app data dir (`~/Library/Application Support/io.github.canhassancode.type-less/models/` on macOS), verifies their SHAs, and writes any new SHAs back to `models.json`. Re-run it any time the registry changes.

## Commands

| Command | What it does |
| --- | --- |
| `pnpm tauri dev` | Run the app locally |
| `pnpm bootstrap:models` | Download / verify models listed in `models.json` |
| `pnpm test` | TypeScript unit tests (Vitest) |
| `pnpm lint` | Biome lint + format check |
| `pnpm typecheck` | TypeScript type-check |
| `cargo test --workspace` | Rust unit tests |
| `cargo clippy --workspace --all-targets -- -D warnings` | Rust lint |
| `pnpm timing` | Pipeline perf gate — asserts ASR + Cleanup p95 < 1s. See [Perf regression gate](#perf-regression-gate). |

## Perf regression gate

`pnpm timing` runs `crates/eval/src/bin/timing.rs`: loads the ASR + Cleanup engines from the app data dir, runs 10 synthetic 5s F32 PCM fixtures (mix of silence and seeded low-amplitude noise), and asserts p95 < 1000ms.

**This is an on-demand gate, not a routine command.** Run it when:

- Touching anything in `crates/pipeline/` (asr, cleanup, resample)
- Bumping `whisper-rs` or `llama-cpp-2` versions
- Swapping the model files referenced in `models.json`
- Before tagging a release
- Investigating a suspected perf regression

It needs the real models on disk (`pnpm bootstrap:models` first) and takes ~30-60s. The `pnpm` script pins `--release` because debug builds are 10-50× slower for whisper / llama work and would fail the budget on healthy code. Quality scoring (semantic similarity, F1, recorded fixtures) is out of scope here — it lives in a future eval slice.

## Project layout

- `modules/app/` — Tauri frontend (Vite + React 18 + Tailwind 4 + Zustand)
- `crates/pipeline/` — audio capture, ASR, cleanup, resampling, model registry
- `crates/tauri-shell/` — the desktop app binary, IPC commands
- `crates/eval/` — dev binaries (`eval`, `bootstrap-models`, `timing`)
- `docs/adr/` — architecture decision records
- `CONTEXT.md` — domain glossary
- `CLAUDE.md` — repo conventions for AI agents
