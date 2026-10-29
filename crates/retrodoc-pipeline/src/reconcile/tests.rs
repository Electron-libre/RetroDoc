use std::path::PathBuf;

use retrodoc_core::model::{ConfidenceScore, RuleKind};

use super::*;
use crate::domains::DomainCluster;

fn doc(path: &str, content: &str) -> ExistingDoc {
    ExistingDoc {
        path: PathBuf::from(path),
        content: content.to_string(),
    }
}

fn domain(slug: &str, name: &str) -> DomainCluster {
    DomainCluster {
        slug: slug.to_string(),
        name: name.to_string(),
        ..DomainCluster::default()
    }
}

fn feature(slug: &str, domain: &str, name: &str, entry_points: usize) -> Feature {
    Feature {
        back_office: false,
        slug: slug.to_string(),
        domain_slug: domain.to_string(),
        name: name.to_string(),
        description: String::new(),
        source_paths: vec![],
        entry_points: (0..entry_points).map(|i| format!("a.rb#e{i}")).collect(),
        confidence: None,
    }
}

fn use_case(slug: &str, feature: &str, name: &str) -> UseCase {
    UseCase {
        entry_points: Vec::new(),
        primary_actor: None,
        narrative: None,
        business_language: None,
        slug: slug.to_string(),
        feature_slug: feature.to_string(),
        name: name.to_string(),
        description: String::new(),
        steps: vec![],
        diagram_mermaid: None,
        confidence: None,
    }
}

fn rule(slug: &str, statement: &str, evidence: Vec<RuleEvidence>) -> BusinessRule {
    BusinessRule {
        slug: slug.to_string(),
        statement: statement.to_string(),
        kind: RuleKind::Limit,
        entities: Vec::new(),
        use_cases: Vec::new(),
        evidence,
        confidence: None,
    }
}

fn verdict(label: &str, entry_points: usize, confidence: Option<f32>) -> ItemVerdict {
    ItemVerdict {
        kind: ItemKind::Feature,
        label: label.to_string(),
        domain: "d".to_string(),
        name: label.to_string(),
        entry_points,
        confidence,
        status: Status::Undocumented,
        sections: Vec::new(),
        contradictions: Vec::new(),
    }
}

// spec: REC-2
#[test]
fn an_item_keeps_at_most_three_sections_sharing_half_of_its_words_best_first() {
    let docs = [doc(
        "docs/guide.md",
        "# Invoices\nPay an invoice by card.\n\n# Paying\nPay the invoice.\n\n# Cards\nInvoice card.\n\n\
         # Again\nPay an invoice, pay it, the invoice.\n\n# Riders\nRiders deliver orders.\n",
    )];
    let corpus = corpus(&docs);
    let found = candidates(
        vec![
            Item {
                query: "Pay invoice".into(),
                ..items(
                    &DomainMap::default(),
                    &[feature("pay", "billing", "x", 0)],
                    &[],
                    &[],
                )
                .remove(0)
            },
            Item {
                query: "It is what it is".into(),
                ..items(
                    &DomainMap::default(),
                    &[feature("it", "billing", "x", 0)],
                    &[],
                    &[],
                )
                .remove(0)
            },
        ],
        &corpus,
    );

    assert_eq!(found.len(), 1, "a query of stop words only is left out");
    let origins: Vec<String> = found[0].sections.iter().map(|s| s.at.origin()).collect();
    assert_eq!(origins.len(), MAX_SECTIONS_PER_ITEM);
    assert_eq!(origins[0], "docs/guide.md#Again");
    assert!(!origins.contains(&"docs/guide.md#Riders".to_string()));
}

