//! `[output] language`: the writing passes are asked in it, the reading passes are not
//! (`specs/docs-language.md`).

mod support;

use std::process::Command;

use support::{generate, warnings, App, FakeLlm};

// spec: DLG-2, DLG-3
#[test]
fn the_brief_is_asked_in_the_language_and_the_sources_are_not() {
    let llm = FakeLlm::start(|_| "not json".to_string());
    let dir = support::repo(
        &[
            ("README.md", "# Shop\n\nBuyers pay for their orders.\n"),
            ("app.rb", "class Order; end\n"),
        ],
        &llm,
    );
    let config = dir.path().join("retrodoc.toml");
    let raw = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, format!("{raw}\n[output]\nlanguage = \"fr\"\n")).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_retrodoc"))
        .args(["brief", "--path"])
        .arg(dir.path())
        .env("RETRODOC_TEST_FAKE_LLM_KEY", "unused")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let instruction = "in French. Keep exactly as given";
    let (brief, sources) = (llm.calls_of("brief"), llm.calls_of("sources"));
    assert!(!brief.is_empty(), "{stdout}");
    assert!(!sources.is_empty(), "{stdout}");
    assert!(
        brief.iter().all(|c| c.system.contains(instruction)),
        "{brief:?}"
    );
    assert!(
        sources.iter().all(|c| !c.system.contains(instruction)),
        "{sources:?}"
    );
}

/// A shop whose README shares no word with what is generated about it, the docs language set to
/// `language`.
fn shop_with_unrelated_readme(language: Option<&str>) -> (tempfile::TempDir, FakeLlm) {
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
            ("README.md", "# Zebra\n\nStriped animals graze.\n"),
            (
                "app/orders_controller.rb",
                "get '/orders' do\n  Order.all\nend\n",
            ),
            ("app/order.rb", "class Order\nend\n"),
        ],
        &llm,
    );
    if let Some(language) = language {
        let config = dir.path().join("retrodoc.toml");
        let raw = std::fs::read_to_string(&config).unwrap();
        std::fs::write(
            &config,
            format!("{raw}\n[output]\nlanguage = \"{language}\"\n"),
        )
        .unwrap();
    }
    (dir, llm)
}

// spec: REC-16
#[test]
fn generate_warns_when_the_docs_language_is_set_and_no_doc_section_matched() {
    let (dir, _llm) = shop_with_unrelated_readme(Some("fr"));

    let output = generate(dir.path());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(
        warnings(&output)
            .iter()
            .any(|w| w.contains("no section of the project docs matched") && w.contains("French")),
        "{stdout}"
    );
}

// spec: REC-16
#[test]
fn generate_does_not_blame_the_language_when_none_is_set() {
    let (dir, _llm) = shop_with_unrelated_readme(None);

    let output = generate(dir.path());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(
        !stdout.contains("no section of the project docs matched"),
        "{stdout}"
    );
}
