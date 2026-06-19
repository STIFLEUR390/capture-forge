---
baseline_commit: 4f424600bb246fb81a2d1a4b5d8122af89686e46
---

# Story 1.2: Stream Acquisition — Screen, Tab & Mic

Status: ready-for-dev

## Story

As a user,
I want to record my full screen or a specific browser tab with optional microphone audio,
So that I can capture the right source without relying on external tools.

**Epic:** 1 — Recorder Core (V0.1, P0)
**FRs covered:** FR1 (REC-01), FR2 (REC-02), FR3 (REC-03)

## Acceptance Criteria

### AC1: Full Screen mode stream acquisition

**Given** a user clicks Start with "Full Screen" mode selected
**When** the system requests a display stream via `getDisplayMedia()`
**Then** the browser display picker is shown
**And** the selected display is returned as a `MediaStream`
**And** if the picker is cancelled, `RecordingError::StreamAcquisitionFailed` is returned and the session returns to `Idle`

### AC2: Tab mode stream acquisition

**Given** a user clicks Start with "Tab" mode selected
**When** the system acquires the selected tab stream through the Chrome tab capture flow
**Then** the selected tab stream is returned as a `MediaStream`
**And** if access is denied or acquisition fails, `RecordingError::StreamAcquisitionFailed` is returned with a user-facing message

### AC3: Microphone capture — mic ON

**Given** the mic toggle is ON in the popup
**When** recording starts
**Then** microphone input is requested
**And** an `AudioContext` mixer combines source audio and microphone audio when both are available
**And** the recorder receives a single mixed audio track

### AC4: Microphone capture — mic OFF

**Given** the mic toggle is OFF
**When** recording starts
**Then** no microphone track is added to the recording stream
**And** available source audio is preserved if present

### AC5: Microphone permission denied

**Given** the mic toggle is ON
**When** microphone permission is denied
**Then** the system shows a confirmation dialog explaining that microphone audio is unavailable
**And** the user can choose "Continue without mic" or "Cancel"
**And** if the user continues, recording starts without a microphone track
**And** if the user cancels, the session returns to `Idle`

### AC6: Stream acquisition test coverage

**Given** stream acquisition dependencies are tested with mocked browser APIs
**When** recorder acquisition tests are executed
**Then** success, cancellation, denied permission, and missing-audio cases are covered and validated

### AC7: ExtensionMessage support for stream acquisition

**Given** the `ExtensionMessage` protocol from Story 1.1
**When** stream acquisition is triggered
**Then** `StartRecording { mode: RecordingMode }` is dispatched from UI to background
**And** `RecordingError { code: "stream_acquisition_failed", details: "…" }` is returned on failure
**And** `VideoReady { session_id: String }` is dispatched on acquisition success (to be consumed by Story 1.3 lifecycle)

---

## Developer Context — Dev Agent Guardrails

### Architecture compliance (mandatory)

1. **No bare `unwrap()` anywhere**. Use `expect("invariant: ...")` with a descriptive invariant message. The project has a custom panic hook that prevents WASM instance death, but panics should still be prevented.

2. **Exhaustive match** on all enums. No `_` catch-all without `unreachable!("reason")`. Story 1.2 will need new `ExtensionMessage` variants only if the protocol doesn't cover stream acquisition — verify that `StartRecording { mode }` is sufficient before adding new variants.

3. **Derives**: Every new data-carrying type must derive `#[derive(Debug, Clone, Serialize, Deserialize)]`.

4. **`pub` discipline**: `pub(crate)` by default. Functions consumed across the message boundary or by JS shims need `pub`. See detailed rules below under `StreamAcquisitionService`.

5. **`type Result<T>` alias**: Already defined in `src/error.rs` as `pub(crate) type Result<T> = std::result::Result<T, RecordingError>`. Import as `use crate::error::Result;` in each module.

6. **Feature gates**: All code in this story goes in the default feature set (V0.1 foundation, no feature gating needed).

7. **No unused imports or dead code**. The WASM binary size target is <500KB gzipped. Each unused dependency or dead function is binary bloat.

