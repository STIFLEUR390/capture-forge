---
baseline_commit: 8df30d671800fa44f25e08a81ba13b852d66b742
---

# Story 1.9: Heartbeat Keepalive

Status: ready-for-dev

## Story

As a developer,
I want a keepalive mechanism between the offscreen document and service worker,
So that the service worker stays alive during active recording and can detect a dead worker.

**Epic:** 1 — Recorder Core (V0.1, P0)

**FRs covered:**
- FR13 (Architecture): Heartbeat keepalive — ping/pong every 20s from offscreen doc to SW

**NFRs covered:**
- NFR-REL-01: 99% session uptime — prevent SW idle timeout killing active recordings
- NFR-REL-02: 100% detection — 3 missed pings (60s) reliably detected as dead SW
- NFR-REL-05: Graceful degradation — dead SW detection transitions to CrashRecovery, data preserved
- NFR-SEC-01: No data leaves browser during keepalive — all communication is `chrome.runtime.sendMessage` within the extension

**Architecture references:**
- Architecture §Heartbeat / SW Keepalive: Offscreen-document ping/pong every 20s during active recording
- Architecture §Constraints: Chrome MV3 SW ~30s idle timeout — heartbeat prevents premature death

## Acceptance Criteria

### AC1: Keepalive starts when recording starts (offscreen doc)

**Given** a recording session transitions to active (Starting or Recording)
**When** the offscreen document is created (or if no offscreen doc yet, the keepalive manager is started)
**Then** a `setInterval` at 20s interval starts
**And** each tick sends `ExtensionMessage::KeepalivePing` to the service worker via `chrome.runtime.sendMessage`
**And** the keepalive manager tracks the time of the last sent ping and pending response

### AC2: Service worker responds with KeepalivePong

**Given** the service worker receives a `KeepalivePing` message
**When** it processes the KeepalivePing
**Then** it immediately responds with `ExtensionMessage::KeepalivePong`
**And** any message receipt (including KeepalivePing) resets Chrome's SW idle timer automatically (Chrome resets the 30s timer on any `chrome.runtime.onMessage` event)
**And** the pong response routes back to the offscreen document's `sendMessage` callback

### AC3: Pong resets missed-ping counter

**Given** the offscreen document receives a `KeepalivePong` response
**When** the pong is processed
**Then** the consecutive-missed-pings counter is reset to zero
**And** no recovery action is taken

### AC4: 3 consecutive missed pings (60s) — detect dead SW

**Given** the offscreen document sent 3 pings without receiving any pong response (60s of silence)
**When** the 3rd ping times out
**Then** the keepalive manager assumes the service worker is dead
**And** the current chunk is finalised (best-effort OPFS write)
**And** a stale `in_flight` lock is written to `chrome.storage.local` (so the crash recovery scan on next SW start will detect it)
**And** the keepalive manager cleans up its interval and closures
**And** the offscreen document stops recording resources

### AC5: Keepalive stops when recording ends normally

**Given** the recording stops normally (Stop, Cancel, or Preview → Idle)
**When** the recording lifecycle ends
**Then** the keepalive manager clears the ping interval (`clearInterval`)
**And** the keepalive manager state transitions to Stopped
**And** no further keepalive messages are sent
**And** all closures and timer handles are released

### AC6: Keepalive does not start when idle

**Given** no recording session is active
**When** the extension runs
**Then** no keepalive interval is started
**And** no ping/pong messages are exchanged

### AC7: Multiple pings without pong don't overflow counter

**Given** the SW is unresponsive for more than 60s (3 missed pings)
**When** the offscreen doc detects 3 missed pings
**Then** recovery action is taken once
**And** the keepalive manager stops sending further pings
**And** no duplicate recovery actions are triggered

### AC8: Chrome SW idle timer reset on message receipt

**Given** the service worker is running
**When** any `chrome.runtime.onMessage` event fires (including KeepalivePing)
**Then** Chrome resets its 30s SW idle timeout (Chrome MV3 behavior — no explicit JS call needed)
**And** this is verified in testing (the keepalive keeps the SW alive across multiple ping cycles)

