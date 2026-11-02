//! What the map pages show: the docs drawn as a graph by `mermaid`, from a
//! source built here, beside the same items as a list of links. The list is
//! what the drawn nodes open (`data-node`, wired by `diagrams.js`) and what a
//! reader follows when the graph does not draw.

use std::collections::{BTreeMap, BTreeSet};

use retrodoc_mcp::GeneratedDocs;
use retrodoc_render::anchor;

use crate::glossary::uses;
use crate::view::{plural, Link};

/// The concepts named on an edge, the others counted.
const EDGE_CONCEPTS: usize = 3;

/// The map of the domains.
pub struct MapPage {
    /// The source of the graph `mermaid` draws.
    pub source: String,
    /// In the order of the run.
    pub domains: Vec<MapDomain>,
}

/// A domain of the map, the item of its node.
pub struct MapDomain {
    /// The id of its node in the source.
    pub node: String,
    /// Its own map.
    pub href: String,
    pub name: String,
    /// Its numbers of features and use cases, with their units.
    pub counts: String,
    /// The concepts it shares with each other domain, in the order of the run.
    pub shared: Vec<Shared>,
}

/// The concepts a domain shares with another one.
pub struct Shared {
    pub with: Link,
    /// Sorted by name, ignoring case.
    pub concepts: Vec<Link>,
}

/// The map of the domains of `docs`.
#[must_use]
pub fn map_page(docs: &GeneratedDocs) -> MapPage {
    let used = concepts_by_domain(docs);
    let domains: Vec<MapDomain> = docs
        .domains()
        .iter()
        .enumerate()
        .map(|(i, domain)| {
            let features: Vec<_> = docs.features_of(&domain.slug).collect();
            let use_cases = features
                .iter()
                .map(|f| docs.use_cases_of(&f.slug).count())
                .sum();
            let none = BTreeSet::new();
            let own = used.get(domain.slug.as_str()).unwrap_or(&none);
            MapDomain {
                node: format!("d{i}"),
                href: map_href(&domain.slug),
                name: domain.name.clone(),
                counts: format!(
                    "{}, {}",
                    plural(features.len(), "feature", "features"),
                    plural(use_cases, "use case", "use cases")
                ),
                shared: docs
                    .domains()
                    .iter()
                    .filter(|other| other.slug != domain.slug)
                    .filter_map(|other| {
                        let theirs = used.get(other.slug.as_str())?;
                        let concepts: Vec<Link> =
                            own.intersection(theirs).map(Concept::link).collect();
                        (!concepts.is_empty()).then(|| Shared {
                            with: Link {
                                href: map_href(&other.slug),
                                name: other.name.clone(),
                            },
                            concepts,
                        })
                    })
                    .collect(),
            }
        })
        .collect();
    MapPage {
        source: overview_source(&domains),
        domains,
    }
}

/// `flowchart TB`, a node per domain, an edge per pair of domains that share
/// concepts, labelled with the first of them.
fn overview_source(domains: &[MapDomain]) -> String {
    let nodes = domains.iter().map(|domain| {
        let text = format!("{}<br>{}", domain.name, domain.counts);
        format!("    {}[{}]", domain.node, label(&text))
    });
    let edges = domains.iter().enumerate().flat_map(|(i, domain)| {
        domain.shared.iter().filter_map(move |shared| {
            let other = domains[i + 1..]
                .iter()
                .find(|d| d.href == shared.with.href)?;
            Some(format!(
                "    {} ---|{}| {}",
                domain.node,
                label(&edge_text(&shared.concepts)),
                other.node
            ))
        })
    });
    let lines: Vec<String> = nodes.chain(edges).collect();
    format!("flowchart TB\n{}\n", lines.join("\n"))
}

/// The first [`EDGE_CONCEPTS`] names, then the number of the others: `A, B, C +2`.
fn edge_text(concepts: &[Link]) -> String {
    let names: Vec<&str> = concepts
        .iter()
        .take(EDGE_CONCEPTS)
        .map(|c| c.name.as_str())
        .collect();
    match concepts.len().checked_sub(EDGE_CONCEPTS) {
        Some(others) if others > 0 => format!("{} +{others}", names.join(", ")),
        _ => names.join(", "),
    }
}

/// `text` as a `mermaid` label: on one line, between double quotes, its own
/// double quotes written `#quot;` so that it can't close the label.
fn label(text: &str) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    format!("\"{}\"", line.replace('"', "#quot;"))
}

/// A concept of the glossary, ordered by name ignoring case.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Concept {
    key: String,
    name: String,
}

impl Concept {
    fn link(&self) -> Link {
        Link {
            href: format!("/glossary#{}", anchor(&self.name)),
            name: self.name.clone(),
        }
    }
}

/// The concepts the use cases of each domain use, by domain slug.
fn concepts_by_domain(docs: &GeneratedDocs) -> BTreeMap<&str, BTreeSet<Concept>> {
    let mut used: BTreeMap<&str, BTreeSet<Concept>> = BTreeMap::new();
    for entity in docs.glossary().merged_entities() {
        for (feature, _) in uses(docs, &entity.name, &entity.files) {
            used.entry(feature.domain_slug.as_str())
                .or_default()
                .insert(Concept {
                    key: entity.name.to_lowercase(),
                    name: entity.name.clone(),
                });
        }
    }
    used
}

fn map_href(domain: &str) -> String {
    format!("/map/{domain}")
}