### Current project state (after Story 1.1)

```
src/
├── lib.rs              # #[oxichrome::extension] + panic hook + SESSION global
├── error.rs            # RecordingError enum (8 variants) + Result<T> alias
├── recorder.rs         # SessionState (9 states) + RecordingSession + transition()
├── messaging.rs        # ExtensionMessage (11 variants) + RecordingMode enum
```

**`RecordingMode` currently has**: `FullScreen`, `Tab` (already defined in messaging.rs — no change needed).

**`ExtensionMessage` has**: 11 variants including `StartRecording { mode: RecordingMode }`, `RecordingError { code, details }`, `VideoReady { session_id }` — all serializable.

**Current `lib.rs` permissions**: `["storage"]`. You will need to add `"desktopCapture"` and `"tabCapture"` and `"unlimitedStorage"` to the `#[oxichrome::extension(...)]` attribute. Also update `dist/chromium/manifest.json` with matching permissions.

### New module: `src/stream.rs`

Create a new module `src/stream.rs` that handles all stream acquisition logic:

#### Core struct: `StreamAcquisitionService`

```rust
pub(crate) struct StreamAcquisitionService {
    mode: RecordingMode,
    mic_enabled: bool,
}
```

This struct:
- Is created with `RecordingMode` and mic enablement flag
- Provides `acquire() -> Result<AcquiredStream>` and other lifecycle methods
- Does NOT hold a reference to the browser API objects directly — those are returned as opaque handles

#### Result type: `AcquiredStream`

```rust
pub(crate) struct AcquiredStream {
    pub media_stream: MediaStream,     // The combined MediaStream (video + mixed audio)
    pub audio_context: AudioContext,   // Kept alive as long as the stream is needed
    pub mic_track: Option<MediaStreamTrack>, // The mic track, if acquired
}
```

#### Key function signatures

```rust
/// Acquire a display stream via `getDisplayMedia()`.
/// Called when `RecordingMode::FullScreen` is active.
async fn acquire_display() -> Result<MediaStream>

/// Acquire a tab stream via the JS shim for `chrome.tabCapture`.
/// Called when `RecordingMode::Tab` is active.
async fn acquire_tab() -> Result<MediaStream>

/// Request microphone access via `getUserMedia({ audio: true })`.
/// Returns `None` if permission is denied but user chooses to continue without mic.
async fn acquire_microphone() -> Result<Option<MediaStreamTrack>>

/// Combine video source and mic audio into a single stream using AudioContext.
/// Creates a `MediaStreamAudioSourceNode` from the video source's audio track
/// and a `MediaStreamAudioSourceNode` from the mic track, connects both to a
/// `MediaStreamAudioDestinationNode`, and returns the destination stream.
fn mix_audio(
    video_source: &MediaStream,
    mic_track: Option<MediaStreamTrack>,
    ctx: &AudioContext,
) -> Result<MediaStream>
```

**Important**: `getDisplayMedia()` is available directly via `web-sys` as `wasm_bindgen::JsCast` on the `window` object. `tabCapture` is NOT in `web-sys` — it requires a JS shim.

### JS shim: `js/chrome_shim.js`

The JS shim for `tabCapture` already exists in architecture plans as `js/chrome_shim.js` (~20 lines). For this story you need:

```js
// js/chrome_shim.js
// Shim for Chrome APIs not yet exposed via web-sys.
// Used by the Rust stream acquisition module via wasm-bindgen.

// Tab capture — returns a Promise<MediaStream>
export function tabCaptureCapture(callback) {
    chrome.tabCapture.capture(
        { audio: true, video: true },
        (stream) => {
            if (chrome.runtime.lastError) {
                callback({ error: chrome.runtime.lastError.message });
            } else {
                callback({ streamId: stream.id });
            }
        }
    );
}
```

The Rust side imports this via `#[wasm_bindgen(module = "/js/chrome_shim.js")]`.

