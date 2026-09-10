# Local Speech-to-Text Integration Plan
## Replace Azure Speech-to-Text with Local Silero VAD + Moonshine Streaming, while keeping Azure OpenAI for question understanding and answer generation

**Target host application:** Vysper (Electron / Node.js)  
**Reference implementation for audio architecture:** VoiceTypr (Tauri / Rust / React)  
**Primary platform:** Windows 10/11 x64  
**Target hardware:** 16 GB RAM Windows laptop  
**Primary language:** English  
**Primary use case:** Real-time transcription of spoken technical/professional questions, followed by Azure OpenAI processing.

---

# 1. Objective

The current host application uses Azure Speech Services for speech recognition.

The goal is to remove the cloud Speech-to-Text dependency and replace it with a completely local speech pipeline:

```text
Microphone
    ↓
Local Audio Capture
    ↓
Normalize + Resample to 16 kHz Mono PCM
    ↓
Silero VAD
    ↓
Moonshine Streaming Small
    ↓
Live Partial Transcript
    ↓
Silero End-of-Utterance Detection
    ↓
Final Raw Transcript
    ↓
Azure OpenAI
    ↓
Infer Intended Question + Generate Answer
    ↓
Application UI
```

Azure OpenAI remains cloud-based and is used only **after a final transcript is available**.

Azure Speech SDK must no longer be required for STT.

---

# 2. Core Design Decisions

## ADR-001 — Host application

Keep the existing Electron/Node.js host application.

Do **not** rewrite the full application in Tauri.

VoiceTypr is used as an architecture and code-reference source for robust audio handling.

---

## ADR-002 — Audio capture

Use a dedicated **native Rust sidecar** for:

- microphone enumeration
- microphone capture
- channel conversion
- sample normalization
- resampling
- VAD
- STT orchestration

The Electron process communicates with the native sidecar through stdin/stdout IPC.

Reason:

- isolates native audio/model crashes
- prevents Electron renderer/main-process stalls
- gives access to Rust `cpal`
- gives access to robust real-time audio patterns
- makes STT engine replaceable
- prevents model inference from blocking Electron

---

## ADR-003 — Canonical audio format

All audio after preprocessing must use:

```text
Sample rate: 16,000 Hz
Channels:    1
Sample type: float32
Range:       -1.0 to +1.0
Language:    English
```

Input microphones may provide:

- 48 kHz stereo
- 48 kHz mono
- 44.1 kHz stereo
- 44.1 kHz mono
- 16 kHz mono

All formats must be normalized to the canonical format.

---

## ADR-004 — Voice Activity Detection

Use **Silero VAD ONNX** locally.

Initial configuration:

```text
speech_start_threshold = 0.55
speech_continue_threshold = 0.40
min_speech_ms = 250
pre_roll_ms = 250
post_roll_ms = 150
endpoint_silence_ms = 900
max_utterance_ms = 30000
sample_rate = 16000
```

These values must be configurable.

---

## ADR-005 — Speech-to-text

Use:

```text
Moonshine Streaming Small
Language = English
Local CPU inference initially
```

Do not use Parakeet in V1.

Do not use Whisper in V1.

Do not use Azure Speech Services in V1.

---

## ADR-006 — Final question processing

Use Azure OpenAI after endpoint detection.

Azure receives the final raw transcript.

Azure performs both:

1. infer the intended question despite small ASR spelling/phonetic errors
2. generate the answer

Do this in **one model call**, not two separate calls.

---

## ADR-007 — Streaming

There are two independent streaming paths:

```text
Moonshine
    ↓
partial transcript events
    ↓
Electron UI

Azure OpenAI
    ↓
streamed response tokens
    ↓
Electron UI
```

---

## ADR-008 — STT engine abstraction

The application must expose an STT interface so Moonshine can later be replaced.

Conceptual interface:

```rust
pub trait StreamingSpeechEngine {
    fn load(&mut self) -> Result<(), SpeechError>;
    fn start_session(&mut self) -> Result<(), SpeechError>;
    fn push_audio(&mut self, samples: &[f32]) -> Result<(), SpeechError>;
    fn partial_text(&self) -> Option<String>;
    fn finalize(&mut self) -> Result<String, SpeechError>;
    fn reset(&mut self);
}
```

V1 implementation:

```text
StreamingSpeechEngine
       └── MoonshineEngine
```

Future implementations may include other engines without changing Electron IPC.

---

# 3. Existing Host Application Integration Point

The host repository currently has:

```text
speech-recognition.js
src/services/speech.service.js
main.js
package.json
```

The existing `speech-recognition.js` is only a thin wrapper around:

```text
src/services/speech.service.js
```

The existing `main.js` already expects generic speech-service events such as:

```text
recording-stopped
transcription
interim-transcription
status
error
```

Therefore the preferred strategy is:

> **Keep the current speech service public API and event names stable. Replace only the implementation behind the service.**

This minimizes changes in `main.js`.

---

# 4. Current Event Contract That Must Be Preserved

The replacement speech service must continue to emit:

```javascript
speechService.emit("interim-transcription", text);
speechService.emit("transcription", text);
speechService.emit("status", status);
speechService.emit("error", error);
speechService.emit("recording-stopped");
```

It must continue to expose:

```javascript
speechService.startRecording();
speechService.stopRecording();
speechService.getStatus();
```

Therefore the rest of the application should not need to know whether transcription comes from Azure Speech, Moonshine, Whisper, or another engine.

---

# 5. New High-Level Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                    ELECTRON APPLICATION                     │
│                                                             │
│ main.js                                                     │
│      │                                                      │
│      ▼                                                      │
│ speech-recognition.js                                       │
│      │                                                      │
│      ▼                                                      │
│ src/services/speech.service.js                              │
│      │                                                      │
│      ▼                                                      │
│ LocalSpeechBridge                                           │
└──────┼──────────────────────────────────────────────────────┘
       │
       │ JSON control messages
       │ + framed/binary audio if needed
       │
       ▼
