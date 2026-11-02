//! The pages no run produces any more are removed when nobody edited them since they were written
//! (`specs/docs-pruning.md`).

mod support;

use std::path::Path;

use support::{generate, App, FakeLlm};

/// A shop documented by `generate`, its single domain `orders`.
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

/// Renames the domain `orders` to `purchases` in the saved artifacts, as a rerun would.
fn rename_the_domain(dir: &Path) {
    for artifact in ["domains.yaml", "features.yaml", "use-cases.yaml"] {
        let path = dir.join(".retrodoc/cache").join(artifact);
        let raw = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, raw.replace(": orders\n", ": purchases\n")).unwrap();
    }
}

fn render(dir: &Path, dry_run: bool) -> String {
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_retrodoc"));
    command.args(["render", "--path"]).arg(dir);
    if dry_run {
        command.arg("--dry-run");
    }
    let output = command
        .env("RETRODOC_TEST_FAKE_LLM_KEY", "unused")
        .env_remove("RUST_LOG")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(output.status.success(), "{stdout}");
    stdout
}

// spec: RND-1
#[test]
fn generate_records_the_hash_of_the_pages_it_writes() {
    let (dir, _llm) = documented_shop();

    let record =
        std::fs::read_to_string(dir.path().join("docs/_retrodoc/rendered-pages.json")).unwrap();

    assert!(record.contains("functional/orders/README.md"), "{record}");
}

// spec: RND-2, RND-4
#[test]
fn render_removes_the_folder_of_a_renamed_domain() {
    let (dir, _llm) = documented_shop();
    rename_the_domain(dir.path());

    let stdout = render(dir.path(), false);

    let functional = dir.path().join("docs/functional");
    assert!(!functional.join("orders").exists(), "{stdout}");
    assert!(functional.join("purchases/README.md").exists(), "{stdout}");
    assert!(
        stdout.contains("remove  functional/orders/README.md"),
        "{stdout}"
    );
    assert!(!stdout.contains("Not produced by this run"), "{stdout}");
}

// spec: RND-3
#[test]
fn render_keeps_an_edited_page_of_a_renamed_domain() {
    let (dir, _llm) = documented_shop();
    let page = dir.path().join("docs/functional/orders/README.md");
    let edited = format!(
        "{}\nA note by hand.\n",
        std::fs::read_to_string(&page).unwrap()
    );
    std::fs::write(&page, edited).unwrap();
    rename_the_domain(dir.path());

    let stdout = render(dir.path(), false);

    assert!(page.exists(), "{stdout}");
    assert!(stdout.contains("Not produced by this run"), "{stdout}");
    assert!(stdout.contains("functional/orders/README.md"), "{stdout}");
}

// spec: RND-3
#[test]
fn render_keeps_a_page_it_has_no_record_of() {
    let (dir, _llm) = documented_shop();
    std::fs::remove_file(dir.path().join("docs/_retrodoc/rendered-pages.json")).unwrap();
    rename_the_domain(dir.path());

    let stdout = render(dir.path(), false);

    assert!(
        dir.path().join("docs/functional/orders/README.md").exists(),
        "{stdout}"
    );
    assert!(stdout.contains("Not produced by this run"), "{stdout}");
}

// spec: RND-5
#[test]
fn a_dry_run_lists_the_pages_it_would_remove_and_removes_none() {
    let (dir, _llm) = documented_shop();
    rename_the_domain(dir.path());

    let stdout = render(dir.path(), true);

    assert!(
        stdout.contains("remove  functional/orders/README.md"),
        "{stdout}"
    );
    assert!(
        dir.path().join("docs/functional/orders/README.md").exists(),
        "{stdout}"
    );
}

// spec: RND-6
#[test]
fn a_second_render_after_a_removal_changes_nothing() {
    let (dir, _llm) = documented_shop();
    rename_the_domain(dir.path());
    render(dir.path(), false);

    let stdout = render(dir.path(), false);

    assert!(
        stdout.contains("Docs are up to date, nothing to write."),
        "{stdout}"
    );
    assert!(!stdout.contains("remove  "), "{stdout}");
}
