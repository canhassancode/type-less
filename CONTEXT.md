# type-less

An offline, lightweight AI dictation tool. The user holds a global hotkey, speaks, releases — cleaned text is inserted at the **Insertion Point** (the focused text field, wherever a manual `Cmd+V` would land). Every N words, a Skyrim-style speechcraft level-up overlay fires as the product's signature flourish.

## Language

### Dictation pipeline

**Dictation Session**:
One activation-to-deactivation cycle producing one transcript that flows through the pipeline and lands at the **Insertion Point**. Started and ended by the user via the **Activation Mode**.
_Avoid_: Recording, capture, utterance

**Insertion Point**:
The element that receives the cleaned text — defined by **keyboard focus**, not mouse position. Whatever a manual `Cmd+V` (macOS) or `Ctrl+V` (Windows) would target right now. If no text field is focused, the paste lands nowhere useful (documented edge case).
_Avoid_: Cursor (overloaded — colloquially means mouse pointer; we never mean the mouse), caret position

**Activation Mode**:
How the user starts and stops a **Dictation Session**. Two modes are supported: **Push-to-Talk** and **Toggle**. A single hotkey supports both via gesture — hold for Push-to-Talk, double-tap to enter Toggle, single-tap-while-toggled to end Toggle. The hotkey is registered only while the **ASR** and **Cleanup** engines are loaded and the model files are present and verified; outside that state, the hotkey does nothing.
_Avoid_: Trigger mode, recording mode

**Push-to-Talk**:
Activation Mode where the **Dictation Session** lasts exactly as long as the hotkey is held. Release ends the session.
_Avoid_: Hold mode, PTT

**Toggle**:
Activation Mode where one gesture starts the **Dictation Session** and a second gesture ends it. The session is active hands-free between the two.
_Avoid_: Tap-tap, latched mode, lock mode

**ASR**:
Automatic Speech Recognition — the stage that turns the captured audio of a **Dictation Session** into a literal transcript.
_Avoid_: Transcription engine, STT (use ASR consistently)

**Cleanup**:
The local-LLM stage that turns a raw ASR transcript into polished text — punctuation, capitalisation, filler removal, unambiguous recognition-error fixes, paragraph breaks on topic shift, and a minimal voice-command set (`new line`, `new paragraph`, `period`, `comma`, `question mark`). A strict task: must not paraphrase or add content.
_Avoid_: Post-processing, formatting, editing

**Cleanup Context**:
Optional context hints passed to the **Cleanup** stage describing the active app and (in future) user profile, custom dictionary, or session history. Always `Generic` in MVP; phase 2 lights up app-specific behaviour for Slack, Email, and Claude Code (the last including verbal slash-command recognition).
_Avoid_: App context, hints

### System surfaces

type-less has no dock icon and no top menu bar — the **Menu-bar Tray Icon** is the sole persistent OS affordance. The **Pill** and **Overlay** are indication-only (the user does not interact with them).

**Menu-bar Tray Icon**:
The status-item in the top-right of the macOS menu bar (alongside WiFi, Battery, Spotlight). type-less's sole persistent affordance — click to summon the **Settings Popover**, right-click for a small contextual menu (Settings, Quit). Distinct from the **Pill** (indication, not affordance) and the **Overlay** (ephemeral celebration).
_Avoid_: Tray icon (ambiguous — Windows "system tray" is a different surface), status-bar icon, taskbar icon, menu-bar icon (omits "tray" — clashes with the Top Menu Bar below)

**Settings Popover**:
The window that opens anchored beneath the **Menu-bar Tray Icon** when the user clicks it. Hosts all user-configurable settings (hotkey, Theme, Pill Visibility Mode, Overlay toggles). Decorated, non-transparent, not always-on-top — a conventional macOS menu-bar-app popover. Closes on blur or repeat tray-click.
_Avoid_: Settings window (it is not free-floating; it anchors), preferences (we say "settings" everywhere), settings panel (generic)

**Top Menu Bar**:
The per-app `File / Edit / View / Window / Help` strip at the very top of the macOS screen, present when a regular app is focused. type-less does **not** have one (`LSUIElement: true` in the bundle plist suppresses it, along with the dock icon). Called out only to disambiguate the overloaded word "menu bar" — say **Top Menu Bar** when you mean this, **Menu-bar Tray Icon** when you mean the status area.
_Avoid_: App menu, menu bar (ambiguous on its own)

### Recording feedback

