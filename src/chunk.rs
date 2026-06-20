use serde::{Deserialize, Serialize};
use xxhash_rust::xxh3::xxh3_64;

use crate::error::{RecordingError, Result};

// ---------------------------------------------------------------------------
// ChunkStatus
// ---------------------------------------------------------------------------

/// Status of a single chunk in the lifecycle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) enum ChunkStatus {
    /// File created but write not yet validated. Extension: `.partial`.
    Partial,
    /// Write validated and flushed. Extension: `.written`.
    Written,
    /// Manifest committed; chunk is considered durable. Extension: `.bin`.
    Committed,
}

// ---------------------------------------------------------------------------
// ChunkHeader
// ---------------------------------------------------------------------------

/// A fully parsed chunk header (32 bytes on disk).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ChunkHeader {
    pub magic: [u8; 4],       // "CFCH" expected
    pub version: u8,          // 0x01
    pub chunk_index: u32,     // LE
    pub timestamp_ms: f64,    // LE
    pub payload_size: u64,    // LE
    pub checksum: u32,        // XXH3 LE
    pub reserved: [u8; 3],    // zero
}

/// Expected magic bytes: `CFCH`.
const MAGIC: [u8; 4] = *b"CFCH";
const CURRENT_VERSION: u8 = 0x01;

impl ChunkHeader {
    /// Create a new header for the given chunk.
    pub fn new(chunk_index: u32, timestamp_ms: f64, payload: &[u8]) -> Self {
        Self {
            magic: MAGIC,
            version: CURRENT_VERSION,
            chunk_index,
            timestamp_ms,
            payload_size: payload.len() as u64,
            checksum: Self::calc_checksum(payload),
            reserved: [0u8; 3],
        }
    }

    /// Encode header to a 32-byte array.
    ///
    /// Layout:
    ///   [0..4)   magic      (4 bytes)
    ///   [4)      version    (1 byte)
    ///   [5..9)   chunk_index (4 bytes LE)
    ///   [9..17)  timestamp_ms (8 bytes LE)
    ///   [17..25) payload_size (8 bytes LE)
    ///   [25..29) checksum   (4 bytes LE)
    ///   [29..32) reserved   (3 bytes zero)
    pub fn encode(&self) -> [u8; 32] {
        let mut buf = [0u8; 32];

        buf[0..4].copy_from_slice(&self.magic);
        buf[4] = self.version;
        buf[5..9].copy_from_slice(&self.chunk_index.to_le_bytes());
        buf[9..17].copy_from_slice(&self.timestamp_ms.to_le_bytes());
        buf[17..25].copy_from_slice(&self.payload_size.to_le_bytes());
        buf[25..29].copy_from_slice(&self.checksum.to_le_bytes());
        // buf[29..32] is already zero

        buf
    }

    /// Decode header from a 32-byte array.
    ///
    /// Returns `WriteError` if magic bytes don't match or version is unknown.
    pub fn decode(bytes: &[u8; 32]) -> Result<Self> {
        let magic: [u8; 4] = bytes[0..4].try_into().expect("invariant: slice is 4 bytes");
        if magic != MAGIC {
            return Err(RecordingError::WriteError {
                details: format!(
                    "Invalid chunk header magic: expected CFCH, got {magic:02x?}",
                ),
            });
        }

        let version = bytes[4];
        if version != CURRENT_VERSION {
            return Err(RecordingError::WriteError {
                details: format!("Unsupported chunk header version: {version}"),
            });
        }

        let chunk_index = u32::from_le_bytes(
            bytes[5..9].try_into().expect("invariant: slice is 4 bytes"),
        );
        let timestamp_ms = f64::from_le_bytes(
            bytes[9..17].try_into().expect("invariant: slice is 8 bytes"),
        );
        let payload_size = u64::from_le_bytes(
            bytes[17..25].try_into().expect("invariant: slice is 8 bytes"),
        );
        let checksum = u32::from_le_bytes(
            bytes[25..29].try_into().expect("invariant: slice is 4 bytes"),
        );

        let reserved: [u8; 3] = bytes[29..32]
            .try_into()
            .expect("invariant: slice is 3 bytes");

        Ok(Self {
            magic,
            version,
            chunk_index,
            timestamp_ms,
            payload_size,
            checksum,
            reserved,
        })
    }

