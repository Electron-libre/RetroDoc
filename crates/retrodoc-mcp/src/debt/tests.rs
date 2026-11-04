use retrodoc_core::model::{
    Actor, ActorKind, BusinessRule, ConfidenceScore, Domain, Feature, RuleEvidence, SetApartReason,
    SourceRef, Step, UseCase,
};
use retrodoc_pipeline::cache::hash_content;
use retrodoc_pipeline::reconcile::{
    Contradiction, ItemKind, ItemVerdict, Reconciliation, SectionRef, Status,
};
use retrodoc_pipeline::{BehaviourCoverage, Glossary, SourceHashes};

use std::collections::HashMap;
use std::path::PathBuf;

use super::*;
use crate::freshness::FileState;
use crate::read_model::Uncovered;

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

fn low_items(list: &DebtList) -> Vec<&DebtItem> {
    list.items
        .iter()
        .filter(|i| i.kind != DebtKind::SetApart)
        .collect()
}

fn kinds(list: &DebtList) -> Vec<(DebtKind, String)> {
    list.items.iter().map(|i| (i.kind, i.label())).collect()
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
            (DebtKind::NotScored, "billing/pay/check".to_string()),
            (DebtKind::LowConfidence, "auth/orphan".to_string()),
            (DebtKind::LowConfidence, "billing/pay/void".to_string()),
            (DebtKind::LowConfidence, "billing/pay/refund".to_string()),
        ]
    );
    let void = &low_items(&list)[2];
    assert_eq!(void.score, Some(0.3));
    assert_eq!(void.rationale.as_deref(), Some("why void"));
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
            (DebtKind::SetApart, "billing/pay/helper".to_string()),
            (DebtKind::SetApart, "auth/admin/reimport".to_string()),
            (
                DebtKind::TechnicalLanguage,
                "billing/pay/helper".to_string()
            ),
        ]
    );
    assert_eq!(list.items[0].rationale.as_deref(), Some("not in the code"));
    assert_eq!(list.items[3].score, Some(0.2));
    assert_eq!(
        list.items[1].reasons,
        [SetApartReason::BusinessLanguage, SetApartReason::Confidence]
    );
    assert_eq!(list.items[2].reasons, [SetApartReason::BackOffice]);
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

    assert_eq!(list.items[0].domain(), None);
}

fn verdict(kind: ItemKind, label: &str, status: Status, entry_points: usize) -> ItemVerdict {
    ItemVerdict {
        kind,
        label: label.into(),
        domain: label.split('/').next().unwrap_or_default().into(),
        name: format!("name of {label}"),
        entry_points,
        confidence: None,
        status,
        sections: Vec::new(),
        contradictions: Vec::new(),
    }
}

// spec: DBT-15, DBT-16, DBT-8 (internal: the read model of the debt page, shown by DBT-1)
#[test]
fn the_contradicted_and_undocumented_items_follow_the_sections() {
    let mut contradicted = verdict(
        ItemKind::UseCase,
        "billing/pay/refund",
        Status::Contradicted,
        1,
    );
    contradicted.contradictions = ["README.md#Refunds", "docs/faq.md"]
        .map(|origin| Contradiction {
            section: SectionRef::parse(origin),
            doc_states: format!("{origin} says 60 days"),
            item_states: "30 days".into(),
        })
        .to_vec();
    let reconciliation = Reconciliation {
        items: vec![
            verdict(ItemKind::Domain, "billing", Status::Undocumented, 1),
            contradicted,
            verdict(ItemKind::Feature, "billing/pay", Status::Matched, 3),
            verdict(ItemKind::Feature, "auth/login", Status::Documented, 3),
            verdict(ItemKind::Rule, "billing/pay/pay-1", Status::Undocumented, 4),
        ],
    };
    let docs = docs(
        vec![feature("pay", "billing", Some(0.9))],
        vec![use_case("refund", "pay", None)],
    )
    .with_uncovered(Uncovered {
        reconciliation: Some(reconciliation),
        ..Uncovered::default()
    });

    let list = debt(&docs);

    let doc_kinds: Vec<_> = kinds(&list)
        .into_iter()
        .filter(|(k, _)| matches!(k, DebtKind::Contradicted | DebtKind::Undocumented))
        .collect();
    assert_eq!(
        doc_kinds,
        [
            (DebtKind::Undocumented, "billing/pay/pay-1".to_string()),
            (DebtKind::Contradicted, "billing/pay/refund".to_string()),
            (DebtKind::Undocumented, "billing".to_string()),
        ]
    );
    assert!(!list.no_docs);
    let contradicted = list
        .items
        .iter()
        .find(|i| i.kind == DebtKind::Contradicted)
        .unwrap();
    let Subject::DocItem(item) = &contradicted.subject else {
        panic!("not a doc item");
    };
    assert_eq!(item.kind, ItemKind::UseCase);
    assert_eq!(item.contradictions.len(), 2);
    assert_eq!(contradicted.domain(), Some("billing"));
}

