---
baseline_commit: 714b0d6dcc11f9c73d7549db009e9329e9419c86
---

# Story 1.5: WebM Export Pipeline

Status: review

## Story

As a user,
I want my recorded chunks to be assembled into a valid WebM file,
So that I can play, share, and archive the recording.

**Epic:** 1 — Recorder Core (V0.1, P0)
**FRs covered:** FR8 (REC-08)
**NFRs covered:** NFR-PERF-04 (WebM export 5min <3s)

## Acceptance Criteria

### AC1: Chunk validation during export

**Given** all chunks for a session are in `.bin` state with valid 32-byte headers
**When** the export pipeline reads the chunks in index order
**Then** each chunk header is validated for:
- Magic bytes (`0x43464348` / `CFCH`)
- Version byte (`0x01`)
- XXH3 checksum matches payload (`verify_checksum()`)  
- Index contiguity (no gaps, sequential order)
- `payload_size` matches actual payload length
**And** the validated chunk payloads are assembled into a single WebM blob
**And** the resulting blob is created with MIME type `video/webm`

### AC2: Corrupted chunk detection

**Given** `concat()` is called
**When** a chunk has a corrupted checksum, invalid magic bytes, missing header, or a non-contiguous index
**Then** `RecordingError::ExportError` is returned with the chunk index and failure reason
**And** the export pipeline halts immediately (no partial blob assembly)

### AC3: Empty session rejection

**Given** the session contains no exportable chunks (empty manifest or no `.bin` files)
**When** export is requested
**Then** `RecordingError::EmptySession` is returned with `"No chunks to export"`

### AC4: Export from ChunkManifest / chunk byte data

**Given** a `ChunkManifest` with committed entries
**When** export is called with an ordered list of parsed chunks (header + payload)
**Then** only entries with `ChunkStatus::Committed` status are included
**And** the assembled blob is returned as a `Vec<u8>` of concatenated payloads

### AC5: Performance benchmark scaffolding

**Given** a 5-minute recording is exported (simulated with N chunks totalling ~50MB payload)
**When** measured from export call to blob ready
**Then** the wall-clock time meets the NFR benchmark target of under 3 seconds
**And** the benchmark is implemented as a `#[test]` using synthetic data (no OPFS dependency)

### AC6: Test suite

**Given** the export test suite is executed natively (`cargo test`)
**When** valid, empty, and corrupted chunk sequences are tested
**Then** valid sequences produce a correct concatenated payload
**And** empty and corrupted sequences return appropriate errors
**And** header validation rejects each class of corruption (magic, version, checksum, index gap, size mismatch)

## Tasks / Subtasks

- [x] Task 1: Create `src/export.rs` export module (AC1–AC6)
  - [x] 1.1 Define `ExportChunk` struct — parsed chunk representation (header + payload bytes)
  - [x] 1.2 Implement chunk validation logic — `validate_sequence()` checks each header
  - [x] 1.3 Implement `concat()` — payload concatenation into `Vec<u8>` with `video/webm` metadata
  - [x] 1.4 Wire `RecordingError::ExportError` and `RecordingError::EmptySession` return paths
- [x] Task 2: Update `src/lib.rs` — add `mod export;` (AC4)
- [x] Task 3: Write native unit tests (AC6)
  - [x] 3.1 `test_export_valid_sequence` — valid chunks → correct concatenated payload
  - [x] 3.2 `test_export_empty_session` — empty manifest → `EmptySession`
  - [x] 3.3 `test_export_corrupted_checksum` — wrong payload → `ExportError`
  - [x] 3.4 `test_export_invalid_magic` — bad magic bytes → `ExportError`
  - [x] 3.5 `test_export_version_mismatch` — wrong version → `ExportError`
  - [x] 3.6 `test_export_index_gap` — non-contiguous indices → `ExportError`
  - [x] 3.7 `test_export_payload_size_mismatch` — header says X, actual is Y → `ExportError`
  - [x] 3.8 `test_export_committed_only` — only committed chunks are included
  - [x] 3.9 `test_export_benchmark_5min` — N chunks in 50MB under 3s wall-clock
- [x] Task 4: Verify compilation and tests — `cargo check` + `cargo test` (all pass)

## Dev Notes

### Architecture context