## Tasks / Subtasks

- [ ] **Task 1: Create `src/keepalive.rs` — KeepaliveManager (AC1, AC3–AC7)**
  - [ ] 1.1 Define `KeepaliveState` enum: `Idle`, `Active`, `Stopped`, `DeadSwDetected`
  - [ ] 1.2 Define `KeepaliveManager` struct with fields:
    - `state: KeepaliveState` — current lifecycle state
    - `interval_id: Option<i32>` — `setInterval` return value (for `clearInterval`)
    - `interval_closure: Option<Closure<dyn FnMut()>>` — holds the interval closure alive (prevents GC)
    - `missed_pings: u32` — consecutive missed pings counter (0..3)
    - `on_crash_detected: Option<Box<dyn FnMut() + Send>>` — callback invoked when 3 pings missed
    - `pong_callback_holder: Option<Closure<dyn FnMut(JsValue)>>` — holds `sendMessage` response callback
  - [ ] 1.3 Implement `new()` — initialises in Idle state with missed_pings = 0
  - [ ] 1.4 Implement `start()` — creates 20s `setInterval`, stores closure on struct, transitions to Active
  - [ ] 1.5 Implement `stop()` — calls `clearInterval(interval_id)`, drops closures, transitions to Stopped
  - [ ] 1.6 Implement `handle_pong()` — resets `missed_pings` to 0
  - [ ] 1.7 Implement `on_ping_tick()` — increments missed_pings; if ≥3, sets state to DeadSwDetected and invokes `on_crash_detected` callback
  - [ ] 1.8 Implement `current_state()` — returns current KeepaliveState
  - [ ] 1.9 Implement `set_on_crash_detected(callback)` — stores the crash callback
  - [ ] 1.10 Implement `Drop` — clears interval if active, drops all closures
  - [ ] 1.11 Add `#[cfg(test)] mod tests` with pure-logic tests:
    - `test_keepalive_initial_state` — state is Idle, missed_pings = 0
    - `test_keepalive_start_transitions_to_active` — start() changes state
    - `test_keepalive_stop_transitions_to_stopped` — stop() cleans up
    - `test_keepalive_pong_resets_counter` — handle_pong() after 2 missed resets to 0
    - `test_keepalive_three_misses_triggers_crash` — 3 ticks without pong fires callback
    - `test_keepalive_two_misses_then_pong_does_not_trigger` — 2 misses + pong resets, 3rd tick does not fire
    - `test_keepalive_no_double_crash` — after 3rd miss, state is DeadSwDetected; 4th tick does nothing
    - `test_keepalive_start_while_active_is_noop` — calling start() twice is a no-op or returns error
    - `test_keepalive_stop_while_idle_is_noop` — calling stop() in Idle is a no-op
    - `test_keepalive_drop_cleans_up` — dropping manager clears interval
    - `test_keepalive_ping_20s_interval` — start() configures 20000ms interval

