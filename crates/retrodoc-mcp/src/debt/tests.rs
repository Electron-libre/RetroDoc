use retrodoc_core::model::{
    Actor, ActorKind, ConfidenceScore, Domain, Feature, SetApartReason, SourceRef, Step, UseCase,
};
use retrodoc_pipeline::cache::hash_content;
use retrodoc_pipeline::{Glossary, SourceHashes};

use super::*;
use crate::freshness::FileState;

fn feature(slug: &str, domain: &str, confidence: Option<f32>) -> Feature {
    Feature {
        back_office: false,
        technical: false,
        slug: slug.into(),
        domain_slug: domain.into(),
        name: slug.into(),
        description: String::new(),
        source_paths: vec![format!("{slug}.rb")],
        entry_points: Vec::new(),
        confidence: confidence.map(|c| ConfidenceScore::new(c, None)),
    }
}

fn use_case(slug: &str, feature: &str, confidence: Option<f32>) -> UseCase {
    UseCase {
        entry_points: Vec::new(),
        primary_actor: None,
        narrative: None,
        business_language: None,
        slug: slug.into(),
        feature_slug: feature.into(),
        name: slug.into(),
        description: String::new(),
        steps: vec![Step {
            order: 1,
            description: String::new(),
            actor: Actor {
                name: "Clerk".into(),
                kind: ActorKind::Human,
            },
            action: String::new(),
            source_refs: vec![SourceRef {
                path: format!("{slug}.rb"),
                start_line: Some(1),
                end_line: None,
            }],
        }],
        diagram_mermaid: None,
        confidence: confidence.map(|c| ConfidenceScore::new(c, format!("why {slug}"))),
        narrative_check: None,
    }
}

fn docs(features: Vec<Feature>, use_cases: Vec<UseCase>) -> GeneratedDocs {
    let domains = ["billing", "auth"]
        .map(|slug| Domain {
            slug: slug.into(),
            name: slug.to_uppercase(),
            description: String::new(),
            confidence: None,
        })
        .to_vec();
    GeneratedDocs::new(domains, features, use_cases, Glossary::default(), &[])
}

fn kinds(list: &DebtList) -> Vec<(DebtKind, String)> {
    list.items
        .iter()
        .map(|i| (i.kind, i.section.label()))
        .collect()
}

// spec: DBT-2, DBT-3, DBT-8 (internal: the read model of the debt page, shown by DBT-1)
#[test]
fn the_weak_and_unscored_sections_are_listed_lowest_score_first() {
    let docs = docs(
        vec![
            feature("pay", "billing", Some(0.9)),
            feature("orphan", "auth", Some(0.1)),
            feature("weak", "billing", Some(0.2)),
        ],
        vec![
            use_case("refund", "pay", Some(0.4)),
            use_case("void", "pay", Some(0.3)),
            use_case("check", "pay", None),
            use_case("fine", "pay", Some(0.9)),
            use_case("explained", "weak", Some(0.9)),
        ],
    );

    let list = debt(&docs);

    let low: Vec<_> = kinds(&list)
        .into_iter()
        .filter(|(k, _)| *k != DebtKind::SetApart)
        .collect();
    assert_eq!(
        low,
        [
            (DebtKind::LowConfidence, "auth/orphan".to_string()),
            (DebtKind::LowConfidence, "billing/pay/void".to_string()),
            (DebtKind::LowConfidence, "billing/pay/refund".to_string()),
            (DebtKind::NotScored, "billing/pay/check".to_string()),
        ]
    );
    assert_eq!(list.items[1].score, Some(0.3));
    assert_eq!(list.items[1].rationale.as_deref(), Some("why void"));
    assert!(!list.no_score);
}