The `WebM Export Pipeline` is the final step in the recording lifecycle after `Stopping` state. The architecture (architecture.md §Chunk Binary Format, architecture.md §Export, PRD §6.2 REC-08) defines it as pure chunk concatenation — no decode/re-encode, no FFmpeg dependency in V0.1.

**How MediaRecorder chunk concatenation works:**

The browser's `MediaRecorder` output has a critical property: **every `dataavailable` event produces a self-contained, valid WebM segment.** These are called "clusters" in WebM terminology. Because each chunk already contains the required WebM headers (EBML header, Segment info, Tracks), **concatenating them in sequential order produces a valid, playable WebM file.**

This is a well-documented technique — notably used by Chrome's own `chrome.tabCapture` examples and various MediaRecorder recording libraries. No inter-chunk processing is required beyond concatenation in capture order.

WebM structure for MediaRecorder output:
```
[EBML header] [Segment] [Cluster 1] [Cluster 2] ... [Cluster N]
```
Each chunk from `dataavailable` starts from a new `Cluster` element (already containing referred headers). Simple `[payload1] + [payload2] + ... + [payloadN]` works for playable output.

### Core module design

Create `src/export.rs` as a flat module (no sub-module directory needed for V0.1). The architecture's `export/webm.rs` split is aspirational for P1 when editor has its own export path — keep it simple for V0.1.

```rust
/// A parsed export chunk with validated header and raw payload.
#[derive(Debug, Clone)]
pub(crate) struct ExportChunk {
    pub index: u32,
    pub header: ChunkHeader,
    /// Raw MediaRecorder payload (header stripped, just the WebM segment).
    pub payload: Vec<u8>,
}

/// Export pipeline: validates and concatenates chunks into a WebM blob.
pub(crate) struct ExportPipeline;

impl ExportPipeline {
    /// Validate a sequence of export chunks for correctness.
    ///
    /// Checks performed on the full sequence:
    /// 1. Non-empty
    /// 2. Index contiguity (0, 1, 2, ...)
    /// 3. Each header's magic, version, checksum, payload_size
    ///
    /// Returns `Ok(())` or the first `ExportError` encountered.
    pub fn validate_sequence(chunks: &[ExportChunk]) -> Result<()> { ... }

    /// Concatenate chunk payloads into a single WebM byte vector.
    ///
    /// 1. Validates the sequence via `validate_sequence()`.
    /// 2. Assembles payloads in index order.
    /// 3. Returns the concatenated result.
    pub fn concat(chunks: &[ExportChunk]) -> Result<Vec<u8>> { ... }
}
```

### Error handling

| Failure Mode | Error Variant | Details |
|-------------|---------------|---------|
| Empty session (no chunks) | `EmptySession` | `"No chunks to export"` |
| Invalid magic bytes | `ExportError` | `"Chunk {index}: invalid magic, expected CFCH, got {actual:02x?}"` |
| Version mismatch | `ExportError` | `"Chunk {index}: unsupported version {version}"` |
| Checksum mismatch | `ExportError` | `"Chunk {index}: checksum mismatch (expected {expected}, got {actual})"` |
| Index gap | `ExportError` | `"Chunk sequence gap: expected index {expected}, got {actual}"` |
| Payload size mismatch | `ExportError` | `"Chunk {index}: header payload_size {header_size} != actual {actual_size}"` |
| Empty chunk payload | `ExportError` | `"Chunk {index}: empty payload in export"` |

### Integration with existing modules

- **`src/chunk.rs`** — Uses `ChunkHeader`, `ChunkHeader::decode()`, `ChunkHeader::verify_checksum()`, and `ChunkManifest` entries. The raw byte data (32-byte header + payload) is stored in `MockChunkStorage` and will be served by OPFS at runtime.
- **`src/error.rs`** — Uses existing `RecordingError::ExportError { details }` and `RecordingError::EmptySession { details }` variants (both already defined).
- **`src/lib.rs`** — Add `mod export;` declaration.
- **No new crate dependencies.** `xxhash-rust` is already in scope via `chunk.rs`.

### Performance notes (NFR-PERF-04)

The 5min @ 3s target is achieved by:
- Pure `Vec::extend_from_slice()` concatenation — O(n) memcpy, no decode.
- No allocation per chunk beyond the single output `Vec`.
- Pre-sizing the output buffer: sum of all `payload_size` values from the manifest.
- For 5min VP8+Opus at ~1.5 Mbps, total payload ≈ 56MB. `Vec::reserve_exact(56MB)` then `extend_from_slice()` 30× (one per 10s chunk). This completes in well under 1s on modern hardware.

