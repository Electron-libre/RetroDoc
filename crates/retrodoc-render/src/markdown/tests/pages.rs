use std::path::Path;

use retrodoc_core::model::{Actor, ActorKind, ConfidenceScore, Step};

use super::super::{render, segment, step_item};
use super::{meta, page, sample};

#[test]
fn a_step_with_one_text_shows_it_after_its_actor() {
    let step = Step {
        order: 2,
        description: "Saves the invoice.".to_string(),
        actor: Actor {
            name: "System".to_string(),
            kind: ActorKind::System,
        },
        action: String::new(),
        source_refs: Vec::new(),
    };
    assert_eq!(
        step_item(&step),
        "2. **System** (system) — Saves the invoice.\n"
    );
}

#[test]
fn technical_use_cases_are_set_apart_on_the_feature_page() {
    let (d, f, mut u) = sample();
    let mut business = u[0].clone();
    business.slug = "pay-by-wire".to_string();
    business.name = "Pay by wire".to_string();
    business.confidence = Some(ConfidenceScore::new(0.9, None));
    u.push(business);
    let files = render(&d, &f, &u, "", &meta()).unwrap();
    let feature = page(&files, "functional/billing/pay-invoice.md");
    let (core, apart) = feature
        .split_once("## Administration and technical")
        .expect("a section for what is set apart");
    assert!(core.contains("Pay by wire") && !core.contains("Pay by card"));
    assert!(apart.contains("Pay by card") && !apart.contains("Pay by wire"));
}

#[test]
fn a_page_without_technical_use_cases_has_no_such_section() {
    let (d, f, mut u) = sample();
    u[0].confidence = Some(ConfidenceScore::new(0.9, None));
    let files = render(&d, &f, &u, "", &meta()).unwrap();
    for file in &files {
        assert!(!file.content.contains("Administration and technical"));
    }
}

#[test]
fn a_back_office_feature_is_listed_apart_in_its_domain_and_the_index() {
    let (d, mut f, mut u) = sample();
    f[0].back_office = true;
    u[0].confidence = Some(ConfidenceScore::new(0.9, None));
    let files = render(&d, &f, &u, "", &meta()).unwrap();
    let readme = page(&files, "functional/billing/README.md");
    let (core, apart) = readme
        .split_once("## Administration and technical")
        .expect("a section for the back-office features");
    assert!(!core.contains("pay-invoice.md") && apart.contains("pay-invoice.md"));
    let index = page(&files, "functional/llms.txt");
    let (core, apart) = index
        .split_once("  - Administration and technical:")
        .expect("a group for the back-office features");
    assert!(!core.contains("pay-invoice.md") && apart.contains("pay-invoice.md"));
    // The use case of a back-office feature is set apart even with a good score.
    let feature = page(&files, "functional/billing/pay-invoice.md");
    assert!(feature.contains("All of them are set apart below."));
}

#[test]
fn renders_the_planned_layout_with_working_links() {
    let (d, f, u) = sample();
    let files = render(&d, &f, &u, "# report\n", &meta()).unwrap();
    let paths: Vec<String> = files.iter().map(|f| f.path.display().to_string()).collect();
    assert_eq!(
        paths,
        [
            "functional/billing/README.md",
            "functional/billing/pay-invoice.md",
            "functional/billing/use-cases/pay-invoice/pay-by-card.md",
            "functional/billing/list.md",
            "functional/llms.txt",
            "_retrodoc/coverage-report.md",
            "_retrodoc/run-metadata.json",
        ]
    );
    let content = |p: &str| {
        &files
            .iter()
            .find(|f| f.path == Path::new(p))
            .unwrap()
            .content
    };

    let readme = content("functional/billing/README.md");
    assert!(readme.contains("](pay-invoice.md)"));
    assert!(readme.contains("](list.md)"));

    let feature = content("functional/billing/pay-invoice.md");
    assert!(feature.contains("](README.md)"));
    assert!(feature.contains("](use-cases/pay-invoice/pay-by-card.md)"));
    assert!(feature.contains("⚠️ 30% — weak"));

    let use_case = content("functional/billing/use-cases/pay-invoice/pay-by-card.md");
    assert!(use_case.contains("](../../pay-invoice.md)"));
    assert!(use_case.contains("1. **Customer** (human) — submit. Submit the form"));
    assert!(use_case.contains("`src/pay.rs:3-9`"));
    assert!(use_case.contains("```mermaid\nsequenceDiagram"));
}

#[test]
fn a_narrative_comes_first_and_folds_the_technical_steps() {
    let (d, f, mut u) = sample();
    u[0].narrative = Some("A customer pays an invoice by card.".to_string());
    let files = render(&d, &f, &u, "r", &meta()).unwrap();
    let page = &files
        .iter()
        .find(|f| f.path.ends_with("pay-by-card.md"))
        .unwrap()
        .content;

    let narrative = page.find("## What happens").unwrap();
    let details = page.find("<details>").unwrap();
    let step = page.find("1. **Customer**").unwrap();
    assert!(narrative < details && details < step);
    assert!(page.contains("A customer pays an invoice by card."));
    assert!(page.contains("<summary>Technical details (1 step)</summary>"));
    assert!(page.contains("### Steps") && page.contains("### Diagram"));
    assert!(page.trim_end().ends_with("</details>"));
    // The business-language score is shown next to the confidence, flagged when low.
    assert!(!page.contains("Business language"));
    u[0].business_language = Some(ConfidenceScore::new(
        0.3,
        "names no business entity".to_string(),
    ));
    let files = render(&d, &f, &u, "r", &meta()).unwrap();
    let scored = &files
        .iter()
        .find(|f| f.path.ends_with("pay-by-card.md"))
        .unwrap()
        .content;
    assert!(scored.contains("**Business language:** ⚠️ 30% — names no business entity"));
    // Without a narrative the steps stay in the open, unfolded.
    let (d, f, u) = sample();
    let files = render(&d, &f, &u, "r", &meta()).unwrap();
    let page = &files
        .iter()
        .find(|f| f.path.ends_with("pay-by-card.md"))
        .unwrap()
        .content;
    assert!(page.contains("## Steps") && !page.contains("<details>"));
}

#[test]
fn hostile_slugs_cannot_escape_the_docs_dir() {
    assert_eq!(segment("../../etc"), "etc");
    assert_eq!(segment("A b/c"), "a-b-c");
    assert_eq!(segment(".."), "unnamed");
}