    /// Verify the XXH3 checksum against the payload.
    pub fn verify_checksum(&self, payload: &[u8]) -> bool {
        self.checksum == Self::calc_checksum(payload)
    }

    /// Recalculate the XXH3 checksum for a payload.
    ///
    /// `xxh3_64` returns a `u64`; we take the lower 32 bits.
    pub(crate) fn calc_checksum(payload: &[u8]) -> u32 {
        (xxh3_64(payload) & 0xFFFF_FFFF) as u32
    }
}

// ---------------------------------------------------------------------------
// ManifestEntry & ChunkManifest
// ---------------------------------------------------------------------------

/// One entry in the in-memory chunk manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ManifestEntry {
    pub chunk_index: u32,
    pub payload_size: u64,
    pub checksum: u32,
    pub status: ChunkStatus,
    pub timestamp_ms: f64,
}

/// In-memory manifest tracking all chunks for a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ChunkManifest {
    pub session_id: String,
    pub entries: Vec<ManifestEntry>,
}

impl ChunkManifest {
    /// Create a new empty manifest for a session.
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
            entries: Vec::new(),
        }
    }

    /// Append a new entry to the manifest.
    pub fn add_entry(&mut self, entry: ManifestEntry) {
        self.entries.push(entry);
    }

    /// Update the status of a chunk entry by index.
    ///
    /// Returns `WriteError` if no entry with the given chunk_index exists.
    pub fn update_status(&mut self, chunk_index: u32, status: ChunkStatus) -> Result<()> {
        let entry = self
            .entries
            .iter_mut()
            .find(|e| e.chunk_index == chunk_index)
            .ok_or_else(|| RecordingError::WriteError {
                details: format!(
                    "Cannot update status for chunk index {chunk_index}: entry not found",
                ),
            })?;

        entry.status = status;
        Ok(())
    }

    /// Return the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return true when no entries exist.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---------------------------------------------------------------------------
// ChunkStorage trait
// ---------------------------------------------------------------------------

/// Abstract storage interface — implemented by mock backend for tests and
/// by OPFS in production (WASM-only).
pub(crate) trait ChunkStorage {
    /// Write header + payload to the given path.
    fn write_chunk(&mut self, path: &str, header: &[u8; 32], payload: &[u8]) -> Result<()>;
    /// Rename a chunk from one extension to another (e.g., .partial → .written).
    fn rename_chunk(&mut self, from: &str, to: &str) -> Result<()>;
}

// ---------------------------------------------------------------------------
// MockChunkStorage
// ---------------------------------------------------------------------------

/// A mock storage backend that stores chunks in a `Vec<u8>` buffer.
///
/// Used in native tests. Tracks writes and renames in memory.
#[derive(Debug, Clone)]
pub(crate) struct MockChunkStorage {
    /// Maps file path → stored bytes (header + payload).
    chunks: Vec<(String, Vec<u8>)>,
}

impl MockChunkStorage {
    pub fn new() -> Self {
        Self { chunks: Vec::new() }
    }
}

impl ChunkStorage for MockChunkStorage {
    fn write_chunk(&mut self, path: &str, header: &[u8; 32], payload: &[u8]) -> Result<()> {
        let mut data = Vec::with_capacity(32 + payload.len());
        data.extend_from_slice(header);
        data.extend_from_slice(payload);

        self.chunks.push((path.to_string(), data));
        Ok(())
    }

