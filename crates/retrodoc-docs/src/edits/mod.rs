//! The human edits of the docs (`specs/section-edit.md`, ADR 0045): the name, the description and,
//! for a use case, the narrative of a domain, a feature or a use case, as a human corrected them,
//! with the author and the date, kept in `.retrodoc/edits.json`, outside the cache. An overlay: the
//! artifacts keep the generated text beneath.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use retrodoc_ingest::git_history::user_name;
use serde::{Deserialize, Serialize};

use crate::read_model::{GeneratedDocs, Sections};
use crate::reviews::{self, Found, Outdated};
use crate::store::{kind_key, Record, Records, Section, SectionKind, Store, StoreError};

mod live;

pub use live::Live;

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
    /// The fingerprint of the section as generated when it was edited, the edits left out
    /// (`reviews::fingerprint_in` over [`GeneratedDocs::generated`], not [`crate::reviews::fingerprint`],
    /// which fingerprints the section as shown).
    pub fingerprint: String,
    /// The hash of each file the section cited, as the file was when it was edited; none in an
    /// edit written by hand.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub files: BTreeMap<String, String>,
    /// By the key of a field ([`Field::key`]), the base of a field kept as an outdated former
    /// edit gave it (SED-15), still outdated when it was saved; a field absent has the base of the
    /// edit.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub field_bases: BTreeMap<String, Base>,
}

/// What a field of an edit corrects: the fingerprint of its section as generated and the hashes
/// of the files it cited when the field was written (SED-8, SED-9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Base {
    pub fingerprint: String,
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

/// A field of a section a human can edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Name,
    Description,
    Narrative,
}

impl Field {
    pub const ALL: [Self; 3] = [Self::Name, Self::Description, Self::Narrative];

    /// Its name in the file of the edits.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Description => "description",
            Self::Narrative => "narrative",
        }
    }
}

impl Fields {
    /// The text it gives for `field`.
    #[must_use]
    pub fn get(&self, field: Field) -> Option<&str> {
        match field {
            Field::Name => self.name.as_deref(),
            Field::Description => self.description.as_deref(),
            Field::Narrative => self.narrative.as_deref(),
        }
    }
}

impl Edit {
    /// The base of `field`: its own when it was kept from an outdated former edit, else the
    /// edit's.
    #[must_use]
    pub fn base(&self, field: Field) -> Base {
        self.field_bases
            .get(field.key())
            .cloned()
            .unwrap_or_else(|| Base {
                fingerprint: self.fingerprint.clone(),
                files: self.files.clone(),
            })
    }

