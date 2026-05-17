# Offline-first: one-time model download is the only permitted network IO

type-less makes one category of network request, and one only: downloading model files (Whisper and Qwen) from their canonical Hugging Face URLs, listed in the root-level `models.json`. After the model files are present on disk and verified by SHA, the app makes zero network calls for the rest of its lifetime — across every session, every relaunch, indefinitely.

No telemetry. No crash reporting. No usage stats. No "check for updates." No "new model available" probes. No share-to-social. No analytics, including privacy-preserving analytics. No CDN fetches for fonts, images, sounds, or any other Theme asset. No license-key validation. No A/B-test config fetch.

Tauri's capability system enforces this at the trust boundary: the HTTP capability is allowlisted to model-host domains only (Hugging Face). A future regression that calls `api.example-telemetry.com` is rejected by the runtime, not just by code review.

The principle is load-bearing because:

1. **The product promise to the user.** type-less is a dictation tool that hears every word the user speaks. The promise of "your voice never leaves your machine" is meaningful only if there is no plausible side channel that could undermine it. Even a sandbox-respecting analytics call from the renderer process visibly opens the door.

2. **The trust gradient of dual-purpose code.** Once one network primitive lives in the codebase for a "legitimate" reason (auto-update, crash reporting, model-update probe), the cost-to-add for the *next* one drops, regardless of whether that next one is justifiable. The way to keep the surface zero is to keep the surface zero.

3. **The portability of the product.** type-less runs identically on a laptop in a faraday cage. This isn't a side-effect; it's a requirement for the target user (privacy-conscious individual on a plane, in a hospital, in a SCIF, under a flaky-wifi roof). Any feature that quietly assumes connectivity erodes this.

## Considered alternatives that this ADR rejects

- **Sentry SDK for crash diagnostics** — rejected: phones home by construction. Useful for debugging; incompatible with the product promise.
- **Auto-update infrastructure** (Tauri's updater plugin, Squirrel, Sparkle) — rejected: requires periodic network checks. Users update by downloading new `.dmg`s from GitHub Releases manually.
- **"Check for new model versions" on launch** — rejected: a model-update probe is still a network probe, even if it's against a model-host domain. Users get new models when they manually re-download via Settings → Models with a new `models.json` shipped in a release.
- **Privacy-preserving analytics** (PostHog with no PII, Plausible, etc.) — rejected: still a network call. The product promise outranks the diagnostic convenience.
- **Sharing features** ("Share my Speechcraft Level to Bluesky") — rejected: any social-share button requires network. Out of scope permanently.
- **License-key online validation** — N/A (MIT licensed, no licensing), but listed to lock the principle should someone propose a commercial fork pattern later.

## Allowed exceptions and how they are scoped

- **Model download via `download_model` Tauri command** — the single permitted egress. Implemented in Rust (`reqwest` or `tauri-plugin-http`), capability-scoped to Hugging Face hosts. Only called from the Model Download Manager. User-triggered (Settings → Models → Download) or part of the Onboarding flow (#12); never automatic background polling.
- **Dependency choices that pull in network code at compile time but never invoke it** — acceptable provided the runtime call surface stays empty. New crate/npm dependencies are reviewed for runtime network primitives during PR review.

## Accepted cost

Genuine product capabilities are forfeited: in-product crash diagnostics, frictionless auto-update, model-staleness warnings, social sharing, behavioural analytics. We trade them for a clean, defensible privacy claim that the user can verify by watching their firewall.