### Chunk storage integration (deferred to Story 2.1)

For V0.1 native tests, `MockChunkStorage` holds the bytes. The export pipeline receives already-parsed `ExportChunk` items (header decoded, payload extracted). The OPFS read path (loading chunks from disk at export time) is deferred to Story 2.1 when `OpfsChunkStorage` gets its full read implementation.

For WASM/OPFS, the future flow is:
```
storage: OpfsChunkStorage (or MockChunkStorage in tests)
    → read chunk_{index:06}.bin
    → decode first 32 bytes as ChunkHeader
    → split into ExportChunk { header, payload }
    → collect in Vec<ExportChunk> sorted by index
    → ExportPipeline::concat(&chunks)
```

### Key implementation patterns (must follow)

1. **No bare `unwrap()` anywhere.** Use `expect("invariant: ...")` with descriptive message.
2. **Exhaustive match** on all enums. No `_` catch-all without `unreachable!("reason")`.
3. **Derives**: Every new data-carrying type derives `#[derive(Debug, Clone, Serialize, Deserialize)]`.
4. **`pub` discipline**: `pub(crate)` by default. `pub` only across the message boundary or for external shims.
5. **`type Result<T>` alias**: Import as `use crate::error::Result;` in export.rs.
6. **No unused imports or dead code.** WASM binary size target is <500KB gzipped.
7. **Feature gates**: All code in this story goes in the default feature set (V0.1 foundation, no feature gating needed).
8. **Reorder test assertion order:** `assert_eq!(expected, actual)` — expected value first.

### Contrast with Story 1.4 patterns

| Aspect | Story 1.4 (chunk.rs) | Story 1.5 (export.rs) |
|--------|---------------------|----------------------|
| Storage access | Uses `ChunkStorage` trait | No storage trait dependency — receives parsed `ExportChunk` slices |
| test execution | Synchronous via `MockChunkStorage` | Purely function-based: `ExportPipeline::concat(&chunks)` |
| Header interaction | Creates headers via `ChunkHeader::new()` | Decodes + validates existing headers via `ChunkHeader::decode()` |
| Complexity | Writes, renames, manages lifecycle | Read-only validation + concatenation |

### Project Structure Notes

**Variance from architecture blueprint:**
- The architecture (architecture.md §Project Structure) shows `export.rs` + `export/webm.rs` as a directory module. For V0.1, a single flat `src/export.rs` module is used since WebM is the only export format. If MP4 export is added in P1, the module can be refactored into a directory at that point.
- This is a deliberate **deferral** — keeping the structure flat and simple for V0.1 matches the pattern established by `chunk.rs`, `lifecycle.rs`, `stream.rs`, etc.
- The `export` feature flag in `Cargo.toml` is defined as `export` in the default set. No Cargo.toml changes needed since `export` is already in the default features and no new crate dependencies are introduced.

### Current project state (after Story 1.4)

```
src/
├── lib.rs              # #[oxichrome::extension] + panic hook + SESSION global
├── error.rs            # RecordingError enum (8 variants) + Result<T> alias
├── recorder.rs         # SessionState (9 states) + RecordingSession + transition()
├── messaging.rs        # ExtensionMessage (11 variants) + RecordingMode
├── stream.rs           # StreamAcquisitionService + AcquiredStream + mix_audio
├── lifecycle.rs        # RecordingLifecycle — start/stop/pause/resume/cancel
├── chunk.rs            # ChunkHeader, ChunkManifest, ChunkWriter, ChunkStorage, MockChunkStorage
```

**Existing `RecordingSession` fields**: `state`, `mode`, `mic_enabled`, `session_id`, `accumulated_duration_ms`.

**Permissions**: `["storage", "unlimitedStorage", "desktopCapture", "tabCapture", "downloads"]` — unchanged.

### Files to CREATE

| File | Purpose |
|------|---------|
| `src/export.rs` | `ExportChunk`, `ExportPipeline` struct with `validate_sequence()` and `concat()`, unit tests |

### Files to UPDATE

| File | What changes |
|------|-------------|
| `src/lib.rs` | Add `mod export;` |

### No Cargo.toml changes

No new crate dependencies. All required types are already available: `ChunkHeader` from `crate::chunk`, `RecordingError` from `crate::error`, `Vec`/`u32`/`u64` from std.

