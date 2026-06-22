# Deferred Work

Tracking items deferred from code reviews that are not yet actionable but should not be forgotten.

---

## Deferred from: code review of 1-2-stream-acquisition-screen-tab-mic (2026-06-19)

- Tab capture returns empty MediaStream — `acquire_tab()` returns a dummy stream. Full reconstruction depends on offscreen doc infrastructure in Story 1.3+.
- Temp mic stream not explicitly owned — minor GC concern in `mix_audio()`; no correctness impact.
- get_display_media without constraints — future enhancement; works correctly now without them.
- getUserMedia fails in SW context (Tab+mic) — architectural issue resolved when offscreen document handoff is implemented.
- Race window in async acquire() — orchestrator concern in Story 1.3+.
- AcquiredStream cannot cross Send boundary — `!Send` types prevent storing stream in `RecordingSession`; managed ephemerally.

## Deferred from: code review of 1-1-error-system-state-machine-foundation (2026-06-19)

- No message routing — ExtensionMessage variants are dead letters (routing implemented in background.rs in later stories)
- Starting→Idle not allowed — cancellation during stream acquisition forces Error UI (depends on product choice)
- Error wrapping — no `#[source]` or `#[from]` on RecordingError variants (V0.1 simplicity; structured error chaining in V0.2+)
- No session identifier or metadata in RecordingSession (Story 1.3+ when lifecycle is implemented)
- No SW restart detection / state reconciliation (Story 1.8 — Crash Recovery)
- RecordingError `code: String` in ExtensionMessage has no validation (Story 1.3+ when message routing is built)
- RecordingSession missing serde derives while SessionState has them (add when serialization needed in V0.2+)
- ApplyStreamingData `data: String` has no format contract (format defined when streaming data module is implemented)

## Deferred from: code review of 1-4-chunk-writer-foundation (2026-06-20)

- `commit_chunk` only commits most recent chunk — By design: single-chunk commit is correct for V0.1 lifecycle.
- Reserved bytes not validated on decode — Forward-compatible acceptance; a future version may use these bytes.
- `update_status` allows invalid transitions (Committed→Partial) — In-memory manifest only; crash-recovery concern is acceptable for V0.1.
- `payload_size` from decoded header not validated — Caller responsibility to validate at read time.
- `session_id` stored but unused — Will be used in Story 2.1 for OPFS directory naming.
- Orphaned `.partial` files on rename failure — No cleanup rollback in V0.1; acknowledged as acceptable.

## Deferred from: code review of 1-5-webm-export-pipeline (2026-06-20)

- `i as u32` truncation on 64-bit platforms [src/export.rs:48] — MAX_CHUNK_INDEX = 999,999 prevents u32 overflow in practice. Deferred, pre-existing architecture constraint.
- `usize` overflow in `total_size` sum on 32-bit WASM [src/export.rs:131] — Real recordings <2GB in WASM memory. Allocation would fail gracefully first. Deferred, pre-existing architecture constraint.
- Benchmark in unit tests rather than `#[bench]` [src/export.rs:tests] — `#[bench]` is unstable and criterion is not available. Acceptable as `#[test]` for V0.1.

## Deferred from: code review of 1-6-countdown-recorder-status-bar (2026-06-20)

- 5 spec-required unit tests missing (pause label accessor, icon state, CSS class checks, WASM injection tests) — requires adding accessor methods to native struct. Low priority, spec coverage vs implementation depth trade-off.
- Task 4 (background router wiring) explicitly deferred — integration dependent on future orchestrator work.

## Deferred from: code review of 1-7-preview-page-play-download-delete (2026-06-20)

- Incomplete integrity state lacks recovery explanation message [AC8] — "This recording could not be fully recovered." message depends on IntegrityReport infrastructure from Story 1.8 (crash recovery).
- Partial integrity state lacks recovered-chunk detail [AC8] — "up to chunk N of M" requires chunk metadata from IntegrityReport (Story 1.8).
- Preview page not registered in manifest.json — Manual HTML approach works for V0.1, oxichrome registration can be done later.
- Space key test coverage gap [AC5] — No negative test for Space when video unfocused. Code behavior is correct but untested.
- PreviewClosed variant in ExtensionMessage unused in runtime handler — Handler matches raw string "PREVIEW_CLOSED" instead of deserializing the variant. Works correctly. Consolidation in future refactor.