// spec: DBT-3 (internal: the read model of the debt page, shown by DBT-1)
#[test]
fn a_run_with_no_score_lists_no_unscored_use_case() {
    let docs = docs(
        vec![feature("pay", "billing", None)],
        vec![
            use_case("refund", "pay", None),
            use_case("void", "pay", None),
        ],
    );

    let list = debt(&docs);

    assert!(list.no_score);
    assert!(list.items.is_empty(), "{:?}", kinds(&list));
}

// spec: DBT-4, DBT-5, DBT-6, DBT-8 (internal: the read model of the debt page, shown by DBT-1)
#[test]
fn unbacked_technical_and_set_apart_use_cases_have_their_own_kinds() {
    let mut unbacked = use_case("unbacked", "pay", Some(0.9));
    unbacked.narrative_check = Some(ConfidenceScore::new(0.0, "not in the code".to_string()));
    let mut partial = use_case("partial", "pay", Some(0.9));
    partial.narrative_check = Some(ConfidenceScore::new(0.5, None));
    let mut technical = use_case("helper", "pay", Some(0.55));
    technical.business_language = Some(ConfidenceScore::new(0.2, "reads like code".to_string()));
    let mut admin = feature("admin", "auth", Some(0.9));
    admin.back_office = true;
    let docs = docs(
        vec![feature("pay", "billing", Some(0.9)), admin],
        vec![
            unbacked,
            partial,
            technical,
            use_case("reimport", "admin", Some(0.9)),
        ],
    );

    let list = debt(&docs);

    assert_eq!(
        kinds(&list),
        [
            (
                DebtKind::UnbackedNarrative,
                "billing/pay/unbacked".to_string()
            ),
            (
                DebtKind::TechnicalLanguage,
                "billing/pay/helper".to_string()
            ),
            (DebtKind::SetApart, "billing/pay/helper".to_string()),
            (DebtKind::SetApart, "auth/admin/reimport".to_string()),
        ]
    );
    assert_eq!(list.items[0].rationale.as_deref(), Some("not in the code"));
    assert_eq!(list.items[1].score, Some(0.2));
    assert_eq!(
        list.items[2].reasons,
        [SetApartReason::BusinessLanguage, SetApartReason::Confidence]
    );
    assert_eq!(list.items[3].reasons, [SetApartReason::BackOffice]);
}

// spec: DBT-7, DBT-8 (internal: the read model of the debt page, shown by DBT-1)
#[test]
fn the_sections_citing_a_changed_file_are_stale() {
    let dir = tempfile::tempdir().unwrap();
    let mut hashes = SourceHashes::default();
    for name in ["pay.rb", "refund.rb", "void.rb"] {
        std::fs::write(dir.path().join(name), name).unwrap();
        hashes.put(name, &hash_content(name));
    }
    hashes.save(dir.path()).unwrap();
    std::fs::write(dir.path().join("pay.rb"), "edited").unwrap();
    std::fs::remove_file(dir.path().join("void.rb")).unwrap();
    let docs = docs(
        vec![feature("pay", "billing", Some(0.9))],
        vec![
            use_case("refund", "pay", Some(0.9)),
            use_case("void", "pay", Some(0.9)),
            // Its file has no recorded hash: not judged.
            use_case("budget", "pay", Some(0.9)),
        ],
    )
    .with_freshness(Freshness::load(dir.path()));

    let list = debt(&docs);

    assert_eq!(
        kinds(&list),
        [
            (DebtKind::Stale, "billing/pay".to_string()),
            (DebtKind::Stale, "billing/pay/void".to_string()),
        ]
    );
    assert_eq!(
        list.items[0].changed,
        [("pay.rb".to_string(), FileState::Modified)]
    );
    assert_eq!(
        list.items[1].changed,
        [("void.rb".to_string(), FileState::Deleted)]
    );
}

#[test]
fn a_use_case_whose_feature_is_not_in_the_run_has_no_domain() {
    let docs = docs(vec![], vec![use_case("lost", "gone", Some(0.1))]);

    let list = debt(&docs);

    assert_eq!(list.items[0].section.domain, None);
}
