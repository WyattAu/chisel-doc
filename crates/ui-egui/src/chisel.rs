//! The Chisel platform, composed into the desktop app: a content-addressed
//! store for document bytes and a tamper-evident trail for what happened to
//! them. Everything recorded here is observational — the filesystem stays the
//! source of truth — so a trail or store failure degrades to a log warning,
//! never to lost work.
//!
//! Transitional: the UI action sites (open/save) call the recorders next;
//! until then the composed surface is exercised by these tests alone, and
//! the module carries a deliberate `dead_code` allow.

use std::path::PathBuf;

use chisel_assets::AssetStore;
use chisel_audit::{AuditEvent, DocumentRef, DocumentTrail, DurableDocumentTrail, VerificationResult};
use chisel_kernel::DesktopSettings;

/// The composed platform: bytes go into the store, events go into the trail.
pub struct Chisel {
    store: AssetStore,
    trail: Trail,
    settings: DesktopSettings,
    actor: String,
}

impl std::fmt::Debug for Chisel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The store holds a blob-cache `Arc` that is not `Debug`; what the
        // caller can act on is the actor and whether a trail is attached.
        f.debug_struct("Chisel")
            .field("actor", &self.actor)
            .field("team_library", &self.settings.team_library)
            .field("trail_attached", &true)
            .finish_non_exhaustive()
    }
}

/// The trail's backing: durable on disk (the desktop), or in memory (tests
/// and previews).
pub enum Trail {
    OnDisk(DurableDocumentTrail),
    InMemory(DocumentTrail),
}

impl Trail {
    /// Append one lifecycle event to the chain.
    async fn record(&self, event: AuditEvent) -> Result<(), chisel_audit::Error> {
        match self {
            Trail::OnDisk(t) => t.record(event).await,
            Trail::InMemory(t) => t.record(event).await,
        }
    }

    /// Verify the whole chain from genesis.
    pub async fn verify(&self) -> Result<VerificationResult, chisel_audit::Error> {
        match self {
            Trail::OnDisk(t) => t.verify().await,
            Trail::InMemory(t) => t.verify().await,
        }
    }
}

impl Chisel {
    /// Opens (creating if needed) the store under `<data>/assets` and the
    /// durable trail at `<data>/trail.jsonl`. Both survive restarts: the
    /// store by content address, the trail by its hash-chained JSONL file —
    /// a corrupt trail file is refused at open, naming the line.
    ///
    /// # Errors
    ///
    /// Propagates the store's open error (an unwritable data directory);
    /// the durable trail's own open surfaces as `ChiselError::Trail`.
    pub fn open(data_dir: impl Into<PathBuf>, config_file: Option<impl Into<PathBuf>>, actor: impl Into<String>) -> Result<Self, ChiselError> {
        let dir: PathBuf = data_dir.into();
        let mut root: PathBuf = dir.clone();
        root.push("assets");
        let cfg: Option<PathBuf> = config_file.map(Into::into);
        let settings = chisel_kernel::DesktopSettings::load(cfg.as_deref()).map_err(ChiselError::Config)?;
        let trail = pollster::block_on(DurableDocumentTrail::on_disk(dir.clone())).map_err(|e| ChiselError::Trail(e.into()))?;
        Ok(Self { store: AssetStore::open(root).map_err(ChiselError::Store)?, trail: Trail::OnDisk(trail), settings, actor: actor.into() })
    }

