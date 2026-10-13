//! The domains of a run on the bounded contexts (`domains_from = "contexts"`, ADR 0029): a context
//! is a domain, with no LLM call. Its files are its own and the ones read for it; its entities are
//! those the glossary found in them; its entry points are those of its files (`by_paths`), never
//! those of a resource it shares with another context. Its vocabulary is its own, so reading
//! another context does not change what the prompts of this one say.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use crate::domains::{DomainCluster, DomainMap};
use crate::glossary::Glossary;

use super::Resolved;

/// The domains for the contexts `slugs` (in the order of the map).
#[must_use]
pub fn domains_from_contexts(
    resolved: &Resolved,
    reads: &BTreeMap<String, Vec<PathBuf>>,
    glossary: &Glossary,
    slugs: &[String],
) -> DomainMap {
    let domains = resolved
        .contexts
        .iter()
        .filter(|context| slugs.contains(&context.slug))
        .map(|context| {
            let paths: Vec<PathBuf> = context
                .files
                .iter()
                .chain(reads.get(&context.slug).into_iter().flatten())
                .cloned()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            let mut entities: Vec<String> = Vec::new();
            for path in &paths {
                for entity in glossary.models.get(path).iter().flat_map(|m| &m.entities) {
                    if !entities.contains(&entity.name) {
                        entities.push(entity.name.clone());
                    }
                }
            }
            DomainCluster {
                slug: context.slug.clone(),
                name: context.name.clone(),
                description: context.description.clone(),
                paths,
                entities,
                by_paths: true,
                ..DomainCluster::default()
            }
        })
        .collect();
    DomainMap { domains }
}

/// The names of the `max` best connected entities of each domain (attributes, then associations,
/// then name): the vocabulary its prompts speak.
#[must_use]
pub fn domain_vocabulary(
    domains: &DomainMap,
    glossary: &Glossary,
    max: usize,
) -> BTreeMap<String, Vec<String>> {
    domains
        .domains
        .iter()
        .map(|domain| {
            let mut found: Vec<(usize, &str)> = Vec::new();
            for path in &domain.paths {
                for entity in glossary.models.get(path).iter().flat_map(|m| &m.entities) {
                    if !found.iter().any(|(_, name)| *name == entity.name) {
                        found.push((
                            entity.attributes.len() + entity.associations.len(),
                            &entity.name,
                        ));
                    }
                }
            }
            found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
            let names = found
                .into_iter()
                .take(max)
                .map(|(_, name)| name.to_string())
                .collect();
            (domain.slug.clone(), names)
        })
        .collect()
}

#[cfg(test)]
#[path = "domains_tests.rs"]
mod tests;
