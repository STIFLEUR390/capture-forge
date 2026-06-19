---
stepsCompleted: [1, 2, 3, 4, 5, 6, 7, 8]
inputDocuments:
  - planning-artifacts/prds/prd-capture-forge-2026-06-19/prd.md
  - planning-artifacts/briefs/brief-capture-forge-2026-06-19/brief.md
  - planning-artifacts/ux-designs/ux-capture-forge-2026-06-19/DESIGN.md
  - planning-artifacts/ux-designs/ux-capture-forge-2026-06-19/EXPERIENCE.md
  - planning-artifacts/ux-designs/ux-capture-forge-2026-06-19/.decision-log.md
  - planning-artifacts/research/technical-capture-persistence-architecture-2026-06-19.md
  - planning-artifacts/research/technical-feasibility-webcodecs-opfs-chrome-apis-capture-forge-2026-06-19.md
  - docs/architect.md
  - docs/product-brief.md
  - docs/prd.md
  - docs/ux-designer.md
  - docs/audience-lens-architecture.md
  - docs/brainstorming.md
  - docs/sprint-stories-resilient-storage.md
  - CLAUDE.md
workflowType: architecture
lastStep: 8
status: complete
completedAt: 2026-06-19
project_name: capture-forge
user_name: Herold
date: 2026-06-19
---

# Architecture Decision Document — CaptureForge

*This document builds collaboratively through step-by-step discovery. Sections are appended as we work through each architectural decision together.*

## Project Context Analysis

### Requirements Overview

**Functional Requirements (V0.1 — Recorder Core):**
- Screen capture via `getDisplayMedia` (full desktop) and `tabCapture` (specific tab)
- Microphone capture via simple AudioContext mixer — single mixed track (video+audio)
- Pause / Resume / Stop / Cancel with correct duration tracking
- 3-2-1 countdown animation on a full-viewport overlay
- WebM export via chunk concatenation (no re-encode, no FFmpeg)
- OPFS storage with formal chunk lifecycle: `.partial` → `.written` → `.bin` → `Verified`
- Crash recovery: detect orphaned OPFS chunks at startup, surface via non-modal toast
- Minimal preview page with play, download, delete actions
- 3 UI surfaces: Popup (280px), Content script overlay (toolbar), Offscreen document (preview)
- Default keyboard shortcuts: `Alt+Shift+G/M/X`

**Non-Functional Requirements:**
- Performance: ≥25 FPS @ 1080p, audio desync <100ms, WebM export 5min <3s
- Memory: <500MB RAM for 1h recording (OPFS chunk every 10s keeps heap low)
- Startup: WASM load <1s, binary <500KB gzipped
- Reliability: 99% session completion, 100% orphan detection, triple verification on recovery
- Accessibility: WCAG 2.1 AA, `aria-live` for state changes, `prefers-reduced-motion`
- Privacy: Zero telemetry, zero network (except user-configured API key in P2)
- Dark/Light: System theme auto via `prefers-color-scheme`

**Scale & Complexity:**
- Complexity level: **Medium-High** — real-time media capture in a constrained WASM+MV3 runtime
- Primary domain: Chrome extension (Manifest V3) with Rust/WASM compilation
- Estimated architectural components (V0.1): 8 modules + 3 JS shims
- Three sub-products: Recorder Core (V0.1) ← Editor + Overlay (V0.5) ← AI (V2.0+)

### Technical Constraints & Dependencies

