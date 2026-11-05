//! The render stays robust to the edge cases an LLM can put in the artifacts
//! (`specs/markdown-render.md`) and writes the human edits over them (`specs/section-edit.md`).

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

/// The list item linking to `target` on `page` (its indent included), with the lines that follow it up
/// to the next item or heading.
fn item<'a>(page: &'a str, target: &str) -> &'a str {
    let start = page
        .find(&format!("]({target})"))
        .and_then(|at| page[..at].rfind('\n'))
        .unwrap_or_else(|| panic!("no item to {target} in {page}"))
        + 1;
    let rest = &page[start..];
    // Up to the next item, at any indent, or the next heading.
    let end = rest
        .match_indices('\n')
        .map(|(at, _)| at)
        .find(|&at| {
            let next = rest[at + 1..].trim_start_matches(' ');
            next.starts_with("- ") || next.starts_with('#')
        })
        .unwrap_or(rest.len());
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

// spec: MDR-3
#[test]
fn an_inverted_range_is_shown_in_order() {
    let (dir, _llm) = documented_shop();
    edit(
        dir.path(),
        "use-cases.yaml",
        "start_line: 1\n      end_line: 1",
        "start_line: 9\n      end_line: 3",
    );

    let page = rendered(dir.path(), "orders/use-cases/list-orders/list-orders.md");

    assert!(page.contains("`app/orders_controller.rb:3-9`"), "{page}");
}

// spec: MDR-4
#[test]
fn two_slugs_folded_into_one_path_fail_the_render() {
    let (dir, _llm) = documented_shop();
    edit(
        dir.path(),
        "domains.yaml",
        "domains:\n",
        "domains:\n- slug: Orders\n  name: Orders again\n  description: The same folder.\n",
    );
    // A domain with no feature gets no page (EDM-1): give it one.
    edit(
        dir.path(),
        "features.yaml",
        "- slug: list-orders",
        "- slug: count-orders\n  domain_slug: Orders\n  name: Count orders\n  description: Counts the orders.\n  \
         source_paths: []\n  entry_points: []\n- slug: list-orders",
    );

    let readme = dir.path().join("docs/functional/orders/README.md");
    let before = std::fs::read_to_string(&readme).unwrap();

    let output = render(dir.path());

    assert_eq!(std::fs::read_to_string(&readme).unwrap(), before);
    let printed = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!output.status.success(), "{printed}");
    assert!(printed.contains("functional/orders/README.md"), "{printed}");
}

// spec: MDR-5
#[test]
fn a_hostile_slug_stays_in_the_docs_dir() {
    let (dir, _llm) = documented_shop();
    edit(
        dir.path(),
        "domains.yaml",
        "slug: orders",
        "slug: ../../etc",
    );
    edit(
        dir.path(),
        "features.yaml",
        "domain_slug: orders",
        "domain_slug: ../../etc",
    );

    let page = rendered(dir.path(), "etc/README.md");

    assert!(page.starts_with("# Orders\n"), "{page}");
    assert!(!dir.path().join("etc").exists());
}

/// Saves the edits a human would make of the domain `orders`, the feature `list-orders` and the
/// narrative of its use case.
fn edit_the_shop(dir: &Path) {
    std::fs::write(
        dir.join(".retrodoc/edits.json"),
        r#"{"version":1,"edits":[
            {"kind":"domain","id":"orders","description":"What the customers bought.","author":null,
             "edited_at":"2026-11-05T10:00:00Z","fingerprint":"f"},
            {"kind":"feature","id":"list-orders","name":"Browse the orders","author":null,
             "edited_at":"2026-11-05T10:00:00Z","fingerprint":"f"},
            {"kind":"use_case","id":"list-orders/list-orders","narrative":"The customer reads past orders.",
             "author":null,"edited_at":"2026-11-05T10:00:00Z","fingerprint":"f"}]}"#,
    )
    .unwrap();
}

// spec: SED-30
#[test]
fn the_rendered_docs_show_the_edited_fields() {
    let (dir, _llm) = documented_shop();
    edit_the_shop(dir.path());

    let domain = rendered(dir.path(), "orders/README.md");
    let use_case = rendered(dir.path(), "orders/use-cases/list-orders/list-orders.md");

    assert!(domain.contains("What the customers bought."), "{domain}");
    assert!(!domain.contains("Orders of the shop."), "{domain}");
    assert_eq!(
        item(&domain, "list-orders.md"),
        "- [Browse the orders](list-orders.md) (100%): Lists the orders."
    );
    assert!(
        use_case.contains("The customer reads past orders."),
        "{use_case}"
    );
    assert!(
        !use_case.contains("The customer lists the orders."),
        "{use_case}"
    );
}

