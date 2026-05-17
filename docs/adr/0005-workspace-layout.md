# Workspace layout: pnpm `modules/` and cargo `crates/` as sibling roots

The repository hosts two independent build systems — pnpm for the TypeScript frontend, cargo for the Rust shell and the eval suite. Each gets its own top-level glob: `modules/*` for pnpm packages, `crates/*` for cargo crates. They are siblings, not nested. This keeps each build system's discovery root unambiguous and matches the literal naming each ecosystem uses ("module" / "crate"), avoiding the heterogeneous-package-types confusion a unified `packages/` directory would create.

## The three Rust crates

The cargo workspace contains three crates, split by the dependency boundary that matters most: which code does the eval CLI need to compile?

- **`crates/pipeline/`** — a library crate. Contains Audio Capture (cpal), the ASR Engine (whisper-rs), and the Cleanup Engine (llama-cpp-rs). No Tauri dependency. Both the app and the eval CLI depend on this crate.
- **`crates/tauri-shell/`** — the binary crate that becomes the shipped `.app` executable. Holds `tauri.conf.json`, the IPC command surface, the Insertion module (clipboard paste with restore), and the macOS Fn Hook (IOHIDManager event tap). Depends on `pipeline`.
- **`crates/eval/`** — the binary crate behind `cargo run --bin eval`. Depends on `pipeline` only — never on `tauri-shell`. This is load-bearing: when iterating on the Cleanup system prompt, eval must rebuild in seconds, not the ~30s a clean Tauri build takes. Pulling Tauri into eval's dependency graph would kneecap the prompt-iteration loop that the eval suite exists to enable.

The Tauri shell lives at `crates/tauri-shell/` rather than the default `src-tauri/` at repository root. Tauri 2 supports this via the `tauri` CLI configuration in the root `package.json`; the cost is a one-time pointer, and the benefit is uniform "all Rust lives in `crates/`" naming.

## The TypeScript split rule

`modules/*` does **not** mirror brushfeed's pattern of one package per deployable. type-less has one deployable (the Tauri app). Instead, pnpm packages are reserved for **pure-logic modules** — code with no React hooks, no Tauri APIs, no DOM. Three packages qualify at v1:

- `@type-less/levelling` — Speechcraft curve math and state reducers
- `@type-less/activation` — gesture state machine over key events
- `@type-less/theme-loader` — Theme manifest parser; takes a `ThemeReader` dependency injected by the caller, keeping the package itself I/O-free

Everything else — Pill State Driver, Onboarding Orchestrator, Permission Manager, Model Download Manager, Session Orchestrator, Settings Store — is intrinsically coupled to React hooks or Tauri APIs. Those live as feature folders inside `modules/app/src/features/<name>/`, where the boundary is the feature folder rather than a package.

The rule, succinctly: **if it can be unit-tested as pure functions of its inputs, it earns a package. Otherwise it is a feature folder inside the app.** Pnpm's `exports` field becomes a contractual wall for the pure modules — the strongest "deep module with a simple interface" enforcement available short of npm-publishing.

## Multi-HTML-entry frontend

Each Tauri window (Settings, Pill, Overlay) has its own HTML entry file at the root of `modules/app/` (`settings.html`, `pill.html`, `overlay.html`) and its own `main.tsx` in `src/windows/<name>/`. Vite's `rollupOptions.input` declares all three as build entries. Each window loads only its own JS bundle; cross-window code is shared through normal imports and tree-shaken per entry.

The load-bearing reason is the Pill: it is a continuous-presence window with a real performance budget (a slit shape plus a live waveform animation). Forcing it to load Settings forms, the Onboarding state machine, and the Theme picker — code it will never execute — would meaningfully inflate the most-visible UI in the product. The Overlay benefits similarly: a smaller bundle means faster first paint when a Speechcraft Level-Up fires.

## Snapshot of the resulting tree (illustrative, not canonical)

```
/
├── pnpm-workspace.yaml          packages: ["modules/*"]
├── Cargo.toml                   workspace.members = ["crates/*"]
├── package.json                 root scripts; tauri CLI configured to point at crates/tauri-shell
├── tsconfig.base.json
├── biome.json
├── modules/
│   ├── levelling/
│   ├── activation/
│   ├── theme-loader/
│   └── app/
│       ├── settings.html
│       ├── pill.html
│       ├── overlay.html
│       └── src/
│           ├── windows/{settings,pill,overlay}/main.tsx
│           ├── features/{pill-driver,onboarding,permissions,session,settings-store,model-download}/
│           └── shared/{ipc,ui,design-tokens}/
├── crates/
│   ├── pipeline/                lib — audio + ASR + cleanup
│   ├── tauri-shell/             bin — Tauri app; holds tauri.conf.json
│   └── eval/                    bin — regression CLI
└── docs/
```

This tree is the shape at the time of writing. It will drift as features land. The body of this ADR is the durable contract; the tree is a visual anchor.

## Considered alternatives

- **Single `src-tauri` crate with eval as a second binary inside (`src-tauri/src/bin/eval.rs`)** — rejected: eval inherits Tauri's full dependency graph, so prompt-iteration rebuilds take ~30s instead of seconds. Eval-iteration speed is the single most-named justification for the workspace in the v1 PRD's eval suite section.
- **Two cargo crates — `tauri-shell` (with `pipeline` modules as `pub`-exported internals) and `eval` depending on `tauri-shell`** — rejected for the same reason: eval still pulls Tauri transitively.
- **Single pnpm package with feature folders for everything** — rejected: viable for size but gives no enforcement of the deep-module principle. Lint rules can suggest boundaries; `exports` walls them.
- **One pnpm package per PRD-listed module (~10 packages)** — rejected: mirrors brushfeed's surface but not its intent. brushfeed's packages are deployables sharing types; here, the coupled modules (Pill Driver, Session Orchestrator, etc.) gain no testability benefit from `exports` enforcement because their public surface is already a React hook or Zustand store — the boundary that matters is the hook signature, not the package wall.
- **Single SPA with query-string routing across the three windows** — rejected: the Pill window would ship the full app bundle (Settings forms, Onboarding, Theme picker) for code it never executes. Continuous-presence UI has a real bundle budget; this option spends it badly.
- **Unified `packages/` umbrella holding both pnpm packages and cargo crates** — rejected: the two build systems each discover their members by crawling the glob and looking for a file (`package.json` vs `Cargo.toml`). Mixing them in one folder forces every reader (and tool) to inspect each subfolder to learn what kind of thing it is. Literal naming (`modules/` and `crates/`) is a cheap legibility win.

## Naming detail: bundle identifier

The macOS bundle identifier is `io.github.canhassancode.type-less`. type-less is an MIT-licensed open-source project on a personal GitHub account, and the identifier should reflect where the project lives, not the developer account that happens to pay for code signing. The Apple Developer Team ID and the bundle identifier are independent; signing with the oneforge developer certificate later requires no change to the identifier. If type-less ever moves under the oneforge brand officially, the identifier renames then — the migration cost (orphaning `~/Library/Application Support/io.github.canhassancode.type-less/`) is justified at that point by the brand shift, not now.
