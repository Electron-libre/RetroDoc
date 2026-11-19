//! The debt report `retrodoc report` prints is the one `render` writes, built from the docs as
//! shown (`specs/section-edit.md`, SED-39 and SED-40).

mod support;

use std::path::Path;

use support::{generate, retrodoc, stdout_of, App, FakeLlm};

/// A shop documented by `generate`: the domain `orders`, its feature `list-orders` and its use
/// case `list-orders`, with the project doc `docs/guide.md` when `guide`.
fn shop(guide: bool) -> (tempfile::TempDir, FakeLlm) {
    let app = App {
        stack: "Ruby",
        source_extensions: vec!["rb"],
        chunk_boundaries: Vec::new(),
        business: "app/",
        entry: "app/orders_controller.rb",
        model: "app/order.rb",
    };
    let llm = FakeLlm::start(move |call| app.answer(call));
    let mut files = vec![
        (
            "app/orders_controller.rb",
            "get '/orders' do\n  Order.all\nend\n",
        ),
        ("app/order.rb", "class Order\nend\n"),
    ];
    if guide {
        files.push((
            "docs/guide.md",
            "# Orders\n\nThe customers list their orders.\n",
        ));
    }
    let dir = support::repo(&files, &llm);
    let output = generate(dir.path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    (dir, llm)
}

/// Renames the feature `list-orders` by a human edit and marks its use case wrong.
fn edit_and_review(dir: &Path) {
    std::fs::write(
        dir.join(".retrodoc/edits.json"),
        r#"{"version":1,"edits":[
            {"kind":"feature","id":"list-orders","name":"Browse the orders","author":null,
             "edited_at":"2026-11-05T10:00:00Z","fingerprint":"f"}]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join(".retrodoc/reviews.json"),
        r#"{"version":1,"reviews":[
            {"kind":"use_case","id":"list-orders/list-orders","status":"wrong","comment":"Lists carts too",
             "author":null,"reviewed_at":"2026-11-04T10:00:00Z","fingerprint":"x"}]}"#,
    )
    .unwrap();
}

// spec: SED-H1
// spec: SED-39
// spec: SED-40
#[test]
fn the_printed_report_is_the_published_one_with_the_edited_names() {
    let (dir, _llm) = shop(false);
    let root = dir.path();
    edit_and_review(root);

    let published = published_and_printed(root);

    assert!(
        published.contains("`orders/list-orders` feature: Browse the orders"),
        "{published}"
    );
    assert!(
        published.contains("## Sections marked wrong\n\n- use case `orders/list-orders/list-orders`: Lists carts too ("),
        "{published}"
    );
}

// spec: SED-39
#[test]
fn both_reports_compare_the_items_with_the_project_docs() {
    let (dir, _llm) = shop(true);

    let published = published_and_printed(dir.path());

    assert!(
        published.contains("| orders | 4 | 0 | 4 | 0 | 0 |"),
        "{published}"
    );
}

/// The `coverage-report.md` that `render` writes in `dir`, checked to be what `report` prints.
fn published_and_printed(dir: &Path) -> String {
    let output = retrodoc(dir, "render");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let published = std::fs::read_to_string(dir.join("docs/_retrodoc/coverage-report.md")).unwrap();
    assert_eq!(stdout_of(dir, "report"), published);
    published
}
