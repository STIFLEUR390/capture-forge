---
baseline_commit: 4f424600bb246fb81a2d1a4b5d8122af89686e46
---

# Story 1.1: Error System & State Machine Foundation

Status: done

## Story

As a developer embedding the recorder,
I want a well-defined error system, state machine, and message protocol,
So that all recording operations have consistent error handling, valid state transitions are enforced, and UI surfaces can communicate with the core.

## Acceptance Criteria

### AC1: RecordingError enum

**Given** the project has no error infrastructure yet
**When** a `RecordingError` enum is defined using `thiserror` with stable error codes
**Then** the enum includes at minimum these variants: `StreamAcquisitionFailed`, `MediaRecorderError`, `WriteError`, `ExportError`, `StateViolation`, `Panic`, `EmptySession`, `Unknown`
**And** each variant carries a `details: String` field for context
**And** `std::fmt::Display` is derived (via `thiserror`) — each variant outputs `"{variant_name}: {details}"`
**And** `RecordingError` implements `std::error::Error`

### AC2: Module-level Result alias

**Given** the `RecordingError` type is defined
**When** each module in the crate needs to return `Result`
**Then** every module defines `type Result<T> = std::result::Result<T, RecordingError>` at the top
**And** every public function in core modules returns `Result<T, RecordingError>`

### AC3: SessionState state machine

**Given** the `SessionState` enum defines all V0.1 states (`Idle`, `Starting`, `Countdown`, `Recording`, `Paused`, `Stopping`, `Preview`, `Error`, `CrashRecovery`)
**When** the `RecordingSession` struct is implemented with a `transition()` method
**Then** valid transitions are enforced:
- `Idle → Starting`
- `Starting → Countdown | Error`
- `Countdown → Recording | Idle` (Escape cancels)
- `Recording → Paused | Stopping`
- `Paused → Recording | Stopping`
- `Stopping → Preview | Error`
- `Preview → Idle`
- `Error → Idle`
- `CrashRecovery → Preview | Idle`
**And** invalid transitions return `Err(RecordingError::StateViolation { details: "Cannot transition from X to Y" })`
**And** the session state remains unchanged on invalid transition

### AC4: ExtensionMessage IPC protocol

**Given** the `ExtensionMessage` enum defines all V0.1 IPC variants
**When** every variant derives `Debug, Clone, Serialize, Deserialize`
**Then** these variants exist with their fields:
- `StartRecording { mode: RecordingMode }`
- `StopRecording`
- `PauseRecording`
- `ResumeRecording`
- `CancelRecording`
- `VideoReady { session_id: String }`
- `RecordingError { code: String, details: String }`
- `KeepalivePing`
- `KeepalivePong`
- `GetStreamingData`
- `ApplyStreamingData { data: String }`
**And** `RecordingMode` is a separate enum with variants `FullScreen` and `Tab`, deriving `Debug, Clone, Serialize, Deserialize, PartialEq`
**And** messages round-trip through serde JSON without data loss, verified by `cargo test`

### AC5: Panic hook override

**Given** a panic hook override installed at extension init
**When** a panic occurs in any core module
**Then** the hook logs the panic message via `console.error`, transitions the session to `Error` state, and does not abort the WASM instance
**And** the hook is installed in the `#[oxichrome::background]` init function

### AC6: Test coverage

**Given** the state machine module, error module, and messaging module all have `#[cfg(test)] mod tests` blocks
**When** `cargo test` is executed
**Then** all valid state transitions are verified (happy path: `Idle→Starting→Countdown→Recording→Paused→Recording→Stopping→Preview→Idle`, plus all error paths)
**And** invalid transitions return correct errors (double-start, double-stop, pause-in-idle, resume-in-idle, start-during-recording, cancel-in-idle)
**And** all serde roundtrips pass for every `ExtensionMessage` variant and `RecordingMode`
**And** `RecordingError` Display output is correct for each variant

## Tasks / Subtasks

- [x] Task 1: Create `src/error.rs` — RecordingError enum (AC1, AC2)
  - [x] 1.1 Define `RecordingError` with `thiserror` derive and all V0.1 variants
  - [x] 1.2 Add `type Result<T>` alias export
  - [x] 1.3 Add `#[cfg(test)] mod tests` verifying Display output
