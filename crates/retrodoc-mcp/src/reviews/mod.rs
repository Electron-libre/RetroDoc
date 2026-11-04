//! The human reviews of the docs (`specs/section-review.md`, ADR 0044): a domain, a feature or a
//! use case marked validated, wrong or to review, with a comment, its author and its date, kept in
//! `.retrodoc/reviews.json`, outside the cache, so a `generate --force` keeps them and a team can
//! version them.

use std::io::{ErrorKind, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use chrono::{SecondsFormat, Utc};
use retrodoc_ingest::git_history::user_name;
use retrodoc_pipeline::cache::hash_content;
use serde::{Deserialize, Serialize};

pub use crate::history::SectionKind;
use crate::read_model::GeneratedDocs;

/// The format of the file written now.
pub const VERSION: u32 = 1;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Validated,
    Wrong,
    ToReview,
}

/// What a human said of a section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Review {
    #[serde(flatten)]
    pub section: Section,
    pub status: Status,
    /// Empty when none was given.
    #[serde(default)]
    pub comment: String,
    /// The `user.name` of the git config, `None` when unset.
    pub author: Option<String>,
    /// RFC 3339, UTC, to the second.
    pub reviewed_at: String,
    /// The fingerprint of the section as it was reviewed ([`fingerprint`]).
    pub fingerprint: String,
}

/// The reviews of a repository, by kind then id.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reviews {
    reviews: Vec<Review>,
}

impl Reviews {
    #[must_use]
    pub fn reviews(&self) -> &[Review] {
        &self.reviews
    }

    #[must_use]
    pub fn get(&self, section: &Section) -> Option<&Review> {
        self.reviews.iter().find(|r| &r.section == section)
    }

    /// Sets `review`, replacing the former review of its section.
    fn set(&mut self, review: Review) {
        self.unset(&review.section);
        let at = self.reviews.partition_point(|r| r.section < review.section);
        self.reviews.insert(at, review);
    }

    fn unset(&mut self, section: &Section) {
        self.reviews.retain(|r| &r.section != section);
    }
}

/// The kind as the file names it.
fn kind_key(kind: SectionKind) -> &'static str {
    match kind {
        SectionKind::Domain => "domain",
        SectionKind::Feature => "feature",
        SectionKind::UseCase => "use_case",
    }
}

/// The version of the file, read before the rest.
#[derive(Deserialize)]
struct Version {
    version: u32,
}

/// The file as saved.
#[derive(Serialize, Deserialize)]
struct File {
    version: u32,
    reviews: Vec<Review>,
}

#[derive(Debug, thiserror::Error)]
pub enum ReviewsError {
    #[error("{} cannot be read: {reason}", path.display())]
    Unreadable { path: PathBuf, reason: String },
    #[error("{} is of format {version}, this RetroDoc reads up to {VERSION}", path.display())]
    LaterFormat { path: PathBuf, version: u32 },
    #[error("the docs hold no {} {}", kind_key(.0.kind), .0.id)]
    UnknownSection(Section),
    #[error("the reviews cannot be saved: {0}")]
    Io(#[from] std::io::Error),
}

/// Where the reviews of `repo_root` are saved.
#[must_use]
pub fn reviews_path(repo_root: &Path) -> PathBuf {
    repo_root.join(".retrodoc").join("reviews.json")
}

/// The fingerprint of `section` in `docs`, `None` when the docs lack it.
#[must_use]
pub fn fingerprint(docs: &GeneratedDocs, section: &Section) -> Option<String> {
    let id = section.id.as_str();
    let saved = match section.kind {
        SectionKind::Domain => serde_json::to_string(docs.domains().iter().find(|d| d.slug == id)?),
        SectionKind::Feature => {
            serde_json::to_string(docs.features().iter().find(|f| f.slug == id)?)
        }
        SectionKind::UseCase => {
            let (feature, slug) = id.split_once('/')?;
            serde_json::to_string(
                docs.use_cases()
                    .iter()
                    .find(|u| u.feature_slug == feature && u.slug == slug)?,
            )
        }
    };
    saved.ok().map(|json| hash_content(&json))
}

/// The reviews saved in `repo_root`, none when no file is.
///
/// # Errors
///
/// The file cannot be read or is of a later format.
pub fn load(repo_root: &Path) -> Result<Reviews, ReviewsError> {
    let path = reviews_path(repo_root);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Reviews::default()),
        Err(e) => return Err(unreadable(path, &e)),
    };
    // The version first: a later format may not parse as this one.
    let version = serde_json::from_str::<Version>(&text)
        .map_err(|e| unreadable(path.clone(), &e))?
        .version;
    if version > VERSION {
        return Err(ReviewsError::LaterFormat { path, version });
    }
    let mut file: File = serde_json::from_str(&text).map_err(|e| unreadable(path, &e))?;
    // The newest review of a section first, then the others of that section dropped: a file merged
    // by hand may hold several.
    file.reviews.sort_by(|a, b| {
        a.section
            .cmp(&b.section)
            .then_with(|| b.reviewed_at.cmp(&a.reviewed_at))
    });
    file.reviews
        .dedup_by(|later, kept| later.section == kept.section);
    Ok(Reviews {
        reviews: file.reviews,
    })
}