    /// A composition over in-memory backing: tests, previews, and wasm-style
    /// targets where no data directory exists.
    ///
    /// # Errors
    ///
    /// Propagates the store's constructor error (typed, though the in-memory
    /// backend has no failure mode today).
    pub fn in_memory(actor: impl Into<String>) -> Result<Self, chisel_assets::AssetError> {
        Ok(Self {
            store: AssetStore::open_in_memory()?,
            trail: Trail::InMemory(DocumentTrail::in_memory()),
            settings: DesktopSettings::default(),
            actor: actor.into(),
        })
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
    pub fn trail(&self) -> &Trail {
        &self.trail
    }

    /// The store, for tests and for the service continuation.
    #[must_use]
    pub fn store(&self) -> &AssetStore {
        &self.store
    }

    /// The loaded platform settings.
    #[must_use]
    pub fn settings(&self) -> &DesktopSettings {
        &self.settings
    }

    /// Local-only mode: no team library configured, so nothing leaves the machine.
    #[must_use]
    pub fn local_only(&self) -> bool {
        self.settings.team_library.is_none()
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
    /// The platform settings file exists and failed to parse.
    Config(chisel_kernel::Error),
}

impl std::fmt::Display for ChiselError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChiselError::Store(e) => write!(f, "document store: {e}"),
            ChiselError::Trail(e) => write!(f, "document trail: {e}"),
            ChiselError::Config(e) => write!(f, "platform settings: {e}"),
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
    fn on_disk_composition_survives_a_reopen_and_the_chain_verifies() {
        let dir = std::env::temp_dir().join(format!("chisel-app-on-disk-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        // One session: open, edit, export.
        let chisel = Chisel::open(&dir, None::<PathBuf>, "local").expect("the on-disk composition opens");
        chisel.record_opened("report.pdf", b"original").expect("opened records");
        chisel.record_saved("report.pdf", b"saved bytes").expect("saved records");
        drop(chisel);

        // A brand-new session over the same directory: the store serves the
        // saved bytes and the trail continues from where it left off.
        let chisel = Chisel::open(&dir, None::<PathBuf>, "local").expect("the composition reopens");
        let all = chisel.store().total_size().expect("size");
        assert!(all > 0, "the store persisted: {all}");

        let verification = pollster::block_on(chisel.trail().verify()).expect("verify runs");
        assert!(verification.is_valid(), "{verification:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn settings_load_lenient_and_surface_the_mode() {
        let dir = std::env::temp_dir().join(format!("chisel-app-settings-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create");

        // No settings file: local-only defaults.
        let chisel = Chisel::open(&dir, None::<PathBuf>, "local").expect("opens without a settings file");
        assert!(chisel.local_only(), "no team library configured");

        // A settings file naming a team library surfaces through the accessor.
        let cfg = dir.join("chisel-settings.toml");
        std::fs::write(&cfg, "team_library = \"https://assets.example.com\"\n").expect("write");
        let chisel = Chisel::open(&dir, Some(&cfg), "local").expect("opens with settings");
        assert!(!chisel.local_only());
        assert_eq!(chisel.settings().team_library.as_deref(), Some("https://assets.example.com"));

        // An unknown key is ignored (lenient: an old config must not strand a
        // fresh install), and the composition keeps working.
        std::fs::write(&cfg, "legacy_thing = 1\nteam_library = \"https://x.example\"\n").expect("write");
        let chisel = Chisel::open(&dir, Some(&cfg), "local").expect("lenient load ignores unknown keys");
        assert!(!chisel.local_only());

        // A corrupt settings file is a Config error (the app logs and runs
        // unconfigured rather than refusing to start).
        std::fs::write(&cfg, "team_library = 42\n").expect("write");
        let err = match Chisel::open(&dir, Some(&cfg), "local") {
            Err(ChiselError::Config(_)) => "config error".to_string(),
            other => panic!("expected a config error, got {other:?}"),
        };
        assert!(err.contains("config error"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn on_disk_composition_refuses_a_corrupt_trail_file() {
        let dir = std::env::temp_dir().join(format!("chisel-app-corrupt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("assets")).expect("create");

        // A trail file whose second line is not JSON: the open refuses it,
        // naming the line.
        // Line 1 is a structurally valid entry (the fields the chain needs);
        // line 2 is not JSON — the open refuses the file, naming line 2.
        std::fs::write(
            dir.join("trail.jsonl"),
            concat!(
                "{\"id\":\"00000000-0000-0000-0000-000000000001\",",
                "\"timestamp\":\"2026-10-24T00:00:00Z\",\"actor\":\"a\",",
                "\"action\":\"doc.opened\",\"resource\":\"r\",\"details\":{},",
                "\"previous_hash\":\"0\",\"hash\":\"a\"}\n",
                "NOT-JSON\n"
            ),
        )
        .expect("write");
        let err = Chisel::open(&dir, None::<PathBuf>, "local").expect_err("a corrupt trail is refused at open");
        assert!(err.to_string().contains("line 2"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
