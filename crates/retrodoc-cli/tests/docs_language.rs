//! `[output] language`: the writing passes are asked in it, the reading passes are not
//! (`specs/docs-language.md`).

mod support;

use std::process::Command;

use support::FakeLlm;

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
