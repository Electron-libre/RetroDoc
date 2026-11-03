//! The features the features answer marks technical are set aside out of the functional docs and
//! listed in the debt report (`specs/technical-features-set-aside.md`).

mod support;

use serde_json::{json, Value};
use support::{generate, stdout_of, warnings, App, Call, FakeLlm};

const ENTRY: &str = "app/orders_controller.rb";

/// A shop whose entry file also holds a task that seeds test data, which the features answer marks
/// technical.
fn shop_with_a_test_data_task() -> (tempfile::TempDir, FakeLlm) {
    let app = App {
        stack: "Ruby",
        source_extensions: vec!["rb"],
        chunk_boundaries: Vec::new(),
        business: "app/",
        entry: ENTRY,
        model: "app/order.rb",
    };
    let llm = FakeLlm::start(move |call| answer(&app, call));
    let dir = support::repo(
        &[
            (
                ENTRY,
                "get '/orders' do\n  Order.all\nend\n\ntask :seed do\n  Order.create\nend\n",
            ),
            ("app/order.rb", "class Order\nend\n"),
        ],
        &llm,
    );
    (dir, llm)
}

fn answer(app: &App, call: &Call) -> String {
    match call.pass.as_str() {
        "business-read" | "glossary" | "entry-points" => {
            let mut read: Value = serde_json::from_str(&app.answer(call)).unwrap();
            if call.files().iter().any(|(file, _)| file == ENTRY) {
                read["entry_points"].as_array_mut().unwrap().push(json!({
                    "file": ENTRY, "kind": "cli_command", "name": "rake seed", "verb": "seed",
                    "resource": "order", "description": "Creates fake orders for tests.", "outputs": [],
                }));
            }
            read.to_string()
        }
        "features" => json!({"features": [
            {"slug": "list-orders", "name": "List orders", "description": "Lists the orders.",
             "back_office": false, "technical": false, "entry_points": ["E1"]},
            {"slug": "test-data", "name": "Test data generation", "description": "Seeds fake orders.",
             "back_office": true, "technical": true, "entry_points": ["E2"]},
        ]})
        .to_string(),
        _ => app.answer(call),
    }
}

fn cache(dir: &std::path::Path, file: &str) -> String {
    std::fs::read_to_string(dir.join(".retrodoc/cache").join(file)).unwrap_or_default()
}

// spec: TEF-1, TEF-2, TEF-3, TEF-4, TEF-5, TEF-6
#[test]
fn a_technical_feature_is_set_aside_listed_in_the_report_and_kept_on_a_rerun() {
    let (dir, llm) = shop_with_a_test_data_task();

    let output = generate(dir.path());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{stdout}");

    let features = cache(dir.path(), "features.yaml");
    assert!(features.contains("List orders"), "{features}");
    assert!(!features.contains("Test data generation"), "{features}");
    let aside = cache(dir.path(), "features-set-aside.yaml");
    assert!(aside.contains("Test data generation"), "{aside}");
    assert!(aside.contains("domain_slug: orders"), "{aside}");
    assert!(
        aside.contains("app/orders_controller.rb#rake seed"),
        "{aside}"
    );
    assert!(
        llm.calls_of("features-repair").is_empty(),
        "every entry point is held: no placement call"
    );
    let coverage = cache(dir.path(), "behaviour-coverage.yaml");
    assert!(!coverage.contains("rake seed"), "{coverage}");
    assert!(
        !warnings(&output)
            .iter()
            .any(|w| w.contains("no feature holds")),
        "{stdout}"
    );
    assert!(
        llm.calls_of("use-cases")
            .iter()
            .all(|c| !c.user.contains("Test data generation")),
        "a technical feature gets no use case"
    );

    let report = stdout_of(dir.path(), "report");
    assert!(
        report.contains("## Technical features set aside"),
        "{report}"
    );
    assert!(
        report.contains("| orders | Test data generation | 1 |"),
        "{report}"
    );

    let calls = llm.calls().len();
    let again = generate(dir.path());
    assert!(again.status.success());
    assert_eq!(llm.calls().len(), calls, "an unchanged rerun makes no call");
    assert_eq!(cache(dir.path(), "features-set-aside.yaml"), aside);
}