- [x] Task 2: Create `src/recorder.rs` — SessionState + RecordingSession (AC3)
  - [x] 2.1 Define `SessionState` enum with all 9 V0.1 states
  - [x] 2.2 Define `RecordingSession` struct with `state: SessionState` and `transition()` method
  - [x] 2.3 Implement valid transition matrix — return `Err(StateViolation)` on invalid moves
  - [x] 2.4 Add `#[cfg(test)] mod tests` covering all valid and invalid transitions
- [x] Task 3: Create `src/messaging.rs` — ExtensionMessage enum (AC4)
  - [x] 3.1 Define `ExtensionMessage` with all V0.1 variants
  - [x] 3.2 Define `RecordingMode` enum (`FullScreen`, `Tab`)
  - [x] 3.3 Ensure all types derive `Debug, Clone, Serialize, Deserialize`
  - [x] 3.4 Add `#[cfg(test)] mod tests` verifying serde roundtrips
- [x] Task 4: Update `src/lib.rs` — module declarations + panic hook (AC5)
  - [x] 4.1 Add `mod error;`, `mod recorder;`, `mod messaging;` declarations
  - [x] 4.2 Install `panic::set_hook()` override in background init
  - [x] 4.3 Verify `#[oxichrome::extension]` permissions include `["storage"]`
- [x] Task 5: Update `Cargo.toml` — add `thiserror` dependency (AC1)
  - [x] 5.1 Add `thiserror = "2"` to `[dependencies]` with `derive` feature
- [x] Task 6: Verify compilation and tests
  - [x] 6.1 Run `cargo check` — must compile cleanly
  - [x] 6.2 Run `cargo test` — all tests pass
  - [x] 6.3 Confirm no bare `unwrap()` anywhere in new code

## Dev Notes

### Architecture compliance (mandatory)

1. **Error handling**: Every public function returns `Result<T, RecordingError>`. No bare `unwrap()` anywhere — use `expect("invariant: ...")` with a descriptive invariant message. See [Architecture: Error Handling in WASM] for full rationale.

2. **Exhaustive match**: Always exhaustive on `SessionState` and `ExtensionMessage` enums. No `_` catch-all without `unreachable!("reason")`.

3. **Derives**: Every data-carrying type: `#[derive(Debug, Clone, Serialize, Deserialize)]`. `SessionState` adds `PartialEq, Eq`.

4. **`pub` discipline**: `pub(crate)` by default. `pub` only for interfaces consumed across the message boundary (`RecordingError` must be `pub` for cross-module use; `ExtensionMessage` must be `pub` for serde IPC).

5. **`type Result<T>` alias**: Each module defines `type Result<T> = std::result::Result<T, RecordingError>` to avoid importing `Result` everywhere.

6. **Feature gates**: All code in this story goes in the default feature set (no feature gating needed — this is V0.1 foundation).

7. **No `use` of `thiserror::Error` without `derive`**: `thiserror` v2 requires the `derive` feature. The `derive` macro is on by default but should be explicit in `Cargo.toml`.

### Source tree components

#### Files to CREATE:

| File | Purpose |
|------|---------|
| `src/error.rs` | `RecordingError` enum with `thiserror` + `type Result<T>` alias |
| `src/recorder.rs` | `SessionState` enum + `RecordingSession` struct + `transition()` |
| `src/messaging.rs` | `ExtensionMessage` enum + `RecordingMode` enum |

#### Files to UPDATE:

| File | What changes |
|------|-------------|
| `src/lib.rs` | Add `mod error; mod recorder; mod messaging;` + install panic hook in background init |
| `Cargo.toml` | Add `thiserror = "2"` to dependencies |

### Testing standards

- **All modules** must have `#[cfg(test)] mod tests` blocks with unit tests.
- **State machine tests** verify every valid and invalid transition. Use `match` on `SessionState` to confirm state after transition.
- **Serde tests** verify every `ExtensionMessage` variant roundtrips through `serde_json::to_string`/`from_str`.
- **Error tests** verify `RecordingError::Display` output format.
- Test at native `cargo test` speed — no browser needed for this story's tests.

### Current project state

Only `src/lib.rs` exists with a basic oxichrome scaffold:
```rust
#[oxichrome::extension(name = "Capture Forge", version = "0.1.0", permissions = ["storage"])]
struct Extension;

#[oxichrome::background]
async fn start() {
    oxichrome::log!("Capture Forge started!");
}

#[oxichrome::on(runtime::on_installed)]
async fn handle_install(details: oxichrome::__private::wasm_bindgen::JsValue) {
    oxichrome::log!("Capture Forge installed: {:?}", details);
}
```