fn unreadable(path: PathBuf, reason: &impl std::fmt::Display) -> ReviewsError {
    ReviewsError::Unreadable {
        path,
        reason: reason.to_string(),
    }
}

/// Records the review of `section` in `docs`, replacing its former one.
///
/// # Errors
///
/// The section is not in `docs`, or the file cannot be read or written.
pub fn record(
    repo_root: &Path,
    docs: &GeneratedDocs,
    section: &Section,
    status: Status,
    comment: &str,
) -> Result<Review, ReviewsError> {
    let fingerprint =
        fingerprint(docs, section).ok_or_else(|| ReviewsError::UnknownSection(section.clone()))?;
    let _saving = SAVING.lock().unwrap_or_else(PoisonError::into_inner);
    let mut reviews = load(repo_root)?;
    let review = Review {
        section: section.clone(),
        status,
        comment: comment.to_string(),
        author: user_name(repo_root),
        reviewed_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        fingerprint,
    };
    reviews.set(review.clone());
    save(repo_root, &reviews)?;
    Ok(review)
}

/// Removes the review of `section`, the others kept.
///
/// # Errors
///
/// The file cannot be read or written.
pub fn remove(repo_root: &Path, section: &Section) -> Result<(), ReviewsError> {
    let _saving = SAVING.lock().unwrap_or_else(PoisonError::into_inner);
    let mut reviews = load(repo_root)?;
    if reviews.get(section).is_none() {
        return Ok(());
    }
    reviews.unset(section);
    save(repo_root, &reviews)
}

/// Held from the load to the save of a change, so two requests of one process never save over each
/// other's change.
static SAVING: Mutex<()> = Mutex::new(());

/// Tells apart the drafts of the saves of one process.
static DRAFTS: AtomicU64 = AtomicU64::new(0);

/// Writes `reviews` whole aside, then renames it over the former file, so no reader sees a file
/// cut short. Called after a [`load`] that succeeded, so it never replaces a file it can't read.
fn save(repo_root: &Path, reviews: &Reviews) -> Result<(), ReviewsError> {
    let path = reviews_path(repo_root);
    let dir = path.parent().unwrap_or(repo_root);
    std::fs::create_dir_all(dir)?;
    let file = File {
        version: VERSION,
        reviews: reviews.reviews.clone(),
    };
    let mut json = serde_json::to_string_pretty(&file).map_err(std::io::Error::other)?;
    json.push('\n');
    let draft = dir.join(format!(
        ".reviews.{}.{}.tmp",
        std::process::id(),
        DRAFTS.fetch_add(1, Ordering::Relaxed)
    ));
    let saved = std::fs::File::create_new(&draft)
        .and_then(|mut out| {
            out.write_all(json.as_bytes())?;
            out.sync_all()
        })
        .and_then(|()| std::fs::rename(&draft, &path));
    if saved.is_err() {
        let _ = std::fs::remove_file(&draft);
    }
    saved?;
    // The new name itself on the disk, past a crash of the machine.
    std::fs::File::open(dir)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests;
