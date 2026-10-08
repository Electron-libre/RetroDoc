use super::*;

use retrodoc_ingest::signals::SignalKind;

use crate::testing::FakeLlm;

fn signal(kind: SignalKind, origin: &str, text: &str) -> Signal {
    Signal {
        kind,
        origin: origin.to_string(),
        text: text.to_string(),
    }
}

fn signals() -> Vec<Signal> {
    vec![
        signal(
            SignalKind::DocSection,
            "docs/deep.md#Details",
            "Deep details.",
        ),
        signal(
            SignalKind::DocSection,
            "README.md#Shop",
            "A shop where buyers pay orders.",
        ),
        signal(
            SignalKind::Manifest,
            "package.json",
            "name: shop\ndependencies: stripe",
        ),
        signal(
            SignalKind::CommitSubject,
            "commit:aaaa1111",
            "feat: add refunds",
        ),
        signal(
            SignalKind::Schema,
            "db/schema.rb",
            "orders: total, buyer_id",
        ),
    ]
}

const ANSWER: &str = r#"{
  "purpose": {"text": "Lets buyers pay for orders.", "signals": ["S2"]},
  "users": [{"text": "Buyer who pays an order", "signals": ["[S2]", "S99", "S2"]}, "Support agent"],
  "objects": [{"text": "Order", "signals": ["S4"]}, {"text": "order", "signals": []}],
  "capabilities": [{"text": "Pay an order", "signals": ["S2", "S1"]}],
  "external_systems": [{"text": "Stripe", "signals": ["S1"]}],
  "open_questions": ["Who refunds?", "  "]
}"#;

fn sample_ids(signals: &[Signal]) -> Sample {
    Sample::build(signals, SAMPLE_BUDGET)
}

#[test]
fn the_sample_numbers_signals_and_puts_root_docs_first() {
    let sample = sample_ids(&signals());
    let readme = sample.text.find("README.md#Shop").unwrap();
    let deep = sample.text.find("docs/deep.md#Details").unwrap();
    assert!(readme < deep);
    assert!(sample.text.contains("== Project manifests =="));
    assert!(sample.text.contains("== Commit history =="));
    assert_eq!(sample.origins.len(), 5);
    assert_eq!(sample.origins["S1"], "package.json");
    assert!(sample.origins.values().any(|o| o == "commit:aaaa1111"));
    assert!(!sample.text_without_commits().contains("feat: add refunds"));
    assert!(sample.text_without_commits().contains("orders: total"));
}

#[test]
fn the_sample_stays_within_its_budget_and_spreads_over_the_signals() {
    let big = "line of text\n".repeat(400);
    let mut many: Vec<Signal> = (0..40)
        .map(|i| signal(SignalKind::I18n, &format!("loc/{i}.yml"), &big))
        .collect();
    many.extend((0..40).map(|i| signal(SignalKind::DocSection, &format!("d{i}.md"), &big)));
    let sample = Sample::build(&many, 4_000);
    assert!(
        sample.text.chars().count() <= 4_000 + 2_000,
        "{}",
        sample.text.len()
    );
    let i18n = sample
        .origins
        .values()
        .filter(|o| o.starts_with("loc/"))
        .count();
    let docs = sample
        .origins
        .values()
        .filter(|o| o.starts_with('d'))
        .count();
    assert!(i18n > 3 && docs > 3, "{i18n} {docs}");
    // The same input gives the same sample.
    assert_eq!(Sample::build(&many, 4_000).text, sample.text);
}

#[test]
fn new_commits_never_change_the_text_the_brief_is_reused_on() {
    let big = "word ".repeat(300);
    let many: Vec<Signal> = (0..60)
        .map(|i| signal(SignalKind::DocSection, &format!("d{i}.md"), &big))
        .collect();
    let few: Vec<Signal> = (0..3)
        .map(|i| signal(SignalKind::CommitSubject, &format!("commit:{i}"), "feat: x"))
        .collect();
    let lots: Vec<Signal> = (0..500)
        .map(|i| {
            signal(
                SignalKind::CommitSubject,
                &format!("commit:{i}"),
                "fix: some change",
            )
        })
        .collect();
    let with = |commits: &[Signal]| {
        let mut all = many.clone();
        all.extend_from_slice(commits);
        Sample::build(&all, SAMPLE_BUDGET).text_without_commits()
    };
    assert_eq!(with(&few), with(&lots));
}

#[test]
fn a_kind_that_needs_little_leaves_its_share_to_the_others() {
    let big = "word ".repeat(300);
    let many: Vec<Signal> = (0..30)
        .map(|i| signal(SignalKind::DocSection, &format!("d{i}.md"), &big))
        .collect();
    // Alone, the docs get far more than their 26 % share of the budget.
    let alone = Sample::build(&many, 10_000);
    assert!(alone.text.chars().count() > 5_000);
}

