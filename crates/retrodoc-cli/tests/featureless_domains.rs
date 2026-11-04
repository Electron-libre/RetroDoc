//! A domain no feature belongs to is left out of the docs and listed in the debt report
//! (`specs/featureless-domains.md`).

mod support;

use std::path::Path;

use support::{generate, stdout_of, App, FakeLlm};

/// A shop documented by `generate` (the domain `orders` and its feature `list-orders`), to which
/// the domains `feeds` (named for an entity alone) and `operations` (whose one feature was set
/// aside as technical) are added, as the domains pass can name them.
fn shop_with_featureless_domains() -> (tempfile::TempDir, FakeLlm) {
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
    append(
        dir.path(),
        "domains.yaml",
        "- slug: feeds\n  name: Feeds\n  description: Following the orders in a feed reader.\n  \
         entities:\n  - FeedToken\n\
         - slug: operations\n  name: Operations\n  description: Keeping the shop running.\n",
    );
    std::fs::write(
        dir.path().join(".retrodoc/cache/features-set-aside.yaml"),
        "- slug: database-backup\n  domain_slug: operations\n  name: Database backup\n  \
         description: Backs the database up.\n  entry_points:\n  - app/orders_controller.rb#backup\n  \
         technical: true\n",
    )
    .unwrap();
    (dir, llm)
}

fn append(dir: &Path, artifact: &str, items: &str) {
    let path = dir.join(".retrodoc/cache").join(artifact);
    let raw = std::fs::read_to_string(&path).unwrap();
    assert!(raw.starts_with("domains:\n"), "{raw}");
    std::fs::write(&path, format!("{}\n{items}", raw.trim_end())).unwrap();
}

fn render(dir: &Path) {
    let output = support::retrodoc(dir, "render");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

// spec: EDM-1, EDM-2
#[test]
fn a_domain_with_no_feature_gets_no_page_and_no_index_entry() {
    let (dir, _llm) = shop_with_featureless_domains();

    render(dir.path());

    let functional = dir.path().join("docs/functional");
    assert!(functional.join("orders/README.md").exists());
    assert!(!functional.join("feeds").exists());
    assert!(!functional.join("operations").exists());
    let index = std::fs::read_to_string(functional.join("llms.txt")).unwrap();
    assert!(index.contains("orders"), "{index}");
    assert!(
        !index.contains("feeds") && !index.contains("Feeds"),
        "{index}"
    );
    assert!(!index.contains("Operations"), "{index}");
}

// spec: EDM-5, EDM-6
#[test]
fn the_debt_report_lists_the_domains_with_no_feature_out_of_its_domain_table() {
    let (dir, _llm) = shop_with_featureless_domains();

    let report = stdout_of(dir.path(), "report");

    assert!(report.contains("## Domains with no feature"), "{report}");
    assert!(report.contains("| Feeds | 0 |"), "{report}");
    assert!(report.contains("| Operations | 1 |"), "{report}");
    let table = report
        .split("## Domains\n")
        .nth(1)
        .and_then(|rest| rest.split("\n## ").next())
        .unwrap_or_else(|| panic!("no domain table in {report}"));
    assert!(table.contains("| orders |"), "{table}");
    let lower = table.to_lowercase();
    assert!(!lower.contains("feeds"), "{table}");
    assert!(!lower.contains("operations"), "{table}");
}
