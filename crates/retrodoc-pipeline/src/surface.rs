//! The application surface (PLAN.md §7.1, phase 7 step 4): what the
//! glossary (entities) and the entry points inventory say the application
//! is about, condensed into a view small enough for a prompt. The business
//! is expressed by its nouns (entities), its verbs and the resources they
//! act on (entry points); the passes that name and group things (domains,
//! later actors and use cases) start from here instead of from the folder
//! structure.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

use crate::entry_points::EntryPoints;
use crate::glossary::{Glossary, MergedEntity};

/// Entities listed in a prompt, the best connected first.
const MAX_PROMPT_ENTITIES: usize = 80;
/// Resources listed in a prompt, the most exposed first.
const MAX_PROMPT_RESOURCES: usize = 40;
const MAX_DESCRIPTION_CHARS: usize = 120;
const MAX_ASSOCIATIONS_SHOWN: usize = 3;
const MAX_VERBS_SHOWN: usize = 6;

/// A business object the entry points act on, with what can be done to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    pub name: String,
    pub entry_count: usize,
    /// Distinct verbs, in first-seen order.
    pub verbs: Vec<String>,
    /// Files that define entry points on this resource, in path order.
    pub files: Vec<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct Surface {
    /// Sorted by decreasing connectivity (files mentioning the entity, then
    /// associations), then by name.
    pub entities: Vec<MergedEntity>,
    /// Sorted by decreasing number of entry points, then by name.
    pub resources: Vec<Resource>,
}

