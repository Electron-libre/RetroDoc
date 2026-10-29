//! Placing the entry points no feature claimed (ADR 0020, coverage by construction): one small call
//! per batch shows the features and the entry points left out; the model puts each into an existing
//! feature or leaves it out. What stays out is the uncategorized bucket of the entry points, saved
//! in `behaviour-coverage.yaml` and listed by the debt report.

use super::*;
use crate::behaviour_coverage::BehaviourCoverage;

/// Entry points per placement call.
const REPAIR_BATCH: usize = 60;
const MAX_FEATURES_SHOWN: usize = 150;

const PLACEMENT_SYSTEM_PROMPT: &str = "You are completing the functional (business) documentation \
of a software project. Some entry points (what users and systems can do) were left out of its \
features. For each entry point listed, pick the existing feature it belongs to, from the ones \
given. Copy the entry point's short id (E1, E2…, the first word of its line) and the feature slug \
verbatim. Leave an entry point out of your \
answer if no feature fits it; never invent a feature. Reply with ONLY a single JSON object, no \
prose and no Markdown code fence, matching this shape: \
{\"placements\":[{\"entry_point\":\"id\",\"feature\":\"slug\"}]}.";

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RawPlacements {
    #[serde(default)]
    placements: Vec<RawPlacement>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RawPlacement {
    entry_point: String,
    feature: String,
}

/// An entry point no feature holds.
struct Unclaimed<'a> {
    id: String,
    file: &'a Path,
    point: &'a EntryPoint,
}

fn unclaimed<'a>(entry_points: &'a EntryPoints, features: &[Feature]) -> Vec<Unclaimed<'a>> {
    let claimed: BTreeSet<&str> = features
        .iter()
        .flat_map(|f| f.entry_points.iter().map(String::as_str))
        .collect();
    entry_points
        .iter()
        .map(|(file, point)| Unclaimed {
            id: entry_point_id(file, point),
            file,
            point,
        })
        .filter(|u| !claimed.contains(u.id.as_str()))
        .collect()
}

/// The ids of the entry points of `entry_points` that no feature of `features` holds.
pub(super) fn unplaced_ids(entry_points: &EntryPoints, features: &[Feature]) -> Vec<String> {
    unclaimed(entry_points, features)
        .into_iter()
        .map(|u| u.id)
        .collect()
}

fn placement_print(
    left: &[Unclaimed<'_>],
    features: &[Feature],
    brief: &ProductBrief,
    llm: &dyn LlmProvider,
) -> String {
    fingerprint(
        std::iter::once("entry point placement".to_string())
            .chain(
                left.iter()
                    .map(|u| format!("entry {}\n{}", u.id, u.point.description)),
            )
            .chain(
                features
                    .iter()
                    .map(|f| format!("feature {}\n{}\n{}", f.slug, f.name, f.description)),
            )
            .chain(brief.print_part())
            .chain(std::iter::once(model_part(llm))),
    )
}

fn placement_prompt(
    features: &[Feature],
    batch: &[&Unclaimed<'_>],
    brief: &ProductBrief,
) -> String {
    let mut prompt = format!("{}Features:\n", brief.prompt_head());
    for f in features.iter().take(MAX_FEATURES_SHOWN) {
        let _ = writeln!(
            prompt,
            "- {} ({}): {} — {}",
            f.slug, f.domain_slug, f.name, f.description
        );
    }
    prompt.push_str("\nEntry points to place:\n");
    for (position, u) in batch.iter().enumerate() {
        let description: String = u
            .point
            .description
            .chars()
            .take(MAX_DESCRIPTION_CHARS)
            .collect();
        let _ = writeln!(
            prompt,
            "- {} — {} \"{}\" ({}): {} on \"{}\": {description}",
            tag(position),
            u.point.kind.label(),
            u.point.name,
            u.file.display(),
            u.point.verb,
            u.point.resource
        );
    }
    prompt
}

/// Puts the entry points of `entry_points` that no feature holds into a feature, and saves the ones
/// that stay out in the coverage artifact. With an unchanged input (unplaced entry points, features,
/// brief, model) the print in `saved_print` matches and no call is made. Returns whether `features`
/// changed.
///
/// # Errors
///
/// Returns an error if an LLM call fails or an artifact can't be saved.
pub(super) async fn place_unclaimed(
    repo_root: &Path,
    features: &mut [Feature],
    entry_points: &EntryPoints,
    brief: &ProductBrief,
    saved_print: &mut Option<String>,
    llm: &dyn LlmProvider,
) -> Result<bool, PipelineError> {
    let left = unclaimed(entry_points, features);
    let mut coverage = BehaviourCoverage::load(repo_root).unwrap_or_default();
    if left.is_empty() || features.is_empty() {
        coverage.unplaced_entry_points = left.iter().map(|u| u.id.clone()).collect();
        coverage.save(repo_root)?;
        *saved_print = None;
        return Ok(false);
    }
    let print = placement_print(&left, features, brief, llm);
    if saved_print.as_ref() == Some(&print) {
        tracing::info!(
            entry_points = left.len(),
            "entry point placement unchanged, reused"
        );
        return Ok(false);
    }

    let mut changed = false;
    let mut placed: BTreeSet<String> = BTreeSet::new();
    let batches: Vec<Vec<&Unclaimed<'_>>> = left
        .iter()
        .collect::<Vec<_>>()
        .chunks(REPAIR_BATCH)
        .map(<[_]>::to_vec)
        .collect();
    for batch in &batches {
        let prompt = placement_prompt(features, batch, brief);
        let raw = complete_json::<RawPlacements>(
            llm,
            PLACEMENT_SYSTEM_PROMPT,
            &prompt,
            "placement of the entry points left out",
        )
        .await?;
        let Some(raw) = raw else { continue };
        for placement in raw.placements {
            let id = placement.entry_point.trim();
            let found = tag_position(id, batch.len())
                .and_then(|p| batch.get(p))
                .or_else(|| batch.iter().find(|u| u.id == id));
            let Some(entry) = found else {
                tracing::warn!(entry_point = %id, "placement of an entry point not asked for, dropped");
                continue;
            };
            let Some(feature) = features.iter_mut().find(|f| f.slug == placement.feature) else {
                tracing::warn!(feature = %placement.feature, "placement in an unknown feature, dropped");
                continue;
            };
            if !placed.insert(entry.id.clone()) {
                continue;
            }
            feature.entry_points.push(entry.id.clone());
            let file = entry.file.display().to_string();
            if !feature.source_paths.contains(&file) {
                feature.source_paths.push(file);
                feature.source_paths.sort();
            }
            changed = true;
        }
    }

    coverage.unplaced_entry_points = left
        .iter()
        .filter(|u| !placed.contains(&u.id))
        .map(|u| u.id.clone())
        .collect();
    if !coverage.unplaced_entry_points.is_empty() {
        tracing::warn!(
            entry_points = coverage.unplaced_entry_points.len(),
            "entry points no feature holds, listed in the coverage report"
        );
    }
    coverage.save(repo_root)?;
    // The print of the input *after* the placements, so the next run compares like with like.
    let after = unclaimed(entry_points, features);
    *saved_print = Some(placement_print(&after, features, brief, llm));
    Ok(changed)
}
