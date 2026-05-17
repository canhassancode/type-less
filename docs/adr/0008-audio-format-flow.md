# Audio capture stays at native rate; resample at the cross-module boundary; stereo-to-mono via channel averaging

`pipeline::audio` captures at the input device's default configuration — typically 44.1 or 48 kHz F32, often stereo on built-in MacBook microphones. The captured buffer is returned as `CapturedAudio { samples: Vec<f32>, sample_rate: u32, channels: u16 }`. A separate `pipeline::resample` module converts `CapturedAudio` to the 16 kHz mono F32 format that `pipeline::asr` requires, using `rubato`'s FFT-based path. The stereo-to-mono step averages left and right channels (`(L + R) * 0.5`), not picking channel 0.

The format flow is pinned across three modules:

```
cpal → AudioSession::stop() → CapturedAudio
                              ↓ resample::to_whisper_format
                              ↓ Vec<f32> @ 16 kHz mono
                              ↓ asr::transcribe
                              ↓ String
```

Each module's interface is a function of its own contract. `pipeline::audio` knows nothing about Whisper's input requirements. `pipeline::asr` knows nothing about cpal's native formats. `pipeline::resample` is a pure function — trivially unit-testable, no I/O, no hardware coupling.

The native-rate `CapturedAudio` shape is also what slice #5 (live waveform) needs: amplitude visualisation reads better at the device's native sample rate. Resampling earlier — inside `AudioSession::stop()` — would either force the waveform feature to maintain a parallel high-rate buffer, or accept a degraded visualisation. Returning `CapturedAudio` from `stop()` lets #5 sample raw amplitudes during capture without duplication.

## Considered alternatives

- **Resample inside `pipeline::audio::AudioSession::stop()`** — rejected: couples the Audio module to Whisper's requirements (16 kHz mono). Future-tense: if the eval suite or a phase-2 streaming consumer wants native-rate audio, the Audio module would either have to expose a second method or undo the conversion. Worse, the live waveform in #5 would have to maintain a parallel pre-resample buffer to render amplitude at native fidelity.
- **Resample inside `pipeline::asr::transcribe()`** — rejected: ASR's contract becomes `transcribe(samples: &[f32], rate: u32, channels: u16) -> String`. Three arguments where one is morally constant in this codebase. Future consumers of ASR (the eval CLI, regression-bar tests) all want the same input shape; doing the conversion once at the boundary is cleaner than per-call.
- **Hand-rolled linear resampling** — rejected: ratio-heavy resampling (48 kHz → 16 kHz, factor 3) introduces audible aliasing that Whisper handles less accurately than properly band-limited resampling. The 5-10ms cost of a quality FFT resample is invisible in the latency budget.
- **`samplerate` crate (libsamplerate bindings)** — rejected: adds a C dependency where the rest of the audio pipeline is pure-Rust. `rubato` is mature, FFT-based, and dependency-light.
- **Stereo-to-mono via channel 0 only** — rejected: drops half the signal information. For a MacBook built-in microphone, both channels usually carry identical signal, so the choice is invisible. But for stereo USB microphones facing two speakers in a meeting, averaging captures both speakers; channel-0-only loses one. Averaging also matches `whisper.cpp`'s own preprocessing default, keeping our pipeline aligned with the reference implementation.
- **Stereo-to-mono via max-magnitude** — rejected: introduces non-linearity that complicates downstream amplitude visualisation in #5 and biases the audio toward whichever channel happens to be louder, hiding genuine binaural content.

## Accepted cost

`CapturedAudio` is a new cross-module type — one struct in `pipeline::audio`, exported. Three modules instead of two (`audio` → `resample` → `asr`). The cost is small (one extra type, ~30 LoC for the resample module) and the testability win is large: the resampler is a pure function with golden-vector tests, decoupled from cpal entirely.

## Accepted failure mode

Stereo recordings of two phase-inverted signals — physically rare in dictation contexts (essentially never) — would phase-cancel to silence under averaging. The realistic dictation cases (built-in mono mic upmixed to stereo by macOS, headset mono mic, USB mono mic, USB stereo mic with one speaker) all behave correctly or strictly better than channel-0-only.
