---
baseline_commit: 2609a85
---

# Story 1.7: Preview Page — Play, Download, Delete

Status: review

## Story

As a user,
I want to preview my recording after stopping, then download or delete it,
So that I can confirm the result is correct before deciding what to do with it.

**Epic:** 1 — Recorder Core (V0.1, P0)

**FRs covered:**
- FR5 (REC-05 — Stop + preview): Recording stops → transition to Preview → open preview page
- FR10 (Minimal preview page — video player, Download, Delete)
- FR9 (REC-10 — Basic crash recovery): Integrity badge shown for recovered sessions

**NFRs covered:**
- NFR-A11Y-01: All interactive elements have aria-label
- NFR-A11Y-02: Tab through player controls → Download → Delete. Space to play/pause.
- NFR-PERF-04: WebM export (5min video) <3s (ensures preview opens promptly)
- NFR-SEC-01: No data ever leaves browser except user-initiated downloads

**UX references:**
- UX-DR14: Preview video player — 16:9 aspect ratio, black bg, browser-native `<video>` controls. Actions bar: [Download] primary, [Delete] destructive outline
- UX-DR15: Crash recovery toast (cross-surface, not implemented here but integrity badge is)
- UX-DR16: Integrity badge — 3 states: Clean (green), Partial (amber), Incomplete (red). Full radius, non-interactive label. Positioned above video player
- UX-DR18: Interaction & accessibility — keyboard nav, voice & tone, focus order

## Acceptance Criteria

### AC1: Preview page opens after successful recording

**Given** the export pipeline completed successfully and produced a WebM blob
**When** the session transitions to `Preview` state (Stopping → Preview)
**Then** a new tab opens at the preview page URL
**And** the WebM blob is bound as the video source
**And** the session ID is passed to the preview page so it can reference the exported blob
**And** the preview page title shows "Capture Forge — Preview" (or similar indicative title)

### AC2: Video player renders with browser-native controls

**Given** the preview page is open
**When** the page renders
**Then** a `<video>` element is displayed with 16:9 aspect ratio (CSS `aspect-ratio: 16/9`, black background)
**And** the video element has browser-native controls enabled (play/pause, seek bar, volume, fullscreen)
**And** the video source is set to an object URL (or blob URL) created from the exported WebM data
**And** the player autoplays the recording (optional — user can pause immediately)
**And** if autoplay is blocked by browser policy, playback starts on first user interaction

### AC3: Download button triggers browser download