// spec: DBT-17 (internal: the read model of the debt page, shown by DBT-1)
#[test]
fn with_no_collected_doc_no_item_is_compared() {
    let docs = docs(vec![feature("pay", "billing", Some(0.9))], vec![]);

    let list = debt(&docs);

    assert!(list.no_docs);
    assert!(list.items.is_empty(), "{:?}", kinds(&list));
}

// spec: DBT-19, DBT-20, DBT-21, DBT-22, DBT-8 (internal: the read model of the debt page, shown by DBT-1)
#[test]
fn the_code_no_doc_covers_is_listed_by_kind_and_folder() {
    let paths = |paths: &[&str]| paths.iter().map(PathBuf::from).collect::<Vec<_>>();
    let docs = docs(vec![feature("pay", "billing", Some(0.9))], vec![]).with_uncovered(Uncovered {
        reconciliation: None,
        no_domain: paths(&["Rakefile"]),
        skipped: paths(&["vendor/a.rb"]),
        coverage: Some(BehaviourCoverage {
            unplaced_entry_points: vec!["app/x.rb#create".into(), "app/x.rb#delete".into()],
            technical_support: paths(&["lib/c.rb", "app/b.rb", "app/a.rb"]),
            ..BehaviourCoverage::default()
        }),
    });

    let list = debt(&docs);

    assert_eq!(
        kinds(&list),
        [
            (DebtKind::EntryPointWithoutFeature, "app".to_string()),
            (DebtKind::NoDomain, ".".to_string()),
            (DebtKind::NoBehaviour, "app".to_string()),
            (DebtKind::NoBehaviour, "lib".to_string()),
            (DebtKind::NotAnalysed, "vendor".to_string()),
        ]
    );
    let Subject::Folder(app) = &list.items[2].subject else {
        panic!("not a folder");
    };
    assert_eq!(app.entries, ["app/b.rb", "app/a.rb"]);
    let Subject::Folder(entries) = &list.items[0].subject else {
        panic!("not a folder");
    };
    assert_eq!(entries.entries, ["app/x.rb#create", "app/x.rb#delete"]);
    assert!(list.items.iter().all(|i| i.domain().is_none()));
}

// spec: DBT-21 (internal: the read model of the debt page, shown by DBT-1)
#[test]
fn an_entry_point_saved_before_ids_names_no_file() {
    let uncovered = Uncovered {
        coverage: Some(BehaviourCoverage {
            unplaced_entry_points: vec!["Billing#purge".into(), "app/x.rb#create".into()],
            ..BehaviourCoverage::default()
        }),
        ..Uncovered::default()
    };
    assert_eq!(entry_file("Billing#purge"), None);
    assert_eq!(entry_file("app/x.rb#create"), Some("app/x.rb"));
    assert_eq!(uncovered.paths(), ["app/x.rb"]);

    let list = debt(&docs(vec![], vec![]).with_uncovered(uncovered));

    assert_eq!(
        kinds(&list),
        [
            (DebtKind::EntryPointWithoutFeature, ".".to_string()),
            (DebtKind::EntryPointWithoutFeature, "app".to_string()),
        ]
    );
}