- [ ] **Task 2: Update `src/lib.rs` — wire KeepalivePing handler in service worker (AC2, AC8)**
  - [ ] 2.1 Add `mod keepalive;` declaration
  - [ ] 2.2 In the runtime message handler, add a match arm for `"KEEPALIVE_PING"` that:
      - Creates a response object with type `"KEEPALIVE_PONG"`
      - Calls `send_response(response)` immediately (synchronous response)
      - Returns `wasm_bindgen::JsValue::from(true)` to indicate synchronous response
  - [ ] 2.3 The existing `_ => {}` catch-all already discards unknown messages — pong responses (from offscreen doc's `sendMessage` callback) arrive as the Promise resolution, not as `onMessage` events, so no additional handling needed
  - [ ] 2.4 Verify that Chrome automatically resets the SW idle timer on `onMessage` receipt — no explicit code needed (this is Chrome MV3 behavior)

- [ ] **Task 3: Update `src/lib.rs` — integrate keepalive into lifecycle (AC1, AC5, AC6)**
  - [ ] 3.1 Add `KEEPALIVE_MANAGER: OnceLock<Mutex<Option<KeepaliveManager>>>` global in lib.rs
  - [ ] 3.2 Implement `init_keepalive_manager()` — initialise the OnceLock
  - [ ] 3.3 Implement `start_keepalive()` — creates KeepaliveManager, calls `start()`, stores in global
  - [ ] 3.4 Implement `stop_keepalive()` — takes manager from global, calls `stop()`, drops it
  - [ ] 3.5 Call `init_keepalive_manager()` during `start()` init alongside session/preview/recovery init
  - [ ] 3.6 The lifecycle integration (calling start_keepalive when recording starts, stop_keepalive when recording ends) will be wired in a future orchestration pass — for now, expose the functions ready for integration
  - [ ] 3.7 The `on_crash_detected` callback should:
      - Log the event via `oxichrome::log!`
      - Write stale `in_flight` lock to `chrome.storage.local` (timestamp >30s old)
      - Attempt to finalise current chunk data
      - Clean up keepalive resources
      - The actual session state transition to CrashRecovery happens when the SW restarts and `scan_and_propose_recovery()` detects orphaned data

- [ ] **Task 4: Update `Cargo.toml` — add any new web-sys features (AC1–AC3)**
  - [ ] 4.1 No new web-sys features are expected — keepalive uses `js_sys::Function` (setInterval/clearInterval) and `chrome.runtime.sendMessage` via `js_sys::Reflect`, both already available
  - [ ] 4.2 Verify existing compilation with `cargo check`

- [ ] **Task 5: Update `dist/chromium/manifest.json` — verify offscreen permissions (AC1)**
  - [ ] 5.1 Verify `storage` permission is present for `chrome.storage.local` lock writing
  - [ ] 5.2 Verify `chrome.offscreen` API availability — no explicit permission needed in MV3 (available to all extensions)
  - [ ] 5.3 No other permission changes needed for keepalive

- [ ] **Task 6: Add keepalive tests (AC1–AC8)**
  - [ ] 6.1 Pure-logic unit tests in `keepalive.rs` (covered in Task 1.11)
  - [ ] 6.2 Verify existing test suite still passes: `cargo test`
  - [ ] 6.3 Add serde roundtrip test for KeepalivePing/KeepalivePong if not already covered

## Dev Notes

### Architecture context

The heartbeat keepalive is the **last line of defense** against Chrome MV3's 30-second service worker idle timeout. Without it, a recording that produces chunks infrequently (e.g., long pause between interactions, or long chunks) could have its service worker killed by Chrome, losing the message routing capability.

The architecture defines two sides of the heartbeat:

```
Offscreen document (recording context)         Service Worker (message router)
│                                                    │
│  setInterval(20s)                                   │
│  ├─ chrome.runtime.sendMessage(KeepalivePing) ────→ │  onMessage → respond KeepalivePong
│  │                                                  │  (SW idle timer auto-reset)
│  └─ ← KeepalivePong response ──────────────────── │
│       → reset missed_pings = 0                     │
│                                                    │
│  If 3 pings unanswered (60s):                       │
│  ├─ Finalise current chunk                         │  (SW may be dead)
│  ├─ Write stale in_flight lock                     │
│  └─ Clean up offscreen resources                   │
│                                                    │
│  Next SW restart → scan_and_propose_recovery()     │
│  → detects orphaned data → CrashRecovery toast     │
```

### What exists already

| Component | Status | Relevance |
|-----------|--------|-----------|
| `ExtensionMessage::KeepalivePing` | ✅ Already defined in messaging.rs | Used directly — no changes needed |
| `ExtensionMessage::KeepalivePong` | ✅ Already defined in messaging.rs | Used directly — no changes needed |
| `is_keepalive()` helper | ✅ Already implemented | Returns true for Ping/Pong variants |
| Keepalive serde roundtrip tests | ✅ Already exist (test_keepalive_ping, test_keepalive_pong) | Pass — no changes needed |
| `SessionState::CrashRecovery` | ✅ Defined in recorder.rs | Target state after dead SW detection |
| `SESSION` global + transition() | ✅ Available in lib.rs | Used for state transitions |
| `scan_and_propose_recovery()` | ✅ Implemented in lib.rs | Runs on SW init, detects orphan sessions |
| `check_in_flight_lock_stale()` | ✅ Implemented in lib.rs | Reads in_flight lock, checks staleness |
| `show_recovery_toast()` | ✅ Implemented in lib.rs | Recovery UI for orphan detection |
| Background message handler (lib.rs) | ✅ Raw string matching | New `KEEPALIVE_PING` handler needed |
| Offscreen document creation | ❌ Not yet implemented | New — JS-side `chrome.offscreen.createDocument()` |
| KeepaliveManager (ping sender + tracker) | ❌ Not implemented | New `src/keepalive.rs` module needed |
| KeepalivePing SW handler | ❌ Not implemented | Add to raw string match in lib.rs |
| Recovery on dead SW detection | ❌ Not implemented | Integration with crash recovery path |
| Lifecycle integration (start/stop keepalive with recording) | ❌ Not implemented | Functions exposed ready for orchestration |

### KeepaliveManager design

```rust
#[derive(Debug, Clone, PartialEq)]
enum KeepaliveState {
    Idle,
    Active,
    Stopped,
    DeadSwDetected,
}

pub(crate) struct KeepaliveManager {
    state: KeepaliveState,
    interval_id: Option<i32>,
    interval_closure: Option<Closure<dyn FnMut()>>,
    missed_pings: u32,
    on_crash_detected: Option<Box<dyn FnMut() + Send>>,
    pong_callback_holder: Option<Closure<dyn FnMut(JsValue)>>,
}
```

### Ping/pong data flow

```
Offscreen document:
  setInterval(20000) ──→ send chrome.runtime.sendMessage({
                              type: "KEEPALIVE_PING"
                           })
                           └─→ Promise callback fires on pong response
                               └─→ handle_pong() → reset missed_pings = 0

Service Worker:
  chrome.runtime.onMessage:
    if msg.type == "KEEPALIVE_PING" →
      sendResponse({type: "KEEPALIVE_PONG"})
      Chrome auto-resets SW idle timer
```

**Key detail:** The pong response arrives via `sendMessage`'s Promise resolution, NOT as a separate `onMessage` event. This means:
1. The offscreen doc calls `chrome.runtime.sendMessage(pingMsg)` which returns a Promise
2. The SW `onMessage` handler calls `sendResponse({type: "KEEPALIVE_PONG"})`
3. The Promise resolves with the pong response on the offscreen doc side
4. The `KeepaliveManager` holds a `Closure<dyn FnMut(JsValue)>` as the Promise callback

This means the SW-side handler is a **one-liner**: match KEEPALIVE_PING → sendResponse(pong). The offscreen doc side manages the Promise callback lifecycle.

### Closure and timer management

Critical patterns inherited from Story 1.7/1.8 code reviews:

1. **setInterval/clearInterval** via `js_sys::Function`:
   ```rust
   let setTimeout = js_sys::Reflect::get(&js_sys::global(), &"setInterval".into())
       .expect("invariant: setInterval exists")
       .dyn_into::<js_sys::Function>()
       .expect("invariant: setInterval is a Function");
   let id = setTimeout.call2(
       &JsValue::NULL,
       &closure.as_ref().unchecked_ref(),
       &2000.into(),  // 2000ms for test, 20000ms for production
   ).expect("invariant: setInterval succeeds")
    .as_f64()
    .expect("invariant: setInterval returns a number") as i32;
   ```

2. **clearInterval on stop/Drop**:
   ```rust
   if let Some(id) = self.interval_id {
       let clearInterval = js_sys::Reflect::get(&js_sys::global(), &"clearInterval".into())
           .expect("invariant: clearInterval exists")
           .dyn_into::<js_sys::Function>()
           .expect("invariant: clearInterval is a Function");
       let _ = clearInterval.call1(&JsValue::NULL, &JsValue::from(id as f64));
       self.interval_id = None;
   }
   ```

3. **Closure must be stored on the struct** to prevent garbage collection. Drop order matters — interval closure is dropped AFTER clearInterval is called.

4. **Double-invocation guard**: `start()` must check current state and be a no-op if already Active. Use an `AtomicBool` or return `Err(RecordingError::StateViolation)`.

5. **Send-safe callback**: `on_crash_detected` uses `Box<dyn FnMut() + Send>` so it can be stored in a `OnceLock<Mutex<...>>` global.

### sendMessage Promise callback pattern

```rust
// On the offscreen document side:
fn send_keepalive_ping(manager: &mut KeepaliveManager) {
    let chrome = js_sys::Reflect::get(&js_sys::global(), &"chrome".into()).ok()?;
    let runtime = js_sys::Reflect::get(&chrome, &"runtime".into()).ok()?;
    let send_msg = js_sys::Reflect::get(&runtime, &"sendMessage".into()).ok()?;

    let msg = js_sys::Object::new();
    js_sys::Reflect::set(&msg, &"type".into(), &"KEEPALIVE_PING".into()).ok()?;

    // sendMessage returns a Promise
    let promise = js_sys::Reflect::apply(&send_msg, &runtime, &js_sys::Array::of1(&msg)).ok()?;
    let promise = js_sys::Promise::from(promise);

    // Create closure for the Promise resolution (receives pong)
    let pong_cb = Closure::wrap(Box::new(move |response: JsValue| {
        // response contains {type: "KEEPALIVE_PONG"}
        // Reset missed_pings counter
        // Access manager via raw pointer or global
    }) as Box<dyn FnMut(JsValue)>);

    promise.then(&pong_cb);  // Attach resolution handler
    manager.pong_callback_holder = Some(pong_cb);
}
```

### SW handler pattern

In `src/lib.rs`, the existing `KEEPALIVE_PING` handler follows the same raw-string pattern as existing handlers:

```rust
"KEEPALIVE_PING" => {
    // Respond immediately with KeepalivePong.
    // Chrome automatically resets the SW idle timer on onMessage receipt.
    let response = Object::new();
    Reflect::set(&response, &"type".into(), &"KEEPALIVE_PONG".into()).ok();
    if let Some(sr) = send_response.dyn_ref::<js_sys::Function>() {
        let _ = sr.call1(&JsValue::NULL, &response);
    }
    will_respond = true;
}
```

### Recovery on dead SW detection

When the offscreen doc detects the SW is dead (3 missed pings), it should:

1. **Finalise current chunk**: Best-effort write of any in-memory chunk data to OPFS
2. **Set stale in_flight lock**: Write to `chrome.storage.local`:
   ```js
   chrome.storage.local.set({
     in_flight: {
       session_id: "rec_...",
       started_at: Date.now() - 60000  // 60s ago → stale
     }
   })
   ```
3. **Clean up**: Clear interval, drop closures, release media resources
4. **Note**: The offscreen doc does NOT directly transition the SESSION state machine (SESSION lives in the SW WASM instance, which may be dead). Instead, the stale in_flight lock + orphaned OPFS chunks serve as the crash signal. When Chrome restarts the SW (on next user interaction), `scan_and_propose_recovery()` detects both signals and proposes recovery.

This is consistent with the existing crash recovery architecture:
```
Dead SW detected (offscreen doc)
  → Write stale in_flight lock + finalise chunks
  → Chrome restarts SW on next event
  → scan_and_propose_recovery()
  → toast: "A previous recording session was found."
  → User clicks Restore → triple verification → preview
```

### Offscreen document creation

For V0.1, the offscreen document creation is a prerequisite. The lifecycle orchestrator needs to:

1. Create the offscreen document when recording starts:
   ```js
   chrome.offscreen.createDocument({
     url: 'offscreen.html',
     reasons: ['USER_MEDIA'],
     justification: 'Recording media with MediaRecorder'
   });
   ```
2. Close it when recording ends:
   ```js
   chrome.offscreen.closeDocument();
   ```

The offscreen document (offscreen.html) loads the WASM module and starts the keepalive manager. For V0.1, the offscreen document HTML/JS is a thin wrapper:
- Loads `wasm/capture_forge.js`
- Calls an exported init function that starts the keepalive
- The keepalive manager's ping interval and message handling run inside the offscreen WASM instance

For this story (1.9), the offscreen document creation mechanism should be **scaffolded** — a placeholder function that creates the offscreen doc and starts the keepalive. Full offscreen doc integration with the MediaRecorder lifecycle will be completed in the orchestration wiring (post-1.9).

### Feature gates

All code in this story goes in the **default feature set** (V0.1, no feature gating). Keepalive is a core V0.1 feature needed by every recording session.

### Past review findings to avoid

From Story 1.7 and 1.8 code reviews:

1. **Closure storage and Drop**: Store all `Closure<...>` values as struct fields on `KeepaliveManager`. Implement `Drop` that clears interval, drops all closures, and revokes callback references.

2. **Double-invocation guard**: `start()` must guard against being called twice. Check `state != Idle` and return early or error.

3. **Timer management**: `interval_id` stored as struct field (i32 from setInterval). Cleared via `clearInterval` in `stop()` and `Drop`. Always clear the interval BEFORE dropping the closure that the interval calls (prevents dangling closure call).

4. **Drop ordering**: In `Drop`, clear interval first, THEN drop closures. The struct field order helps (Rust drops fields in declaration order), but explicit ordering in the Drop impl is clearer.

5. **No bare unwrap**: All unwraps use `expect("invariant: ...")`.

6. **`pub(crate)` discipline**: Default to `pub(crate)` on all new types and methods. Only promote for message boundary.

7. **`#[derive(Debug, Clone, Serialize, Deserialize)]`**: Only where serde serialization is needed. `KeepaliveState` should derive `Debug, Clone, PartialEq` but NOT `Serialize/Deserialize` (it's internal state, never sent over IPC).

8. **Reordered assertions**: `assert_eq!(expected, actual)` — expected value first.

### Error states

| Failure Mode | Behaviour | Notes |
|-------------|-----------|-------|
| Offscreen doc not available | Keepalive cannot start | Log warning, recording continues without keepalive (SW may die after 30s idle) |
| sendMessage fails (SW busy) | Missed ping counter incremented | Same as unanswered ping — counts toward 3-miss threshold |
| Interval creation fails | Keepalive start returns error | Recording should still proceed; keepalive is best-effort |
| SW never responds (dead) | 3 missed pings → crash detected | Recovery path via orphan chunk scan at next SW init |

### NFR compliance notes

| NFR | Implementation |
|-----|----------------|
| NFR-REL-01 | 99% session uptime: keepalive prevents SW idle timeout during active recording. 20s interval provides 10s margin before Chrome's 30s timeout. |
| NFR-REL-02 | 100% detection: 3 missed pings (60s) reliably indicates dead SW. Counter-based — no ambiguity. |
| NFR-REL-05 | Graceful degradation: dead SW detection writes stale in_flight lock + finalises chunks. Crash recovery path surfaces the recovered data. |
| NFR-SEC-01 | All keepalive communication is intra-extension via `chrome.runtime.sendMessage`. No network calls. |

### Current project state (after Story 1.8)

```
src/
├── lib.rs              # #[oxichrome::extension], panic hook, SESSION, PREVIEW_DATA, RECOVERY_TOAST globals, message handler
├── error.rs            # RecordingError enum (8 variants), Result<T> alias
├── recorder.rs         # SessionState (9 states including CrashRecovery), RecordingSession, transition()
├── messaging.rs        # ExtensionMessage (~16 variants including KeepalivePing/Pong), RecordingMode, is_keepalive()
├── stream.rs           # StreamAcquisitionService, AcquiredStream, mix_audio
├── lifecycle.rs        # RecordingLifecycle — start/stop/pause/resume/cancel, MediaRecorder, duration
├── chunk.rs            # ChunkHeader (32-byte), ChunkManifest, ChunkWriter, ChunkStatus, MockChunkStorage, OpfsChunkStorage
├── export.rs           # ExportChunk, ExportPipeline::validate_sequence(), concat()
├── countdown.rs        # CountdownOverlay — 3-2-1 animation, circle ring, Escape handler
├── status_bar.rs       # RecorderStatusBar — timer, Pause/Resume, Stop, blink animation
├── preview.rs          # PreviewPage — video player, Download/Delete, integrity badge, confirmation dialog, error state
├── recovery.rs         # RecoveryManager, IntegrityReport, IntegrityStatus, triple verification
├── recovery_toast.rs   # RecoveryToast — non-modal crash recovery toast UI with auto-dismiss
```

### File naming

All files go directly in `src/` following the existing flat module structure (not `src/keepalive/mod.rs`).

## Project Structure Notes

### Files to CREATE

| File | Purpose |
|------|---------|
| `src/keepalive.rs` | `KeepaliveManager`, `KeepaliveState` — ping/pong cycle, missed-ping tracking, interval management, Drop cleanup |

### Files to UPDATE

| File | What changes |
|------|-------------|
| `src/lib.rs` | Add `mod keepalive;`. Add `KEEPALIVE_MANAGER` global. Add `init_keepalive_manager()`, `start_keepalive()`, `stop_keepalive()`. Add `KEEPALIVE_PING` message handler in the runtime message router. Call `init_keepalive_manager()` during `start()` init. |
| `dist/chromium/manifest.json` | Verify `storage` permission present (needed for in_flight lock writing) — expected: already present |
| `Cargo.toml` | Verify any new web-sys features (expected: none) |

### Cargo.toml changes

No new crate dependencies expected. Keepalive uses `js_sys::Function` (setInterval/clearInterval) and `js_sys::Reflect` (chrome.runtime.sendMessage), both already available. web-sys features not needed — keepalive does not use browser APIs beyond `chrome.runtime`.

### Architecture blueprint variance

The architecture document mentions heartbeat under `background.rs` section, listing the SW listeners. Following the current **flat module structure** (all `.rs` files directly in `src/`), the keepalive logic goes in `src/keepalive.rs`. The `background.rs` routing for ping/pong goes in `src/lib.rs` where all other message handlers live.

## Testing Requirements

### Unit tests (`cargo test`)

| # | Test name | What it validates |
|---|-----------|-------------------|
| 1 | `test_keepalive_initial_state` | KeepaliveManager starts in Idle, missed_pings = 0 |
| 2 | `test_keepalive_start_transitions_to_active` | `start()` transitions state to Active |
| 3 | `test_keepalive_stop_transitions_to_stopped` | `stop()` transitions state to Stopped |
| 4 | `test_keepalive_pong_resets_counter` | `handle_pong()` after 2 missed pings resets counter to 0 |
| 5 | `test_keepalive_three_misses_triggers_crash` | 3 `on_ping_tick()` calls without pong fires crash callback |
| 6 | `test_keepalive_two_misses_then_pong_does_not_trigger` | 2 misses + pong → 3rd tick does not fire crash |
| 7 | `test_keepalive_no_double_crash` | After 3 misses (DeadSwDetected), 4th tick does nothing |
| 8 | `test_keepalive_start_while_active` | `start()` when already Active is a no-op or returns error |
| 9 | `test_keepalive_stop_while_idle` | `stop()` when Idle is a no-op |
| 10 | `test_keepalive_crash_callback_fired_once` | Crash callback fires exactly once per detection |
| 11 | `test_keepalive_ping_20s_interval` | `start()` configures interval with 20000ms delay |
| 12 | `test_keepalive_drop_cleans_up` | Dropping KeepaliveManager clears state |

### WASM tests (`wasm-pack test --headless --chrome`)

| # | Test name | What it validates |
|---|-----------|-------------------|
| 1 | `test_wasm_keepalive_set_interval_cleared` | Interval ID created and cleared via js_sys |
| 2 | `test_wasm_keepalive_send_message_runtime` | `chrome.runtime.sendMessage` call creates proper message |
| 3 | `test_wasm_keepalive_noop_without_chrome` | Graceful handling when chrome API not available |

## References

- [Architecture: Heartbeat Keepalive] — architecture.md §Heartbeat / SW Keepalive (offscreen doc ping/pong 20s, 3 missed → CrashRecovery)
- [Architecture: Constraints] — architecture.md §Technical Constraints (Chrome MV3 SW ~30s idle timeout)
- [Epics: Story 1.9] — epics.md §Story 1.9 (Heartbeat Keepalive — ACs 1-4)
- [PRD §6.5] — prd.md §6.5 (Message protocol — KeepalivePing, KeepalivePong variants)
- [Existing code: messaging.rs] — src/messaging.rs (KeepalivePing, KeepalivePong already defined)
- [Existing code: lib.rs] — src/lib.rs (message handler pattern, globals, init flow)
- [Existing code: recorder.rs] — src/recorder.rs (SessionState::CrashRecovery)
- [Previous Story 1.8] — 1-8-crash-recovery-detection-restore.md (closure storage pattern, Drop cleanup, timer management, review findings)
- [Deferred Work] — deferred-work.md (no opfs/enumeration blockers for this story)

## Dev Agent Record

### Guardrails

1. **Keepalive is best-effort** — If the offscreen doc or chrome.runtime is unavailable, recording still works (just without keepalive protection). Never block recording start on keepalive availability.

2. **Closure storage is critical** — The interval closure and pong callback MUST be stored as struct fields on KeepaliveManager. If dropped, the interval silently stops firing. Follow the exact pattern from lifecycle.rs `_ondataavailable_closure`.

3. **Clear interval before dropping closure** — In `stop()` and `Drop`, call `clearInterval(id)` BEFORE dropping the closure that the interval invokes. Otherwise the interval may fire into freed WASM memory.

4. **No Chrome API in native tests** — All keepalive logic that touches `chrome.runtime.sendMessage` must be behind `#[cfg(target_arch = "wasm32")]`. Pure-logic tests (state machine, counter) run on `cargo test` without browser.

5. **Dead SW detection is one-shot** — Once `DeadSwDetected` state is reached, no further actions fire. The guard in `on_ping_tick()` prevents duplicate recovery triggers.

6. **Single crash callback invocation** — The `on_crash_detected` callback must fire exactly once per dead-SW event. Use a flag or state check to prevent re-entry.

7. **No session transition in keepalive** — The KeepaliveManager does NOT directly call `SESSION.transition()`. It fires the `on_crash_detected` callback. The callback handles external side effects (stale lock, chunk finalisation). Session state transition happens on SW restart via crash recovery scan.

8. **20s interval, not 30s** — Chrome's SW idle timeout is 30s. A 20s ping interval provides 10s margin. Do not change to 30s — there must be room for timing variance.

9. **`pub(crate)` discipline** — All new types and methods default to `pub(crate)`. Only promote for message boundary or external shim.

10. **No bare unwrap** — All unwraps use `expect("invariant: ...")`.

11. **Drop implementation** — Must clear interval, drop closures, and reset state. Test that dropping does not panic.

### File List

#### Files Created
- `src/keepalive.rs` — `KeepaliveManager`, `KeepaliveState`, ping/pong tracking, interval management, crash detection, 12 unit tests

#### Files Modified
- `src/lib.rs` — `mod keepalive;`, `KEEPALIVE_MANAGER` global, `init_keepalive_manager()`, `start_keepalive()`, `stop_keepalive()`, `KEEPALIVE_PING` message handler in runtime message router, call in `start()` init
- `dist/chromium/manifest.json` — Verify `storage` permission present (expected: already present; `chrome.offscreen` needs no explicit permission)
- `src/lib.rs` `#[oxichrome::extension(...)]` — Add `"offscreen"` permission if not present

## Change Log

| Date | Change |
|------|--------|
| 2026-06-28 | Initial creation of Story 1.9 — Heartbeat Keepalive |