┌─────────────────────────────────────────────────────────────┐
│                  NATIVE SPEECH SIDECAR                      │
│                         Rust                                │
│                                                             │
│  CPAL Recorder                                              │
│       ↓                                                     │
│  Audio Normalizer                                           │
│       ↓                                                     │
│  Resampler → 16 kHz Mono f32                                │
│       ↓                                                     │
│  Ring Buffer                                                │
│       ↓                                                     │
│  Silero VAD                                                 │
│       ↓                                                     │
│  Moonshine Streaming Small                                  │
│       ↓                                                     │
│  Partial / Final Transcript                                 │
└──────┼──────────────────────────────────────────────────────┘
       │
       │ stdout JSON events
       ▼
┌─────────────────────────────────────────────────────────────┐
│                   ELECTRON SPEECH SERVICE                   │
│                                                             │
│ partial  → interim-transcription                            │
│ final    → transcription                                    │
│ status   → status                                           │
│ error    → error                                            │
└──────┬──────────────────────────────────────────────────────┘
       │
       ▼
main.js
       │
       ▼
Existing application session / LLM flow
       │
       ▼
Azure OpenAI
       │
       ▼
Answer UI
```

---

# 6. Recommended Repository Layout After Integration

```text
Vysper/
│
├── main.js
├── speech-recognition.js
├── package.json
├── preload.js
│
├── src/
│   └── services/
│       ├── speech.service.js                 MODIFY
│       ├── local-speech-bridge.service.js    NEW
│       └── llm.service.js                    EXISTING / ADAPT IF NEEDED
│
├── native/
│   └── speech-sidecar/
│       ├── Cargo.toml                        NEW
│       ├── build.rs                          OPTIONAL
│       │
│       ├── src/
│       │   ├── main.rs
│       │   ├── protocol.rs
│       │   ├── error.rs
│       │   │
│       │   ├── audio/
│       │   │   ├── mod.rs
│       │   │   ├── recorder.rs
│       │   │   ├── converter.rs
│       │   │   ├── normalizer.rs
│       │   │   ├── resampler.rs
│       │   │   ├── device_watcher.rs
│       │   │   ├── recorder_watchdog.rs
│       │   │   ├── level_meter.rs
│       │   │   └── ring_buffer.rs
│       │   │
│       │   ├── vad/
│       │   │   ├── mod.rs
│       │   │   ├── silero.rs
│       │   │   ├── state.rs
│       │   │   └── endpoint_detector.rs
│       │   │
│       │   ├── stt/
│       │   │   ├── mod.rs
│       │   │   ├── engine.rs
│       │   │   └── moonshine/
│       │   │       ├── mod.rs
│       │   │       ├── engine.rs
│       │   │       ├── process.rs
│       │   │       └── session.rs
│       │   │
│       │   └── pipeline/
│       │       ├── mod.rs
│       │       ├── coordinator.rs
│       │       ├── state.rs
│       │       └── events.rs
│       │
│       ├── models/
│       │   ├── silero/
│       │   │   └── silero_vad.onnx
│       │   └── moonshine/
│       │       └── streaming-small/
│       │
│       └── tests/
│           ├── audio_pipeline.rs
│           ├── vad_tests.rs
│           ├── protocol_tests.rs
│           └── endpoint_tests.rs
│
├── scripts/
│   ├── build-speech-sidecar.ps1
│   ├── download-silero.ps1
│   ├── download-moonshine.ps1
│   └── verify-models.ps1
│
└── resources/
    └── speech/
        └── README.md
```

---

# 7. VoiceTypr Files to Reference / Adapt

Reference repository:

```text
https://github.com/moinulmoin/voicetypr
```

---

## 7.1 VoiceTypr audio files

Source folder:

```text
voicetypr/src-tauri/src/audio/
```

Current files of interest:

```text
converter.rs
device_watcher.rs
level_meter.rs
mod.rs
normalizer.rs
recorder.rs
recorder_watchdog.rs
resampler.rs
silence_detector.rs
speech_evidence.rs
```

### `converter.rs`

**Use:** audio sample/channel conversion reference.

**Action:**

```text
REFERENCE / ADAPT
```

Create:

```text
native/speech-sidecar/src/audio/converter.rs
```

Responsibilities:

- convert device sample type to float32
- stereo/multi-channel to mono
- clamp samples
- avoid allocation where practical in hot paths

---

### `device_watcher.rs`

**Use:** microphone disconnect/change handling.

**Action:**

```text
REFERENCE / ADAPT
```

Create:

```text
native/speech-sidecar/src/audio/device_watcher.rs
```

Responsibilities:

- enumerate microphones
- detect device disappearance
- notify coordinator
- stop session safely on device loss
- allow restart with new/default device

---

### `level_meter.rs`

**Use:** audio level calculation.

**Action:**

```text
REFERENCE / ADAPT
```

Create:

```text
native/speech-sidecar/src/audio/level_meter.rs
```

Emit:

```json
{
  "type": "audio_level",
  "value": 0.42
}
```

Throttle UI events to avoid excessive IPC.

Recommended:

```text
10–20 audio-level events per second maximum
```

---

### `normalizer.rs`

**Use:** normalize incoming microphone data.

**Action:**

```text
REFERENCE / ADAPT
```

Create:

```text
native/speech-sidecar/src/audio/normalizer.rs
```

Output:

```text
mono f32
-1.0 to +1.0
```

---

### `recorder.rs`

**Use:** primary CPAL microphone capture architecture.

**Action:**

```text
HIGH PRIORITY REFERENCE / ADAPT
```

Create:

```text
native/speech-sidecar/src/audio/recorder.rs
```

This is one of the most important VoiceTypr references.

Required design:

```text
CPAL callback
   ↓
