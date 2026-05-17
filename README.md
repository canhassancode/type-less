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
| `cargo run --bin eval` | Pipeline regression suite (placeholder) |

## Project layout

- `modules/app/` — Tauri frontend (Vite + React 18 + Tailwind 4 + Zustand)
- `crates/pipeline/` — audio capture, ASR, cleanup, resampling, model registry
- `crates/tauri-shell/` — the desktop app binary, IPC commands
- `crates/eval/` — dev binaries (`eval`, `bootstrap-models`)
- `docs/adr/` — architecture decision records
- `CONTEXT.md` — domain glossary
- `CLAUDE.md` — repo conventions for AI agents