| Constraint | Impact |
|------------|--------|
| Chrome MV3 service worker ~30s idle timeout | Recording must run in offscreen document, not SW. Heartbeat needed. |
| WASM compilation (wasm32-unknown-unknown) | No native threading, no SIMD, limited std. Must optimize binary size. |
| Oxichrome v0.2 (young framework) | Exit strategy defined: inner wrappers, framework-agnostic modules, fork-ready. |
| OPFS sole storage (V0.1, no IndexedDB fallback) | 100% reliance on OPFS availability in Chrome 120+. Assumption-05. |
| web-sys / js-sys bindings | Chrome APIs not fully exposed (tabCapture needs JS shim). |
| Leptos 0.7 CSR | Reactive UI but WASM-compiled. No access to browser DOM from content scripts. |
| MediaRecorder API (browser-native) | Codec support varies. V0.1 targets VP8+Opus as baseline. |
| Format: WebM only (V0.1) | MP4 added in P1 via FFmpeg WASM JS shim. |

### Cross-Cutting Concerns Identified

| Concern | Modules Affected | Mitigation |
|---------|-----------------|------------|
| Recording state machine | All (background, recorder, popup, overlay, preview) | Single `RecorderSession` in `recorder.rs` — all surfaces are consumers. |
| Chunk integrity | storage, recovery, export | Triple verification + `IntegrityReport` per session. |
| Message routing | background (router), all consumers | `ExtensionMessage` enum, serde-serialized, ~10 handlers V0.1. |
| Dark/Light theme | popup, overlay, preview | CSS `prefers-color-scheme` + CSS custom properties. No JS toggle V0.1. |
| Feature flags | build (Cargo), runtime (if/else) | 3 default features, 6 optional. Compiled out — no runtime cost. |
| Permission management | background, popup | Chrome native dialogs. Declared in manifest + `#[oxichrome::extension]`. |
| Crash recovery | storage, background, popup | Cross-cutting: SW restart → detect orphans → toast → restore flow. |

## Starter Template Evaluation

### Primary Technology Domain

Chrome browser extension (Manifest V3) compiled from Rust to WebAssembly. Not a conventional web/backend stack — no generic starter template applies.

### Stack Already Established

| Layer | Technology | Status |
|-------|-----------|--------|
| Extension framework | Oxichrome v0.2 | In Cargo.toml, scaffold generated |
| UI framework | Leptos v0.7 (CSR) | Declared in Cargo.toml, components pending |
| Language | Rust (wasm32-unknown-unknown) | Workspace structure in place |
| Build | wasm-pack + cargo oxichrome | Commands documented in CLAUDE.md |
| Media APIs | web-sys (MediaRecorder, Canvas, AudioContext) | Features declared in Cargo.toml |
| Storage | opfs crate + indexed_db_futures | Added to dependencies |
| Serialization | serde v1 (derive) | In Cargo.toml |
| JS interop | JS shims (chrome_shim.js, ffmpeg.js, mediapipe.js) | Directory structure in docs |
| AI (P2, optional) | sherpa-onnx crate, aisdk crate | Feature-gated in Cargo.toml |

### Initialization

No CLI init needed — the Rust crate is already scaffolded:

```bash
cargo check                          # Validate Rust compilation
wasm-pack build --target web         # Build WASM
cargo oxichrome build --release      # Full pipeline with manifest
```

Architectural decisions from here focus on **module design, data flow, state management, and error handling** — not on project initialization.

## Core Architectural Decisions

### Decision Priority Analysis

**Critical Decisions (Block Implementation):**
- Module communication pattern (Decided: Hybrid)
- WASM binary strategy (Decided: 2-WASM — core + AI)
- Error handling in WASM (Pending)
- Chunk binary format (Pending)
- Heartbeat / SW keepalive strategy (Pending)
- Test strategy for WASM modules (Pending)

**Important Decisions (Shape Architecture):**
- JS shim interface design (Decided: thin trait wrappers per PRD §5.2)
- Offscreen document lifecycle (Pending)
- OPFS directory layout (Decided: per §6.6 of PRD)
- Session manifest schema (Pending)
- Keyboard shortcut registration (Decided: `chrome.commands` defaults V0.1)

**Deferred Decisions (Post-MVP):**
- Multi-track storage layout (P1+)
- WASM split timing for AI modules (re-evaluate when P2 begins)
- FFmpeg WASM integration pattern (P1)
- MediaPipe JS interop design (P1)
- Community lens sandboxing (P2+)

