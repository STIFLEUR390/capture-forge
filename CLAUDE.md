# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Capture Forge** — a browser extension (Chrome first, Firefox planned) for screen recording and video editing, written in Rust and compiled to WebAssembly via the [oxichrome](https://crates.io/crates/oxichrome) framework.

Phased roadmap:
1. **Recorder Core** (P0, V0.1) — screen/tab capture + mic + pause/resume/stop + WebM export + OPFS storage + crash recovery
2. **Editor + Overlay** (P1, V0.5) — non-destructive trim/mute/crop, toolbar, canvas annotations, camera PiP, Firefox support, MP4/GIF export
3. **AI & Semantic** (P2, V2.0+) — local STT (sherpa-onnx/WASM), optional LLM, DOM capture, Audience Lenses

**Key decisions:** MIT license, GitHub + Chrome Web Store distribution, Firefox in P1 not P0, local-first/privacy-first positioning.

**Current state:** PRD v1.0, UX spines, and architecture are finalized. Story 1.1 (Error System & State Machine Foundation) is `done`. The next implementation target is Story 1.2 (Stream Acquisition).

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
cargo test -- <test_name>
cargo test --lib -- tests::state_machine::test_happy_path_full_cycle  # Example

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
| Unit | `cargo test` | State machine, serde roundtrip, chunk header, validation | Every commit |
| WASM | `wasm-pack test --headless --chrome` | OPFS R/W, MediaRecorder lifecycle | CI nightly |
| E2E | `npx playwright test` | Record→Stop→Download, Kill SW→Recovery, Pause/Resume | Pre-release |

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
recorder.rs ──→ storage.rs ──→ export.rs
    │              │              │
    │         OPFS (chunks)   WebM blob
    │              │
    ▼              ▼