// spec: REC-2
#[test]
fn an_item_its_description_matches_nowhere_is_searched_with_its_name_alone() {
    let docs = [doc(
        "docs/api.md",
        "# Tags\nList, create and delete tags.\n\n# Bookmarks\nSave a bookmark with a title and notes.\n",
    )];
    let elsewhere = "Users rename, merge and categorize labels used across the library.";
    let map = DomainMap {
        domains: vec![DomainCluster {
            description: elsewhere.into(),
            ..domain("tagging", "Tags")
        }],
    };
    let mut tags = feature("tags", "tagging", "Manage tags", 0);
    tags.description = elsewhere.into();
    let mut delete = use_case("delete", "tags", "Delete tags");
    delete.description = elsewhere.into();
    let mut save = feature("save", "tagging", "Tags of a bookmark", 0);
    save.description = "Save a bookmark with a title and notes.".into();

    let found = candidates(items(&map, &[tags, save], &[delete], &[]), &corpus(&docs));

    let origins = |label: &str| -> Vec<String> {
        let c = found.iter().find(|c| c.item.label == label).unwrap();
        c.sections.iter().map(|s| s.at.origin()).collect()
    };
    for label in ["tagging", "tagging/tags", "tagging/tags/delete"] {
        assert_eq!(
            origins(label),
            ["docs/api.md#Tags"],
            "{label}: the name alone finds it"
        );
    }
    assert_eq!(
        origins("tagging/save"),
        ["docs/api.md#Bookmarks"],
        "the name alone is not searched when the name and description match a section"
    );
}

// spec: REC-2
#[test]
fn every_kind_of_item_is_searched_with_its_name_and_description() {
    let map = DomainMap {
        domains: vec![
            DomainCluster {
                description: "Money owed".into(),
                ..domain("billing", "Billing")
            },
            domain(UNCATEGORIZED_SLUG, "Uncategorized"),
        ],
    };
    let mut pay = feature("pay", "billing", "Payment", 2);
    pay.description = "Settle a bill".into();
    let mut by_card = use_case("by-card", "pay", "Pay by card");
    by_card.description = "With a card".into();
    let found = items(
        &map,
        &[pay, feature("refund", "billing", "Refunds", 1)],
        &[by_card],
        &[rule("pay-1", "A bill is paid once.", vec![])],
    );

    let labels: Vec<(&str, ItemKind, &str, usize)> = found
        .iter()
        .map(|i| (i.label.as_str(), i.kind, i.query.as_str(), i.entry_points))
        .collect();
    assert_eq!(
        labels,
        [
            ("billing", ItemKind::Domain, "Billing\nMoney owed", 3),
            (
                "billing/pay",
                ItemKind::Feature,
                "Payment\nSettle a bill",
                2
            ),
            ("billing/refund", ItemKind::Feature, "Refunds\n", 1),
            (
                "billing/pay/by-card",
                ItemKind::UseCase,
                "Pay by card\nWith a card",
                0
            ),
            (
                "billing/pay/pay-1",
                ItemKind::Rule,
                "A bill is paid once.",
                2
            ),
        ]
    );
}

// spec: REC-3
#[test]
fn no_matching_section_is_undocumented_and_a_cited_doc_section_documents_a_rule() {
    let docs = [doc("README.md", "# Limits\nA cart holds ten items.\n")];
    let rules = [
        rule("cart-1", "Riders above the weekly cap are paused.", vec![]),
        rule(
            "cart-2",
            "Cooking time counts before the rider leaves.",
            vec![RuleEvidence::Doc {
                path: "README.md".into(),
                heading: "Limits".into(),
            }],
        ),
        rule("cart-3", "A cart holds ten items at most.", vec![]),
    ];

    let result = reconcile(
        &DomainMap::default(),
        &[],
        &[],
        &rules,
        &docs,
        &Judgements::default(),
    );
    let status: Vec<(&str, Status, Vec<String>)> = result
        .items
        .iter()
        .map(|i| {
            (
                i.label.as_str(),
                i.status,
                i.sections.iter().map(SectionRef::origin).collect(),
            )
        })
        .collect();
    assert_eq!(
        status,
        [
            ("?/?/cart-1", Status::Undocumented, vec![]),
            (
                "?/?/cart-2",
                Status::Documented,
                vec!["README.md#Limits".to_string()]
            ),
            (
                "?/?/cart-3",
                Status::Matched,
                vec!["README.md#Limits".to_string()]
            ),
        ]
    );
}

