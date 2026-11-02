//! What the map pages show: the docs drawn as a graph by `mermaid`, from a
//! source built here, beside the same items as a list of links. The list is
//! what the drawn nodes open (`data-node`, wired by `diagrams.js`) and what a
//! reader follows when the graph does not draw.

use std::collections::{BTreeMap, BTreeSet};

use retrodoc_core::model::{ActorKind, Feature};
use retrodoc_mcp::GeneratedDocs;
use retrodoc_render::anchor;

use crate::glossary::uses;
use crate::view::{feature_href, home_crumb, plural, Link};

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

/// The map of a domain: its features, the humans of their use cases and the
/// concepts they use.
pub struct DomainMapPage {
    pub crumbs: Vec<Link>,
    /// The name of the domain.
    pub name: String,
    /// The page of the domain.
    pub href: String,
    /// The source of the graph `mermaid` draws.
    pub source: String,
    /// In the order of the run.
    pub features: Vec<MapFeature>,
}

/// A feature of the map of a domain, the item of its node.
pub struct MapFeature {
    pub node: MapItem,
    /// Its number of use cases, with its unit.
    pub counts: String,
    /// The humans of its use cases, sorted by name ignoring case.
    pub actors: Vec<MapItem>,
    /// The concepts its use cases use, sorted by name ignoring case.
    pub concepts: Vec<MapItem>,
}

/// A node of a map with the link it opens.
#[derive(Clone)]
pub struct MapItem {
    /// The id of its node in the source.
    pub node: String,
    pub href: String,
    pub name: String,
}

/// How an actor takes part in the use cases of a feature.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Part {
    /// The actor of one of their steps only: a dotted edge.
    Takes,
    /// The primary actor of one of them: a solid edge.
    Starts,
}

/// The map of the domain `slug` of `docs`, `None` when the run has none.
#[must_use]
pub fn domain_map_page(docs: &GeneratedDocs, slug: &str) -> Option<DomainMapPage> {
    let domain = docs.domains().iter().find(|d| d.slug == slug)?;
    let features: Vec<&Feature> = docs.features_of(slug).collect();
    let parts = actor_parts(docs, &features);
    let used = concepts_by_feature(docs, slug);
    let actors = items(parts.iter().map(|(name, _)| name.clone()), "a", |_| {
        "/actors".to_string()
    });
    let concepts = items(used.keys().map(|c| c.name.clone()), "c", |name| {
        format!("/glossary#{}", anchor(name))
    });
    let map_features: Vec<MapFeature> = features
        .iter()
        .enumerate()
        .map(|(i, feature)| MapFeature {
            node: MapItem {
                node: format!("f{i}"),
                href: feature_href(feature),
                name: feature.name.clone(),
            },
            counts: plural(
                docs.use_cases_of(&feature.slug).count(),
                "use case",
                "use cases",
            ),
            actors: parts
                .iter()
                .map(|(_, by_feature)| by_feature)
                .zip(&actors)
                .filter(|(by_feature, _)| by_feature.contains_key(feature.slug.as_str()))
                .map(|(_, actor)| actor.clone())
                .collect(),
            concepts: used
                .values()
                .zip(&concepts)
                .filter(|(slugs, _)| slugs.contains(feature.slug.as_str()))
                .map(|(_, concept)| concept.clone())
                .collect(),
        })
        .collect();
    let source = domain_source(&map_features, &actors, &concepts, &parts, &features);
    Some(DomainMapPage {
        crumbs: vec![
            home_crumb(),
            Link {
                href: "/map".into(),
                name: "Map".into(),
            },
        ],
        name: domain.name.clone(),
        href: format!("/domains/{slug}"),
        source,
        features: map_features,
    })
}