minimal conversion
   ↓
non-blocking channel/ring buffer
   ↓
return immediately
```

The CPAL callback must never:

- call Moonshine
- call Silero
- call Azure
- perform network I/O
- write large files
- wait for mutexes for long periods
- block on IPC

---

### `recorder_watchdog.rs`

**Use:** robust stop/recovery behavior.

**Action:**

```text
REFERENCE / ADAPT
```

Create:

```text
native/speech-sidecar/src/audio/recorder_watchdog.rs
```

Responsibilities:

- detect stalled recorder
- prevent shutdown deadlocks
- support bounded stop
- report audio failure to coordinator

---

### `resampler.rs`

**Use:** resampling architecture using `rubato`.

**Action:**

```text
HIGH PRIORITY REFERENCE / ADAPT
```

Create:

```text
native/speech-sidecar/src/audio/resampler.rs
```

Canonical output:

```text
16000 Hz
mono
f32
```

Keep resampler state persistent between chunks.

Do not construct a new resampler for every callback.

---

### `silence_detector.rs`

**Use:** only as a reference for where VoiceTypr currently places simple silence logic.

**Action:**

```text
DO NOT COPY AS THE FINAL VAD
```

Replace with:

```text
native/speech-sidecar/src/vad/silero.rs
native/speech-sidecar/src/vad/endpoint_detector.rs
```

---

### `speech_evidence.rs`

**Use:** reference for speech/no-speech decision organization.

**Action:**

```text
REFERENCE CONCEPTS ONLY
```

Merge useful state ideas into the Silero VAD state machine.

---

## 7.2 VoiceTypr transcription files

Source:

```text
voicetypr/src-tauri/src/transcription/
```

Current files:

```text
capabilities.rs
error.rs
executor.rs
request.rs
```

### `executor.rs`

**Use:** architecture reference for transcription orchestration, cancellation, timeouts, and recovery.

**Action:**

```text
HIGH PRIORITY ARCHITECTURAL REFERENCE
```

Do not port all VoiceTypr provider logic.

Create a simpler equivalent:

```text
native/speech-sidecar/src/pipeline/coordinator.rs
```

Responsibilities:

- start microphone
- start/stop Moonshine session
- consume audio
- run VAD
- emit partial text
- detect endpoint
- finalize utterance
- emit final transcript
- handle cancel
- recover from model/audio failure

---

### `error.rs`

**Use:** error taxonomy reference.

Create:

```text
native/speech-sidecar/src/error.rs
```

Recommended error categories:

```rust
pub enum SpeechError {
    MicrophoneUnavailable,
    MicrophoneDisconnected,
    AudioStreamFailed,
    AudioConversionFailed,
    ResampleFailed,
    VadModelLoadFailed,
    VadInferenceFailed,
    MoonshineStartFailed,
    MoonshineInferenceFailed,
    MoonshineExited,
    ProtocolError,
    Timeout,
    Cancelled,
}
```

---

### `request.rs`

**Use:** reference for request/session parameters.

Create configuration model:

```rust
pub struct SpeechSessionConfig {
    pub language: String,
    pub sample_rate: u32,
    pub endpoint_silence_ms: u64,
    pub min_speech_ms: u64,
    pub max_utterance_ms: u64,
}
```

---

## 7.3 VoiceTypr top-level state machine

Reference:

```text
voicetypr/src-tauri/src/state_machine.rs
```

**Action:**

```text
REFERENCE / SIMPLIFY
```

Create:

```text
native/speech-sidecar/src/pipeline/state.rs
```

Use this state model:

```text
Idle
Initializing
Listening
SpeechDetected
Streaming
EndpointDetected
Finalizing
Listening
Stopping
Error
```

---

## 7.4 VoiceTypr Cargo dependencies to inspect

Reference:

```text
voicetypr/src-tauri/Cargo.toml
```

Relevant libraries/patterns include:

```text
cpal
rubato
tokio
serde
serde_json
thiserror
```

Do not copy the full dependency list.

Only add dependencies required by the speech sidecar.

---

# 8. Files That Must NOT Be Imported From VoiceTypr

Do not import unrelated product subsystems such as:

```text
license/
cloud_stt/
remote/
writing/
menu/
history/
updater/
billing/trial code
Whisper-specific implementation
Parakeet-specific implementation
AI-polish provider catalog
cursor insertion
network sharing
file transcription
```

They are not required for this integration.

---

# 9. Vysper Files to Modify

## 9.1 `src/services/speech.service.js`

This is the primary replacement point.

Current Azure Speech implementation must be removed.

The replacement must:

1. extend `EventEmitter` if the existing service uses it
2. create/manage `LocalSpeechBridge`
3. preserve old public methods
4. translate sidecar events into existing application events

Required behavior:

```javascript
class SpeechService extends EventEmitter {
  async initialize() {}

  async startRecording() {}

  async stopRecording() {}

  getStatus() {}

  async shutdown() {}
}
```

Event mapping:

```text
sidecar "partial"
    ↓
speechService.emit("interim-transcription", text)

sidecar "final"
    ↓
speechService.emit("transcription", text)

sidecar "state"
    ↓
speechService.emit("status", mappedStatus)

sidecar "error"
    ↓
speechService.emit("error", message)

sidecar "recording_stopped"
    ↓