### Module Communication Pattern

**Decision:** Hybrid — direct Rust calls within core modules, message-passing at the UI boundary.

**Options considered:**
- **A — Message-passing pur:** Rejected — unnecessary boilerplate for intra-module calls in the same WASM binary. Every recursive call would serialize/deserialize.
- **B — Appels directs Rust:** Rejected — surfaces the entire internal API to UI consumers, creating coupling between the popup/overlay and core internals.
- **C — Hybride:** **Selected.** Core modules (recorder, storage, export) call each other directly via `pub fn` and traits. UI surfaces (popup, overlay, preview) communicate via `ExtensionMessage` through the background message router.

**Interface boundary:**
```
UI (popup / overlay / preview)
    │ ExtensionMessage (serde, via router)
    ▼
Background router (dispatches to core)
    │ direct Rust calls
    ▼
Core modules (recorder → storage → export)
```

**Affects:** All modules. The `ExtensionMessage` enum in `background.rs` defines the public IPC surface. Internal function signatures are the private API.

### WASM Binary Strategy

**Decision:** 2-WASM architecture — `core.wasm` + `ai.wasm`.

**Rationale:**
- **core.wasm** (V0.1+) — recorder, storage, export, editor, overlay. Everything a user needs to record, preview, edit, and export. Feature-gated internally (editor, overlay compiled in via Cargo features but inactive at runtime if gated).
- **ai.wasm** (P2+) — sherpa-onnx STT, aisdk LLM, DOM capture. Loaded on-demand only when the user activates an AI feature. Never loaded during a recording-only session.
- The AI module has fundamentally different characteristics: model weights, ML runtime, memory profile, async initialization, optional downloads. Keeping it separate ensures a recording-only user never pays the AI cold-start cost.
- Editor P1 stays in core.wasm because it shares the same session storage, metadata format, and export pipeline as the recorder.

**Build rule:** Any module not required for "start recording in <2 clicks" must not degrade the core cold-start.

```
V0.1 – V1.0:              P2+:
┌──────────────────┐      ┌──────────────────┐
│  core.wasm        │      │  core.wasm        │
│  ├─ recorder      │      │  └─ (unchanged)  │
│  ├─ storage       │      └──────────────────┘
│  ├─ export        │      ┌──────────────────┐
│  ├─ editor (P1)   │      │  ai.wasm (lazy)   │
│  ├─ overlay (P1)  │      │  ├─ stt (sherpa) │
│  └─ ...           │      │  ├─ llm (aisdk)  │
└──────────────────┘      │  └─ dom_capture  │
                          └──────────────────┘
```

**Affects:** Build pipeline (`Cargo.toml` features, `oxichrome.config.toml`), JS/WASM loader, cold-start performance.

### Error Handling in WASM

**Decision:** `thiserror` + `RecordingError` enum with stable error codes + custom panic hook.

**Mechanism:**
- Every public function in core modules returns `Result<T, RecordingError>`.
- `RecordingError` is a `thiserror`-derived enum with stable string codes (`StreamAcquisitionFailed`, `MediaRecorderError`, `WriteError`, `ExportError`, `StateViolation`, etc.) and a `details: String` field for context.
- `?` operator propagates errors upward through the call chain.
- At the message boundary, the background router catches `Err` and maps it to `ExtensionMessage::RecordingError { code, details }`.
- A custom `panic::set_hook()` override catches unexpected panics, logs the message via `console.error`, and transitions the session to `Error` state instead of letting the WASM instance die.
- `expect("invariant: ...")` in hot paths where failure should be impossible (state machine transitions, lock acquisition, confirmed-write reads). Never bare `unwrap()`.

**Affects:** All modules. Every `recorder.rs`, `storage.rs`, `export.rs` function signature uses `Result`. The `RecordingError` type is defined in a shared module.

