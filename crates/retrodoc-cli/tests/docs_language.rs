//! `[output] language`: the writing passes are asked in it, the reading passes are not
//! (`specs/docs-language.md`).

mod support;

use support::{generate, llm_section, repo_with_config, retrodoc, warnings, App, FakeLlm};

// spec: DLG-2, DLG-3
#[test]
fn the_brief_is_asked_in_the_language_and_the_sources_are_not() {
    let llm = FakeLlm::start(|_| "not json".to_string());
    let dir = repo_with_config(
        &[
            ("README.md", "# Shop\n\nBuyers pay for their orders.\n"),
            ("app.rb", "class Order; end\n"),
        ],
        &format!("{}\n[output]\nlanguage = \"fr\"\n", llm_section(&llm)),
    );

    let output = retrodoc(dir.path(), "brief");
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

/// The generated pages under `docs/functional`, path and content, sorted by path.
fn functional_pages(dir: &std::path::Path) -> Vec<(String, String)> {
    let root = dir.join("docs/functional");
    let mut pages = Vec::new();
    let mut todo = vec![root.clone()];
    while let Some(dir) = todo.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                todo.push(path);
            } else {
                let relative = path.strip_prefix(&root).unwrap().display().to_string();
                pages.push((relative, std::fs::read_to_string(&path).unwrap()));
            }
        }
    }
    pages.sort();
    pages
}

/// The labels of the English render no French page must carry.
const ENGLISH_LABELS: &[&str] = &[
    "## Use cases",
    "## Source files",
    "**Confidence:**",
    "## Domains",
    "[← Domain overview]",
];

// spec: DLG-7, DLG-8
#[test]
fn the_fixed_text_of_the_docs_is_french_and_their_paths_are_not() {
    let (english, _llm) = shop_with_unrelated_readme(None);
    let (french, _llm_fr) = shop_with_unrelated_readme(Some("fr"));
    assert!(generate(english.path()).status.success());
    assert!(generate(french.path()).status.success());

    let (english, french) = (
        functional_pages(english.path()),
        functional_pages(french.path()),
    );

    let paths = |pages: &[(String, String)]| -> Vec<String> {
        pages.iter().map(|(p, _)| p.clone()).collect()
    };
    assert_eq!(paths(&english), paths(&french));
    let all =
        |pages: &[(String, String)]| -> String { pages.iter().map(|(_, c)| c.as_str()).collect() };
    let (english, french) = (all(&english), all(&french));
    for label in ENGLISH_LABELS {
        assert!(english.contains(label), "{label} missing from {english}");
        assert!(!french.contains(label), "{label} left in {french}");
    }
    assert!(french.contains("## Cas d'usage"), "{french}");
    assert!(french.contains("**Confiance :**"), "{french}");
}

// spec: DLG-7, DLG-9
#[test]
fn a_native_name_or_a_regional_tag_of_french_gives_the_french_text() {
    for language in ["Français", "fr_FR"] {
        let (dir, _llm) = shop_with_unrelated_readme(Some(language));
        assert!(generate(dir.path()).status.success());

        let pages: String = functional_pages(dir.path())
            .into_iter()
            .map(|(_, c)| c)
            .collect();

        assert!(pages.contains("## Cas d'usage"), "{language}: {pages}");
        assert!(!pages.contains("## Use cases"), "{language}: {pages}");
    }
}

// spec: DLG-7
#[test]
fn a_language_with_no_table_keeps_the_english_text() {
    let (dir, _llm) = shop_with_unrelated_readme(Some("German"));
    assert!(generate(dir.path()).status.success());

    let pages: String = functional_pages(dir.path())
        .into_iter()
        .map(|(_, c)| c)
        .collect();

    assert!(pages.contains("## Use cases"), "{pages}");
}

// spec: DLG-8
#[test]
fn a_second_render_in_french_writes_nothing() {
    let (dir, _llm) = shop_with_unrelated_readme(Some("fr"));
    assert!(generate(dir.path()).status.success());

    let stdout = support::stdout_of(dir.path(), "render");

    assert!(
        stdout.contains("Docs are up to date, nothing to write."),
        "{stdout}"
    );
}
