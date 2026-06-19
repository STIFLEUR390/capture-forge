# Deferred Work

Tracking items deferred from code reviews that are not yet actionable but should not be forgotten.

---

## Deferred from: code review of 1-1-error-system-state-machine-foundation (2026-06-19)

- No message routing — ExtensionMessage variants are dead letters (routing implemented in background.rs in later stories)
- Starting→Idle not allowed — cancellation during stream acquisition forces Error UI (depends on product choice)
- Error wrapping — no `#[source]` or `#[from]` on RecordingError variants (V0.1 simplicity; structured error chaining in V0.2+)
- No session identifier or metadata in RecordingSession (Story 1.3+ when lifecycle is implemented)
- No SW restart detection / state reconciliation (Story 1.8 — Crash Recovery)
- RecordingError `code: String` in ExtensionMessage has no validation (Story 1.3+ when message routing is built)
- RecordingSession missing serde derives while SessionState has them (add when serialization needed in V0.2+)
- ApplyStreamingData `data: String` has no format contract (format defined when streaming data module is implemented)
