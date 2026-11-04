//! The trends of the audit page, asked of the router without a socket over the snapshots a
//! `generate` saves in `.retrodoc/cache/history/`.

use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use retrodoc_core::model::{ConfidenceScore, Feature};
use retrodoc_mcp::audit::{EntryCoverage, Share};
use retrodoc_mcp::history::{save, DebtCount, Figures, Snapshot, VERSION};
use retrodoc_mcp::GeneratedDocs;
use retrodoc_pipeline::{DomainCluster, DomainMap};
use retrodoc_render::DocLinks;
use retrodoc_web::Project;
use tower::ServiceExt;

/// The docs the interface serves: `billing` and its feature `pay-invoice`.
fn served_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    retrodoc_pipeline::save_features(
        root,
        &[Feature {
            back_office: false,
            technical: false,
            slug: "pay-invoice".into(),
            domain_slug: "billing".into(),
            name: "Pay invoice".into(),
            description: String::new(),
            source_paths: Vec::new(),
            entry_points: Vec::new(),
            confidence: Some(ConfidenceScore::new(0.9, "why".to_string())),
        }],
    )
    .unwrap();
    DomainMap {
        domains: vec![DomainCluster {
            slug: "billing".into(),
            name: "Billing".into(),
            ..DomainCluster::default()
        }],
    }
    .save(root)
    .unwrap();
    dir
}

fn share(count: usize) -> Share {
    Share { count, total: 4 }
}

fn debt(kind: &str, domain: Option<&str>, count: usize) -> DebtCount {
    DebtCount {
        kind: kind.into(),
        domain: domain.map(str::to_string),
        count,
    }
}

fn run(day: u8, commit: Option<&str>, figures: Figures) -> Snapshot {
    Snapshot {
        version: VERSION,
        commit: commit.map(str::to_string),
        finished_at: format!("2026-11-0{day}T10:00:00Z"),
        cost: None,
        figures,
        sections: Vec::new(),
    }
}

/// Three runs, oldest first. Run 2 keeps the confidence at 80% as shown, has no business language
/// nor coverage, and adds debt; run 3 lowers the confidence and brings the debt down.
fn with_history(root: &Path) {
    let runs = [
        run(
            1,
            Some("aaaaaaa1111"),
            Figures {
                confidence: Some(0.804),
                business_language: Some(0.7),
                coverage: Some(EntryCoverage {
                    placed: Some(share(3)),
                    cited: Some(share(2)),
                }),
                debt: vec![debt("low_confidence", Some("billing"), 4)],
            },
        ),
        run(
            2,
            Some("bbbbbbb2222"),
            Figures {
                confidence: Some(0.796),
                business_language: None,
                coverage: None,
                debt: vec![
                    debt("low_confidence", Some("billing"), 6),
                    debt("entry_point_without_feature", None, 1),
                ],
            },
        ),
        run(
            3,
            None,
            Figures {
                confidence: Some(0.6),
                business_language: Some(0.8),
                coverage: Some(EntryCoverage {
                    placed: Some(share(1)),
                    cited: Some(share(3)),
                }),
                debt: vec![
                    debt("low_confidence", Some("billing"), 3),
                    debt("low_confidence", Some("shipping"), 1),
                    debt("entry_point_without_feature", None, 1),
                ],
            },
        ),
    ];
    for snapshot in &runs {
        assert!(save(root, snapshot).unwrap().is_some());
    }
}

async fn audit_in(lang: Option<&'static str>, root: &Path) -> String {
    let project = Project {
        lang,
        ..Project::default()
    };
    let response = retrodoc_web::router(
        project,
        GeneratedDocs::load(root, &[]),
        DocLinks::default(),
        Vec::new(),
    )
    .oneshot(Request::get("/audit").body(Body::empty()).unwrap())
    .await
    .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let page = String::from_utf8(body.to_vec()).unwrap();
    assert_eq!(status, StatusCode::OK, "{page}");
    page
}

async fn audit(root: &Path) -> String {
    audit_in(None, root).await
}

fn between<'a>(text: &'a str, start: &str, end: &str) -> &'a str {
    let from = text
        .find(start)
        .unwrap_or_else(|| panic!("{start} not in {text}"));
    let rest = &text[from..];
    let to = rest.find(end).unwrap_or(rest.len());
    &rest[..to]
}

/// The text of an HTML fragment, its tags and the end of its opening tag left out.
fn text_of(html: &str) -> String {
    let mut text = String::new();
    let mut in_tag = true;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(c),
            _ => {}
        }
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The rows of the table of class `class`, each its header cell then its cells, as text.
fn rows(page: &str, class: &str) -> Vec<Vec<String>> {
    let table = between(page, &format!("<table class=\"{class}\""), "</table>");
    let body = between(table, "<tbody>", "</tbody>");
    body.split("<tr>")
        .skip(1)
        .map(|row| {
            row.split("<t")
                .skip(1)
                .filter(|cell| cell.starts_with('h') || cell.starts_with('d'))
                .map(|cell| text_of(&cell[..cell.find("</t").unwrap_or(cell.len())]))
                .collect()
        })
        .collect()
}

fn headers(page: &str, class: &str) -> Vec<String> {
    let table = between(page, &format!("<table class=\"{class}\""), "</table>");
    let head = between(table, "<thead>", "</thead>");
    head.split("<th")
        .filter(|cell| cell.starts_with(' '))
        .map(|cell| text_of(&cell[..cell.find("</th>").unwrap()]))
        .collect()
}