speechService.emit("recording-stopped")
```

---

## 9.2 `speech-recognition.js`

Keep this file extremely small.

Recommended final form:

```javascript
const speechService = require("./src/services/speech.service");
module.exports = speechService;
```

No Moonshine logic belongs here.

---

## 9.3 `main.js`

Minimize changes.

Existing speech integration should remain based on:

```javascript
speechService.on("transcription", ...)
speechService.on("interim-transcription", ...)
speechService.on("status", ...)
speechService.on("error", ...)
```

Existing IPC handlers should continue to call:

```javascript
speechService.startRecording()
speechService.stopRecording()
speechService.getStatus()
```

Do not put native STT/model management directly in `main.js`.

---

## 9.4 `package.json`

Remove:

```json
"microsoft-cognitiveservices-speech-sdk"
```

if no other subsystem requires it.

Evaluate removal of:

```json
"node-record-lpcm16"
```

after the Rust sidecar fully owns microphone capture.

If the package is unused elsewhere, remove it.

Add build steps for the native sidecar.

Example:

```json
{
  "scripts": {
    "speech:build": "powershell -ExecutionPolicy Bypass -File scripts/build-speech-sidecar.ps1",
    "speech:models": "powershell -ExecutionPolicy Bypass -File scripts/verify-models.ps1",
    "prestart": "npm run speech:build",
    "build:win": "npm run speech:build && electron-builder --win"
  }
}
```

Adapt scripts to the actual project build process.

---

# 10. New Electron Local Speech Bridge

Create:

```text
src/services/local-speech-bridge.service.js
```

Responsibilities:

- resolve sidecar executable path
- spawn child process
- monitor process health
- send control messages
- parse stdout JSON lines
- forward events
- capture stderr in logs
- restart after unexpected failure
- stop cleanly when Electron quits

Suggested class:

```javascript
class LocalSpeechBridge extends EventEmitter {
  constructor(options = {}) {}

  async startSidecar() {}

  async stopSidecar() {}

  async waitUntilReady() {}

  async startRecording(options = {}) {}

  async stopRecording() {}

  async cancel() {}

  async listDevices() {}

  isReady() {}

  getState() {}
}
```

---

# 11. Sidecar IPC Protocol

Use JSON Lines for control/events.

One JSON object per line.

## Electron → Sidecar

### Initialize

```json
{
  "type": "initialize",
  "config": {
    "language": "en",
    "sample_rate": 16000,
    "speech_start_threshold": 0.55,
    "speech_continue_threshold": 0.40,
    "min_speech_ms": 250,
    "pre_roll_ms": 250,
    "post_roll_ms": 150,
    "endpoint_silence_ms": 900,
    "max_utterance_ms": 30000
  }
}
```

### Start

```json
{
  "type": "start_recording",
  "device_id": null
}
```

### Stop

```json
{
  "type": "stop_recording"
}
```

### Cancel

```json
{
  "type": "cancel"
}
```

### List devices

```json
{
  "type": "list_devices"
}
```

### Shutdown

```json
{
  "type": "shutdown"
}
```

---

## Sidecar → Electron

### Ready

```json
{
  "type": "ready"
}
```

### State

```json
{
  "type": "state",
  "state": "listening"
}
```

Valid state strings:

```text
initializing
ready
listening
speech_detected
transcribing
endpoint_detected
finalizing
stopping
error
```

### Partial transcript

```json
{
  "type": "partial",
  "text": "what is the difference between docker"
}
```

### Final transcript

```json
{
  "type": "final",
  "text": "What is the difference between Docker and Kubernetes?"
}
```

### VAD state

```json
{
  "type": "vad",
  "speech": true,
  "probability": 0.91
}
```

### Audio level

```json
{
  "type": "audio_level",
  "value": 0.35
}
```

### Error

```json
{
  "type": "error",
  "code": "MICROPHONE_DISCONNECTED",
  "message": "The selected microphone is no longer available.",
  "recoverable": true
}
```

---

# 12. Audio Processing Pipeline

The Rust sidecar must implement:

```text
CPAL Input Stream
      ↓
Fast sample conversion
      ↓
Non-blocking audio queue / ring buffer
      ↓
Worker thread
      ↓
Convert channels to mono
      ↓
Normalize float audio
      ↓
Stateful resampler
      ↓
16 kHz mono f32
      ↓
Pre-roll circular buffer
      ↓
Silero VAD
      ↓
Moonshine stream
```

---

# 13. Critical Real-Time Rule

The CPAL callback is a real-time path.

It must do minimal work.

Correct:

```text
CPAL callback
   ↓
copy/push samples
   ↓
return
```

Incorrect:

```text
CPAL callback
   ↓
Silero
   ↓
Moonshine
   ↓
JSON serialization
   ↓
disk logging
   ↓
return
```

AI inference must run in worker threads/processes.

---

# 14. Ring Buffer

Create:

```text
native/speech-sidecar/src/audio/ring_buffer.rs
```

Requirements:

- bounded
- non-blocking or extremely short lock duration
- enough capacity for temporary scheduling delays
- metrics for overflow
- never grow without bound

Recommended rolling capacity:

```text
5 to 10 seconds of canonical audio
```

At 16 kHz mono float32:

```text
16000 samples/sec × 4 bytes = ~64 KB/sec
10 seconds ≈ ~640 KB
```

This is very small.

---

# 15. Pre-Roll

VAD may recognize speech only after a few frames.

Maintain:

```text
pre_roll_ms = 250
```

When speech begins, prepend the previous 250 ms to the utterance.

This prevents clipping initial phonemes/words.

Example:

```text
rolling buffer:
... silence ... "What is Kubernetes"
                    ↑
              VAD fires here
```

The captured segment must still include the start of `"What"`.

---

# 16. Silero VAD

Create:

```text
native/speech-sidecar/src/vad/silero.rs
native/speech-sidecar/src/vad/state.rs
native/speech-sidecar/src/vad/endpoint_detector.rs
```

Silero returns speech probability.

Use hysteresis.

State behavior:

```text
SILENCE
  │
  │ probability >= 0.55
  ▼
