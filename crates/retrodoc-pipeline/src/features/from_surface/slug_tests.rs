//! The saved slugs of the features a domain writes again (`specs/rerun.md` RER-13 to RER-15).

use super::tests::{big_domain, fixture};
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

/// A review `wrong` of the feature `slug`, held by `entry_points`: it asks its slice again.
fn wrong(domain: &str, slug: &str, entry_points: &[&str]) -> Corrections {
    let mut corrections = Corrections::default();
    corrections.add_feature(
        domain,
        Correction {
            slug: slug.to_string(),
            entry_points: entry_points.iter().map(ToString::to_string).collect(),
            verdict: Some(Verdict::Wrong {
                comment: "Name it better.".to_string(),
                reviewed_at: "2026-11-24T10:00:00Z".to_string(),
            }),
            ..Correction::default()
        },
    );
    corrections
}

fn slugs(features: &[Feature]) -> Vec<(&str, &str)> {
    features
        .iter()
        .map(|f| (f.domain_slug.as_str(), f.slug.as_str()))
        .collect()
}

/// `orders` answers `place-order` (E1, E2), `routing` answers `list-routes` (E1); asked again
/// with a correction, each answers its features under the slugs of `again`.
fn answers(prompt: &str, again: &[&str; 2]) -> String {
    let asked_again = prompt.contains("Human corrections");
    let [orders, routing] = if asked_again {
        *again
    } else {
        ["place-order", "list-routes"]
    };
    if prompt.contains("Entry points to place") {
        r#"{"placements":[]}"#.to_string()
    } else if prompt.contains("GET /orders") {
        format!(
            r#"{{"features":[{{"slug":"{orders}","name":"N","description":"d","entry_points":["E1","E2"]}}]}}"#
        )
    } else {
        format!(
            r#"{{"features":[{{"slug":"{routing}","name":"N","description":"d","entry_points":["E1"]}}]}}"#
        )
    }
}

// spec: RER-13 (internal: the application of the binary tests has one feature per domain, which its correction names)
#[tokio::test]
async fn a_feature_written_again_under_a_new_slug_keeps_its_saved_one() {
    let dir = tempfile::tempdir().unwrap();
    let (domains, entries) = fixture();
    let llm = FakeLlm::replying(|_, request| {
        Ok(answers(
            &request.messages[1].content,
            &["order-goods", "list-routes"],
        ))
    });
    run(
        dir.path(),
        (&domains, &entries),
        &llm,
        &Corrections::default(),
    )
    .await;

    let corrections = wrong(
        "orders",
        "place-order",
        &["orders.rb#GET /orders", "orders.rb#POST /orders"],
    );
    let features = run(dir.path(), (&domains, &entries), &llm, &corrections).await;

    assert_eq!(
        slugs(&features),
        [("orders", "place-order"), ("routing", "list-routes")]
    );
}

// spec: RER-15 (internal: two domains answering the same slug, which the fake application of the binary tests can't give)
#[tokio::test]
async fn a_saved_slug_another_domain_took_is_not_given_back() {
    let dir = tempfile::tempdir().unwrap();
    let (domains, entries) = fixture();
    // Asked again, `orders` gives the slug `routing` had to a part of its feature, and `routing`
    // answers another one.
    let llm = FakeLlm::replying(|_, request| {
        let prompt = &request.messages[1].content;
        let answer = answers(prompt, &["list-routes", "routes"]);
        Ok(if prompt.contains("Human corrections") {
            answer.replace(r#"["E1","E2"]"#, r#"["E1"]"#)
        } else {
            answer
        })
    });
    run(
        dir.path(),
        (&domains, &entries),
        &llm,
        &Corrections::default(),
    )
    .await;

    let mut corrections = wrong(
        "orders",
        "place-order",
        &["orders.rb#GET /orders", "orders.rb#POST /orders"],
    );
    corrections.add_feature(
        "routing",
        Correction {
            slug: "list-routes".to_string(),
            entry_points: vec!["orders.rb#GET /routes".to_string()],
            verdict: Some(Verdict::Wrong {
                comment: "Name it better.".to_string(),
                reviewed_at: "2026-11-24T10:00:00Z".to_string(),
            }),
            ..Correction::default()
        },
    );
    let features = run(dir.path(), (&domains, &entries), &llm, &corrections).await;

    assert_eq!(
        slugs(&features),
        [("orders", "list-routes"), ("routing", "routes")]
    );
}

// spec: RER-13, RER-14 (internal: a domain over the prompt limit takes 100 entry points, too many for the fake application of the binary tests)
#[tokio::test]
async fn the_part_of_a_feature_a_slice_writes_again_merges_back_under_its_slug() {
    let dir = tempfile::tempdir().unwrap();
    let (domains, entries) = big_domain();
    // Each slice claims its first entry point in `all`, the second one also its second in
    // `later`; asked again, the second slice renames both.
    let llm = FakeLlm::replying(|_, request| {
        let prompt = &request.messages[1].content;
        if prompt.contains("Entry points to place") {
            return Ok(r#"{"placements":[]}"#.to_string());
        }
        let [all, later] = if prompt.contains("Human corrections") {
            ["everything", "afterwards"]
        } else {
            ["all", "later"]
        };
        let extra = if prompt.contains("already found") {
            format!(r#",{{"slug":"{later}","name":"L","description":"d","entry_points":["E2"]}}"#)
        } else {
            String::new()
        };
        Ok(format!(
            r#"{{"features":[{{"slug":"{all}","name":"{all}","description":"d","entry_points":["E1"]}}{extra}]}}"#
        ))
    });
    run(
        dir.path(),
        (&domains, &entries),
        &llm,
        &Corrections::default(),
    )
    .await;

    let corrections = wrong("big", "later", &["b.rb#GET /b1"]);
    let features = run(dir.path(), (&domains, &entries), &llm, &corrections).await;

    assert_eq!(slugs(&features), [("big", "all"), ("big", "later")]);
    assert_eq!(
        features[0].entry_points,
        ["a.rb#GET /a0", "b.rb#GET /b0"],
        "the part of the second slice merged back"
    );
}
