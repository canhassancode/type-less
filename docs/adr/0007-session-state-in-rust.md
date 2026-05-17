# Dictation Session state lives in Rust; TypeScript subscribes via typed events

The state machine for a Dictation Session — the `Idle → Recording → Loading → Idle` transitions — lives in a single `Session` struct in `crates/tauri-shell/src/session.rs`. TypeScript subscribes to typed `dictation:state-changed` events emitted by Rust and renders accordingly. TypeScript does not own a parallel copy of the state; it derives UI from the events.

The IPC surface is paired commands (`start_dictation_session`, `end_dictation_session`, and the deferred `cancel_dictation_session`) plus the typed event stream wired via `tauri-specta::collect_events!` and the `Event` derive. Each side of the boundary has one job: Rust authors the lifecycle, TypeScript renders it.

Rust owns the state machine because the *boundaries* of each stage are only legible from Rust:

- `Recording` ends when audio capture is drained — known by `pipeline::audio::AudioSession::stop()` returning, not by the TypeScript caller awaiting an IPC roundtrip
- `Loading` ends when the Cleanup engine's worker thread has produced cleaned text AND `insertion::paste()` has returned — both Rust-side events
- The error envelope per stage (`asr` / `cleanup` / `paste`) is naturally where the failure is observed

A TypeScript-owned state machine would necessarily lag the truth — derived from IPC return values and event subscriptions anyway, but with the additional cost of being a second source of truth that can drift.

## Considered alternatives

- **Shape A — TypeScript owns the state machine, Rust is mechanical** — rejected: every UI state transition that TS needs to render (Recording started, transcription done, paste landed) is observed in Rust first. TS subscribing to events to track state, while Rust internally tracks the same state, is a duplication. Picking one owner — Rust, where the boundaries originate — removes the duplication.
- **Shape B — Single `run_dictation_session()` command, no event stream** — rejected: collapses the press/release pair into one command, then needs a separate stop-signal channel anyway (because release is genuinely a separate event). The "one command" framing is partially illusory. Also requires synthesising session identity into the TS API (`Promise<DictationSession>` with `.cancel()`) which doesn't map cleanly to Tauri commands.
- **No event stream, IPC return values only** — rejected: `end_dictation_session` would block the IPC roundtrip for the full ASR + Cleanup window (~300-800ms). During that window the Pill window has no signal to render the Loading state; the only path is the TS handler emitting a side-channel notification, which is just an ad-hoc event stream with worse typing.

## Accepted cost

Net-new event surface (~4 typed events) and the worker-thread plumbing inside `Session` for async inference. The trade is a slightly heavier `Session` impl for a cleaner ownership boundary; every later slice (waveform amplitude in #5, gesture decoder in #6, voice commands in #8, Speechcraft subscriber in #9, error toast in future) attaches to the same event stream with no new IPC commands required.
