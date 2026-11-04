//! The human reviews of the docs (`specs/section-review.md`, ADR 0044): a domain, a feature or a
//! use case marked validated, wrong or to review, with a comment, its author and its date, kept in
//! `.retrodoc/reviews.json`, outside the cache, so a `generate --force` keeps them and a team can
//! version them.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use retrodoc_core::model::Feature;
use retrodoc_ingest::git_history::user_name;
use retrodoc_pipeline::cache::hash_content;
use retrodoc_pipeline::{ChangedFile, OutdatedReview};
use serde::{Deserialize, Serialize};

use crate::debt::use_case_cited_paths;
use crate::freshness::{file_hash, state_against, FileState};
use crate::read_model::{GeneratedDocs, Sections};
use crate::store::{kind_key, Record, Records, Store};

#[derive(Debug, thiserror::Error)]
pub enum ReviewsError {
    #[error("the docs hold no {} {}", kind_key(.0.kind), .0.id)]
    UnknownSection(Section),
    #[error(transparent)]
    Store(#[from] StoreError),
}
pub use crate::store::{Section, SectionKind, StoreError};

/// The format of the file written now.
pub const VERSION: u32 = 1;

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
    /// The hash of each file the section cited, as the file was when it was reviewed; none in a
    /// review written by hand.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub files: BTreeMap<String, String>,
}

impl Record for Review {
    fn section(&self) -> &Section {
        &self.section
    }

    fn made_at(&self) -> &str {
        &self.reviewed_at
    }
}

/// Why a review no longer vouches for its section as it is now.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Outdated {
    /// The section in the docs differs from the one reviewed.
    pub rewritten: bool,
    /// The cited files modified or deleted since the review, in path order.
    pub changed: Vec<(String, FileState)>,
}

impl Outdated {
    /// As the debt report holds it.
    #[must_use]
    pub fn report(&self) -> OutdatedReview {
        OutdatedReview {
            rewritten: self.rewritten,
            changed: self
                .changed
                .iter()
                .map(|(path, state)| ChangedFile {
                    path: path.clone(),
                    deleted: *state == FileState::Deleted,
                })
                .collect(),
        }
    }

    /// The reasons that apply, on one line ([`OutdatedReview::reasons`]).
    #[must_use]
    pub fn reasons(&self) -> String {
        self.report().reasons()
    }
}

/// The reviews of a repository, by kind then id.
pub type Reviews = Records<Review>;

impl Reviews {
    #[must_use]
    pub fn reviews(&self) -> &[Review] {
        self.all()
    }
}

/// Where the reviews of `repo_root` are saved.
#[must_use]
pub fn reviews_path(repo_root: &Path) -> PathBuf {
    repo_root.join(".retrodoc").join("reviews.json")
}

/// The fingerprint of `section` in `docs` as they show it, the edits applied, `None` when the docs
/// lack it.
#[must_use]
pub fn fingerprint(docs: &GeneratedDocs, section: &Section) -> Option<String> {
    fingerprint_in(docs.sections(), section)
}

/// The fingerprint of `section` in `sections`, `None` when they lack it.
pub(crate) fn fingerprint_in(sections: &Sections, section: &Section) -> Option<String> {
    let id = section.id.as_str();
    let saved = match section.kind {
        SectionKind::Domain => {
            serde_json::to_string(sections.domains.iter().find(|d| d.slug == id)?)
        }
        SectionKind::Feature => {
            serde_json::to_string(sections.features.iter().find(|f| f.slug == id)?)
        }
        SectionKind::UseCase => {
            let (feature, slug) = id.split_once('/')?;
            serde_json::to_string(
                sections
                    .use_cases
                    .iter()
                    .find(|u| u.feature_slug == feature && u.slug == slug)?,
            )
        }
    };
    saved.ok().map(|json| hash_content(&json))
}

