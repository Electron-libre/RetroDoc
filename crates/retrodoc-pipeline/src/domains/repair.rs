use std::collections::BTreeSet;

use super::*;
use crate::response::complete_json;

/// Files per repair call: keeps the prompt small when a clustering forgot a
/// lot of files.
const REPAIR_BATCH: usize = 60;

const REPAIR_SYSTEM_PROMPT: &str = "You are completing the functional (business) domain breakdown \
of a software repository. Some files were left out of it. For each file listed, pick the domain \
(and, if one fits better, the sub-domain of that domain) it belongs to, from the ones given. Use \
the domain and sub-domain slugs verbatim and copy each file path verbatim. Leave a file out of \
your answer if no domain fits it. Reply with ONLY a single JSON object, no prose and no Markdown \
code fence, matching this shape: {\"assignments\":[{\"path\":\"...\",\"domain\":\"slug\",\
\"sub_domain\":\"slug or null\"}]}.";

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RepairAnswer {
    #[serde(default)]
    assignments: Vec<Assignment>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct Assignment {
    path: PathBuf,
    domain: String,
    #[serde(default)]
    sub_domain: Option<String>,
}

/// The files of `files` that no domain or sub-domain of `map` holds.
pub(super) fn unassigned_files(map: &DomainMap, files: &[FileSummary]) -> Vec<FileSummary> {
    let assigned: BTreeSet<&PathBuf> = map
        .domains
        .iter()
        .flat_map(|d| {
            d.paths
                .iter()
                .chain(d.sub_domains.iter().flat_map(|s| s.paths.iter()))
        })
        .collect();
    files
        .iter()
        .filter(|f| !assigned.contains(&f.path))
        .cloned()
        .collect()
}

fn repair_prompt(map: &DomainMap, files: &[FileSummary]) -> String {
    let mut prompt = String::from("Domains:\n");
    for domain in &map.domains {
        let _ = writeln!(
            prompt,
            "- {} ({}): {}",
            domain.slug, domain.name, domain.description
        );
        for sub in &domain.sub_domains {
            let _ = writeln!(
                prompt,
                "  - sub-domain {} ({}): {}",
                sub.slug, sub.name, sub.description
            );
        }
    }
    prompt.push_str("\nFiles to place:\n");
    for file in files {
        let _ = writeln!(prompt, "- {}: {}", file.path.display(), file.role_summary);
    }
    prompt
}

/// Places the `files` the clustering left unassigned, one LLM call per batch
/// of [`REPAIR_BATCH`]: the model sees each file's summary and the domains
/// found. Only a known file and an existing domain (and sub-domain) are
/// honoured; an unparseable answer or an invalid assignment leaves the file
/// unassigned, for the coverage repair to bucket. Returns the number of
/// files placed.
///
/// # Errors
///
/// Returns an error only if an LLM call itself fails.
pub(super) async fn assign_unplaced_files(
    map: &mut DomainMap,
    files: &[FileSummary],
    llm: &dyn LlmProvider,
) -> Result<usize, PipelineError> {
    if map.domains.is_empty() {
        return Ok(0);
    }
    let mut placed = 0;
    for batch in files.chunks(REPAIR_BATCH) {
        let prompt = repair_prompt(map, batch);
        let Some(answer) = complete_json::<RepairAnswer>(
            llm,
            REPAIR_SYSTEM_PROMPT,
            &prompt,
            "placement of unassigned files",
        )
        .await?
        else {
            continue;
        };
        let mut done: BTreeSet<&Path> = BTreeSet::new();
        for assignment in &answer.assignments {
            if !batch.iter().any(|f| f.path == assignment.path) || !done.insert(&assignment.path) {
                continue;
            }
            let Some(domain) = map.domains.iter_mut().find(|d| d.slug == assignment.domain) else {
                done.remove(assignment.path.as_path());
                continue;
            };
            let sub = assignment
                .sub_domain
                .as_deref()
                .and_then(|slug| domain.sub_domains.iter_mut().find(|s| s.slug == slug));
            match sub {
                Some(sub) => sub.paths.push(assignment.path.clone()),
                None => domain.paths.push(assignment.path.clone()),
            }
            placed += 1;
        }
    }
    Ok(placed)
}
