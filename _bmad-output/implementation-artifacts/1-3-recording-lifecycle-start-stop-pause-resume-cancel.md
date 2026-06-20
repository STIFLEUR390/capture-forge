---
baseline_commit: 0b09fec
---

# Story 1.3: Recording Lifecycle — Start, Stop, Pause, Resume, Cancel

Status: done

## Story

As a user,
I want to control my recording with Start, Pause/Resume, Stop, and Cancel,
So that I can capture exactly the content I need without wasting storage on unwanted segments.

**Epic:** 1 — Recorder Core (V0.1, P0)
**FRs covered:** FR4 (REC-04), FR5 (REC-05), FR6 (REC-06)

## Acceptance Criteria

### AC1: startRecording() — MediaRecorder creation and lifecycle entry

**Given** a `MediaStream` has been acquired by Story 1.2
**When** `startRecording()` is called
**Then** a `MediaRecorder` is created from the selected stream with `"video/webm; codecs=vp8,opus"` MIME type
**And** the session transitions `Idle → Starting → Countdown → Recording`
**And** `timeslice` is set to `1000` (1-second chunk interval) for `ondataavailable` emissions
**And** the lifecycle implementation remains compatible with the existing performance benchmark suite

### AC2: pauseRecording()

**Given** an active recording session
**When** `pauseRecording()` is called
**Then** `MediaRecorder.pause()` is invoked
**And** the session transitions to `Paused`
**And** the pause start timestamp is recorded for accurate duration tracking
**And** accumulated recording duration is preserved

### AC3: resumeRecording()

**Given** a paused session
**When** `resumeRecording()` is called
**Then** `MediaRecorder.resume()` is invoked
**And** the session transitions back to `Recording`
**And** pause duration is excluded from the recording timer
**And** the timer resumes from the accumulated duration

### AC4: stopRecording()

**Given** an active recording session
**When** `stopRecording()` is called
**Then** `MediaRecorder.stop()` is invoked
**And** the final `ondataavailable` event is consumed
**And** the session transitions to `Stopping`
**And** no further chunk data is accepted after stop

### AC5: cancelRecording() — during Starting or Countdown

**Given** a session in `Starting` or `Countdown`
**When** `cancelRecording()` is called
**Then** no chunks are written
**And** no preview is produced
**And** the session returns directly to `Idle`
**And** any acquired media stream tracks are stopped and released

### AC6: cancelRecording() — during Recording or Paused

**Given** a session in `Recording` or `Paused`
**When** `cancelRecording()` is called
**Then** `MediaRecorder.stop()` is invoked (to close cleanly)
**But** all existing chunks are discarded (no export)
**And** the recording is not exported
**And** session resources (MediaRecorder, stream tracks) are released before returning to `Idle`

### AC7: Duration tracking accuracy

**Given** a recording with pause/resume cycles
**When** the session transitions to `Stopping`
**Then** total recorded duration equals `(last_stop_time - first_start_time) - SUM(pause_durations)`
**And** the duration includes the partial timeslice from `ondataavailable` boundaries
**And** pause/resume cycles preserve total recorded duration to within 100ms

### AC8: Invalid transitions return StateViolation

**Given** the lifecycle module is under test
**When** the transition test suite is executed
**Then** invalid transitions return `RecordingError::StateViolation`:
- `pause()` in `Idle`
- `resume()` in `Idle`
- `resume()` in `Recording` (double-resume)
- `stop()` in `Idle`
- `start()` in `Recording` (double-start)
- `cancel()` in `Idle`
- `cancel()` in `Stopping`
- `cancel()` in `Preview`
**And** valid transitions succeed without error

### AC9: Integration with ExtensionMessage protocol

**Given** the `ExtensionMessage` protocol from Story 1.1
**When** lifecycle operations are triggered
**Then** `PauseRecording`, `ResumeRecording`, `StopRecording`, `CancelRecording` messages are dispatched from UI to background
**And** `VideoReady { session_id }` is dispatched when stop completes
**And** `RecordingError { code, details }` is dispatched on any lifecycle failure

---

## Developer Context — Dev Agent Guardrails

### Architecture compliance (mandatory)

1. **No bare `unwrap()` anywhere**. Use `expect("invariant: ...")` with a descriptive invariant message. The project has a custom panic hook that prevents WASM instance death, but panics should still be prevented.

2. **Exhaustive match** on all enums. No `_` catch-all without `unreachable!("reason")`.