// spec: REC-10
#[test]
fn undocumented_items_rank_by_entry_points_then_confidence_then_label() {
    let mut matched = verdict("m", 9, None);
    matched.status = Status::Matched;
    let reconciliation = Reconciliation {
        items: vec![
            verdict("b-unscored", 1, None),
            verdict("c-low", 1, Some(0.2)),
            verdict("a-high", 1, Some(0.9)),
            verdict("z-many", 4, None),
            verdict("a-unscored", 1, None),
            matched,
        ],
    };

    let order: Vec<&str> = reconciliation
        .undocumented()
        .iter()
        .map(|i| i.label.as_str())
        .collect();
    assert_eq!(
        order,
        ["z-many", "a-high", "c-low", "a-unscored", "b-unscored"]
    );
}

// spec: REC-9
#[test]
fn each_domain_counts_its_items_per_status() {
    let mut documented = verdict("x", 0, None);
    documented.status = Status::Documented;
    let mut other = verdict("y", 0, None);
    other.domain = "other".into();
    let reconciliation = Reconciliation {
        items: vec![documented, verdict("z", 0, None), other],
    };

    let counts = reconciliation.by_domain();
    assert_eq!(
        counts["d"],
        StatusCount {
            documented: 1,
            undocumented: 1,
            ..StatusCount::default()
        }
    );
    assert_eq!(counts["other"].total(), 1);
}

#[test]
fn a_domain_is_as_trusted_as_its_features() {
    let mut a = feature("a", "billing", "A", 0);
    a.confidence = Some(ConfidenceScore::new(0.4, None));
    let mut b = feature("b", "billing", "B", 0);
    b.confidence = Some(ConfidenceScore::new(0.8, None));
    let map = DomainMap {
        domains: vec![domain("billing", "Billing")],
    };
    let found = items(&map, &[a, b], &[], &[]);
    assert!((found[0].confidence.unwrap() - 0.6).abs() < 1e-6);
}

fn section(origin: &str, text: &str) -> Section {
    Section {
        at: SectionRef::parse(origin),
        text: text.to_string(),
    }
}

fn candidate(label: &str, query: &str, sections: &[(&str, &str)]) -> Candidate {
    Candidate {
        item: Item {
            kind: ItemKind::Feature,
            label: label.to_string(),
            domain: "billing".to_string(),
            name: label.to_string(),
            query: query.to_string(),
            entry_points: 0,
            confidence: None,
            cited_sections: Vec::new(),
        },
        sections: sections.iter().map(|(o, t)| section(o, t)).collect(),
    }
}

fn three_candidates() -> Vec<Candidate> {
    vec![
        candidate(
            "billing/pay",
            "Pay an invoice by card",
            &[("docs/guide.md#Paying", "Pay by card.")],
        ),
        candidate(
            "billing/refund",
            "Refund at once",
            &[
                ("docs/guide.md#Paying", "Pay by card."),
                ("docs/guide.md#Refunds", "Refunds take a week."),
            ],
        ),
        candidate(
            "billing/riders",
            "Riders",
            &[("docs/guide.md#Team", "Our riders.")],
        ),
    ]
}

const THREE_ANSWERS: &str = r#"{"items":[
    {"n":1,"sections":[{"s":"S1","answer":"covers"}]},
    {"n":2,"sections":[{"s":"S1","answer":"Unrelated"},{"s":"S2","answer":"contradicts",
        "doc_states":"Refunds take a week.","item_states":"Refunds are made at once."}]},
    {"n":"3","sections":[{"s":3,"answer":"unrelated"}]}]}"#;

fn on() -> PipelineConfig {
    PipelineConfig::default()
}

use retrodoc_core::config::PipelineConfig;

use crate::testing::FakeLlm;