SPEECH
  │
  │ probability temporarily falls
  ▼
POSSIBLE_END
  │
  ├── probability recovers
  │        └── SPEECH
  │
  └── silence >= endpoint_silence_ms
           ↓
     END_OF_UTTERANCE
```

Do not finalize on one low-probability VAD frame.

---

# 17. Endpoint Detection

Initial default:

```text
endpoint_silence_ms = 900
```

Make it configurable.

Test:

```text
700 ms
800 ms
900 ms
1000 ms
1200 ms
```

Technical questions often contain pauses.

Example:

```text
"Explain Kubernetes..."

pause

"...and how its control plane works."
```

This must remain one utterance when the pause is below the configured endpoint.

---

# 18. Moonshine Streaming Integration

Create:

```text
native/speech-sidecar/src/stt/engine.rs
native/speech-sidecar/src/stt/moonshine/engine.rs
native/speech-sidecar/src/stt/moonshine/session.rs
native/speech-sidecar/src/stt/moonshine/process.rs
```

Preferred model:

```text
Moonshine Streaming Small
English
```

V1 should use CPU execution on Windows unless a tested native backend offers stable acceleration.

Do not embed Python into Electron.

Use the Moonshine native API/runtime or an isolated model process bundled with the Rust sidecar.

---

# 19. Moonshine Session Lifecycle

Application launch:

```text
Electron starts
    ↓
Speech bridge starts Rust sidecar
    ↓
Rust initializes Silero
    ↓
Rust initializes Moonshine
    ↓
warm-up inference
    ↓
sidecar emits READY
```

Do not wait for first spoken audio to load models.

---

# 20. Partial Transcript Semantics

Moonshine partial results must replace the current partial transcript.

Example sequence:

```text
"what is"
"what is docker"
"what is docker and"
"what is docker and kubernetes"
```

UI must display only the newest current hypothesis.

Do not concatenate all hypotheses.

Maintain:

```text
stable_text
partial_text
```

Finalization:

```text
partial_text = ""
stable_text = finalized utterance
```

---

# 21. Speech Pipeline State Machine

Implement:

```text
IDLE
 │
 ▼
INITIALIZING
 │
 ▼
READY
 │
 ▼
LISTENING
 │
 │ VAD speech
 ▼
SPEECH_DETECTED
 │
 ▼
STREAMING
 │
 │ Moonshine partials
 ├─────────────→ emit PARTIAL
 │
 │ silence >= endpoint
 ▼
ENDPOINT_DETECTED
 │
 ▼
FINALIZING
 │
 ▼
emit FINAL
 │
 ▼
LISTENING
```

Manual stop:

```text
ANY ACTIVE STATE
       ↓
STOPPING
       ↓
flush/finalize current speech if meaningful
       ↓
READY
```

Cancel:

```text
ANY ACTIVE STATE
       ↓
CANCELLED
       ↓
discard current utterance
       ↓
READY
```

---

# 22. Azure OpenAI Integration

The final raw transcript is sent to the existing LLM layer.

The local speech service does **not** call Azure OpenAI itself unless the host architecture is later deliberately refactored.

Preferred flow:

```text
speech.service.js
   ↓ emits "transcription"
main.js
   ↓
existing session manager
   ↓
existing LLM processing method
   ↓
Azure OpenAI provider
```

This preserves separation between:

```text
Speech Recognition
```

and:

```text
Question Answering
```

---

# 23. Azure Prompt Requirement

Add or adapt the system prompt so it understands ASR errors.

Suggested system instruction:

```text
You receive questions transcribed by a local speech-to-text system.

The transcript may contain phonetic spelling mistakes, missing punctuation,
incorrect capitalization, or small recognition errors in technical terminology.

Infer the intended technical question from context without changing its meaning.
Then answer that intended question directly and accurately.

Do not invent missing requirements.
If an ambiguity would materially change the answer, state the ambiguity briefly.

For technical questions, prefer a concise explanation first, then an example or
important details when useful.
```

Example input:

```text
"what is cubernetis deployment and state full set"
```

Expected interpretation:

```text
"What is the difference between a Kubernetes Deployment and StatefulSet?"
```

---

# 24. Do Not Add a Separate Spelling-Correction Model in V1

Do not create:

```text
Moonshine
  ↓
Parakeet
  ↓
Spell checker
  ↓
Azure
```

Use:

```text
Moonshine
  ↓
Azure OpenAI
```

Minor errors are acceptable when semantic intent survives.

Important distinctions to benchmark include:

```text
REST vs Rust
C vs C++
Java vs JavaScript
SQL vs NoSQL
RAG vs DAG
TCP vs UDP
HTTP vs HTTPS
CNN vs RNN
```

Azure cannot reliably recover information that the STT completely destroys.

Therefore local STT still needs good semantic/technical-word accuracy.

---

# 25. Configuration

Create configuration file or environment-based defaults.

Example:

```json
{
  "speech": {
    "language": "en",
    "sampleRate": 16000,
    "vad": {
      "startThreshold": 0.55,
      "continueThreshold": 0.40,
      "minSpeechMs": 250,
      "preRollMs": 250,
      "postRollMs": 150,
      "endpointSilenceMs": 900,
      "maxUtteranceMs": 30000
    },
    "moonshine": {
      "model": "streaming-small",
      "device": "cpu"
    }
  }
}
```

---

# 26. Environment Variables

Azure Speech variables should no longer be required by the STT path.

Remove documentation requiring Azure Speech configuration.

Azure OpenAI variables remain.

Example names:

```text
AZURE_OPENAI_ENDPOINT
AZURE_OPENAI_API_KEY
AZURE_OPENAI_DEPLOYMENT
AZURE_OPENAI_API_VERSION
```

Use the names already used by the host application's LLM service where possible.

Do not expose the API key to renderer JavaScript.

---

# 27. Packaging

The Windows package must include:

```text
speech-sidecar.exe
Silero ONNX model
Moonshine Streaming Small model/runtime files
required native runtime DLLs
```

Electron Builder must copy these outside ASAR when necessary.

Recommended package layout:

```text
resources/
└── speech/
    ├── speech-sidecar.exe
    ├── silero/
    │   └── silero_vad.onnx
    └── moonshine/
        └── streaming-small/
            └── ...