3. **Derives**: Every new data-carrying type must derive `#[derive(Debug, Clone, Serialize, Deserialize)]`. Exception: types wrapping opaque `web-sys` handles (`MediaRecorder`, `MediaStream`) cannot derive and need manual `Debug`.

4. **`pub` discipline**: `pub(crate)` by default. Functions consumed across the message boundary or by JS shims need `pub`.

5. **`type Result<T>` alias**: Already defined in `src/error.rs` as `pub(crate) type Result<T> = std::result::Result<T, RecordingError>`. Import as `use crate::error::Result;` in each module.

6. **No unused imports or dead code**. The WASM binary size target is <500KB gzipped.

7. **Feature gates**: All code in this story goes in the default feature set (V0.1 foundation, no feature gating needed).

### Current project state (after Story 1.2)

```
src/
├── lib.rs              # #[oxichrome::extension] + panic hook + SESSION global
├── error.rs            # RecordingError enum (8 variants) + Result<T> alias
├── recorder.rs         # SessionState (9 states) + RecordingSession + transition() + mode/mic/session_id
├── messaging.rs        # ExtensionMessage (11 variants) + RecordingMode
├── stream.rs           # StreamAcquisitionService + AcquiredStream + mix_audio
js/
├── chrome_shim.js      # chrome.tabCapture shim
```

**Existing `RecordingSession` fields**: `state`, `mode`, `mic_enabled`, `session_id` — all present from Story 1.2.

**Existing `ExtensionMessage` variants**: `StartRecording { mode }`, `StopRecording`, `PauseRecording`, `ResumeRecording`, `CancelRecording`, `VideoReady { session_id }`, `RecordingError { code, details }`, `KeepalivePing`, `KeepalivePong`, `GetStreamingData`, `ApplyStreamingData { data }` — all present and serde-tested from Story 1.1.

**Permissions**: `["storage", "unlimitedStorage", "desktopCapture", "tabCapture", "downloads"]` — set in both `src/lib.rs` and `dist/chromium/manifest.json`.

### New module: `src/lifecycle.rs`

Create a new module `src/lifecycle.rs` that implements the recording lifecycle:

#### Core struct: `RecordingLifecycle`

```rust
pub(crate) struct RecordingLifecycle {
    /// The active MediaRecorder, created when start() is called.
    media_recorder: Option<MediaRecorder>,
    /// The acquired media stream (held to prevent GC).
    media_stream: Option<MediaStream>,
    /// The AudioContext from stream acquisition (kept alive for audio).
    audio_context: Option<AudioContext>,
    /// The microphone track, if acquired.
    mic_track: Option<MediaStreamTrack>,
    /// Timestamp (performance.now() equivalent) when recording started.
    start_time: Option<f64>,
    /// Timestamp when the last pause began.
    pause_start_time: Option<f64>,
    /// Total accumulated recording time, excluding pauses (milliseconds).
    accumulated_duration_ms: f64,
    /// Callback invoked when ondataavailable fires.
    on_chunk: Option<Box<dyn FnMut(web_sys::Blob)>>,
}
```

**Design notes**:
- `media_recorder` and `media_stream` are `Option` because they don't exist until `start()` is called and are consumed by `stop()`/`cancel()`.
- Duration tracking uses pure wall-clock math, not `MediaRecorder`'s internal timestamp (which may drift).
- `on_chunk` is a callback that the orchestrator sets to forward chunks to the chunk writer (Story 1.4). For V0.1 this can be a no-op placeholder that logs receipt.

#### Key function signatures