**IMPORTANT**: Chrome MV3 offscreen documents cannot access `chrome.tabCapture` directly. The tab capture MUST be called from the service worker (background.js) and the resulting stream ID sent to the offscreen document. This means the stream acquisition design must account for a two-phase approach:

- **Phase 1 (Background SW)**: Request the stream via `getDisplayMedia` / `tabCapture`
- **Phase 2 (Offscreen doc)**: Receive the stream and set up MediaRecorder

For V0.1 simplicity, `getDisplayMedia` (Full Screen mode) can be called from the offscreen document since it doesn't need extension API permissions beyond `desktopCapture`. For Tab mode, the flow is:

1. `ExtensionMessage::StartRecording { mode: Tab }` → background receives
2. Background calls `chrome.tabCapture.capture()` via JS shim
3. Background gets `streamId: string` back
4. Background creates offscreen document, passes `streamId` via URL param or message
5. Offscreen doc uses `navigator.mediaDevices.getUserMedia({ video: { mandatory: { chromeMediaSource: "tab", chromeMediaSourceId: streamId } } })` to reconstruct the stream

**Document this design decision in the story — the stream acquisition is split across SW and offscreen doc.**

### Mic permission denied confirmation dialog

For AC5, when microphone permission is denied, the system must show a confirmation dialog:

1. `getUserMedia({ audio: true })` fails with `NotAllowedError` or `NotFoundError`
2. Show a dialog: "Microphone is unavailable. Continue without mic?" with [Continue without mic] and [Cancel]
3. In V0.1, this dialog can be a simple `window.confirm()` or a structured UI component in the popup/offscreen doc
4. If user clicks "Continue without mic" → proceed with `mic_enabled = false`, no mic track
5. If user clicks "Cancel" → return `RecordingError::StreamAcquisitionFailed { details: "Microphone access denied" }` and the session transitions back to `Idle`

**Note**: The actual UI rendering of the permission dialog will be in Story 3.1 (Popup UI) / Story 3.2 (Permission Request Handling). For Story 1.2, implement the core logic that the UI will call, with the dialog being a simple `confirm()` shim for now. Include a `MicDeniedHandler` callback type that can be injected, so the popup UI story can replace the `confirm()` with a proper styled dialog.

### `RecordingSession` enhancements

The `RecordingSession` struct from Story 1.1 needs to carry stream acquisition state:

```rust
#[derive(Debug, Clone)]
pub struct RecordingSession {
    state: SessionState,
    mode: Option<RecordingMode>,
    mic_enabled: bool,
    session_id: Option<String>,
}

impl RecordingSession {
    pub fn new() -> Self {
        Self {
            state: SessionState::Idle,
            mode: None,
            mic_enabled: true,
            session_id: None,
        }
    }

    /// Set recording mode before starting. Must be called while in Idle.
    pub fn set_mode(&mut self, mode: RecordingMode) -> Result<()> { ... }

    /// Set mic preference. Must be called while in Idle.
    pub fn set_mic_enabled(&mut self, enabled: bool) -> Result<()> { ... }

    /// Generate a new session_id and set it.
    pub fn init_session_id(&mut self) { ... }

    pub fn mode(&self) -> Option<&RecordingMode> { ... }
    pub fn session_id(&self) -> Option<&str> { ... }
    pub fn mic_enabled(&self) -> bool { ... }
}
```

**Design decision**: Add an `is_acquiring` method or a stream acquisition in-progress flag. Consider adding a `StreamAcquiring` intermediate sub-state instead of a new `SessionState` variant to avoid complexity. Use:
```rust
pub fn is_acquiring(&self) -> bool {
    matches!(self.state, SessionState::Starting)
}
```

### AudioContext mixer

The audio mixing approach:

