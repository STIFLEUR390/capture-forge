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