### Chunk Binary Format

**Decision:** 32-byte header + raw MediaRecorder blob — self-describing chunk files.

**Header layout:**
```
Offset  Size  Field
0       4     Magic: "CFCH" (0x43464348)
4       1     Version (0x01)
5       4     Chunk index (u32 LE)
9       8     Timestamp ms (f64 LE)
17      8     Payload size (u64 LE)
25      4     Checksum XXH3 (u32 LE)
29      3     Reserved (zero)
32      N     MediaRecorder blob
```

**Rationale:**
- Self-describing: recovery can reconstruct ordering and verify integrity without the manifest file.
- 32 bytes per ~2MB chunk = 0.0016% overhead.
- Magic bytes + version enable format evolution (future versions can change the header).
- XXH3 checksum is fast (<1GB/s in WASM) and catches file corruption.
- The payload is the raw MediaRecorder output — no transcoding or re-encoding needed.

**File-naming convention (unchanged from PRD):**
```
chunk_{index:06}.partial  →  chunk_{index:06}.written  →  chunk_{index:06}.bin
```
The `.bin` file carries the 32-byte header. `.partial` and `.written` files may have incomplete headers (zero checksum) — identified by their extension during recovery.

**Affects:** `chunk.rs` (writer), `webm.rs` (reader), `recovery.rs` (integrity check).

### Heartbeat / SW Keepalive

**Decision:** Offscreen-document ping/pong every 20s during active recording.

**Mechanism:**
- When recording starts, the offscreen document starts a 20s `setInterval`.
- Each tick sends `ExtensionMessage::KeepalivePing` to the service worker.
- SW responds with `KeepalivePong`. Receiving any message resets Chrome's SW idle timer.
- When recording stops, the offscreen document clears the interval.
- If 3 consecutive pings go unanswered (60s timeout), the offscreen document assumes the SW is dead, finalizes the current chunk, and transitions to `CrashRecovery` state.

**Rationale:**
- The offscreen document's lifecycle (created at recording start, destroyed at stop) matches the keepalive window exactly — no stale keepalive cycles.
- 20s is well within the 30s SW idle timeout with margin.
- 3-ping timeout adds resilience: a single missed ping (transient Chrome scheduling delay) doesn't trigger a false recovery.

**Affects:** `background.rs` (listeners — add ping/pong handler), `recorder.rs` (offscreen document — keepalive loop).

### Test Strategy for WASM Modules

**Decision:** Three-tier test pyramid — unit (native), WASM (headless), E2E (Playwright).

| Tier | Tool | Scope | Frequency | What it covers |
|------|------|-------|-----------|----------------|
| **Unit (A)** | `cargo test` (native host) | Pure Rust logic | Every commit | State machine transitions, serde roundtrip, chunk header encode/decode, `RecordingError` formatting, manifest validation, index contiguity checks |
| **WASM (B)** | `wasm-pack test --headless --chrome` | web-sys bindings | CI nightly / pre-release | OPFS write/read/delete cycles, MediaRecorder lifecycle, export concat with real blobs, chunk lifecycle (`partial → written → bin`) |
| **E2E (C)** | Playwright + loaded extension | Full stack | Pre-release | Record → Stop → Preview → Download, Kill SW → Recovery, Pause/Resume timing, 30min stress test |

**Rationale:**
- 70%+ of business logic (state machine, serialization, validation) is pure Rust — testable at `cargo test` speed without any browser.
- WASM tests cover the web-sys integration that native host can't simulate (OPFS, MediaRecorder).
- E2E tests validate the extension behaves correctly in a real Chrome environment — critical for MV3-specific behavior (offscreen document, SW lifecycle).

**Affects:** CI pipeline, `Cargo.toml` (dev-dependencies).

## Implementation Patterns & Consistency Rules

### Naming Patterns

