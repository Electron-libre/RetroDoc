//! The format version of the cache: recorded by `generate`, checked by the commands that read the
//! generated docs back (`specs/cache-format.md`).

mod support;

use std::path::{Path, PathBuf};
use std::process::Output;

use support::{generate, retrodoc, App, FakeLlm};

/// The format the binary under test writes and reads.
const CURRENT: u32 = 1;

/// A shop documented by `generate`.
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
    assert!(output.status.success(), "{}", all_of(&output));
    (dir, llm)
}

fn format_file(dir: &Path) -> PathBuf {
    dir.join(".retrodoc/cache/format.json")
}

fn all_of(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// Runs `command` on `dir`, which must fail; its output.
fn refused(dir: &Path, command: &str) -> String {
    let output = retrodoc(dir, command);
    let all = all_of(&output);
    assert!(!output.status.success(), "{command} did not fail:\n{all}");
    all
}

// spec: CFV-H1, CFV-1, CFV-6
#[test]
fn generate_records_the_format_and_render_reads_the_docs() {
    let (dir, _llm) = shop();

    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(format_file(dir.path())).unwrap()).unwrap();
    assert_eq!(saved, serde_json::json!({ "version": CURRENT }));
    let output = retrodoc(dir.path(), "render");
    assert!(output.status.success(), "{}", all_of(&output));
}

// spec: CFV-H2, CFV-2
#[test]
fn a_cache_of_an_older_format_is_refused_with_the_command_to_run() {
    let (dir, _llm) = shop();
    std::fs::write(format_file(dir.path()), r#"{"version": 0}"#).unwrap();

    for command in ["ui", "render", "report"] {
        let error = refused(dir.path(), command);
        assert!(error.contains("format 0"), "{command}: {error}");
        assert!(
            error.contains(&format!("format {CURRENT}")),
            "{command}: {error}"
        );
        assert!(
            error.contains("retrodoc generate --force"),
            "{command}: {error}"
        );
    }
}

// spec: CFV-3
#[test]
fn a_cache_of_a_later_format_is_refused_asking_to_update_retrodoc() {
    let (dir, _llm) = shop();
    std::fs::write(format_file(dir.path()), r#"{"version": 99}"#).unwrap();

    let error = refused(dir.path(), "report");

    assert!(error.contains("format 99"), "{error}");
    assert!(error.contains(&format!("format {CURRENT}")), "{error}");
    assert!(error.contains("update RetroDoc"), "{error}");
}

// spec: CFV-8
#[test]
fn an_unreadable_format_file_is_refused_naming_it() {
    let (dir, _llm) = shop();
    std::fs::write(format_file(dir.path()), "{").unwrap();

    let error = refused(dir.path(), "render");

    assert!(error.contains(".retrodoc/cache/format.json"), "{error}");
    assert!(error.contains("update RetroDoc"), "{error}");
}

// spec: CFV-7
#[test]
fn a_cache_with_no_format_file_reads_as_format_1() {
    let (dir, _llm) = shop();
    std::fs::remove_file(format_file(dir.path())).unwrap();

    let output = retrodoc(dir.path(), "render");

    assert!(output.status.success(), "{}", all_of(&output));
}

// spec: CFV-9
#[test]
fn recording_the_format_on_an_older_cache_calls_no_llm() {
    let (dir, llm) = shop();
    std::fs::remove_file(format_file(dir.path())).unwrap();
    let before = llm.calls().len();

    let output = generate(dir.path());

    assert!(output.status.success(), "{}", all_of(&output));
    assert_eq!(llm.calls().len(), before, "{}", all_of(&output));
    assert!(format_file(dir.path()).is_file());
}