/// `flowchart LR`: the actors, the features, the concepts, then an edge from
/// each actor to the features it starts a use case of (`-->`) or only takes
/// part in (`-.->`), and from each feature to the concepts it uses (`---`).
fn domain_source(
    map_features: &[MapFeature],
    actors: &[MapItem],
    concepts: &[MapItem],
    parts: &[(String, BTreeMap<&str, Part>)],
    features: &[&Feature],
) -> String {
    let node_of = |slug: &str| {
        features
            .iter()
            .zip(map_features)
            .find(|(f, _)| f.slug == slug)
            .map(|(_, m)| m.node.node.as_str())
    };
    let actor_nodes = actors
        .iter()
        .map(|a| format!("    {}([{}])", a.node, label(&a.name)));
    let feature_nodes = map_features.iter().map(|f| {
        let text = format!("{}<br>{}", f.node.name, f.counts);
        format!("    {}[{}]", f.node.node, label(&text))
    });
    let concept_nodes = concepts
        .iter()
        .map(|c| format!("    {}[({})]", c.node, label(&c.name)));
    let actor_edges = parts
        .iter()
        .map(|(_, by_feature)| by_feature)
        .zip(actors)
        .flat_map(|(by_feature, actor)| {
            by_feature.iter().filter_map(move |(slug, part)| {
                let arrow = match part {
                    Part::Starts => "-->",
                    Part::Takes => "-.->",
                };
                Some(format!("    {} {arrow} {}", actor.node, node_of(slug)?))
            })
        });
    let concept_edges = map_features.iter().flat_map(|f| {
        f.concepts
            .iter()
            .map(move |c| format!("    {} --- {}", f.node.node, c.node))
    });
    let lines: Vec<String> = actor_nodes
        .chain(feature_nodes)
        .chain(concept_nodes)
        .chain(actor_edges)
        .chain(concept_edges)
        .collect();
    format!("flowchart LR\n{}\n", lines.join("\n"))
}

/// Each human of the use cases of `features`, sorted by name ignoring case,
/// with how it takes part in the use cases of each feature (by slug). A
/// primary actor is a human: it starts the use case.
fn actor_parts<'a>(
    docs: &'a GeneratedDocs,
    features: &[&'a Feature],
) -> Vec<(String, BTreeMap<&'a str, Part>)> {
    let mut parts: BTreeMap<(String, String), BTreeMap<&str, Part>> = BTreeMap::new();
    for feature in features {
        for use_case in docs.use_cases_of(&feature.slug) {
            let starts = use_case
                .primary_actor
                .iter()
                .filter(|name| !name.trim().is_empty())
                .map(|name| (name, Part::Starts));
            let takes = use_case
                .steps
                .iter()
                .filter(|step| step.actor.kind == ActorKind::Human)
                .map(|step| (&step.actor.name, Part::Takes));
            for (name, part) in starts.chain(takes) {
                let seen = parts
                    .entry((name.to_lowercase(), name.clone()))
                    .or_default()
                    .entry(feature.slug.as_str())
                    .or_insert(part);
                *seen = (*seen).max(part);
            }
        }
    }
    parts
        .into_iter()
        .map(|((_, name), by_feature)| (name, by_feature))
        .collect()
}

/// The concepts the use cases of the domain `slug` use, sorted by name
/// ignoring case, each with the slugs of the features that use it.
fn concepts_by_feature<'a>(
    docs: &'a GeneratedDocs,
    slug: &str,
) -> BTreeMap<Concept, BTreeSet<&'a str>> {
    let mut used: BTreeMap<Concept, BTreeSet<&str>> = BTreeMap::new();
    for entity in docs.glossary().merged_entities() {
        let features: BTreeSet<&str> = uses(docs, &entity.name, &entity.files)
            .into_iter()
            .filter(|(feature, _)| feature.domain_slug == slug)
            .map(|(feature, _)| feature.slug.as_str())
            .collect();
        if !features.is_empty() {
            used.insert(
                Concept {
                    key: entity.name.to_lowercase(),
                    name: entity.name.clone(),
                },
                features,
            );
        }
    }
    used
}

/// `names` as the nodes `<prefix>0`, `<prefix>1`… linked to `href` of each.
fn items(
    names: impl Iterator<Item = String>,
    prefix: &str,
    href: impl Fn(&str) -> String,
) -> Vec<MapItem> {
    names
        .enumerate()
        .map(|(i, name)| MapItem {
            node: format!("{prefix}{i}"),
            href: href(&name),
            name,
        })
        .collect()
}

pub(crate) fn map_href(domain: &str) -> String {
    format!("/map/{domain}")
}