| Category | Convention | Example |
|----------|-----------|---------|
| Rust functions/variables | `snake_case` | `start_recording()`, `is_recording` |
| Rust types, enums, traits | `PascalCase` | `RecorderSession`, `ExtensionMessage` |
| Enum variants | `PascalCase` | `StartRecording`, `VideoReady` |
| Module names | `snake_case`, one word | `recorder`, `storage` |
| File names (Rust) | Module name | `recorder.rs`, `storage.rs` |
| Feature flags | Lowercase, one word | `recorder`, `storage`, `export` |
| JS shim files | `kebab-case.js` | `chrome-shim.js`, `ffmpeg.js` |
| Message fields | `snake_case` | `{ mode: RecordingMode, session_id: String }` |

### Structure Patterns

**Module organization:**
- Each functional area gets its own `.rs` file under `src/`.
- Complex modules may have a `mod.rs` + sub-modules (e.g., `recorder/lifecycle.rs`, `recorder/chunk.rs`).
- `#[cfg(test)] mod tests` at the bottom of every production module. Integration tests go in `tests/` at crate root.
- E2E tests go in `tests/e2e/` (Playwright).

**Code placement:**
- `pub(crate)` by default — only expose what other modules genuinely need.
- `pub` only on interfaces consumed across the `ExtensionMessage` boundary or by external shims.
- Cross-module types (`RecordingError`, `SessionState`, `ChunkStatus`) live in the consuming module, not a shared `types.rs`.

### Communication Patterns

**Message protocol:**
- All IPC goes through `ExtensionMessage`. No new message type is added without verifying the enum in `background.rs` first.
- Messages are `serde` serialized. Every variant must derive `Serialize + Deserialize`.
- Messages are one-way (fire-and-forget) or request-response (via a callback channel). No shared mutable state across the IPC boundary.

**Error protocol:**
- Every `RecordingError` variant represents a root cause, not a symptom. `WriteError` not `ChunkTooBig`, `StorageFull`, `PermissionDenied`.
- The background router maps `RecordingError` to `ExtensionMessage::RecordingError { code, details }` — the code is the variant name in `snake_case`.

### Rust-Specific Patterns

- **Derives:** Every data-carrying type derives `Debug, Clone, Serialize, Deserialize`. State enums add `PartialEq, Eq`.
- **`match`:** Exhaustive on all enums. No `_` catch-all unless the variant explicitly documents why it's unreachable (`unreachable!("state invariant: ...")`).
- **`expect`:** Only with an invariant description. Never bare `unwrap()`. Never `unwrap()` on user-supplied data.
- **`use` ordering:** `std` → external crates → `crate::` → `super::`. One blank line between groups.
- **`Result` alias:** Each module defines `type Result<T> = std::result::Result<T, RecordingError>` to avoid importing `Result` everywhere.

### Enforcement

- These patterns are encoded in `CLAUDE.md` for AI agent consumption.
- Code review checklist (human or AI): naming conventions, `pub` scope, error handling, match exhaustiveness, derive completeness.

## Project Structure & Boundaries

### Complete Directory Structure