fn churn(counts: &[(&str, u32)]) -> HashMap<String, u32> {
    counts.iter().map(|(p, c)| ((*p).to_string(), *c)).collect()
}

fn priority_of<'a>(list: &'a DebtList, kind: DebtKind, label: &str) -> &'a Priority {
    &list
        .items
        .iter()
        .find(|i| i.kind == kind && i.label() == label)
        .unwrap_or_else(|| panic!("no {kind:?} {label}"))
        .priority
}

// spec: DBT-23, DBT-24 (internal: the read model of the debt page, shown by DBT-27)
#[test]
fn the_priority_crosses_the_weakness_the_commits_and_the_entry_points() {
    let mut weak = feature("pay", "billing", Some(0.2));
    weak.entry_points = vec!["pay.rb#create".into(), "pay.rb#update".into()];
    let mut admin = feature("admin", "auth", Some(0.9));
    admin.back_office = true;
    let docs = docs(
        vec![weak, admin],
        vec![use_case("reimport", "admin", Some(0.9))],
    )
    .with_churn(churn(&[("pay.rb", 6)]));

    let list = debt(&docs);

    let low = priority_of(&list, DebtKind::LowConfidence, "billing/pay");
    assert_eq!((low.weakness, low.commits, low.entry_points), (0.8, 6, 2));
    let expected = f64::from(0.8_f32) * (1.0 + 6_f64.ln_1p()) * (1.0 + 2_f64.ln_1p());
    assert!((low.value() - expected).abs() < 1e-9, "{}", low.value());
    let set_apart = priority_of(&list, DebtKind::SetApart, "auth/admin/reimport");
    assert_eq!(
        (
            set_apart.weakness,
            set_apart.commits,
            set_apart.entry_points
        ),
        (1.0, 0, 0)
    );
    assert!((set_apart.value() - 1.0).abs() < 1e-9);
}

// spec: DBT-25, DBT-26 (internal: the read model of the debt page, shown by DBT-27)
#[test]
fn the_commits_and_entry_points_come_from_the_files_and_entries_of_each_item() {
    let mut refund = use_case("refund", "pay", Some(0.1));
    refund.steps.push(refund.steps[0].clone());
    refund.steps[1].source_refs.push(SourceRef {
        path: "money.rb".into(),
        start_line: None,
        end_line: None,
    });
    refund.entry_points = vec!["refund.rb#create".into()];
    let mut pay = feature("pay", "billing", Some(0.9));
    pay.source_paths = vec!["pay.rb".into(), "money.rb".into()];
    let mut rule = BusinessRule {
        slug: "pay-1".into(),
        statement: String::new(),
        kind: retrodoc_core::model::RuleKind::Validation,
        entities: Vec::new(),
        use_cases: Vec::new(),
        evidence: Vec::new(),
        confidence: None,
    };
    rule.evidence = ["money.rb", "money.rb", "rule.rb"]
        .map(|path| RuleEvidence::Code {
            source: SourceRef {
                path: path.into(),
                start_line: None,
                end_line: None,
            },
        })
        .to_vec();
    let reconciliation = Reconciliation {
        items: vec![
            verdict(ItemKind::Domain, "billing", Status::Undocumented, 3),
            verdict(ItemKind::Feature, "billing/pay", Status::Undocumented, 0),
            verdict(
                ItemKind::UseCase,
                "billing/pay/refund",
                Status::Undocumented,
                1,
            ),
            verdict(ItemKind::Rule, "billing/pay/pay-1", Status::Undocumented, 0),
            verdict(ItemKind::Feature, "?/gone", Status::Undocumented, 0),
        ],
    };
    let docs = docs(vec![pay], vec![refund])
        .with_rules(vec![rule])
        .with_uncovered(Uncovered {
            reconciliation: Some(reconciliation),
            no_domain: vec![PathBuf::from("lib/a.rb"), PathBuf::from("lib/b.rb")],
            coverage: Some(BehaviourCoverage {
                unplaced_entry_points: vec!["app/x.rb#create".into(), "app/x.rb#delete".into()],
                ..BehaviourCoverage::default()
            }),
            ..Uncovered::default()
        })
        .with_churn(churn(&[
            ("pay.rb", 1),
            ("money.rb", 10),
            ("refund.rb", 100),
            ("rule.rb", 1000),
            ("lib/a.rb", 2),
            ("lib/b.rb", 3),
            ("app/x.rb", 7),
        ]));

    let list = debt(&docs);

    let parts = |kind, label| {
        let p = priority_of(&list, kind, label);
        (p.commits, p.entry_points)
    };
    // refund.rb, and money.rb once though two steps cite it.
    assert_eq!(
        parts(DebtKind::LowConfidence, "billing/pay/refund"),
        (110, 1)
    );
    assert_eq!(
        parts(DebtKind::Undocumented, "billing/pay/refund"),
        (110, 1)
    );
    assert_eq!(parts(DebtKind::Undocumented, "billing"), (11, 3));
    assert_eq!(parts(DebtKind::Undocumented, "billing/pay"), (11, 0));
    assert_eq!(
        parts(DebtKind::Undocumented, "billing/pay/pay-1"),
        (1010, 0)
    );
    assert_eq!(parts(DebtKind::Undocumented, "?/gone"), (0, 0));
    assert_eq!(parts(DebtKind::NoDomain, "lib"), (5, 0));
    assert_eq!(parts(DebtKind::EntryPointWithoutFeature, "app"), (7, 2));
}