## Testing Requirements

### Unit tests (`cargo test` — native, no browser needed)

| # | Test name | What it validates |
|---|-----------|-------------------|
| 1 | `test_export_valid_sequence` | 3 valid chunks → concatenated payload equals expected sum |
| 2 | `test_export_empty_session` | Empty slice → `Err(RecordingError::EmptySession)` |
| 3 | `test_export_corrupted_checksum` | Chunk with bad checksum → `Err(ExportError)` with chunk index |
| 4 | `test_export_invalid_magic` | Chunk with bad magic → `Err(ExportError)` with "magic" in details |
| 5 | `test_export_version_mismatch` | Chunk with version `0xFF` → `Err(ExportError)` with "version" in details |
| 6 | `test_export_index_gap` | Sequence [0, 1, 3] → `Err(ExportError)` with "gap" in details |
| 7 | `test_export_payload_size_mismatch` | Header says 100, actual 50 → `Err(ExportError)` with "payload_size" in details |
| 8 | `test_export_committed_only` | Mix of committed + written chunks → only committed exported |
| 9 | `test_export_benchmark_5min` | ~30 chunks totalling ~56MB → under 3000ms wall-clock |
| 10 | `test_export_empty_chunk_payload` | Chunk with 0-byte payload → `Err(ExportError)` |
| 11 | `test_export_single_chunk` | Single chunk → payload unchanged |
| 12 | `test_export_validate_sequence_ordered` | `validate_sequence()` returns `Ok(())` for correct ascending order |
| 13 | `test_export_validate_sequence_empty` | `validate_sequence([])` returns `Err(EmptySession)` |

### Test data approach

All tests should construct `ExportChunk` values directly (no storage backend needed):

```rust
fn make_valid_chunk(index: u32, payload: &[u8]) -> ExportChunk {
    let header = ChunkHeader::new(index, 0.0, payload);
    ExportChunk {
        index,
        header,
        payload: payload.to_vec(),
    }
}

// Build valid header bytes, then corrupt them for negative tests:
fn make_chunk_with_bad_checksum(index: u32, payload: &[u8]) -> ExportChunk {
    let mut chunk = make_valid_chunk(index, payload);
    chunk.header.checksum = 0xDEADBEEF;
    chunk
}
```

## Dependencies

### New crate dependencies

**None.** The export module only uses:
- `crate::chunk::ChunkHeader` (already defined)
- `crate::error::{RecordingError, Result}` (already defined)
- Standard library types

### Existing dependencies used

- `ChunkHeader::decode()` — parse 32-byte header bytes
- `ChunkHeader::verify_checksum()` — validate payload integrity
- `ChunkHeader::MAGIC` / `ChunkHeader::CURRENT_VERSION` — validation constants
- `xxhash_rust::xxh3::xxh3_64` — available via `crate::chunk` if needed for recalc

## Previous Story Intelligence (Story 1.4)

### Key learnings applicable to this story

1. **`pub(crate)` discipline**: The code review flagged that new public methods should default to `pub(crate)`. Apply to all export module functions.

2. **No `web-sys` dependency in core logic**: The chunk module kept OPFS-wasm behind `#[cfg(target_arch = "wasm32")]` gates. The export module similarly operates on pure Rust types (`Vec<u8>` slices) — no browser API calls in the core export logic.

3. **`ChunkHeader::decode()` is already battle-tested**: Header decode with magic/version/checksum validation is already implemented and tested in `chunk.rs`. The export module calls this method rather than reimplementing header parsing.

4. **Assertion order**: `assert_eq!(expected, actual)` — the expected value comes first.

5. **`expect()` over `unwrap()`**: All unwraps use `expect("invariant: ...")` with descriptive messages explaining why the branch is infallible.

6. **Error details strings as documentation**: The code review applied patches improving error detail messages (hex formatting for magic bytes, etc.). The export module follows the same pattern — error details serve as both debugging context and user-facing messages.

### Review fixes applied in Story 1.4

- Magic byte error uses hex formatting `{magic:02x?}` (not string comparison)
- `debug_assert_eq!` for invariant is changed to `assert_eq!` (release-mode validation)
- `commit_chunk()` is idempotent (can be called multiple times safely)
- Path parameter in `write_chunk()` eliminates rename ambiguity
- Duplicate chunk index entries are explicitly checked

### Code patterns established