// spec: AUD-11, AUD-13
#[tokio::test]
async fn the_trends_list_the_figures_of_each_saved_run_newest_first_and_mark_the_worse_ones() {
    let repo = served_repo();
    with_history(repo.path());

    let page = audit(repo.path()).await;

    assert_eq!(
        headers(&page, "trends"),
        [
            "Run",
            "Commit",
            "Confidence",
            "Business language",
            "Placed in a feature",
            "Cited by a use case",
            "Debt items",
        ]
    );
    assert_eq!(
        rows(&page, "trends"),
        [
            [
                "2026-11-03 10:00:00 UTC",
                "no commit",
                "60% ▼ worse",
                "80%",
                "25%",
                "75%",
                "5",
            ],
            [
                "2026-11-02 10:00:00 UTC",
                "bbbbbbb",
                "80%",
                "–",
                "–",
                "–",
                "7 ▼ worse",
            ],
            [
                "2026-11-01 10:00:00 UTC",
                "aaaaaaa",
                "80%",
                "70%",
                "75%",
                "50%",
                "4",
            ],
        ]
    );
}

// spec: AUD-12, AUD-13
#[tokio::test]
async fn the_debt_of_the_trends_sums_each_kind_over_the_domains_with_a_column_per_kind_found() {
    let repo = served_repo();
    with_history(repo.path());

    let page = audit(repo.path()).await;

    assert_eq!(
        headers(&page, "trends-debt"),
        ["Run", "Low confidence", "Entry point without a feature"]
    );
    assert_eq!(
        rows(&page, "trends-debt"),
        [
            ["2026-11-03 10:00:00 UTC", "4", "1"],
            ["2026-11-02 10:00:00 UTC", "6 ▼ worse", "1 ▼ worse"],
            ["2026-11-01 10:00:00 UTC", "4", "0"],
        ]
    );
}

// spec: AUD-15
#[tokio::test]
async fn with_a_single_saved_run_the_trends_say_they_need_two_runs() {
    let repo = served_repo();
    save(
        repo.path(),
        &run(
            1,
            None,
            Figures {
                confidence: Some(0.8),
                business_language: None,
                coverage: None,
                debt: Vec::new(),
            },
        ),
    )
    .unwrap();

    let page = audit(repo.path()).await;

    let trends = between(&page, "<section id=\"audit-trends\"", "</section>");
    assert!(trends.contains("The trends need two runs"), "{trends}");
    assert!(!trends.contains("<table"), "{trends}");
    assert!(page.contains("id=\"audit-heatmap\""), "{page}");
}

// spec: AUD-15, WEB-77
#[tokio::test]
async fn the_trends_are_in_french() {
    let repo = served_repo();
    with_history(repo.path());

    let page = audit_in(Some("fr"), repo.path()).await;

    assert!(page.contains("Tendances"), "{page}");
    assert!(page.contains("▼ pire"), "{page}");
}

/// The chart of `score`: its points' titles and its number of line segments.
fn chart(page: &str, score: &str) -> (Vec<String>, usize) {
    let svg = between(
        page,
        &format!("<svg class=\"trend\" role=\"img\" aria-label=\"{score}"),
        "</svg>",
    );
    let points = svg
        .split("<circle")
        .skip(1)
        .map(|point| text_of(between(point, "<title>", "</title>")))
        .collect();
    (points, svg.matches("<line class=\"trend-line\"").count())
}

// spec: AUD-14
#[tokio::test]
async fn each_score_has_a_line_chart_with_a_point_per_run_that_has_it_and_breaks_across_one_that_lacks_it(
) {
    let repo = served_repo();
    with_history(repo.path());

    let page = audit(repo.path()).await;

    let trends = between(&page, "<section id=\"audit-trends\"", "</section>");
    assert!(
        trends.find("<svg").unwrap() < trends.find("<table").unwrap(),
        "{trends}"
    );
    assert_eq!(
        chart(&page, "Confidence"),
        (
            vec![
                "2026-11-01 10:00:00 UTC: 80%".to_string(),
                "2026-11-02 10:00:00 UTC: 80%".to_string(),
                "2026-11-03 10:00:00 UTC: 60%".to_string(),
            ],
            2
        )
    );
    assert_eq!(
        chart(&page, "Business language"),
        (
            vec![
                "2026-11-01 10:00:00 UTC: 70%".to_string(),
                "2026-11-03 10:00:00 UTC: 80%".to_string(),
            ],
            0
        )
    );
    assert_eq!(chart(&page, "Cited by a use case").0.len(), 2);
}

// spec: AUD-14
#[tokio::test]
async fn a_score_no_saved_run_has_gets_no_chart() {
    let repo = served_repo();
    for (day, confidence) in [(1, 0.8), (2, 0.7)] {
        let figures = Figures {
            confidence: Some(confidence),
            business_language: None,
            coverage: None,
            debt: Vec::new(),
        };
        save(repo.path(), &run(day, None, figures)).unwrap();
    }

    let page = audit(repo.path()).await;

    assert_eq!(page.matches("<svg class=\"trend\"").count(), 1, "{page}");
    assert!(page.contains("aria-label=\"Confidence"), "{page}");
}
