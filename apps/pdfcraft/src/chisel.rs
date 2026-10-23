//! The Chisel platform, composed into the desktop app: a content-addressed
//! store for document bytes and a tamper-evident trail for what happened to
//! them. Everything recorded here is observational — the filesystem stays the
//! source of truth — so a trail or store failure degrades to a log warning,
//! never to lost work.
//!
//! Transitional: the UI action sites (open/save) call the recorders next;
//! until then the composed surface is exercised by these tests alone, and
//! the module carries a deliberate `dead_code` allow.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use chisel_assets::AssetStore;
use chisel_audit::{AuditEvent, DocumentRef, DocumentTrail};

/// The composed platform: bytes go into the store, events go into the trail.
pub struct Chisel {
    store: AssetStore,
    trail: DocumentTrail,
    actor: String,
}

impl std::fmt::Debug for Chisel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Chisel").field("actor", &self.actor).finish_non_exhaustive()
    }
}

impl Chisel {
    /// Opens (creating if needed) the store under `<data>/assets` and an
    /// in-memory trail for this session. The service continuation takes the
    /// trail over on publish; the store is the durable half.
    ///
    /// # Errors
    ///
    /// Propagates the store's open error (an unwritable data directory).
    pub fn open(data_dir: &Path, actor: impl Into<String>) -> Result<Self, chisel_assets::AssetError> {
        let mut root: PathBuf = data_dir.to_path_buf();
        root.push("assets");
        Ok(Self { store: AssetStore::open(root)?, trail: DocumentTrail::in_memory(), actor: actor.into() })
    }

    /// A composition over in-memory backing: tests, previews, and wasm-style
    /// targets where no data directory exists.
    ///
    /// # Errors
    ///
    /// Propagates the store's constructor error (typed, though the in-memory
    /// backend has no failure mode today).
    pub fn in_memory(actor: impl Into<String>) -> Result<Self, chisel_assets::AssetError> {
        Ok(Self { store: AssetStore::open_in_memory()?, trail: DocumentTrail::in_memory(), actor: actor.into() })
    }

    /// Records a document opening: the bytes enter the store (addressed by
    /// their hash) and the trail gains an `opened` entry. Failures are
    /// returned for the caller to log — never to block the open.
    ///
    /// # Errors
    ///
    /// The store rejected the bytes, or the trail refused the append.
    pub fn record_opened(&self, name: &str, bytes: &[u8]) -> Result<StoredRecord, ChiselError> {
        let stored = self.store.put(bytes).map_err(ChiselError::Store)?;
        let event = AuditEvent::opened(&self.actor, &DocumentRef::new(name));
        pollster::block_on(self.trail.record(event)).map_err(ChiselError::Trail)?;
        Ok(StoredRecord { stored, bytes: bytes.len() })
    }

    /// Records saved bytes: a fresh store entry (the new content hash) and an
    /// `exported` entry — the document left the editing session as bytes.
    ///
    /// # Errors
    ///
    /// The store rejected the bytes, or the trail refused the append.
    pub fn record_saved(&self, name: &str, bytes: &[u8]) -> Result<StoredRecord, ChiselError> {
        let stored = self.store.put(bytes).map_err(ChiselError::Store)?;
        let event = AuditEvent::exported(&self.actor, &DocumentRef::new(name), "pdf");
        pollster::block_on(self.trail.record(event)).map_err(ChiselError::Trail)?;
        Ok(StoredRecord { stored, bytes: bytes.len() })
    }

    /// The trail, for the service continuation and for tests.
    #[must_use]
    pub fn trail(&self) -> &DocumentTrail {
        &self.trail
    }

    /// The store, for tests and for the service continuation.
    #[must_use]
    pub fn store(&self) -> &AssetStore {
        &self.store
    }
}

/// What one recorded lifecycle step produced.
#[derive(Debug, Clone)]
pub struct StoredRecord {
    /// The content-addressed entry.
    pub stored: chisel_assets::StoredDocument,
    /// The byte length that was stored.
    pub bytes: usize,
}

/// A composition failure: the store or the trail refused.
#[derive(Debug)]
pub enum ChiselError {
    /// The content-addressed store rejected the operation.
    Store(chisel_assets::AssetError),
    /// The trail refused the append.
    Trail(chisel_audit::Error),
}

impl std::fmt::Display for ChiselError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChiselError::Store(e) => write!(f, "document store: {e}"),
            ChiselError::Trail(e) => write!(f, "document trail: {e}"),
        }
    }
}

impl std::error::Error for ChiselError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_and_save_land_in_the_store_and_the_chain_verifies() {
        let chisel = Chisel::in_memory("local").expect("the in-memory composition opens");
        let bytes: &[u8] = b"%PDF-1.7 original document bytes";
        let opened = chisel.record_opened("report.pdf", bytes).expect("opened records");
        assert_eq!(opened.bytes, bytes.len());
        assert!(chisel.store().contains(&opened.stored));

        let saved_bytes: &[u8] = b"%PDF-1.7 saved document bytes";
        let saved = chisel.record_saved("report.pdf", saved_bytes).expect("saved records");
        assert_ne!(saved.stored.hash(), opened.stored.hash(), "different content, different hash");

        // The store reads the saved bytes back, verified on read.
        assert_eq!(chisel.store().read(&saved.stored).expect("read"), saved_bytes);

        // The chain verifies end to end (the genesis anchor counts as an entry).
        let verification = pollster::block_on(chisel.trail().verify()).expect("verify runs");
        assert!(verification.is_valid(), "{verification:?}");
        assert_eq!(verification.valid_entries, 3, "anchor + opened + saved");
    }

    #[test]
    fn on_disk_composition_opens_and_creates_the_store() {
        let dir = std::env::temp_dir().join(format!("chisel-app-test-{}", std::process::id()));
        let chisel = Chisel::open(&dir, "local").expect("the store opens");
        chisel.record_opened("a.pdf", b"bytes").expect("records");
        // A second open sees the same store (the bytes are still there).
        let again = Chisel::open(&dir, "local").expect("reopen");
        assert!(again.store().total_size().expect("size") > 0, "the store persisted");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
