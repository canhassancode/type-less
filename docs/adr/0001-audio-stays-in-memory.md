# Audio stays in memory in production builds

Microphone audio captured during a Dictation Session is held only as an in-memory PCM buffer (16 kHz mono `f32`) and discarded immediately after ASR returns. Production builds never write audio to disk — not as a temp WAV, not as a cache, not as a debug artefact.

The reason is structural rather than policy: "audio cannot leak" must be true by construction. Disk-resident audio (even briefly) creates recovery, backup-sync, and accidental-disclosure vectors that no amount of cleanup discipline can fully close. Any future debug tooling that wants to persist audio (e.g. to diagnose a transcription bug) must be gated behind an explicit non-default developer build flag, never enabled in shipped binaries.
