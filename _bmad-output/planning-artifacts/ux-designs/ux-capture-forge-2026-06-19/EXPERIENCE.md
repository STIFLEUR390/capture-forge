---
name: CaptureForge
status: final
sources:
  - {planning_artifacts}/prds/prd-capture-forge-2026-06-19/prd.md
  - docs/ux-designer.md
  - docs/product-brief.md
updated: 2026-06-19
---

# CaptureForge — Experience Spine

## Foundation

**Form-factor:** Chrome browser extension (Manifest V3). Popup (extension icon click), offscreen document (preview page, recorder page), content script (recording overlay toolbar). No responsive web surface — the extension is used within the browser chrome.

**UI system:** No inherited component library. All UI rendered via Leptos + web-sys (Rust/WASM). `DESIGN.md` is the visual identity reference; this spine defines behavior, states, interactions, and accessibility.

**Surfaces (V0.1):**
| Surface | Type | Entry |
|---------|------|-------|
| Popup | `chrome.action` popup | Click extension icon |
| Recording overlay | Content script overlay | Recording starts |
| Countdown overlay | Content script overlay | 3s before recording |
| Preview page | Offscreen document | Recording stops |
| Crash recovery toast | Overlay (any surface) | Service worker restart detects orphan chunks |

**Toolbar overlay** is rendered as a shadow-DOM content script injected into the active tab. It floats above page content and is removed when recording ends.

**Preview page** opens as a new tab (via `chrome.tabs.create` pointing to the offscreen document). Not a popup — the user needs the full viewport to inspect the recording.

**i18n:** English-only V0.1. French locale added V0.2. Both spines and all microcopy strings authored in English; locale files are extracted by the i18n crate (see PRD §10.5).

**First-run experience:** No setup wizard. The first extension click opens the popup in its Idle state. Chrome's native permission dialogs (tabCapture, desktopCapture) fire on first use of each mode. No onboarding overlay, no tutorial, no "welcome" page.

## Information Architecture

The extension has no navigation hierarchy. Each surface is event-triggered and transient:

```
Popup (Idle)
  │ click [Start]
  ▼
Countdown (3-2-1)
  │ auto
  ▼
Recording overlay (active)
  │ click [Pause] → [Resume]
  │ click [Stop]
  ▼
Preview page
  │ [Download] → browser save dialog
  │ [Delete] → confirm → popup returns to Idle
  ▼
Popup (Idle)
```

**Crash recovery** is a cross-cutting concern — triggers on any surface when the service worker detects orphaned chunks. A toast appears on the current surface (popup, preview, even blank).

**No back button.** No sidebar. No tabs. The user completes a recording, previews it, downloads or deletes it, and the extension returns to Idle.

## Voice and Tone

Microcopy principles apply to all surfaces. Brand voice and aesthetic posture live in `DESIGN.md`.

| Do | Don't |
|----|-------|
| "Record Full Screen" / "Record Tab" | "Start capturing your display!" |
| "Preparing…" | "Almost there! 🎬" |
| "Paused" (blinking) | "Recording paused — click resume when ready" |
| "Finalizing…" | "Saving your masterpiece…" |
| "Recording lost. This session could not be recovered." | "Something went wrong 😢" |
| "3, 2, 1" (numbers alone) | Numbers with emoji or labels |
| Duration format: `00:03:42` / `01:02:15` | "3m 42s" (inconsistent) |

The voice is **neutral, precise, and calm**. No exclamation marks. No emoji. No celebration animation. Error messages name the problem and suggest a fix — they don't apologize.

Recording duration always uses `HH:MM:SS` format, zero-padded. Shorter recordings show `MM:SS` until the hour mark.

## Component Patterns

Behavioral. Visual specs live in `DESIGN.md.Components`.

### Mode selector (popup)
- Two mutually exclusive options: `Full Screen` / `Tab`
- Tapping `Tab` triggers `chrome.tabCapture` permission request if not granted
- Tapping `Full Screen` triggers `getDisplayMedia` browser dialog
- Selection persists for the session (until popup closes, then resets to `Full Screen`)

### Mic toggle (popup)
- Binary on/off. Default: on
- Shows mic icon + label ("Microphone")
- Changing state mid-recording is **not supported in V0.1** — set before starting

