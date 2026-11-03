//! The read model of the MCP server and the web interface leaves out the domains no feature
//! belongs to (`specs/featureless-domains.md`).

use retrodoc_core::model::Feature;
use retrodoc_mcp::GeneratedDocs;
use retrodoc_pipeline::{DomainCluster, DomainMap};

fn domain(slug: &str, name: &str) -> DomainCluster {
    DomainCluster {
        slug: slug.into(),
        name: name.into(),
        description: format!("{name} of the shop"),
        ..DomainCluster::default()
    }
}

// spec: EDM-3
#[test]
fn the_read_model_lists_no_domain_without_a_feature() {
    let dir = tempfile::tempdir().unwrap();
    let feature = Feature {
        back_office: false,
        technical: false,
        slug: "pay-invoice".into(),
        domain_slug: "billing".into(),
        name: "Pay an invoice".into(),
        description: "Settle what is due".into(),
        source_paths: vec!["app/invoice.rb".into()],
        entry_points: Vec::new(),
        confidence: None,
    };
    retrodoc_pipeline::save_features(dir.path(), &[feature]).unwrap();
    DomainMap {
        domains: vec![domain("billing", "Billing"), domain("feeds", "Feeds")],
    }
    .save(dir.path())
    .unwrap();

    let docs = GeneratedDocs::load(dir.path(), &[]).unwrap();

    let slugs: Vec<&str> = docs.domains().iter().map(|d| d.slug.as_str()).collect();
    assert_eq!(slugs, ["billing"]);
}