/// Why `review` is outdated in `docs` and the files of their repository now, `None` when it is
/// not; docs that carry no repository judge no file. Only the files the section cites now are
/// read: the paths of a file of reviews are not trusted.
#[must_use]
pub fn outdated(docs: &GeneratedDocs, review: &Review) -> Option<Outdated> {
    let rewritten = fingerprint(docs, &review.section).as_ref() != Some(&review.fingerprint);
    let changed: Vec<(String, FileState)> = docs.repo_root().map_or_else(Vec::new, |root| {
        let cited: HashSet<&str> = cited_paths(docs, &review.section).into_iter().collect();
        review
            .files
            .iter()
            .filter(|(path, _)| cited.contains(path.as_str()))
            .map(|(path, hash)| (path.clone(), state_against(root, path, hash)))
            .filter(|(_, state)| matches!(state, FileState::Modified | FileState::Deleted))
            .collect()
    });
    (rewritten || !changed.is_empty()).then_some(Outdated { rewritten, changed })
}

/// The files `section` cites in `docs`: the source files of a feature, the files the steps of a
/// use case cite, the source files of the features of a domain; each once, in order.
#[must_use]
pub fn cited_paths<'a>(docs: &'a GeneratedDocs, section: &Section) -> Vec<&'a str> {
    let id = section.id.as_str();
    let source_paths = |of: &dyn Fn(&Feature) -> bool| -> Vec<&'a str> {
        docs.features()
            .iter()
            .filter(|f| of(f))
            .flat_map(|f| f.source_paths.iter().map(String::as_str))
            .collect()
    };
    let mut paths = match section.kind {
        SectionKind::Domain => source_paths(&|f| f.domain_slug == id),
        SectionKind::Feature => source_paths(&|f| f.slug == id),
        SectionKind::UseCase => id
            .split_once('/')
            .and_then(|(feature, slug)| {
                docs.use_cases()
                    .iter()
                    .find(|u| u.feature_slug == feature && u.slug == slug)
            })
            .map(use_case_cited_paths)
            .unwrap_or_default(),
    };
    let mut seen = HashSet::new();
    paths.retain(|path| seen.insert(*path));
    paths
}

/// The file of the reviews of `repo_root`.
fn store(repo_root: &Path) -> Store {
    Store {
        path: reviews_path(repo_root),
        version: VERSION,
        key: "reviews",
    }
}

/// The reviews saved in `repo_root`, none when no file is.
///
/// # Errors
///
/// The file cannot be read or is of a later format.
pub fn load(repo_root: &Path) -> Result<Reviews, ReviewsError> {
    Ok(store(repo_root).load()?)
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
    // Read before the lock: the saves of the process wait on it.
    let review = Review {
        section: section.clone(),
        status,
        comment: comment.to_string(),
        author: user_name(repo_root),
        reviewed_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        fingerprint,
        files: cited_hashes(repo_root, docs, section),
    };
    Ok(store(repo_root).change(|reviews| {
        reviews.set(review.clone());
        (true, review)
    })?)
}

/// The hash of each file `section` cites in `docs`, as it is now in `repo_root`; a file that
/// cannot be read gets none.
pub(crate) fn cited_hashes(
    repo_root: &Path,
    docs: &GeneratedDocs,
    section: &Section,
) -> BTreeMap<String, String> {
    cited_paths(docs, section)
        .into_iter()
        .filter_map(|path| Some((path.to_string(), file_hash(repo_root, path).ok()?)))
        .collect()
}

/// Removes the review of `section`, the others kept.
///
/// # Errors
///
/// The file cannot be read or written.
pub fn remove(repo_root: &Path, section: &Section) -> Result<(), ReviewsError> {
    Ok(store(repo_root).change(|reviews: &mut Reviews| {
        let held = reviews.get(section).is_some();
        reviews.unset(section);
        (held, ())
    })?)
}

#[cfg(test)]
pub(crate) mod tests;
