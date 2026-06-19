# Sprint Stories — Resilient Storage Quick Wins

**Epic:** Recorder Core (P0) — Resilience
**Derived from:** Brainstorming session 2026-06-19 (docs/brainstorming.md)
**Depends on:** Existing OPFS storage layer (`storage.rs`, `RecoveryManager`, `OpfsCleanup`)
**Output format:** Each story is stand-alone, testable, and implements one concrete brick of the truth-first persistence model.

---

## Story 1: Chunk Status Lifecycle

### Context

Today the system writes chunks to OPFS but has no formal lifecycle per chunk. The `RecoveryManager` can detect an interrupted session but cannot tell whether a specific chunk is valid, partial, or orphaned. This story introduces a per-chunk status enum that travels alongside the chunk from creation to verification.

### Problem

After a service-worker kill during a chunk write, the system cannot distinguish:
- A chunk that was fully written and confirmed (`committed`)
- A chunk that was partially written when the SW died (`partial`)
- A stale chunk whose metadata exists but whose file is gone (`orphaned`)

Without this distinction, recovery can only make binary decisions (session OK / session lost) instead of precise ones ("session recovered to chunk N-1, last chunk orphaned").

### Solution

Introduce a named status enum for each chunk and encode it in both the **file-naming convention** and the **in-memory manifest**:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChunkStatus {
    /// Writer acquired the lock, file name reserved
    Started,
    /// `FileSystemWritableFileStream.close()` resolved without error
    Written,
    /// Written AND recorded in the session manifest
    Committed,
    /// Committed AND size/checksum verified against manifest entry
    Verified,
    /// Found on disk but no matching manifest entry (or vice versa)
    Orphaned,
}
```

**File-naming convention:**

| Status | File Pattern |
|--------|-------------|
| `Started` | `chunk_{index:06}.partial` |
| `Written` | `chunk_{index:06}.written` |
| `Committed` | `chunk_{index:06}.bin` |
| `Verified` | `chunk_{index:06}.bin` (status tracked in manifest only) |

The write protocol becomes:

1. Create `chunk_000123.partial` → status = `Started`
2. Write blob → close stream
3. Rename to `chunk_000123.written` → status = `Written`
4. Append entry to manifest with size + timestamp → rename to `chunk_000123.bin` → status = `Committed`
5. Background verification → status = `Verified`

### Acceptance Criteria

- [ ] A newly created chunk file is named `chunk_NNNNNN.partial`
- [ ] After stream close + rename, the file is `chunk_NNNNNN.written`
- [ ] After manifest append + rename, the file is `chunk_NNNNNN.bin`
- [ ] `RecoveryManager::check_in_flight()` reports the highest `Verified` or `Committed` index, not the highest `partial`
- [ ] On session recovery, any `.partial` or `.written` file without a matching `committed` manifest entry is reported as `orphaned` and cleaned up by `OpfsCleanup`
- [ ] The manifest records per-chunk entries in the shape: `{index, track, size_bytes, checksum_xxh3, status, timestamp}`
- [ ] No breaking changes to existing chunk accumulation during active recording (the naming change is backward-compatible via a write-adapter)

### Edge Cases

- **Zero-byte chunk**: A chunk with 0 bytes is `Written` immediately but should transition to `Committed` + `Verified` eagerly (trivially valid)
- **Duplicate index**: If two chunks claim index 123, the second write must fail at the file-creation step (`.partial` already exists)
- **Rename failure**: If OPFS does not support rename (unexpected), fall back to write-at-final-path + manifest-first strategy
- **Concurrent recovery**: If the session is being recovered while a new chunk write is in-flight, the `partial` file is ignored by recovery

### Definition of Done

- [ ] `ChunkStatus` enum + file-naming implemented in `storage.rs`
- [ ] Write path follows `Started → Written → Committed` protocol
- [ ] `RecoveryManager` reads chunk statuses from file extension + manifest
- [ ] `OpfsCleanup::cleanup_orphans()` handles `.partial` and `.written` files
- [ ] Unit tests cover all valid transitions
- [ ] Integration test: kill SW mid-write, recover, verify orphan detection

---

## Story 2: Triple Recovery Verification

### Context

The `RecoveryManager` currently loads the session state from `chrome.storage.local` and checks for in-flight sessions. It does not cross-reference chunk blobs in OPFS against the manifest. After introducing chunk statuses (Story 1), the recovery must validate that the **stated truth** matches the **stored truth**.

### Problem

If OPFS silently loses files (quota exceeded, cleanup by browser, corruption) or `chrome.storage.local` holds stale metadata, recovery cannot detect the mismatch. It would either:
- Assume the session is intact when chunks are missing, or
- Assume the session is lost when it's actually recoverable from a previous state.

### Solution

Implement three independent checks that run during `RecoveryManager::check_in_flight()` and produce a structured `IntegrityReport`:

```rust
pub struct IntegrityReport {
    pub session_id: SessionId,
    pub total_chunks_expected: u64,
    pub total_found: u64,
    pub total_missing: u64,
    pub total_orphaned: u64,
    pub total_size_mismatch: u64,
    pub status: IntegrityStatus,
    pub details: Vec<ChunkDiscrepancy>,
}

