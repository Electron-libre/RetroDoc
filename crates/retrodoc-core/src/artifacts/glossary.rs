//! The glossary the pipeline reads from the model and business files and the tests, saved as
//! `.retrodoc/cache/glossary.yaml`: the entities of each file with the hash of the content they were
//! read from, and the descriptions of the tests.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::cache::{load_yaml, save_yaml, Artifact, CacheError};
use crate::text::normalize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Association {
    pub kind: String,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Entity {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub attributes: Vec<String>,
    #[serde(default)]
    pub associations: Vec<Association>,
}

/// What was read from one model file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelFile {
    pub content_hash: String,
    pub entities: Vec<Entity>,
}

/// Descriptions found in the tests of one file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestVocabulary {
    pub file: PathBuf,
    pub phrases: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Glossary {
    #[serde(default)]
    pub models: BTreeMap<PathBuf, ModelFile>,
    #[serde(default)]
    pub tests: Vec<TestVocabulary>,
}

impl Glossary {
    /// Missing or unreadable: `None` (first run).
    #[must_use]
    pub fn load(repo_root: &Path) -> Option<Self> {
        load_yaml(&Artifact::Glossary.path(repo_root))
    }

    /// # Errors
    ///
    /// Returns an error if the cache folder or file can't be written, or
    /// serialization fails.
    pub fn save(&self, repo_root: &Path) -> Result<(), CacheError> {
        save_yaml(&Artifact::Glossary.path(repo_root), self)
    }

    /// Every entity with the file it was read from, in path order.
    pub fn entities(&self) -> impl Iterator<Item = (&Path, &Entity)> {
        self.models
            .iter()
            .flat_map(|(path, file)| file.entities.iter().map(move |e| (path.as_path(), e)))
    }

    /// The entities merged by name (case-insensitive): the LLM also lists the
    /// classes a file merely references, so one concept shows up under
    /// several files. Name and description come from its *home* entry (the
    /// one in a file named after it, `company.rb` for `Company`, else the
    /// richest one); attributes and associations are the union; `files` lists
    /// the home file first, then the others. Sorted by name.
    #[must_use]
    pub fn merged_entities(&self) -> Vec<MergedEntity> {
        let mut groups: BTreeMap<String, Vec<(&Path, &Entity)>> = BTreeMap::new();
        for (path, entity) in self.entities() {
            groups
                .entry(entity.name.trim().to_lowercase())
                .or_default()
                .push((path, entity));
        }
        groups
            .into_values()
            .map(|group| merge_group(&group))
            .collect()
    }

    #[must_use]
    pub fn phrase_count(&self) -> usize {
        self.tests.iter().map(|t| t.phrases.len()).sum()
    }
}

/// One business concept after merging the per-file entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergedEntity {
    pub name: String,
    pub description: String,
    pub attributes: Vec<String>,
    pub associations: Vec<Association>,
    /// Files that mention the entity, the home file first.
    pub files: Vec<PathBuf>,
}

fn merge_group(group: &[(&Path, &Entity)]) -> MergedEntity {
    let richness = |entity: &Entity| {
        (
            entity.attributes.len() + entity.associations.len(),
            entity.description.len(),
        )
    };
    // `max_by_key` keeps the last maximum: iterate in reverse so that, on a
    // full tie, the first file in path order wins.
    let home = group
        .iter()
        .rev()
        .max_by_key(|(path, entity)| {
            let named_after = path
                .file_stem()
                .is_some_and(|stem| normalize(&stem.to_string_lossy()) == normalize(&entity.name));
            (named_after, richness(entity))
        })
        .expect("a group has at least one entry");

    let mut attributes: Vec<String> = Vec::new();
    let mut associations: Vec<Association> = Vec::new();
    let mut files: Vec<PathBuf> = vec![home.0.to_path_buf()];
    for (path, entity) in
        std::iter::once(home).chain(group.iter().filter(|e| !std::ptr::eq(*e, home)))
    {
        for attribute in &entity.attributes {
            if !attributes.iter().any(|a| a.eq_ignore_ascii_case(attribute)) {
                attributes.push(attribute.clone());
            }
        }
        for association in &entity.associations {
            if !associations.contains(association) {
                associations.push(association.clone());
            }
        }
        if !files.iter().any(|f| f == path) {
            files.push(path.to_path_buf());
        }
    }
    MergedEntity {
        name: home.1.name.trim().to_string(),
        description: home.1.description.clone(),
        attributes,
        associations,
        files,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_entities_listed_under_several_files() {
        let entity =
            |name: &str, description: &str, attributes: &[&str], target: Option<&str>| Entity {
                name: name.to_string(),
                description: description.to_string(),
                attributes: attributes.iter().map(|a| (*a).to_string()).collect(),
                associations: target
                    .map(|t| Association {
                        kind: "has_many".to_string(),
                        target: t.to_string(),
                    })
                    .into_iter()
                    .collect(),
            };
        let file = |entities| ModelFile {
            content_hash: String::new(),
            entities,
        };
        let glossary = Glossary {
            models: BTreeMap::from([
                (
                    PathBuf::from("app/models/actions/company_user_actions.rb"),
                    file(vec![entity(
                        "Company",
                        "short",
                        &["name"],
                        Some("CompanyUser"),
                    )]),
                ),
                (
                    PathBuf::from("app/models/company.rb"),
                    file(vec![entity(
                        "company",
                        "A business organization",
                        &["Name", "siret"],
                        Some("Worksite"),
                    )]),
                ),
                (
                    PathBuf::from("app/models/user.rb"),
                    file(vec![entity("User", "A person", &[], None)]),
                ),
            ]),
            tests: Vec::new(),
        };

        let merged = glossary.merged_entities();
        assert_eq!(merged.len(), 2);
        let company = &merged[0];
        assert_eq!(company.name, "company");
        assert_eq!(company.description, "A business organization");
        assert_eq!(company.attributes, vec!["Name", "siret"]);
        assert_eq!(company.associations.len(), 2);
        assert_eq!(
            company.files,
            vec![
                PathBuf::from("app/models/company.rb"),
                PathBuf::from("app/models/actions/company_user_actions.rb"),
            ]
        );
        assert_eq!(merged[1].name, "User");
    }
}
