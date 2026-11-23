//! The format version of the cache: recorded by `generate`, checked by the commands that read the
//! generated docs back (`specs/cache-format.md`).

mod support;

use std::path::{Path, PathBuf};
use std::process::Output;

use support::{generate, retrodoc, App, FakeLlm};

/// The format the binary under test writes and reads.
const CURRENT: u32 = retrodoc_core::cache::format::FORMAT_VERSION;

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

/// The content of every file under the cache, by path.
fn cache_of(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files: Vec<_> = walk(&dir.join(".retrodoc/cache"))
        .into_iter()
        .map(|p| {
            let content = std::fs::read(&p).unwrap();
            (p, content)
        })
        .collect();
    files.sort();
    files
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .unwrap()
        .flat_map(|e| {
            let path = e.unwrap().path();
            if path.is_dir() {
                walk(&path)
            } else {
                vec![path]
            }
        })
        .collect()
}

// spec: CFV-4
#[test]
fn generate_refuses_an_older_cache_without_force() {
    let (dir, llm) = shop();
    std::fs::write(format_file(dir.path()), r#"{"version": 0}"#).unwrap();
    let (before, calls) = (cache_of(dir.path()), llm.calls().len());

    let error = refused(dir.path(), "generate");

    assert!(error.contains("format 0"), "{error}");
    assert!(error.contains("--force"), "{error}");
    assert_eq!(llm.calls().len(), calls);
    assert_eq!(cache_of(dir.path()), before);

    let output = support::run(dir.path(), &["generate", "--force"]);
    assert!(output.status.success(), "{}", all_of(&output));
    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(format_file(dir.path())).unwrap()).unwrap();
    assert_eq!(saved, serde_json::json!({ "version": CURRENT }));
}

// spec: CFV-5
#[test]
fn generate_refuses_a_later_cache_even_with_force() {
    let (dir, llm) = shop();
    for content in [r#"{"version": 99}"#, "{"] {
        std::fs::write(format_file(dir.path()), content).unwrap();
        let (before, calls) = (cache_of(dir.path()), llm.calls().len());

        for args in [&["generate"][..], &["generate", "--force"]] {
            let output = support::run(dir.path(), args);
            let all = all_of(&output);
            assert!(!output.status.success(), "{args:?} on {content}: {all}");
            assert!(
                all.contains("update RetroDoc"),
                "{args:?} on {content}: {all}"
            );
            assert_eq!(llm.calls().len(), calls, "{args:?} on {content}");
            assert_eq!(cache_of(dir.path()), before, "{args:?} on {content}");
        }
    }
}

/// The commands of a single pass, `--force` added to those that take it.
const SINGLE_PASS: [&[&str]; 9] = [
    &["roles", "--force"],
    &["glossary"],
    &["entry-points"],
    &["actors", "--force"],
    &["business-read"],
    &["business-files", "--force"],
    &["contexts", "--force"],
    &["brief", "--force"],
    &["surface"],
];

// spec: CFV-11
#[test]
fn the_commands_of_a_single_pass_refuse_a_cache_of_another_format() {
    let (dir, llm) = shop();
    for (version, says) in [(0, "retrodoc generate --force"), (99, "update RetroDoc")] {
        std::fs::write(
            format_file(dir.path()),
            format!(r#"{{"version": {version}}}"#),
        )
        .unwrap();
        let (before, calls) = (cache_of(dir.path()), llm.calls().len());

        for args in SINGLE_PASS {
            let output = support::run(dir.path(), args);
            let all = all_of(&output);
            assert!(!output.status.success(), "{args:?} did not fail:\n{all}");
            assert!(
                all.contains(&format!("format {version}")),
                "{args:?}: {all}"
            );
            assert!(all.contains(says), "{args:?}: {all}");
            assert_eq!(llm.calls().len(), calls, "{args:?}");
            assert_eq!(cache_of(dir.path()), before, "{args:?}");
        }
    }
}

// spec: CFV-12
#[test]
fn a_command_of_a_single_pass_records_the_format_of_what_it_writes() {
    let (dir, _llm) = shop();
    std::fs::remove_dir_all(dir.path().join(".retrodoc/cache")).unwrap();

    let signals = support::run(dir.path(), &["brief", "--signals"]);
    assert!(signals.status.success(), "{}", all_of(&signals));
    assert!(
        !format_file(dir.path()).exists(),
        "brief --signals writes nothing"
    );
    let output = retrodoc(dir.path(), "roles");

    assert!(output.status.success(), "{}", all_of(&output));
    assert!(dir.path().join(".retrodoc/cache/roles.yaml").is_file());
    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(format_file(dir.path())).unwrap()).unwrap();
    assert_eq!(saved, serde_json::json!({ "version": CURRENT }));
}