```
capture-forge/
├── Cargo.toml                          # cdylib — all deps + feature flags
├── oxichrome.config.toml               # Oxichrome build config
├── CLAUDE.md                           # Project guide (this architecture doc referenced)
├── README.md                           # Badges, build, quick start
├── LICENSE                             # MIT
│
├── src/
│   ├── lib.rs                          # #[oxichrome::extension] — entry point
│   │
│   ├── background.rs                   # SW: init, listeners, message router, heartbeat
│   │
│   ├── recorder.rs                     # RECORDER CORE (V0.1)
│   │   ├── mod.rs                      # RecordingSession, SessionState, RecordingMode
│   │   ├── lifecycle.rs                # start/stop/pause/resume/cancel
│   │   ├── chunk.rs                    # Chunk accumulation, header encode, OPFS write
│   │   └── stream.rs                   # Stream acquisition (getDisplayMedia / tabCapture)
│   │
│   ├── storage.rs                      # STORAGE (V0.1)
│   │   ├── mod.rs                      # StorageResult, ChunkStatus enum
│   │   ├── opfs.rs                     # OPFS writer (single track)
│   │   ├── indexdb.rs                  # IndexedDB fallback (V0.2+)
│   │   └── recovery.rs                 # RecoveryManager, IntegrityReport, triple check
│   │
│   ├── export.rs                       # EXPORT (V0.1)
│   │   └── webm.rs                     # Chunk concatenation → WebM blob
│   │
│   ├── error.rs                        # RecordingError enum (thiserror)
│   │
│   ├── messaging.rs                    # ExtensionMessage enum + serde
│   │
│   ├── countdown.rs                    # 3-2-1 countdown overlay (content script)
│   │
│   ├── popup.rs                        # Popup UI (Leptos) — mode selection, mic, start
│   │
│   ├── preview.rs                      # Preview page (Leptos) — video player, actions
│   │
│   ├── permissions.rs                  # Permission request UI
│   │
│   ├── content_script.rs               # EDITOR + OVERLAY (P1, feature-gated)
│   │   ├── mod.rs                      # Shadow DOM injection
│   │   ├── overlay.rs                  # Floating toolbar
│   │   ├── canvas.rs                   # Annotation engine
│   │   │   ├── tools.rs
│   │   │   └── history.rs              # Undo/Redo
│   │   └── camera.rs                   # Camera PiP (P1)
│   │
│   ├── editor.rs                       # EDITOR (P1, feature-gated)
│   │   ├── player.rs                   # Video player
│   │   ├── operations.rs               # Trim/mute/crop (non-destructive)
│   │   └── export.rs                   # Export after edit
│   │
│   ├── camera_page.rs                  # Camera-only recording (P1)
│   ├── region_page.rs                  # Region selection (P1)
│   ├── setup.rs                        # Setup wizard (V0.2+)
│   │
│   └── ai/                             # AI (P2, separate ai.wasm, feature-gated)
│       ├── mod.rs                      # Lazy load entry point
│       ├── transcription.rs            # sherpa-onnx STT
│       ├── captions.rs                 # SRT/VTT generation
│       ├── docgen.rs                   # aisdk LLM tutorial gen
│       └── dom_capture.rs              # HTML snapshots
│
├── js/                                 # JS shims (minimal)
│   ├── chrome_shim.js                  # tabCapture, offscreen APIs
│   ├── ffmpeg.js                       # FFmpeg WASM (P1)
│   └── mediapipe.js                    # MediaPipe (P1)
│
├── static/                             # Static assets
│   ├── icons/                          # Extension icons (16, 48, 128)
│   ├── locales/                        # i18n JSON files
│   │   ├── en.json
│   │   └── fr.json (V0.2+)
│   └── fonts/                          # (empty in V0.1 — system font stack)
│
├── tests/
│   ├── e2e/                            # Playwright E2E tests
│   │   ├── recorder.spec.ts
│   │   ├── recovery.spec.ts
│   │   └── pause-resume.spec.ts
│   └── fixtures/                       # Test recordings, mock manifests
│
├── models/                             # ONNX models (P2)
│   └── zipformer-en/                   # sherpa-onnx default model
│
└── dist/                               # Build output (gitignored)
    └── chromium/
        ├── manifest.json
        ├── background.js
        ├── popup.html / popup.js
        ├── preview.html / preview.js
        ├── wasm/
        │   ├── capture_forge.js        # wasm-bindgen glue
        │   ├── capture_forge_bg.wasm   # core.wasm
        │   └── ai.wasm                 # ai.wasm (P2+)
        ├── js/                          # Copied shims
        └── static/                      # Copied assets
```

### Architectural Boundaries

