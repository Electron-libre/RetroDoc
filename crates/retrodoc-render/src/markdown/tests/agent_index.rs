use super::super::{agent_index, render};
use super::{index_of, meta, sample};
use crate::AGENT_INDEX_PATH;

#[test]
fn the_agent_index_lists_domains_and_features_with_links_and_confidence() {
    let (d, f, u) = sample();
    let files = render(&d, &f, &u, "r", &meta()).unwrap();
    let index = index_of(&files);
    assert!(index.starts_with("# Functional documentation\n"), "{index}");
    assert!(
        index.contains("- [Billing](billing/README.md) (80%): Invoices."),
        "{index}"
    );
    assert!(
        index.contains("[Pay an invoice](billing/pay-invoice.md) (⚠️ 30%): Customers pay."),
        "{index}"
    );
    assert!(
        index.contains("[List](billing/list.md) (not scored): Listing."),
        "{index}"
    );
    assert!(
        index.contains("(../_retrodoc/coverage-report.md)"),
        "{index}"
    );
    assert!(index.contains("retrodoc mcp --path"), "{index}");
}

#[test]
fn a_domain_without_feature_is_still_listed_and_no_domain_gives_a_clear_index() {
    let (d, _, _) = sample();
    let index = agent_index(&d, &[], 0);
    assert!(index.contains("- [Billing](billing/README.md)"), "{index}");
    let empty = agent_index(&[], &[], 0);
    assert!(empty.contains("No domain documented yet."), "{empty}");
}

// spec: RER-8
#[test]
fn a_rerun_on_unchanged_artifacts_rewrites_nothing_including_the_index() {
    let (d, f, u) = sample();
    let dir = tempfile::tempdir().unwrap();
    let first = crate::plan(dir.path(), render(&d, &f, &u, "r", &meta()).unwrap()).unwrap();
    first.apply(dir.path()).unwrap();
    assert!(dir.path().join(AGENT_INDEX_PATH).exists());
    let second = crate::plan(dir.path(), render(&d, &f, &u, "r", &meta()).unwrap()).unwrap();
    assert_eq!(second.change_count(), 0);
}

#[test]
fn rendering_is_deterministic() {
    let (d, f, u) = sample();
    assert_eq!(
        render(&d, &f, &u, "r", &meta()).unwrap(),
        render(&d, &f, &u, "r", &meta()).unwrap()
    );
}