### Start button (popup)
- Disabled until a mode is selected (always one selected by default)
- Disabled during `Starting` and `Countdown` states
- Click triggers `Starting → Countdown` transition
- Keyboard shortcut `Alt+Shift+G` also fires this action (if popup is open)

### Countdown overlay
- Full-viewport semi-transparent overlay, centered number
- Sequence: 3 → 2 → 1 → fade out → recording starts
- Each number appears for 1s with a scale-up + fade animation
- Circle ring fills clockwise over each 1s interval
- `Escape` during countdown cancels the recording (returns to Idle)
- No sound during countdown in V0.1

### Recording toolbar
- Floating overlay at the top edge of the viewport
- Not draggable in V0.1 (fixed position, top-center)
- Always visible on the recorded tab — even during scroll or navigation
- Contains: Timer (left) | Pause button | Stop button (right)
- **No close button** — recording can only be stopped or paused
- Toolbar is semi-transparent at rest, full opacity on hover (to reduce visual obstruction)

### Timer display
- Shows elapsed recording time
- Format: `MM:SS` → `HH:MM:SS` after 1 hour
- Updates every 250ms
- Red color (`{components.timer-display.foreground}`) during active recording
- Blinks (opacity 0.3 ↔ 1.0, 1s cycle) during pause, with "Paused" text to the right

### Pause button
- Toggle: Pause → Resume
- Icon switches between ⏸ and ▶
- During pause: timer blinks, toolbar opacity drops to 0.6
- Resume returns toolbar to full opacity, timer stops blinking

### Stop button
- Click triggers `Stopping` state
- Icon: ⏹ (square, red)
- No confirmation dialog — stop is immediate
- If accidentally triggered, user cannot cancel (but can avoid saving in preview)

### Preview page
- Standard `<video>` element with browser-native controls (play/pause, seek, volume, fullscreen)
- Below player: action buttons (Download, Delete)
- `Space` toggles play/pause when player is focused
- `Escape` from preview returns to popup (closes preview tab)
- Download triggers `chrome.downloads.download()` with the WebM blob
- Delete shows a confirmation dialog "Delete this recording?" [Cancel] [Delete]
- Maximum preview tab lifetime is unbounded (user closes when done)

### Crash recovery toast
- Non-modal, bottom-center
- Appears on whatever surface is currently open (popup, preview, or idle)
- Text: "A previous recording session was found."
- Actions: [Restore] (primary) | [Dismiss] (text link)
- Restore opens preview page, same as successful stop flow
- Dismiss dismisses the toast and orphans the chunks (opfs-cleanup handles later)
- Auto-dismisses after 8s of no interaction
- Only one toast visible at a time

### Integrity badge
- Appears in the preview page header and session browser (V0.2+)
- Three states: `Clean` (green) — all chunks verified | `Partial` (amber) — some chunks lost, contiguous prefix recovered | `Incomplete` (red) — insufficient data for meaningful recovery
- Non-interactive — visual label only, no click behavior, no tooltip
- The preview page always shows integrity status above the player, even for clean sessions (as proof)
- After crash recovery, the toast outcome includes integrity status: "Session recovered to 92%."

## State Patterns

| State | Surface | Visual | Transition |
|-------|---------|--------|------------|
| `Idle` | Popup | Mode selector, mic toggle, Start button enabled | → Starting (on Start) |
| `Starting` | Popup / overlay | Spinner "Preparing…" | → Countdown (stream acquired) / → Error (acquisition failed) |
| `Countdown` | Content script overlay | 3→2→1 animated numbers + circle ring | → Recording / → Idle (Escape cancels) |
| `Recording` | Toolbar overlay | Timer counting, pause + stop buttons | → Paused / → Stopping |
| `Paused` | Toolbar overlay | Timer blinking, "Paused" label, resume button | → Recording (resume) / → Stopping (stop) |
| `Stopping` | Toolbar overlay | "Finalizing…" spinner, buttons disabled | → Preview (chunks concatenated) / → Error (concat failed) |
| `Preview` | Offscreen tab | Video player + actions | → Idle (close tab or delete) |
| `Error` | Popup / overlay | Error message + suggestion + [Back] button | → Idle |
| `CrashRecovery` | Toast on any surface | Toast with [Restore] | → Preview (on restore) / → Idle (dismiss) |

