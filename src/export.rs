use serde::{Deserialize, Serialize};

use crate::chunk::{ChunkHeader, ChunkStatus};
use crate::error::{RecordingError, Result};

// ---------------------------------------------------------------------------
// ExportChunk
// ---------------------------------------------------------------------------

/// A parsed export chunk with validated header and raw payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ExportChunk {
    pub index: u32,
    pub header: ChunkHeader,
    /// Raw MediaRecorder payload (header stripped, just the WebM segment).
    pub payload: Vec<u8>,
    /// Commit status of this chunk. Only `ChunkStatus::Committed` chunks
    /// are included in the final export.
    pub status: ChunkStatus,
}

// ---------------------------------------------------------------------------
// ExportPipeline
// ---------------------------------------------------------------------------

/// Export pipeline: validates and concatenates chunks into a WebM blob.
pub(crate) struct ExportPipeline;

impl ExportPipeline {
    /// Validate a sequence of export chunks for correctness.
    ///
    /// Checks performed on the full sequence:
    /// 1. Non-empty
    /// 2. Index contiguity (0, 1, 2, ...)
    /// 3. Each header's magic, version, checksum, payload_size
    /// 4. Empty payload rejection
    ///
    /// Returns `Ok(())` or the first `ExportError` / `EmptySession` encountered.
    pub(crate) fn validate_sequence(chunks: &[ExportChunk]) -> Result<()> {
        // 1. Non-empty check.
        if chunks.is_empty() {
            return Err(RecordingError::EmptySession {
                details: "No chunks to export".into(),
            });
        }

        // 2–4. Index contiguity and per-chunk validation.
        for (i, chunk) in chunks.iter().enumerate() {
            let expected_idx = i as u32;

            // Index contiguity.
            if chunk.index != expected_idx {
                return Err(RecordingError::ExportError {
                    details: format!(
                        "Chunk sequence gap: expected index {}, got {}",
                        expected_idx, chunk.index,
                    ),
                });
            }

            // Magic bytes validation.
            if chunk.header.magic != *b"CFCH" {
                return Err(RecordingError::ExportError {
                    details: format!(
                        "Chunk {}: invalid magic, expected CFCH, got {:02x?}",
                        expected_idx, chunk.header.magic,
                    ),
                });
            }

            // Version validation.
            if chunk.header.version != 0x01 {
                return Err(RecordingError::ExportError {
                    details: format!(
                        "Chunk {}: unsupported version {}",
                        expected_idx, chunk.header.version,
                    ),
                });
            }

            // Checksum validation.
            if !chunk.header.verify_checksum(&chunk.payload) {
                let actual = ChunkHeader::calc_checksum(&chunk.payload);
                return Err(RecordingError::ExportError {
                    details: format!(
                        "Chunk {}: checksum mismatch (expected {}, got {})",
                        expected_idx, chunk.header.checksum, actual,
                    ),
                });
            }

            // Payload size validation.
            let actual_size = chunk.payload.len() as u64;
            if chunk.header.payload_size != actual_size {
                return Err(RecordingError::ExportError {
                    details: format!(
                        "Chunk {}: header payload_size {} != actual {}",
                        expected_idx, chunk.header.payload_size, actual_size,
                    ),
                });
            }

            // Empty payload check.
            if chunk.payload.is_empty() {
                return Err(RecordingError::ExportError {
                    details: format!("Chunk {}: empty payload in export", expected_idx),
                });
            }
        }

        Ok(())
    }

