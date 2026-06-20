# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Capture Forge** — a browser extension (Chrome first, Firefox planned) for screen recording and video editing, written in Rust and compiled to WebAssembly via the [oxichrome](https://crates.io/crates/oxichrome) framework.

Phased roadmap:
1. **Recorder Core** (P0, V0.1) — screen/tab capture + mic + pause/resume/stop + WebM export + OPFS storage + crash recovery
2. **Editor + Overlay** (P1, V0.5) — non-destructive trim/mute/crop, toolbar, canvas annotations, camera PiP, Firefox support, MP4/GIF export
3. **AI & Semantic** (P2, V2.0+) — local STT (sherpa-onnx/WASM), optional LLM, DOM capture, Audience Lenses

**Key decisions:** MIT license, GitHub + Chrome Web Store distribution, Firefox in P1 not P0, local-first/privacy-first positioning.

**Current state:** Stories 1.1–1.4 complete. Next: Story 1.5 (WebM export pipeline).

**Canonical technical reference:** `_bmad-output/planning-artifacts/architecture.md`
**Product spec:** `_bmad-output/planning-artifacts/prds/prd-capture-forge-2026-06-19/prd.md`
**Sprint tracking:** `_bmad-output/implementation-artifacts/sprint-status.yaml`

## Prerequisites

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
# Optional: only when regenerating manifest/JS shims
cargo install cargo-oxichrome
```

## Build Commands

```bash
# Fast compile-time validation (wasm32 target not needed)
cargo check

# Unit tests (native host — pure Rust, no browser required)
cargo test
cargo test -- <test_name>                          # Single test by name
cargo test -- chunk                                # All chunk tests
cargo test --lib -- tests::state_machine::test_happy_path_full_cycle  # Full path

# WASM tests (requires Chrome headless)
wasm-pack test --headless --chrome

# Recompile Rust → WASM
wasm-pack build --target web

# Full oxichrome pipeline — regenerates manifest.json, background.js, JS shims
cargo oxichrome build              # Debug
cargo oxichrome build --release    # Optimised + wasm-opt -Oz

# Feature-gated builds
cargo oxichrome build --no-default-features --features recorder,storage,export
cargo oxichrome build --features overlay,editor,camera

# E2E tests (Playwright with loaded extension — pre-release only)
npx playwright test tests/e2e/
```

### Three-tier test strategy

| Tier | Command | Scope | Frequency |
|------|---------|-------|-----------|
| Unit | `cargo test` | State machine, serde roundtrip, chunk header, checksum, lifecycle transitions | Every commit |
| WASM | `wasm-pack test --headless --chrome` | OPFS R/W, MediaRecorder lifecycle | CI nightly |
| E2E | `npx playwright test` | Record→Stop→Download, Kill SW→Recovery, Pause/Resume | Pre-release |

### Current test suites (110+ tests)

- `error.rs` — Display format, Error trait, serde roundtrip for all 8 variants
- `recorder.rs` — SessionState transitions (9 states, valid + invalid), RecordingSession construction
- `stream.rs` — StreamAcquisitionService config, mic handling
- `lifecycle.rs` — RecordingLifecycle start/stop/pause/resume/cancel, MediaRecorder creation
- `chunk.rs` — 25+ tests: header encode/decode, checksum (XXH3), manifest, MockChunkStorage, ChunkWriter lifecycle (write_blob, commit, idempotency, naming, empty rejection, overflow, invalid timestamps, storage integration)

### Build output

```
dist/chromium/
  manifest.json           # Manifest V3 (edit when permissions change)
  background.js           # ES module service worker
  wasm/
    capture_forge.js      # wasm-bindgen JS glue
    capture_forge_bg.wasm # WebAssembly binary
```

Edit `manifest.json` and `background.js` directly when adding permissions or entry points. When changing extension name/version, update both `src/lib.rs` and `manifest.json`.

## Architecture

### Data flow

```
Popup/UI (Leptos CSR)
    │ ExtensionMessage (serde, via background router)
    ▼
background.rs (service worker)
    │ dispatches to core modules
    ▼
recorder.rs ──→ chunk.rs ──→ export.rs
    │              │              │
    │         OPFS (chunks)   WebM blob
    │              │
    ▼              ▼