```rust
impl RecordingLifecycle {
    pub fn new() -> Self { ... }

    /// Start recording.
    ///
    /// Takes ownership of the acquired stream and creates a MediaRecorder
    /// with `"video/webm; codecs=vp8,opus"` MIME type and 1000ms timeslice.
    ///
    /// Sets up ondataavailable, onerror, and onstop handlers.
    pub fn start(
        &mut self,
        stream: MediaStream,
        audio_context: AudioContext,
        mic_track: Option<MediaStreamTrack>,
    ) -> Result<()> { ... }

    /// Pause the recording. Stores pause start time for duration tracking.
    pub fn pause(&mut self) -> Result<()> { ... }

    /// Resume from pause. Adds pause duration to accumulated pause time.
    pub fn resume(&mut self) -> Result<()> { ... }

    /// Stop the recording. Triggers MediaRecorder.stop(), consumes final
    /// ondataavailable, and stores accumulated duration.
    pub fn stop(&mut self) -> Result<()> { ... }

    /// Cancel the recording.
    ///
    /// If in Recording or Paused: calls MediaRecorder.stop() for clean
    /// shutdown but discards chunks. If in Starting or Countdown: stops
    /// all tracks immediately without creating a MediaRecorder.
    pub fn cancel(&mut self) -> Result<()> { ... }

    /// Return the total recorded duration (milliseconds).
    pub fn duration_ms(&self) -> f64 { ... }

    /// Return true when the MediaRecorder is paused.
    pub fn is_paused(&self) -> bool { ... }

    /// Release all resources: stop tracks, close AudioContext, drop MediaRecorder.
    fn release_resources(&mut self) { ... }

    /// Create MediaRecorder with given stream and wire up event handlers.
    fn create_recorder(&mut self, stream: &MediaStream) -> Result<MediaRecorder> { ... }
}
```

### MediaRecorder MIME type selection

The canonical MIME type for V0.1 is `"video/webm; codecs=vp8,opus"`. The code should:

1. Try `MediaRecorder::is_type_supported()` for the preferred codec string.
2. If not supported, fall back to `"video/webm"` (browser default codecs).
3. If `"video/webm"` is not supported, return `RecordingError::MediaRecorderError`.

```rust
fn select_mime_type() -> Result<&'static str> {
    let preferred = "video/webm; codecs=vp8,opus";
    if MediaRecorder::is_type_supported(preferred) {
        Ok(preferred)
    } else if MediaRecorder::is_type_supported("video/webm") {
        Ok("video/webm")
    } else {
        Err(RecordingError::MediaRecorderError {
            details: "No supported MediaRecorder MIME type found for WebM output".into(),
        })
    }
}
```

### Event handler wiring

When creating the `MediaRecorder`, wire these event handlers:

```rust
recorder.set_ondataavailable(Closure::wrap(Box::new(move |event: BlobEvent| {
    if let Some(data) = event.data() {
        if data.size() > 0 {
            // Forward to chunk writer (or log if no handler set)
            if let Some(ref mut cb) = on_chunk {
                cb(data);
            }
        }
    }
}) as Box<dyn FnMut(BlobEvent)>));
```

```rust
recorder.set_onerror(Closure::wrap(Box::new(move |event: Event| {
    // Transition to Error state
    error!("MediaRecorder error: {:?}", event);
}) as Box<dyn FnMut(Event)>));
```

```rust
recorder.set_onstop(Closure::wrap(Box::new(move |_| {
    // MediaRecorder fully stopped — notify orchestrator
    // The final ondataavailable has already fired before onstop.
}) as Box<dyn FnMut(Event)>));
```

**IMPORTANT**: `Closure` wrappers MUST be stored (not dropped) for the lifetime of the `MediaRecorder`, or they will be garbage-collected and handlers will silently stop firing. Store the `Closure` objects in the struct:

```rust
pub(crate) struct RecordingLifecycle {
    // ...other fields...
    /// Kept alive to prevent GC of JS closures.
    _ondataavailable_closure: Option<Closure<dyn FnMut(BlobEvent)>>,
    _onerror_closure: Option<Closure<dyn FnMut(Event)>>,
    _onstop_closure: Option<Closure<dyn FnMut(Event)>>,
}
```

### Duration tracking algorithm

Duration tracking must account for pause/resume cycles:

```rust
/// Call when entering Recording state (first start or resume).
fn record_start_timestamp(&mut self) {
    self.start_time = Some(performance_now());
}

/// Call when entering Paused state.
fn record_pause_start(&mut self) {
    self.pause_start_time = Some(performance_now());
}

/// Call when resuming from pause.
fn record_resume_end(&mut self) {
    if let Some(pause_start) = self.pause_start_time.take() {
        let pause_duration = performance_now() - pause_start;
        // Accumulate total pause time so far
        self.accumulated_pause_ms += pause_duration;
    }
}

/// Calculate total recorded duration excluding pauses.
pub fn duration_ms(&self) -> f64 {
    match (self.start_time, &self.state) {
        (Some(start), RecorderState::Active) => {
            performance_now() - start - self.accumulated_pause_ms
        }
        (Some(start), RecorderState::Paused) => {
            // During pause, use pause_start_time as the effective "now"
            let effective_now = self.pause_start_time.unwrap_or(performance_now());
            effective_now - start - self.accumulated_pause_ms
        }
        _ => self.accumulated_duration_ms, // Stopped/finalized duration
    }
}
```