```rust
fn mix_audio(
    video_source: &MediaStream,
    mic_track: Option<MediaStreamTrack>,
) -> Result<(MediaStream, AudioContext)> {
    let ctx = AudioContext::new()?;

    // Connect video source audio tracks (if any) to the destination
    if let Some(audio_tracks) = video_source.audio_tracks() {
        let src_node = ctx.create_media_stream_source(&video_source)?;
        let dst_node = ctx.create_media_stream_destination()?;
        src_node.connect_with_audio_node(&dst_node)?;
    }

    // Connect mic track to the destination
    if let Some(mic) = &mic_track {
        let mic_stream = MediaStream::new_with_tracks(&mic)?;
        let mic_src = ctx.create_media_stream_source(&mic_stream)?;
        let dst_node = ctx.create_media_stream_destination()?;
        mic_src.connect_with_audio_node(&dst_node)?;
    }

    let combined = ctx.create_media_stream_destination()?.stream();
    Ok((combined, ctx))
}
```

**Important**: The `AudioContext` MUST be kept alive for the duration of recording — if it's garbage-collected or goes out of scope, audio stops flowing. Store it alongside the stream.

### User-facing error messages (UX-DR17)

| Failure Mode | Error Variant | `details` for User |
|-------------|---------------|---------------------|
| Stream acquisition cancelled by user | `StreamAcquisitionFailed` | "Screen or tab selection was cancelled." |
| Tab capture permission denied | `StreamAcquisitionFailed` | "Could not access tab. Check permissions in chrome://extensions and try again." |
| Microphone permission denied | `StreamAcquisitionFailed` | "Microphone access was denied. You can continue without mic." (shown before dialog) |
| No audio hardware available | `StreamAcquisitionFailed` | "No microphone found. Recording will continue without audio." |
| getDisplayMedia not available | `StreamAcquisitionFailed` | "Screen capture is not supported in this browser." |

### transition() additions needed

Review the state machine in `recorder.rs` — the existing transitions for stream acquisition already work:
- `Idle → Starting` (Start button clicked, acquisition begins)
- `Starting → Error` (acquisition failed)
- `Starting → Countdown` (acquisition succeeded)

These are already valid. No new state transitions are needed for this story.

However, the `Starting` state's **internal behavior** (not the transition surface) needs to be defined:
1. `Starting` entry: call `StreamAcquisitionService::acquire()`
2. On success → `Starting → Countdown` with the acquired stream stored
3. On failure → `Starting → Error` with `RecordingError::StreamAcquisitionFailed`

The actual `MediaRecorder` creation happens in Story 1.3 — this story only handles getting the stream.

### Permissions update

Update `Cargo.toml` — no changes needed (no new Rust deps).

Update `src/lib.rs`:
```rust
#[oxichrome::extension(
    name = "Capture Forge",
    version = "0.1.0",
    permissions = ["storage", "unlimitedStorage", "desktopCapture", "tabCapture", "downloads"]
)]
```

Update `dist/chromium/manifest.json` with the same permissions. The full V0.1 permissions are:
- `storage` — session lock, preferences
- `unlimitedStorage` — large OPFS chunks
- `desktopCapture` — getDisplayMedia (Full Screen mode)
- `tabCapture` — chrome.tabCapture (Tab mode)
- `downloads` — download exported WebM (Story 1.7)

---

## File Structure Requirements

### Files to CREATE

| File | Purpose |
|------|---------|
| `src/stream.rs` | `StreamAcquisitionService`, `AcquiredStream`, audio mixer, all stream acquisition logic |
| `js/chrome_shim.js` | JS shim for `chrome.tabCapture` (not yet exposed in web-sys) |

### Files to UPDATE

| File | What changes |
|------|-------------|
| `src/lib.rs` | Add `mod stream;`, update permissions in `#[oxichrome::extension(...)]` |
| `src/recorder.rs` | Add `mode`, `mic_enabled`, `session_id` fields to `RecordingSession`; add `set_mode()`, `set_mic_enabled()`, `init_session_id()` methods |
| `dist/chromium/manifest.json` | Add permissions: `unlimitedStorage`, `desktopCapture`, `tabCapture`, `downloads` |

---

## Testing Requirements

### Unit tests (`cargo test` — native, no browser needed)