| Boundary | Type | Mechanism |
|----------|------|-----------|
| UI ↔ Core | Message-passing | `ExtensionMessage` via background router |
| Core ↔ Storage | Direct Rust calls | `storage::opfs::write_chunk()` etc. |
| Core ↔ Export | Direct Rust calls | `export::webm::concat()` |
| Core ↔ JS shims | wasm-bindgen interop | `#[wasm_bindgen]` extern functions |
| Core ↔ AI (P2) | Lazy WASM load | `ai.wasm` loaded on-demand, ipc via messages |
| Content script ↔ Core | `chrome.runtime.sendMessage` | Serialized ExtensionMessage |

### Requirements to Structure Mapping

| PRD Section | Module | Key Files |
|-------------|--------|-----------|
| §6 (Recorder Core V0.1) | `recorder.rs`, `storage.rs`, `export.rs` | lifecycle, chunk, stream, opfs, recovery, webm |
| §6.4 (UI States) | `popup.rs`, `preview.rs`, `countdown.rs` | 9 states mapped to Leptos components |
| §6.5 (Messages) | `messaging.rs` | ExtensionMessage enum |
| §6.6 (Storage) | `storage.rs` | opfs.rs, recovery.rs, chunk.rs |
| §5.3 (Oxichrome exit) | `background.rs` | Inner wrappers via traits |
| §7 (Editor P1) | `content_script.rs`, `editor.rs` | Feature-gated `#![cfg(feature = "editor")]` |
| §8 (AI P2) | `ai/` | Separate WASM, feature-gated `#![cfg(feature = "stt")]` |

## Architecture Validation Results

### Coherence Validation ✅

**Decision Compatibility:**
All technology choices are compatible. Oxichrome v0.2 + Leptos 0.7 + web-sys v0.3 + opfs crate form a consistent Rust/WASM ecosystem targeting the same `wasm32-unknown-unknown` target. The hybrid message-passing pattern (direct calls within core, IPC to UI) aligns with the single-WASM constraint. The 2-WASM strategy (core + AI) preserves cold-start performance while keeping the build pipeline simple for V0.1.

**Pattern Consistency:**
Implementation patterns (naming, error handling, test tiers) are consistent across all modules. The `RecordingError` + `ExtensionMessage` protocol provides a uniform error surface. Feature gates isolate P1/P2 code without affecting module structure.

**Structure Alignment:**
The project tree maps one-to-one to the module hierarchy in `docs/architect.md` and follows the three-sub-product decomposition from the PRD. No architectural decision contradicts the structure, and vice versa.

### Requirements Coverage Validation ✅

**Recorder Core (V0.1):**
| REC-ID | Requirement | Architectural Support |
|--------|-------------|----------------------|
| REC-01 | Record full screen | `recorder/stream.rs` — `getDisplayMedia` |
| REC-02 | Record tab | `recorder/stream.rs` — `tabCapture` via JS shim |
| REC-03 | Microphone | `recorder/stream.rs` — AudioContext mixer |
| REC-04 | Pause/Resume | `recorder/lifecycle.rs` — state transitions |
| REC-05 | Stop + preview | `recorder/lifecycle.rs` + `preview.rs` |
| REC-06 | Cancel | `recorder/lifecycle.rs` — returns to Idle |
| REC-07 | Countdown | `countdown.rs` — 3-2-1 overlay |
| REC-08 | WebM export | `export/webm.rs` — chunk concat |
| REC-09 | OPFS storage | `storage/opfs.rs` — chunk lifecycle |
| REC-10 | Crash recovery | `storage/recovery.rs` — triple verification + toast |

**Non-Functional Requirements:**
| NFR | Architectural Coverage |
|-----|----------------------|
| ≥25 FPS @ 1080p | Chunk strategy (10s interval keeps heap low), direct web-sys MediaRecorder |
| <500MB RAM for 1h | OPFS chunking + no in-memory accumulation |
| <1s WASM load | 2-WASM split, `wasm-opt -Oz`, brotli |
| WCAG 2.1 AA | `aria-live` in EXPERIENCE.md, Leptos CSR |
| Zero telemetry | No network in architecture, no analytics SDK |
| Dark/Light system | CSS `prefers-color-scheme` throughout |

