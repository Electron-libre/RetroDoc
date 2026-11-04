//! The human edits of the docs (`specs/section-edit.md`, ADR 0045): the name, the description and,
//! for a use case, the narrative of a domain, a feature or a use case, as a human corrected them,
//! with the author and the date, kept in `.retrodoc/edits.json`, outside the cache. An overlay: the
//! artifacts keep the generated text beneath.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use retrodoc_ingest::git_history::user_name;
use serde::{Deserialize, Serialize};

use crate::read_model::GeneratedDocs;
use crate::reviews;
use crate::store::{Record, Records, Section, SectionKind, Store, StoreError};

/// The format of the file written now.
pub const VERSION: u32 = 1;

/// The fields an edit changes; a field left out keeps the generated text.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fields {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// A use case only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub narrative: Option<String>,
}

/// What a human wrote over a section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edit {
    #[serde(flatten)]
    pub section: Section,
    #[serde(flatten)]
    pub fields: Fields,
    /// The `user.name` of the git config, `None` when unset.
    pub author: Option<String>,
    /// RFC 3339, UTC, to the second.
    pub edited_at: String,
    /// The fingerprint of the generated section when it was edited
    /// ([`crate::reviews::fingerprint`]).
    pub fingerprint: String,
    /// The hash of each file the section cited, as the file was when it was edited; none in an
    /// edit written by hand.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub files: BTreeMap<String, String>,
}

impl Record for Edit {
    fn section(&self) -> &Section {
        &self.section
    }

    fn made_at(&self) -> &str {
        &self.edited_at
    }
}

/// The edits of a repository, by kind then id.
pub type Edits = Records<Edit>;

impl Edits {
    #[must_use]
    pub fn edits(&self) -> &[Edit] {
        self.all()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EditsError {
    #[error("only a use case has a narrative, not the {} {}", crate::store::kind_key(.0.kind), .0.id)]
    NoNarrative(Section),
    #[error("the edit changes no field")]
    NothingEdited,
    #[error("the {0} of an edit cannot be blank")]
    Blank(&'static str),
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Where the edits of `repo_root` are saved.
#[must_use]
pub fn edits_path(repo_root: &Path) -> PathBuf {
    repo_root.join(".retrodoc").join("edits.json")
}

/// The file of the edits of `repo_root`.
fn store(repo_root: &Path) -> Store<'static> {
    Store {
        path: edits_path(repo_root),
        version: VERSION,
        key: "edits",
    }
}

/// The edits saved in `repo_root`, none when no file is.
///
/// # Errors
///
/// The file cannot be read or is of a later format.
pub fn load(repo_root: &Path) -> Result<Edits, EditsError> {
    Ok(store(repo_root).load()?)
}

/// Records the edit of `section` in `docs`, the generated docs with no edit applied, replacing its
/// former one.
///
/// # Errors
///
/// The section is not in `docs`, the fields are refused (a narrative outside a use case, no field,
/// a blank one), or the file cannot be read or written.
pub fn record(
    repo_root: &Path,
    docs: &GeneratedDocs,
    section: &Section,
    fields: Fields,
) -> Result<Edit, EditsError> {
    let fingerprint = reviews::fingerprint(docs, section)
        .ok_or_else(|| StoreError::UnknownSection(section.clone()))?;
    check(section, &fields)?;
    Ok(store(repo_root).change(|edits| {
        let edit = Edit {
            section: section.clone(),
            fields,
            author: user_name(repo_root),
            edited_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            fingerprint,
            files: reviews::cited_hashes(repo_root, docs, section),
        };
        edits.set(edit.clone());
        (true, edit)
    })?)
}

/// Refuses a narrative outside a use case, no field and a blank one.
fn check(section: &Section, fields: &Fields) -> Result<(), EditsError> {
    if fields.narrative.is_some() && section.kind != SectionKind::UseCase {
        return Err(EditsError::NoNarrative(section.clone()));
    }
    let given = [
        ("name", &fields.name),
        ("description", &fields.description),
        ("narrative", &fields.narrative),
    ];
    if given.iter().all(|(_, field)| field.is_none()) {
        return Err(EditsError::NothingEdited);
    }
    match given
        .iter()
        .find(|(_, field)| field.as_deref().is_some_and(|text| text.trim().is_empty()))
    {
        Some((name, _)) => Err(EditsError::Blank(name)),
        None => Ok(()),
    }
}

/// Removes the edit of `section`, the others kept.
///
/// # Errors
///
/// The file cannot be read or written.
pub fn remove(repo_root: &Path, section: &Section) -> Result<(), EditsError> {
    Ok(store(repo_root).change(|edits: &mut Edits| {
        let held = edits.get(section).is_some();
        edits.unset(section);
        (held, ())
    })?)
}

#[cfg(test)]
mod tests;
