//! The pages of the web interface, asked of the router without a socket over
//! the artifacts a run leaves in `.retrodoc/cache/`.

use std::collections::BTreeMap;
use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use retrodoc_core::model::{ConfidenceScore, Feature, UseCase};
use retrodoc_mcp::GeneratedDocs;
use retrodoc_pipeline::cache::hash_content;
use retrodoc_pipeline::{DomainCluster, DomainMap, SourceHashes};
use tower::ServiceExt;

fn feature(slug: &str, domain: &str, confidence: f32, path: &str) -> Feature {
    Feature {
        back_office: false,
        slug: slug.into(),
        domain_slug: domain.into(),
        name: format!("Feature {slug}"),
        description: String::new(),
        source_paths: vec![path.into()],
        entry_points: Vec::new(),
        confidence: Some(ConfidenceScore::new(confidence, None)),
    }
}

fn use_case(slug: &str, feature: &str) -> UseCase {
    UseCase {
        slug: slug.into(),
        feature_slug: feature.into(),
        name: format!("Use case {slug}"),
        description: String::new(),
        steps: Vec::new(),
        entry_points: Vec::new(),
        primary_actor: None,
        narrative: None,
        business_language: None,
        diagram_mermaid: None,
        confidence: None,
        narrative_check: None,
    }
}

fn cluster(slug: &str, name: &str, description: &str) -> DomainCluster {
    DomainCluster {
        slug: slug.into(),
        name: name.into(),
        description: description.into(),
        ..DomainCluster::default()
    }
}

/// A run over three domains: `billing` (two features, three use cases, 80%),
/// `delivery` (40%, its cited file edited since the run) and `archive` (no
/// feature, no score).
fn generated_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("app")).unwrap();
    let mut hashes = SourceHashes::default();
    for name in ["app/invoice.rb", "app/plan.rb", "app/rider.rb"] {
        let content = format!("content of {name}");
        std::fs::write(root.join(name), &content).unwrap();
        hashes.put(name, &hash_content(&content));
    }
    hashes.save(root).unwrap();
    std::fs::write(root.join("app/rider.rb"), "edited since the run").unwrap();
    retrodoc_pipeline::save_features(
        root,
        &[
            feature("pay-invoice", "billing", 0.8, "app/invoice.rb"),
            feature("cancel-plan", "billing", 0.8, "app/plan.rb"),
            feature("assign-rider", "delivery", 0.4, "app/rider.rb"),
        ],
    )
    .unwrap();
    retrodoc_pipeline::save_use_cases(
        root,
        &[
            use_case("pay-by-card", "pay-invoice"),
            use_case("pay-late", "pay-invoice"),
            use_case("cancel-now", "cancel-plan"),
        ],
    )
    .unwrap();
    DomainMap {
        domains: vec![
            cluster("billing", "Billing", "Invoices and payments"),
            cluster("delivery", "Delivery", "Orders brought to customers"),
            cluster("archive", "Archive", "Old records"),
        ],
    }
    .save(root)
    .unwrap();
    dir
}

fn load(root: &Path) -> Option<GeneratedDocs> {
    GeneratedDocs::load(root, &[])
}

async fn get(docs: Option<GeneratedDocs>, path: &str) -> (StatusCode, String, Option<String>) {
    let response = retrodoc_web::router(docs)
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let content_type = response
        .headers()
        .get("content-type")
        .map(|v| v.to_str().unwrap().to_string());
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (
        status,
        String::from_utf8(body.to_vec()).unwrap(),
        content_type,
    )
}

async fn home(root: &Path) -> String {
    let (status, body, _) = get(load(root), "/").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

/// The row of the domain `slug` in the list of the home page.
fn row<'a>(page: &'a str, slug: &str) -> &'a str {
    let start = page
        .find(&format!("<tr id=\"domain-{slug}\""))
        .unwrap_or_else(|| panic!("no row for {slug} in {page}"));
    let end = start + page[start..].find("</tr>").unwrap();
    &page[start..end]
}

/// The text of the cells of a row, tags removed.
fn cells(row: &str) -> Vec<String> {
    row.split("<td")
        .skip(1)
        .map(|cell| {
            let mut text = String::new();
            let mut in_tag = true;
            for c in cell.split("</td>").next().unwrap().chars() {
                match c {
                    '<' => in_tag = true,
                    '>' => in_tag = false,
                    c if !in_tag => text.push(c),
                    _ => {}
                }
            }
            text.split_whitespace().collect::<Vec<_>>().join(" ")
        })
        .collect()
}

// spec: WEB-4
#[tokio::test]
async fn the_home_page_lists_every_domain_with_its_confidence_and_counts() {
    let repo = generated_repo();
    let page = home(repo.path()).await;
    assert_eq!(
        cells(row(&page, "billing")),
        ["Billing", "Invoices and payments", "80%", "2", "3"]
    );
    assert_eq!(
        cells(row(&page, "archive")),
        ["Archive", "Old records", "not scored", "0", "0"]
    );
    assert!(row(&page, "billing").contains("href=\"/domains/billing\""));
}

