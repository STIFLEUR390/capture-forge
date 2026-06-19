# Product Requirements Document — CaptureForge

**Version**: 1.0
**Status**: draft
**Created**: 2026-06-19
**Updated**: 2026-06-19

---

## Table of Contents

1. [Vision](#1-vision)
2. [Target Audience](#2-target-audience)
3. [Success Metrics & Counter-Metrics](#3-success-metrics--counter-metrics)
4. [Product Principles](#4-product-principles)
5. [Product Architecture](#5-product-architecture)
6. [Sub-Product A: Recorder Core (P0, V0.1)](#6-sub-product-a-recorder-core-p0-v01)
7. [Sub-Product B: Editor & Overlay (P1, V0.5)](#7-sub-product-b-editor--overlay-p1-v05)
8. [Sub-Product C: AI & Enrichment (P2, V2.0+)](#8-sub-product-c-ai--enrichment-p2-v20)
9. [Architectural North Star: AudienceLens](#9-architectural-north-star-audiencelens)
10. [Non-Functional Requirements](#10-non-functional-requirements)
11. [Privacy Model](#11-privacy-model)
12. [Browser Compatibility](#12-browser-compatibility)
13. [Build Configuration & Feature Flags](#13-build-configuration--feature-flags)
14. [Non-Goals](#14-non-goals)
15. [Adoption Thesis](#15-adoption-thesis)
16. [Maintenance Model](#16-maintenance-model)
17. [QA Plan](#17-qa-plan)
18. [Phased Roadmap](#18-phased-roadmap)
19. [Open Questions & Assumptions](#19-open-questions--assumptions)

---

## 1. Vision

CaptureForge is a privacy-first open-source screen recorder built as a browser extension, written in Rust and compiled to WebAssembly via [Oxichrome](https://crates.io/crates/oxichrome). Chrome-first, with Firefox planned after the recorder core is stable.

> *"Record like a pro, without giving up your privacy."*

### Core Tenets

| Tenet | Implication |
|-------|-------------|
| **Zero telemetry** | No analytics, no tracking, no Sentry. Every recording stays on the user's machine. |
| **Zero account** | No login, no cloud backend, no SaaS dependency. |
| **No artificial limits** | No duration cap, no watermark, no feature gate — within browser and device constraints. Free and fully capable out of the box. |
| **Local-first** | All processing happens in-browser — Rust/WASM for core, JS shims only where no Rust equivalent exists. |
| **Open source** | MIT license. Source on GitHub, distributed via GitHub Releases and Chrome Web Store. |

The product is structured as **three independent sub-products**, each independently developable, testable, and shippable:

```
Recorder Core (P0, V0.1)  ←  Editor + Overlay (P1, V0.5)  ←  AI / Enrichment (P2, V2.0+)
```

A bug or delay in one sub-product never blocks the others.

### Format Strategy

**V0.1** targets WebM output (VP8 + Opus via MediaRecorder). This is the format that maximises capture reliability with zero additional dependencies — no transcoding, no FFmpeg, no re-encode pipeline.

**MP4 export** is treated as priority P1, driven by real user workflows (Jira uploads, Slack sharing, Windows compatibility). The V0.1 focus is reliable capture and recovery; interop expands in P1 via FFmpeg WASM.

---

## 2. Target Audience

### Primary Personas

| Persona | Pain Point | Key Features Needed |
|---------|-----------|-------------------|
| **Alex, Developer** | Needs to record code reviews, technical demos, and create GIFs for PRs without a watermark or 5-min limit. Annotates code areas during recording. | Screen/tab recording, pen annotations, GIF export, keyboard shortcuts. |
| **Marie, Trainer** | Creates long-form tutorials (30min+). Needs pause/resume, microphone overlay, and post-recording trim. Future: subtitles, STT transcription. | Long recordings, mic, pause/resume, trim, camera PiP (P1), transcription (P2). |
| **Karim, QA Engineer** | Files bug reports with screen recordings. Needs to blur sensitive data, annotate bugs, and export MP4 for Jira/Linear integration. | Desktop capture, blur tool, annotations, MP4 export (P1). |

### Secondary Audiences

| Audience | Use Case | Captured In |
|----------|----------|-------------|
| Product managers | Walkthrough recordings for stakeholders | P0 (Recorder Core) |
| Technical writers | Step-by-step tutorial creation with screenshots | P2 (AudienceLens — Docs Lens) |
| Sales engineers | Demo recordings for prospects | P2 (AudienceLens — Sales Lens) |

---

## 3. Success Metrics & Counter-Metrics

### Primary Metrics (Product Quality)

| Metric | V0.1 Target | V1.0 Target | How Measured |
|--------|------------|-------------|--------------|
| Session completion rate | ≥95% of sessions finish without error | ≥98% | `SessionState::Stopping → Preview` ratio vs total starts |
| Recovery success rate | ≥90% of interrupted sessions propose valid recovery | ≥95% | `CrashRecovery → Preview` ratio vs recovery attempts |
| Start → first frame latency | <2s from click to recording | <1.5s | `performance.now()` from `StartRecording` to `MediaRecorder.start` callback |
| Export success rate | ≥98% of WebM exports produce valid files | ≥99% | Post-export file integrity check (header, duration, seekable) |
| Session re-record rate | — (baseline) | <15% of users re-record within 5min (proxy for early abandonment) | Telemetry-free: inferred from consecutive session starts without a completed export |
| Recording FPS | ≥25 FPS @ 1080p | ≥30 FPS @ 1080p | Automated benchmark in CI |
| Audio sync | Desync <100ms | Desync <50ms | Integration test suite |
| Max recording duration | 1h without crash | 2h without crash | Stress test suite |
| Export time (5min video) | <3s (WebM) | <3s (WebM), <2min (MP4) | Benchmark |

### Secondary Metrics (Adoption & Community)

| Metric | V0.1 Target | V1.0 Target | How Measured |
|--------|------------|-------------|--------------|
| GitHub stars | 500 | 5,000 | GitHub API |
| Weekly downloads (CWS) | 100 | 1,000 | Chrome Web Store dashboard |
| Issue resolution rate | 10/week | 20/week | GitHub project board |

### Counter-Metrics

| Counter-Metric | Why | Mitigation |
|----------------|-----|------------|
| WASM binary size bloat | Feature creep increases download size | Feature flags, `wasm-opt -Oz`, brotli compression. Target: <500KB gzipped for V0.1. |
| Memory exhaustion on long recordings | Users recording 1h+ may hit browser limits | Memory alert at 80% of storage quota, chunk-based OPFS writing to keep RAM <500MB. |
| Privacy concerns from DOM capture | DOM snapshot feature (P2) could capture sensitive data | `activeTab` scope only, disabled by default, auto-mask patterns, documented in privacy model. |
| Extension permission creep | Each new feature may require additional permissions | Audit every permission at PRD level. P0 requires: `storage`, `unlimitedStorage`, `desktopCapture`, `tabCapture`, `downloads`. No new permissions without PRD amendment. |

---

## 4. Product Principles

1. **Local-first by default.** Every feature works offline. No cloud dependency for core functionality. Network is opt-in and user-configured.

2. **No silent failure.** Every error produces a user-facing message with a suggestion. Background failures are logged at `warn!` and surfaced as badges in the popup. A crashed recording is never "lost" without the user knowing why.

3. **Standard export over lock-in.** Outputs use standard containers (WebM, MP4, GIF) and codecs (VP8, H.264). No proprietary format. No CaptureForge-specific player required to view exports.

4. **Optional intelligence, never mandatory.** AI features (STT, LLM, DOM capture) are feature-gated and compiled out by default. The extension is 100% functional without them. A missing AI module never blocks recording, editing, or export.

5. **Architecture must remain forkable and understandable.** Modules are decoupled by sub-product boundary. Dependencies are minimized. The workspace structure maps one-to-one to the feature hierarchy — a new contributor should find the relevant file without reading a design doc.

---

## 5. Product Architecture

### 5.1 Stack

| Layer | Technology | Purpose |
|-------|-----------|---------|
| Extension framework | Oxichrome v0.2 | Proc macros (`#[extension]`, `#[background]`, `#[popup]`), runtime wrappers for Chrome APIs |
| UI framework | Leptos v0.7 (CSR) | Reactive UI with `RwSignal`, `Effect`, `view!` macros |
| Rust-WASM bridge | wasm-bindgen v0.2 | Rust ↔ JS interop |
| Media APIs | web-sys v0.3 | MediaRecorder, MediaStream, Canvas, AudioContext |
| Storage | `opfs` crate + `indexed_db_futures` | OPFS primary, IndexedDB fallback |
| Serialization | serde v1 | Message passing, storage persistence |
| Video processing | FFmpeg WASM (JS shim, P1) | MP4/GIF export, transcoding |
| ML inference | sherpa-onnx crate (P2, optional) | Local STT transcription |
| LLM integration | `aisdk` crate (P2, optional) | Cloud LLM for doc generation, summarization |

### 5.2 Rust → JS Strategy

Rust-first with minimal JS shims where the ecosystem dictates:

| Capability | Approach | Rationale |
|------------|----------|-----------|
| Core business logic | Rust (Oxichrome + Leptos) | Recording, storage, state machine, UI |
| MediaRecorder | `web-sys` — native Rust bindings | Fully exposed via web-sys |
| Canvas annotations | `web-sys` Canvas 2D — direct, no abstraction | Performance-critical (<16ms per frame) |
| OPFS | `opfs` crate — native Rust | Full filesystem access from WASM |
| tabCapture / offscreen document | JS shim (`js/chrome_shim.js`, ~20 lines) | Chrome API not yet in web-sys |
| FFmpeg WASM | JS shim (`js/ffmpeg.js`) | No satisfactory Rust crate for browser-based FFmpeg |
| MediaPipe | JS shim (`js/mediapipe.js`) | Proprietary WASM model, JS-only distribution |
| sherpa-onnx | Native Rust crate | Full ONNX runtime in WASM |

### 5.3 Oxichrome Exit Strategy

Oxichrome v0.2 is a young framework. The architecture includes three layers of defence against framework risk:

**Internal wrappers on browser APIs.** All Chrome API calls (storage, tabs, commands, downloads) go through thin Rust traits under `src/background/`. These traits are framework-agnostic — they use `wasm-bindgen` and `web-sys` directly, not Oxichrome runtime wrappers. If Oxichrome becomes unmaintainable, only the proc-macro attributes in `src/lib.rs` and the build pipeline need replacement.

**Business modules are framework-agnostic.** The core modules (`recorder.rs`, `storage.rs`, `export.rs`, `editor.rs`) contain zero Oxichrome imports. They consume and produce `serde`-serializable messages through a `MessageRouter` trait. The same modules could be compiled into a Node.js CLI, a Tauri desktop app, or a pure WASM library without source changes.

**UI and runtime decoupled from recorder core.** The Leptos UI layer communicates with the recorder core exclusively through messages (`ExtensionMessage` enum). The popup, overlay, and preview pages are consumers of recording state, not owners. Replacing Leptos with a different framework (or native HTML) requires rewriting only the UI components, not the recording engine.

**Fork readiness.** The Oxichrome source is vendored in the build pipeline. If necessary, the project can fork and patch Oxichrome independently — the proc-macro surface is small enough to maintain (~5 attributes in V0.1).

### 5.4 Data Flow (Recorder Core)

```
User: Popup → "Start Recording"
    │
    ▼
background.rs (service worker)
    ├── 1. startRecording() → validate state, check for stale locks
    ├── 2. Create offscreen document (tab mode) or open recorder tab
    │
    ▼
recorder.rs (offscreen document / recorder page)
    ├── 3. acquireStream() — getDisplayMedia (desktop) / tabCapture (tab)
    ├── 4. MediaRecorder.start() — default codec: VP8 + Opus
    ├── 5. Chunk accumulation → OPFS every ~10s
    │      with chunk lifecycle: .partial → .written → .bin
    │
    User: "Stop" or crash
    ▼
    ├── 6. stop() → finalize last chunk
    ├── 7. Concat chunks → WebM blob
    └── 8. Open preview page (or recovery dialog after crash)
```

### 5.5 Workspace Structure

```
src/
├── lib.rs                     # #[extension] entry point
├── background.rs              # Service worker (listeners, messaging, heartbeat)
├── recorder.rs                # Recorder Core state machine
│   ├── lifecycle.rs           # start/stop/pause/resume/cancel
│   ├── chunk.rs               # Chunk accumulation + OPFS write protocol
│   └── stream.rs              # Stream acquisition (tab/desktop)
├── storage.rs                 # Storage layer
│   ├── opfs.rs                # OPFS writer
│   ├── indexdb.rs             # IndexedDB fallback (post-V0.1)
│   └── recovery.rs            # RecoveryManager, IntegrityReport
├── export.rs                  # Export pipeline
│   └── webm.rs                # WebM concatenation
├── content_script.rs          # Overlay + annotations (P1)
│   ├── overlay.rs             # Floating toolbar
│   ├── canvas.rs              # Annotation engine (pen, highlighter, text, shapes)
│   │   ├── tools.rs
│   │   └── history.rs         # Undo/Redo
│   ├── camera.rs              # Camera PiP (P1)
│   └── countdown.rs           # 3-2-1 countdown overlay
├── editor.rs                  # Editor (P1)
│   ├── player.rs              # Video player
│   ├── operations.rs          # Non-destructive trim/mute/crop
│   └── export.rs              # Export after edit
├── ai/                        # AI/Enrichment (P2, feature-gated)
│   ├── transcription.rs       # sherpa-onnx STT
│   ├── captions.rs            # SRT/VTT generation
│   ├── docgen.rs              # aisdk LLM tutorial generation
│   └── dom_capture.rs         # HTML snapshot capture
├── camera_page.rs             # Camera-only page (P1)
├── region_page.rs             # Region selection page (P1)
├── popup.rs                   # Mode selection popup
├── setup.rs                   # First-run setup wizard (post-V0.1)
└── permissions.rs             # Permission request UI
```

*(For full detail see `docs/architect.md`.)*

---

## 6. Sub-Product A: Recorder Core (P0, V0.1)

### 6.1 Scope

The minimum viable recording experience: screen and tab capture with microphone, pause/resume/stop, WebM export, OPFS storage, and basic crash recovery.

**V0.1 includes:**
- Full-screen recording (`desktopCapture` / `getDisplayMedia`)
- Tab recording (`tabCapture`)
- Microphone capture (simple AudioContext mix, single track)
- Pause / Resume
- Stop / Cancel
- 3-2-1 countdown
- WebM export (chunk concatenation, no re-encode)
- OPFS storage with chunk lifecycle (`Started → Written → Committed → Verified`)
- Basic crash recovery (detect orphan chunks, propose restore)
- Minimal preview (play, download, delete)
- Simple popup UI for mode selection

**Explicitly deferred to V0.2 / V0.3:**
- Storage manager with quota display (post-V0.1)
- Configurable keyboard shortcuts (`chrome.commands` defaults only)
- Setup wizard (permission onboarding via native Chrome dialogs)
- IndexedDB fallback (V0.2 — OPFS is reliably available on Chrome 120+; fallback adds test surface without user-facing value at launch)

### 6.2 User Stories

| ID | Story | Priority | Phase |
|----|-------|----------|-------|
| REC-01 | As a user, I can record my entire screen | P0 | V0.1 |
| REC-02 | As a user, I can record a specific browser tab | P0 | V0.1 |
| REC-03 | As a user, I can record my microphone alongside the screen | P0 | V0.1 |
| REC-04 | As a user, I can pause and resume a recording without losing data | P0 | V0.1 |
| REC-05 | As a user, I can stop recording and preview the result | P0 | V0.1 |
| REC-06 | As a user, I can cancel a recording-in-progress | P0 | V0.1 |
| REC-07 | As a user, I see a visual 3-2-1 countdown before recording starts | P0 | V0.1 |
| REC-08 | As a user, I can export my recording as WebM | P0 | V0.1 |
| REC-09 | As a user, my recording is stored in OPFS during capture | P0 | V0.1 |
| REC-10 | As a user, I am offered to recover a recording after a crash | P0 | V0.1 |

**Deferred stories:**

| ID | Story | Priority | Phase |
|----|-------|----------|-------|
| REC-11 | As a user, I can delete my recordings from the storage manager | P0 | V0.2 |
| REC-12 | As a user, I can start/stop recording with configurable keyboard shortcuts | P0 | V0.2 |
| REC-13 | As a user, I see storage usage and free space before starting | P0 | V0.3 |
| REC-14 | As a user, recordings seamlessly fall back to IndexedDB if OPFS is unavailable | P1 | V0.2 |

### 6.3 Acceptance Criteria

| ID | Criterion | Target | Reference Browser | Condition |
|----|-----------|--------|-------------------|-----------|
| REC-A1 | Recording framerate | ≥25 FPS @ 1080p | Chrome 120+ x86_64 | Desktop with GPU (Intel/AMD/Nvidia). Fallback to 720p accepted. |
| REC-A2 | Audio sync (mic + screen) | Desync <100ms | Chrome 120+ | Basic AudioContext mixer. Headset recommended. |
| REC-A3 | Pause/resume correctness | Total duration accurate | Chrome 120+ | <3 frames lost at resume boundary accepted. |
| REC-A4 | Chunk OPFS write interval | Every 10s ±1s | Chrome 120+ | No fallback in V0.1 — OPFS assumed available. |
| REC-A5 | No duration limit | 1h without crash | Chrome 120+ | RAM monitoring with alert at 80% of safe threshold. |
| REC-A6 | Start guard | Stale lock >30s auto-cleaned | Chrome 120+ | Lock stored in `chrome.storage.local`. |
| REC-A7 | WebM export (5min video) | <3s | Chrome 120+ | Simple chunk concatenation, no re-encode. |
| REC-A8 | Crash recovery | Manual restore proposed | Chrome 120+ | If OPFS chunks found at startup, offer "Restore". |
| REC-A9 | Chunk integrity | Written → Committed → Verified | Chrome 120+ | Triple verification (manifest vs filesystem, size check, index contiguity). |
| REC-A10 | Start → first frame | <2s from click to recording | Chrome 120+ | Measured from popup click to MediaRecorder `ondataavailable` first fire. |

### 6.4 User Interface States (Recorder Core)

| State | Visual | Transitions |
|-------|--------|-------------|
| `Idle` | Popup ready with mode selection, mic toggle, start button | → Starting |
| `Starting` | Spinner "Preparing…" | → Countdown / → Error |
| `Countdown` | Animated 3 → 2 → 1 circle fill | → Recording |
| `Recording` | Timer + toolbar (pause, stop) | → Paused / → Stopping |
| `Paused` | Blinking timer + "Paused" label | → Recording / → Stopping |
| `Stopping` | "Finalizing…" spinner | → Preview |
| `Preview` | Video player + actions (Download, Delete) | → Idle |
| `Error` | Error message with suggestion | → Idle |
| `CrashRecovery` | Toast "A previous recording was found — Restore?" | → Preview / → Idle |

For detailed component tree see `docs/ux-designer.md`.

### 6.5 Message Protocol (Recorder Core)

All messages serialized via `serde` and routed through the `background.rs` message router (~10 handlers for P0).

```rust
pub enum ExtensionMessage {
    StartRecording { mode: RecordingMode },
    StopRecording,
    PauseRecording,
    ResumeRecording,
    CancelRecording,
    GetStreamingData,
    ApplyStreamingData { data: String },
    VideoReady { session_id: String },
    RecordingError { code: String, details: String },
}
```

### 6.6 Storage Layout

```
OPFS:
  capture-forge/
    sessions/
      <sessionId>/
        screen/
          chunk_000000.bin        # Verified or Committed
          chunk_000001.partial    # In-progress
          ...
        manifest.json             # Session metadata + chunk index
        integrity-report.json     # Generated after recovery (Story 3)

chrome.storage.local:
  in_flight: SessionId | null     # Currently active session lock
```

**Chunk lifecycle:**

```
.partial (Started) → .written (Written) → .bin (Committed) → Verified (manifest only)
```

For full chunk lifecycle specification, see `docs/sprint-stories-resilient-storage.md`.

---

## 7. Sub-Product B: Editor & Overlay (P1, V0.5)

### 7.1 Scope

Non-destructive video editor with floating overlay toolbar and canvas annotations.

**Includes:**
- Video player for recorded sessions
- Trim (start/end cut)
- Mute audio
- Simple crop
- Floating toolbar during recording (pen, highlighter, text, shapes, blur)
- Undo/Redo for annotations
- Export after editing (WebM, P1+)
- Camera PiP overlay

### 7.2 User Stories

| ID | Story | Priority |
|----|-------|----------|
| ED-01 | As a user, I can play back my recording in a video player | P1 |
| ED-02 | As a user, I can trim the start and end of my recording | P1 |
| ED-03 | As a user, I can mute the audio track | P1 |
| ED-04 | As a user, I can crop the visible area of the video | P1 |
| ED-05 | As a user, I can export my edited video | P1 |
| ED-06 | As a user, I can draw on the screen during recording (pen, highlighter) | P1 |
| ED-07 | As a user, I can add text, shapes, and arrows during recording | P1 |
| ED-08 | As a user, I can blur sensitive areas of the screen | P1 |
| ED-09 | As a user, I can undo/redo my annotations | P1 |
| ED-10 | As a user, I can overlay my webcam in picture-in-picture mode | P1 |
| ED-11 | As a user, I can export my recording as MP4 or GIF | P1 |

### 7.3 Editor Architecture

```rust
pub struct EditorSession {
    source: SourceRef,        // OPFS session reference
    trim_start: f64,
    trim_end: f64,
    crop: Option<CropRect>,
    muted: bool,
}
```

Editing is **non-destructive**: operations are stored as metadata and applied at export time. The source session is never mutated.

---

## 8. Sub-Product C: AI & Enrichment (P2, V2.0+)

### 8.1 Scope

**Entirely optional.** The extension works at 100% without this module. All features are feature-gated.

### 8.2 Local STT — sherpa-onnx

| Property | Value |
|----------|-------|
| Engine | sherpa-onnx v1.13 (Rust crate, WASM target) |
| Model | Zipformer EN (~20MB) |
| Download | At first use, stored in OPFS |
| Language | English only (initial release) |
| CPU | 2 threads max |
| RAM | <200MB for 30min transcription |
| VAD | Voice Activity Detection for segment timing |
| Output | SRT, VTT subtitle formats |
| Fallback | No transcription if model not downloaded |

### 8.3 Cloud LLM — aisdk

| Property | Value |
|----------|-------|
| Engine | `aisdk` v0.2 crate |
| Providers | OpenAI, Anthropic, OpenRouter |
| Auth | User-supplied API key, stored in `chrome.storage.local` |
| Features | Tutorial generation (Markdown), auto-summary, smart search |
| Fallback | Not available without configured API key |

### 8.4 DOM Capture

| Property | Value |
|----------|-------|
| Trigger | User click on extension action |
| Scope | `activeTab` only, no background DOM access |
| Privacy | Disabled by default, auto-filter sensitive fields (password, credit card) |
| Storage | Snapshots stored in OPFS |
| Usage | Context for LLM doc generation, DOM-based analysis in audience lenses |

---

## 9. Architectural North Star: AudienceLens

*This section describes a long-term vision for semantic recording capabilities. It exists to constrain today's architectural decisions (storage format, message protocol, module boundaries), not to define a near-term roadmap commitment. No AudienceLens feature is planned before V2.0+.*

### 9.1 Concept

An **AudienceLens** is a declarative transformation engine that derives audience-specific publications from a single captured session — without ever mutating the source.

```
Session Source (immutable)          Publication (one per lens)
    │                                   │
    │  OPFS: tracks, manifest,          │  Sales / Dev / QA / Docs / Custom
    │  chunks, integrity signals         │
    ▼                                   ▼
AudienceLens ──────────────────────►  Derived output
    (visibility + transforms + outputs)
```

The source never changes. Lenses never write back. Publications are cached derivations, recomputed on invalidation.

### 9.2 Built-In Lenses (Illustrative)

| Lens | Tracks Included | Outputs |
|------|----------------|---------|
| **Sales** | video, audio_mic, camera_pip, overlays | Polished video, Markdown summary |
| **Dev** | video, audio_mic, audio_system, DOM, overlays | Video with tech overlay, Markdown with code snippets |
| **QA** | video, audio_mic, DOM, cursor | QA report, video with assertions |
| **Docs** | video, DOM, overlays | Tutorial (Markdown/MDX), screenshot gallery |

### 9.3 Security Model (Future)

Community lenses would be sandboxed at four levels:

| Phase | Mechanism |
|-------|-----------|
| **Install** | Static `CapabilitySet` validation against declared `VisibilityRules` |
| **Load** | Sandboxed WASM module or `<iframe>` with `sandbox` attribute |
| **Transform** | Capability proxy — lens receives `TrackSegment` views, never raw buffers |
| **Output** | Output size cap, content-type validation |

### 9.4 How the North Star Guides Today's Architecture

| Today's Decision | AudienceLens Requirement |
|-----------------|-------------------------|
| Chunk-based OPFS storage with manifest | Session structure is compatible with multi-track manifests |
| Serde messages on `ExtensionMessage` | Message router is extensible to lens render requests |
| Non-destructive editor | Source tracks remain immutable — lenses can always re-read |
| Feature flags per module | Capability gates map naturally to `CapabilitySet` declarations |
| Integrity reports per session | Every publication can reference its source integrity |

*(For full specification see `docs/audience-lens-architecture.md`.)*

---

## 10. Non-Functional Requirements

### 10.1 Performance

| ID | Requirement | Target | Measurement |
|----|-------------|--------|-------------|
| NFR-PERF-01 | Recording framerate | ≥25 FPS @ 1080p | `performance.now()` per frame callback |
| NFR-PERF-02 | Audio sync tolerance | <100ms drift | Cross-correlation of audio/video tracks |
| NFR-PERF-03 | RAM during recording | <500MB for 1h session | `performance.memory.usedJSHeapSize` |
| NFR-PERF-04 | WebM export (5min) | <3s | Wall-clock from export trigger to blob ready |
| NFR-PERF-05 | WASM load time | <1s | `performance.measure()` from SW start to ready |
| NFR-PERF-06 | Canvas annotation latency | <16ms per stroke | `performance.now()` per `requestAnimationFrame` |
| NFR-PERF-07 | MP4 export (5min) | <2min (P1) | Wall-clock, Web Worker off main thread |
| NFR-PERF-08 | Chunk write overhead | <200ms per 10s chunk | OPFS write latency per `FileSystemWritableFileStream` |

### 10.2 Reliability

| ID | Requirement | Target |
|----|-------------|--------|
| NFR-REL-01 | Uptime for recording sessions | 99% of sessions ≥1h complete without error |
| NFR-REL-02 | Crash recovery detection | 100% detection of orphaned OPFS chunks at startup |
| NFR-REL-03 | Data integrity after crash | 0% false positives in recovery (never claim full recovery when data is lost) |
| NFR-REL-04 | Chunk verification | Triple check (manifest vs filesystem, size, index contiguity) on every recovery |
| NFR-REL-05 | Graceful degradation | Every failure mode produces a user-facing message, not a silent error |

### 10.3 Security

| ID | Requirement |
|----|-------------|
| NFR-SEC-01 | No data ever leaves the browser except user-initiated downloads and optional API calls (P2) |
| NFR-SEC-02 | DOM capture (P2) is disabled by default, `activeTab` only, with auto-mask for sensitive fields |
| NFR-SEC-03 | Community audience lenses (P2+) run in sandboxed environments with declared `CapabilitySet` |
| NFR-SEC-04 | API keys stored in `chrome.storage.local` — never in extension code or localStorage |
| NFR-SEC-05 | All network requests (P2 only) go to user-configured endpoints — no hardcoded third-party URLs |

### 10.4 Accessibility

| ID | Requirement | Standard |
|----|-------------|----------|
| NFR-A11Y-01 | All interactive elements have `aria-label` | WCAG 2.1 AA |
| NFR-A11Y-02 | Full keyboard navigation (Tab/Enter/Escape) | WCAG 2.1 AA |
| NFR-A11Y-03 | Color contrast ≥4.5:1 | WCAG 2.1 AA |
| NFR-A11Y-04 | Animations respect `prefers-reduced-motion` | WCAG 2.1 AA |
| NFR-A11Y-05 | Screen reader announcements for recording state changes | WCAG 2.1 AA |

### 10.5 Internationalization

| ID | Requirement |
|----|-------------|
| NFR-I18N-01 | V0.1: English + French |
| NFR-I18N-02 | V1.0: 18 languages (en, fr, de, es, ca, hi, id, it, ko, pl, pt_BR, pt_PT, ru, ta, tr, uk, zh_CN, zh_TW) |
| NFR-I18N-03 | Locale files in `static/locales/{lang}.json`, `i18n` crate for Rust-side strings |

---

## 11. Privacy Model

| Data Type | Storage | Network |
|-----------|---------|---------|
| Video recordings | OPFS (local) | ❌ Never |
| Audio recordings | OPFS (local) | ❌ Never |
| Session metadata (duration, date) | `chrome.storage.local` | ❌ Never |
| Debug logs | `chrome.storage.local` | ❌ Never |
| API key (aisdk/LLM) | `chrome.storage.local` | ✅ HTTPS to user-configured provider (P2) |
| ONNX model (sherpa) | OPFS | ✅ Single download, then offline (P2) |
| DOM snapshots | OPFS | ❌ Never (if feature enabled, P2) |
| Usage analytics | ❌ N/A | ❌ None — zero telemetry by design |
| Account credentials | ❌ N/A | ❌ No account system |

**Privacy guarantees:**
- No analytics SDK, no Sentry, no error-reporting service
- No background network requests unless the user explicitly configures an API key (P2)
- All AI inference runs locally via WASM (P2) — no cloud dependency for core features
- Open source codebase — anyone can verify the privacy model

---

## 12. Browser Compatibility

| Feature | Chrome 120+ | Firefox 125+ | Edge 120+ | Notes |
|---------|-------------|--------------|-----------|-------|
| Screen recording (desktopCapture) | ✅ V0.1 | ⬜ P1 | ✅ V0.1 | |
| Tab recording (tabCapture) | ✅ V0.1 | ⬜ P1 | ✅ V0.1 | Firefox uses different API |
| MediaRecorder API | ✅ V0.1 | ✅ V0.1 | ✅ V0.1 | Baseline for WebM capture |
| OPFS | ✅ V0.1 | ⬜ P1 | ✅ V0.1 | Firefox support added 2024 |
| Offscreen document | ✅ V0.1 | ❌ N/A | ✅ V0.1 | Firefox uses dedicated tab instead |
| Extension popup | ✅ V0.1 | ✅ V0.1 | ✅ V0.1 | |
| Content scripts | ✅ V0.1 | ✅ V0.1 | ✅ V0.1 | For overlay toolbar (P1) |
| Canvas 2D annotations | ✅ V0.1 | ✅ V0.1 | ✅ V0.1 | web-sys Canvas bindings |
| sherpa-onnx WASM | ✅ P2 | ⬜ P2 | ✅ P2 | WASM threading support varies |
| Downloads API | ✅ V0.1 | ✅ V0.1 | ✅ V0.1 | For export |

**Development reference browser:** Chrome 120+ on x86_64.

**Firefox:** Planned after recorder core is stable (V1.0). No Firefox-specific work in V0.1 or V0.5.

---

## 13. Build Configuration & Feature Flags

```toml
[features]
default = ["recorder", "storage", "export"]
recorder = []         # Recording core, stream acquisition
storage = []          # OPFS + recovery
export = []           # WebM concatenation
indexeddb = []        # IndexedDB fallback (V0.2+)
overlay = []          # Content script overlay + canvas annotations (P1)
editor = []           # Video editor (P1)
camera = []           # Camera PiP (P1)
stt = ["sherpa-onnx"] # Local STT transcription (P2)
llm = ["aisdk"]       # Cloud LLM integration (P2)
dom = []              # DOM capture (P2)
```

**Build commands:**

```bash
# Development
wasm-pack build --target web

# Full pipeline with manifest generation
cargo oxichrome build              # Debug (~361KiB WASM)
cargo oxichrome build --release    # Optimized with wasm-opt -Oz

# Feature-gated builds
cargo oxichrome build --no-default-features --features recorder,storage,export
cargo oxichrome build --features overlay,editor,camera
```

---

## 14. Non-Goals

| Feature | Rationale |
|---------|-----------|
| Cloud recording / SaaS backend | Zero-backend architecture. No CaptureForge Pro or cloud tier planned. |
| User accounts / Login | Zero-auth by design. No sign-up, no session management. |
| Telemetry / Analytics | Zero tracking. No Sentry, no GA, no crash reporting. |
| Safari support | Insufficient market share for WebExtensions; limited API surface. |
| Multi-scene / timeline editing | Would require significant video infrastructure. Beyond V1 scope. |
| Keyframe zoom animations | Niche feature for power users. Tracked as future idea. |
| Direct YouTube/Vimeo export | Third-party API instability; violates privacy-first positioning. |
| Background removal in pure Rust | MediaPipe JS interop is the pragmatic choice. |
| Speaker diarization | Model too large for browser-based first release. |
| Region selection | Complexity too high for MVP. Scoped to P1. |
| Multiple audio tracks | MVP uses single mixed track (video+audio). Multi-track in P1+. |
| MP4 export in V0.1 | Requires FFmpeg WASM dependency; V0.1 optimises reliable capture over max interop. |
| Storage manager UI in V0.1 | Basic download/delete in preview suffices; quota management is V0.3. |
| Configurable keyboard shortcuts in V0.1 | `chrome.commands` defaults (Alt+Shift+G/M/X) ship as-is; customization UI deferred. |

---

## 15. Adoption Thesis

CaptureForge grows its user base through a **bottom-up, workflow-driven model**, not top-down marketing.

### Initial Beachhead (V0.1)

**Dev / QA / technical support** is the entry point. These users already reach for screen recording tools daily. They understand browser extensions, tolerate permission prompts, and value privacy guarantees. They are also the most likely to discover CaptureForge through GitHub (Rust/WASM novelty, Oxichrome ecosystem, open-source screen recorder).

**Acquisition channels:**
- GitHub: Rust/WASM community, Oxichrome showcase, Hacker News launch
- Chrome Web Store: organic search for "screen recorder", "open source screen recorder"
- Developer forums: Reddit (r/rust, r/webdev), dev.to, lobste.rs

### Growth Mechanics

1. **Individual adoption.** A developer installs CaptureForge for a code review or demo. Zero friction — no account, no credit card, no time limit.
2. **Recurring use.** The recording quality, crash recovery, and keyboard shortcuts pull them back. WebM exports work everywhere.
3. **Intra-team spread.** The developer shares a recording with a colleague. The colleague sees no watermark, no "Powered by" branding — and asks where to get it.
4. **Cross-role expansion.** A team's QA engineer (Karim persona) starts using it for bug reports. A technical writer (Docs Lens, P2) adopts it for tutorials.
5. **Community contribution.** Open source invites feature contributions, translations, and Firefox porting help. The MIT license removes adoption friction for companies.

### What GitHub Stars Measure vs What Matters

GitHub stars are a **signal of interest, not a driver of adoption**. The real V0.1 retention signal is: do users complete a second recording session within 7 days of the first? That metric cannot be tracked (no telemetry), so the product must make the experience good enough that repeat usage is the natural outcome — reliable capture, no data loss, instant export.

---

## 16. Maintenance Model

### Governance

- **Minimal BDFL structure.** Project lead owns the vision and merge decisions. Community contributions are welcome but go through the same PR process.
- **No formal foundation or steering committee in V0.1.** If the project grows beyond a single maintainer, a lightweight CONTRIBUTORS.md model can be introduced.

### Contribution Acceptance Criteria

| Criterion | Explanation |
|-----------|-------------|
| Feature corresponds to a PRD story or approved RFC | Avoids scope creep from well-intentioned drive-by PRs |
| No new dependencies without justification | Every added `Cargo.toml` entry must include a rationale |
| Tests included | Rust unit tests for business logic; Playwright for E2E |
| Feature-gated for P1/P2 features | New functionality behind a Cargo feature flag |
| Privacy reviewed | Does this change introduce a network request, a new permission, or data leakage? |

### Module Compatibility

- **Breaking changes** to the message protocol (`ExtensionMessage`) require a major version bump (0.x → 0.y+1).
- **Breaking changes** to the OPFS session format (manifest.json schema, chunk naming) must include a migration adapter that reads old formats.
- **Public API surface** is limited to what Oxichrome's `#[extension]` exports. The internal module API (`recorder.rs`, `storage.rs`, etc.) is considered private even when `pub` in Rust — don't depend on it from outside the crate.

### Stability Policy

| Artifact | V0.x Stability | V1.0+ Stability |
|----------|---------------|-----------------|
| OPFS session format (manifest + chunks) | Experimental — may change within V0.x | Stable — backward-compatible readers required |
| Export format (WebM container) | Stable — VP8+Opus is a published spec | Stable |
| Extension message protocol | Unstable — may add variants | Stable — additive changes only |
| Cargo feature flags | Unstable — may rename or merge | Deprecation cycle before removal |
| Keyboard shortcut bindings | Fixed at `chrome.commands` defaults | Configurable via UI |

---

## 17. QA Plan

### 17.1 Unit Tests (Rust)

```bash
cargo test                           # All modules
cargo test --features editor         # With editor module
cargo test --features stt            # With transcription module
```

**State machine coverage:**

```
Idle → Starting → Recording → Paused → Recording → Stopping → Idle
Idle → Starting → Recording → Stopping → Exporting → Done
Idle → Starting → Recording → Error → Idle
Idle → Starting → Error → Idle
Idle → CrashRecovery → Preview → Idle
```

### 17.2 Integration Tests

| Test Suite | What It Covers |
|------------|----------------|
| Message router | Every handler receives correct message and produces correct response |
| OPFS read/write/delete | Full file system operations with chunk lifecycle |
| Chunk lifecycle | Started → Written → Committed → Verified transitions |
| Export pipeline | WebM concatenation correctness |
| Recovery integrity | Triple verification: manifest vs filesystem, size check, index contiguity |
| Crash recovery E2E | Kill service worker mid-write, restart, verify recovery proposal |
| Session boundary | State machine rejects double-start, double-stop, pause-in-idle, etc. |

### 17.3 E2E Tests (Playwright)

| Scenario | What It Validates |
|----------|-------------------|
| Record 10s → Stop → Preview → Download | Happy path |
| Record → Pause 3s → Resume → Record 5s → Stop → total = 15s | Pause/resume accuracy |
| Record → Kill SW → Restart → Recovery proposed | Crash recovery |
| Record → Editor → Trim → Export (P1) | Edit pipeline |
| Record → Annotations → Stop → Export (P1) | Annotation pipeline |
| Record 1h → check memory + storage | Stress test |

### 17.4 Performance Benchmarks

| Benchmark | Tool | Frequency |
|-----------|------|-----------|
| FPS during recording | Custom Playwright + `performance.now()` | Per commit |
| Export time vs video length | Custom script with variable-length recordings | Per release |
| WASM module load time | `performance.measure()` | Per build |
| Memory growth per minute | `performance.memory` during 30min recording | Per release |

### 17.5 Crash Recovery Test Protocol

1. Start a recording session
2. Kill the service worker (via `chrome://extensions` or programmatic `self.close()`)
3. Reload the extension
4. Verify that the recovery dialog appears
5. Accept recovery → verify the preview shows content up to the last committed chunk
6. Verify the integrity report correctly identifies the recovered range and any orphans

---

## 18. Phased Roadmap

### Phase 0: Recorder Core (V0.1) — P0

**Theme: Reliable capture, resilient recovery.**

```
├── Screen + Tab recording
├── Microphone (simple mix, single track)
├── Pause / Resume / Stop / Cancel
├── 3-2-1 countdown animation
├── WebM export (chunk concatenation)
├── OPFS storage (single mixed track)
│   ├── Chunk lifecycle (.partial → .written → .committed → .verified)
│   └── Triple verification (Story 2-3)
├── Basic crash recovery
├── Integrity report as session output
├── Default keyboard shortcuts (Alt+Shift+G/M/X)
├── Popup UI (mode selection, mic toggle, start)
├── Minimal preview page (play, download, delete)
└── Permissions (storage, desktopCapture, tabCapture, downloads)
```

**Dependencies:** Oxichrome v0.2, Leptos v0.7, web-sys, opfs crate.

### Phase 1: Polish & Peripherals (V0.2) — P0

**Theme: Closing the UX gaps.**

```
├── Storage manager (list sessions, delete, quota estimate)
├── Configurable keyboard shortcuts (chrome.commands UI)
├── IndexedDB fallback (OPFS unavailable path)
├── First-run permission onboarding page
├── i18n: French locale
└── Setup wizard (basic: first-launch explanation)
```

### Phase 2: Editor + Overlay (V0.5) — P1

```
├── Floating toolbar (shadow DOM injection)
├── Canvas annotations (pen, highlighter, text, shapes, arrow, blur)
├── Undo/Redo annotation history
├── Video player (web-sys <video>)
├── Trim (start/end)
├── Mute
├── Simple crop
├── Export after editing
├── Camera PiP (select, resize, drag)
├── Camera-only recording page
├── Region selection page
└── i18n (18 languages)
```

### Phase 3: Firefox Support (V1.0) — P1

```
├── Build target: --target firefox
├── Offscreen document → dedicated tab adaptation
├── Firefox-specific permission model
├── E2E tests on Firefox
└── Firefox Add-ons store submission
```

### Phase 4: Advanced Export (V1.0) — P1

```
├── MP4 export (FFmpeg WASM)
├── GIF export (FFmpeg WASM)
└── Quality presets (high / medium / fast)
```

### Phase 5: AI / Enrichment (V2.0+) — P2

```
├── sherpa-onnx Zipformer EN
│   ├── Model download → OPFS (progressive)
│   ├── VAD-based transcription
│   └── SRT/VTT export
├── aisdk LLM integration
│   ├── Tutorial generation (Markdown)
│   ├── Auto-summary
│   └── Smart search
└── DOM capture
    ├── ActiveTab scope only
    ├── Privacy filters (auto-mask)
    └── OPFS storage
```

### Phase 6: AudienceLens (V2.0+) — P2

```
├── Lens lifecycle (Draft → Validated → Active → Disabled)
├── Built-in lenses: Sales, Dev, QA, Docs
├── Lens sandboxing (WASM / iframe isolation)
├── CapabilitySet declaration and enforcement
├── Publication pipeline (Pending → Rendering → Ready / Failed / Partial)
└── Community lens marketplace exploration
```

---

## 19. Open Questions & Assumptions

### Open Questions

| ID | Question | Owner | Resolution Condition |
|----|----------|-------|---------------------|
| OQ-01 | Single codec (VP8) or codec ladder with fallback in V0.1? | [TBD] | After first round of user testing on varied hardware |
| OQ-02 | What is the maximum acceptable WASM binary size? | [TBD] | After measuring cold-load times on mid-tier devices |
| OQ-03 | Should Firefox recording use the same offscreen-document approach or a dedicated background tab? | [TBD] | When Firefox P1 work begins |
| OQ-04 | What is the right chunk size for OPFS flush? 10s or adaptive based on scene complexity? | [TBD] | After performance benchmarks on baseline hardware |
| OQ-05 | sherpa-onnx: Zipformer EN vs Moonshine tiny (~20MB each) — which performs better in WASM? | [TBD] | When P2 STT work begins, benchmark both |
| OQ-06 | What is the minimum Chrome version to support? Currently 120+, but can we go lower? | [TBD] | After checking OPFS and offscreen document availability |
| OQ-07 | What is the threshold for "re-recording" as a quality signal in a telemetry-free product? | [TBD] | After analyzing session patterns in V0.1 internal dogfooding |

### Assumptions

| ID | Assumption | Risk if Wrong |
|----|------------|---------------|
| [ASSUMPTION-01] | Oxichrome v0.2 is stable enough for production use. | Need to fork and maintain Oxichrome internally, delaying all development. |
| [ASSUMPTION-02] | Chrome 120+ market share is sufficient for V0.1 adoption. | Need to support older Chrome versions, adding polyfill complexity. |
| [ASSUMPTION-03] | VP8+Opus in MediaRecorder is universally supported. | Fallback to VP9 may be needed on some hardware, adding codec negotiation complexity. |
| [ASSUMPTION-04] | Users are willing to install an extension with `desktopCapture` + `tabCapture` permissions. | May need to split permissions or add educational screens if users bounce. |
| [ASSUMPTION-05] | OPFS is reliably available in Chrome 120+ for all users. | Heavy reliance on IndexedDB fallback path (V0.2) may degrade user experience. |
| [ASSUMPTION-06] | The 3 sub-products model (Recorder → Editor → AI) is the right decomposition. | If coupling between modules is higher than expected, refactoring cost increases. |
| [ASSUMPTION-07] | Dev/QA audience is the correct initial beachhead for V0.1. | Need to pivot positioning and features if early adopters are non-technical users expecting MP4 and camera PiP out of the box. |