impl Surface {
    #[must_use]
    pub fn new(glossary: &Glossary, entry_points: &EntryPoints) -> Self {
        let mut entities = glossary.merged_entities();
        entities.sort_by(|a, b| {
            (b.files.len(), b.associations.len(), &a.name).cmp(&(
                a.files.len(),
                a.associations.len(),
                &b.name,
            ))
        });

        let mut by_resource: BTreeMap<String, Resource> = BTreeMap::new();
        for (file, entry) in entry_points.iter() {
            let name = entry.resource.trim().to_lowercase();
            if name.is_empty() {
                continue;
            }
            let resource = by_resource.entry(name.clone()).or_insert_with(|| Resource {
                name,
                entry_count: 0,
                verbs: Vec::new(),
                files: Vec::new(),
            });
            resource.entry_count += 1;
            for verb in entry.verb.split(',').map(|v| v.trim().to_lowercase()) {
                if !verb.is_empty() && !resource.verbs.contains(&verb) {
                    resource.verbs.push(verb);
                }
            }
            if !resource.files.iter().any(|f| f == file) {
                resource.files.push(file.to_path_buf());
            }
        }
        let mut resources: Vec<Resource> = by_resource.into_values().collect();
        resources.sort_by(|a, b| {
            b.entry_count
                .cmp(&a.entry_count)
                .then_with(|| a.name.cmp(&b.name))
        });

        Self {
            entities,
            resources,
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty() && self.resources.is_empty()
    }

    /// Everything that feeds [`Self::prompt_section`], one string per item,
    /// to hash: a changed surface must invalidate the passes built on it.
    pub fn fingerprint_parts(&self) -> impl Iterator<Item = String> + '_ {
        let entities = self.entities.iter().map(|e| {
            format!(
                "entity {}\n{}\n{}",
                e.name,
                e.description,
                e.associations
                    .iter()
                    .map(|a| format!("{} {}", a.kind, a.target))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        });
        let resources = self.resources.iter().map(|r| {
            format!(
                "resource {} {} {}",
                r.name,
                r.entry_count,
                r.verbs.join(",")
            )
        });
        entities.chain(resources)
    }

    /// The prompt view: the best connected entities, then the most exposed
    /// resources, with the files they live in so that a module can be placed
    /// next to the concept it serves.
    #[must_use]
    pub fn prompt_section(&self) -> String {
        let mut out = String::new();
        if !self.entities.is_empty() {
            out.push_str("Business entities (from the data models):\n");
            for entity in self.entities.iter().take(MAX_PROMPT_ENTITIES) {
                let description: String = entity
                    .description
                    .chars()
                    .take(MAX_DESCRIPTION_CHARS)
                    .collect();
                let associations = entity
                    .associations
                    .iter()
                    .take(MAX_ASSOCIATIONS_SHOWN)
                    .map(|a| format!("{} {}", a.kind, a.target))
                    .collect::<Vec<_>>()
                    .join(", ");
                let home = entity
                    .files
                    .first()
                    .map_or(String::new(), |f| f.display().to_string());
                let _ = write!(out, "- {} ({home}): {description}", entity.name);
                if !associations.is_empty() {
                    let _ = write!(out, " [{associations}]");
                }
                out.push('\n');
            }
            let hidden = self.entities.len().saturating_sub(MAX_PROMPT_ENTITIES);
            if hidden > 0 {
                let _ = writeln!(out, "… and {hidden} less connected entities");
            }
        }
        if !self.resources.is_empty() {
            out.push_str("\nEntry points by resource (what users and systems can do):\n");
            for resource in self.resources.iter().take(MAX_PROMPT_RESOURCES) {
                let verbs = resource
                    .verbs
                    .iter()
                    .take(MAX_VERBS_SHOWN)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ");
                let files = resource
                    .files
                    .iter()
                    .take(2)
                    .map(|f| f.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = writeln!(
                    out,
                    "- {}: {} entry point(s) — {verbs}; in {files}",
                    resource.name, resource.entry_count
                );
            }
            let hidden = self.resources.len().saturating_sub(MAX_PROMPT_RESOURCES);
            if hidden > 0 {
                let _ = writeln!(out, "… and {hidden} less exposed resources");
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::entry_points::{EntryFile, EntryKind, EntryPoint};
    use crate::glossary::{Association, Entity, ModelFile};

    fn entity(name: &str, target: Option<&str>) -> Entity {
        Entity {
            name: name.to_string(),
            description: format!("{name} description"),
            attributes: Vec::new(),
            associations: target
                .map(|t| Association {
                    kind: "has_many".to_string(),
                    target: t.to_string(),
                })
                .into_iter()
                .collect(),
        }
    }

    fn entry(name: &str, verb: &str, resource: &str) -> EntryPoint {
        EntryPoint {
            kind: EntryKind::HttpRoute,
            name: name.to_string(),
            verb: verb.to_string(),
            resource: resource.to_string(),
            description: String::new(),
            outputs: Vec::new(),
        }
    }

    fn surface() -> Surface {
        let glossary = Glossary {
            models: BTreeMap::from([
                (
                    PathBuf::from("app/models/user.rb"),
                    ModelFile {
                        content_hash: String::new(),
                        entities: vec![entity("User", None)],
                    },
                ),
                (
                    PathBuf::from("app/models/contract.rb"),
                    ModelFile {
                        content_hash: String::new(),
                        entities: vec![entity("Contract", Some("Signatory"))],
                    },
                ),
            ]),
            tests: Vec::new(),
        };
        let entry_points = EntryPoints {
            files: BTreeMap::from([(
                PathBuf::from("app/controllers/contracts_controller.rb"),
                EntryFile {
                    content_hash: String::new(),
                    entry_points: vec![
                        entry("POST /contracts/:id/sign", "sign", "Contract"),
                        entry("GET /contracts/:id", "show, view", "contract"),
                        entry("Healthcheck", "get", ""),
                    ],
                },
            )]),
        };
        Surface::new(&glossary, &entry_points)
    }

    #[test]
    fn groups_entry_points_by_resource_and_orders_entities_by_connectivity() {
        let surface = surface();
        assert_eq!(surface.entities[0].name, "Contract");
        assert_eq!(surface.resources.len(), 1);
        let contract = &surface.resources[0];
        assert_eq!(contract.name, "contract");
        assert_eq!(contract.entry_count, 2);
        assert_eq!(contract.verbs, vec!["sign", "show", "view"]);
        assert!(!surface.is_empty());
        assert!(Surface::default().is_empty());
    }

    #[test]
    fn prompt_section_lists_entities_and_resources() {
        let section = surface().prompt_section();
        assert!(section.contains(
            "- Contract (app/models/contract.rb): Contract description [has_many Signatory]"
        ));
        assert!(section.contains(
            "- contract: 2 entry point(s) — sign, show, view; in app/controllers/contracts_controller.rb"
        ));
    }

    #[test]
    fn fingerprint_changes_with_the_surface() {
        let print = |s: &Surface| crate::fingerprints::fingerprint(s.fingerprint_parts());
        let mut changed = surface();
        assert_eq!(print(&surface()), print(&changed));
        changed.resources[0].entry_count += 1;
        assert_ne!(print(&surface()), print(&changed));
    }
}