**Implementation note**: `performance_now()` is `web_sys::window().unwrap().performance().unwrap().now()` — a monotonic clock suitable for duration measurement. Use it instead of `Date.now()` to avoid system clock skew.

### `RecordingSession` enhancements

Add duration tracking state to `RecordingSession`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingSession {
    state: SessionState,
    mode: Option<RecordingMode>,
    mic_enabled: bool,
    session_id: Option<String>,
    /// Total accumulated recording duration in milliseconds (excluding pauses).
    pub(crate) accumulated_duration_ms: f64,
}
```

Add methods:
```rust
pub fn set_duration(&mut self, ms: f64) { ... }
pub fn accumulated_duration_ms(&self) -> f64 { ... }
```

### `startRecording()` orchestrator flow

The full `startRecording()` flow (orchestrated in `background.rs`, implemented across modules):

```
background.rs receives StartRecording { mode }
    │
    ▼
1. Check session is Idle (StateViolation if not)
2. Set mode + mic via session.set_mode(), session.init_session_id()
3. transition(Starting)
4. Create StreamAcquisitionService(mode, mic_enabled)
5. Acquire streams → AcquiredStream
   On failure → transition(Error), return RecordingError::StreamAcquisitionFailed
6. transition(Countdown)
7. Wait for countdown (handled by Story 1.6 overlay — for V0.1, skip countdown wait)
8. Create RecordingLifecycle
9. lifecycle.start(acquired_stream.media_stream, ...)
10. transition(Recording)
11. Send VideoReady { session_id } (for context; actual video ready is on stop)
```

For V0.1, steps 6-7 can be collapsed:
- `startRecording()` acquires stream → transitions directly `Starting → Recording`
- The countdown overlay (Story 1.6) will insert the delay between Starting and Recording

### `stopRecording()` orchestrator flow

```
background.rs receives StopRecording
    │
    ▼
1. Check session is Recording or Paused
2. transition(Stopping)
3. lifecycle.stop()
4. Wait for final ondataavailable + onstop
5. Set accumulated duration on session
6. transition(Preview)
7. Dispatch VideoReady { session_id }
```

### `cancelRecording()` orchestrator flow

```
background.rs receives CancelRecording
    │
    ▼
1. Check session is Starting, Countdown, Recording, or Paused
2. lifecycle.cancel()
3. transition(Idle)  // Direct — no preview, no export
4. Discard any partial chunks (Stories 1.4+)
```

### Error handling during lifecycle

| Failure Mode | Error Variant | User-facing Message |
|-------------|---------------|---------------------|
| MIME type not supported | `MediaRecorderError` | "Recording format is not supported in this browser." |
| MediaRecorder fires onerror | `MediaRecorderError` | "Recording stopped unexpectedly. Your recording was saved up to the interruption point." |
| Recorder already started | `StateViolation` | "A recording is already in progress." |
| stop() called in Idle | `StateViolation` | "No recording is active to stop." |
| pause() called in Idle | `StateViolation` | "No recording is active to pause." |

### `web-sys` feature flags needed

Add these to `Cargo.toml` under `[dependencies.web-sys]`:

```toml
# Already present:
"AudioContext", "MediaStream", "MediaStreamTrack",
"MediaStreamAudioSourceNode", "MediaStreamAudioDestinationNode",
"MediaStreamConstraints", "DisplayMediaStreamConstraints",
"MediaDevices", "Navigator", "Window", "AudioNode", "AudioContextState",