// spec: CFV-13
#[test]
fn generate_force_moves_an_older_cache_aside_with_its_hand_edits() {
    let (dir, _llm) = shop();
    std::fs::write(format_file(dir.path()), r#"{"version": 0}"#).unwrap();
    let roles = dir.path().join(".retrodoc/cache/roles.yaml");
    let edited = format!(
        "{}# edited by hand\n",
        std::fs::read_to_string(&roles).unwrap()
    );
    std::fs::write(&roles, &edited).unwrap();
    let history = dir.path().join(".retrodoc/cache/history");
    let snapshots = std::fs::read_dir(&history).unwrap().count();

    let estimate = support::run(dir.path(), &["generate", "--force", "--estimate"]);
    assert!(estimate.status.success(), "{}", all_of(&estimate));
    assert!(!dir.path().join(".retrodoc/cache-format-0").exists());

    let output = support::run(dir.path(), &["generate", "--force"]);
    let all = all_of(&output);
    assert!(output.status.success(), "{all}");
    let aside = dir.path().join(".retrodoc/cache-format-0");
    assert_eq!(
        std::fs::read_to_string(aside.join("roles.yaml")).unwrap(),
        edited
    );
    assert!(all.contains(".retrodoc/cache-format-0"), "{all}");
    assert!(all.contains("roles.yaml"), "{all}");
    assert_ne!(std::fs::read_to_string(&roles).unwrap(), edited);
    assert!(!aside.join("history").exists());
    assert!(std::fs::read_dir(&history).unwrap().count() >= snapshots);
    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(format_file(dir.path())).unwrap()).unwrap();
    assert_eq!(saved, serde_json::json!({ "version": CURRENT }));
}

// spec: CFV-13
#[test]
fn generate_force_refused_by_the_edits_moves_nothing() {
    let (dir, llm) = shop();
    std::fs::write(format_file(dir.path()), r#"{"version": 0}"#).unwrap();
    std::fs::write(dir.path().join(".retrodoc/edits.json"), "{").unwrap();
    let (before, calls) = (cache_of(dir.path()), llm.calls().len());

    let error = support::run(dir.path(), &["generate", "--force"]);

    assert!(!error.status.success(), "{}", all_of(&error));
    assert_eq!(cache_of(dir.path()), before);
    assert_eq!(llm.calls().len(), calls);
    assert!(!dir.path().join(".retrodoc/cache-format-0").exists());
}

/// `benchmark` against the reference of the default smoke repository, `extra` added.
fn benchmark_args(extra: &[&'static str]) -> Vec<&'static str> {
    let reference = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../benchmark/delivery_router/reference.yaml"
    );
    let mut args = vec!["benchmark", "--reference", reference];
    args.extend_from_slice(extra);
    args
}

// spec: CFV-14
#[test]
fn scan_and_benchmark_refuse_a_cache_of_another_format() {
    let (dir, llm) = shop();
    for (version, says) in [(0, "retrodoc generate --force"), (99, "update RetroDoc")] {
        std::fs::write(
            format_file(dir.path()),
            format!(r#"{{"version": {version}}}"#),
        )
        .unwrap();
        let calls = llm.calls().len();

        for args in [
            vec!["scan"],
            benchmark_args(&[]),
            benchmark_args(&["--judge"]),
            benchmark_args(&["--saved-judge"]),
        ] {
            let output = support::run(dir.path(), &args);
            let all = all_of(&output);
            assert!(!output.status.success(), "{args:?} did not fail:\n{all}");
            assert!(
                all.contains(&format!("format {version}")),
                "{args:?}: {all}"
            );
            assert!(all.contains(says), "{args:?}: {all}");
            assert_eq!(llm.calls().len(), calls, "{args:?}");
        }
    }
}

// spec: CFV-14
#[test]
fn scan_reads_a_repository_with_no_cache() {
    let (dir, _llm) = shop();
    std::fs::remove_dir_all(dir.path().join(".retrodoc/cache")).unwrap();

    let output = retrodoc(dir.path(), "scan");

    assert!(output.status.success(), "{}", all_of(&output));
    assert!(all_of(&output).contains("Files:"), "{}", all_of(&output));
}

// spec: CFV-14
#[test]
fn scan_names_a_later_format_before_a_config_it_does_not_know() {
    let (dir, _llm) = shop();
    std::fs::write(format_file(dir.path()), r#"{"version": 99}"#).unwrap();
    let config = dir.path().join("retrodoc.toml");
    let mut text = std::fs::read_to_string(&config).unwrap();
    text.push_str("\n[project]\na_later_key = 1\n");
    std::fs::write(&config, text).unwrap();

    let error = refused(dir.path(), "scan");

    assert!(error.contains("update RetroDoc"), "{error}");
}