`Cargo.toml` has deps: `oxichrome 0.1`, `wasm-bindgen 0.2`, `serde 1` (with `derive`).

### Implementation details

#### RecordingError enum structure

```rust
use thiserror::Error;

#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum RecordingError {
    #[error("Stream acquisition failed: {details}")]
    StreamAcquisitionFailed { details: String },

    #[error("MediaRecorder error: {details}")]
    MediaRecorderError { details: String },

    #[error("Write error: {details}")]
    WriteError { details: String },

    #[error("Export error: {details}")]
    ExportError { details: String },

    #[error("State violation: {details}")]
    StateViolation { details: String },

    #[error("Panic: {details}")]
    Panic { details: String },

    #[error("Empty session: {details}")]
    EmptySession { details: String },

    #[error("Unknown error: {details}")]
    Unknown { details: String },
}

pub(crate) type Result<T> = std::result::Result<T, RecordingError>;
```

#### SessionState transition matrix

The 9 V0.1 states map into these valid transitions:

```
                ┌──────────────────────────────┐
                │            Idle               │
                ├───────┬──────────┬────────────┤
                │       │          │            │
                ▼       ▼          ▼            ▼
            Starting  CrashRecovery            (terminal)
                │       │    ┌─────┴─────┐
                │       │    │           │
                ▼       ▼    ▼           ▼
          ┌────────┐ Preview          Idle
     ┌───►Error   │
     │   └────┬───┘
     │        │
     │        ▼
     │      Idle
     │
Starting ──► Countdown ──► Recording ──► Stopping ──► Preview ──► Idle
                  │              │  │          │            │
                  │              │  │          ▼            │
                  ▼              │  │        Error          │
                Idle             │  │          │            │
                                 │  │          ▼            │
                                 │  └──► Paused             │
                                 │        │  │              │
                                 │        │  │              │
                                 │        │  ▼              │
                                 │        │ Error           │
                                 │        │    │            │
                                 │        │    ▼            │
                                 │        └──► Recording    │
                                 └──────────────────────────┘
```

Key invalid transition pairs to test:
- `start()` in `Recording`, `Paused`, `Stopping`, `Preview`, `Error`, `Starting`, `Countdown`
- `stop()` in `Idle`, `Countdown`, `Paused`, `Error`, `Preview`, `CrashRecovery`
- `pause()` in `Idle`, `Starting`, `Countdown`, `Paused`, `Stopping`, `Preview`, `Error`, `CrashRecovery`
- `resume()` in all states except `Paused`
- `cancel()` in `Idle`, `Stopping`, `Preview`, `Error`, `CrashRecovery`

#### RecordingSession struct

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SessionState {
    Idle,
    Starting,
    Countdown,
    Recording,
    Paused,
    Stopping,
    Preview,
    Error,
    CrashRecovery,
}

#[derive(Debug, Clone)]
pub struct RecordingSession {
    state: SessionState,
}

impl RecordingSession {
    pub fn new() -> Self {
        Self { state: SessionState::Idle }
    }

    pub fn state(&self) -> &SessionState {
        &self.state
    }

    pub fn transition(&mut self, target: SessionState) -> Result<()> {
        // Validate transition, return StateViolation if invalid
        // Update state if valid
    }
}
```

#### ExtensionMessage structure

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecordingMode {
    FullScreen,
    Tab,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExtensionMessage {
    StartRecording { mode: RecordingMode },
    StopRecording,
    PauseRecording,
    ResumeRecording,
    CancelRecording,
    VideoReady { session_id: String },
    RecordingError { code: String, details: String },
    KeepalivePing,
    KeepalivePong,
    GetStreamingData,
    ApplyStreamingData { data: String },
}
```

### References