Offscreen doc    RecoveryManager (triple verification)
(keepalive,
MediaRecorder)
```

### Module layout (current — 7 modules, all implemented)

```
src/
├── lib.rs              # Entry point. Module declarations, #[oxichrome::extension], panic hook, SESSION global
├── error.rs            # RecordingError enum (8 variants, thiserror), pub(crate) type Result<T>
├── recorder.rs         # SessionState (9 states), RecordingSession, transition() with match matrix
├── messaging.rs        # ExtensionMessage (11 variants), RecordingMode, is_keepalive()
├── stream.rs           # StreamAcquisitionService, AcquiredStream, mix_audio (AudioContext)
├── lifecycle.rs        # RecordingLifecycle — start/stop/pause/resume/cancel, MediaRecorder create
├── chunk.rs            # ChunkHeader (32-byte binary), ChunkManifest, ChunkWriter, ChunkStorage trait + MockChunkStorage
```

### Module responsibilities

| Module | Key types | Depends on |
|--------|-----------|------------|
| `error.rs` | `RecordingError`, `Result<T>` | thiserror, serde |
| `recorder.rs` | `SessionState` (9 states), `RecordingSession` | error |
| `messaging.rs` | `ExtensionMessage` (11 variants), `RecordingMode` | serde |
| `stream.rs` | `StreamAcquisitionService`, `AcquiredStream`, `StreamGuard` | error, messaging, web-sys (MediaDevices) |
| `lifecycle.rs` | `RecordingLifecycle`, `LifecycleState`, `ChunkHandler` | error, stream, web-sys (MediaRecorder, Blob) |
| `chunk.rs` | `ChunkHeader`, `ChunkManifest(Entry)`, `ChunkWriter`, `ChunkStorage` trait, `MockChunkStorage` | error, xxhash-rust |

### RecordingSession (global state machine)

Wrapped in `OnceLock<Mutex<RecordingSession>>` accessible from lib.rs as `SESSION`. The panic hook uses it to transition to `Error` state.

**9 states:** `Idle`, `Starting`, `Countdown`, `Recording`, `Paused`, `Stopping`, `Preview`, `Error`, `CrashRecovery`

Valid transitions:
```
Idle → Starting | CrashRecovery
Starting → Countdown | Error
Countdown → Recording | Idle | Error
Recording → Paused | Stopping | Error | Idle
Paused → Recording | Stopping | Error
Stopping → Preview | Error
Preview → Idle
Error → Idle
CrashRecovery → Preview | Idle | Error
```

### Chunk binary format (32-byte header)

| Offset | Size | Field | Value |
|--------|------|-------|-------|
| 0–3 | 4 | Magic | `0x43464348` ("CFCH") |
| 4 | 1 | Version | `0x01` |
| 5–8 | 4 | Chunk index | `u32` LE |
| 9–16 | 8 | Timestamp ms | `f64` LE |
| 17–24 | 8 | Payload size | `u64` LE |
| 25–28 | 4 | XXH3 checksum | `u32` LE |
| 29–31 | 3 | Reserved | zero |

Chunk lifecycle: `.partial → .written → .bin`. Checksum is lower 32 bits of `xxh3_64`.

### Key architectural decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Module communication | Hybrid — direct Rust calls within core, `ExtensionMessage` IPC to UI | Performance where it matters, decoupling where it counts |
| WASM strategy | 2 binaries: `core.wasm` + `ai.wasm` (lazy-loaded, P2+) | AI cold-start never penalizes recording-only users |
| Error handling | `thiserror` + `RecordingError` + `panic::set_hook()` override | No `unwrap()` anywhere. Panic in WASM kills the extension — prevented via custom hook |
| State machine | `RecordingSession::transition()` with explicit match matrix + `AtomicBool` re-entrancy guard | Invalid transitions produce `RecordingError::StateViolation`; session state unchanged |
| Chunk format | 32-byte header + raw MediaRecorder blob | Self-describing — recovery works without manifest file |
| Chunk storage | `ChunkStorage` trait with mock backend for native tests | Full OPFS is WASM-only; native tests use `MockChunkStorage` (in-memory `Vec`) |
| Heartbeat | Ping/pong every 20s from offscreen doc to SW | Chrome MV3 kills SW after ~30s idle |

### Critical implementation patterns

- **Every public function returns `Result<T, RecordingError>`**. No bare `unwrap()` — use `expect("invariant: ...")` with a message.
- **Exhaustive match** on all enums. No `_` catch-all without `unreachable!("reason")`.
- **Derives:** Every data-carrying type: `#[derive(Debug, Clone, Serialize, Deserialize)]`. State enums add `PartialEq, Eq`.
- **`pub` discipline:** `pub(crate)` by default. `pub` only across the message boundary or for external shims.
- **`use crate::error::Result;`** in each module (not redefining the alias).
- **`RecordingSession`** wrapped in a `OnceLock<Mutex<...>>` global in `lib.rs` for cross-module access.
- **Panic hook** uses `console.error()` shim, preserves the previous hook, has an `AtomicBool` re-entrancy guard.
- **Reorder test assertion order:** `assert_eq!(expected, actual)` — expected value first.
- **Feature gates:** V0.1 default = `recorder, storage, export`. P1+ features must be non-default.

