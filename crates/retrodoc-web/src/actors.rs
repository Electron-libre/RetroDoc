//! What the actors page shows (`specs/web-actors.md`): an index of the
//! actors, then a section per actor with the use cases it starts or takes
//! part in, by domain and feature, and the permission rules of those use
//! cases. The actors are grouped by their exact name: the LLM names them per
//! use case, and merging two spellings is left out.

use std::collections::{BTreeSet, HashSet};

use retrodoc_core::model::{ActorKind, Feature, RuleKind, UseCase};
use retrodoc_docs::GeneratedDocs;

use crate::texts::WebTexts;
use crate::view::{domain_href, feature_href, rule_href, use_case_href, Link};

pub struct ActorsPage {
    /// The human actors then the system ones, a group with no actor left
    /// out.
    pub groups: Vec<ActorGroup>,
}

pub struct ActorGroup {
    /// The `id` of its list in the index.
    pub id: &'static str,
    pub heading: &'static str,
    /// Sorted by name, ignoring case.
    pub actors: Vec<ActorSection>,
}

pub struct ActorSection {
    /// The `id` of its section (WAC-7).
    pub id: String,
    pub name: String,
    /// The use cases it starts (its primary actor).
    pub starts: usize,
    /// The other use cases it takes part in (the actor of one of their
    /// steps).
    pub takes_part: usize,
    /// In the order of the run, those with none of its use cases left out.
    pub domains: Vec<ActorDomain>,
    /// The permission rules that apply to one of its use cases.
    pub permissions: Vec<Link>,
}

pub struct ActorDomain {
    pub link: Link,
    pub features: Vec<ActorFeature>,
}

pub struct ActorFeature {
    pub link: Link,
    /// Those it starts first, then the others, each in the order of the run.
    pub use_cases: Vec<ActorUseCase>,
}

/// A use case the actor starts (its primary actor) or takes part in.
pub struct ActorUseCase {
    pub link: Link,
    pub starts: bool,
}

/// The actors page of `docs`.
#[must_use]
pub fn actors_page(docs: &GeneratedDocs, texts: &WebTexts) -> ActorsPage {
    let domains = domains_in_order(docs);
    let all = || docs.use_cases().iter();
    let mut actors: BTreeSet<(String, bool)> = all()
        .flat_map(|u| &u.steps)
        .filter(|s| !s.actor.name.trim().is_empty())
        .map(|s| (s.actor.name.clone(), s.actor.kind == ActorKind::Human))
        .collect();
    // A primary actor no step names is a human one.
    for name in all().filter_map(primary) {
        if !actors.iter().any(|(n, _)| n == name) {
            actors.insert((name.to_string(), true));
        }
    }
    // A primary actor has no kind: it starts its use cases under one kind
    // only, the human one of its name when there is one.
    let section = |(name, human): &(String, bool)| {
        let starts = *human || !actors.contains(&(name.clone(), true));
        actor_section(docs, &domains, name, *human, starts)
    };
    let (human, system): (Vec<_>, Vec<_>) = actors.iter().partition(|(_, human)| *human);
    let sorted = |actors: Vec<&(String, bool)>| {
        let mut sections: Vec<ActorSection> = actors.into_iter().map(section).collect();
        sections.sort_by(|a, b| {
            (a.name.to_lowercase(), &a.name).cmp(&(b.name.to_lowercase(), &b.name))
        });
        sections
    };
    let mut groups: Vec<ActorGroup> = [
        ("human-actors", texts.people, "actor", sorted(human)),
        ("system-actors", texts.systems, "system", sorted(system)),
    ]
    .into_iter()
    .filter(|(_, _, _, actors)| !actors.is_empty())
    .map(|(id, heading, prefix, mut actors)| {
        for actor in &mut actors {
            actor.id = format!("{prefix}-{}", slug(&actor.name));
        }
        ActorGroup {
            id,
            heading,
            actors,
        }
    })
    .collect();
    // The lists of the index hold the ids `human-actors` and `system-actors`.
    let mut taken: HashSet<String> = groups.iter().map(|g| g.id.to_string()).collect();
    for actor in groups.iter_mut().flat_map(|g| &mut g.actors) {
        actor.id = unique(&actor.id, &mut taken);
    }
    ActorsPage { groups }
}