    /// Concatenate chunk payloads into a single WebM byte vector.
    ///
    /// 1. Validates the sequence via `validate_sequence()`.
    /// 2. Filters to committed chunks only.
    /// 3. Pre-allocates the output buffer.
    /// 4. Assembles committed payloads in index order.
    /// 5. Returns the concatenated result.
    pub(crate) fn concat(chunks: &[ExportChunk]) -> Result<Vec<u8>> {
        // Validate the full sequence first.
        Self::validate_sequence(chunks)?;

        // Filter to only committed chunks.
        let committed: Vec<&ExportChunk> = chunks
            .iter()
            .filter(|c| c.status == ChunkStatus::Committed)
            .collect();

        // If no committed chunks, return EmptySession.
        if committed.is_empty() {
            return Err(RecordingError::EmptySession {
                details: "No chunks to export".into(),
            });
        }

        // Pre-allocate output buffer from total payload size.
        let total_size: usize = committed.iter().map(|c| c.payload.len()).sum();
        let mut output = Vec::with_capacity(total_size);

        // Assemble payloads in index order (already sorted — filter preserves order).
        for chunk in committed {
            output.extend_from_slice(&chunk.payload);
        }

        debug_assert_eq!(output.len(), total_size, "invariant: output size must match pre-allocated capacity");

        Ok(output)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::ChunkHeader;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn make_valid_chunk(index: u32, payload: &[u8]) -> ExportChunk {
        let header = ChunkHeader::new(index, 0.0, payload);
        ExportChunk {
            index,
            header,
            payload: payload.to_vec(),
            status: ChunkStatus::Committed,
        }
    }

    fn make_chunk_with_status(index: u32, payload: &[u8], status: ChunkStatus) -> ExportChunk {
        let mut chunk = make_valid_chunk(index, payload);
        chunk.status = status;
        chunk
    }

    fn make_chunk_with_bad_checksum(index: u32, payload: &[u8]) -> ExportChunk {
        let mut chunk = make_valid_chunk(index, payload);
        chunk.header.checksum = 0xDEAD_BEEF;
        chunk
    }

    fn make_chunk_with_bad_magic(index: u32, payload: &[u8]) -> ExportChunk {
        let mut chunk = make_valid_chunk(index, payload);
        chunk.header.magic = *b"BAD!";
        chunk
    }

    fn make_chunk_with_bad_version(index: u32, payload: &[u8]) -> ExportChunk {
        let mut chunk = make_valid_chunk(index, payload);
        chunk.header.version = 0xFF;
        chunk
    }

    fn make_chunk_with_payload_size_mismatch(index: u32, payload: &[u8]) -> ExportChunk {
        // Create a chunk with correct header + payload, then override
        // the header's payload_size to be wrong. Recalculate checksum
        // to match the actual payload so only payload_size fails.
        let mut chunk = make_valid_chunk(index, payload);
        // Set payload_size to an explicitly wrong value (larger than actual).
        chunk.header.payload_size = payload.len() as u64 + 999;
        // Recalculate checksum to match the actual payload so the checksum
        // check passes — we only want to trigger the payload_size mismatch.
        chunk.header.checksum = ChunkHeader::calc_checksum(&chunk.payload);
        chunk
    }

    // -----------------------------------------------------------------------
    // Valid sequence
    // -----------------------------------------------------------------------

    #[test]
    fn test_export_valid_sequence() {
        let chunks = vec![
            make_valid_chunk(0, b"AAA"),
            make_valid_chunk(1, b"BBB"),
            make_valid_chunk(2, b"CCC"),
        ];

        let result = ExportPipeline::concat(&chunks).expect("concat should succeed");
        let expected = [&b"AAA"[..], &b"BBB"[..], &b"CCC"[..]].concat();
        assert_eq!(expected, result);
    }

    // -----------------------------------------------------------------------
    // Empty session
    // -----------------------------------------------------------------------

    #[test]
    fn test_export_empty_session() {
        let chunks: Vec<ExportChunk> = vec![];
        let err = ExportPipeline::concat(&chunks).unwrap_err();
        assert!(
            matches!(&err, RecordingError::EmptySession { details } if details == "No chunks to export"),
            "expected EmptySession for empty chunk list, got {err:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Corrupted checksum
    // -----------------------------------------------------------------------

    #[test]
    fn test_export_corrupted_checksum() {
        let chunks = vec![
            make_valid_chunk(0, b"AAA"),
            make_chunk_with_bad_checksum(1, b"BBB"),
        ];

        let err = ExportPipeline::concat(&chunks).unwrap_err();
        assert!(
            matches!(&err, RecordingError::ExportError { details } if details.contains("checksum mismatch")),
            "expected ExportError for bad checksum, got {err:?}"
        );
        assert!(
            matches!(&err, RecordingError::ExportError { details } if details.contains("Chunk 1")),
            "expected error to reference chunk index 1, got {err:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Invalid magic
    // -----------------------------------------------------------------------

    #[test]
    fn test_export_invalid_magic() {
        let chunks = vec![
            make_valid_chunk(0, b"AAA"),
            make_chunk_with_bad_magic(1, b"BBB"),
        ];

        let err = ExportPipeline::concat(&chunks).unwrap_err();
        assert!(
            matches!(&err, RecordingError::ExportError { details } if details.contains("magic")),
            "expected ExportError for bad magic, got {err:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Version mismatch
    // -----------------------------------------------------------------------

    #[test]
    fn test_export_version_mismatch() {
        let chunks = vec![
            make_valid_chunk(0, b"AAA"),
            make_chunk_with_bad_version(1, b"BBB"),
        ];

        let err = ExportPipeline::concat(&chunks).unwrap_err();
        assert!(
            matches!(&err, RecordingError::ExportError { details } if details.contains("version")),
            "expected ExportError for bad version, got {err:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Index gap
    // -----------------------------------------------------------------------

    #[test]
    fn test_export_index_gap() {
        let chunks = vec![
            make_valid_chunk(0, b"AAA"),
            make_valid_chunk(1, b"BBB"),
            make_valid_chunk(3, b"DDD"), // gap at index 2
        ];

        let err = ExportPipeline::concat(&chunks).unwrap_err();
        assert!(
            matches!(&err, RecordingError::ExportError { details } if details.contains("gap")),
            "expected ExportError for index gap, got {err:?}"
        );
        assert!(
            matches!(&err, RecordingError::ExportError { details } if details.contains("expected index 2, got 3")),
            "expected error mentioning expected index 2, got {err:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Payload size mismatch
    // -----------------------------------------------------------------------

    #[test]
    fn test_export_payload_size_mismatch() {
        let chunks = vec![
            make_valid_chunk(0, b"AAA"),
            make_chunk_with_payload_size_mismatch(1, b"short"),
        ];

        let err = ExportPipeline::concat(&chunks).unwrap_err();
        assert!(
            matches!(&err, RecordingError::ExportError { details } if details.contains("payload_size")),
            "expected ExportError for payload size mismatch, got {err:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Committed only
    // -----------------------------------------------------------------------

    #[test]
    fn test_export_committed_only() {
        let chunks = vec![
            make_chunk_with_status(0, b"AAA", ChunkStatus::Committed),
            make_chunk_with_status(1, b"BBB", ChunkStatus::Written),
            make_chunk_with_status(2, b"CCC", ChunkStatus::Committed),
        ];

        let result = ExportPipeline::concat(&chunks).expect("concat should succeed");
        // Only committed chunks (0 and 2) should be concatenated.
        let expected = [&b"AAA"[..], &b"CCC"[..]].concat();
        assert_eq!(expected, result, "only committed chunks should be in output");
    }

    #[test]
    fn test_export_all_uncommitted_returns_empty() {
        let chunks = vec![
            make_chunk_with_status(0, b"AAA", ChunkStatus::Written),
            make_chunk_with_status(1, b"BBB", ChunkStatus::Partial),
        ];

        let err = ExportPipeline::concat(&chunks).unwrap_err();
        assert!(
            matches!(&err, RecordingError::EmptySession { details } if details == "No chunks to export"),
            "expected EmptySession when no committed chunks, got {err:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Benchmark: 5 minutes
    // -----------------------------------------------------------------------

    #[test]
    fn test_export_benchmark_5min() {
        // Simulate a 5-minute recording: ~30 chunks totalling ~56 MB.
        let chunk_count: usize = 30;
        let chunk_size: usize = 56 * 1024 * 1024 / chunk_count; // ~1.87 MB per chunk
        let payload = vec![0xABu8; chunk_size];

        let chunks: Vec<ExportChunk> = (0..chunk_count)
            .map(|i| make_valid_chunk(i as u32, &payload))
            .collect();

        let start = std::time::Instant::now();
        let result = ExportPipeline::concat(&chunks).expect("concat should succeed");
        let elapsed = start.elapsed();

        assert_eq!(result.len(), chunk_count * chunk_size);
        assert!(
            elapsed.as_millis() < 3000,
            "Export took {} ms, expected <3000 ms",
            elapsed.as_millis()
        );
    }

    // -----------------------------------------------------------------------
    // Empty chunk payload
    // -----------------------------------------------------------------------

    #[test]
    fn test_export_empty_chunk_payload() {
        // Create a chunk with empty payload — validation should reject it.
        let empty_payload: Vec<u8> = vec![];
        let mut chunk = make_valid_chunk(0, b"not empty");
        chunk.payload = empty_payload;
        // The header still says the original length, so this will fail
        // payload_size mismatch, not empty payload check.
        // Create a properly empty header+payload.
        let header = ChunkHeader::new(0, 0.0, b"");
        let export_chunk = ExportChunk {
            index: 0,
            header,
            payload: vec![],
            status: ChunkStatus::Committed,
        };

        let err = ExportPipeline::concat(&[export_chunk]).unwrap_err();
        assert!(
            matches!(&err, RecordingError::ExportError { details } if details.contains("empty payload")),
            "expected ExportError for empty payload, got {err:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Single chunk
    // -----------------------------------------------------------------------

    #[test]
    fn test_export_single_chunk() {
        let chunks = vec![make_valid_chunk(0, b"single chunk payload")];
        let result = ExportPipeline::concat(&chunks).expect("concat should succeed");
        assert_eq!(result, b"single chunk payload");
    }

    // -----------------------------------------------------------------------
    // validate_sequence specifically
    // -----------------------------------------------------------------------

    #[test]
    fn test_export_validate_sequence_ordered() {
        let chunks = vec![
            make_valid_chunk(0, b"first"),
            make_valid_chunk(1, b"second"),
            make_valid_chunk(2, b"third"),
        ];
        let result = ExportPipeline::validate_sequence(&chunks);
        assert!(result.is_ok(), "validate_sequence should accept ordered chunks");
    }

    #[test]
    fn test_export_validate_sequence_empty() {
        let chunks: Vec<ExportChunk> = vec![];
        let err = ExportPipeline::validate_sequence(&chunks).unwrap_err();
        assert!(
            matches!(&err, RecordingError::EmptySession { details } if details == "No chunks to export"),
            "expected EmptySession for empty sequence, got {err:?}"
        );
    }
}