## Current Dependencies

```toml
[dependencies]
oxichrome = "0.1"                                # Proc macros + Chrome API wrappers
wasm-bindgen = "0.2"                             # Rust↔JS interop
serde = { version = "1", features = ["derive"] } # Serialization
thiserror = "2"                                  # Error derive
web-sys = "0.3"                                  # Browser APIs (MediaRecorder, MediaStream, AudioContext, etc.)
js-sys = "0.3"                                   # JS types (Date, Array, etc.)
wasm-bindgen-futures = "0.4"                     # Future→Promise conversion
xxhash-rust = { version = "0.8", features = ["xxh3"] }  # Chunk checksums

[dev-dependencies]
serde_json = "1"
wasm-bindgen-test = "0.3"
```

## Development Workflow

1. Edit Rust code under `src/`
2. `cargo check` for compile-time validation
3. `cargo test` for native unit tests
4. `wasm-pack build --target web` for WASM compilation
5. Load `dist/chromium/` as unpacked extension at `chrome://extensions/` (Developer mode on)
6. Inspect service worker via Extensions → Capture Forge → Service Worker → Console
7. When changing permissions, name, or version: update **both** `src/lib.rs` and `dist/chromium/manifest.json`

Popups and options pages: add `#[oxichrome::popup]` or `#[oxichrome::options_page]` to a Leptos component, then run `cargo oxichrome build` (not `wasm-pack` alone — the CLI generates the HTML/JS shims).

## BMAD Methodology

This project uses structured BMAD workflows for development (skills in `.claude/skills/`, config in `_bmad/`, artifacts in `_bmad-output/`):

| Workflow | Skill | Purpose |
|----------|-------|---------|
| Sprint Planning | `bmad-sprint-planning` | Update sprint status, plan stories |
| Story Creation | `bmad-create-story` | Create story file from epics |
| Story Implementation | `bmad-dev-story` | Implement a story following red-green-refactor |
| Code Review | `bmad-code-review` | Parallel adversarial review layers + triage |

Sprint status tracked in `_bmad-output/implementation-artifacts/sprint-status.yaml`. Story files live in `_bmad-output/implementation-artifacts/` as `{n}-{n}-{slug}.md`.

## Project Documentation Index

| File | Purpose |
|---|---|
| `_bmad-output/planning-artifacts/architecture.md` | **Canonical architecture — all decisions, patterns, project tree** |
| `_bmad-output/planning-artifacts/epics.md` | Epic breakdown, requirements inventory, FR coverage map, all stories |
| `_bmad-output/planning-artifacts/prds/prd-capture-forge-2026-06-19/prd.md` | PRD v1.0 — user stories, acceptance criteria, message protocol, NFRs |
| `_bmad-output/planning-artifacts/ux-designs/ux-capture-forge-2026-06-19/DESIGN.md` | Visual identity — colors, typography, spacing, components |
| `_bmad-output/planning-artifacts/ux-designs/ux-capture-forge-2026-06-19/EXPERIENCE.md` | UX — IA, states, flows, interactions, accessibility |
| `_bmad-output/implementation-artifacts/sprint-status.yaml` | Sprint tracking — epic and story status |
| `_bmad-output/implementation-artifacts/deferred-work.md` | Items deferred from code reviews |
| `docs/architect.md` | Technical architecture reference |
| `docs/product-brief.md` | Product vision, positioning, scope |
| `docs/prd.md` | Pre-finalization PRD |

## Permissions

Currently declared: `["storage", "unlimitedStorage", "desktopCapture", "tabCapture", "downloads"]`. Add permissions to both `#[oxichrome::extension(...)]` in `src/lib.rs` **and** `dist/chromium/manifest.json`.
