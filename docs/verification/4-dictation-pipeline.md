# Manual verification — issue #4 (real dictation: ASR + Cleanup pipeline)

Walk this checklist on the `feat/4-dictation-pipeline` branch before opening the PR. Each section pairs an acceptance criterion from the brief with the concrete command, path, or observation that proves it.

Concrete values (file sizes, SHAs, exact filenames) are filled in as the relevant sub-slice lands. Anything tagged `[fill]` means the sub-slice that produces it has not yet shipped.

---

## A. Models on disk

**Target directory** (per ADR 0005 bundle identifier):

```
~/Library/Application Support/io.github.canhassancode.type-less/models/
```

**Expected after a successful install** (filenames + sizes come from `models.json`):

```bash
ls -lh ~/Library/Application\ Support/io.github.canhassancode.type-less/models/
```

| filename | approx size | source |
| --- | --- | --- |
| `[fill: whisper filename]` | `[fill: bytes]` | Hugging Face (`[fill: repo]`) |
| `[fill: qwen filename]` | `[fill: bytes]` | Hugging Face (`[fill: repo]`) |

**During an in-flight download**, a `<filename>.partial` sibling exists. On SHA-256 verify it is atomically renamed to the final filename. To observe:

```bash
watch -n 1 'ls -la ~/Library/Application\ Support/io.github.canhassancode.type-less/models/'
```

**SHA verification by hand** (paranoia check):

```bash
shasum -a 256 ~/Library/Application\ Support/io.github.canhassancode.type-less/models/<filename>
# expect: matches sha256 in models.json
```

---

## B. First-run notification + hotkey gating

1. Remove any prior models: `rm -rf ~/Library/Application\ Support/io.github.canhassancode.type-less/models/`.
2. `pnpm tauri dev`.
3. **Expect:** one macOS notification: *"type-less needs to install its language models. Open Settings to begin."*
4. Press the hotkey (`Cmd+Shift+.`). **Expect:** nothing happens (no Pill, no paste). This proves the hotkey is registered only when engines are loaded.
5. Tray icon → Settings is reachable; press the hotkey while Settings is focused — still silent.

---

## C. Download flow (Settings → Models)

1. Open the tray icon → click to open Settings.
2. The **Models** section shows one row per entry in `models.json`. Both initially render as "Not installed" with a **Download** action.
3. Click **Download** on one row.
4. **Expect:** row transitions through `Downloading X%` (updating live) → `Verifying` → `Installed`.
5. Click **Download** on the other row.
6. After both rows show "Installed":
   - The macOS notification from B does **not** re-fire.
   - The hotkey now functions (verify in section D). No app restart required — the `engine-state-changed: ready` event flips the hotkey gate.
7. The **Developer tools** disclosure (`<details>`) below the Models section contains the four pre-existing buttons (Show Settings, Show Pill, Show Overlay, Hide Overlay).

---

## D. Dictation golden path

Pre-requisites: both models installed (see C).

1. Place the Insertion Point in a real text field (Notes.app, Messages, a browser textarea).
2. Hold the hotkey ~2-3 seconds while saying "hello world".
3. **Expect, in order:**
   - Pill appears at bottom-centre showing the **Recording** state (existing dot / "REC" label).
   - On release, Pill stays visible and switches to a distinct **Loading** state (animated dot or grey colour change — anything visually distinct from Recording).
   - Within ~1 second of release, cleaned text (`Hello, world.` or similar) appears at the Insertion Point.
   - Pill hides immediately after the paste lands.

---

## E. Cleanup quality