// spec: REC-4
#[tokio::test]
async fn the_llm_tells_covering_contradicting_and_unrelated_sections_apart() {
    let dir = tempfile::tempdir().unwrap();
    let llm = FakeLlm::answering(THREE_ANSWERS);

    judge(dir.path(), &llm, &on(), &three_candidates())
        .await
        .unwrap();

    assert_eq!(llm.calls(), 1, "the three items go in one request");
    let prompt = &llm.prompts()[0];
    assert_eq!(
        prompt.matches("[S1] docs/guide.md#Paying").count(),
        1,
        "a section is shown once: {prompt}"
    );
    assert!(
        prompt.contains("2. feature: Refund at once (sections: S1, S2)"),
        "{prompt}"
    );
    let result = Reconciliation::new(three_candidates(), &Judgements::load(dir.path()));
    let status: Vec<(&str, Status, Vec<String>)> = result
        .items
        .iter()
        .map(|i| {
            (
                i.label.as_str(),
                i.status,
                i.sections.iter().map(SectionRef::origin).collect(),
            )
        })
        .collect();
    assert_eq!(
        status,
        [
            (
                "billing/pay",
                Status::Documented,
                vec!["docs/guide.md#Paying".to_string()]
            ),
            (
                "billing/refund",
                Status::Contradicted,
                vec!["docs/guide.md#Refunds".to_string()]
            ),
            ("billing/riders", Status::Undocumented, vec![]),
        ]
    );
    assert_eq!(
        result.items[1].contradictions,
        [Contradiction {
            section: SectionRef::parse("docs/guide.md#Refunds"),
            doc_states: "Refunds take a week.".into(),
            item_states: "Refunds are made at once.".into(),
        }]
    );
}

// spec: REC-4
#[tokio::test]
async fn items_go_to_the_llm_a_batch_at_a_time() {
    let dir = tempfile::tempdir().unwrap();
    let candidates: Vec<Candidate> = (0..=MAX_ITEMS_PER_REQUEST)
        .map(|i| candidate(&format!("f{i}"), "Pay", &[("a.md#A", "Pay.")]))
        .collect();
    let llm = FakeLlm::answering(r#"{"items":[]}"#);

    judge(dir.path(), &llm, &on(), &candidates).await.unwrap();

    assert_eq!(llm.calls(), 2);
}

// spec: REC-5
#[tokio::test]
async fn a_contradiction_without_both_sides_or_an_unreadable_answer_decides_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let llm = FakeLlm::answering(
        r#"{"items":[{"n":1,"sections":[{"s":"S1","answer":"contradicts","doc_states":"x"}]},
        {"n":2,"sections":[{"s":"S1","answer":"covers"}]}]}"#,
    );

    judge(dir.path(), &llm, &on(), &three_candidates())
        .await
        .unwrap();

    let saved = Judgements::load(dir.path());
    assert!(
        saved.items.is_empty(),
        "every item has a section unanswered: {saved:?}"
    );
    let result = Reconciliation::new(three_candidates(), &saved);
    assert!(result.items.iter().all(|i| i.status == Status::Matched));

    let unreadable = FakeLlm::answering("I think they match.");
    judge(dir.path(), &unreadable, &on(), &three_candidates())
        .await
        .unwrap();
    assert_eq!(
        unreadable.calls(),
        2,
        "asked once more, then left to the next run"
    );
    assert_eq!(Judgements::load(dir.path()).items, []);
}