#[tokio::test]
async fn the_brief_cites_signals_by_origin_and_is_saved() {
    let dir = tempfile::tempdir().unwrap();
    let llm = FakeLlm::answering(ANSWER);

    let brief = build_brief(dir.path(), &signals(), &llm, false)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(llm.calls(), 1);
    assert!(llm.prompts()[0].contains("[S1] package.json"));
    assert_eq!(brief.purpose.text, "Lets buyers pay for orders.");
    assert_eq!(brief.purpose.sources, ["README.md#Shop"]);
    // [S2] and S2 are the same origin, S99 isn't in the sample.
    assert_eq!(brief.users[0].sources, ["README.md#Shop"]);
    // A bare string is an unsupported claim, kept.
    assert_eq!(brief.users[1].text, "Support agent");
    assert!(!brief.users[1].is_supported());
    // A repeated object is dropped.
    assert_eq!(brief.objects.len(), 1);
    assert_eq!(brief.objects[0].sources, ["db/schema.rb"]);
    assert_eq!(
        brief.capabilities[0].sources,
        ["README.md#Shop", "package.json"]
    );
    assert_eq!(brief.open_questions, ["Who refunds?"]);
    assert_eq!(ProductBrief::load(dir.path()).unwrap(), brief);
    assert!(!brief.edited());
}

#[tokio::test]
async fn an_unchanged_sample_reuses_the_brief_even_with_new_commits() {
    let dir = tempfile::tempdir().unwrap();
    let llm = FakeLlm::answering(ANSWER);
    build_brief(dir.path(), &signals(), &llm, false)
        .await
        .unwrap();

    let mut more = signals();
    more.push(signal(
        SignalKind::CommitSubject,
        "commit:bbbb2222",
        "fix: round totals",
    ));
    build_brief(dir.path(), &more, &llm, false).await.unwrap();
    assert_eq!(llm.calls(), 1);

    // New evidence of another kind writes it again.
    more.push(signal(
        SignalKind::DocSection,
        "CHANGELOG.md#1.0",
        "Subscriptions added.",
    ));
    build_brief(dir.path(), &more, &llm, false).await.unwrap();
    assert_eq!(llm.calls(), 2);
}

#[tokio::test]
async fn a_hand_edit_is_kept_until_force() {
    let dir = tempfile::tempdir().unwrap();
    let llm = FakeLlm::answering(ANSWER);
    let mut brief = build_brief(dir.path(), &signals(), &llm, false)
        .await
        .unwrap()
        .unwrap();
    brief.purpose.text = "Corrected by a person.".to_string();
    brief.save(dir.path()).unwrap();
    assert!(ProductBrief::load(dir.path()).unwrap().edited());

    let mut changed = signals();
    changed.push(signal(
        SignalKind::DocSection,
        "CHANGELOG.md#1.0",
        "Subscriptions added.",
    ));
    let kept = build_brief(dir.path(), &changed, &llm, false)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(kept.purpose.text, "Corrected by a person.");
    assert_eq!(llm.calls(), 1);

    let forced = build_brief(dir.path(), &changed, &llm, true)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(forced.purpose.text, "Lets buyers pay for orders.");
    assert_eq!(llm.calls(), 2);
}

#[tokio::test]
async fn no_signal_or_no_parseable_answer_gives_no_brief() {
    let dir = tempfile::tempdir().unwrap();
    let llm = FakeLlm::answering(ANSWER);
    assert!(build_brief(dir.path(), &[], &llm, false)
        .await
        .unwrap()
        .is_none());
    assert_eq!(llm.calls(), 0);

    let nonsense = FakeLlm::answering("no idea");
    assert!(build_brief(dir.path(), &signals(), &nonsense, false)
        .await
        .unwrap()
        .is_none());
    assert!(ProductBrief::load(dir.path()).is_none());
}

#[tokio::test]
async fn an_unreadable_brief_file_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let path = Artifact::Product.path(dir.path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "purpose: [oops").unwrap();
    let llm = FakeLlm::answering(ANSWER);

    assert!(build_brief(dir.path(), &signals(), &llm, false)
        .await
        .unwrap()
        .is_none());
    assert_eq!(llm.calls(), 0);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "purpose: [oops");
}

#[tokio::test]
async fn the_prompt_section_carries_the_words_not_the_citations() {
    let dir = tempfile::tempdir().unwrap();
    let llm = FakeLlm::answering(ANSWER);
    let mut brief = build_brief(dir.path(), &signals(), &llm, false)
        .await
        .unwrap()
        .unwrap();

    let section = brief.prompt_section();
    assert!(section.starts_with("Product brief of the application"));
    assert!(section.contains("Purpose: Lets buyers pay for orders."));
    assert!(section.contains("- Buyer who pays an order"));
    assert!(section.contains("External systems:\n- Stripe"));
    assert!(!section.contains("README.md"));
    assert!(!section.contains("Who refunds?"));

    // Citations don't move the fingerprint, an edit of the words does.
    let before = brief.fingerprint();
    brief.purpose.sources.clear();
    assert_eq!(brief.fingerprint(), before);
    brief.objects[0].text = "Purchase".to_string();
    assert_ne!(brief.fingerprint(), before);

    assert_eq!(ProductBrief::default().prompt_section(), "");
    assert_eq!(ProductBrief::default().fingerprint(), "");
}

#[test]
fn a_long_text_is_cut_at_a_word() {
    assert_eq!(cut_at_word("short", 10), "short");
    assert_eq!(cut_at_word("one two three four", 12), "one two…");
}
