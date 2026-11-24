//! The saved slugs of the domains written again (`specs/rerun.md` RER-13, RER-16).

use super::tests::shop_surface;
use super::*;
use crate::brief::ProductBrief;
use crate::corrections::{Correction, Corrections, Verdict};
use crate::testing::FakeLlm;

/// A review `wrong` of the domain `ordering`: it asks the domains again.
fn ordering_wrong() -> Corrections {
    let mut corrections = Corrections::default();
    corrections.add_domain(Correction {
        slug: "ordering".to_string(),
        verdict: Some(Verdict::Wrong {
            comment: "Name it better.".to_string(),
            reviewed_at: "2026-11-24T10:00:00Z".to_string(),
        }),
        ..Correction::default()
    });
    corrections
}

/// The slugs of the domains of a first answer, then of a second one asked by a review `wrong`.
async fn slugs_after(first: &str, second: &str) -> Vec<String> {
    let dir = tempfile::tempdir().unwrap();
    let llm = FakeLlm::sequence(&[first, second]);
    let surface = shop_surface();
    let brief = ProductBrief::default();
    build_domains_from_surface(dir.path(), &surface, &brief, &llm, &Corrections::default())
        .await
        .unwrap();
    let map = build_domains_from_surface(dir.path(), &surface, &brief, &llm, &ordering_wrong())
        .await
        .unwrap();
    assert_eq!(llm.calls(), 2);
    map.domains.into_iter().map(|d| d.slug).collect()
}

fn answer(domains: &[(&str, &str)]) -> String {
    let domains: Vec<String> = domains
        .iter()
        .map(|(slug, resource)| {
            format!(
                r#"{{"slug":"{slug}","name":"N","description":"d","resources":["{resource}"]}}"#
            )
        })
        .collect();
    format!(r#"{{"domains":[{}]}}"#, domains.join(","))
}

// spec: RER-13 (internal: the application of the binary tests has one domain, whose correction names it)
#[tokio::test]
async fn a_domain_written_again_under_a_new_slug_keeps_its_saved_one() {
    let slugs = slugs_after(
        &answer(&[
            ("ordering", "order"),
            ("billing", "invoice"),
            ("hooks", "webhook"),
        ]),
        &answer(&[
            ("order-taking", "order"),
            ("invoicing", "Invoice"),
            ("hooks", "webhook"),
        ]),
    )
    .await;

    assert_eq!(slugs, ["ordering", "billing", "hooks", UNCATEGORIZED_SLUG]);
}

// spec: RER-16 (internal: a surface the fake application of the binary tests can't give)
#[tokio::test]
async fn the_uncategorized_domain_neither_takes_nor_gives_a_saved_slug() {
    // The entities no answer places are in `uncategorized` too.
    // The resources of a saved domain left out are gathered in `uncategorized`, which keeps its slug.
    let slugs = slugs_after(
        &answer(&[
            ("ordering", "order"),
            ("billing", "invoice"),
            ("hooks", "webhook"),
        ]),
        &answer(&[("ordering", "order"), ("billing", "invoice")]),
    )
    .await;
    assert_eq!(slugs, ["ordering", "billing", UNCATEGORIZED_SLUG]);

    // A domain of the model with the resources of the saved `uncategorized` keeps its own slug.
    let slugs = slugs_after(
        &answer(&[("ordering", "order"), ("billing", "invoice")]),
        &answer(&[
            ("ordering", "order"),
            ("billing", "invoice"),
            ("hooks", "webhook"),
        ]),
    )
    .await;
    assert_eq!(slugs, ["ordering", "billing", "hooks", UNCATEGORIZED_SLUG]);
}

// spec: RER-13 (internal: a surface the fake application of the binary tests can't give)
#[tokio::test]
async fn a_domain_of_entities_alone_written_again_keeps_its_saved_slug() {
    let entities = |slug: &str| {
        format!(
            r#"{{"domains":[{{"slug":"ordering","name":"N","description":"d","resources":["order","invoice","webhook"]}},
            {{"slug":"{slug}","name":"N","description":"d","entities":["Customer"]}}]}}"#
        )
    };
    let slugs = slugs_after(&entities("people"), &entities("customers")).await;

    assert_eq!(slugs, ["ordering", "people", UNCATEGORIZED_SLUG]);
}