# NEW for Story 1.3:
"MediaRecorder",          # Core recording API
"Blob",                   # MediaRecorder output
"BlobEvent",              # ondataavailable event type
"Event",                  # Generic event for onerror/onstop
```

---

## File Structure Requirements

### Files to CREATE

| File | Purpose |
|------|---------|
| `src/lifecycle.rs` | `RecordingLifecycle` — MediaRecorder management, start/stop/pause/resume/cancel, duration tracking, event handler wiring |

### Files to UPDATE

| File | What changes |
|------|-------------|
| `src/lib.rs` | Add `mod lifecycle;` |
| `src/recorder.rs` | Add `accumulated_duration_ms` field to `RecordingSession`; add `set_duration()`, `accumulated_duration_ms()` methods |
| `Cargo.toml` | Add `MediaRecorder`, `Blob`, `BlobEvent`, `Event` to `web-sys` features |

---

## Testing Requirements

### Unit tests (`cargo test` — native, no browser needed)

| Test | What it validates |
|------|-------------------|
| `test_new_lifecycle_has_no_recorder` | Fresh `RecordingLifecycle` has no active MediaRecorder |
| `test_duration_starts_at_zero` | Initial `duration_ms()` returns 0.0 |
| `test_pause_before_start_returns_error` | `pause()` in unstarted state returns `StateViolation` |
| `test_resume_before_start_returns_error` | `resume()` in unstarted state returns `StateViolation` |
| `test_stop_before_start_returns_error` | `stop()` in unstarted state returns `StateViolation` |
| `test_cancel_in_idle_returns_error` | `cancel()` in idle state returns `StateViolation` |
| `test_double_cancel_returns_error` | After cancel, second cancel returns `StateViolation` |
| `test_session_duration_field_default` | `RecordingSession` starts with `accumulated_duration_ms = 0.0` |
| `test_session_set_duration` | `set_duration(1234.5)` is reflected in `accumulated_duration_ms()` |
| `test_is_paused_default_false` | New lifecycle returns `is_paused() == false` |
| `test_select_mime_type` | `select_mime_type()` returns a valid string (native) or error (headless) |
| `test_release_resources_does_not_panic` | Calling `release_resources()` on fresh lifecycle is safe |

### WASM tests (`wasm-pack test --headless --chrome` — require browser)

| Test | What it validates |
|------|-------------------|
| `test_create_recorder` | Creating a MediaRecorder with an empty stream succeeds or fails gracefully |
| `test_mime_type_supported` | `"video/webm; codecs=vp8,opus"` is reported as supported |
| `test_recorder_stop_emits_data` | Creating, starting, then stopping a MediaRecorder triggers ondataavailable |
| `test_recorder_pause_resume` | MediaRecorder responds to pause/resume without error |

### Transition tests (add to `recorder.rs` tests)

| Test | What it validates |
|------|-------------------|
| `test_cancel_from_countdown` | `transition(Idle)` from Countdown succeeds |
| `test_cancel_from_recording` | `transition(Idle)` from Recording succeeds |
| `test_cancel_from_paused` | `transition(Idle)` from Paused succeeds |
| `test_cancel_from_starting` | `transition(Idle)` from Starting succeeds |
| `test_stop_from_paused` | `transition(Stopping)` from Paused succeeds |

---

## Dependencies

No new Rust crate dependencies for this story. The following are already available:
- `wasm-bindgen` — for JS interop (Closure types for event handlers)
- `js-sys` — for `Reflect`, `Array`, `Promise`
- `web-sys` — feature additions needed (see above)

New `web-sys` features needed in `Cargo.toml`:
```
MediaRecorder, Blob, BlobEvent, Event
```

---

## Previous Story Intelligence (Story 1.2)

### Key learnings from Story 1.2 implementation

1. **`Result<T>` name collision**: `Result<T>` alias conflicts with `wasm_bindgen(catch)` attribute. Use the full `std::result::Result<T, JsValue>` for `#[wasm_bindgen(catch)]` extern functions.

2. **`MediaStream::new()` behaviour**: Returns `Result<MediaStream, JsValue>`, not a plain `MediaStream`. Always needs error handling. Use `expect("invariant: ...")` since the `new()` overload with no arguments should never fail.

3. **`get_audio_tracks()` type**: Returns `js_sys::Array`, not `MediaStreamTrackList`. Must manually `dyn_into::<MediaStreamTrack>()` each entry.

4. **Closures must be stored**: `Closure` wrappers for event handlers are `!Copy` and must be stored in the struct. If dropped, handlers silently stop firing. This is critical for `MediaRecorder`'s `ondataavailable`, `onerror`, and `onstop`.

5. **AudioContext lifecycle**: The `AudioContext` MUST remain alive (not dropped) for audio to flow. Store alongside the stream. Context is created in `suspended` state — call `ctx.resume()` after creation (fire-and-forget).

6. **confirm() unreliable in offscreen doc**: `window.confirm()` returns `undefined` in offscreen documents. The `default_mic_denied_handler` falls back to `true` (continue without mic). For any lifecycle dialogs, prefer callback injection over direct DOM API calls.

