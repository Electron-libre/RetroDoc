//! The human corrections given to the domains pass (`specs/human-corrections.md`).

use super::tests::{shop_surface, SHOP_ANSWER};
use super::*;
use crate::brief::ProductBrief;
use crate::corrections::{Correction, Corrections, Verdict};
use crate::testing::FakeLlm;

// spec: HCR-1, HCR-5, HCR-6, HCR-9 (internal: the application of the binary tests has one domain, whose surface a test can't change)
#[tokio::test]
async fn the_domains_asked_again_are_given_the_edited_name_of_a_domain() {
    let dir = tempfile::tempdir().unwrap();
    let llm = FakeLlm::answering(SHOP_ANSWER);
    let mut surface = shop_surface();
    let none = Corrections::default();
    build_domains_from_surface(dir.path(), &surface, &ProductBrief::default(), &llm, &none)
        .await
        .unwrap();
    let mut corrections = Corrections::default();
    corrections.add_domain(Correction {
        slug: "ordering".to_string(),
        name: Some("Order taking".to_string()),
        ..Correction::default()
    });
    // The uncategorized domain is not the model's: no correction of it reaches the call.
    corrections.add_domain(Correction {
        slug: UNCATEGORIZED_SLUG.to_string(),
        verdict: Some(Verdict::Wrong {
            comment: "Split it.".to_string(),
            reviewed_at: "2026-11-24T10:00:00Z".to_string(),
        }),
        ..Correction::default()
    });

    build_domains_from_surface(
        dir.path(),
        &surface,
        &ProductBrief::default(),
        &llm,
        &corrections,
    )
    .await
    .unwrap();
    assert_eq!(llm.calls(), 1, "an edit asks nothing again by itself");

    surface.entities.pop();
    build_domains_from_surface(
        dir.path(),
        &surface,
        &ProductBrief::default(),
        &llm,
        &corrections,
    )
    .await
    .unwrap();

    assert_eq!(llm.calls(), 2);
    let prompt = &llm.prompts()[1];
    assert!(prompt.contains("domain `ordering`"), "{prompt}");
    assert!(prompt.contains("\"Order taking\""), "{prompt}");
    assert!(!prompt.contains("Split it."), "{prompt}");
}
