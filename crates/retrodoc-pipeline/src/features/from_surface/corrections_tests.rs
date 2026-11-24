//! The human corrections given to the features pass (`specs/human-corrections.md`).

use std::path::PathBuf;

use super::tests::{answers_per_slice, big_domain, entry, fixture};
use super::*;
use crate::corrections::{Correction, Corrections, Verdict};
use crate::testing::FakeLlm;

async fn run(
    dir: &Path,
    (domains, entries): (&DomainMap, &EntryPoints),
    llm: &FakeLlm,
    corrections: &Corrections,
) -> Vec<Feature> {
    build_features_from_surface(
        dir,
        domains,
        entries,
        &Surface::default(),
        &ProductBrief::default(),
        &Evidence::default(),
        llm,
        Retention::OnlyRead,
        corrections,
    )
    .await
    .unwrap()
}

/// `place-order` for the domain `orders`, `list-routes` for `routing`.
fn answers_per_domain(prompt: &str) -> String {
    if prompt.contains("Entry points to place") {
        r#"{"placements":[]}"#
    } else if prompt.contains("GET /orders") {
        r#"{"features":[{"slug":"place-order","name":"Place order","description":"d",
        "entry_points":["E1","E2"]}]}"#
    } else {
        r#"{"features":[{"slug":"list-routes","name":"List routes","description":"d",
        "entry_points":["E1"]}]}"#
    }
    .to_string()
}

fn renamed(slug: &str, name: &str, entry_points: &[&str]) -> Correction {
    Correction {
        slug: slug.to_string(),
        name: Some(name.to_string()),
        entry_points: entry_points.iter().map(ToString::to_string).collect(),
        ..Correction::default()
    }
}

fn wrong(slug: &str, comment: &str, entry_points: &[&str]) -> Correction {
    Correction {
        slug: slug.to_string(),
        entry_points: entry_points.iter().map(ToString::to_string).collect(),
        verdict: Some(Verdict::Wrong {
            comment: comment.to_string(),
            reviewed_at: "2026-11-24T10:00:00Z".to_string(),
        }),
        ..Correction::default()
    }
}

// spec: HCR-H2, HCR-1, HCR-5, HCR-6 (internal: the application of the binary tests has one domain, whose entry points a test can't change)
#[tokio::test]
async fn a_domain_asked_again_is_given_the_edited_name_of_its_feature() {
    let dir = tempfile::tempdir().unwrap();
    let (domains, mut entries) = fixture();
    let llm = FakeLlm::replying(|_, request| Ok(answers_per_domain(&request.messages[1].content)));
    let none = Corrections::default();
    run(dir.path(), (&domains, &entries), &llm, &none).await;
    let calls = llm.calls();
    let mut corrections = Corrections::default();
    corrections.add_feature(
        "orders",
        renamed("place-order", "Order goods", &["orders.rb#GET /orders"]),
    );
    corrections.add_feature(
        "routing",
        renamed("list-routes", "Browse routes", &["orders.rb#GET /routes"]),
    );

    run(dir.path(), (&domains, &entries), &llm, &corrections).await;
    assert_eq!(llm.calls(), calls, "an edit asks nothing again by itself");

    entries
        .files
        .get_mut(&PathBuf::from("orders.rb"))
        .unwrap()
        .entry_points
        .push(entry("DELETE /orders", "order"));
    run(dir.path(), (&domains, &entries), &llm, &corrections).await;

    let prompts = llm.prompts();
    let asked: Vec<&String> = prompts[calls..]
        .iter()
        .filter(|p| !p.contains("Entry points to place"))
        .collect();
    assert_eq!(asked.len(), 1, "only `orders` gains an entry point");
    assert!(asked[0].contains("`place-order`"), "{}", asked[0]);
    assert!(asked[0].contains("\"Order goods\""), "{}", asked[0]);
    assert!(!asked[0].contains("Browse routes"), "{}", asked[0]);
}

// spec: HCR-4, HCR-6 (internal: a domain over the prompt limit takes 100 entry points, too many for the fake application of the binary tests)
#[tokio::test]
async fn a_feature_marked_wrong_asks_again_the_slice_that_holds_it_once() {
    let dir = tempfile::tempdir().unwrap();
    let (domains, entries) = big_domain();
    let llm = FakeLlm::replying(|_, request| Ok(answers_per_slice(&request.messages[1].content)));
    run(
        dir.path(),
        (&domains, &entries),
        &llm,
        &Corrections::default(),
    )
    .await;
    let calls = llm.calls();
    let mut corrections = Corrections::default();
    // `later` holds an entry point of the second slice only.
    corrections.add_feature(
        "big",
        wrong("later", "It is done at once.", &["b.rb#GET /b1"]),
    );

    run(dir.path(), (&domains, &entries), &llm, &corrections).await;

    let prompts = llm.prompts();
    let asked = &prompts[calls..];
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert!(asked[0].contains("GET /b59") && !asked[0].contains("GET /a0\""));
    assert!(asked[0].contains("It is done at once."), "{}", asked[0]);

    let calls = llm.calls();
    run(dir.path(), (&domains, &entries), &llm, &corrections).await;
    assert_eq!(llm.calls(), calls, "the review is answered");
}