    fn rename_chunk(&mut self, from: &str, to: &str) -> Result<()> {
        let idx = self
            .chunks
            .iter()
            .position(|(path, _)| path == from)
            .ok_or_else(|| RecordingError::WriteError {
                details: format!("Cannot rename {from}: chunk not found"),
            })?;

        self.chunks[idx].0 = to.to_string();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// OpfsChunkStorage — WASM scaffold (Story 2.x)
// ---------------------------------------------------------------------------

/// OPFS-backed chunk storage.
///
/// This is a structural scaffold for V0.1. The full OPFS write path will be
/// implemented in Story 2.1 (Session Manifest & Storage Layout). For now,
/// all native tests use `MockChunkStorage`.
#[cfg(target_arch = "wasm32")]
pub(crate) struct OpfsChunkStorage {
    session_id: String,
}

#[cfg(target_arch = "wasm32")]
impl OpfsChunkStorage {
    /// Initialise OPFS root directory handle (scaffold).
    pub async fn init(session_id: &str) -> Result<Self> {
        // TODO(Story 2.1): Open OPFS root via
        //   `navigator.storage.getDirectory()` and store the handle.
        Ok(Self {
            session_id: session_id.to_string(),
        })
    }
}

#[cfg(target_arch = "wasm32")]
impl ChunkStorage for OpfsChunkStorage {
    fn write_chunk(&mut self, _path: &str, _header: &[u8; 32], _payload: &[u8]) -> Result<()> {
        // TODO(Story 2.1): Write through OPFS `FileSystemWritableFileStream`.
        Err(RecordingError::WriteError {
            details: "OPFS write_chunk not yet implemented (Story 2.1)".into(),
        })
    }

    fn rename_chunk(&mut self, _from: &str, _to: &str) -> Result<()> {
        // TODO(Story 2.1): Rename via OPFS `FileSystemDirectoryHandle`.
        Err(RecordingError::WriteError {
            details: "OPFS rename_chunk not yet implemented (Story 2.1)".into(),
        })
    }
}

// ---------------------------------------------------------------------------
// ChunkWriter
// ---------------------------------------------------------------------------

/// Maximum number of chunks per session.
///
/// The 6-digit zero-padded filename (e.g., `chunk_999999.bin`) supports up
/// to 999,999 unique indices. Writing beyond this limit is an error.
const MAX_CHUNK_INDEX: u32 = 999_999;

/// Writes MediaRecorder blobs to OPFS with the binary header and manages
/// the chunk lifecycle (.partial → .written → .bin).
pub(crate) struct ChunkWriter {
    session_id: String,
    manifest: ChunkManifest,
    next_chunk_index: u32,
    storage: Box<dyn ChunkStorage>,
}

impl ChunkWriter {
    /// Create a new ChunkWriter with the given storage backend.
    pub fn new(session_id: String, storage: Box<dyn ChunkStorage>) -> Self {
        Self {
            manifest: ChunkManifest::new(session_id.clone()),
            session_id,
            next_chunk_index: 0,
            storage,
        }
    }

    /// Write a MediaRecorder blob as a chunk.
    ///
    /// 1. Validate chunk index, timestamp, and payload.
    /// 2. Build header.
    /// 3. Write `chunk_{index:06}.partial`.
    /// 4. Promote to `.written`.
    /// 5. Add a manifest entry.
    pub fn write_blob(&mut self, blob: &[u8], timestamp_ms: f64) -> Result<()> {
        // Reject empty payloads.
        if blob.is_empty() {
            return Err(RecordingError::WriteError {
                details: format!(
                    "Cannot write empty chunk (index {})",
                    self.next_chunk_index,
                ),
            });
        }

        // Validate timestamp is finite and non-negative.
        if !timestamp_ms.is_finite() || timestamp_ms < 0.0 {
            return Err(RecordingError::WriteError {
                details: format!(
                    "Invalid timestamp for chunk index {}: {timestamp_ms}",
                    self.next_chunk_index,
                ),
            });
        }

        // Validate chunk index does not overflow the filename scheme.
        if self.next_chunk_index > MAX_CHUNK_INDEX {
            return Err(RecordingError::WriteError {
                details: format!(
                    "Chunk index {} exceeds maximum ({})",
                    self.next_chunk_index, MAX_CHUNK_INDEX,
                ),
            });
        }

        let index = self.next_chunk_index;
        let header = ChunkHeader::new(index, timestamp_ms, blob);
        let header_bytes = header.encode();

        // Verify header payload_size matches actual payload length.
        assert_eq!(
            header.payload_size,
            blob.len() as u64,
            "invariant: header payload_size must match blob length"
        );

        // Write as .partial — pass the path to storage so there's no ambiguity.
        let partial_path = format!("chunk_{index:06}.partial");
        self.storage.write_chunk(&partial_path, &header_bytes, blob)?;

        // Promote to .written.
        let written_path = format!("chunk_{index:06}.written");
        self.storage.rename_chunk(&partial_path, &written_path)?;

        // Add manifest entry (AC5: check for duplicate index first).
        if self.manifest.entries.iter().any(|e| e.chunk_index == index) {
            return Err(RecordingError::WriteError {
                details: format!("Duplicate chunk index {index} in manifest"),
            });
        }
        let entry = ManifestEntry {
            chunk_index: index,
            payload_size: blob.len() as u64,
            checksum: header.checksum,
            status: ChunkStatus::Written,
            timestamp_ms,
        };
        self.manifest.add_entry(entry);

        self.next_chunk_index += 1;
        Ok(())
    }

    /// Commit the current chunk: promote from `.written` to `.bin`.
    ///
    /// Calling `commit_chunk()` when no chunks have been written is a no-op
    /// (there is nothing to commit). Calling it multiple times for the same
    /// chunk is also a no-op (idempotent).
    pub fn commit_chunk(&mut self) -> Result<()> {
        if self.next_chunk_index == 0 {
            return Ok(());
        }

        let index = self.next_chunk_index - 1;

        // Idempotency: skip if already committed.
        if self
            .manifest
            .entries
            .iter()
            .any(|e| e.chunk_index == index && e.status == ChunkStatus::Committed)
        {
            return Ok(());
        }

        // Rename file first, then update manifest.
        let written_path = format!("chunk_{index:06}.written");
        let bin_path = format!("chunk_{index:06}.bin");
        self.storage.rename_chunk(&written_path, &bin_path)?;
        self.manifest
            .update_status(index, ChunkStatus::Committed)?;
        Ok(())
    }

    /// Return the in-memory manifest.
    pub fn manifest(&self) -> &ChunkManifest {
        &self.manifest
    }

    /// Return the next expected file path for a given status extension.
    /// Note: this returns the path for the *next* (unwritten) chunk,
    /// not a chunk that has already been written.
    pub fn next_chunk_path(&self, status: &str) -> String {
        format!("chunk_{:06}.{}", self.next_chunk_index, status)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Header encode / decode
    // -----------------------------------------------------------------------

    #[test]
    fn test_header_new() {
        let payload = b"hello world";
        let header = ChunkHeader::new(0, 12345.0, payload);

        assert_eq!(&header.magic, b"CFCH");
        assert_eq!(header.version, 0x01);
        assert_eq!(header.chunk_index, 0);
        assert_eq!(header.payload_size, 11);
        assert_eq!(header.reserved, [0u8; 3]);

        // checksum should be non-zero for non-empty payload
        assert_ne!(header.checksum, 0);
    }

    #[test]
    fn test_header_encode_decode_roundtrip() {
        let payload = b"some recording data";
        let original = ChunkHeader::new(42, 98765.4321, payload);

        let encoded = original.encode();
        let decoded = ChunkHeader::decode(&encoded).expect("decode should succeed");

        assert_eq!(original, decoded);
    }

    #[test]
    fn test_header_decode_invalid_magic() {
        let mut encoded = ChunkHeader::new(0, 0.0, b"x").encode();
        // Corrupt magic bytes
        encoded[0..4].copy_from_slice(b"BAD!");
        let err = ChunkHeader::decode(&encoded).unwrap_err();
        assert!(
            matches!(&err, RecordingError::WriteError { details } if details.contains("Invalid chunk header magic")),
            "expected WriteError with magic mismatch, got {err:?}"
        );
    }

    #[test]
    fn test_header_decode_invalid_version() {
        let mut header = ChunkHeader::new(0, 0.0, b"x");
        header.version = 0xFF;
        let encoded = header.encode();
        let err = ChunkHeader::decode(&encoded).unwrap_err();
        assert!(
            matches!(&err, RecordingError::WriteError { details } if details.contains("Unsupported chunk header version")),
            "expected WriteError with version mismatch, got {err:?}"
        );
    }

    #[test]
    fn test_header_payload_size_match() {
        let payload = b"exact size check";
        let header = ChunkHeader::new(7, 100.0, payload);
        assert_eq!(header.payload_size, payload.len() as u64);

        let encoded = header.encode();
        let decoded = ChunkHeader::decode(&encoded).expect("decode should succeed");
        assert_eq!(decoded.payload_size, payload.len() as u64);
    }

    // -----------------------------------------------------------------------
    // Checksum
    // -----------------------------------------------------------------------

    #[test]
    fn test_checksum_valid() {
        let payload = b"verify me";
        let header = ChunkHeader::new(0, 0.0, payload);
        assert!(header.verify_checksum(payload));
    }

    #[test]
    fn test_checksum_corrupted() {
        let payload = b"original data";
        let header = ChunkHeader::new(0, 0.0, payload);
        assert!(!header.verify_checksum(b"tampered data"));
    }

    #[test]
    fn test_checksum_empty_payload() {
        let payload = b"";
        let header = ChunkHeader::new(0, 0.0, payload);
        // Empty payload should still produce a valid (non-zero) checksum.
        assert_ne!(header.checksum, 0);
        assert!(header.verify_checksum(b""));
    }

    // -----------------------------------------------------------------------
    // Manifest
    // -----------------------------------------------------------------------

    #[test]
    fn test_manifest_new() {
        let m = ChunkManifest::new("session-1".into());
        assert_eq!(m.session_id, "session-1");
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn test_manifest_add_entry() {
        let mut m = ChunkManifest::new("s".into());
        m.add_entry(ManifestEntry {
            chunk_index: 0,
            payload_size: 100,
            checksum: 0xDEAD,
            status: ChunkStatus::Partial,
            timestamp_ms: 1.0,
        });
        assert_eq!(m.len(), 1);
        assert!(!m.is_empty());
    }

    #[test]
    fn test_manifest_update_status() {
        let mut m = ChunkManifest::new("s".into());
        m.add_entry(ManifestEntry {
            chunk_index: 0,
            payload_size: 100,
            checksum: 0,
            status: ChunkStatus::Partial,
            timestamp_ms: 1.0,
        });

        m.update_status(0, ChunkStatus::Written)
            .expect("update should succeed");
        assert_eq!(m.entries[0].status, ChunkStatus::Written);

        m.update_status(0, ChunkStatus::Committed)
            .expect("update should succeed");
        assert_eq!(m.entries[0].status, ChunkStatus::Committed);
    }

    #[test]
    fn test_manifest_update_nonexistent() {
        let mut m = ChunkManifest::new("s".into());
        let err = m
            .update_status(99, ChunkStatus::Written)
            .unwrap_err();
        assert!(
            matches!(&err, RecordingError::WriteError { details } if details.contains("entry not found")),
            "expected WriteError for missing entry, got {err:?}"
        );
    }

    // -----------------------------------------------------------------------
    // MockChunkStorage
    // -----------------------------------------------------------------------

    #[test]
    fn test_mock_storage_write() {
        let mut storage = MockChunkStorage::new();
        let header = ChunkHeader::new(0, 1.0, b"payload").encode();
        storage
            .write_chunk("chunk_000000.partial", &header, b"payload")
            .expect("write should succeed");

        // Verify the stored size: 32-byte header + 7-byte payload
        assert_eq!(storage.chunks.len(), 1);
        assert_eq!(storage.chunks[0].0, "chunk_000000.partial");
        assert_eq!(storage.chunks[0].1.len(), 32 + 7);
    }

    #[test]
    fn test_mock_storage_rename() {
        let mut storage = MockChunkStorage::new();
        let header = ChunkHeader::new(0, 1.0, b"data").encode();
        storage
            .write_chunk("chunk_000000.partial", &header, b"data")
            .unwrap();

        storage
            .rename_chunk("chunk_000000.partial", "chunk_000000.written")
            .expect("rename should succeed");

        assert_eq!(storage.chunks[0].0, "chunk_000000.written");
    }

    #[test]
    fn test_mock_storage_rename_nonexistent() {
        let mut storage = MockChunkStorage::new();
        let err = storage
            .rename_chunk("chunk_999999.partial", "chunk_999999.written")
            .unwrap_err();
        assert!(
            matches!(&err, RecordingError::WriteError { details } if details.contains("chunk not found")),
            "expected WriteError for missing chunk, got {err:?}"
        );
    }

    // -----------------------------------------------------------------------
    // ChunkWriter
    // -----------------------------------------------------------------------

    #[test]
    fn test_writer_new() {
        let storage = Box::new(MockChunkStorage::new());
        let writer = ChunkWriter::new("session-w".into(), storage);
        assert_eq!(writer.next_chunk_index, 0);
        assert!(writer.manifest().is_empty());
    }

    #[test]
    fn test_writer_write_blob() {
        let mut writer = ChunkWriter::new("s".into(), Box::new(MockChunkStorage::new()));
        writer
            .write_blob(b"some media data", 1000.0)
            .expect("write should succeed");

        assert_eq!(writer.next_chunk_index, 1);
        assert_eq!(writer.manifest().len(), 1);

        let entry = &writer.manifest().entries[0];
        assert_eq!(entry.chunk_index, 0);
        assert_eq!(entry.payload_size, 15);
        assert_eq!(entry.status, ChunkStatus::Written);
        assert_eq!(entry.timestamp_ms, 1000.0);
        assert_ne!(entry.checksum, 0);
    }

    #[test]
    fn test_writer_commit_chunk() {
        let mut writer = ChunkWriter::new("s".into(), Box::new(MockChunkStorage::new()));
        writer.write_blob(b"data", 1.0).unwrap();

        writer.commit_chunk().expect("commit should succeed");
        assert_eq!(
            writer.manifest().entries[0].status,
            ChunkStatus::Committed
        );
    }

    #[test]
    fn test_writer_chunk_naming() {
        let storage = Box::new(MockChunkStorage::new());
        let writer = ChunkWriter::new("s".into(), storage);

        assert_eq!(writer.next_chunk_path("partial"), "chunk_000000.partial");
        assert_eq!(writer.next_chunk_path("written"), "chunk_000000.written");
        assert_eq!(writer.next_chunk_path("bin"), "chunk_000000.bin");
    }

    #[test]
    fn test_writer_empty_blob_rejected() {
        let mut writer = ChunkWriter::new("s".into(), Box::new(MockChunkStorage::new()));

        let err = writer.write_blob(b"", 0.0).unwrap_err();
        assert!(
            matches!(&err, RecordingError::WriteError { details } if details.contains("Cannot write empty chunk")),
            "expected WriteError for empty blob, got {err:?}"
        );
        // No state change
        assert_eq!(writer.next_chunk_index, 0);
        assert!(writer.manifest().is_empty());
    }

    #[test]
    fn test_writer_multiple_chunks() {
        let mut writer = ChunkWriter::new("s".into(), Box::new(MockChunkStorage::new()));

        for i in 0..5 {
            writer
                .write_blob(&[i as u8; 100], i as f64 * 1000.0)
                .expect("write should succeed");
        }

        assert_eq!(writer.next_chunk_index, 5);
        assert_eq!(writer.manifest().len(), 5);

        for (i, entry) in writer.manifest().entries.iter().enumerate() {
            assert_eq!(entry.chunk_index, i as u32);
            assert_eq!(entry.payload_size, 100);
        }
    }

    #[test]
    fn test_writer_commit_noop_when_empty() {
        let mut writer = ChunkWriter::new("s".into(), Box::new(MockChunkStorage::new()));
        // Committing with no chunks should be a no-op (not an error).
        writer.commit_chunk().expect("commit on empty writer should be ok");
        assert!(writer.manifest().is_empty());
    }

    /// Verify write_blob + commit_chunk at the storage level: actual file
    /// paths and contents in MockChunkStorage.
    #[test]
    fn test_writer_storage_integration() {
        let mut writer = ChunkWriter::new("s".into(), Box::new(MockChunkStorage::new()));

        // Write three blobs
        for i in 0..3 {
            writer
                .write_blob(&[i as u8; 100], i as f64 * 1000.0)
                .expect("write should succeed");
        }

        // Check manifest: all three chunks should be at .written
        assert_eq!(writer.manifest().len(), 3);
        for entry in writer.manifest().entries.iter() {
            assert_eq!(entry.status, ChunkStatus::Written);
        }

        // Commit the last chunk
        writer.commit_chunk().expect("commit should succeed");
        assert_eq!(
            writer.manifest().entries[2].status,
            ChunkStatus::Committed
        );
        assert_eq!(
            writer.manifest().entries[0].status,
            ChunkStatus::Written
        );

        // Second commit should be a no-op (idempotency)
        writer.commit_chunk().expect("second commit should be no-op");
        assert_eq!(
            writer.manifest().entries[2].status,
            ChunkStatus::Committed
        );
    }

    /// Verify that invalid timestamps are rejected.
    #[test]
    fn test_writer_invalid_timestamp_rejected() {
        let mut writer = ChunkWriter::new("s".into(), Box::new(MockChunkStorage::new()));

        let err = writer.write_blob(b"data", f64::NAN).unwrap_err();
        assert!(
            matches!(&err, RecordingError::WriteError { details } if details.contains("Invalid timestamp")),
            "expected WriteError for NaN timestamp, got {err:?}"
        );

        let err = writer.write_blob(b"data", f64::INFINITY).unwrap_err();
        assert!(
            matches!(&err, RecordingError::WriteError { details } if details.contains("Invalid timestamp")),
            "expected WriteError for Infinity timestamp, got {err:?}"
        );

        let err = writer.write_blob(b"data", -1.0).unwrap_err();
        assert!(
            matches!(&err, RecordingError::WriteError { details } if details.contains("Invalid timestamp")),
            "expected WriteError for negative timestamp, got {err:?}"
        );

        // Valid timestamp should succeed
        writer
            .write_blob(b"data", 0.0)
            .expect("valid zero timestamp should succeed");
        writer
            .write_blob(b"data", 1.0)
            .expect("valid positive timestamp should succeed");
    }

    #[test]
    fn test_writer_index_overflow() {
        let mut writer = ChunkWriter::new("s".into(), Box::new(MockChunkStorage::new()));
        // Set the writer to the max index + 1 by writing MANY chunks — but
        // that's impractical with MockChunkStorage (O(n) path). Instead we
        // test the boundary by setting next_chunk_index directly.
        writer.next_chunk_index = MAX_CHUNK_INDEX + 1;

        let err = writer.write_blob(b"data", 0.0).unwrap_err();
        assert!(
            matches!(&err, RecordingError::WriteError { details } if details.contains("exceeds maximum")),
            "expected WriteError for overflow, got {err:?}"
        );
    }

    #[test]
    fn test_header_roundtrip_multiple_headers() {
        // Verify a diverse set of headers roundtrip correctly.
        let cases: Vec<(u32, f64, &[u8])> = vec![
            (0, 0.0, b""),
            (1, 1.0, b"a"),
            (42, 123456.789, b"some longer payload data"),
            (999_999, 999999.999, &[0xFF; 256]),
        ];

        for (index, ts, payload) in cases {
            let header = ChunkHeader::new(index, ts, payload);
            let encoded = header.encode();
            let decoded = ChunkHeader::decode(&encoded).expect("roundtrip should succeed");
            assert_eq!(header, decoded, "roundtrip failed for index={index}, ts={ts}");
        }
    }

    /// Ensure that header_roundtrip_multiple_headers correctly tests empty
    /// payload (the comment says it does but let's be explicit).
    #[test]
    fn test_header_empty_payload_roundtrip() {
        let header = ChunkHeader::new(0, 0.0, b"");
        let encoded = header.encode();
        let decoded = ChunkHeader::decode(&encoded).expect("decode should succeed");
        assert_eq!(header, decoded);
        assert_eq!(header.payload_size, 0);
    }
}

// ---------------------------------------------------------------------------
// WASM tests — require `wasm-pack test --headless --chrome`
// ---------------------------------------------------------------------------

#[cfg(all(test, target_arch = "wasm32"))]
mod wasm_tests {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    /// OpfsChunkStorage scaffold initialisation (may fail gracefully until
    /// Story 2.1 provides the full OPFS write path).
    #[wasm_bindgen_test]
    async fn test_opfs_storage_init_scaffold() {
        let result = OpfsChunkStorage::init("test-session").await;
        // In V0.1 the scaffold returns an Ok with no OPFS handle.
        // If the environment lacks OPFS it may still succeed structurally.
        // This test exists to verify the scaffold compiles and runs.
        assert!(result.is_ok(), "OpfsChunkStorage::init should not panic");
    }

    /// OpfsChunkStorage write method returns the expected "not implemented"
    /// error in V0.1 (the full OPFS path is deferred to Story 2.1).
    #[wasm_bindgen_test]
    async fn test_opfs_write_in_headless_scaffold() {
        let mut storage = OpfsChunkStorage::init("test-session")
            .await
            .expect("init should succeed");
        let header = ChunkHeader::new(0, 0.0, b"data").encode();

        let err = storage.write_chunk("chunk_000000.partial", &header, b"data").unwrap_err();
        assert!(
            matches!(&err, RecordingError::WriteError { details } if details.contains("not yet implemented")),
            "expected 'not yet implemented' error, got {err:?}"
        );
    }
}
