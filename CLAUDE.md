This is a AI dictation tool similar to Wispr Flow, but local-first, local model, local everything. Completely open-source.
Check `CONTEXT.md` for terminology questions.
Check `docs/adr` for a list of ADR decisions made.

# Practice

- This is a `typescript` and `rust` project. The default of the project owner is `typescript`, however, `rust` is required to build this with performance in mind.
- NO `any` types.
- I combine the concept of John Ousterhout `deep modules`, meaning a simple interface for usability. With Robert C. Martins `clean code`, suggesting minimal functions, easy to read and no code comments.
- TDD first, using the `/tdd` skill on ALL implementation where possible.

# Stack

- App shell: Tauri 2.x — frontend in `modules/app/` (Vite + React 18 + Tailwind 4 + Zustand), Rust in `crates/`
- Rust crates: `pipeline` (Audio Capture, ASR, Cleanup), `tauri-shell` (the app binary), `eval` (regression-bar CLI)
- ASR via `whisper-rs`; Cleanup via `llama-cpp-rs`; audio capture via `cpal`; clipboard via `arboard`
- Rust→TS type generation via `specta`
- Lint/format via Biome; pnpm workspaces; cargo workspace

# Commands

- `pnpm tauri dev` — run the app locally
- `pnpm test` — Vitest (TypeScript)
- `pnpm lint` — Biome
- `cargo test` — Rust unit tests
- `cargo clippy` — Rust lint
- `cargo run --bin eval` — pipeline regression suite

# Others

- Issue tracker is on Github issues for this repo.