### Implementation Readiness Validation ✅

**Decision Completeness:**
All critical decisions documented: module communication (hybrid), WASM strategy (2-binary), error handling (`thiserror` + panic hook), chunk format (32-byte header), keepalive (ping/pong), test strategy (3-tier). Technology versions are locked per PRD §5.1.

**Structure Completeness:**
Complete project tree defined for V0.1. Every module has a mapped location. JS shims, static assets, test directories, and build output paths are specified. P1/P2 modules are present but feature-gated.

**Pattern Completeness:**
Naming, communication, error, and Rust-specific patterns are defined. The `ExtensionMessage` and `RecordingError` protocols are specified. Test strategy covers unit through E2E.

### Gap Analysis

| Gap | Severity | Impact | When to Address |
|-----|----------|--------|-----------------|
| Firefox portability (offscreen → tab adaptation) | Medium | P1 blocker | When Firefox P1 begins |
| Editor-specific architecture (trim/mute/crop internals) | Medium | P1 blocker | When Editor P1 begins |
| Privacy model architecture (data flow diagram) | Low | V0.1 polish | Before CWS submission |
| AI module lazy-loading mechanism (WASM fetch, init) | Low | P2 enabler | When P2 begins |
| 3-WASM → 2-WASM migration path in build pipeline | Low | Developer UX | When P2 begins |

No critical gaps. All V0.1 requirements are fully covered.

### Architecture Completeness Checklist

**Requirements Analysis**
- [x] Project context thoroughly analyzed
- [x] Scale and complexity assessed
- [x] Technical constraints identified
- [x] Cross-cutting concerns mapped

**Architectural Decisions**
- [x] Critical decisions documented with versions
- [x] Technology stack fully specified
- [x] Integration patterns defined
- [x] Performance considerations addressed

**Implementation Patterns**
- [x] Naming conventions established
- [x] Structure patterns defined
- [x] Communication patterns specified
- [x] Process patterns documented

**Project Structure**
- [x] Complete directory structure defined
- [x] Component boundaries established
- [x] Integration points mapped
- [x] Requirements to structure mapping complete

### Architecture Readiness Assessment

**Overall Status:** READY FOR IMPLEMENTATION (V0.1)

**Confidence Level:** High — all 16 checklist items verified. No critical gaps. All V0.1 decisions are documented with concrete mechanisms.

**Key Strengths:**
- Clean separation of concerns (3 sub-products, feature-gated)
- Hybrid message-passing keeps UI decoupled from core
- 2-WASM strategy anticipates AI growth without complexity today
- Three-tier test strategy matches module characteristics
- Self-describing chunk format enables robust crash recovery
- Exit strategy on Oxichrome reduces framework risk

**Areas for Future Enhancement:**
- Firefox module abstraction (offscreen → tab pattern)
- Editor-specific state machine (P1)
- AI WASM lazy-loading orchestrator (P2)
- Formal data flow diagram for privacy audit (pre-CWS)

### Implementation Handoff

**AI Agent Guidelines:**
- Follow architectural decisions exactly as documented
- Use implementation patterns consistently across all modules
- Respect module boundaries — `pub(crate)` by default
- All errors return `RecordingError`, never `unwrap()`
- All messages go through `ExtensionMessage` enum
- Tests at the appropriate tier (unit → WASM → E2E)
- Feature-gate P1/P2 code — never compile it into default V0.1 binary

**First Implementation Priority:**
`recorder.rs` state machine (Idle → Starting → Recording → Paused → Stopping → Preview → Error → CrashRecovery) with native `cargo test` coverage. This is the foundation every other module depends on.
