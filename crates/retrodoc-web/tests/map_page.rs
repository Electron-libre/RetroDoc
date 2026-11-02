//! The maps of the docs, asked of the router without a socket over the
//! artifacts a run leaves in `.retrodoc/cache/`.

use std::path::{Path, PathBuf};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use retrodoc_core::model::{Actor, ActorKind, Feature, SourceRef, Step, UseCase};
use retrodoc_mcp::GeneratedDocs;
use retrodoc_pipeline::glossary::ModelFile;
use retrodoc_pipeline::{DomainCluster, DomainMap, Entity, Glossary};
use retrodoc_render::DocLinks;
use tower::ServiceExt;

fn feature(slug: &str, domain: &str) -> Feature {
    Feature {
        back_office: false,
        slug: slug.into(),
        domain_slug: domain.into(),
        name: format!("Feature {slug}"),
        description: String::new(),
        source_paths: Vec::new(),
        entry_points: Vec::new(),
        confidence: None,
    }
}

/// A use case of `feature` with one step per cited file, each of `actor`.
fn use_case(slug: &str, feature: &str, cited: &[&str]) -> UseCase {
    UseCase {
        slug: slug.into(),
        feature_slug: feature.into(),
        name: format!("Use case {slug}"),
        description: String::new(),
        steps: cited
            .iter()
            .enumerate()
            .map(|(order, path)| Step {
                order: u32::try_from(order).unwrap() + 1,
                description: String::new(),
                actor: Actor {
                    name: "System".into(),
                    kind: ActorKind::System,
                },
                action: String::new(),
                source_refs: vec![SourceRef {
                    path: (*path).into(),
                    start_line: Some(1),
                    end_line: None,
                }],
            })
            .collect(),
        entry_points: Vec::new(),
        primary_actor: None,
        narrative: None,
        business_language: None,
        diagram_mermaid: None,
        confidence: None,
        narrative_check: None,
    }
}

fn cluster(slug: &str, name: &str) -> DomainCluster {
    DomainCluster {
        slug: slug.into(),
        name: name.into(),
        description: String::new(),
        ..DomainCluster::default()
    }
}

fn concept(name: &str) -> ModelFile {
    ModelFile {
        content_hash: String::new(),
        entities: vec![Entity {
            name: name.into(),
            description: String::new(),
            attributes: Vec::new(),
            associations: Vec::new(),
        }],
    }
}

/// A run over three domains: `billing` (two features, three use cases) and
/// `delivery` (one feature, one use case) share the concepts `Address`,
/// `Card`, `customer`, `Invoice` and `Zone`; `billing` alone uses `Plan`;
/// `archive` has no feature; nothing uses `Unused`.
fn generated_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    retrodoc_pipeline::save_features(
        root,
        &[
            feature("pay-invoice", "billing"),
            feature("cancel-plan", "billing"),
            feature("assign-rider", "delivery"),
        ],
    )
    .unwrap();
    let shared = ["app/address.rb", "app/card.rb", "app/customer.rb"];
    retrodoc_pipeline::save_use_cases(
        root,
        &[
            use_case("pay-by-card", "pay-invoice", &shared),
            use_case(
                "pay-late",
                "pay-invoice",
                &["app/invoice.rb", "app/zone.rb"],
            ),
            use_case("cancel-now", "cancel-plan", &["app/plan.rb"]),
            use_case(
                "assign",
                "assign-rider",
                &[
                    shared[0],
                    shared[1],
                    shared[2],
                    "app/invoice.rb",
                    "app/zone.rb",
                ],
            ),
        ],
    )
    .unwrap();
    save_glossary(
        root,
        &[
            ("app/address.rb", "Address"),
            ("app/card.rb", "Card"),
            ("app/customer.rb", "customer"),
            ("app/invoice.rb", "Invoice"),
            ("app/zone.rb", "Zone"),
            ("app/plan.rb", "Plan"),
            ("app/unused.rb", "Unused"),
        ],
    );
    save_domains(
        root,
        vec![
            cluster("billing", "Billing"),
            cluster("delivery", "Delivery"),
            cluster("archive", "Archive"),
        ],
    );
    dir
}

fn save_glossary(root: &Path, concepts: &[(&str, &str)]) {
    Glossary {
        models: concepts
            .iter()
            .map(|(path, name)| (PathBuf::from(path), concept(name)))
            .collect(),
        tests: Vec::new(),
    }
    .save(root)
    .unwrap();
}

fn save_domains(root: &Path, domains: Vec<DomainCluster>) {
    DomainMap { domains }.save(root).unwrap();
}

