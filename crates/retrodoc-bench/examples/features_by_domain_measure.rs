//! Measures, with no LLM call, how many entry points the features of a saved run hold, per domain
//! (`issues/done/placement_into_unknown_features.md`, step 1).
//!
//! Usage: `cargo run -q -p retrodoc-pipeline --example features_by_domain_measure -- <run dir>...`
//!
//! A run dir is a checkout of the repository with its `.retrodoc/cache` (`domains.yaml`,
//! `entry-points.yaml`, `features.yaml`). Prints, per run, the entry points held by a feature, those
//! no feature holds, and per domain: its entry points, its features and the share they hold. A
//! domain with entry points and no feature is where the features pass found nothing to derive.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use retrodoc_pipeline::{entry_point_id, load_features, DomainMap, EntryPoints};

/// Lowercase letters and digits only, as the resource names are compared.
fn key(name: &str) -> String {
    name.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

fn report(repo_root: &Path) -> Option<String> {
    let domains = DomainMap::load(repo_root)?;
    let entry_points = EntryPoints::load(repo_root)?;
    let features = load_features(repo_root)?;
    let held: BTreeSet<&str> = features
        .iter()
        .flat_map(|f| f.entry_points.iter().map(String::as_str))
        .collect();
    let all: Vec<(&Path, String, String)> = entry_points
        .iter()
        .map(|(file, e)| (file, entry_point_id(file, e), key(&e.resource)))
        .collect();
    let total = all.len();
    let in_features = all
        .iter()
        .filter(|(_, id, _)| held.contains(id.as_str()))
        .count();
    let mut out = format!(
        "## {}\n\n{total} entry points: {in_features} held by a feature, {} held by none; {} features.\n\n\
         | Domain | Entry points | Held | Features |\n|---|---|---|---|\n",
        repo_root.file_name()?.to_string_lossy(),
        total - in_features,
        features.len(),
    );
    for domain in &domains.domains {
        let resources: BTreeSet<String> = domain.resources.iter().map(|r| key(r)).collect();
        let of_domain: Vec<&(&Path, String, String)> = all
            .iter()
            .filter(|(file, id, resource)| {
                if domain.by_paths {
                    domain.routed_in.contains(id)
                        || (!domain.routed_out.contains(id)
                            && domain.paths.iter().any(|p| p == file))
                } else {
                    resources.contains(resource)
                }
            })
            .collect();
        let held_here = of_domain
            .iter()
            .filter(|(_, id, _)| held.contains(id.as_str()))
            .count();
        let n_features = features
            .iter()
            .filter(|f| f.domain_slug.as_str() == domain.slug)
            .count();
        let _ = writeln!(
            out,
            "| {} | {} | {held_here} | {n_features} |",
            domain.slug,
            of_domain.len()
        );
    }
    Some(out)
}

fn main() {
    let dirs: Vec<std::path::PathBuf> = std::env::args().skip(1).map(Into::into).collect();
    if dirs.is_empty() {
        eprintln!("usage: features_by_domain_measure <run dir>...");
        std::process::exit(2);
    }
    for dir in dirs {
        match report(&dir) {
            Some(text) => println!("{text}"),
            None => eprintln!(
                "{}: domains, entry points or features missing",
                dir.display()
            ),
        }
    }
}