| Test | What it validates |
|------|-------------------|
| `test_set_mode_valid` | Setting mode in Idle succeeds |
| `test_set_mode_invalid_state` | Setting mode in non-Idle returns StateViolation |
| `test_audio_mixer_no_mic` | `mix_audio()` works with video-only stream |
| `test_audio_mixer_with_mic` | `mix_audio()` works with video + mic track (mocked) |
| `test_audio_mixer_no_audio_source` | `mix_audio()` works when video source has no audio |
| `test_session_id_generated` | `init_session_id()` produces a non-empty unique string |
| `test_mic_enabled_default` | New session has `mic_enabled == true` |
| `test_is_acquiring` | `is_acquiring()` returns true in Starting state |

### WASM tests (`wasm-pack test --headless --chrome` — require browser)

| Test | What it validates |
|------|-------------------|
| `test_acquire_display_cancelled` | Simulate getDisplayMedia cancellation → StreamAcquisitionFailed |
| `test_acquire_microphone_denied` | Simulate getUserMedia denial → Continue dialog |
| `test_acquire_microphone_success` | Mic acquisition returns valid track |

**Note**: WASM tests for real `getDisplayMedia` / `tabCapture` require actual browser API support and cannot be fully automated in headless mode. Use JS mocks/stubs for the Chrome-specific APIs.

---

## Dependencies

No new Rust crate dependencies for this story. The following are already available:
- `wasm-bindgen` — for JS interop (shim import via `#[wasm_bindgen(module = "...")]`)

The `web-sys` crate features needed may include:
- `AudioContext`
- `MediaStream`
- `MediaStreamTrack`
- `MediaStreamAudioSourceNode`
- `MediaStreamAudioDestinationNode`
- These should be declared in `Cargo.toml` under `[dependencies.web-sys]` features

If `web-sys` is not yet fully declared in `Cargo.toml` with feature flags for the audio/media types, add them now.

---

## References

- [Architecture: Error Handling in WASM] — `_bmad-output/planning-artifacts/architecture.md#error-handling-in-wasm`
- [Architecture: Rust → JS Strategy] — tabCapture JS shim per §5.2
- [Architecture: Data Flow (Recorder Core)] — §5.4, SW → offscreen doc flow
- [PRD §6.1: Scope] — REC-01 (Full Screen), REC-02 (Tab), REC-03 (Microphone)
- [PRD §6.2: User Stories] — REC-01 through REC-03
- [PRD §6.5: Message Protocol] — ExtensionMessage variants for start/error/video-ready
- [PRD §6.6: Storage Layout] — for session_id format reference
- [UX: Error States (UX-DR17)] — error messages table for stream acquisition
- [UX: EXPERIENCE.md §Component Patterns] — Mode selector, mic toggle, start button behavior
- [Epics: Story 1.2] — `_bmad-output/planning-artifacts/epics.md#story-12-stream-acquisition--screen-tab--mic`
- [Previous Story: 1.1] — `_bmad-output/implementation-artifacts/1-1-error-system-state-machine-foundation.md`

---

## Dev Agent Record

### Agent Model Used

Claude Opus 4.8

### Completion Notes

- [ ] Task 1: Create `src/stream.rs` — StreamAcquisitionService, AcquiredStream, acquire_display(), acquire_tab(), acquire_microphone(), mix_audio()
- [ ] Task 2: Create `js/chrome_shim.js` — tabCapture JS shim + wasm-bindgen import
- [ ] Task 3: Update `src/recorder.rs` — add mode, mic_enabled, session_id fields + setter/getter methods + test for is_acquiring()
- [ ] Task 4: Update `src/lib.rs` — add `mod stream;`, update permissions
- [ ] Task 5: Update `manifest.json` — add permissions
- [ ] Task 6: Add web-sys feature flags to `Cargo.toml` (AudioContext, MediaStream types)
- [ ] Task 7: Write unit tests for audio mixer, mode setting, session ID generation
- [ ] Task 8: Write WASM tests (mocked) for stream acquisition flows
- [ ] Task 9: Verify compilation and tests — `cargo check` + `cargo test`