pub enum IntegrityStatus {
    Clean,       // All chunks verified
    Partial,    // Some chunks missing or mismatched
    Incomplete, // No contiguous prefix can be reconstructed
    Unknown,    // Insufficient data to form an opinion
}
```

### The Three Checks

**Check 1 — Manifest vs Filesystem**

For every `committed` entry in the manifest, verify a file exists at `cloud-chunks/<sessionId>/<track>/chunk_NNNNNN.bin`.

- Missing file → record `ChunkDiscrepancy { index, kind: Missing }`
- Extra file not in manifest → record `ChunkDiscrepancy { index, kind: Orphaned }`

**Check 2 — File Size vs Manifest Size**

For every file that passes Check 1, read its `File.size` and compare to the manifest entry's `size_bytes`.

- Mismatch → record `ChunkDiscrepancy { index, kind: SizeMismatch { expected, actual } }`
- Check is skipped if manifest size is 0 (backward-compat with pre-manifest sessions)

**Check 3 — Index Contiguity**

From the verified chunks, find the longest contiguous prefix starting at index 0.

- If gaps exist before the first missing index → `Partial`
- If no contiguous prefix longer than 1 chunk → `Incomplete`
- Report the `last_contiguous_index` field so the UI can show "recovered up to 12:34.5"

### Acceptance Criteria

- [ ] `RecoveryManager::check_in_flight()` returns an `IntegrityReport` instead of a boolean
- [ ] All three checks run unconditionally on recovery
- [ ] A `Clean` report is produced when all checks pass for all chunks
- [ ] A `Partial` report is produced when Check 1 or Check 2 finds ≥1 discrepancy but a contiguous prefix ≥2 chunks exists
- [ ] An `Incomplete` report is produced when less than 2 contiguous chunks can be verified
- [ ] Discrepancies are logged at `warn!` level with chunk index + kind
- [ ] Recovery only replays chunks up to `last_contiguous_index` (exclusive)
- [ ] Orphaned files (not in manifest, no matching partial) are queued for cleanup but do not fail recovery

### Edge Cases

- **Empty session (no chunks yet):** Session has `in_flight=true` but zero manifest entries → report `Incomplete`, recovery cleans up cleanly
- **Single-chunk session:** A session with exactly 1 chunk → if checks pass, report `Clean` (contiguous prefix is `[0]`)
- **Large gap in the middle:** Chunks 0–5 present, 6–8 missing, 9–10 present → report `Partial`, `last_contiguous_index = 5`
- **Manifest newer than files:** If chrome.storage was saved but OPFS writes didn't finish → detected by Check 1 → `Partial`

### Definition of Done

- [ ] `IntegrityReport` struct with serialization support
- [ ] Three check functions implemented and unit-tested
- [ ] `RecoveryManager` integrates all three checks into `check_in_flight()`
- [ ] Recovery decisions use `IntegrityReport.status` and `last_contiguous_index`
- [ ] Quota-exceeded scenario simulated in integration test with partial OPFS loss

---

## Story 3: Native Integrity Report as Session Output

### Context

Once the system can detect partial sessions (Stories 1 + 2), it should surface this information to the user as a **first-class output** — not a hidden log entry. The integrity report is a natural complement to the video, transcript, and Markdown outputs of a session.

### Problem

Today, when a session is recovered, the user sees either "Recording saved" or "Recording failed". There is no intermediate state that communicates *what* was recovered and *what* was lost. As sessions become the primary artifact (Recorder Sémantique vision), the user needs to trust the integrity of what they're working with.

### Solution

The `IntegrityReport` (from Story 2) is rendered as a human-readable document attached to the session, accessible from the session browser and from the Recovery dialog:

```rust
pub struct IntegrityReportDocument {
    pub summary: String,       // "Session recovered to 92%"
    pub detail_sentences: Vec<String>,  // "3 chunks missing between 08:12 and 08:29"
    pub recommendation: String, // "You can view the recovered portion or retry the full session."
    pub raw_report: IntegrityReport, // Machine-readable, for UI rendering
}
```

**UX Placement:**

- **After recovery:** A non-modal banner appears: "Session partially recovered (92%) — [View report]"
- **Session browser:** Each session card shows an integrity badge: ✅ Clean / ⚠️ Partial / ❌ Incomplete
- **Before export:** A warning if the session status is not `Clean`: "This session has missing fragments. Some publications may be incomplete."
- **Output header:** Every derived publication (video, Markdown, QA report) includes a metadata header with the integrity status at render time

### Acceptance Criteria

- [ ] `IntegrityReportDocument` is generated automatically after recovery completes
- [ ] The document is persisted as a metadata file next to the session (same OPFS session folder)
- [ ] The summary line follows the template: "Session recovered to {pct}% — {last_contiguous_duration} of {total_duration}"
- [ ] Detail sentences are generated from each `ChunkDiscrepancy`: "Chunk #{index} missing ({kind})"
- [ ] The recommendation is context-aware:
  - `Clean` → "All chunks verified. Ready for editing and export."
  - `Partial` → "You can edit the recovered portion. Missing fragments won't affect published outputs."
  - `Incomplete` → "Not enough data for meaningful recovery. You may want to retry the recording."
- [ ] The session browser reads the report and displays the integrity badge on the session card
- [ ] The export pipeline checks the integrity status before rendering and attaches the report to the output if `Partial` or `Incomplete`
- [ ] At least one sub-second rendering path for the report (no expensive computation — just string formatting)

### Edge Cases

- **User dismisses report:** The report persists on disk; dismissing the UI banner only hides it until the next session-recovery event
- **Session is Clean:** Report is still generated (as proof) but marked as non-actionable — no UI banner shown
- **Report for very large sessions (500+ chunks):** Detail sentences should summarize: "3 chunks missing" not list 500 lines — provide a `full_details_available` flag for expandable UI

### Definition of Done

- [ ] `IntegrityReportDocument` generated as a native output of recovery
- [ ] Persisted in OPFS session folder as `integrity-report.json`
- [ ] Session browser displays integrity badge per session card
- [ ] Export pipeline checks integrity and attaches report metadata
- [ ] Integration test: recover a partial session, verify report content matches actual chunk discrepancies

---

## Dependency Graph

```
Story 1: Chunk Status Lifecycle
    │  provides ChunkStatus, file naming, manifest entries
    ▼
Story 2: Triple Recovery Verification
    │  consumes ChunkStatus + manifest → IntegrityReport
    ▼
Story 3: Native Integrity Report Output
    │  consumes IntegrityReport → user-facing document
```

All three stories can be implemented in a single sprint (they share the `storage.rs` module) and should be tested together in integration.