**Error sub-states:**

| Error | Message | Suggestion |
|-------|---------|------------|
| `Stream acquisition failed` | "Could not access screen or tab." | "Check permissions in chrome://extensions and try again." |
| `MediaRecorder error` | "Recording stopped unexpectedly." | "Your recording was saved up to the interruption point. Try a shorter recording." |
| `Export failed` | "Could not create WebM file." | "Check available disk space and try again." |
| `OPFS write error` | "Could not save recording data." | "Storage may be full. Free up space and try again." |

All errors are ephemeral (shown once, dismissed by user action). No error is logged to a remote server (zero telemetry). Debug logs stay in `chrome.storage.local`.

## Interaction Primitives

**V0.1 is mouse-first.** Keyboard shortcuts exist (Alt+Shift+G/M/X) but are not discoverable from the UI. No keyboard shortcut configuration UI.

| Input | Surface | Action |
|-------|---------|--------|
| Click [Start] | Popup | Begin recording |
| Click [Stop] | Toolbar overlay | End recording |
| Click [Pause] / [Resume] | Toolbar overlay | Toggle pause |
| `Alt+Shift+G` | Global | Start recording (equivalent to popup Start) |
| `Alt+Shift+X` | Global | Cancel recording (during countdown or active) |
| `Alt+Shift+M` | Global | Pause / Resume |
| `Escape` | Countdown | Cancel recording |
| `Escape` | Preview | Close preview tab |
| `Space` | Preview (player focused) | Toggle play/pause |
| Click [Restore] | Crash toast | Open preview |
| Click [Dismiss] | Crash toast | Dismiss, return to Idle |

**Banned in V0.1:** drag-to-reposition toolbar, multi-select in popup, hover-to-reveal controls (touch-incompatible on recorded pages with their own hover states), right-click context menu items.

## Accessibility Floor

Behavioral. Visual contrast is defined in `DESIGN.md` (tokens verified to meet WCAG 2.1 AA).

| Requirement | Implementation |
|-------------|---------------|
| Screen reader announcements | `aria-live="polite"` region on popup announces state changes: "Recording started," "Recording paused," "Recording saved." Timer updates via `aria-atomic`. |
| Keyboard navigation popup | `Tab` through mode selector, mic toggle, Start button. `Enter` or `Space` activates. Focus visible via `ring` token. |
| Keyboard navigation toolbar | No Tab target — toolbar is an overlay without focusable controls inside it. Keyboard shortcuts (Alt+Shift) are the alternative. |
| Keyboard navigation preview | `Tab` through player controls, Download, Delete. `Space` to play/pause. |
| Countdown accessibility | Visual countdown + `aria-live="assertive"` announcement per number: "3", "2", "1", "Recording started." |
| Toast accessibility | `role="alert"`, `aria-live="assertive"`. Focus moves to toast when it appears. `Tab` to Restore/Dismiss. `Escape` dismisses. |
| Color not sole indicator | Recording state is indicated by icon (pause/stop icons), text ("Paused" label), and color (red timer). Red dot is supplemented by timer text. |
| Reduced motion | Countdown circle fill animation respects `prefers-reduced-motion`. Use a simple opacity fade instead of scale animation. Timer blink rate slows to 2s cycle. |
| Focus order | Popup: Mode selector → Mic toggle → Start button. Preview: Player → Download → Delete. Linear, no jumps. |

## Key Flows

### Flow 1 — Alex records a code review (dev, Chrome, weekday morning)

1. Alex opens the PR they want to review on GitHub. Clicks the CaptureForge extension icon in the toolbar.
2. Popup opens with "Full Screen" pre-selected and mic on. Alex taps "Tab" to switch mode, then clicks "Start."
3. Chrome shows the tabCapture permission dialog. Alex approves (once per session).
4. **Countdown:** 3, 2, 1 overlaid on the GitHub PR page. The semi-transparent overlay counts down with a blue circle filling clockwise. Alex sees exactly where the recording will start.
5. **Recording:** Toolbar appears at the top of the viewport. Timer starts at `00:00:00`. Alex scrolls through the diff, comments on a few lines. Microphone captures their commentary.
6. Alex finishes the review, hits `Alt+Shift+X` to stop. Timer stops.
7. **Preview:** A new tab opens with the recording ready to play. Alex presses Space to preview. The video plays back the full review session with audio.
8. Alex clicks "Download." The WebM file saves to Downloads. Closes the preview tab.
9. Popup shows Idle again, ready for the next recording.

