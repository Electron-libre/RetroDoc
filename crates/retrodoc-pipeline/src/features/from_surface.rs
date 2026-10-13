//! Features from the entry points (`[pipeline] domains_from = "surface"`, ADR 0020): for each domain
//! of the surface, one call groups the entry points of its resources into features, framed by the
//! product brief and the domain's entities. An entry point is cited by its id, `<file>#<name>`; a
//! feature keeps the ids it groups (`Feature::entry_points`) and the files that define them. No repo
//! map is read.
//!
//! The first placement of an entry point wins, an id the domain does not know is dropped, a feature
//! left with no entry point is dropped. An entry point no feature claimed is only logged for now.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use self::repair::place_unclaimed;
use super::*;
use crate::artifact::Retention;
use crate::behaviour_coverage::BehaviourCoverage;
use crate::entry_points::EntryFile;
use crate::naming::normalize;
use crate::surface::Surface;

/// Entry points listed in one prompt.
const MAX_ENTRIES_PER_PROMPT: usize = 150;
const MAX_DESCRIPTION_CHARS: usize = 160;

const SURFACE_FEATURES_SYSTEM_PROMPT: &str = "You are documenting a software project from a \
functional (business) point of view. Given a functional domain, its business entities and its \
entry points (what users and systems can do: routes, commands, jobs, public API), group the entry \
points into the user-facing features of the domain: capabilities a user or an external system \
would recognize, not technical layers. Every feature must list at least one entry point, and an \
entry point goes in exactly one feature. Entry points are cited by their short id (E1, E2…, the \
first word of each line of the list), copied verbatim. Reply with ONLY a single JSON object, no prose and no \
Markdown code fence, matching this shape: {\"features\":[{\"slug\":\"kebab-case\",\"name\":\"...\",\
\"description\":\"one or two sentences\",\"entry_points\":[\"...\"]}]}.";

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RawSurfaceFeatures {
    #[serde(default)]
    features: Vec<RawSurfaceFeature>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RawSurfaceFeature {
    slug: String,
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    entry_points: Vec<String>,
}

/// An entry point of a domain, with the id the model cites it by.
struct Entry<'a> {
    id: String,
    file: &'a Path,
    point: &'a EntryPoint,
}

/// The short tag the prompts show for the entry point at `position` of a list (`E1`, `E2`…): the
/// model copies it reliably, where it garbles `<file>#<name>`.
pub(super) fn tag(position: usize) -> String {
    format!("E{}", position + 1)
}

/// The position a tag cited by the model stands for, if it is one of `len`.
pub(super) fn tag_position(cited: &str, len: usize) -> Option<usize> {
    let number: usize = cited.trim().strip_prefix(['E', 'e'])?.parse().ok()?;
    (1..=len).contains(&number).then(|| number - 1)
}

/// The id of an entry point: its file and its name, unique by construction.
#[must_use]
pub fn entry_point_id(file: &Path, entry: &EntryPoint) -> String {
    format!("{}#{}", file.display(), entry.name)
}