async fn get(root: &Path, path: &str) -> (StatusCode, String) {
    let response = retrodoc_web::router(
        retrodoc_web::Project::default(),
        GeneratedDocs::load(root, &[]),
        DocLinks::default(),
        Vec::new(),
    )
    .oneshot(Request::get(path).body(Body::empty()).unwrap())
    .await
    .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

async fn page(root: &Path, path: &str) -> String {
    let (status, page) = get(root, path).await;
    assert_eq!(status, StatusCode::OK, "{path}: {page}");
    page
}

/// The source of the graph of `page`, as `mermaid` reads it.
fn source(page: &str) -> String {
    let start = page.find("<pre class=\"mermaid").expect("a graph");
    let pre = &page[start..];
    let pre = &pre[pre.find('>').unwrap() + 1..pre.find("</pre>").unwrap()];
    unescape(pre)
}

fn unescape(html: &str) -> String {
    html.replace("&#34;", "\"")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#60;", "<")
        .replace("&lt;", "<")
        .replace("&#62;", ">")
        .replace("&gt;", ">")
        .replace("&#38;", "&")
        .replace("&amp;", "&")
}

/// The list of the page beside its graph.
fn list(page: &str) -> &str {
    let start = page.find("class=\"map-links\"").expect("a list");
    let rest = &page[start..];
    &rest[..rest.find("</section>").unwrap()]
}

/// The `data-node` of each link of `html`, in order.
fn data_nodes(html: &str) -> Vec<String> {
    html.split("data-node=\"")
        .skip(1)
        .map(|rest| rest[..rest.find('"').unwrap()].to_string())
        .collect()
}

/// The ids of the nodes the source declares: `id["label"]`.
fn node_ids(source: &str) -> Vec<String> {
    source
        .lines()
        .map(str::trim)
        .filter_map(|line| line.split_once("[\""))
        .map(|(id, _)| id.to_string())
        .collect()
}

// spec: WEB-70
#[tokio::test]
async fn the_map_draws_a_node_per_domain_with_its_counts() {
    let repo = generated_repo();
    let source = source(&page(repo.path(), "/map").await);
    assert!(source.starts_with("flowchart"), "{source}");
    for label in [
        "\"Billing<br>2 features, 3 use cases\"",
        "\"Delivery<br>1 feature, 1 use case\"",
        "\"Archive<br>0 features, 0 use cases\"",
    ] {
        assert!(source.contains(label), "no {label} in {source}");
    }
}

// spec: WEB-70
#[tokio::test]
async fn two_domains_that_share_concepts_are_linked_by_the_first_three_and_a_count() {
    let repo = generated_repo();
    let source = source(&page(repo.path(), "/map").await);
    let edges: Vec<&str> = source.lines().filter(|l| l.contains("---")).collect();
    assert_eq!(edges.len(), 1, "{source}");
    assert!(
        edges[0].contains("|\"Address, Card, customer +2\"|"),
        "{}",
        edges[0]
    );
    assert!(!source.contains("Unused"), "{source}");
    assert!(!source.contains("Plan"), "{source}");
}

// spec: WEB-70
#[tokio::test]
async fn a_single_domain_is_a_node_and_no_edge() {
    let repo = generated_repo();
    save_domains(repo.path(), vec![cluster("billing", "Billing")]);
    let source = source(&page(repo.path(), "/map").await);
    assert!(source.contains("Billing<br>"), "{source}");
    assert!(!source.contains("---"), "{source}");
}

// spec: WEB-71
#[tokio::test]
async fn the_map_lists_each_domain_with_the_concepts_it_shares_with_each_other() {
    let repo = generated_repo();
    let page = page(repo.path(), "/map").await;
    let list = list(&page);
    for href in ["/map/billing", "/map/delivery", "/map/archive"] {
        assert!(list.contains(&format!("href=\"{href}\"")), "{href}: {list}");
    }
    let billing = &list[list.find("href=\"/map/billing\"").unwrap()..];
    let billing = &billing[..billing.find("href=\"/map/archive\"").unwrap()];
    for concept in ["address", "card", "customer", "invoice", "zone"] {
        assert!(
            billing.contains(&format!("href=\"/glossary#{concept}\"")),
            "{concept}: {billing}"
        );
    }
    assert!(!billing.contains("/glossary#plan"), "{billing}");
}

// spec: WEB-74
#[tokio::test]
async fn each_node_of_the_map_is_the_link_of_its_item_in_the_list() {
    let repo = generated_repo();
    let page = page(repo.path(), "/map").await;
    let nodes = node_ids(&source(&page));
    assert_eq!(nodes.len(), 3, "{nodes:?}");
    let mut linked = data_nodes(list(&page));
    linked.sort();
    linked.dedup();
    let mut nodes_sorted = nodes.clone();
    nodes_sorted.sort();
    assert_eq!(linked, nodes_sorted, "{page}");
    let billing = &nodes[0];
    assert!(
        page.contains(&format!("href=\"/map/billing\" data-node=\"{billing}\"")),
        "{page}"
    );
}

// spec: WEB-75, WEB-10
#[tokio::test]
async fn a_double_quote_of_a_name_stays_inside_its_label() {
    let repo = generated_repo();
    save_domains(
        repo.path(),
        vec![cluster("billing", "a\"] --> x[\"b <script>")],
    );
    let page = page(repo.path(), "/map").await;
    assert!(!page.contains("<script>"), "{page}");
    let source = source(&page);
    assert!(source.contains("\"a#quot;] --> x[#quot;b"), "{source}");
    assert_eq!(node_ids(&source).len(), 1, "{source}");
}

// spec: WEB-24
#[tokio::test]
async fn the_map_loads_the_scripts_that_draw_it() {
    let repo = generated_repo();
    let page = page(repo.path(), "/map").await;
    assert!(page.contains("src=\"/assets/mermaid-"), "{page}");
    assert!(page.contains("src=\"/assets/diagrams.js\""), "{page}");
}

// spec: WEB-70
#[tokio::test]
async fn with_no_run_the_map_says_to_run_generate() {
    let repo = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(repo.path().join(".retrodoc")).unwrap();
    let (status, page) = get(repo.path(), "/map").await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("retrodoc generate"), "{page}");
}
