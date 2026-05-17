# IPC commands are named after the domain primitive they model

The Tauri commands that bracket a Dictation Session are named `start_dictation_session`, `end_dictation_session`, and (deferred to slice #6) `cancel_dictation_session`. Not `start_session` / `stop_session`. Not `run_dictation_session`. The names match the canonical term in `CONTEXT.md` exactly.

Every later slice's IPC additions follow the same convention — command names quote the domain term in `CONTEXT.md`, no domain-suffix abbreviation, no gesture-flavoured shorthand. The tracer slice (#3) shipped `start_session` / `stop_session` before this convention was made explicit; the rename was the first act of slice #4.

The naming pattern is load-bearing because the Activation Gesture Decoder (#6), the voice-command extension (#8), the Speechcraft levelling subscriber (#9), and the Onboarding flow (#12) all sit on top of these commands and inherit their names into their own logic. Reversing the convention later means a multi-file rename across every slice in the v1 milestone — possible, but expensive, and the failure mode (some slices renamed, others not) leaves the vocabulary fragmented.

## Considered alternatives

- **Gesture-flavoured pair** (`start_session` / `stop_session`) — rejected: "session" is overloaded in software vocabulary (HTTP session, auth session, terminal session, telemetry session). A reader landing on the command from `Cmd-click` has to infer from context which kind of session this is. The domain-oriented variant resolves it in the name.
- **Single command modelling the whole pipeline** (`run_dictation_session`) — rejected: the press and the release are genuine separate events; collapsing them into one command forces a stop signal across IPC anyway (via a second command, an event, or a one-shot channel). The "one command" framing is partially illusory once the implementation is written out.
- **Verb-first generic naming** (`session_start` / `session_end`) — rejected: stylistically inconsistent with Tauri 2 conventions, which use `verb_noun` ordering for commands.

## Accepted cost

The rename of `start_session` → `start_dictation_session` touched 5 call sites + 1 test string from slice #3's PR (`crates/tauri-shell/src/main.rs`, `modules/app/src/features/activation/index.ts`, `bindHotkey.test.ts`). `bindings.ts` regenerated automatically. One-off cost; future slices ship the long-form name from the start.