**Climax:** The preview plays back without any corruption, the audio is in sync, and the file downloads in under 2 seconds. Alex doesn't need to re-record.

**Failure:** If the tabCapture dialog is denied, popup shows "Could not access screen or tab" with a link to `chrome://extensions`. Alex re-enables permissions, closes and re-opens the popup, and tries again.

### Flow 2 — Marie records a training tutorial (trainer, Chrome, afternoon)

1. Marie has her slide deck open in one tab and a code editor in another. She clicks the CaptureForge icon.
2. Popup: selects "Full Screen" mode, mic on. Starts recording.
3. **Countdown → Recording.** Marie narrates through her slides, using the mouse to point and click.
4. 12 minutes in, she notices a typo in her slide. She clicks **Pause** on the toolbar.
5. **Paused:** Timer blinks `00:12:37` with "Paused" label. Marie fixes the typo.
6. She clicks **Resume.** Timer stops blinking, continues from `00:12:37`. She finishes the tutorial.
7. **Stop → Preview.** 23-minute recording plays back. Marie reviews the full capture. V0.1 has no trim — she downloads the full WebM to edit elsewhere.
8. She later opens it in a separate video editor to trim.

**Climax:** Despite the 23-minute length, the recording never stuttered, audio stayed in sync, and the pause/resume boundary is seamless. Marie trusts the tool enough to use it for her next tutorial.

**Failure:** If the recording had hit a memory limit, the toolbar would show "Finalizing…" and the preview would contain content up to the limit. The integrity badge shows `Partial` — Marie chooses whether to keep the partial recording or re-record.

### Flow 3 — Karim files a bug report with recording (QA, Chrome, sprint end)

1. Karim reproduces a bug in the staging environment. Clicks CaptureForge icon.
2. Popup: "Full Screen" mode, mic on. **Starts recording.**
3. Karim walks through the bug steps: navigates to the page, fills a form, clicks submit, observes the error state. Narrates what they're seeing.
4. Recording is 47 seconds. Karim hits **Stop.**
5. **Preview:** Plays back. Karim confirms the error is visible.
6. Clicks **Download.** The WebM file is 4.2 MB.
7. Karim attaches the file to the Jira issue. Jira doesn't support WebM playback inline. Karim notes this and continues — the file plays fine on their local machine.
8. Back in the popup, Karim clicks "Delete" on the preview tab to clear the session from OPFS.

**Climax:** The recording was captured, exported, and attached in under 2 minutes. The QA workflow is faster than with Loom/Screencastify because there's no upload wait.

**Failure:** If Karim's session had been interrupted by a service-worker crash (long idle between reproducing the bug and starting CaptureForge), the recovery toast would appear at step 2: "A previous recording session was found." Karim would Restore it, download, and continue. No data lost.

## Inspiration & Anti-patterns

- **Lifted from Loom:** Simple popup with minimal options before recording. The start-now, configure-later philosophy.
- **Lifted from OBS:** Timer and stop/pause toolbar visible at a glance. The recording dot (red) is universal.
- **Lifted from Screenity (original):** Shadow-DOM content script approach for overlay — doesn't interfere with page styles.
- **Lifted from developer tools (Chrome DevTools):** Dark/light auto-theme. System fonts. Monospace timer. Visual restraint.
- **Rejected — Loom's share-and-copy flow:** No cloud, no share links, no copy-to-clipboard URL. CaptureForge downloads files locally.
- **Rejected — OBS's scene switcher / multi-source complexity:** V0.1 is one screen, one mic, one track. No scenes, no sources.
- **Rejected — Screenity's floating control panel:** The V0.1 toolbar is fixed top-center, not draggable. Dragging is a P1 addition.
- **Rejected — Modal confirmation on Stop:** Stop is immediate. Undo is available by not downloading in preview.
- **Rejected — Onboarding wizard:** First-run experience is the popup itself. No step-by-step setup in V0.1.
