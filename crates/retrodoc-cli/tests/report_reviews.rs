//! The debt report of `retrodoc report` lists the sections a human marked wrong
//! (`specs/debt-view.md`, DBT-35 and DBT-36).

mod support;

use std::path::Path;

use support::{generate, retrodoc, stdout_of, warnings, App, FakeLlm};

/// A shop documented by `generate`: the domain `orders`, its feature `list-orders` and its use
/// case `list-orders`.
fn shop() -> (tempfile::TempDir, FakeLlm) {
    let app = App {
        stack: "Ruby",
        source_extensions: vec!["rb"],
        chunk_boundaries: Vec::new(),
        business: "app/",
        entry: "app/orders_controller.rb",
        model: "app/order.rb",
    };
    let llm = FakeLlm::start(move |call| app.answer(call));
    let dir = support::repo(
        &[
            (
                "app/orders_controller.rb",
                "get '/orders' do\n  Order.all\nend\n",
            ),
            ("app/order.rb", "class Order\nend\n"),
        ],
        &llm,
    );
    let output = generate(dir.path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    (dir, llm)
}

fn write_reviews(dir: &Path, reviews: &[(&str, &str, &str, &str)]) {
    let reviews: Vec<serde_json::Value> = reviews
        .iter()
        .map(|(kind, id, status, comment)| {
            serde_json::json!({
                "kind": kind, "id": id, "status": status, "comment": comment,
                "author": null, "reviewed_at": "2026-11-04T10:00:00Z", "fingerprint": "x",
            })
        })
        .collect();
    std::fs::write(
        dir.join(".retrodoc/reviews.json"),
        serde_json::json!({ "version": 1, "reviews": reviews }).to_string(),
    )
    .unwrap();
}

const HEADING: &str = "## Sections marked wrong";

// spec: DBT-35
#[test]
fn the_report_lists_the_sections_of_the_run_marked_wrong_with_their_comment() {
    let (dir, _llm) = shop();
    write_reviews(
        dir.path(),
        &[
            ("domain", "orders", "wrong", "Mixes orders and carts"),
            ("feature", "gone", "wrong", "no longer in the run"),
            ("feature", "list-orders", "validated", ""),
            ("use_case", "list-orders/list-orders", "wrong", ""),
        ],
    );

    let report = stdout_of(dir.path(), "report");

    let section = report
        .split(HEADING)
        .nth(1)
        .and_then(|rest| rest.split("\n## ").next())
        .unwrap_or_else(|| panic!("no {HEADING} in {report}"));
    assert_eq!(
        section.trim(),
        "- domain `orders`: Mixes orders and carts\n\
         - use case `orders/list-orders/list-orders`"
    );
}

// spec: DBT-35
#[test]
fn the_report_has_no_heading_of_the_sections_marked_wrong_when_none_is() {
    let (dir, _llm) = shop();
    let report = stdout_of(dir.path(), "report");
    assert!(!report.contains(HEADING), "{report}");

    write_reviews(dir.path(), &[("domain", "orders", "to_review", "")]);
    let report = stdout_of(dir.path(), "report");
    assert!(!report.contains(HEADING), "{report}");
}

// spec: DBT-36
#[test]
fn an_unreadable_file_of_reviews_is_warned_of_and_the_report_printed_without_it() {
    let (dir, _llm) = shop();
    std::fs::write(dir.path().join(".retrodoc/reviews.json"), "{ not json").unwrap();

    let output = retrodoc(dir.path(), "report");

    assert!(output.status.success());
    let warnings = warnings(&output);
    assert!(
        warnings.iter().any(|w| w.contains("reviews")),
        "{warnings:?}"
    );
    let report = String::from_utf8_lossy(&output.stdout);
    assert!(
        report.contains("# Documentation coverage report"),
        "{report}"
    );
    assert!(!report.contains(HEADING), "{report}");
}