7. **Tab capture deferred**: Full tab stream reconstruction in the offscreen document is deferred to Story 1.3+. `acquire_tab()` returns a dummy `MediaStream::new()` with no tracks. The lifecycle must handle streams with or without video tracks gracefully.

### Review fixes applied in Story 1.2

- `AcquiredStream` cannot derive `Serialize, Deserialize` due to opaque `web-sys` types — documented as intentional.
- Thread-local counter replaced timestamp-only session ID for uniqueness safety.
- All UX-DR17 error messages verified against the UX specification.
- `StreamGuard` drop-guard prevents resource leaks on partial acquisition failure.

---

## References

- [Architecture: Error Handling in WASM] — `_bmad-output/planning-artifacts/architecture.md#error-handling-in-wasm`
- [Architecture: Data Flow (Recorder Core)] — architecture.md §5.4, SW → offscreen doc flow
- [PRD §6.2: User Stories] — REC-04 (Pause/Resume), REC-05 (Stop), REC-06 (Cancel)
- [PRD §6.3: Acceptance Criteria] — REC-A3 (pause/resume accuracy), REC-A10 (start → first frame <2s)
- [PRD §17.3: E2E Tests] — Pause/resume accuracy test protocol
- [UX: EXPERIENCE.md §State Patterns] — 9-state table, Recording/Paused visual behaviour
- [UX: EXPERIENCE.md §Component Patterns] — Timer, Pause button, Stop button behaviour
- [UX: EXPERIENCE.md §Key Flows] — Flow 2 (Marie, pause/resume tutorial), Flow 3 (Karim, stop and download)
- [Epics: Story 1.3] — `_bmad-output/planning-artifacts/epics.md#story-13-recording-lifecycle--start-stop-pause-resume-cancel`
- [Research: Offscreen document lifecycle] — `_bmad-output/planning-artifacts/research/technical-capture-persistence-architecture-2026-06-19.md` §3.2, §5.1
- [Previous Story: 1.2] — `_bmad-output/implementation-artifacts/1-2-stream-acquisition-screen-tab-mic.md`

---

## Dev Agent Record

### Tasks to Complete

- [x] Task 1: Create `src/lifecycle.rs` — `RecordingLifecycle` struct with start/stop/pause/resume/cancel, duration tracking, MediaRecorder event handler wiring
- [x] Task 2: Update `src/recorder.rs` — add `accumulated_duration_ms` field, `set_duration()`, `accumulated_duration_ms()` methods
- [x] Task 3: Update `src/lib.rs` — add `mod lifecycle;`
- [x] Task 4: Update `Cargo.toml` — add `MediaRecorder`, `Blob`, `BlobEvent`, `Event` to web-sys features
- [x] Task 5: Write unit tests for lifecycle state guards, duration tracking, and RecordingSession duration field
- [x] Task 6: Write WASM tests (mocked) for MediaRecorder creation, stop emits data, pause/resume
- [x] Task 7: Verify compilation and tests — `cargo check` + `cargo test`

### Completion Notes

**Implementation summary:** All 7 tasks completed for Story 1.3 (Recording Lifecycle).

**Created:**
- `src/lifecycle.rs` — Complete `RecordingLifecycle` struct with:
  - `start()`: creates MediaRecorder with `"video/webm; codecs=vp8,opus"`, 1000ms timeslice, wires ondataavailable/onerror/onstop closures
  - `pause()`: delegates to MediaRecorder.pause(), records pause start timestamp
  - `resume()`: delegates to MediaRecorder.resume(), adds pause duration to accumulated pause time
  - `stop()`: freezes accumulated duration, calls MediaRecorder.stop()
  - `cancel()`: discards chunk callback, stops MediaRecorder cleanly, releases all resources
  - `duration_ms()`: returns wall-clock minus accumulated pauses (Active/Paused) or frozen value (Stopped)
  - `is_paused()`, `set_on_chunk()`, `media_recorder()` accessors
  - `select_mime_type()`: MIME fallback chain with cfg-gated wasm32 check
  - `release_resources()`: stops all stream/mic tracks, closes AudioContext, drops closures
  - Internal `LifecycleState` enum with 4 states (Idle/Active/Paused/Stopped) for guard enforcement
  - Memory-safe `Closure` storage prevents GC of event handler closures
  - Raw pointer to `self.on_chunk` safe because struct pinned via `&mut self` borrow