// spec: SED-31
#[test]
fn the_same_artifacts_and_edits_render_the_same_bytes() {
    let (dir, _llm) = documented_shop();
    edit_the_shop(dir.path());
    let snapshot = |dir: &Path| -> Vec<(std::path::PathBuf, Vec<u8>)> {
        let root = dir.join("docs/functional");
        let mut files: Vec<_> = walk(&root)
            .into_iter()
            .filter(|path| path.file_name().is_some_and(|name| name != "metadata.json"))
            .map(|path| {
                (
                    path.strip_prefix(&root).unwrap().to_path_buf(),
                    std::fs::read(&path).unwrap(),
                )
            })
            .collect();
        files.sort();
        files
    };

    assert!(render(dir.path()).status.success());
    let first = snapshot(dir.path());
    let output = render(dir.path());

    assert!(output.status.success());
    assert_eq!(snapshot(dir.path()), first);
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("0 file(s) written"),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

/// The line under the `# ` heading of `page`.
fn under_heading(page: &str) -> &str {
    page.lines()
        .skip_while(|line| !line.starts_with("# "))
        .nth(2)
        .unwrap_or_default()
}

// spec: SED-38
#[test]
fn the_page_of_an_edited_section_says_who_edited_it_and_when() {
    let (dir, _llm) = documented_shop();
    std::fs::write(
        dir.path().join(".retrodoc/edits.json"),
        r#"{"version":1,"edits":[
            {"kind":"domain","id":"orders","description":"  ","author":"B",
             "edited_at":"2026-11-04T09:00:00Z","fingerprint":"f"},
            {"kind":"feature","id":"list-orders","name":"Browse the orders","author":"A",
             "edited_at":"2026-11-05T10:00:00Z","fingerprint":"f"},
            {"kind":"use_case","id":"list-orders/list-orders","description":"Reads the orders.",
             "narrative":"The customer reads past orders.","author":null,
             "edited_at":"2026-11-05T11:00:00Z","fingerprint":"f"}]}"#,
    )
    .unwrap();

    let domain = rendered(dir.path(), "orders/README.md");
    let feature = rendered(dir.path(), "orders/list-orders.md");
    let use_case = rendered(dir.path(), "orders/use-cases/list-orders/list-orders.md");

    assert_eq!(
        under_heading(&feature),
        "**Human edit** name *by A, 2026-11-05T10:00:00Z*",
        "{feature}"
    );
    assert_eq!(
        under_heading(&use_case),
        "**Human edit** description, narrative *2026-11-05T11:00:00Z*",
        "{use_case}"
    );
    assert!(!domain.contains("Human edit"), "{domain}");
}

/// The files under `dir`, at any depth.
fn walk(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(walk(&path));
        } else {
            files.push(path);
        }
    }
    files
}

// spec: SED-32
#[test]
fn a_file_of_edits_that_cannot_be_read_fails_the_render_and_writes_no_doc() {
    let (dir, _llm) = documented_shop();
    let page = dir.path().join("docs/functional/orders/README.md");
    let before = std::fs::read_to_string(&page).unwrap();
    edit_the_shop(dir.path());
    assert!(render(dir.path()).status.success());
    assert_ne!(
        std::fs::read_to_string(&page).unwrap(),
        before,
        "the edit was written"
    );
    std::fs::write(dir.path().join(".retrodoc/edits.json"), "{ not json").unwrap();
    let written = std::fs::read_to_string(&page).unwrap();

    let output = render(dir.path());

    assert!(!output.status.success());
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(said.contains(".retrodoc/edits.json"), "{said}");
    assert_eq!(std::fs::read_to_string(&page).unwrap(), written);
}

// spec: SED-32
#[test]
fn a_file_of_edits_that_cannot_be_read_fails_generate_before_any_llm_call() {
    let llm = FakeLlm::start(|_| unreachable!("no call expected"));
    let dir = support::repo(&[("app/order.rb", "class Order\nend\n")], &llm);
    std::fs::create_dir_all(dir.path().join(".retrodoc")).unwrap();
    std::fs::write(dir.path().join(".retrodoc/edits.json"), "{ not json").unwrap();

    let output = generate(dir.path());

    assert!(!output.status.success());
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(said.contains(".retrodoc/edits.json"), "{said}");
    assert_eq!(llm.calls().len(), 0);
    assert!(!dir.path().join("docs/functional").exists());
}

// spec: SED-33
#[test]
fn a_blank_field_written_by_hand_shows_the_generated_text() {
    let (dir, _llm) = documented_shop();
    std::fs::write(
        dir.path().join(".retrodoc/edits.json"),
        r#"{"version":1,"edits":[{"kind":"feature","id":"list-orders","name":"  ","author":null,
            "edited_at":"2026-11-05T10:00:00Z","fingerprint":"f"}]}"#,
    )
    .unwrap();

    let domain = rendered(dir.path(), "orders/README.md");

    assert_eq!(
        item(&domain, "list-orders.md"),
        "- [List orders](list-orders.md) (100%): Lists the orders."
    );
}