```rust
// Module-level Result alias
use crate::error::Result;

// pub(crate) by default
pub(crate) fn my_function() -> Result<()> { ... }

// Exhaustive match on enums
match value {
    MyEnum::Variant1 => { ... }
    MyEnum::Variant2 => { ... }
}

// expect with invariant messages
let handle = something.expect("invariant: should never fail");
```

## Git Intelligence Summary

Recent commits:
1. `ca08261` — docs(CLAUDE.md): update project status and module documentation
2. `b4d684c` — fix(chunk): apply code review patches (8 review patches applied)
3. `5169fd8` — chore(repo): add README, LICENSE (MIT), improve .gitignore
4. `180fc79` — feat(chunk): implement chunk writer foundation
5. `14a20ce` — feat(chunk): add chunk writer foundation with header and lifecycle

**Commits 2–5 are directly relevant:** The chunk module was recently implemented and reviewed. The review patches (commit `b4d684c`) fixed patterns that the export module should follow from the start: correct hex error formatting, idempotent operations, assert_eq! in release mode, and proper path parameter naming.

## Web Research / Latest API Information

### MediaRecorder WebM chunks — concatenation works natively

The behavior of `MediaRecorder` producing independently-playable WebM segments is governed by the **WebM specification** (webmproject.org) and **MediaRecorder API** (W3C). MediaRecorder defaults to "fragmented" WebM output where each `dataavailable` event produces a valid cluster. Research confirms:

- Chrome's `MediaRecorder` with `video/webm;codecs=vp8,opus` produces fragmented WebM.
- Each chunk starts from a new Cluster element; concatenation in capture order produces valid files.
- No inter-chunk processing (no header repair, no duration fixup) is needed for basic playback.
- The `Blob` with MIME type `video/webm` is the standard output format.

### Performance data

- VP8 at 1080p averages ~1.5–2 Mbps. A 5-min recording ≈ 56–75 MB total.
- `Vec::extend_from_slice()` throughput on modern x86_64: ~8 GB/s for a single thread.
- Even at conservative 2 GB/s effective throughput, 75 MB concatenates in ~37ms — well under the 3s NFR target.
- The 3s target accounts for OPFS read overhead, which will be added in Story 2.1. The pure concatenation benchmark should validate well under 500ms.

### No new API surfaces

No `web-sys` features, `js-sys` APIs, `chrome.*` APIs, or external crate features are needed for the core export logic. The `Blob` creation happens at the WASM boundary (future story — 1.7 or 2.1), not in the export pipeline itself.

## References

- [Architecture: Chunk Binary Format] — architecture.md §Chunk Binary Format (32-byte header layout)
- [Architecture: Export Pipeline] — architecture.md §Export (chunk concatenation → WebM blob)
- [Architecture: Project Structure] — architecture.md §Project Structure (export.rs + export/webm.rs for P1)
- [Architecture: Implementation Patterns] — architecture.md §Implementation Patterns (naming, error handling, Result alias)
- [PRD §6.1: Scope] — prd.md §6.1 (WebM export via chunk concatenation, no re-encode)
- [PRD §6.2: REC-08] — prd.md §6.2 REC-08 (user story: export as WebM)
- [PRD §6.3: REC-A7] — prd.md §6.3 REC-A7 (performance: WebM export 5min <3s)
- [PRD §5.4: Data Flow] — prd.md §5.4 step 7 (concat chunks → WebM blob)
- [PRD §10.3: NFR-PERF-04] — prd.md §10.3 NFR-PERF-04 (WebM export 5min <3s)
- [Epics: Story 1.5] — epics.md §Story 1.5 (WebM Export Pipeline)
- [Previous Story 1.4] — implementation-artifacts/1-4-chunk-writer-foundation.md (full story with review patches)
- [Existing code: chunk.rs] — src/chunk.rs (ChunkHeader, MockChunkStorage, test patterns)
- [Existing code: error.rs] — src/error.rs (ExportError + EmptySession variants)
- [Existing code: lib.rs] — src/lib.rs (module declarations)

## Dev Agent Record

### Guardrails

1. **`src/export.rs` is a flat module** — one file, no directory sub-module. The architecture's `export/webm.rs` split is for P1. Keep it simple for V0.1.

2. **No OPFS integration in this story.** The export pipeline receives parsed `ExportChunk` values. Reading from OPFS is deferred to Story 2.1.

3. **No new crate or web-sys dependencies.** All needed types are in existing modules or std.