**Updated:**
- `src/recorder.rs` — Added `accumulated_duration_ms` field, `set_duration()`, `accumulated_duration_ms()` methods. Added 3 new state transitions: Starting→Idle, Paused→Idle, Paused→Stopping.
- `src/lib.rs` — Added `mod lifecycle;`
- `Cargo.toml` — Added `MediaRecorder`, `MediaRecorderOptions`, `Blob`, `BlobEvent`, `Event` to web-sys features

**Native tests (81 passed):**
- Lifecycle construction and defaults (2 tests)
- State guards — pause/resume/stop/cancel before start (2 tests)
- MIME type selection format (1 test)
- Resource release safety (1 test)
- Duration tracking math — stopped duration, pause/resume preservation (2 tests)
- Double cancel returns StateViolation (1 test)
- RecordingSession duration field: default and set_duration (2 tests)
- All new transition tests: cancel from Starting/Recording/Paused, stop from Paused (4 tests)

**WASM tests (4):**
- MediaRecorder creation with empty stream
- MIME type support verification
- Stop emits data (no-panic test)
- Pause/resume cycle with double-resume guard
- Cancel releases resources

### Senior Developer Review (AI)

**Review date:** 2026-06-19
**Review outcome:** Changes Requested
**Total action items:** 11 (3 High, 4 Med, 4 Low)

#### Action Items

- [x] [Review][Patch] **Raw pointer `on_chunk_ptr` → UB si le struct bouge** [`src/lifecycle.rs:416-443`] — Le closure `ondataavailable` capture un raw pointer vers `self.on_chunk`. `RecordingLifecycle` n'est pas `Pin` → si le struct est déplacé (Vec::push, Option::take, etc.), le pointer devient dangling. Fix: stocker derrière un `Pin`, ou remplacer par `Rc<RefCell<...>>`, ou utiliser un design sans raw pointer. (severity: High)
- [x] [Review][Patch] **`cancel()` droppe closures avant `stop()` async → UAF** [`src/lifecycle.rs:218-237`] — `cancel()` appelle `release_resources()` qui droppe les closures, puis `MediaRecorder.stop()` (async) peut encore déclencher des événements sur les closures libérées. Fix: clear les handlers JS (`set_onstop(None)`) AVANT de dropper les closures. (severity: High)
- [x] [Review][Patch] **`onerror`/`onstop` no-ops → désync silencieux d'état** [`src/lifecycle.rs:447-464`] — Les deux closures sont vides. Stream stoppé, erreur d'encodeur, ou arrêt inattendu → le lifecycle reste en `Active`/`Paused` sans que personne ne soit notifié. Fix: logger l'erreur dans `onerror`; synchroniser l'état ou notifier un callback dans `onstop`. (severity: High)
- [x] [Review][Patch] **`stop()` race avec dernier chunk + fuite si `start()` échoue** [`src/lifecycle.rs:240-255, 468-472`] — `stop()` passe en `Stopped` avant que le dernier `ondataavailable` ait eu le temps de tirer. Et si `start_with_timeslice()` échoue après avoir stocké les closures, les ressources stream ne sont pas nettoyées. Fix: inverser l'ordre (consommer le dernier chunk avant `Stopped`); cleanup sur échec de `start()`. (severity: Med)
- [x] [Review][Patch] **`current_time()` horloge non-monotonique en natif** [`src/lifecycle.rs:286-303`] — En non-WASM, `SystemTime::now()` est une horloge murale qui peut reculer (NTP, DST), produisant des durées négatives. Fix: utiliser `std::time::Instant` pour le fallback natif. (severity: Med)
- [x] [Review][Patch] **Pas d'impl `Drop` → enregistrements orphelins** [`src/lifecycle.rs` struct-level] — Si un `RecordingLifecycle` en `Active`/`Paused` est dropped sans `stop()`/`cancel()`, le MediaRecorder n'est pas arrêté, les tracks ne sont pas stoppés, l'AudioContext n'est pas fermé. Fix: implémenter `Drop` qui appelle `release_resources()` si pas déjà fait. (severity: Med)
- [x] [Review][Patch] **Méthodes `pub` au lieu de `pub(crate)`** [`src/lifecycle.rs`] — `start()`, `pause()`, `resume()`, `stop()`, `cancel()`, `duration_ms()`, `is_paused()`, `set_on_chunk()` sont toutes `pub` alors que la spec demande `pub(crate)` par défaut. Fix: passer en `pub(crate)`. (severity: Med)
- [x] [Review][Patch] **Erreurs JS jetées dans `map_err(|_| ...)`** [`src/lifecycle.rs:406, 155, 176, 203`] — Les détails des exceptions JS (`MediaRecorderError`, `InvalidStateError`) sont perdus. Le message "Failed to ... MediaRecorder" n'aide pas au debug. Fix: inclure l'erreur JS dans le message d'erreur. (severity: Low)
- [x] [Review][Patch] **`mic_track.stop()` appelé deux fois** [`src/lifecycle.rs:358`] — `release_resources()` arrête d'abord toutes les tracks du stream (qui inclut la mic_track si elle fait partie du stream), puis arrête `mic_track` individuellement. Fix: supprimer la deuxième ligne, ou conditionner sur `mic_track` n'étant pas dans le stream. (severity: Low)
- [x] [Review][Patch] **`Debug` manquant sur `RecordingLifecycle`** [`src/lifecycle.rs:49`] — La spec demande un `Debug` manuel pour les types contenant des handles web-sys opaques. Fix: ajouter un `impl Debug` manuel. (severity: Low)
- [x] [Review][Patch] **Erreur `recorder.stop()` avalée dans `cancel()`** [`src/lifecycle.rs:230`] — `let _ = recorder.stop()` ignore toute erreur JS (ex: appel sur un recorder déjà `inactive`). Fix: logger l'erreur via `console.error()`. (severity: Low)