Offscreen doc    RecoveryManager (triple verification)
(keepalive,
MediaRecorder)
```

### Module layout (current)

```
src/
├── lib.rs                  # Entry point. Module declarations, panic hook, global SESSION
├── error.rs                # RecordingError enum (thiserror), Result<T> alias
├── recorder.rs             # SessionState (9 states), RecordingSession, transition()
├── messaging.rs            # ExtensionMessage (11 variants), RecordingMode
├── background.rs           # (planned) Service worker, listeners, message router
├── storage.rs/             # (planned) OPFS writer + IndexedDB fallback
├── export.rs/              # (planned) WebM concatenation
├── popup.rs                # (planned) Mode selection UI
├── preview.rs              # (planned) Video player + actions
└── ...                     # P1+ modules (feature-gated)
```

### Key architectural decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Module communication | Hybrid — direct Rust calls within core, `ExtensionMessage` IPC to UI | Performance where it matters, decoupling where it counts |
| WASM strategy | 2 binaries: `core.wasm` + `ai.wasm` (lazy-loaded, P2+) | AI cold-start never penalizes recording-only users |
| Error handling | `thiserror` + `RecordingError` + `panic::set_hook()` override | No `unwrap()` anywhere. Panic in WASM kills the extension — prevented via custom hook |
| State machine | `RecordingSession::transition()` with explicit match matrix + `AtomicBool` re-entrancy guard | Invalid transitions produce `RecordingError::StateViolation`; session state unchanged |
| Chunk format | 32-byte header (magic + index + timestamp + size + XXH3) + raw MediaRecorder blob | Self-describing — recovery works without manifest file |
| Heartbeat | Ping/pong every 20s from offscreen doc to SW | Chrome MV3 kills SW after ~30s idle |

### Critical implementation patterns

- **Every public function returns `Result<T, RecordingError>`**. No bare `unwrap()` — use `expect("invariant: ...")` with a message.
- **Exhaustive match** on all enums. No `_` catch-all without `unreachable!("reason")`.
- **Derives:** Every data-carrying type: `#[derive(Debug, Clone, Serialize, Deserialize)]`. State enums add `PartialEq, Eq`.
- **`pub` discipline:** `pub(crate)` by default. `pub` only across the message boundary or for external shims.
- **Module `Result` alias:** Each module defines `type Result<T> = std::result::Result<T, RecordingError>`.
- **`RecordingSession`** is wrapped in a `OnceLock<Mutex<...>>` global in `lib.rs` for cross-module access (especially the panic hook).
- **Panic hook** uses `console.error()` (via `#[wasm_bindgen] extern shim), preserves and re-invokes the previous hook, and has an `AtomicBool` re-entrancy guard.
- **Feature gates:** V0.1 default = `recorder, storage, export`. P1+ features must be non-default. Never compile P2 features into the default binary.

### State machine transitions (V0.1)

All 9 states: `Idle`, `Starting`, `Countdown`, `Recording`, `Paused`, `Stopping`, `Preview`, `Error`, `CrashRecovery`.

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

## Current Dependencies

```toml
[dependencies]
oxichrome = "0.1"             # Proc macros + Chrome API wrappers
wasm-bindgen = "0.2"          # Rust↔JS interop (also provides #[wasm_bindgen] for extern shims)
serde = { version = "1", features = ["derive"] }
thiserror = "2"

[dev-dependencies]
serde_json = "1"
```

Additional deps (`leptos`, `web-sys`, `opfs`, `indexed_db_futures`, `sherpa-onnx`, `aisdk`) are declared in the architecture docs but not yet in `Cargo.toml` — they will be added as each story requires them.

## Development Workflow

1. Edit Rust code under `src/`
2. `wasm-pack build --target web`
3. Load `dist/chromium/` as unpacked extension at `chrome://extensions/` (Developer mode on)
4. Inspect service worker via Extensions → Capture Forge → Service Worker → Console
5. When changing permissions, name, or version: update **both** `src/lib.rs` and `dist/chromium/manifest.json`

Popups and options pages: add `#[oxichrome::popup]` or `#[oxichrome::options_page]` to a Leptos component, then run `cargo oxichrome build` (not `wasm-pack` alone — the CLI generates the HTML/JS shims).

## BMAD Methodology

This project uses structured BMAD workflows for development (skills in `.claude/skills/`, config in `_bmad/`, artifacts in `_bmad-output/`):

- **Sprint Planning** → `bmad-sprint-planning`
- **Story Creation** → `bmad-create-story`
- **Story Implementation** → `bmad-dev-story`
- **Code Review** → `bmad-code-review`

Sprint status tracked in `_bmad-output/implementation-artifacts/sprint-status.yaml`. Story files live in `_bmad-output/implementation-artifacts/stories/`.

## Project Documentation Index

| File | Purpose |
|---|---|
| `_bmad-output/planning-artifacts/architecture.md` | **Canonical architecture — all decisions, patterns, project tree** |
| `_bmad-output/planning-artifacts/epics.md` | Epic breakdown, requirements inventory, FR coverage map, all stories |
| `_bmad-output/planning-artifacts/prds/prd-capture-forge-2026-06-19/prd.md` | PRD v1.0 — user stories, acceptance criteria, message protocol, NFRs |
| `_bmad-output/planning-artifacts/ux-designs/ux-capture-forge-2026-06-19/DESIGN.md` | Visual identity — colors, typography, spacing, components |
| `_bmad-output/planning-artifacts/ux-designs/ux-capture-forge-2026-06-19/EXPERIENCE.md` | UX — IA, states, flows, interactions, accessibility |
| `_bmad-output/implementation-artifacts/1-1-error-system-state-machine-foundation.md` | Story 1.1 — completed implementation |
| `_bmad-output/implementation-artifacts/sprint-status.yaml` | Sprint tracking — epic and story status |
| `docs/architect.md` | Technical architecture reference |
| `docs/product-brief.md` | Product vision, positioning, scope |
| `docs/prd.md` | Pre-finalization PRD |

## Permissions

Currently declared: `"storage"`. Future: `unlimitedStorage`, `desktopCapture`, `tabCapture`, `downloads`. Add permissions to both `#[oxichrome::extension(...)]` in `src/lib.rs` **and** `dist/chromium/manifest.json`.