- [Architecture: Error Handling in WASM] — `_bmad-output/planning-artifacts/architecture.md#error-handling-in-wasm`
- [Architecture: Implementation Patterns] — `_bmad-output/planning-artifacts/architecture.md#implementation-patterns--consistency-rules`
- [Architecture: Rust-Specific Patterns] — `_bmad-output/planning-artifacts/architecture.md#rust-specific-patterns`
- [PRD §6.4: UI States] — `_bmad-output/planning-artifacts/prds/prd-capture-forge-2026-06-19/prd.md#64-user-interface-states-recorder-core`
- [PRD §6.5: Message Protocol] — `_bmad-output/planning-artifacts/prds/prd-capture-forge-2026-06-19/prd.md#65-message-protocol-recorder-core`
- [Epics: Story 1.1] — `_bmad-output/planning-artifacts/epics.md#story-11-error-system--state-machine-foundation`

## Dev Agent Record

### Agent Model Used

Claude Opus 4.8

### Debug Log References

### Completion Notes List

- Implemented `RecordingError` enum with 8 variants using `thiserror` (AC1, AC2)
- Implemented `SessionState` enum with 9 states + `RecordingSession` state machine with full transition matrix (AC3)
- Implemented `ExtensionMessage` enum with 11 variants + `RecordingMode` enum (AC4)
- Installed `panic::set_hook()` override in lib.rs background init (AC5)
- Added `serde_json` as dev-dependency for test roundtrips
- Added 43 unit tests covering: Display format for all error variants, serde roundtrips for all message types, every valid transition path, 15+ invalid transition edge cases, state predicates (is_idle, is_active)
- All tests pass, no bare `unwrap()` in production code

### Review Findings

#### Decision needed

- [x] [Review][Decision] CancelRecording during Recording: add `Recording→Idle` — resolved (option A)

#### Patch items

- [x] [Review][Patch] `panic::take_hook()` incorrectly removes hook after first panic [src/lib.rs:57-60] — fixed: proper chaining with `prev` capture + re-invocation
- [x] [Review][Patch] Missing Error transitions from `Recording`, `Paused`, `Countdown` [src/recorder.rs] — fixed: added `Recording→Error`, `Paused→Error`, `Countdown→Error`
- [x] [Review][Patch] Missing `CrashRecovery→Error` transition [src/recorder.rs] — fixed: added `CrashRecovery→Error`
- [x] [Review][Patch] Panic hook double-panic guard missing [src/lib.rs:39-64] — fixed: added `PANICKING` AtomicBool re-entrancy guard
- [x] [Review][Patch] RecordingError::Panic never constructed [src/lib.rs:53-55] — acknowledged; panic details logged via console.error but not stored in session (Error state transition still occurs; full error context storage is a future concern)
- [x] [Review][Patch] `test_stop_in_paused` is misnamed / copy-pasted [src/recorder.rs] — fixed: removed duplicate test (Paused→Starting already covered by `test_start_in_paused`)
- [x] [Review][Patch] Rename misnamed tests `test_cancel_in_idle` and `test_cancel_in_stopping` [src/recorder.rs] — fixed: renamed to `test_idle_to_idle_self_transition` and `test_stopping_to_idle_rollback`
- [x] [Review][Patch] Panic hook uses `log!` (console.log) not `console.error` [src/lib.rs:46] — fixed: uses `error()` extern shim targeting `console.error`

#### Deferred items

- [x] [Review][Defer] No message routing — ExtensionMessage variants are dead letters — deferred, out of scope for Story 1.1 (routing implemented in background.rs in later stories)
- [x] [Review][Defer] Starting→Idle not allowed (cancellation during stream acquisition forces Error UI) — deferred, related to CancelRecording decision above; depends on product choice
- [x] [Review][Defer] Error wrapping — no `#[source]` or `#[from]` on RecordingError variants — deferred, V0.1 simplicity; structured error chaining can be added in V0.2+
- [x] [Review][Defer] No session identifier or metadata in RecordingSession — deferred, Story 1.3+ when lifecycle is implemented
- [x] [Review][Defer] No SW restart detection / state reconciliation — deferred, Story 1.8 (Crash Recovery)
- [x] [Review][Defer] RecordingError `code: String` in ExtensionMessage has no validation — deferred, Story 1.3+ when message routing is built
- [x] [Review][Defer] RecordingSession missing serde derives (SessionState has them) — deferred, add when serialization is needed in V0.2+
- [x] [Review][Defer] ApplyStreamingData `data: String` has no format contract — deferred, format defined when streaming data module is implemented

### File List

- src/lib.rs (UPDATE)
- src/error.rs (CREATE)
- src/recorder.rs (CREATE)
- src/messaging.rs (CREATE)
- Cargo.toml (UPDATE)
