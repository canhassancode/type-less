# Clipboard paste with restore is the sole text-insertion strategy in MVP

Cleaned text is delivered to the cursor by snapshotting the user's current clipboard, writing the cleaned text, synthesising `Cmd+V` (macOS) or `Ctrl+V` (Windows), then restoring the original clipboard ~200ms later. This is the only insertion strategy in MVP — no per-app fallback, no detection of failure.

## Considered alternatives

- **Simulated keystrokes** — rejected: visible character-by-character typing on long dictations feels unprofessional, scales linearly with output length, and triggers IDE autocomplete/intellisense interference.
- **OS Accessibility API direct write** (`AXUIElement` on macOS, `UIAutomation` on Windows) — rejected: coverage is patchy and fails *silently* in Electron, Chrome, terminals, and many games. A strategy that works "most of the time but mysteriously breaks in $popular_app" is worse than one with a documented universal limitation.

## Accepted failure modes

- Clipboard managers that snapshot on change (Paste, Maccy, Raycast) will record a brief artefact between write and restore. Documented; a settings toggle to skip restore is available for users who prioritise clipboard-history hygiene.
- Citrix and some remote-desktop apps block clipboard injection entirely. Documented; not solvable without a different strategy.
- A few apps intercept `Cmd+V` without reading the clipboard, producing a literal `v` character. Inherited from Wispr Flow's identical approach; documented in the troubleshooting section.
