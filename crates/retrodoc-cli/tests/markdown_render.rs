//! The render stays robust to the edge cases an LLM can put in the artifacts
//! (`specs/markdown-render.md`).

mod support;

use std::path::Path;
use std::process::Output;

use support::{generate, App, FakeLlm};

/// A shop documented by `generate`: the domain `orders`, its feature `list-orders` and the use
/// case `list-orders` of that feature.
fn documented_shop() -> (tempfile::TempDir, FakeLlm) {
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

/// Replaces the first `from` of the saved artifact `artifact` with `to`, as a model could have written it.
fn edit(dir: &Path, artifact: &str, from: &str, to: &str) {
    let path = dir.join(".retrodoc/cache").join(artifact);
    let raw = std::fs::read_to_string(&path).unwrap();
    assert!(raw.contains(from), "{raw}");
    std::fs::write(&path, raw.replacen(from, to, 1)).unwrap();
}

fn render(dir: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_retrodoc"))
        .args(["render", "--path"])
        .arg(dir)
        .env("RETRODOC_TEST_FAKE_LLM_KEY", "unused")
        .env_remove("RUST_LOG")
        .output()
        .unwrap()
}

fn rendered(dir: &Path, page: &str) -> String {
    let output = render(dir);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    std::fs::read_to_string(dir.join("docs/functional").join(page)).unwrap()
}

/// The list item linking to `target` on `page`, with the lines that follow it up to the next item or heading.
fn item<'a>(page: &'a str, target: &str) -> &'a str {
    let start = page
        .find(&format!("]({target})"))
        .and_then(|at| page[..at].rfind("\n- "))
        .unwrap_or_else(|| panic!("no item to {target} in {page}"))
        + 1;
    let rest = &page[start..];
    let end = rest.find("\n#").unwrap_or(rest.len());
    rest[..end].trim_end()
}

// spec: MDR-1
#[test]
fn a_multi_line_description_stays_in_its_list_item() {
    let (dir, _llm) = documented_shop();
    let multi_line = "description: \"Lists the orders.\\n\\nBy date.\"";
    edit(
        dir.path(),
        "features.yaml",
        "description: Lists the orders.",
        multi_line,
    );
    edit(
        dir.path(),
        "use-cases.yaml",
        "description: Lists the orders.",
        multi_line,
    );

    let domain = rendered(dir.path(), "orders/README.md");
    let feature = rendered(dir.path(), "orders/list-orders.md");

    assert_eq!(
        item(&domain, "list-orders.md"),
        "- [List orders](list-orders.md) (100%): Lists the orders. By date."
    );
    assert_eq!(
        item(&feature, "use-cases/list-orders/list-orders.md"),
        "- [List orders](use-cases/list-orders/list-orders.md) (100%): Lists the orders. By date."
    );
}

// spec: MDR-2
#[test]
fn a_weak_item_is_flagged_on_the_pages_as_in_the_agent_index() {
    let (dir, _llm) = documented_shop();
    edit(dir.path(), "features.yaml", "value: 1.0", "value: 0.3");
    edit(
        dir.path(),
        "use-cases.yaml",
        "value: 1.0\n    rationale: all steps",
        "value: 0.3\n    rationale: all steps",
    );

    let domain = rendered(dir.path(), "orders/README.md");
    let feature = rendered(dir.path(), "orders/list-orders.md");
    let index = rendered(dir.path(), "llms.txt");

    assert!(
        item(&domain, "list-orders.md").contains("(⚠️ 30%)"),
        "{domain}"
    );
    assert!(
        item(&feature, "use-cases/list-orders/list-orders.md").contains("(⚠️ 30%)"),
        "{feature}"
    );
    assert!(
        item(&index, "orders/list-orders.md").contains("(⚠️ 30%)"),
        "{index}"
    );
}
