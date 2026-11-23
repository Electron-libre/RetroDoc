//! The files of human work under `.retrodoc/`, outside the cache (ADR 0044, ADR 0045): the reviews
//! and the edits of the docs sections, each one versioned JSON file of records keyed by section,
//! loaded and saved by the same code.

use std::io::{ErrorKind, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::de::DeserializeOwned;
use serde::ser::SerializeMap as _;
use serde::{Deserialize, Serialize};

pub use crate::history::SectionKind;

/// A section of the docs, as the run history names it (RHI-7).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Section {
    pub kind: SectionKind,
    /// The slug, `<feature>/<use case>` for a use case.
    pub id: String,
}

impl Section {
    #[must_use]
    pub fn new(kind: SectionKind, id: impl Into<String>) -> Self {
        Self {
            kind,
            id: id.into(),
        }
    }

    /// The use case `slug` of the feature `feature`: its id is `<feature>/<use case>`.
    #[must_use]
    pub fn use_case(feature: &str, slug: &str) -> Self {
        Self::new(SectionKind::UseCase, format!("{feature}/{slug}"))
    }
}

/// What a human said or wrote of one section.
pub trait Record: Clone + Serialize + DeserializeOwned {
    fn section(&self) -> &Section;

    /// When it was made, RFC 3339: of several records of a section, the newest is kept.
    fn made_at(&self) -> &str;
}

/// The records of a repository, one per section at most, by kind then id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Records<R> {
    records: Vec<R>,
}

impl<R> Default for Records<R> {
    fn default() -> Self {
        Self {
            records: Vec::new(),
        }
    }
}

impl<R: Record> Records<R> {
    #[must_use]
    pub fn all(&self) -> &[R] {
        &self.records
    }

    #[must_use]
    pub fn get(&self, section: &Section) -> Option<&R> {
        self.records.iter().find(|r| r.section() == section)
    }

    /// Sets `record`, replacing the former record of its section.
    pub(crate) fn set(&mut self, record: R) {
        self.unset(record.section());
        let at = self
            .records
            .partition_point(|r| r.section() < record.section());
        self.records.insert(at, record);
    }

    pub(crate) fn unset(&mut self, section: &Section) {
        self.records.retain(|r| r.section() != section);
    }
}

/// The kind as the files name it.
pub(crate) fn kind_key(kind: SectionKind) -> &'static str {
    match kind {
        SectionKind::Domain => "domain",
        SectionKind::Feature => "feature",
        SectionKind::UseCase => "use_case",
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("{} cannot be read: {reason}", path.display())]
    Unreadable { path: PathBuf, reason: String },
    #[error("{} is of format {version}, this RetroDoc reads up to {supported}", path.display())]
    LaterFormat {
        path: PathBuf,
        version: u32,
        supported: u32,
    },
    #[error("{} cannot be saved: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// The file of records stored at `path`, of format `version` at most, the records under the key
/// `key`.
pub(crate) struct Store {
    pub path: PathBuf,
    pub version: u32,
    pub key: &'static str,
}

/// The version of a file, read before the rest.
#[derive(Deserialize)]
struct Version {
    version: u32,
}

impl Store {
    /// The records saved, none when no file is.
    pub fn load<R: Record>(&self) -> Result<Records<R>, StoreError> {
        let path = &self.path;
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Records::default()),
            Err(e) => return Err(unreadable(path, &e)),
        };
        // The version first: a later format may not parse as this one.
        let version = serde_json::from_str::<Version>(&text)
            .map_err(|e| unreadable(path, &e))?
            .version;
        if version > self.version {
            return Err(StoreError::LaterFormat {
                path: path.clone(),
                version,
                supported: self.version,
            });
        }
        let mut file: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&text).map_err(|e| unreadable(path, &e))?;
        let records = file
            .remove(self.key)
            .ok_or_else(|| unreadable(path, &format!("missing field `{}`", self.key)))?;
        let mut records: Vec<R> =
            serde_json::from_value(records).map_err(|e| unreadable(path, &e))?;
        // The newest record of a section first, then the others of that section dropped: a file
        // merged by hand may hold several.
        records.sort_by(|a, b| {
            a.section()
                .cmp(b.section())
                .then_with(|| b.made_at().cmp(a.made_at()))
        });
        records.dedup_by(|later, kept| later.section() == kept.section());
        Ok(Records { records })
    }

    /// Loads the records, applies `change` and saves them, one change of the process at a time;
    /// `change` returns whether anything changed, and nothing is saved when not.
    pub fn change<R: Record, T>(
        &self,
        change: impl FnOnce(&mut Records<R>) -> (bool, T),
    ) -> Result<T, StoreError> {
        let saving = SAVING.lock().unwrap_or_else(PoisonError::into_inner);
        let mut records = self.load()?;
        let (changed, value) = change(&mut records);
        if changed {
            self.save(&records, &saving)?;
        }
        Ok(value)
    }

    /// Writes `records` whole aside, then renames it over the former file, so no reader sees a
    /// file cut short. Called after a [`Store::load`] that succeeded, so it never replaces a file
    /// it can't read.
    fn save<R: Record>(
        &self,
        records: &Records<R>,
        _held: &MutexGuard<'_, ()>,
    ) -> Result<(), StoreError> {
        self.write(records).map_err(|source| StoreError::Io {
            path: self.path.clone(),
            source,
        })
    }

    fn write<R: Record>(&self, records: &Records<R>) -> std::io::Result<()> {
        let path = &self.path;
        let dir = path.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(dir)?;
        let file = Saved {
            version: self.version,
            key: self.key,
            records: &records.records,
        };
        let mut json = serde_json::to_string_pretty(&file).map_err(std::io::Error::other)?;
        json.push('\n');
        let draft = dir.join(format!(
            ".{}.{}.{}.tmp",
            self.key,
            std::process::id(),
            DRAFTS.fetch_add(1, Ordering::Relaxed)
        ));
        let saved = std::fs::File::create_new(&draft)
            .and_then(|mut out| {
                out.write_all(json.as_bytes())?;
                out.sync_all()
            })
            .and_then(|()| std::fs::rename(&draft, path));
        if saved.is_err() {
            let _ = std::fs::remove_file(&draft);
        }
        saved?;
        // The new name itself on the disk, past a crash of the machine.
        std::fs::File::open(dir)?.sync_all()?;
        Ok(())
    }
}

/// The file as saved: its version first, then its records, each field in the order of its
/// struct, so a versioned file keeps its layout from one save to the next.
struct Saved<'a, R> {
    version: u32,
    key: &'static str,
    records: &'a [R],
}

impl<R: Serialize> Serialize for Saved<'_, R> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("version", &self.version)?;
        map.serialize_entry(self.key, self.records)?;
        map.end()
    }
}

fn unreadable(path: &Path, reason: &impl std::fmt::Display) -> StoreError {
    StoreError::Unreadable {
        path: path.to_path_buf(),
        reason: reason.to_string(),
    }
}

/// Held from the load to the save of a change, so two requests of one process never save over each
/// other's change.
static SAVING: Mutex<()> = Mutex::new(());

/// Tells apart the drafts of the saves of one process.
static DRAFTS: AtomicU64 = AtomicU64::new(0);
