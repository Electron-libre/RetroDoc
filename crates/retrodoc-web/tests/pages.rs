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
use retrodoc_render::DocLinks;
use retrodoc_web::Project;
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

/// The application the pages of these tests document.
fn shop() -> Project {
    Project {
        name: "Shop and Co".into(),
        purpose: Some("Sells <b>goods</b> online".into()),
        users: vec!["Buyers who order goods".into(), "Clerks".into()],
    }
}

async fn get(docs: Option<GeneratedDocs>, path: &str) -> (StatusCode, String, Option<String>) {
    get_project(shop(), docs, path).await
}

async fn get_project(
    project: Project,
    docs: Option<GeneratedDocs>,
    path: &str,
) -> (StatusCode, String, Option<String>) {
    let response = retrodoc_web::router(project, docs, DocLinks::default(), Vec::new())
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

/// The card of the domain `slug` on the home page.
fn row<'a>(page: &'a str, slug: &str) -> &'a str {
    let id = page
        .find(&format!("id=\"domain-{slug}\""))
        .unwrap_or_else(|| panic!("no card for {slug} in {page}"));
    let start = page[..id].rfind('<').unwrap();
    let end = start + page[start..].find("</article>").unwrap();
    &page[start..end]
}

/// The text of `html`, tags removed and spaces collapsed.
fn text(html: &str) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                text.push(' ');
            }
            c if !in_tag => text.push(c),
            _ => {}
        }
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The part of `page` from `from` to the next `to`.
fn between<'a>(page: &'a str, from: &str, to: &str) -> &'a str {
    let start = page
        .find(from)
        .unwrap_or_else(|| panic!("no {from} in {page}"));
    let end = start + page[start..].find(to).unwrap();
    &page[start..end]
}

// spec: WEB-4
#[tokio::test]
async fn the_home_page_lists_every_domain_with_its_confidence_and_counts() {
    let repo = generated_repo();
    let page = home(repo.path()).await;
    assert_eq!(
        text(row(&page, "billing")),
        "Billing Invoices and payments 80% 2 features 3 use cases"
    );
    assert_eq!(
        text(row(&page, "archive")),
        "Archive Old records not scored 0 features 0 use cases"
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
    for path in ["/unknown", "/domains/billing/pay-invoice/pay-by-card/more"] {
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

/// The text of the `<title>` of `page`.
fn title(page: &str) -> &str {
    let start = page.find("<title>").unwrap() + "<title>".len();
    &page[start..start + page[start..].find("</title>").unwrap()]
}

// spec: WEB-63
#[tokio::test]
async fn every_page_names_the_application_in_its_header_and_title() {
    let repo = generated_repo();
    let empty = tempfile::tempdir().unwrap();
    for (docs, path) in [
        (load(repo.path()), "/"),
        (load(repo.path()), "/domains/billing"),
        (load(repo.path()), "/glossary"),
        (load(repo.path()), "/no-such-page"),
        (load(empty.path()), "/"),
    ] {
        let (_, page, _) = get(docs, path).await;
        assert!(
            title(&page).ends_with(" · Shop and Co"),
            "{path}: {}",
            title(&page)
        );
        let header = &page[page.find("<header").unwrap()..page.find("</header>").unwrap()];
        assert!(header.contains("Shop and Co"), "{path}: {header}");
    }
}

// spec: WEB-64
#[tokio::test]
async fn the_home_page_is_headed_by_the_application_and_its_purpose() {
    let repo = generated_repo();
    let page = home(repo.path()).await;
    let main = &page[page.find("<main>").unwrap()..];
    assert!(main.contains("<h1>Shop and Co</h1>"), "{main}");
    assert!(main.contains("class=\"purpose\">Sells "), "{main}");
    assert!(!main.contains("<b>goods"), "{main}");
    assert!(row(&page, "billing").contains("Billing"));
}

// spec: WEB-64
#[tokio::test]
async fn a_brief_with_no_purpose_leaves_the_home_page_with_the_name_and_domains() {
    let repo = generated_repo();
    let project = Project {
        name: "Shop".into(),
        ..Project::default()
    };
    let (_, page, _) = get_project(project, load(repo.path()), "/").await;
    let main = &page[page.find("<main>").unwrap()..];
    assert!(main.contains("<h1>Shop</h1>"), "{main}");
    assert!(!main.contains("class=\"purpose\""), "{main}");
    assert!(row(&page, "billing").contains("Billing"));
}

// spec: WEB-65
#[tokio::test]
async fn the_home_page_shows_the_brief_then_where_to_start_then_the_domains() {
    let repo = generated_repo();
    let page = home(repo.path()).await;
    let main = &page[page.find("<main>").unwrap()..];
    let order: Vec<usize> = [
        "<h1>Shop and Co</h1>",
        "class=\"purpose\"",
        "Buyers who order goods",
        "Start here",
        "id=\"domain-billing\"",
    ]
    .iter()
    .map(|part| {
        main.find(part)
            .unwrap_or_else(|| panic!("no {part} in {main}"))
    })
    .collect();
    assert!(order.is_sorted(), "{order:?}: {main}");
    assert!(main.contains("Clerks"), "{main}");
}

// spec: WEB-65
#[tokio::test]
async fn a_brief_with_no_user_leaves_no_users_line() {
    let repo = generated_repo();
    let project = Project {
        name: "Shop".into(),
        users: vec!["  ".into()],
        ..Project::default()
    };
    let (_, page, _) = get_project(project, load(repo.path()), "/").await;
    assert!(!page.contains("class=\"users\""), "{page}");
}

// spec: WEB-66
#[tokio::test]
async fn start_here_links_each_reference_page_with_what_it_holds_and_its_count() {
    let repo = generated_repo();
    let page = home(repo.path()).await;
    let start = between(&page, "id=\"start-here\"", "</section>");
    for (href, count) in [
        ("/actors", "0 actors"),
        ("/glossary", "0 concepts"),
        ("/rules", "0 rules"),
        ("/files", "3 cited files"),
    ] {
        let item = between(start, &format!("href=\"{href}\""), "</li>");
        assert!(text(item).contains(count), "{href}: {}", text(item));
        let hint = between(item, "class=\"hint\">", "</");
        assert!(
            hint.len() > "class=\"hint\">".len(),
            "no hint for {href}: {item}"
        );
    }
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
    retrodoc_web::router(shop(), load(repo.path()), DocLinks::default(), Vec::new())
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