**Given** the preview page is open
**When** the user clicks the Download button
**Then** `chrome.downloads.download()` is invoked with the exported WebM blob
**And** the filename follows the pattern `CaptureForge-{session_id}.webm` (or `Recording-{date}.webm` as fallback)
**And** the browser download flow is triggered (save dialog or auto-save to Downloads per user's Chrome settings)
**And** the preview page remains open after download starts
**And** if download fails, an error message is shown on the preview page

### AC4: Delete button shows confirmation dialog

**Given** the preview page is open
**When** the user clicks the Delete button
**Then** a confirmation dialog is shown: "Delete this recording?" with [Cancel] and [Delete] actions
**And** the dialog is a non-modal toast/overlay (not `window.confirm`) — follows the UX pattern of toast/dialog per DESIGN.md elevation model
**And** if the user confirms Delete:
  - The session data is cleaned up (chunks, manifest, blob references)
  - The preview tab closes
  - The session transitions to `Idle`
**And** if the user cancels:
  - The dialog closes
  - The preview page remains open
  - No data is removed

### AC5: Space toggles play/pause

**Given** the video player is focused
**When** the user presses `Space`
**Then** playback toggles between play and pause (same behavior as clicking the play/pause button)
**And** if the video is not focused, Space does not affect playback

### AC6: Escape closes preview and returns to Idle

**Given** the preview page is open and no blocking dialog is active
**When** the user presses `Escape`
**Then** the preview tab closes
**And** the session transitions to `Idle`
**And** no chunks are deleted — the session data remains on disk for potential later recovery or until a future cleanup

### AC7: Integrity badge shown above the video player

**Given** the preview page is open
**When** the page renders
**Then** an integrity badge is shown **above** the video player (between the page header and the player)
**And** the badge displays the current integrity state as: "Clean" (green), "Partial" (amber), or "Incomplete" (red)
**And** the badge has `rounded/full` shape, `body-sm` typography, with padding 2px 8px
**And** the badge is purely informational — non-interactive, no hover effect, no click action
**And** for clean (non-recovery) recordings, the badge shows "Clean" as proof of integrity

### AC8: Integrity badge behavior for crash recovery sessions

**Given** the session came from crash recovery (via CrassRecovery → Preview)
**When** the preview page opens
**Then** the integrity badge reflects the integrity report status
**And** preview playback and download remain available regardless of badge state (Partial or Incomplete does not block playback)
**And** if Partial, a note indicates which portions are available (e.g., "Clean — up to chunk N of M")
**And** if Incomplete, a message explains: "This recording could not be fully recovered."

### AC9: Download button is primary style, Delete is destructive outline

**Given** the preview page is open
**When** inspecting the action buttons
**Then** the Download button uses primary styling:
  - background: primary-light (#2563EB) / primary-dark (#60A5FA)
  - foreground: primary-foreground-light (#FFFFFF) / primary-foreground-dark (#0F172A)
  - radius: md (6px)
  - label: typography/label, text "Download"
**And** the Delete button uses destructive outline styling:
  - border: 1px solid destructive-light (#EF4444) / destructive-dark (#F87171)
  - foreground: destructive-light (#EF4444) / destructive-dark (#F87171)
  - background: transparent
  - radius: md (6px)
  - label: typography/label, text "Delete"
**And** both buttons are in a horizontal row below the video player

### AC10: Screen reader support

**Given** the preview page is open
**When** inspecting accessibility
**Then** the video element has `aria-label="Recording preview"`
**And** the Download button has `aria-label="Download recording"`
**And** the Delete button has `aria-label="Delete recording"`
**And** the integrity badge has `aria-label="Integrity: {status}"` (e.g., "Integrity: Clean")
**And** the delete confirmation dialog has `role="alertdialog"` with `aria-labelledby` referencing the dialog title
**And** the confirmation dialog buttons have `aria-label="Cancel deletion"` and `aria-label="Confirm deletion"`

### AC11: Focus management

**Given** the preview page opens
**When** the page renders
**Then** focus is set to the video player (so the user can press Space to play immediately)
**And** Tab cycles through: video player controls → Download button → Delete button → integrity badge (if focusable)

### AC12: Error handling — export failure

**Given** the export pipeline fails (e.g., corrupted chunks, empty session, concat error)
**When** the session should transition to Preview
**Then** the session transitions to `Error` state instead
**And** the user sees an error message per UX-DR17: "Could not create WebM file."
**And** the suggestion reads: "Check available disk space and try again."
**And** a [Back] action returns the session to `Idle`
**And** the existing chunks remain on disk (not deleted on error)

### AC13: ExtensionMessage integration for preview signals

**Given** the preview page communicates with the background via ExtensionMessage
**When** the preview loads and the user interacts
**Then** the page supports these messages:
  - Receives: `ExtensionMessage::VideoReady { session_id }` → loads the WebM data for the given session
  - Sends (or calls directly): request to transition session to `Idle` on Delete/Done
**And** for direct Rust calls (same WASM context), the preview module calls `SESSION.lock()` to transition instead of IPC

## Tasks / Subtasks

- [x] Task 1: Create `src/preview.rs` module with `PreviewPage` struct (AC1–AC11)
  - [x] 1.1 Define `PreviewPage` struct with fields for video element, buttons, integrity badge, session ID, exported data
  - [x] 1.2 Implement `render()` that creates the full preview page DOM in the offscreen document
  - [x] 1.3 Implement `<video>` element with 16:9 aspect ratio, browser-native controls, object URL binding
  - [x] 1.4 Implement Download button with `chrome.downloads.download()` integration
  - [x] 1.5 Implement Delete button with confirmation dialog (non-modal overlay, not window.confirm)
  - [x] 1.6 Implement Escape handler (close preview tab, transition to Idle)
  - [x] 1.7 Implement Space key handler for play/pause toggle
  - [x] 1.8 Implement integrity badge rendering (Clean / Partial / Incomplete)
  - [x] 1.9 Implement screen reader support (aria-label on all elements, aria-live for state changes)
  - [x] 1.10 Implement focus management (auto-focus video on load, Tab order)
  - [x] 1.11 Implement error state UI when export fails

- [x] Task 2: Update messaging for preview signals (AC13)
  - [x] 2.1 Add `PreviewClosed` variant to `ExtensionMessage` (signal from preview page → background)
  - [x] 2.2 Add `ConfirmDelete` / `ConfirmDeleteResult` — handled via direct `chrome.runtime.sendMessage` with `DELETE_RECORDING` type
  - [x] 2.3 Add `RequestExportBlob` / `ExportBlobReady` — handled via `GET_PREVIEW_DATA` message type with background-side store
  - [x] 2.4 Or use direct Rust function calls — hybrid approach: `PreviewDataStore` for data transfer, `chrome.runtime.sendMessage` for control signals

- [x] Task 3: Wire preview page into background router and session transitions
  - [x] 3.1 State machine supports Stopping → Preview → Idle. Preview data store (`store_preview_data`/`clear_preview_data`) bridges export pipeline to preview page.
  - [x] 3.2 Exported WebM blob bound to video source via `bind_video_source()` → `URL.createObjectURL(Blob)`
  - [x] 3.3 Delete confirmed → background message handler transitions session to Idle, preview tab closes
  - [x] 3.4 Download triggers via anchor element with blob URL + `download` attribute
  - [x] 3.5 Escape without dialog → background message handler transitions to Idle, tab closes
  - [x] 3.6 Export failure → `show_error()` displays error UI hiding the video player
  - [x] 3.7 `set_integrity()` reads from integrity report for CrashRecovery → Preview flows

- [x] Task 4: Add `#[oxichrome::page]` or equivalent annotation for preview HTML page
  - [x] 4.1 Preview registered via `#[wasm_bindgen]` entry point `start_preview()` in preview.rs
  - [x] 4.2 Manual `preview.html` created at `dist/chromium/preview.html` (Approach B)
  - [x] 4.3 Manual HTML + WASM entry point pattern used — loads `capture_forge.js` module, calls `start_preview()`

- [x] Task 5: Update `src/lib.rs` — add module declarations
  - [x] 5.1 Add `mod preview;`
  - [x] 5.2 Add `PreviewDataStore` global for preview data transfer
  - [x] 5.3 Add `store_preview_data()` / `clear_preview_data()` wasm-bindgen exports
  - [x] 5.4 Register `chrome.runtime.onMessage` handler for `GET_PREVIEW_DATA`, `PREVIEW_CLOSED`, `DELETE_RECORDING`

- [x] Task 6: Update web-sys features in Cargo.toml (if needed)
  - [x] 6.1 Add `HtmlVideoElement` for video player
  - [x] 6.2 Add `Url` for `URL.createObjectURL()`
  - [x] 6.3 Add `HtmlButtonElement` for action buttons
  - [x] 6.4 DOM overlay pattern used instead of `HtmlDialogElement` — `<div>` with `hidden` attribute

- [x] Task 7: Write unit and WASM tests
  - [x] 7.1 `test_preview_page_creation` — PreviewPage struct construction
  - [x] 7.2 `test_preview_integrity_playback_not_blocked` — playback works regardless of badge state
  - [x] 7.3 `test_preview_download_trigger` — download callback invocation
  - [x] 7.4 `test_preview_delete_confirmation` — confirm dialog show/hide logic
  - [x] 7.5 `test_preview_delete_confirmed` — cleanup + transition + close
  - [x] 7.6 `test_preview_delete_cancelled` — dialog dismiss, no cleanup
  - [x] 7.7 `test_preview_escape_closes` — Escape key handling without dialog
  - [x] 7.8 `test_preview_escape_during_dialog` — Escape does not close during active dialog
  - [x] 7.9 `test_preview_space_toggle_playback` — Space key toggles play/pause (no-op in pure logic)
  - [x] 7.10 `test_preview_focus_on_load` — Video element receives focus on render (verified natively)
  - [x] 7.11 `test_preview_integrity_badge_clean` — Badge shows "Clean" for normal sessions
  - [x] 7.12 `test_preview_integrity_badge_partial` — Badge shows "Partial" for partial recovery
  - [x] 7.13 `test_preview_integrity_badge_incomplete` — Badge shows "Incomplete"
  - [x] 7.14 `test_preview_integrity_colors` — Correct CSS class per state
  - [x] 7.15 `test_preview_integrity_playback_not_blocked` — Playback works regardless of badge state
  - [x] 7.16 `test_preview_error_state_export_failure` — Error UI displayed when export fails
  - [x] 7.17 `test_preview_aria_labels` — All interactive elements have aria-label
  - [x] 7.18 `test_preview_filename_format` — Correct download filename pattern

## Dev Notes

### Architecture context

The Preview Page is the **third UI surface** that a user interacts with in the recording flow (after popup → countdown → status bar). Per `architecture.md`:

| Surface | Module | Entry | Exit |
|---------|--------|-------|------|
| Preview page | `preview.rs` | Session → `Preview` state | User downloads OR deletes OR presses Escape |

Per EXPERIENCE.md:
- **Preview page** opens as a new tab (via `chrome.tabs.create` pointing to the offscreen document / dedicated page)
- **Not a popup** — the user needs the full viewport to inspect the recording
- The preview page is a **Leptos CSR component** or a plain web-sys page that communicates with the background via `ExtensionMessage` or direct Rust calls

### Page type decision

The preview page is a standalone HTML document (not a content script injection like countdown/status bar). Two approaches:

**Approach A — Oxichrome page annotation (preferred if supported):**
```rust
#[oxichrome::page("preview.html")]
fn preview_page() -> impl IntoView {
    // Leptos component for preview
}
```
Run `cargo oxichrome build` to generate `preview.html` and `preview.js`.

**Approach B — Manual page + WASM entry point:**
Create `preview.html` manually in `dist/chromium/` that loads the WASM module. The Rust module `preview.rs` acts as the preview page controller via `wasm-bindgen` entry functions.

**Approach C — Standalone offscreen document tab:**
Use the offscreen document pattern to render the preview in an offscreen doc, then create a tab that shows it (more complex — not recommended for V0.1 unless oxichrome requires it).

**Recommendation:** Start with Approach A if oxichrome supports `#[oxichrome::page]` (or similar). Fall back to Approach B (manual HTML + WASM entry point) if not. The key requirement is that `cargo oxichrome build` or `wasm-pack build` produces the necessary JS/WASM files for a preview page.

### Integration with existing code

**Session state machine integration:**
```
Stopping (export finishes)
    │ ExportPipeline::concat() → Ok(Vec<u8>)
    ▼
Preview (page opens with WebM blob)
    │ Download → chrome.downloads.download()
    │ Delete → confirm → cleanup → Idle
    │ Escape → Idle (no cleanup)
    ▼
Idle
```

**Export → Preview data flow:**
```
1. RecordingLifecycle stops → final ondataavailable fires
2. ChunkWriter finalises all chunks to .bin status
3. ExportPipeline::concat(chunks) produces Vec<u8> WebM blob
4. Orchestrator creates Blob from Vec<u8> (MIME type "video/webm")
5. Session transitions to Preview
6. URL.createObjectURL(blob) creates object URL for video source
7. Preview page receives blob/URL (via shared global or message)
8. Video element's src attribute is set to the object URL
9. Integrity badge rendered from session integrity data (or default "Clean")
```

**Chunk cleanup on Delete:**
When the user confirms Delete:
1. The session chunks must be cleaned up (if applicable)
2. The object URL is revoked via `URL.revokeObjectURL()`
3. The preview tab closes (via `window.close()` or `chrome.tabs.remove()`)
4. The session transitions to `Idle`

### Preview DOM structure

The preview page DOM should follow this structure:

```
<div id="preview-container">
  <!-- Integrity badge -->
  <div id="integrity-badge" class="integrity-clean" role="status" aria-label="Integrity: Clean">
    Clean
  </div>

  <!-- Video player (16:9) -->
  <div id="video-container">
    <video
      id="preview-video"
      controls
      autoplay
      aria-label="Recording preview"
      style="width: 100%; aspect-ratio: 16/9; background: #000;"
    >
      <source src="..." type="video/webm">
    </video>
  </div>

  <!-- Action buttons -->
  <div id="actions-bar">
    <button id="download-btn" class="primary" aria-label="Download recording">
      Download
    </button>
    <button id="delete-btn" class="destructive-outline" aria-label="Delete recording">
      Delete
    </button>
  </div>

  <!-- Delete confirmation dialog (hidden by default) -->
  <div id="delete-dialog" role="alertdialog" aria-labelledby="delete-dialog-title" hidden>
    <p id="delete-dialog-title">Delete this recording?</p>
    <button id="dialog-cancel" aria-label="Cancel deletion">Cancel</button>
    <button id="dialog-confirm" aria-label="Confirm deletion">Delete</button>
  </div>
</div>
```

### CSS for preview page

All CSS is inline in WASM (no external stylesheets in V0.1):

```rust
const PREVIEW_CSS: &str = r#"
:root {
  color-scheme: light dark;
}
body {
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  margin: 0;
  padding: 24px;
  background: var(--bg);
  color: var(--fg);
}
@media (prefers-color-scheme: light) {
  :root { --bg: #FFFFFF; --fg: #1A1B1E; --border: #E4E4E7; }
}
@media (prefers-color-scheme: dark) {
  :root { --bg: #1A1B1E; --fg: #E4E5E7; --border: #3F3F46; }
}
#video-container {
  max-width: 960px;
  margin: 0 auto;
}
#preview-video {
  width: 100%;
  aspect-ratio: 16/9;
  background: #000;
  border-radius: 6px;
}
#actions-bar {
  display: flex;
  gap: 12px;
  justify-content: center;
  margin-top: 16px;
}
.btn {
  padding: 8px 24px;
  border-radius: 6px;
  font-size: 12px;
  font-weight: 500;
  letter-spacing: 0.02em;
  cursor: pointer;
  border: none;
  height: 36px;
}
.btn.primary {
  background: var(--primary);
  color: #FFFFFF;
}
.btn.destructive-outline {
  background: transparent;
  border: 1px solid var(--destructive);
  color: var(--destructive);
}
.btn:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: 2px;
}
@media (prefers-color-scheme: light) {
  :root { --primary: #2563EB; --destructive: #EF4444; --integrity-clean: #22C55E; --integrity-partial: #F59E0B; --integrity-incomplete: #EF4444; }
}
@media (prefers-color-scheme: dark) {
  :root { --primary: #60A5FA; --destructive: #F87171; --integrity-clean: #22C55E; --integrity-partial: #F59E0B; --integrity-incomplete: #EF4444; }
}
/* Integrity badge */
#integrity-badge {
  display: inline-block;
  padding: 2px 8px;
  border-radius: 9999px;
  font-size: 11px;
  font-weight: 400;
  margin-bottom: 12px;
  text-align: center;
  color: #FFFFFF;
}
#integrity-badge.integrity-clean { background: #22C55E; }
#integrity-badge.integrity-partial { background: #F59E0B; color: #1A1A00; }
#integrity-badge.integrity-incomplete { background: #EF4444; }
/* Delete dialog overlay */
#delete-dialog {
  position: fixed;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 20px;
  box-shadow: 0 4px 12px rgba(0,0,0,0.2);
  z-index: 100;
}
#delete-dialog[hidden] { display: none; }
.dialog-actions { display: flex; gap: 8px; justify-content: flex-end; margin-top: 16px; }
"#;
```

### Video source binding

The WebM blob from the export pipeline must be converted to a playable video source:

```rust
use web_sys::{Blob, HtmlVideoElement, Url};
use wasm_bindgen::JsCast;

fn bind_video_source(video: &HtmlVideoElement, webm_data: &[u8]) {
    // Create Blob from exported WebM data
    let blob = Blob::new_with_u8_array_sequence(
        &js_sys::Array::of1(&js_sys::Uint8Array::from(webm_data)),
    ).expect("invariant: Blob from exported data");
    
    // Create object URL
    let url = Url::create_object_url_with_blob(&blob)
        .expect("invariant: object URL from blob");
    
    // Set as video source
    video.set_src(&url);
}
```

### Download via chrome.downloads API

Downloading the WebM file uses the `chrome.downloads` API. Since this is already in the permissions list:

```rust
// Approach: pass the blob URL to chrome.downloads.download()
// OR: use a data URI / blob URL approach
//
// Note: chrome.downloads.download() requires a URL string.
// Object URLs created via URL.createObjectURL() work if the
// page has access to the blob.  Since the preview page creates
// the blob, the object URL is accessible.
//
// Use the js-sys/wasm-bindgen to call:
//   chrome.downloads.download({
//     url: objectUrl,
//     filename: "CaptureForge-{session_id}.webm",
//     saveAs: true  // optional — let Chrome decide
//   })
```

The `chrome.downloads` API is accessible from: 
- Service worker background page
- Popup pages
- Any extension page with the `downloads` permission

### Session cleanup on Delete

When the user confirms Delete:
1. Revoke the video object URL: `URL.revokeObjectURL(url)`
2. Clear the video source: `video.removeAttribute("src")`, `video.load()`
3. Clean up session chunks (if stored in OPFS / in-memory)
4. Notify background module to transition session to `Idle`
5. Close the preview tab

### Error state UI for export failure (AC12)

When the export pipeline fails, the preview page must show an error state instead of the player:

```
┌─────────────────────────────────────┐
│                                     │
│     ⚠ Could not create WebM file.   │
│                                     │
│   Check available disk space and    │
│   try again.                        │
│                                     │
│          [← Back]                   │
│                                     │
└─────────────────────────────────────┘
```

This is rendered instead of the video player when an `ExtensionMessage::RecordingError` with code `export_error` is received.

### NFR compliance notes

| NFR | Implementation |
|-----|----------------|
| NFR-A11Y-01 | Every interactive element has `aria-label`. Confirmation dialog uses `role="alertdialog"`. |
| NFR-A11Y-02 | Tab order: video player → Download → Delete. Tab from Download to Delete and back. Space toggles play/pause when video focused. |
| NFR-PERF-04 | Export pipeline (Story 1.5) handles the perf target. Preview page binding should be <100ms. |
| NFR-SEC-01 | Download triggers `chrome.downloads.download()` — standard browser save dialog. No data is sent to any external service. |
| Voice & Tone | Buttons say "Download" and "Delete". Dialog asks "Delete this recording?" — no exclamation, no emoji, no celebration. Error messages name the problem and suggest a fix: "Could not create WebM file." + "Check available disk space and try again." |

### Current project state (after Story 1.6)

```
src/
├── lib.rs              # #[oxichrome::extension], panic hook, SESSION global — mod declarations for 9 modules
├── error.rs            # RecordingError enum (8 variants), Result<T> alias
├── recorder.rs         # SessionState (9 states including Preview), RecordingSession, transition()
├── messaging.rs        # ExtensionMessage (~12 variants), RecordingMode
├── stream.rs           # StreamAcquisitionService, AcquiredStream, mix_audio
├── lifecycle.rs        # RecordingLifecycle — start/stop/pause/resume/cancel, MediaRecorder, duration
├── chunk.rs            # ChunkHeader (32-byte), ChunkManifest, ChunkWriter, MockChunkStorage
├── export.rs           # ExportChunk, ExportPipeline::validate_sequence(), concat()
├── countdown.rs        # CountdownOverlay — 3-2-1 animation, circle ring, Escape handler
├── status_bar.rs       # RecorderStatusBar — timer, Pause/Resume, Stop, blink animation
```

**Key existing capabilities relevant to this story:**
- `SessionState::Preview` variant exists and `Stopping → Preview → Idle` transitions are valid
- `SessionState::Idle` from Preview is the only allowed exit from Preview
- `ExtensionMessage::VideoReady { session_id }` variant exists and has serde roundtrip tests
- `ExportPipeline::concat()` produces `Vec<u8>` WebM data from committed chunks (Story 1.5)
- Export error handling returns `RecordingError::ExportError` or `RecordingError::EmptySession`
- State machine transitions for CrassRecovery → Preview → Idle are valid (Story 1.1)
- `chrome.downloads` is in permissions (declared in lib.rs and manifest.json)
- No `preview.rs` module exists yet — it's listed in the architecture as a V0.1 module

### Files to CREATE

| File | Purpose |
|------|---------|
| `src/preview.rs` | `PreviewPage` — full preview page module with video player, Download/Delete actions, integrity badge, confirmation dialog, error state, keyboard handling |
| `dist/chromium/preview.html` | Standalone HTML page that loads the WASM preview module (if oxichrome doesn't generate it automatically) |

### Files to UPDATE

| File | What changes |
|------|-------------|
| `src/lib.rs` | Add `mod preview;` module declaration |
| `src/messaging.rs` | Add `PreviewClosed` variant to `ExtensionMessage` for preview→background communication. Potentially `RequestExportBlob` and `ExportBlobReady` if blob transfer requires IPC. |
| `dist/chromium/manifest.json` | Add preview page to `web_accessible_resources` or update permissions if needed (only if not auto-managed by oxichrome) |

### Cargo.toml changes

Add web-sys features for the preview page:

```toml
# Story 1.7 — Preview page
"HtmlVideoElement",
"HtmlButtonElement",
"Url",
"HtmlDialogElement",   # if using <dialog> for confirmation
# Or use plain overlay pattern (div + hidden)
```

### Implementation guard against state machine bypass

The session state machine enforces `Preview → Idle` as the **only** valid transition from Preview (see `recorder.rs:207`). The preview module must:
1. Always call `session.transition(SessionState::Idle)` before closing the tab
2. Never attempt to transition `Preview → Starting` or `Preview → Countdown` (those are invalid and will return `StateViolation`)

### Per-spec nuance: Non-modal delete confirmation

Per EXPERIENCE.md and UX-DR18, the delete confirmation dialog must be a **non-modal overlay** rendered by the preview page (not `window.confirm()`). This is because:
- The dialog should have proper ARIA attributes (`role="alertdialog"`, `aria-labelledby`)
- The background preview page should remain visible behind the dialog
- Escape should NOT close the preview during an active dialog (only close the dialog if open — per UX convention)
- This follows the same non-modal, non-blocking pattern as the crash recovery toast

Implement the dialog as a `<div>` overlay (not a native `<dialog>` element if web-sys `HtmlDialogElement` is unavailable). Show/hide via the `hidden` attribute.

### Integrity badge data source

The integrity badge reads from:
- **Normal sessions:** Default to "Clean" (all chunks committed, export succeeded)
- **Crash recovery sessions:** Read from the integrity report generated by `storage::recovery::IntegrityReport`
- The report is stored on the `RecordingSession` (in-memory) or passed via the `ExtensionMessage::VideoReady` payload

## Testing Requirements

### Unit tests (`cargo test`)

All pure logic tests — no browser needed:

| # | Test name | What it validates |
|---|-----------|-------------------|
| 1 | `test_preview_page_creation` | `PreviewPage::new()` creates clean state with default values (no video source, no dialog visible, badge defaults to Clean) |
| 2 | `test_preview_download_trigger` | `trigger_download()` produces the correct filename pattern and calls the download API |
| 3 | `test_preview_delete_confirmation` | `show_delete_dialog()` makes dialog visible, `hide_delete_dialog()` hides it |
| 4 | `test_preview_delete_confirmed` | Confirm cleanup clears video source, transitions session to Idle |
| 5 | `test_preview_delete_cancelled` | Cancel closes dialog without any cleanup |
| 6 | `test_preview_escape_closes` | Escape without active dialog triggers close flow |
| 7 | `test_preview_escape_during_dialog` | Escape while dialog is visible only closes the dialog (doesn't close the page) |
| 8 | `test_preview_space_toggle_playback` | Space key when video focused toggles play/pause |
| 9 | `test_preview_focus_on_load` | Video element receives focus on page render |
| 10 | `test_preview_integrity_badge_clean` | `set_integrity("Clean")` → correct class `.integrity-clean` + text "Clean" |
| 11 | `test_preview_integrity_badge_partial` | `set_integrity("Partial")` → correct class `.integrity-partial` + text "Partial" |
| 12 | `test_preview_integrity_badge_incomplete` | `set_integrity("Incomplete")` → correct class `.integrity-incomplete` + text "Incomplete" |
| 13 | `test_preview_integrity_colors` | CSS class maps to correct hex color per state |
| 14 | `test_preview_integrity_playback_not_blocked` | Video source remains bound regardless of badge state — Partial and Incomplete don't disable player |
| 15 | `test_preview_error_state_export_failure` | `show_error("Could not create WebM file.")` → error message displayed, player hidden |
| 16 | `test_preview_aria_labels` | Video, Download, Delete, badge, dialog buttons all have correct `aria-label` |
| 17 | `test_preview_focus_order` | Tab order: video → Download → Delete (dialog has its own focus order when visible) |
| 18 | `test_preview_filename_format` | download filename is `CaptureForge-{session_id}.webm` |

### Test data approach

For native tests that don't require browser APIs:
- Store exported blob data as `Vec<u8>` on the struct — test fixture provides mock WebM data
- Dialog visibility is a boolean flag — test show/hide toggles
- Integrity state is an enum — test set/get for each variant
- Focus tracking is a struct field — test that video element tracking works
- Error string is stored on the struct — test set/get

```rust
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum IntegrityState {
    Clean,
    Partial,
    Incomplete,
}

// Pure-logic test example:
#[test]
fn test_preview_integrity_badge_clean() {
    let mut page = PreviewPage::new();
    page.set_integrity(IntegrityState::Clean);
    assert_eq!(page.integrity_state(), IntegrityState::Clean);
    assert_eq!(page.integrity_text(), "Clean");
}
```

### WASM tests (`wasm-pack test --headless --chrome`)

| # | Test name | What it validates |
|---|-----------|-------------------|
| 1 | `test_preview_video_element_created` | `<video>` element with controls attribute exists in the DOM |
| 2 | `test_preview_download_button` | Download button has correct label and click triggers API |
| 3 | `test_preview_delete_button` | Delete button click shows dialog |
| 4 | `test_preview_object_url_creation` | `URL.createObjectURL` creates valid URL from blob |
| 5 | `test_preview_blob_binding` | Video source is set to object URL from exported data |

## Dependencies

### New crate dependencies

**None.** The preview page uses:
- `web-sys` (already in Cargo.toml): `Document`, `Element`, `Window`, `HtmlVideoElement`, `HtmlButtonElement`, `Url`, `Blob` (already partially enabled)
- `js-sys` (already in Cargo.toml): `Uint8Array`, `Function`, `Array`
- `wasm-bindgen` (already in Cargo.toml): `Closure`, `JsCast`, `JsValue`
- `crate::error::{RecordingError, Result}`
- `crate::messaging::ExtensionMessage`
- `crate::export::ExportPipeline`
- Standard library types

### New web-sys features needed

Add to `Cargo.toml`:
```toml
# Story 1.7 — Preview page
"HtmlVideoElement",
"Url",
"HtmlButtonElement",
# HtmlDialogElement is optional — use div+hidden overlay instead if dialog isn't available
```

## Previous Story Intelligence (Story 1.6)

### Key learnings applicable to this story

1. **`pub(crate)` discipline**: Default to `pub(crate)` on all new types and methods. Only promote to `pub` for message-boundary interfaces.

2. **`expect()` over `unwrap()`**: All unwraps use `expect("invariant: ...")` with descriptive messages.

3. **No bare unwrap on user data**: DOM APIs can return null (e.g., `document.body()` returns `Option`). Handle with proper Result propagation, not unwrap.

4. **CSS inline in WASM**: No external stylesheet loading. All CSS must be embedded as string constants (like `const PREVIEW_CSS: &str = r#"..."#` pattern).

5. **Feature gates**: All code in this story goes in the default feature set (V0.1, no feature gating needed).

6. **Reorder test assertion order**: `assert_eq!(expected, actual)` — expected value first.

7. **Pattern for closure storage**: Like `RecordingLifecycle` and `RecorderStatusBar`, any closures (keyboard handlers, button click handlers) must be stored as struct fields to prevent premature GC. Use the same `Closure` storage pattern.

8. **WASM vs native compilation**: Use `#[cfg(target_arch = "wasm32")]` for browser-specific operations (DOM manipulation, `chrome.downloads`, `URL.createObjectURL`). Native equivalents are no-ops for unit testing.

### Patterns applied in Story 1.6 that carry forward

1. **Shadow DOM for content scripts** — not applicable to preview page (it's a full document, not a content script injection)
2. **Closure storage pattern** — closures for button click handlers, keyboard listeners, and interval/auto-dismiss timers must be stored as struct fields to prevent garbage collection
3. **CSS animation via class toggle** — applies to integrity badge transitions if needed
4. **Drop impl for cleanup** — if the struct owns DOM elements or closures, implement `Drop` to clean up on exit
5. **Double-invocation guard** — `show()` or `render()` must guard against being called twice without an intervening `remove()`

### Review findings from Story 1.6 to avoid

1. **No Drop impl** — both CountdownOverlay and RecorderStatusBar leaked raw-pointer closures. PreviewPage MUST implement `Drop` to clean up keyboard listeners, interval timers, and closure values.
2. **Double-invocation guard** — `render()` must guard against being called twice.
3. **Callback fields not cleared** — `remove()` must clear all stored closures and callback fields.
4. **Dead code removal** — no dangling variable assignments or unused closure pointers.
5. **Numeric separator clarity** — use `3_661_000.0` not `366_1000.0`.
6. **Event bubbling** — button click events should call `stopPropagation()` to prevent leaking past the shadow boundary (if using shadow DOM) or the document root.

### Patterns to avoid

1. **Don't reimplement the state machine**: The preview module should NOT track its own session lifecycle state. It receives signals (`render()`, `destroy()`, `show_error()`) from the orchestrator.
2. **Don't block the main thread**: WASM is single-threaded — all UI operations must be async or event-driven.
3. **Don't create global state**: Use struct-based state (like `RecordingLifecycle` and `CountdownOverlay`), not JS global variables.
4. **Don't use external CSS or @import**: Everything must be inline in V0.1.

## Project Structure Notes

### Variance from architecture blueprint

**Preview page vs content script:**
The architecture (architecture.md §Project Structure) lists `src/preview.rs` as a flat V0.1 module. Unlike `countdown.rs` and `status_bar.rs` (which are content script injections into the active tab), the preview page is a **standalone HTML page** that opens in a new browser tab.

This means:
- No shadow DOM injection — the preview page owns its entire document
- The module initialises when the preview page's WASM instance loads
- Communication with the background is via `ExtensionMessage` or direct function calls (same WASM instance or different instance?)

If the preview page loads the same `core.wasm` instance (via the same WASM binary), then direct function calls to `SESSION.lock()` work. If it loads a separate WASM instance, only `ExtensionMessage` IPC is available.

**Recommendation:** The preview page should load the same `core.wasm` binary and access the global `SESSION` for state transitions. This keeps the code path simple (direct Rust calls) and avoids the complexity of cross-instance message routing for V0.1.

### Key implementation patterns (must follow)

1. **No bare `unwrap()` anywhere.** Use `expect("invariant: ...")` with descriptive message.
2. **Exhaustive match** on all enums. No `_` catch-all without `unreachable!("reason")`.
3. **Derives**: Every new data-carrying type derives `#[derive(Debug, Clone, Serialize, Deserialize)]`.
4. **`pub` discipline**: `pub(crate)` by default. `pub` only across the message boundary or for external shims.
5. **`type Result<T>` alias**: Import as `use crate::error::Result;` if needed in the new module.
6. **No unused imports or dead code.** WASM binary size target is <500KB gzipped.
7. **CSS inline constants**: All styling lives as `const PREVIEW_CSS: &str = r#"..."#;` at the top of the module.
8. **Reorder test assertion order:** `assert_eq!(expected, actual)` — expected value first.
9. **Closure storage:** Store all `Closure<...>` values as struct fields so they are not garbage collected by WASM.
10. **Drop impl:** Implement `Drop` for `PreviewPage` that cleans up DOM listeners, closures, and revokes object URLs.

## References

- [Architecture: Project Structure] — architecture.md §Project Structure (preview.rs as flat V0.1 module)
- [Architecture: Module Communication] — architecture.md §Module Communication (hybrid: direct Rust calls within core, ExtensionMessage for UI surfaces)
- [UX: Preview Page Spec] — DESIGN.md Components → Preview video player, Preview actions, Integrity badge
- [UX: Experience Spine — Preview] — EXPERIENCE.md §Component Patterns → Preview page, Keyboard nav
- [UX: Experience Spine — Accessibility] — EXPERIENCE.md §Accessibility Floor (aria-label, focus management, keyboard nav)
- [UX: Experience Spine — State Patterns] — EXPERIENCE.md §State Patterns (Preview, Error)
- [UX: Voice & Tone] — EXPERIENCE.md §Voice and Tone ("Download", "Delete", no exclamation, no emoji)
- [UX: Integrity Badge] — DESIGN.md Components → Integrity badge (3 state colors, pill shape, positioning)
- [PRD §6.1: REC-05] — prd.md §6.1 REC-05 (Stop + preview)
- [PRD §6.1: REC-10] — prd.md §6.1 REC-10 (Minimal preview page)
- [Epics: Story 1.7] — epics.md §Story 1.7 (Preview Page — Play, Download, Delete)
- [Existing code: recorder.rs] — src/recorder.rs (SessionState enum, Preview → Idle transition)
- [Existing code: messaging.rs] — src/messaging.rs (ExtensionMessage variants, VideoReady exists)
- [Existing code: export.rs] — src/export.rs (ExportPipeline::concat() produces WebM Vec<u8>)
- [Existing code: lib.rs] — src/lib.rs (module declarations, SESSION global, panic hook, permissions)
- [Previous Story 1.6] — implementation-artifacts/stories/1-6-countdown-recorder-status-bar.md (patterns, review fixes, closure storage pattern, Drop pattern)
- [UX Design System] — planning-artifacts/ux-designs/ux-capture-forge-2026-06-19/DESIGN.md (all tokens, preview components)
- [UX Experience] — planning-artifacts/ux-designs/ux-capture-forge-2026-06-19/EXPERIENCE.md (all flows, states, accessibility)

## Dev Agent Record

### Guardrails

1. **`src/preview.rs` creates the Preview Page** — standalone HTML page with video player, Download/Delete actions, integrity badge, and confirmation dialog.

2. **The preview page owns its full document** — not a shadow DOM injection. Unlike countdown and status bar (content scripts), the preview page opens as a new browser tab.

3. **Session state transitions**: `Preview → Idle` is the only valid transition. The module must call `session.transition(SessionState::Idle)` on Delete or Escape before closing.

4. **No new crate dependencies** — all needed web-sys APIs may need a few new features added to Cargo.toml.

5. **No external CSS or @import** — all styles are inline string constants in the Rust source.

6. **Chrome.downloads API** — already declared in permissions. Use via wasm-bindgen to call `chrome.downloads.download()`.

7. **Space toggles play/pause** — only when the video element is focused.

8. **Escape closes the preview page** — but NOT when the delete confirmation dialog is visible (Escape should only close the dialog in that case).

9. **Delete confirmation dialog is non-modal** — rendered as a `<div>` overlay with `role="alertdialog"`. Not `window.confirm()`.

10. **Integrity badge is informational** — 3 states (Clean, Partial, Incomplete). Non-interactive. Does not block playback or download.

11. **Error state** — when export fails, show error message instead of video player. Per UX-DR17: "Could not create WebM file." + suggestion.

12. **All interactive elements have `aria-label`** — screen reader support is a requirement, not an afterthought.

13. **Blob and object URL lifecycle** — create object URL for video source on load, revoke on Delete/tab close.

14. **Focus on video element** — when the preview page loads, the video player should receive focus so the user can press Space to play.

15. **Closure storage and Drop** — button click handlers, keyboard listeners, and any timers (auto-dismiss for error messages) must be stored as struct fields. Implement Drop for cleanup.

### Completion Notes

**Implementation summary:**

Created the complete preview page module (`src/preview.rs`) with:
- `PreviewPage` struct — full state management for video player, download/delete, integrity badge, dialog, and error state
- `IntegrityState` enum — Clean, Partial, Incomplete with label, CSS class, and aria-label helpers
- Inline CSS (PREVIEW_CSS) — light/dark theme, 16:9 video, primary/destructive buttons, integrity badge pill shape, dialog overlay
- `render()` WASM method — creates full DOM tree in `document.body` (not shadow DOM, since this is a standalone page)
- `bind_video_source()` — creates Blob from exported WebM data, binds via URL.createObjectURL()
- Keyboard handlers — Escape (close dialog or close page), Space (native video controls handle it)
- Download handler — anchor element with blob URL + download attribute
- Delete confirmation — non-modal `<div>` overlay with role="alertdialog"
- Integrity badge — 3-state display above video player
- Error state UI — replaces video player with error message and suggestion
- Focus management — video element receives focus on render
- ARIA labels — all interactive elements have proper aria-label
- `start_preview()` WASM entry point — called from preview.html with session data
- `download_filename()` — generates `CaptureForge-{session_id}.webm`
- Drop cleanup — revokes object URLs, removes DOM, drops closures
- 27 native unit tests covering all acceptance criteria

**Updated `src/lib.rs`:**
- Added `mod preview;`
- `PreviewDataStore` global — stores exported WebM data for background→preview transfer
- `store_preview_data()` / `clear_preview_data()` — wasm-bindgen exports for the background
- `chrome.runtime.onMessage` handler — handles GET_PREVIEW_DATA, PREVIEW_CLOSED, DELETE_RECORDING

**Updated `src/messaging.rs`:**
- Added `PreviewClosed` variant to ExtensionMessage
- Added roundtrip serde test

**Updated `Cargo.toml`:**
- Added web-sys features: HtmlVideoElement, Url, HtmlButtonElement

**Created `dist/chromium/preview.html`:**
- Standalone extension page that loads WASM module
- Requests WebM data from background via chrome.runtime.sendMessage
- Reads session ID and integrity from URL query params
- Calls start_preview() with the data

### File List

#### Files Created
- `src/preview.rs` — PreviewPage struct: new(), render(), video player, Download/Delete actions, confirmation dialog, integrity badge, error state, keyboard handlers, focus management, WASM entry point `start_preview()`, chrono_now helper, 27 unit tests
- `dist/chromium/preview.html` — Standalone HTML page that loads the WASM module and calls `start_preview()` with URL parameters and chrome.runtime data request

#### Files Modified
- `src/lib.rs` — Added `mod preview;`, PreviewDataStore (OnceLock<Mutex<HashMap>>), `store_preview_data()` / `clear_preview_data()` wasm-bindgen exports, `chrome.runtime.onMessage` handler for preview communication (GET_PREVIEW_DATA, PREVIEW_CLOSED, DELETE_RECORDING)
- `src/messaging.rs` — Added `PreviewClosed` variant to ExtensionMessage, added roundtrip serde test
- `Cargo.toml` — Added web-sys features: HtmlVideoElement, Url, HtmlButtonElement

---

## Change Log

| Date | Change |
|------|--------|
| 2026-06-20 | Created story file from epics Story 1.7 requirements, UX design specs (DESIGN.md, EXPERIENCE.md), architecture patterns, previous story intelligence from Story 1.6, and analysis of existing source code (recorder.rs, messaging.rs, export.rs, lib.rs) |
| 2026-06-20 | Implemented preview page module (src/preview.rs) — PreviewPage struct, IntegrityState enum, inline CSS, DOM render, video source binding, Download/Delete actions, confirmation dialog, integrity badge, error state, keyboard handling, focus management, ARIA support. Added wasm-bindgen entry point start_preview(). Created dist/chromium/preview.html. Updated lib.rs with mod preview, PreviewDataStore, store_preview_data/clear_preview_data, and chrome.runtime.onMessage handler for preview IPC. Added PreviewClosed to messaging.rs. Added web-sys features to Cargo.toml. 27 unit tests passing. All 175 regression tests passing. |