4. **`ChunkHeader::decode()` is the single source of truth** for header parsing. The export module does NOT reimplement byte-by-byte header parsing — it calls `ChunkHeader::decode()` and validates the decoded result.

5. **Use `ChunkHeader::verify_checksum()` for validation**, not a recalculated checksum. The header already carries the checksum; verify it against the payload bytes.

6. **Export operates on `Vec<u8>` payloads, not `Blob` objects.** The `Blob` creation with `video/webm` MIME type happens at the WASM boundary. Core logic uses `Vec<u8>` to remain testable natively.

7. **All chunks must be in committed state.** Non-committed chunks (Partial, Written) are excluded from export. If no committed chunks exist, return `EmptySession`.

8. **Index contiguity check is strict.** Sequences must start at 0 and increment by 1. A gap (e.g., [0, 1, 3]) returns `ExportError`. Duplicate indices also fail.

9. **Payload size match is validated.** Each chunk's `header.payload_size` must equal `payload.len()`. Mismatch returns `ExportError`.

10. **The benchmark test must run under 3s** on developer hardware. Use ~30 chunks of ~1.9MB each (~57MB total). Gather timing with `std::time::Instant`.

### Implementation Plan

**Approach:** Implement the WebM export pipeline as a self-contained module with three layers:

1. **Data layer** — `ExportChunk` struct wrapping `ChunkHeader + Vec<u8>` payload. Validated index and metadata.
2. **Validation layer** — `validate_sequence()` iterates chunks, checks:
   - Non-empty (→ `EmptySession`)
   - Index contiguity (0, 1, 2, ...)
   - Per-chunk: magic, version, checksum, payload_size match
3. **Concatenation layer** — `concat()` calls validate, then pre-allocates output buffer, extends with each payload in order, returns `Vec<u8>`.

**Key decisions:**
- All methods operate on `&[ExportChunk]` slices — no storage backends.
- `ChunkHeader::decode()` is called externally (before constructing `ExportChunk`). The export module works with already-decoded headers.
- Validation is separated from concatenation (two methods) for testability and future use by the recovery module.
- Performance: pre-allocate output buffer with `Vec::with_capacity(total_payload_size)`.

### Debug Log

- Created `src/export.rs` with `ExportChunk` struct (added `status: ChunkStatus` field for committed-only filtering), `ExportPipeline` struct with `validate_sequence()` and `concat()` methods
- Made `ChunkHeader::calc_checksum` pub(crate) in `chunk.rs` for checksum error reporting
- Added `mod export;` to `src/lib.rs`
- Wrote 14 tests (13 story-required + 1 extra for all-uncommitted edge case)
- Validation order: index contiguity → magic → version → checksum → payload_size → empty payload
- `concat()` validates full sequence first, then filters to committed chunks only, then concatenates

### Completion Notes

Story 1.5 implémentée et vérifiée :
- **ExportChunk** — structure avec index, header (ChunkHeader), payload (Vec<u8>), status (ChunkStatus)
- **ExportPipeline::validate_sequence()** — valide séquence non-vide, contiguïté des index, magic bytes, version, checksum (via verify_checksum()), payload_size, et payload non-vide
- **ExportPipeline::concat()** — valide la séquence via validate_sequence(), filtre les chunks committed, concatène dans un Vec<u8> pré-alloué via extend_from_slice()
- Tous les messages d'erreur suivent le tableau de spécification dans Dev Notes
- `cargo check` → 0 erreurs, `cargo test` → 124 tests passent (14 nouveaux + 110 existants)
- Performance benchmark (56 MB, 30 chunks) : bien sous 1s (cible <3s NFR-PERF-04)

### File List

#### Files to Create
- `src/export.rs` — ExportChunk, ExportPipeline, concat(), validate_sequence(), unit tests (14 tests)

#### Files Modified
- `src/lib.rs` — Add `mod export;`
- `src/chunk.rs` — Made `calc_checksum()` pub(crate) for export error reporting

#### Cargo.toml
- No changes needed (no new dependencies)

---

## Change Log

| Date | Change |
|------|--------|
| 2026-06-20 | Created story file from epics Story 1.5 requirements and previous story intelligence |
| 2026-06-20 | Implemented export module: ExportChunk, ExportPipeline (validate_sequence + concat), 14 unit tests, module wiring. Made calc_checksum pub(crate). Status → review |
