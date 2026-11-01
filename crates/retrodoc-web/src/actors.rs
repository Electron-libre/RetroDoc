//! What the actors page shows: who starts or takes part in which use cases,
//! an actor per row and a feature per column, with the permission rules of
//! its use cases. The actors are grouped by their exact name: the LLM names
//! them per use case, and merging two spellings is left out.

use std::collections::BTreeSet;

use retrodoc_core::model::{ActorKind, Feature, RuleKind, UseCase};
use retrodoc_mcp::GeneratedDocs;

use crate::view::{feature_href, rule_anchor, use_case_href, Link};

pub struct ActorsPage {
    /// The features with a use case that names an actor, in the order of
    /// the run.
    pub features: Vec<Link>,
    /// The human actors then the system ones, a group with no actor left
    /// out.
    pub groups: Vec<ActorGroup>,
}

pub struct ActorGroup {
    /// The `id` of its table.
    pub id: &'static str,
    pub heading: &'static str,
    /// Sorted by name, ignoring case.
    pub rows: Vec<ActorRow>,
}

pub struct ActorRow {
    pub name: String,
    /// A cell per column of [`ActorsPage::features`].
    pub cells: Vec<Vec<Cell>>,
    /// The permission rules that apply to a use case of its cells.
    pub permissions: Vec<Link>,
}

/// A use case the actor starts (its primary actor) or takes part in.
pub struct Cell {
    pub use_case: Link,
    pub starts: bool,
}

/// The actors page of `docs`.
#[must_use]
pub fn actors_page(docs: &GeneratedDocs) -> ActorsPage {
    let features: Vec<(&Feature, Vec<&UseCase>)> = docs
        .features()
        .iter()
        .map(|f| (f, docs.use_cases_of(&f.slug).collect::<Vec<_>>()))
        .filter(|(_, use_cases)| use_cases.iter().any(|u| names_an_actor(u)))
        .collect();
    let all = || features.iter().flat_map(|(_, u)| u.iter().copied());
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
    // A primary actor has no kind: it starts its use cases in one row only,
    // the human one of its name when there is one.
    let row = |(name, human): &(String, bool)| {
        let starts = *human || !actors.contains(&(name.clone(), true));
        actor_row(docs, &features, name, *human, starts)
    };
    let (human, system): (Vec<_>, Vec<_>) = actors.iter().partition(|(_, human)| *human);
    let sorted = |actors: Vec<&(String, bool)>| {
        let mut rows: Vec<ActorRow> = actors.into_iter().map(row).collect();
        rows.sort_by(|a, b| {
            (a.name.to_lowercase(), &a.name).cmp(&(b.name.to_lowercase(), &b.name))
        });
        rows
    };
    ActorsPage {
        features: features
            .iter()
            .map(|(f, _)| Link {
                href: feature_href(f),
                name: f.name.clone(),
            })
            .collect(),
        groups: [
            ("human-actors", "People", sorted(human)),
            ("system-actors", "Systems", sorted(system)),
        ]
        .into_iter()
        .filter(|(_, _, rows)| !rows.is_empty())
        .map(|(id, heading, rows)| ActorGroup { id, heading, rows })
        .collect(),
    }
}

fn primary(use_case: &UseCase) -> Option<&str> {
    use_case
        .primary_actor
        .as_deref()
        .filter(|name| !name.trim().is_empty())
}

fn names_an_actor(use_case: &UseCase) -> bool {
    primary(use_case).is_some()
        || use_case
            .steps
            .iter()
            .any(|s| !s.actor.name.trim().is_empty())
}

fn actor_row(
    docs: &GeneratedDocs,
    features: &[(&Feature, Vec<&UseCase>)],
    name: &str,
    human: bool,
    starts_here: bool,
) -> ActorRow {
    let mut permissions = Vec::new();
    let cells = features
        .iter()
        .map(|(feature, use_cases)| {
            let cell: Vec<(&UseCase, bool)> = use_cases
                .iter()
                .filter_map(|u| {
                    let starts = starts_here && primary(u) == Some(name);
                    let takes_part = u.steps.iter().any(|s| {
                        s.actor.name == name && (s.actor.kind == ActorKind::Human) == human
                    });
                    (starts || takes_part).then_some((*u, starts))
                })
                .collect();
            for rule in docs.rules().iter().filter(|r| {
                r.kind == RuleKind::Permission
                    && r.belongs_to_feature(&feature.slug)
                    && cell.iter().any(|(u, _)| r.use_cases.contains(&u.slug))
            }) {
                permissions.push(Link {
                    href: format!("{}#{}", feature_href(feature), rule_anchor(&rule.slug)),
                    name: rule.statement.clone(),
                });
            }
            cell.into_iter()
                .map(|(u, starts)| Cell {
                    use_case: Link {
                        href: use_case_href(feature, u),
                        name: u.name.clone(),
                    },
                    starts,
                })
                .collect()
        })
        .collect();
    ActorRow {
        name: name.to_string(),
        cells,
        permissions,
    }
}