/// `name` in lower case, each run of characters other than ASCII letters
/// and digits read as one `-`, none at either end; `actor` when none is
/// left.
fn slug(name: &str) -> String {
    let words: Vec<String> = name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    if words.is_empty() {
        "actor".to_string()
    } else {
        words.join("-")
    }
}

/// `id`, or `id-2`, `id-3`… when an earlier section took it.
fn unique(id: &str, taken: &mut HashSet<String>) -> String {
    let mut free = id.to_string();
    let mut n = 1;
    while taken.contains(&free) {
        n += 1;
        free = format!("{id}-{n}");
    }
    taken.insert(free.clone());
    free
}

/// The domains of the run in its order, with their features in order; a
/// feature whose domain the run lists not comes after them, under its slug.
fn domains_in_order(docs: &GeneratedDocs) -> Vec<(Link, Vec<&Feature>)> {
    let mut domains: Vec<(Link, Vec<&Feature>)> = docs
        .domains()
        .iter()
        .map(|d| {
            let link = Link {
                href: domain_href(&d.slug),
                name: d.name.clone(),
            };
            (link, docs.features_of(&d.slug).collect())
        })
        .collect();
    for feature in docs.features() {
        if docs.domains().iter().any(|d| d.slug == feature.domain_slug) {
            continue;
        }
        let href = domain_href(&feature.domain_slug);
        match domains.iter_mut().find(|(link, _)| link.href == href) {
            Some((_, features)) => features.push(feature),
            None => domains.push((
                Link {
                    href,
                    name: feature.domain_slug.clone(),
                },
                vec![feature],
            )),
        }
    }
    domains
}

fn primary(use_case: &UseCase) -> Option<&str> {
    use_case
        .primary_actor
        .as_deref()
        .filter(|name| !name.trim().is_empty())
}

fn actor_section(
    docs: &GeneratedDocs,
    domains: &[(Link, Vec<&Feature>)],
    name: &str,
    human: bool,
    starts_here: bool,
) -> ActorSection {
    let mut permissions = Vec::new();
    let (mut starts, mut takes_part) = (0, 0);
    let mut sections = Vec::new();
    for (domain, features) in domains {
        let mut shown = Vec::new();
        for feature in features {
            let mut use_cases: Vec<(&UseCase, bool)> = docs
                .use_cases_of(&feature.slug)
                .filter_map(|u| {
                    let starts = starts_here && primary(u) == Some(name);
                    let takes_part = u.steps.iter().any(|s| {
                        s.actor.name == name && (s.actor.kind == ActorKind::Human) == human
                    });
                    (starts || takes_part).then_some((u, starts))
                })
                .collect();
            if use_cases.is_empty() {
                continue;
            }
            use_cases.sort_by_key(|(_, starts)| !starts);
            let started = use_cases.iter().filter(|(_, s)| *s).count();
            starts += started;
            takes_part += use_cases.len() - started;
            permissions.extend(
                docs.rules()
                    .iter()
                    .filter(|r| {
                        r.kind == RuleKind::Permission
                            && r.belongs_to_feature(&feature.slug)
                            && use_cases.iter().any(|(u, _)| r.use_cases.contains(&u.slug))
                    })
                    .map(|rule| Link {
                        href: rule_href(feature, &rule.slug),
                        name: rule.statement.clone(),
                    }),
            );
            shown.push(ActorFeature {
                link: Link {
                    href: feature_href(feature),
                    name: feature.name.clone(),
                },
                use_cases: use_cases
                    .into_iter()
                    .map(|(u, starts)| ActorUseCase {
                        link: Link {
                            href: use_case_href(feature, u),
                            name: u.name.clone(),
                        },
                        starts,
                    })
                    .collect(),
            });
        }
        if !shown.is_empty() {
            sections.push(ActorDomain {
                link: Link {
                    href: domain.href.clone(),
                    name: domain.name.clone(),
                },
                features: shown,
            });
        }
    }
    ActorSection {
        id: String::new(),
        name: name.to_string(),
        starts,
        takes_part,
        domains: sections,
        permissions,
    }
}