// spec: DBT-8, DBT-25 (internal: the read model of the debt page, shown by DBT-27)
#[test]
fn the_items_are_sorted_by_priority_then_by_kind() {
    let docs = docs(
        vec![feature("pay", "billing", Some(0.9))],
        vec![
            use_case("refund", "pay", Some(0.3)),
            use_case("void", "pay", Some(0.1)),
            use_case("check", "pay", None),
            use_case("hot", "pay", Some(0.4)),
        ],
    )
    .with_churn(churn(&[("hot.rb", 50), ("ignored.rb", 3)]));

    let list = debt(&docs);

    assert_eq!(
        kinds(&list),
        [
            (DebtKind::SetApart, "billing/pay/hot".to_string()),
            (DebtKind::LowConfidence, "billing/pay/hot".to_string()),
            (DebtKind::NotScored, "billing/pay/check".to_string()),
            (DebtKind::SetApart, "billing/pay/refund".to_string()),
            (DebtKind::SetApart, "billing/pay/void".to_string()),
            (DebtKind::LowConfidence, "billing/pay/void".to_string()),
            (DebtKind::LowConfidence, "billing/pay/refund".to_string()),
        ]
    );
}

// spec: DBT-25 (internal: the read model of the debt page, shown by DBT-28)
#[test]
fn with_no_history_every_item_counts_no_commit() {
    let docs = docs(vec![feature("pay", "billing", Some(0.1))], vec![]);

    let list = debt(&docs);

    assert_eq!(list.items[0].priority.commits, 0);
    assert!(docs.churn().is_none());
}

// spec: DBT-25 (internal: the read model of the debt page, shown by DBT-27)
#[test]
fn an_item_whose_feature_left_the_run_counts_no_commit() {
    let reconciliation = Reconciliation {
        items: vec![verdict(
            ItemKind::UseCase,
            "?/gone/lost",
            Status::Undocumented,
            0,
        )],
    };
    let docs = docs(vec![], vec![use_case("lost", "gone", Some(0.1))])
        .with_uncovered(Uncovered {
            reconciliation: Some(reconciliation),
            ..Uncovered::default()
        })
        .with_churn(churn(&[("lost.rb", 9)]));

    let list = debt(&docs);

    assert_eq!(kinds(&list).len(), 3, "{:?}", kinds(&list));
    for item in &list.items {
        assert_eq!(item.priority.commits, 0, "{:?} {}", item.kind, item.label());
    }
}