// spec: REC-6
#[tokio::test]
async fn an_unchanged_item_is_not_asked_again_a_changed_one_or_another_model_is() {
    let dir = tempfile::tempdir().unwrap();
    judge(
        dir.path(),
        &FakeLlm::answering(THREE_ANSWERS),
        &on(),
        &three_candidates(),
    )
    .await
    .unwrap();

    let again = FakeLlm::answering(THREE_ANSWERS);
    judge(dir.path(), &again, &on(), &three_candidates())
        .await
        .unwrap();
    assert_eq!(again.calls(), 0);

    let mut changed = three_candidates();
    changed[2].sections[0].text = "Our riders, now in two cities.".into();
    let one =
        FakeLlm::answering(r#"{"items":[{"n":1,"sections":[{"s":"S1","answer":"covers"}]}]}"#);
    judge(dir.path(), &one, &on(), &changed).await.unwrap();
    assert_eq!(one.calls(), 1);
    assert!(
        one.prompts()[0].contains("1. feature: Riders"),
        "{}",
        one.prompts()[0]
    );
    assert!(!one.prompts()[0].contains("Refund"), "{}", one.prompts()[0]);
    assert_eq!(
        Reconciliation::new(changed.clone(), &Judgements::load(dir.path())).items[2].status,
        Status::Documented
    );

    let other = FakeLlm::answering(THREE_ANSWERS).with_model("other-model");
    judge(dir.path(), &other, &on(), &changed).await.unwrap();
    assert_eq!(other.calls(), 1);
}

// spec: REC-7
#[tokio::test]
async fn the_verdicts_are_saved_and_read_while_the_texts_are_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    judge(
        dir.path(),
        &FakeLlm::answering(THREE_ANSWERS),
        &on(),
        &three_candidates(),
    )
    .await
    .unwrap();
    assert!(dir
        .path()
        .join(".retrodoc/cache/reconciliation.yaml")
        .exists());

    let mut edited = three_candidates();
    edited[0].item.query = "Pay an invoice in cash".into();
    let result = Reconciliation::new(edited, &Judgements::load(dir.path()));
    assert_eq!(result.items[0].status, Status::Matched);
    assert_eq!(result.items[1].status, Status::Contradicted);

    let none = Reconciliation::new(three_candidates(), &Judgements::default());
    assert!(none.items.iter().all(|i| i.status == Status::Matched));
}

// spec: REC-8
#[tokio::test]
async fn nothing_matched_or_the_pass_off_makes_no_call() {
    let dir = tempfile::tempdir().unwrap();
    let llm = FakeLlm::answering(THREE_ANSWERS);

    judge(dir.path(), &llm, &on(), &[candidate("a", "Pay", &[])])
        .await
        .unwrap();
    judge(dir.path(), &llm, &on(), &[]).await.unwrap();
    let off = PipelineConfig {
        reconcile: false,
        ..PipelineConfig::default()
    };
    judge(dir.path(), &llm, &off, &three_candidates())
        .await
        .unwrap();

    assert_eq!(llm.calls(), 0);
    assert!(!dir
        .path()
        .join(".retrodoc/cache/reconciliation.yaml")
        .exists());
}

// spec: REC-2
#[test]
fn two_sections_of_one_file_under_the_same_heading_are_one_section() {
    let docs = [doc(
        "docs/guide.md",
        "# Example\nPay by card.\n\n# Other\nNothing.\n\n# Example\nRefund by wire.\n",
    )];
    let sections = corpus(&docs);
    let examples: Vec<&Section> = sections
        .iter()
        .filter(|s| s.at.origin() == "docs/guide.md#Example")
        .collect();
    assert_eq!(examples.len(), 1);
    assert!(
        examples[0].text.contains("Pay by card."),
        "{}",
        examples[0].text
    );
    assert!(
        examples[0].text.contains("Refund by wire."),
        "{}",
        examples[0].text
    );
}

// spec: REC-3
#[test]
fn a_rule_citing_a_section_that_is_gone_is_matched_like_the_others() {
    let docs = [doc("README.md", "# Limits\nA cart holds ten items.\n")];
    let rules = [
        rule(
            "cart-1",
            "A cart holds ten items at most.",
            vec![RuleEvidence::Doc {
                path: "docs/old.md".into(),
                heading: "Caps".into(),
            }],
        ),
        rule(
            "cart-2",
            "Riders above the weekly cap are paused.",
            vec![RuleEvidence::Doc {
                path: "docs/old.md".into(),
                heading: "Caps".into(),
            }],
        ),
    ];

    let result = reconcile(
        &DomainMap::default(),
        &[],
        &[],
        &rules,
        &docs,
        &Judgements::default(),
    );

    let status: Vec<Status> = result.items.iter().map(|i| i.status).collect();
    assert_eq!(status, [Status::Matched, Status::Undocumented]);
}

// spec: REC-14
#[tokio::test]
async fn a_failing_llm_keeps_the_verdicts_got_and_fails_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut candidates = three_candidates();
    candidates.extend(
        (0..MAX_ITEMS_PER_REQUEST)
            .map(|i| candidate(&format!("more-{i}"), "Pay", &[("a.md#A", "Pay.")])),
    );
    let llm = FakeLlm::failing_after(1, |_| THREE_ANSWERS.to_string());

    judge(dir.path(), &llm, &on(), &candidates).await.unwrap();

    assert_eq!(Judgements::load(dir.path()).items.len(), 3);
}