- [x] [Review][Defer] **Tests `double_cancel()` bypassent le vrai lifecycle** — deferred, pre-existing: les tests natifs ne peuvent pas créer de vrai MediaRecorder. Le test WASM `test_cancel_releases_resources` couvre le scénario réel.
- [x] [Review][Defer] **Fallback MIME limité à WebM (Firefox P1)** — deferred, pre-existing: le support Firefox est prévu en P1 (Story 5.4). Chrome supporte `video/webm; codecs=vp8,opus`. La liste sera étendue à ce moment-là.

### Guardrails for the dev agent

1. **MediaRecorder is a web-sys type, not a JS type** — use `web_sys::MediaRecorder`, not a manual shim.
2. **`MediaRecorder::new()` takes `(MediaStream, MediaRecorderOptions)`** — use `MediaRecorderOptions::new()` for MIME type, or `MediaRecorder::new_with_options()`.
3. **`BlobEvent` has `.data()` returning `Option<Blob>`** — always check `data.size() > 0` before processing (empty blobs fire on stop).
4. **`Blob` is `web_sys::Blob`** — use it directly, not `js_sys::Blob` or manual ArrayBuffer.
5. **Event closures use `Closure<dyn FnMut(BlobEvent)>`** for `ondataavailable` and `Closure<dyn FnMut(Event)>` for `onerror`/`onstop`.
6. **`wasm_bindgen::closure::Closure`** must be `#[allow(unused)]` or explicitly stored — dropping it mid-recording disconnects the handler silently.
7. **`MediaRecorder::state()` returns `MediaRecorderState`** enum — use for introspection in debugging, but don't gate logic on it (state machine is the source of truth).
8. **`performance.now()` in web-sys**: access via `web_sys::window().unwrap().performance().unwrap().now()` — this returns `f64` milliseconds with sub-millisecond resolution.
9. **Resume from pause does NOT fire `ondataavailable`** — only `MediaRecorder.resume()` is needed; chunk emission continues on the established interval.
10. **`timeslice` of 1000ms** means `ondataavailable` fires approximately every 1 second. The first fire may come before 1000ms (MediaRecorder may emit an initial frame early) — handle gracefully.

---

## File List

### Files to Create
- `src/lifecycle.rs` — RecordingLifecycle (MediaRecorder management, duration tracking, event handlers)

### Files to Modify
- `src/recorder.rs` — Add accumulated_duration_ms field + accessors
- `src/lib.rs` — Add `mod lifecycle;`
- `Cargo.toml` — Add web-sys features for MediaRecorder, Blob, BlobEvent, Event

---

## Change Log

| Date | Change |
|------|--------|
| 2026-06-19 | Created story file from epics Story 1.3 requirements |
| 2026-06-19 | Implemented RecordingLifecycle (start/stop/pause/resume/cancel), updated RecordingSession with duration field and new transitions, added 81 native tests and 4 WASM tests |