    /// The fields it changes as the readers show them, in the order of [`Field::ALL`]: a blank
    /// one (SED-33) and a narrative outside a use case, which a file written by hand may hold,
    /// left out.
    #[must_use]
    pub fn changed(&self) -> Vec<Field> {
        Field::ALL
            .into_iter()
            .filter(|field| *field != Field::Narrative || self.section.kind == SectionKind::UseCase)
            .filter(|field| {
                self.fields
                    .get(*field)
                    .is_some_and(|text| !text.trim().is_empty())
            })
            .collect()
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
    #[error("the docs hold no {} {}", kind_key(.0.kind), .0.id)]
    UnknownSection(Section),
    #[error("only a use case has a narrative, not the {} {}", kind_key(.0.kind), .0.id)]
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
fn store(repo_root: &Path) -> Store {
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

/// `sections` with `edits` applied: each field an edit gives replaces the generated one; an edit of
/// a section they don't hold, and a blank field of an edit written by hand, are left out.
#[must_use]
pub fn apply(mut shown: Sections, edits: &Edits) -> Sections {
    let edit_of = |kind, id: String| edits.get(&Section::new(kind, id)).map(|e| &e.fields);
    let given = |field: &Option<String>| field.clone().filter(|text| !text.trim().is_empty());
    let replace = |text: &mut String, by: &Option<String>| {
        if let Some(by) = given(by) {
            *text = by;
        }
    };
    for domain in &mut shown.domains {
        if let Some(fields) = edit_of(SectionKind::Domain, domain.slug.clone()) {
            replace(&mut domain.name, &fields.name);
            replace(&mut domain.description, &fields.description);
        }
    }
    for feature in &mut shown.features {
        if let Some(fields) = edit_of(SectionKind::Feature, feature.slug.clone()) {
            replace(&mut feature.name, &fields.name);
            replace(&mut feature.description, &fields.description);
        }
    }
    for use_case in &mut shown.use_cases {
        let id = format!("{}/{}", use_case.feature_slug, use_case.slug);
        if let Some(fields) = edit_of(SectionKind::UseCase, id) {
            replace(&mut use_case.name, &fields.name);
            replace(&mut use_case.description, &fields.description);
            if let Some(narrative) = given(&fields.narrative) {
                use_case.narrative = Some(narrative);
            }
        }
    }
    shown
}

/// Why `edit` no longer corrects its section as `docs` generated it and the files of their
/// repository are now (SED-34, SED-35), `None` when it does: the rule of the reviews
/// ([`reviews::outdated`]), against the section as generated, the edits left out, for the edit and
/// for each field it kept from an outdated former edit (SED-15), the reasons merged.
#[must_use]
pub fn outdated(docs: &GeneratedDocs, edit: &Edit) -> Option<Outdated> {
    let judge = |fingerprint: &str, files: &BTreeMap<String, String>| {
        reviews::judge(docs, docs.generated(), &edit.section, fingerprint, files)
    };
    let mut merged: Option<Outdated> = None;
    let found = std::iter::once(judge(&edit.fingerprint, &edit.files)).chain(
        edit.field_bases
            .values()
            .map(|base| judge(&base.fingerprint, &base.files)),
    );
    for outdated in found.flatten() {
        let into = merged.get_or_insert_with(|| Outdated {
            rewritten: false,
            changed: Vec::new(),
        });
        into.rewritten |= outdated.rewritten;
        into.changed.extend(outdated.changed);
    }
    merged.map(|mut outdated| {
        outdated.changed.sort_by(|a, b| a.0.cmp(&b.0));
        outdated.changed.dedup_by(|a, b| a.0 == b.0);
        outdated
    })
}

/// The text the run generated for each field `edit` changes ([`Edit::changed`]), empty for a use
/// case with no narrative; none when `docs` don't hold its section.
#[must_use]
pub fn generated(docs: &GeneratedDocs, edit: &Edit) -> Option<Vec<(Field, String)>> {
    let fields = fields_in(docs.generated(), &edit.section)?;
    Some(
        edit.changed()
            .into_iter()
            .map(|field| (field, fields.get(field).unwrap_or_default().to_string()))
            .collect(),
    )
}

/// The name, the description and, for a use case, the narrative of `section` in `sections`
/// ([`GeneratedDocs::sections`] as shown, [`GeneratedDocs::generated`] as generated); `None` when
/// they lack it.
#[must_use]
pub fn fields_in(sections: &Sections, section: &Section) -> Option<Fields> {
    let (name, description, narrative) = match reviews::find_in(sections, section)? {
        Found::Domain(d) => (&d.name, &d.description, None),
        Found::Feature(f) => (&f.name, &f.description, None),
        Found::UseCase(u) => (&u.name, &u.description, u.narrative.as_ref()),
    };
    Some(Fields {
        name: Some(name.clone()),
        description: Some(description.clone()),
        narrative: narrative.cloned(),
    })
}

/// Records the edit of `section` in `docs`, replacing its former one; its fingerprint is taken on
/// the section as generated, the edits `docs` shows left out (SED-25); a field it keeps as an
/// outdated former edit gave it keeps the base it had there (SED-15).
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
    save(repo_root, docs, section, fields, false)
}

/// Records the edit of `section` in `docs` confirmed: as [`record`], each field, one kept from
/// the former edit included, on the base of the edit (SED-36).
///
/// # Errors
///
/// As [`record`].
pub fn record_confirmed(
    repo_root: &Path,
    docs: &GeneratedDocs,
    section: &Section,
    fields: Fields,
) -> Result<Edit, EditsError> {
    save(repo_root, docs, section, fields, true)
}

fn save(
    repo_root: &Path,
    docs: &GeneratedDocs,
    section: &Section,
    fields: Fields,
    confirmed: bool,
) -> Result<Edit, EditsError> {
    let fingerprint = reviews::fingerprint_in(docs.generated(), section)
        .ok_or_else(|| EditsError::UnknownSection(section.clone()))?;
    check(section, &fields)?;
    // Read before the lock: the saves of the process wait on it.
    let former = if confirmed {
        None
    } else {
        load(repo_root)?.get(section).cloned()
    };
    let field_bases = former
        .as_ref()
        .map(|former| kept_bases(docs, former, &fields))
        .unwrap_or_default();
    let mut edit = Edit {
        section: section.clone(),
        fields,
        author: user_name(repo_root),
        edited_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        fingerprint,
        files: reviews::cited_hashes(repo_root, docs, section),
        field_bases,
    };
    Ok(store(repo_root).change(|edits| {
        // A former edit replaced since it was read: its bases are judged again, under the lock.
        if !confirmed && edits.get(section) != former.as_ref() {
            edit.field_bases = edits
                .get(section)
                .map(|former| kept_bases(docs, former, &edit.fields))
                .unwrap_or_default();
        }
        edits.set(edit.clone());
        (true, edit)
    })?)
}

/// The base `former` held for each field `fields` keep as it gave it (compared cleaned, as the
/// form cleans them), when that base is outdated in `docs` (SED-15).
fn kept_bases(docs: &GeneratedDocs, former: &Edit, fields: &Fields) -> BTreeMap<String, Base> {
    let same = |field: Field| match (fields.get(field), former.fields.get(field)) {
        (Some(new), Some(old)) => clean(new) == clean(old),
        _ => false,
    };
    Field::ALL
        .into_iter()
        .filter(|field| same(*field))
        .map(|field| (field, former.base(field)))
        .filter(|(_, base)| {
            reviews::judge(
                docs,
                docs.generated(),
                &former.section,
                &base.fingerprint,
                &base.files,
            )
            .is_some()
        })
        .map(|(field, base)| (field.key().to_string(), base))
        .collect()
}

/// `text` trimmed, with `\n` line ends.
fn clean(text: &str) -> String {
    text.replace("\r\n", "\n").trim().to_string()
}

/// Refuses a narrative outside a use case, no field and a blank one.
pub(crate) fn check(section: &Section, fields: &Fields) -> Result<(), EditsError> {
    if fields.narrative.is_some() && section.kind != SectionKind::UseCase {
        return Err(EditsError::NoNarrative(section.clone()));
    }
    let given: Vec<(Field, &str)> = Field::ALL
        .into_iter()
        .filter_map(|field| Some((field, fields.get(field)?)))
        .collect();
    if given.is_empty() {
        return Err(EditsError::NothingEdited);
    }
    match given.iter().find(|(_, text)| text.trim().is_empty()) {
        Some((field, _)) => Err(EditsError::Blank(field.key())),
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