```

Resolve paths using:

```javascript
process.resourcesPath
```

in packaged builds.

Development should resolve:

```text
native/speech-sidecar/target/release/speech-sidecar.exe
```

or equivalent.

---

# 28. Electron Builder Changes

Update `package.json` build configuration.

Add sidecar/model resources through `extraResources` or `extraFiles`.

Example concept:

```json
{
  "build": {
    "extraResources": [
      {
        "from": "native/speech-sidecar/target/release/speech-sidecar.exe",
        "to": "speech/speech-sidecar.exe"
      },
      {
        "from": "native/speech-sidecar/models",
        "to": "speech/models"
      }
    ]
  }
}
```

Exact paths must match the generated project.

Avoid bundling native executables inside ASAR.

---

# 29. Graceful Error Handling

## Microphone missing

Emit:

```json
{
  "type": "error",
  "code": "MICROPHONE_UNAVAILABLE",
  "recoverable": true
}
```

UI must allow user to select another microphone.

---

## Microphone disconnect

Behavior:

```text
stop capture
preserve finalized transcripts
discard corrupted in-progress buffer
emit error
refresh device list
allow restart
```

---

## Moonshine crash

Behavior:

```text
detect child/model failure
emit STT_UNAVAILABLE
restart model runtime if possible
do not crash Electron
```

Use capped retry behavior.

Example:

```text
retry 1 after 500 ms
retry 2 after 1 sec
retry 3 after 2 sec
then require manual restart
```

---

## Azure failure

Because Azure is downstream:

```text
Final transcript must remain visible and stored.
```

Azure failure must not delete speech text.

Allow retrying answer generation.

---

## Internet unavailable

Expected:

```text
Local STT continues working.
Azure answer generation becomes unavailable.
```

This distinction should be clear in status reporting.

---

# 30. Logging

Add structured logs.

Do not log secret values.

Useful fields:

```text
session_id
device_name
input_sample_rate
input_channels
resampled_sample_rate
vad_start_latency_ms
utterance_duration_ms
endpoint_delay_ms
first_partial_latency_ms
partial_count
final_transcript_length
sidecar_restart_count
buffer_overflow_count
```

Do not log raw microphone audio by default.

Debug WAV capture must be opt-in.

---

# 31. Performance Targets

Initial engineering targets:

| Metric | V1 target |
|---|---:|
| Local speech model ready | preferably < 5 s after process initialization |
| VAD speech-start detection | < 200 ms desirable |
| First meaningful partial | < 1 s after meaningful speech desirable |
| Endpoint detection | ~900–1100 ms after last spoken word |
| UI responsiveness | no visible blocking |
| Audio callback drops | 0 under normal usage |
| Sidecar memory | preferably well under several GB |
| Total application | comfortable on 16 GB RAM |
| Offline STT | fully functional |
| Azure unavailable | transcript remains available |

These are targets, not hard guarantees across every CPU.

---

# 32. Windows 16 GB RAM Target

The architecture must be tested on:

```text
Windows 10/11 x64
16 GB RAM
CPU-only execution
SSD
```

Do not require a discrete NVIDIA GPU.

Optional GPU support can be a later optimization.

V1 acceptance requires reasonable operation on CPU.

---

# 33. Technical Vocabulary Test Set

Create at least 100 spoken questions containing terms such as:

```text
Kubernetes
Docker
PyTorch
TensorFlow
LangChain
LangGraph
Azure OpenAI
RAG
Redis
PostgreSQL
MySQL
MongoDB
GraphQL
REST
Rust
C
C++
C#
.NET
Spring Boot
FastAPI
React
Angular
Node.js
TypeScript
Terraform
Jenkins
Kafka
Spark
CNN
RNN
LSTM
Transformer
BERT
RoBERTa
MCP
CI/CD
AWS
Azure
```

Include different English accents and normal room noise.

---

# 34. Benchmark Metrics

For each question record:

```text
question_id
reference_text
raw_stt_text
first_partial_latency_ms
endpoint_latency_ms
utterance_duration_ms
technical_term_accuracy
word_error_rate
cpu_peak_percent
ram_peak_mb
azure_understood_intent = true/false
answer_started_latency_ms
```

Primary success metric:

> Does the final transcript preserve enough meaning and technical terminology for Azure OpenAI to correctly infer the question?

---

# 35. Test Cases

## Test 1 — simple speech

Speak:

```text
"What is object-oriented programming?"
```

Expected:

- VAD starts
- partial appears
- final appears after silence
- exactly one final transcript event
- LLM receives final transcript once

---

## Test 2 — technical word

Speak:

```text
"Explain Kubernetes architecture."
```

Expected:

- concept preserved
- minor capitalization differences allowed

---

## Test 3 — natural pause

Speak:

```text
"Explain Kubernetes..."
```

pause 600 ms, then:

```text
"...and how the control plane works."
```

Expected:

```text
one utterance
```

---

## Test 4 — endpoint

Speak a complete question and remain silent > 1 second.

Expected:

```text
one final event
```

---

## Test 5 — background noise

Keyboard typing without speech.

Expected:

```text
no false final transcription
```

---

## Test 6 — model unavailable

Rename/delete model in test environment.

Expected:

```text
clean MODEL_LOAD error
Electron remains running
```

---

## Test 7 — microphone disconnect

Disconnect USB microphone while listening.

Expected:

```text
recoverable error
no crash
```

---

## Test 8 — Azure unavailable

Disable network after final transcript.

Expected:

```text
transcript remains visible
answer error shown separately
```

---

# 36. Implementation Phases

## Phase 0 — Baseline

Before modifications:

```text
npm install
npm start
```

Verify current host builds.

Record existing speech-service API behavior.

Create a branch:

```text
feature/local-moonshine-stt
```

---

## Phase 1 — Sidecar skeleton

Create:

```text
native/speech-sidecar/
```

Implement:

- Rust CLI executable
- JSON-lines protocol
- `ready`
- `state`
- `error`
- graceful shutdown

No audio yet.

Electron should successfully start/stop sidecar.

---

## Phase 2 — CPAL microphone capture

Implement:

```text
audio/recorder.rs
audio/converter.rs
audio/device_watcher.rs
audio/level_meter.rs
```

Acceptance:

- microphone opens
- level events work
- stop never hangs
- device change is handled

---

## Phase 3 — Normalization and resampling

Implement:

```text
audio/normalizer.rs
audio/resampler.rs
audio/ring_buffer.rs
```

Add optional debug WAV output.

Acceptance:

- output always 16 kHz mono
- correct pitch/speed
- no chunk discontinuities
- no unbounded queue growth

---

## Phase 4 — Silero VAD

Implement:

```text
vad/silero.rs
vad/state.rs
vad/endpoint_detector.rs
```

Acceptance:

- speech start detected
- short pauses tolerated
- endpoint generated
- pre-roll prevents word clipping

---

## Phase 5 — Moonshine streaming

Implement:

```text
stt/engine.rs
stt/moonshine/*
```

Acceptance:

- model preloads
- partial results appear while speaking
- final result emitted at endpoint

---

## Phase 6 — Replace Azure Speech service

Rewrite:

```text
src/services/speech.service.js
```

Add:

```text
src/services/local-speech-bridge.service.js
```

Preserve legacy public events/API.

Acceptance:

`main.js` continues working without Azure Speech SDK.

---

## Phase 7 — Azure OpenAI prompt adaptation

Update LLM system instruction to tolerate ASR errors.

Do not make a separate correction request.

Acceptance:

technical questions with small phonetic spelling errors still produce the intended answer.

---

## Phase 8 — Packaging

Bundle:

```text
speech-sidecar.exe
Silero model
Moonshine model/runtime
```

Acceptance:

installed/portable Windows build works on a clean Windows machine.

---

## Phase 9 — Hardening

Implement:

- bounded queues
- process watchdog
- model timeout
- microphone recovery
- sidecar restart
- cancellation
- clean application shutdown
- telemetry/log metrics without secrets

---

# 37. Definition of Done

The integration is complete when all of the following are true:

- [ ] Azure Speech SDK is no longer used for microphone transcription.
- [ ] The application launches a local speech sidecar.
- [ ] The sidecar captures Windows microphone audio.
- [ ] Audio is converted to 16 kHz mono float32.
- [ ] Silero VAD runs locally.
- [ ] Moonshine Streaming Small runs locally.
- [ ] Partial transcripts are delivered while the person speaks.
- [ ] Sustained silence finalizes the utterance.
- [ ] Final transcript is delivered through the existing `transcription` event.
- [ ] Existing `main.js` LLM flow receives the transcript.
- [ ] Azure OpenAI understands obvious minor ASR errors and generates the answer.
- [ ] Transcription still functions without internet.
- [ ] Azure failures do not erase final transcripts.
- [ ] Microphone disconnect does not crash Electron.
- [ ] Moonshine failure does not crash Electron.
- [ ] CPU-only Windows operation is supported.
- [ ] 16 GB RAM machine is sufficient in testing.
- [ ] Native models/sidecar are bundled into Windows builds.


---

# 38. Coding-Agent Instructions

When an autonomous coding platform receives this specification, it must follow these priorities:

## Priority 1

Do not rewrite unrelated host functionality.

Only modify what is required to replace the speech pipeline.

---

## Priority 2

Preserve the existing JavaScript speech-service contract.

The rest of the application should continue consuming:

```text
interim-transcription
transcription
status
error
recording-stopped
```

---

## Priority 3

Do not block the real-time microphone callback.

Inference must be isolated from audio capture.

---

## Priority 4

Implement and test each layer separately:

```text
sidecar IPC
→ microphone
→ audio conversion
→ resampling
→ VAD
→ Moonshine
→ Electron bridge
→ Azure LLM
→ packaging
```

Do not integrate every subsystem in one giant change.

---

## Priority 5

Use VoiceTypr selectively.

Reference/adapt only the architecture needed from:

```text
src-tauri/src/audio/converter.rs
src-tauri/src/audio/device_watcher.rs
src-tauri/src/audio/level_meter.rs
src-tauri/src/audio/normalizer.rs
src-tauri/src/audio/recorder.rs
src-tauri/src/audio/recorder_watchdog.rs
src-tauri/src/audio/resampler.rs
src-tauri/src/audio/speech_evidence.rs

src-tauri/src/transcription/error.rs
src-tauri/src/transcription/executor.rs
src-tauri/src/transcription/request.rs

src-tauri/src/state_machine.rs
src-tauri/Cargo.toml
```

Do not import entire VoiceTypr modules blindly.

---

## Priority 6

Replace simple silence detection with Silero.

Do not use VoiceTypr `silence_detector.rs` as the final endpoint detector.

---

## Priority 7

Do not add Parakeet or Whisper to V1.

The pipeline is:

```text
Silero VAD
   +
Moonshine Streaming Small
   +
Azure OpenAI
```

---

# 39. Files Summary

## Modify in host application

```text
src/services/speech.service.js
package.json
main.js                         minimal changes only if needed
speech-recognition.js           preferably unchanged
LLM prompt/config               update for ASR-error tolerance
```

## Add to host application

```text
src/services/local-speech-bridge.service.js

native/speech-sidecar/Cargo.toml
native/speech-sidecar/src/main.rs
native/speech-sidecar/src/protocol.rs
native/speech-sidecar/src/error.rs

native/speech-sidecar/src/audio/mod.rs
native/speech-sidecar/src/audio/recorder.rs
native/speech-sidecar/src/audio/converter.rs
native/speech-sidecar/src/audio/normalizer.rs
native/speech-sidecar/src/audio/resampler.rs
native/speech-sidecar/src/audio/device_watcher.rs
native/speech-sidecar/src/audio/recorder_watchdog.rs
native/speech-sidecar/src/audio/level_meter.rs
native/speech-sidecar/src/audio/ring_buffer.rs

native/speech-sidecar/src/vad/mod.rs
native/speech-sidecar/src/vad/silero.rs
native/speech-sidecar/src/vad/state.rs
native/speech-sidecar/src/vad/endpoint_detector.rs

native/speech-sidecar/src/stt/mod.rs
native/speech-sidecar/src/stt/engine.rs
native/speech-sidecar/src/stt/moonshine/mod.rs
native/speech-sidecar/src/stt/moonshine/engine.rs
native/speech-sidecar/src/stt/moonshine/session.rs
native/speech-sidecar/src/stt/moonshine/process.rs

native/speech-sidecar/src/pipeline/mod.rs
native/speech-sidecar/src/pipeline/coordinator.rs
native/speech-sidecar/src/pipeline/state.rs
native/speech-sidecar/src/pipeline/events.rs

scripts/build-speech-sidecar.ps1
scripts/download-silero.ps1
scripts/download-moonshine.ps1
scripts/verify-models.ps1
```

## Reference from VoiceTypr

```text
src-tauri/src/audio/converter.rs
src-tauri/src/audio/device_watcher.rs
src-tauri/src/audio/level_meter.rs
src-tauri/src/audio/normalizer.rs
src-tauri/src/audio/recorder.rs
src-tauri/src/audio/recorder_watchdog.rs
src-tauri/src/audio/resampler.rs
src-tauri/src/audio/silence_detector.rs       reference placement only
src-tauri/src/audio/speech_evidence.rs

src-tauri/src/transcription/error.rs
src-tauri/src/transcription/executor.rs
src-tauri/src/transcription/request.rs

src-tauri/src/state_machine.rs
src-tauri/Cargo.toml
```

---

# 40. Source Repositories

Host integration target:

```text
https://github.com/varun-singhh/Vysper
```

Audio architecture reference:

```text
https://github.com/moinulmoin/voicetypr
```

Silero VAD:

```text
https://github.com/snakers4/silero-vad
```

Moonshine:

```text
https://github.com/moonshine-ai/moonshine
```

---

# 41. Final Architecture

```text
                    ┌───────────────────────────┐
                    │       Electron UI         │
                    │                           │
                    │ Live transcript           │
                    │ Azure answer              │
                    └────────────▲──────────────┘
                                 │
                           Existing IPC
                                 │
                         ┌───────┴─────────┐
                         │     main.js     │
                         └───────▲─────────┘
                                 │
                       existing speech events
                                 │
                  ┌──────────────┴───────────────┐
                  │     speech.service.js        │
                  │     LocalSpeechBridge        │
                  └──────────────┬───────────────┘
                                 │
                              stdio IPC
                                 │
                  ┌──────────────▼───────────────┐
                  │       Rust Speech Sidecar    │
                  │                              │
                  │ CPAL Recorder                │
                  │      ↓                       │
                  │ Converter / Normalizer       │
                  │      ↓                       │
                  │ Stateful Resampler           │
                  │      ↓                       │
                  │ 16 kHz Mono f32              │
                  │      ↓                       │
                  │ Ring Buffer / Pre-Roll       │
                  │      ↓                       │
                  │ Silero VAD                   │
                  │      ↓                       │
                  │ Moonshine Streaming Small    │
                  └──────────────┬───────────────┘
                                 │
                         partial / final text
                                 │
                  ┌──────────────▼───────────────┐
                  │     speech.service.js        │
                  └──────────────┬───────────────┘
                                 │
                           final transcript
                                 │
                         ┌───────▼─────────┐
                         │    main.js      │
                         │ session manager │
                         └───────┬─────────┘
                                 │
                                 ▼
                         ┌───────────────┐
                         │ Azure OpenAI  │
                         │               │
                         │ infer intent  │
                         │ + answer      │
                         └───────┬───────┘
                                 │
                                 ▼
                           Visible UI answer
```

---

# 42. Final Implementation Principle

The first goal is **not perfect transcription**.

The first goal is:

```text
spoken technical question
        ↓
local STT preserves semantic intent
        ↓
Azure understands intended question
        ↓
useful answer
```

Minor errors such as:

```text
kubernetes → kubernetis
pytorch → py torch
langchain → lang chain
```

are acceptable if Azure can safely infer the intended term.

Errors that destroy semantic distinctions are not acceptable and must be caught during benchmarking.

Keep V1 simple:

```text
VoiceTypr-inspired native audio pipeline
+
Silero VAD
+
Moonshine Streaming Small
+
existing Electron speech event contract
+
Azure OpenAI
```

Do not add additional models until benchmark results demonstrate a need.
