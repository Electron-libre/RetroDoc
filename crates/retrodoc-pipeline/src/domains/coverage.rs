use super::{
    BTreeSet, CoverageReport, DomainCluster, DomainMap, FileSummary, Path, PathBuf,
    SubDomainCluster, UNCATEGORIZED_SLUG,
};

/// One directory the LLM assigned to a domain (or sub-domain), with its
/// depth precomputed for longest-prefix-match resolution.
pub(super) struct DirAssignment {
    pub(super) dir: PathBuf,
    pub(super) depth: usize,
    pub(super) domain_idx: usize,
    pub(super) sub_domain_idx: Option<usize>,
}

/// Flattens every directory `map` assigned (domain `paths` first, then each
/// domain's `sub_domains` `paths`, in order) into a list of
/// [`DirAssignment`]s for [`resolve_target`].
pub(super) fn collect_dir_assignments(map: &DomainMap) -> Vec<DirAssignment> {
    let mut assignments = Vec::new();
    for (domain_idx, domain) in map.domains.iter().enumerate() {
        for dir in &domain.paths {
            assignments.push(DirAssignment {
                dir: dir.clone(),
                depth: dir.components().count(),
                domain_idx,
                sub_domain_idx: None,
            });
        }
        for (sub_domain_idx, sub) in domain.sub_domains.iter().enumerate() {
            for dir in &sub.paths {
                assignments.push(DirAssignment {
                    dir: dir.clone(),
                    depth: dir.components().count(),
                    domain_idx,
                    sub_domain_idx: Some(sub_domain_idx),
                });
            }
        }
    }
    assignments
}

/// Picks the assigned directory that is the deepest (most specific) ancestor
/// of `file_path`, i.e. the longest-prefix match. Ties (only possible when
/// the LLM assigned the same directory twice) resolve to the first
/// occurrence in `assignments`.
pub(super) fn resolve_target<'a>(
    assignments: &'a [DirAssignment],
    file_path: &Path,
) -> Option<&'a DirAssignment> {
    let mut best: Option<&DirAssignment> = None;
    for candidate in assignments.iter().filter(|a| file_path.starts_with(&a.dir)) {
        if best.is_none_or(|b| candidate.depth > b.depth) {
            best = Some(candidate);
        }
    }
    best
}

/// Mechanically expands a module/directory-level clustering into a
/// file-level one: each file is routed to its most-specific assigned
/// ancestor directory (see module docs). Directory assignments that match no
/// real file become domains/sub-domains with empty `paths`, which is
/// harmless. A file matched by no assigned directory is left unassigned;
/// the caller's subsequent [`enforce_coverage`] call buckets it into
/// "uncategorized" exactly as it would a file an LLM forgot under the old
/// flat per-file scheme.
pub(super) fn expand_to_files(map: DomainMap, files: &[FileSummary]) -> DomainMap {
    let assignments = collect_dir_assignments(&map);

    let mut expanded = DomainMap {
        domains: map
            .domains
            .into_iter()
            .map(|domain| DomainCluster {
                paths: Vec::new(),
                sub_domains: domain
                    .sub_domains
                    .into_iter()
                    .map(|sub| SubDomainCluster {
                        paths: Vec::new(),
                        ..sub
                    })
                    .collect(),
                ..domain
            })
            .collect(),
    };

    for file in files {
        let Some(target) = resolve_target(&assignments, &file.path) else {
            continue;
        };
        match target.sub_domain_idx {
            Some(sub_idx) => expanded.domains[target.domain_idx].sub_domains[sub_idx]
                .paths
                .push(file.path.clone()),
            None => expanded.domains[target.domain_idx]
                .paths
                .push(file.path.clone()),
        }
    }

    expanded
}

/// Enforces 100% coverage and no overlap on `map` in place (see module
/// docs), and reports what was found/repaired.
pub(super) fn enforce_coverage(map: &mut DomainMap, all_paths: &[PathBuf]) -> CoverageReport {
    let known: BTreeSet<&PathBuf> = all_paths.iter().collect();
    let mut seen: BTreeSet<PathBuf> = BTreeSet::new();
    let mut overlapping = Vec::new();
    let mut unknown = Vec::new();

    for domain in &mut map.domains {
        retain_known_and_first_seen(
            &mut domain.paths,
            &known,
            &mut seen,
            &mut overlapping,
            &mut unknown,
        );
        for sub in &mut domain.sub_domains {
            retain_known_and_first_seen(
                &mut sub.paths,
                &known,
                &mut seen,
                &mut overlapping,
                &mut unknown,
            );
        }
    }

    let uncovered: Vec<PathBuf> = all_paths
        .iter()
        .filter(|p| !seen.contains(*p))
        .cloned()
        .collect();
    if !uncovered.is_empty() {
        map.domains.push(uncategorized_domain(uncovered.clone()));
    }

    CoverageReport {
        uncovered,
        overlapping,
        unknown,
    }
}

/// Keeps a path only if it's a known source file and its first appearance
/// across the whole clustering; drops duplicates and hallucinated paths
/// into `overlapping`/`unknown` respectively.
pub(super) fn retain_known_and_first_seen(
    paths: &mut Vec<PathBuf>,
    known: &BTreeSet<&PathBuf>,
    seen: &mut BTreeSet<PathBuf>,
    overlapping: &mut Vec<PathBuf>,
    unknown: &mut Vec<PathBuf>,
) {
    paths.retain(|path| {
        if !known.contains(path) {
            unknown.push(path.clone());
            return false;
        }
        if seen.insert(path.clone()) {
            true
        } else {
            overlapping.push(path.clone());
            false
        }
    });
}

pub(super) fn uncategorized_domain(paths: Vec<PathBuf>) -> DomainCluster {
    DomainCluster {
        slug: UNCATEGORIZED_SLUG.to_string(),
        name: "Uncategorized".to_string(),
        description: "Files the clustering pass could not confidently assign to a functional \
            domain; needs manual review."
            .to_string(),
        paths,
        sub_domains: Vec::new(),
    }
}