// spec: WEB-5
#[tokio::test]
async fn every_page_shows_the_tree_of_domains_features_and_use_cases() {
    let repo = generated_repo();
    for path in ["/", "/unknown"] {
        let (_, page, _) = get(load(repo.path()), path).await;
        let tree = &page[page.find("<nav class=\"tree\"").expect(&page)..];
        for link in [
            "href=\"/domains/billing\"",
            "href=\"/domains/billing/pay-invoice\"",
            "href=\"/domains/billing/pay-invoice/pay-by-card\"",
            "href=\"/domains/billing/cancel-plan/cancel-now\"",
            "href=\"/domains/delivery/assign-rider\"",
            "href=\"/domains/archive\"",
        ] {
            assert!(tree.contains(link), "{link} missing on {path}: {tree}");
        }
    }
}

// spec: WEB-6
#[tokio::test]
async fn a_confidence_under_the_threshold_is_flagged_low() {
    let repo = generated_repo();
    let page = home(repo.path()).await;
    assert!(row(&page, "delivery").contains("class=\"badge low\""));
    assert!(!row(&page, "billing").contains("badge low"));
    assert!(!row(&page, "archive").contains("badge low"));
}

// spec: WEB-7
#[tokio::test]
async fn a_domain_citing_a_file_edited_since_the_run_is_flagged_stale() {
    let repo = generated_repo();
    std::fs::remove_file(repo.path().join("app/plan.rb")).unwrap();
    let page = home(repo.path()).await;
    assert!(row(&page, "delivery").contains("class=\"badge stale\""));
    assert!(row(&page, "billing").contains("class=\"badge stale\""));
    assert!(!row(&page, "archive").contains("badge stale"));
}

// spec: WEB-7
#[tokio::test]
async fn a_cited_file_with_no_recorded_hash_is_not_judged() {
    let repo = generated_repo();
    SourceHashes::default().save(repo.path()).unwrap();
    let page = home(repo.path()).await;
    assert!(!page.contains("badge stale"), "{page}");
}

// spec: WEB-8
#[tokio::test]
async fn a_path_not_served_is_the_not_found_page() {
    let repo = generated_repo();
    for path in ["/unknown", "/domains/billing/pay-invoice"] {
        let (status, page, _) = get(load(repo.path()), path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert!(page.contains("<h1>Not found</h1>"), "{page}");
    }
}

// spec: WEB-9
#[tokio::test]
async fn a_repository_with_no_run_gets_a_page_saying_to_run_generate() {
    let dir = tempfile::tempdir().unwrap();
    let docs = load(dir.path());
    assert!(docs.is_none());
    let (status, page, _) = get(docs, "/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("<code>retrodoc generate</code>"), "{page}");
}

// spec: WEB-10
#[tokio::test]
async fn the_text_of_the_docs_is_escaped() {
    let repo = generated_repo();
    DomainMap {
        domains: vec![cluster(
            "billing",
            "<script>alert(1)</script>",
            "<img src=x onerror=alert(2)>",
        )],
    }
    .save(repo.path())
    .unwrap();
    let page = home(repo.path()).await;
    assert!(!page.contains("<script>alert"), "{page}");
    assert!(!page.contains("<img"), "{page}");
    assert!(
        page.contains("&#60;script&#62;alert(1)&#60;/script&#62;"),
        "{page}"
    );
}

// spec: WEB-11
#[tokio::test]
async fn the_scripts_and_stylesheet_come_from_the_interface_itself() {
    let repo = generated_repo();
    let page = home(repo.path()).await;
    let linked: Vec<&str> = page
        .split(['"', '\''])
        .skip(1)
        .step_by(2)
        .filter(|v| v.contains(".js") || v.contains(".css") || v.contains("//"))
        .collect();
    assert_eq!(linked.len(), 2, "{linked:?}");
    for link in &linked {
        assert!(link.starts_with("/assets/"), "{link} in {page}");
        let (status, body, content_type) = get(load(repo.path()), link).await;
        assert_eq!(status, StatusCode::OK, "{link}");
        assert!(body.len() > 100, "{link}");
        let content_type = content_type.unwrap();
        assert!(
            content_type.starts_with("text/css") || content_type.starts_with("text/javascript"),
            "{content_type}"
        );
    }
}

fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(dir: &Path, root: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, root, out);
            } else {
                let name = path.strip_prefix(root).unwrap().display().to_string();
                out.insert(name, std::fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

// spec: WEB-12
#[tokio::test]
async fn serving_the_pages_writes_nothing_in_the_repository() {
    let repo = generated_repo();
    let before = snapshot(repo.path());
    for path in ["/", "/unknown", "/assets/style.css"] {
        get(load(repo.path()), path).await;
    }
    assert_eq!(snapshot(repo.path()), before);
}

async fn status_with_host(host: &str) -> StatusCode {
    let repo = generated_repo();
    retrodoc_web::router(load(repo.path()))
        .oneshot(
            Request::get("/")
                .header("host", host)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}

// spec: WEB-13
#[tokio::test]
async fn a_host_that_is_neither_localhost_nor_an_address_is_refused() {
    for host in [
        "evil.example:8730",
        "evil.example",
        "localhost.evil.example",
    ] {
        assert_eq!(
            status_with_host(host).await,
            StatusCode::FORBIDDEN,
            "{host}"
        );
    }
    for host in [
        "localhost:8730",
        "LOCALHOST",
        "127.0.0.1:8730",
        "[::1]:8730",
        "192.168.1.10:8730",
    ] {
        assert_eq!(status_with_host(host).await, StatusCode::OK, "{host}");
    }
}