/// Derives the features of every domain of `domains` (built from the surface) from the entry points
/// of its resources and persists them to `.retrodoc/cache/features.yaml`.
///
/// Incremental: a domain whose entry points, entities, brief and model are unchanged keeps the
/// features saved then, without an LLM call. A domain with no entry point is skipped, as is the
/// "uncategorized" one.
///
/// # Errors
///
/// Returns an error if an LLM call fails or the artifact can't be saved.
#[allow(clippy::too_many_arguments)] // the inputs of one pass, as in the other passes
pub async fn build_features_from_surface(
    repo_root: &Path,
    domains: &DomainMap,
    entry_points: &EntryPoints,
    surface: &Surface,
    brief: &ProductBrief,
    evidence: &Evidence,
    llm: &dyn LlmProvider,
    keep: Retention,
) -> Result<Vec<Feature>, PipelineError> {
    let previous = load_features(repo_root).unwrap_or_default();
    let mut prints = Fingerprints::load(repo_root);
    let known_units = std::mem::take(&mut prints.features);
    let mut features: Vec<Feature> = Vec::new();
    if keep == Retention::Others {
        features = retain_other_domains(domains, &previous, &known_units, &mut prints);
    }
    let retained = features.len();

    let units: Vec<(&DomainCluster, Vec<Entry<'_>>)> = domains
        .domains
        .iter()
        .filter(|d| d.slug != UNCATEGORIZED_SLUG)
        .map(|d| (d, domain_entries(d, entry_points)))
        .filter(|(_, entries)| !entries.is_empty())
        .collect();
    let mut progress = Progress::new("features", units.len());

    for (domain, entries) in &units {
        let key = unit_key(&domain.slug, None);
        let close = evidence.section(&domain_query(domain, entries));
        let print = domain_fingerprint(domain, entries, surface, brief, &close, llm);
        if known_units.get(&key) == Some(&print) {
            if let Some(kept) = reusable_for(&previous, &features, &domain.slug) {
                tracing::info!(unit = %key, "features unchanged, reused");
                features.extend(kept);
                prints.features.insert(key, print);
                progress.skip();
                continue;
            }
        }
        progress.begin(&key);
        let prompt = surface_features_prompt(domain, entries, surface, brief, &close);
        let what = format!("features of {key}");
        // An answer with no usable feature (ids copied wrongly) is asked once more.
        let mut produced: Vec<Feature> = Vec::new();
        for _ in 0..2 {
            let raw = match complete_json::<RawSurfaceFeatures>(
                llm,
                SURFACE_FEATURES_SYSTEM_PROMPT,
                &prompt,
                &what,
            )
            .await
            {
                Ok(Some(raw)) => raw,
                Ok(None) => continue,
                Err(err) => {
                    save_partial(repo_root, features, &previous, prints, &known_units);
                    return Err(err);
                }
            };
            produced = place_entry_points(raw, domain, entries, &features);
            if !produced.is_empty() {
                break;
            }
        }
        if !produced.is_empty() {
            prints.features.insert(key, print);
        }
        features.extend(produced);
        // After each domain: a run killed later loses the call in flight, nothing more.
        save_partial(
            repo_root,
            features.clone(),
            &previous,
            prints.clone(),
            &known_units,
        );
    }

    settle_duplicates(&mut features, &units);
    // Saved before the placement call: a failure there keeps the units done.
    prints.save(repo_root)?;
    save_features(repo_root, &features)?;
    let placed = if keep == Retention::Others {
        place_per_domain(
            repo_root,
            domains,
            &mut features,
            retained,
            entry_points,
            brief,
            &mut prints,
            llm,
        )
        .await?
    } else {
        place_unclaimed(
            repo_root,
            &mut features,
            entry_points,
            brief,
            &mut prints.entry_point_placement,
            llm,
        )
        .await?
    };
    prints.save(repo_root)?;
    if placed {
        save_features(repo_root, &features)?;
    }
    Ok(features)
}

/// The features of the domains that are not in `domains` (the other bounded contexts), and their
/// fingerprints put back in `prints`.
fn retain_other_domains(
    domains: &DomainMap,
    previous: &[Feature],
    known_units: &BTreeMap<String, String>,
    prints: &mut Fingerprints,
) -> Vec<Feature> {
    let run: BTreeSet<&str> = domains.domains.iter().map(|d| d.slug.as_str()).collect();
    for (key, print) in known_units {
        if key
            .split_once('/')
            .is_some_and(|(slug, _)| !run.contains(slug))
        {
            prints.features.insert(key.clone(), print.clone());
        }
    }
    previous
        .iter()
        .filter(|f| !run.contains(f.domain_slug.as_str()))
        .cloned()
        .collect()
}

/// A slice at a time, the entry points no feature claimed are placed domain by domain: into a
/// feature of their own domain only, with a print per domain, so that a domain already placed is
/// not asked again whatever the others of the run are. The entry points the earlier slices left
/// unplaced stay listed. Returns whether a feature changed.
#[allow(clippy::too_many_arguments)] // the inputs of one step, as in the pass
async fn place_per_domain(
    repo_root: &Path,
    domains: &DomainMap,
    features: &mut [Feature],
    retained: usize,
    entry_points: &EntryPoints,
    brief: &ProductBrief,
    prints: &mut Fingerprints,
    llm: &dyn LlmProvider,
) -> Result<bool, PipelineError> {
    let earlier = BehaviourCoverage::load(repo_root)
        .map(|c| c.unplaced_entry_points)
        .unwrap_or_default();
    let in_run: BTreeSet<String> = domains
        .domains
        .iter()
        .flat_map(|d| d.paths.iter().map(|p| p.display().to_string()))
        .collect();
    // The entry points of the run are placed again; those of its files that another context serves
    // (a route declared there), and the ones of the contexts out of the run, stay as they were.
    let held: BTreeSet<String> = domains
        .domains
        .iter()
        .flat_map(|d| {
            entry_points_of(d, entry_points)
                .iter()
                .map(|(f, e)| entry_point_id(f, e))
                .collect::<Vec<_>>()
        })
        .collect();
    let routed_away: BTreeSet<&String> =
        domains.domains.iter().flat_map(|d| &d.routed_out).collect();
    let mut unplaced: Vec<String> = earlier
        .into_iter()
        .filter(|id| {
            let in_run_file = id
                .split('#')
                .next()
                .is_some_and(|file| in_run.contains(file));
            let stale = held.contains(id) || (in_run_file && !routed_away.contains(id));
            !stale
        })
        .collect();
    let mut changed = false;
    for domain in &domains.domains {
        let own = entry_points_of(domain, entry_points);
        // A domain with no print left to keep: it is placed again from scratch when it comes back.
        let mut print = prints.entry_point_placements.remove(&domain.slug);
        let Some(range) = domain_range(features, retained, &domain.slug) else {
            // No feature at all in this domain: nothing to place into.
            unplaced.extend(own.iter().map(|(file, entry)| entry_point_id(file, entry)));
            continue;
        };
        changed |= place_unclaimed(
            repo_root,
            &mut features[range.clone()],
            &own,
            brief,
            &mut print,
            llm,
        )
        .await?;
        if let Some(print) = print {
            prints
                .entry_point_placements
                .insert(domain.slug.clone(), print);
        }
        unplaced.extend(repair::unplaced_ids(&own, &features[range]));
    }
    let mut coverage = BehaviourCoverage::load(repo_root).unwrap_or_default();
    coverage.unplaced_entry_points = unplaced;
    coverage.save(repo_root)?;
    Ok(changed)
}

/// The positions, from `from`, of the features of `slug` (a domain's features are contiguous).
fn domain_range(features: &[Feature], from: usize, slug: &str) -> Option<std::ops::Range<usize>> {
    let start = from
        + features[from..]
            .iter()
            .position(|f| f.domain_slug == slug)?;
    let len = features[start..]
        .iter()
        .take_while(|f| f.domain_slug == slug)
        .count();
    Some(start..start + len)
}

/// Whether the entry point `id` of `file` is one of the entry points of a domain built from its
/// files: it is in one of them and no other domain serves it, or this domain serves it from a file
/// of another.
fn held_by_paths(domain: &DomainCluster, file: &Path, id: &str) -> bool {
    domain.routed_in.contains(id)
        || (!domain.routed_out.contains(id) && domain.paths.iter().any(|p| p == file))
}

/// The entry points of `domain` (built from its files), as the entry points of its files.
fn entry_points_of(domain: &DomainCluster, entry_points: &EntryPoints) -> EntryPoints {
    let mut held = EntryPoints::default();
    for (file, entry) in entry_points.iter() {
        if held_by_paths(domain, file, &entry_point_id(file, entry)) {
            held.files
                .entry(file.to_path_buf())
                .or_insert_with(|| EntryFile {
                    content_hash: entry_points.files[file].content_hash.clone(),
                    entry_points: Vec::new(),
                })
                .entry_points
                .push(entry.clone());
        }
    }
    held
}

/// The entry points of the resources of a domain, in path order.
fn domain_entries<'a>(domain: &DomainCluster, entry_points: &'a EntryPoints) -> Vec<Entry<'a>> {
    if domain.by_paths {
        return entry_points
            .iter()
            .map(|(file, entry)| (file, entry, entry_point_id(file, entry)))
            .filter(|(file, _, id)| held_by_paths(domain, file, id))
            .map(|(file, entry, id)| Entry {
                id,
                file,
                point: entry,
            })
            .collect();
    }
    let resources: BTreeSet<String> = domain.resources.iter().map(|r| normalize(r)).collect();
    entry_points
        .iter()
        .filter(|(_, entry)| resources.contains(&normalize(&entry.resource)))
        .map(|(file, entry)| Entry {
            id: entry_point_id(file, entry),
            file,
            point: entry,
        })
        .collect()
}

