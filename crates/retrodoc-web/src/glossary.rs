//! What the glossary page shows: the business concepts of the run, by name,
//! each with its definition and the use cases that use it.

use std::path::PathBuf;

use retrodoc_core::model::{Feature, UseCase};
use retrodoc_mcp::GeneratedDocs;
use retrodoc_render::anchor;

use crate::view::{use_case_href, Link};

pub struct GlossaryPage {
    /// Sorted by name, ignoring case.
    pub concepts: Vec<Concept>,
}

pub struct Concept {
    pub name: String,
    /// The `id` of its heading: `anchor` of its name.
    pub anchor: String,
    pub definition: String,
    pub use_cases: Vec<Link>,
}

/// The glossary page of `docs`.
#[must_use]
pub fn glossary_page(docs: &GeneratedDocs) -> GlossaryPage {
    let mut concepts: Vec<Concept> = docs
        .glossary()
        .merged_entities()
        .iter()
        .map(|entity| Concept {
            name: entity.name.clone(),
            anchor: anchor(&entity.name),
            definition: entity.description.clone(),
            use_cases: users(docs, &entity.name, &entity.files),
        })
        .collect();
    concepts.sort_by_key(|c| c.name.to_lowercase());
    GlossaryPage { concepts }
}

/// The use cases that use the concept `name`: one of their steps cites one of
/// the `files` it was read from, or a rule that applies to them names it. In
/// the order of the run.
fn users(docs: &GeneratedDocs, name: &str, files: &[PathBuf]) -> Vec<Link> {
    let files: Vec<String> = files.iter().map(|f| plain(&f.to_string_lossy())).collect();
    docs.features()
        .iter()
        .flat_map(|feature| {
            docs.use_cases_of(&feature.slug)
                .map(move |use_case| (feature, use_case))
        })
        .filter(|(feature, use_case)| {
            cites_a_file(use_case, &files) || ruled(docs, feature, use_case, name)
        })
        .map(|(feature, use_case)| Link {
            href: use_case_href(feature, use_case),
            name: use_case.name.clone(),
        })
        .collect()
}

fn cites_a_file(use_case: &UseCase, files: &[String]) -> bool {
    use_case
        .steps
        .iter()
        .flat_map(|s| &s.source_refs)
        .any(|r| files.contains(&plain(&r.path)))
}

/// A rule of `feature` that applies to `use_case` names the concept `name`.
fn ruled(docs: &GeneratedDocs, feature: &Feature, use_case: &UseCase, name: &str) -> bool {
    docs.rules().iter().any(|rule| {
        rule.belongs_to_feature(&feature.slug)
            && rule.use_cases.contains(&use_case.slug)
            && rule
                .entities
                .iter()
                .any(|e| e.trim().eq_ignore_ascii_case(name.trim()))
    })
}

/// `./app/a.rb` and `app\a.rb` are `app/a.rb`.
fn plain(path: &str) -> String {
    let path = path.replace('\\', "/");
    path.trim_start_matches("./").to_string()
}