The bar is what `crates/pipeline/prompts/cleanup_v1.txt` actually claims: punctuation + capitalisation, contextual filler removal (`like`/`you know` stripped only as fillers), homophone fixes (their/there/they're, its/it's, your/you're, to/too/two), paragraph breaks on topic shift, empty-input handling, no paraphrase, no preamble, no quote-wrapping. Acronyms (e.g. `API`) preserved.

Sub-slice 4 ships exploration tests (`cargo test -p pipeline cleanup::tests::exploration_*`) that probe these claims plus the unstated capability boundary (long-form, numbers/units, technical vocab, restarts, hedging vs filler). The hand-walk below mirrors a subset for end-to-end confirmation; if anything here surprises you compared to the exploration test outputs, that signals a paste-path / engine-state regression rather than a model issue.

Dictate each phrase via the hotkey. Capture the pasted output. Mark each cell ✅ / ❌ / 🟡 (close-but-not-quite) as you go.

### E1. Claims directly stated in cleanup_v1.txt

| # | Spoken phrase | Expected (per prompt) | Filler removed | Punctuation | No paraphrase | No quote-wrap | No preamble |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | "hello world" | `Hello, world.` | n/a | | | | |
| 2 | "um so I was thinking we should ship the feature tomorrow" | `So I was thinking we should ship the feature tomorrow.` | "um" gone | | | | |
| 3 | "their going to the store there are three of them" | `They're going to the store. There are three of them.` | n/a | | | | |
| 4 | "yeah you know its really good" | `Yeah, it's really good.` | "you know" gone | | | | |
| 5 | "lets talk about the design first then we can move on to implementation we need to lock the api shape before anyone writes code" | Two sentences, blank line between (paragraph break on topic shift). `API` capitalised. | n/a | | | | |
| 6 | "um uh" | empty string (no paste) | all filler | n/a | n/a | n/a | n/a |

### E2. Filler vs literal-meaning distinction (prompt explicitly claims this)

| # | Spoken phrase | Filler word | Expected handling |
| --- | --- | --- | --- |
| 7 | "I felt like running today" | "like" | KEPT (comparison, not filler) |
| 8 | "it was like really cold" | "like" | REMOVED (filler) |
| 9 | "you know what I mean" | "you know" | KEPT (literal) |
| 10 | "this is you know really good" | "you know" | REMOVED (filler) |

### E3. Long-form (capability probe)

| # | Spoken phrase | What to look for |
| --- | --- | --- |
| 11 | Dictate ~30s continuous speech with two distinct topics (e.g. talk about the project for ~15s, then shift to "anyway, on a separate note" and talk about something unrelated for ~15s) | Single paragraph break at the shift; no preamble/postamble; sentence boundaries plausible; no paraphrase of the body. |
| 12 | Dictate ~60s with no natural topic shift (one continuous thought, ~150 words) | Sentences broken at plausible pause points; no spurious paragraph breaks; output length within ~10% of input word count (no significant dropout). |

### E4. Restart / false-start handling (capability probe — v1 prompt says "no paraphrase", so restarts should stay)

| # | Spoken phrase | Per-prompt expected | Notes |
| --- | --- | --- | --- |
| 13 | "the meeting is at three — actually four" | Restart preserved verbatim, possibly with em-dash punctuation. NOT collapsed to "The meeting is at four." | If the model collapses, it has paraphrased — violates v1 rule. Note as 🟡 if quality feels better than the rule, ❌ if quality feels worse. |
| 14 | "I was going to write the docs first but actually let me think the api needs to land first" | Both halves preserved; punctuation inferred. | Same — should not be edited to a single clean sentence. |

### E5. Numbers / units / dates (unclaimed capability — probing)

The prompt does not explicitly handle number formatting. These tests reveal the model's default behaviour.

| # | Spoken phrase | What to record |
| --- | --- | --- |
| 15 | "the timeout is three hundred and forty two milliseconds" | Does it write "342 ms", "342 milliseconds", "three hundred and forty two milliseconds"? Just record. |
| 16 | "lets meet on march fifteenth at two pm" | Does it write "March 15th at 2pm", "March 15 at 2:00 PM", spelled-out? Record. |
| 17 | "the file is about five hundred megabytes" | "500 MB" vs "500 megabytes" vs spelled-out. Record. |

### E6. Technical / code vocab (unclaimed capability — probing)

| # | Spoken phrase | What to record |
| --- | --- | --- |
| 18 | "the function check user auth returns a boolean" | Does it produce `checkUserAuth`, `check_user_auth`, or plain prose "check user auth"? Record. |
| 19 | "the api endpoint is slash v one slash users" | Does it produce `/v1/users` or "slash v one slash users"? Record. |
| 20 | "import react from react" | Hopeful: `import React from 'react'` (likely doesn't happen on 1.5B). Record. |

### E7. Hedging preservation vs filler removal (prompt-claimed distinction)

| # | Spoken phrase | Expected |
| --- | --- | --- |
| 21 | "I think we should ship it tomorrow" | "I think" KEPT (hedging is the speaker's word choice, not filler). |
| 22 | "you know like um I think we should ship it" | "you know", "like", "um" REMOVED. "I think" KEPT. |

### E8. Proper nouns / capitalisation

| # | Spoken phrase | Expected |
| --- | --- | --- |
| 23 | "I met sarah from product yesterday" | "Sarah" capitalised. "Product" — judgement call by the model. |
| 24 | "we use postgres on aws" | "Postgres on AWS" ideally. Record actual. |

### E9. Failure surfaces to actively watch for

If any of these appear in the paste output during E1–E8, that's a `cleanup_v1.txt` failure:

- Preamble: `Here is the cleaned text:` / `Cleaned:` / `Output:` / `Sure, here is...`
- Quote-wrap: surrounding `"..."` or backticks
- Code fences: ```` ``` ```` around output
- Commentary or "notes" appended after the cleaned text
- Apologies ("Sorry, I couldn't process that...")
- Translation to another language
- Spelling locale shift (input was British → output is American, or vice versa)
- Filler kept ("um", "uh", "er", "hmm")
- Paraphrase: words the speaker didn't say, examples added, content removed beyond filler
- Spurious blank lines inside a single thought (paragraph breaks should only appear on topic shift)

### E10. Bar interpretation

After E1–E8, the slice is shippable if:

- **All of E1, E2, E7, E8** pass at expected output. These are claims the prompt makes explicitly — failures here are prompt or model bugs to fix in this slice.
- **E3 long-form** is acceptable quality (no significant content dropout, no hallucinations, plausible sentence boundaries). Empirical bar; if it visibly degrades on long-form, that's a candidate for prompt iteration or model bump within this slice.
- **E4 restarts, E5 numbers, E6 code** are *recorded behaviours*, not pass/fail. Their results inform whether we ship v1 as-is, iterate the prompt, or bump the model before opening the PR.
- **E9 failure surfaces** show zero hits. Any hit = block.

---

## F. Latency (timing harness)

```bash
cargo run --bin timing
# expect: p95 < 1000ms; binary exits 0
```

If p95 exceeds 1000ms on M2-class hardware, this is a regression bar failure — the binary asserts and exits non-zero.

---

## G. Network containment (ADR 0010)

1. Confirm both models are on disk (section A).
2. In a second terminal: `sudo nettop -P -p $(pgrep type-less)`.
3. In the app, run a dictation session (section D).
4. **Expect:** zero `bytes_in` / `bytes_out` on the type-less process during the dictation. Network traffic appears **only** while a Re-download is in flight in Settings → Models.

---

## H. Failure paths

### H1. Network drop mid-download
1. Start a download in Settings → Models.
2. Kill network (turn off Wi-Fi) part-way through.
3. **Expect:** row transitions to **Failed** with a **Retry** button. The error message is human-readable.
4. Restore network, click **Retry**.
5. **Expect:** download resumes from the byte offset of the existing `.partial` (HTTP Range request), not from zero. Verify by watching the `.partial` size — it grows from its prior size, not from 0.

### H2. Corrupted model on disk
1. With both models installed, corrupt one: `echo zzz >> <model_path>`.
2. Restart the app (`pnpm tauri dev`).
3. **Expect:**
   - First-run notification fires again (because `installation_status()` sees a SHA mismatch).
   - Settings → Models shows that row as "Failed" with a **Re-download** action.
   - Hotkey is gated (silent) until re-download completes.

### H3. Disk-space precheck
1. (Optional, destructive — skip if disk is tight.) Fill disk to within 2 GB of free.
2. Click Download.
3. **Expect:** immediate failure with a "not enough disk space" error before any bytes are written.

---

## I. CI-equivalent checks

Run locally to mirror what CI would do (slice #16 lands CI proper):

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
pnpm test
pnpm --filter @type-less/app typecheck
pnpm lint
```

All five must be green.

---

## J. Audit: no production network calls outside `download_model`

Static check:

```bash
rg -n "fetch\(|XMLHttpRequest|reqwest::|isahc::|ureq::" -g '!docs/**' -g '!**/*.test.*'
# expect: hits live only inside the download_model command and its helpers, and inside tauri-plugin-http if used.
```

Dynamic check is covered by section G.

---

When every section is green, the slice is shippable. Open the PR with this checklist's outcome summarised in the PR body, then delete this file in the PR (or leave it as a long-form verification record — your call).