fn domain_query(domain: &DomainCluster, entries: &[Entry<'_>]) -> String {
    let mut query = format!("{} {}", domain.name, domain.description);
    for e in entries {
        let _ = write!(query, " {} {}", e.point.name, e.point.resource);
    }
    query
}

/// Everything the features of a domain are derived from, besides the prompt text.
fn domain_fingerprint(
    domain: &DomainCluster,
    entries: &[Entry<'_>],
    surface: &Surface,
    brief: &ProductBrief,
    close: &str,
    llm: &dyn LlmProvider,
) -> String {
    let entities = domain.entities.iter().map(|name| {
        let description = entity_description(surface, name);
        format!("entity {name}\n{description}")
    });
    fingerprint(
        std::iter::once("features from the surface".to_string())
            .chain(entries.iter().map(|e| {
                let outputs: Vec<&str> = e
                    .point
                    .outputs
                    .iter()
                    .map(|o| o.description.as_str())
                    .collect();
                format!(
                    "entry {}\n{} {}\n{}\n{}",
                    e.id,
                    e.point.verb,
                    e.point.resource,
                    e.point.description,
                    outputs.join("|")
                )
            }))
            .chain(entities)
            .chain((!brief.fingerprint().is_empty()).then(|| brief.fingerprint()))
            .chain((!close.is_empty()).then(|| close.to_string()))
            .chain(std::iter::once(model_part(llm))),
    )
}

fn entity_description(surface: &Surface, name: &str) -> String {
    let key = normalize(name);
    surface
        .entities
        .iter()
        .find(|e| normalize(&e.name) == key)
        .map_or_else(String::new, |e| e.description.clone())
}

fn surface_features_prompt(
    domain: &DomainCluster,
    entries: &[Entry<'_>],
    surface: &Surface,
    brief: &ProductBrief,
    evidence: &str,
) -> String {
    let mut prompt = format!(
        "{}Domain: {} — {}\n",
        brief.prompt_head(),
        domain.name,
        domain.description
    );
    if !evidence.is_empty() {
        let _ = write!(prompt, "\n{evidence}");
    }
    if !domain.entities.is_empty() {
        prompt.push_str("\nEntities:\n");
        for name in &domain.entities {
            let description: String = entity_description(surface, name)
                .chars()
                .take(MAX_DESCRIPTION_CHARS)
                .collect();
            let _ = writeln!(prompt, "- {name}: {description}");
        }
    }
    prompt.push_str("\nEntry points:\n");
    for (position, e) in entries.iter().enumerate().take(MAX_ENTRIES_PER_PROMPT) {
        let description: String = e
            .point
            .description
            .chars()
            .take(MAX_DESCRIPTION_CHARS)
            .collect();
        let outputs: Vec<&str> = e
            .point
            .outputs
            .iter()
            .map(|o| o.description.as_str())
            .filter(|d| !d.is_empty())
            .collect();
        let _ = write!(
            prompt,
            "- {} — {} \"{}\" ({}): {} on \"{}\": {description}",
            tag(position),
            e.point.kind.label(),
            e.point.name,
            e.file.display(),
            e.point.verb,
            e.point.resource
        );
        if !outputs.is_empty() {
            let _ = write!(prompt, " (outputs: {})", outputs.join("; "));
        }
        prompt.push('\n');
    }
    if entries.len() > MAX_ENTRIES_PER_PROMPT {
        let _ = writeln!(
            prompt,
            "({} more entry point(s) omitted from this list)",
            entries.len() - MAX_ENTRIES_PER_PROMPT
        );
    }
    prompt
}

/// The previous features of an unchanged domain, unless there are none (or none from the
/// surface) or one of their slugs was taken meanwhile by a newly derived feature.
fn reusable_for(
    previous: &[Feature],
    features: &[Feature],
    domain_slug: &str,
) -> Option<Vec<Feature>> {
    let kept: Vec<&Feature> = previous
        .iter()
        .filter(|f| f.domain_slug == domain_slug && !f.entry_points.is_empty())
        .collect();
    let collides = kept
        .iter()
        .any(|k| features.iter().any(|f| f.slug == k.slug));
    (!kept.is_empty() && !collides).then(|| kept.into_iter().cloned().collect())
}

/// The entry point a feature cites: by tag, else by id, else by its name alone when that is unambiguous in the
/// domain (the model often drops the file), with or without the leading `#` or `.` of a method.
fn resolve<'a, 'b>(entries: &'b [Entry<'a>], cited: &str) -> Option<&'b Entry<'a>> {
    if let Some(found) = tag_position(cited, entries.len()).and_then(|p| entries.get(p)) {
        return Some(found);
    }
    if let Some(found) = entries.iter().find(|e| e.id == cited) {
        return Some(found);
    }
    let bare = |name: &str| name.trim_start_matches(['#', '.']).to_string();
    let wanted = bare(cited);
    let mut matching = entries.iter().filter(|e| bare(&e.point.name) == wanted);
    let found = matching.next()?;
    matching.next().is_none().then_some(found)
}

/// Keeps an entry point in one feature. A feature that survived from a previous run can hold an
/// entry point the placement call had put there for lack of a feature in its own domain; when that
/// domain now claims it, the domain's feature keeps it. The files of a feature that lost entry
/// points follow, and one left with none is dropped.
fn settle_duplicates(features: &mut Vec<Feature>, units: &[(&DomainCluster, Vec<Entry<'_>>)]) {
    let mut holders: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (position, feature) in features.iter().enumerate() {
        for id in &feature.entry_points {
            holders.entry(id.clone()).or_default().push(position);
        }
    }
    let mut changed: BTreeSet<usize> = BTreeSet::new();
    for (id, positions) in holders.into_iter().filter(|(_, p)| p.len() > 1) {
        let owner = |position: usize| {
            units.iter().any(|(domain, entries)| {
                domain.slug == features[position].domain_slug && entries.iter().any(|e| e.id == id)
            })
        };
        let keep = positions
            .iter()
            .copied()
            .find(|p| owner(*p))
            .unwrap_or(positions[0]);
        for position in positions.into_iter().filter(|p| *p != keep) {
            features[position].entry_points.retain(|e| *e != id);
            changed.insert(position);
        }
    }
    for position in &changed {
        let feature = &mut features[*position];
        feature.source_paths = feature
            .entry_points
            .iter()
            .filter_map(|id| id.split_once('#').map(|(file, _)| file.to_string()))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
    }
    let mut position = 0;
    features.retain(|f| {
        let keep = !f.entry_points.is_empty() || !changed.contains(&position);
        position += 1;
        keep
    });
}

/// The model's features turned into [`Feature`]s: ids the domain does not know are dropped, the
/// first placement of an entry point wins, a feature with none left is dropped, slugs are made
/// unique against `existing` and each other.
fn place_entry_points(
    raw: RawSurfaceFeatures,
    domain: &DomainCluster,
    entries: &[Entry<'_>],
    existing: &[Feature],
) -> Vec<Feature> {
    let mut free: BTreeSet<&str> = entries.iter().map(|e| e.id.as_str()).collect();
    let mut produced: Vec<Feature> = Vec::new();
    for raw_feature in raw.features {
        let mut ids: Vec<String> = Vec::new();
        for cited in &raw_feature.entry_points {
            let cited = cited.trim();
            match resolve(entries, cited) {
                None => tracing::warn!(
                    feature = %raw_feature.slug,
                    entry_point = %cited,
                    "feature cites an entry point its domain does not have, dropped"
                ),
                Some(e) if free.remove(e.id.as_str()) => ids.push(e.id.clone()),
                Some(_) => {}
            }
        }
        if ids.is_empty() {
            // An id the domain does not know was reported above; here the ids were all taken.
            tracing::info!(
                feature = %raw_feature.slug,
                "feature dropped: it groups no entry point still free in its domain"
            );
            continue;
        }
        let mut files: Vec<String> = entries
            .iter()
            .filter(|e| ids.contains(&e.id))
            .map(|e| e.file.display().to_string())
            .collect();
        files.sort();
        files.dedup();
        let taken = existing.iter().chain(&produced).map(|f| f.slug.as_str());
        let slug = unique_slug(&raw_feature.slug, taken);
        produced.push(Feature {
            slug,
            domain_slug: domain.slug.clone(),
            sub_domain_slug: None,
            name: raw_feature.name,
            description: raw_feature.description,
            source_paths: files,
            entry_points: ids,
            confidence: None,
        });
    }
    if !free.is_empty() {
        tracing::warn!(
            domain = %domain.slug,
            entry_points = free.len(),
            "entry points no feature claimed"
        );
    }
    produced
}

mod repair;
#[cfg(test)]
mod tests;
