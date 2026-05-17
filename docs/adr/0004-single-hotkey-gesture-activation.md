# Single hotkey, gesture-based dual activation

A single global Activation Hotkey supports both Activation Modes via discrete gestures: hold for Push-to-Talk (release ends the session), double-tap to enter Toggle (single-tap-while-toggled ends the session). Matches Wispr Flow's pattern.

The gesture vocabulary is now load-bearing — the entire activation UX assumes "hold" and "double-tap" are unambiguous user intents, not timing accidents.

## Considered alternatives

- **Two separate hotkeys** (one for each mode) — rejected: doubles hotkey-conflict surface with other apps and forces the user to remember two keybindings for one conceptual action.
- **Time-threshold switching** (hold > 200ms = Push-to-Talk, tap < 200ms = Toggle) — rejected: intended-tap can be misread as short-hold (recording ends before the user has spoken), and intended-hold as tap (user is unexpectedly trapped in Toggle). The failure modes are timing-dependent and impossible to reason about. Gestures, by contrast, are intentional acts and never accidentally produced.