**Pill**:
A floating, always-on-top, transparent, click-through indicator anchored at bottom-centre of the **Focused Display**. The continuous feedback channel for **Dictation Session** state. Distinct from the **Menu-bar Tray Icon** (affordance, not indication) and the **Overlay** (ephemeral celebration). Cycles: Hidden → Slit (idle) → Expanded with live waveform during a session → Loading during **ASR** and **Cleanup** → Slit/Hidden after paste.
_Avoid_: Indicator, tray pill, status bar

**Pill Visibility Mode**:
User-configurable. Two options: **Always Visible** (Pill rests in Slit when idle) or **Only When Dictating** (Pill is Hidden when idle).
_Avoid_: Pill mode, indicator setting

**Focused Display**:
The monitor containing the currently focused window — the window that will receive the pasted text. The **Pill** anchors here. Focus-bound, not cursor-bound.
_Avoid_: Active display, primary display, cursor screen

### Levelling and reward

**Speechcraft Level**:
The user's current dictation rank, 1 through 100. Increases when the cumulative lifetime word count crosses the threshold for the next level, following an escalating curve. Hard-capped at 100 within a tier.
_Avoid_: Rank, tier, XP level

**New Speech+ Tier**:
After reaching **Speechcraft Level** 100, the user may opt in to "New Speech+", which resets the displayed level to 1 with a 1.25× multiplier on the level curve. Compounds per tier. The underlying lifetime word counter is never reset — only the displayed level.
_Avoid_: Prestige, rebirth, ascension

**Speechcraft Level-Up**:
The event fired when cumulative dictated words cross the threshold for the next **Speechcraft Level**. Triggers the **Overlay**.
_Avoid_: Achievement, reward, milestone

**Overlay**:
The ephemeral banner + progress bar + sound rendered when a **Speechcraft Level-Up** fires. Visually configurable via the **Theme** system. Distinct from the **Pill** — the Overlay is a momentary celebration, the Pill is continuous state.
_Avoid_: Notification, popup, toast

**Theme**:
A folder containing the visual + audio + text bundle the **Overlay** renders. Manifest-driven (`theme.json`) with image, sound, and font assets. The app ships one CC0-licensed default Theme; users can drop additional Themes into the themes directory. type-less itself never distributes third-party Themes (e.g. Skyrim assets).
_Avoid_: Skin, pack, mod

## Relationships

- A **Dictation Session** produces exactly one raw transcript via **ASR**, which becomes exactly one cleaned text via **Cleanup**, which is inserted once at the **Insertion Point**.
- The **Insertion Point** lives on the **Focused Display** (the **Pill** anchors to the same display, by definition).
- A **Dictation Session** is initiated via an **Activation Mode** (**Push-to-Talk** or **Toggle**), bound to a single global hotkey.
- The **Pill** reflects the live state of a **Dictation Session**, anchored to the **Focused Display**.
- Each **Dictation Session** contributes its word count to the user's lifetime total. When the lifetime total crosses the threshold for the next **Speechcraft Level**, a **Speechcraft Level-Up** fires.
- A **Speechcraft Level-Up** renders the **Overlay** using the currently selected **Theme**.
- The **Menu-bar Tray Icon** is the sole persistent affordance — clicking it opens the **Settings Popover** anchored beneath the icon.
- The **Pill** is continuous indication; the **Overlay** is ephemeral celebration; the **Menu-bar Tray Icon** is the persistent affordance.

## Example dialogue

> **Dev:** "When the user releases the hotkey mid-word during **Push-to-Talk**, does the **Dictation Session** still produce a transcript?"
> **Domain expert:** "Yes — release ends the session. **ASR** transcribes whatever audio was captured, even if it's cut off. **Cleanup** handles partial-word recovery if it can."
>
> **Dev:** "Does a **Speechcraft Level-Up** fire mid-session if the session's word count crosses the threshold?"
> **Domain expert:** "It fires at the *end* of the session that crosses the threshold, after the text lands, so the **Overlay** doesn't compete with the dictation flow."
>
> **Dev:** "If a user reaches **Speechcraft Level** 100 and doesn't opt into **New Speech+**, what happens on the next session?"
> **Domain expert:** "The lifetime word count keeps incrementing, but no **Speechcraft Level-Up** fires — the displayed level stays at 100. Opting into New Speech+ at any time resumes levelling at the new tier."

## Flagged ambiguities

- *(none yet)*