/// `n` paragraphs of about 1,000 chars that speak of listing only.
fn filler(n: usize) -> String {
    (0..n)
        .map(|i| {
            format!(
                "Listing page {i}. {}",
                "Lists the entries page by page. ".repeat(30)
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// The text shown of the sections in the prompt of `llm`'s first call.
fn sections_shown(llm: &FakeLlm) -> String {
    let prompt = &llm.prompts()[0];
    let end = prompt
        .find("\nItems of the generated documentation:")
        .expect("items listed");
    prompt[..end].to_string()
}

// spec: REC-15
#[tokio::test]
async fn a_section_within_its_share_is_shown_whole_even_past_the_old_cut() {
    let dir = tempfile::tempdir().unwrap();
    let text = format!("### Paying\n{}\n\nRemove a card with DELETE.", filler(3));
    let llm = FakeLlm::answering(r#"{"items":[]}"#);

    judge(
        dir.path(),
        &llm,
        &on(),
        &[candidate(
            "billing/remove-card",
            "Remove a card",
            &[("docs/api.md#Paying", &text)],
        )],
    )
    .await
    .unwrap();

    let shown = sections_shown(&llm);
    assert!(shown.contains("Remove a card with DELETE."), "{shown}");
    assert!(!shown.contains("[...]"), "{shown}");
}

// spec: REC-15
#[tokio::test]
async fn a_long_section_shows_the_paragraphs_of_each_of_its_items_in_turn() {
    let dir = tempfile::tempdir().unwrap();
    let text = format!(
        "### Bookmarks\n**List**\n\n{}\n\n**Archive**\n\nArchives a bookmark.\n\n{}\n\n**Delete**\n\nDeletes a bookmark.",
        filler(20),
        filler(20)
    );
    let llm = FakeLlm::answering(r#"{"items":[]}"#);

    judge(
        dir.path(),
        &llm,
        &on(),
        &[
            candidate(
                "b/remove",
                "Delete a bookmark",
                &[("docs/api.md#Bookmarks", &text)],
            ),
            candidate(
                "b/archive",
                "Archive a bookmark",
                &[("docs/api.md#Bookmarks", &text)],
            ),
        ],
    )
    .await
    .unwrap();

    let shown = sections_shown(&llm);
    assert!(shown.contains("### Bookmarks"), "the heading line is kept");
    let archive = shown
        .find("Archives a bookmark.")
        .expect("the archive paragraph is shown");
    let delete = shown
        .find("Deletes a bookmark.")
        .expect("the delete paragraph is shown");
    assert!(archive < delete, "shown in the order of the section");
    assert!(
        shown[archive..delete].contains("[...]"),
        "the paragraphs left out between them are marked"
    );
    let n = section_shown(&llm, 1).chars().count();
    assert!(n <= 36_000, "{n} chars");
}

// spec: REC-15
#[tokio::test]
async fn the_sections_of_a_request_share_its_budget_and_a_long_paragraph_is_cut() {
    let dir = tempfile::tempdir().unwrap();
    let huge = format!("### Refunds\nRefund {}", "at once, always. ".repeat(4_000));
    let other = format!("### Paying\nPay by card.\n\n{}", filler(30));
    let llm = FakeLlm::answering(r#"{"items":[]}"#);

    judge(
        dir.path(),
        &llm,
        &on(),
        &[
            candidate("billing/refund", "Refund", &[("docs/a.md#Refunds", &huge)]),
            candidate("billing/pay", "Pay", &[("docs/a.md#Paying", &other)]),
        ],
    )
    .await
    .unwrap();

    let shown = sections_shown(&llm);
    let refunds = &shown[..shown.find("[S2]").expect("two sections")];
    let paying = &shown[shown.find("[S2]").unwrap()..];
    assert!(refunds.contains("Refund at once"), "{refunds}");
    let n = refunds.chars().count();
    assert!(
        (17_000..18_200).contains(&n),
        "{n} chars: half of the budget"
    );
    assert!(
        paying.contains("Pay by card.\n\n[...]") && !paying.contains("Listing page"),
        "only the paragraphs holding a query word: {paying}"
    );
}

// spec: REC-15
#[tokio::test]
async fn a_long_section_none_of_whose_paragraphs_holds_a_query_word_is_shown_from_its_start() {
    let dir = tempfile::tempdir().unwrap();
    let text = format!("### Team\nOur riders.\n\n{}", filler(50));
    let llm = FakeLlm::answering(r#"{"items":[]}"#);

    judge(
        dir.path(),
        &llm,
        &on(),
        &[candidate(
            "billing/zzz",
            "Zebra",
            &[("docs/a.md#Team", &text)],
        )],
    )
    .await
    .unwrap();

    let shown = sections_shown(&llm);
    assert!(
        shown.contains("### Team\nOur riders.\n\nListing page 0."),
        "{}",
        &shown[..300]
    );
    let n = section_shown(&llm, 1).chars().count();
    assert!((35_900..=36_000).contains(&n), "{n} chars");
}

// spec: REC-6
#[tokio::test]
async fn a_verdict_given_before_the_rule_of_what_is_shown_changed_is_asked_again() {
    let dir = tempfile::tempdir().unwrap();
    let candidates = three_candidates();
    let before_the_rule = |c: &Candidate| {
        let mut parts = vec![c.item.kind.label().to_string(), c.item.query.clone()];
        for s in &c.sections {
            parts.push(s.at.origin());
            parts.push(s.text.clone());
        }
        crate::fingerprints::fingerprint(parts)
    };
    let model = crate::fingerprints::model_part(&FakeLlm::answering(""));
    Judgements {
        items: candidates
            .iter()
            .map(|c| Judged {
                kind: c.item.kind,
                label: c.item.label.clone(),
                fingerprint: before_the_rule(c),
                model: model.clone(),
                sections: Vec::new(),
            })
            .collect(),
    }
    .save(dir.path())
    .unwrap();
    let llm = FakeLlm::answering(THREE_ANSWERS);

    judge(dir.path(), &llm, &on(), &candidates).await.unwrap();

    assert_eq!(llm.calls(), 1);
}

// spec: REC-15
#[tokio::test]
async fn a_paragraph_with_a_word_rare_in_the_section_beats_ones_with_its_common_words() {
    let dir = tempfile::tempdir().unwrap();
    let common = (0..30)
        .map(|i| {
            format!(
                "Bookmark list {i}: archived entries. {}",
                "Shown page by page. ".repeat(97)
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let text = format!(
        "### Bookmarks\n{common}\n\nUnarchives a bookmark. {}",
        "It goes back to the main view. ".repeat(15)
    );
    let llm = FakeLlm::answering(r#"{"items":[]}"#);

    judge(
        dir.path(),
        &llm,
        &on(),
        &[candidate(
            "b/unarchive",
            "Unarchive an archived bookmark to the list",
            &[("docs/api.md#Bookmarks", &text)],
        )],
    )
    .await
    .unwrap();

    let shown = sections_shown(&llm);
    assert!(
        shown.contains("Unarchives a bookmark."),
        "{}",
        &shown[shown.len() - 600..]
    );
}

/// The text shown of section `S<n>` in the prompt of `llm`'s first call, without its `[S<n>]` line.
fn section_shown(llm: &FakeLlm, n: usize) -> String {
    let all = sections_shown(llm);
    let start = all.find(&format!("[S{n}] ")).expect("section listed");
    let body = &all[start..];
    let body = &body[body.find('\n').unwrap() + 1..];
    let end = body.find("\n[S").unwrap_or(body.len());
    body[..end].trim_end().to_string()
}

async fn shown_for(candidates: &[Candidate]) -> FakeLlm {
    let dir = tempfile::tempdir().unwrap();
    let llm = FakeLlm::answering(r#"{"items":[]}"#);
    judge(dir.path(), &llm, &on(), candidates).await.unwrap();
    llm
}

// spec: REC-15
#[tokio::test]
async fn a_long_paragraph_first_picked_leaves_room_for_the_other_items() {
    let text = format!(
        "### Bookmarks\nThe example: {}\n\nDeletes a bookmark.",
        "tag_names favicon_url bundle shown. ".repeat(2_000)
    );
    let llm = shown_for(&[
        candidate(
            "b/api",
            "Tag names favicon url bundle",
            &[("api.md#Bookmarks", &text)],
        ),
        candidate(
            "b/remove",
            "Delete a bookmark",
            &[("api.md#Bookmarks", &text)],
        ),
    ])
    .await;

    let shown = section_shown(&llm, 1);
    assert!(
        shown.contains("Deletes a bookmark."),
        "{}",
        &shown[shown.len() - 200..]
    );
    assert!(
        shown.chars().count() <= 36_000,
        "{} chars",
        shown.chars().count()
    );
}

// spec: REC-15
#[tokio::test]
async fn the_text_before_the_first_heading_has_no_heading_line_to_keep() {
    let text = format!(
        "{}\n\nZebra stripes are shown.",
        "An opening line. ".repeat(3_000)
    );
    let llm = shown_for(&[candidate(
        "b/zebra",
        "Zebra stripes",
        &[("README.md", &text)],
    )])
    .await;

    let shown = section_shown(&llm, 1);
    assert!(
        shown.contains("Zebra stripes are shown."),
        "{}",
        &shown[..200]
    );
    assert!(
        shown.chars().count() <= 36_000,
        "{} chars",
        shown.chars().count()
    );
}

// spec: REC-15
#[tokio::test]
async fn a_fenced_block_with_blank_lines_is_one_paragraph_and_a_cut_one_is_closed() {
    let fence = "```json\n{\n\n  \"tag_names\": [\"a\"]\n}\n```";
    let long_fence = format!("```\nbundle {}\n```", "x = 1;\n".repeat(8_000));
    let text = format!(
        "### Create\n{}\n\n{fence}\n\n{}\n\n{long_fence}",
        filler(20),
        filler(20)
    );
    let llm = shown_for(&[
        candidate("b/tags", "Tag names", &[("api.md#Create", &text)]),
        candidate("b/bundle", "Bundle", &[("api.md#Create", &text)]),
    ])
    .await;

    let shown = section_shown(&llm, 1);
    assert!(shown.contains(fence), "{shown}");
    assert!(
        shown.contains("bundle x = 1;"),
        "the long fence is shown, cut"
    );
    assert_eq!(shown.matches("```").count() % 2, 0, "every fence is closed");
}

// spec: REC-15
#[tokio::test]
async fn a_blank_line_holding_spaces_splits_paragraphs() {
    let text = format!(
        "### Bookmarks\n{}\n  \nDeletes a bookmark.",
        filler(40).replace("\n\n", "\n \t\n")
    );
    let llm = shown_for(&[candidate(
        "b/remove",
        "Delete a bookmark",
        &[("api.md#Bookmarks", &text)],
    )])
    .await;

    let shown = section_shown(&llm, 1);
    assert!(shown.contains("Deletes a bookmark."), "{}", &shown[..200]);
    assert!(shown.contains("[...]"), "{}", &shown[..200]);
}

// spec: REC-15
#[tokio::test]
async fn a_short_section_gives_its_unused_share_to_the_long_ones() {
    let long = format!("### Listing\n{}", filler(60));
    let mut candidates = vec![candidate(
        "b/list",
        "List the entries page by page",
        &[("a.md#Listing", &long), ("a.md#S0", "Short 0.")],
    )];
    candidates.extend((1..8).map(|i| {
        candidate(
            &format!("b/{i}"),
            "Short",
            &[(&format!("a.md#S{i}"), &format!("Short {i}."))],
        )
    }));
    let llm = shown_for(&candidates).await;

    let shown = section_shown(&llm, 1);
    let n = shown.chars().count();
    assert!(
        (35_000..=36_000).contains(&n),
        "{n} chars: the share the short sections left"
    );
}
