//! Second roles call (LNG-16 to LNG-19): the paths no rule of `roles.yaml`
//! matches, alone (no content), get glob rules saved in `unmatched_rules`,
//! which apply to those files only. The call is made once per set of such
//! files (its fingerprint is saved in `unmatched_asked`), over at most
//! [`MAX_UNMATCHED_PATHS`] of them.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use retrodoc_ingest::{FileEntry, FileKind};
use retrodoc_llm::LlmProvider;
use serde::Deserialize;

use super::{best_match, matchers, FileRole, RoleRule, RoleRules};
use crate::error::PipelineError;
use crate::fingerprints::fingerprint;
use crate::response::complete_json;

/// Paths a call sends, the shallowest first.
pub(crate) const MAX_UNMATCHED_PATHS: usize = 400;

const UNMATCHED_SYSTEM_PROMPT: &str = "You are completing the file role rules of a software \
repository: the files listed match none of its rules. From their paths alone, give glob rules that \
assign them a role. Roles: \"entrypoint\" (routes, controllers, CLI commands, jobs, message \
consumers, webhooks, public API of a library), \"model\" (entities, schemas, domain objects, \
migrations), \"logic\" (services, presenters, policies and other business logic), \"view\" \
(templates, UI components, serializers), \"infra\" (database, queue, HTTP clients, deployment, CI), \
\"config\" (settings, manifests, lockfiles), \"test\", \"docs\", \"other\" (assets, vendored or \
generated files). Patterns use glob syntax relative to the repo root, e.g. \"app/blueprints/**\" \
or \"**/*.coffee\"; a pattern only applies to the listed files. Leave out a file you cannot place. \
Reply with ONLY a single JSON object, no prose and no Markdown code fence, matching this shape: \
{\"rules\":[{\"pattern\":\"...\",\"role\":\"view\"}]}.";

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct UnmatchedRolesResponse {
    #[serde(default)]
    rules: Vec<RoleRule>,
}

impl RoleRules {
    /// The paths the second call would be asked about: the files neither
    /// `rules` nor `unmatched_rules` match, none when the files no rule of
    /// `rules` matches are the set already asked about. The files of the
    /// languages of `source_extensions` that are tests get their role from the
    /// walker, not from a rule.
    #[must_use]
    pub fn unmatched_to_ask(&self, files: &[FileEntry]) -> Vec<PathBuf> {
        let unmatched = self.unmatched_by_rules(files);
        if unmatched.is_empty() || self.unmatched_asked == Some(asked_fingerprint(&unmatched)) {
            return Vec::new();
        }
        let fallback = matchers(&self.unmatched_rules);
        unmatched
            .into_iter()
            .filter(|path| best_match(&fallback, path).is_none())
            .collect()
    }

    /// The source and other files no rule of `rules` matches, once the
    /// languages of `source_extensions` are promoted.
    fn unmatched_by_rules(&self, files: &[FileEntry]) -> Vec<PathBuf> {
        let mut files = files.to_vec();
        self.promote_sources(&mut files);
        let main = matchers(&self.rules);
        files
            .into_iter()
            .filter(|f| matches!(f.kind, FileKind::Source | FileKind::Other))
            .map(|f| f.path)
            .filter(|path| best_match(&main, path).is_none())
            .collect()
    }
}

fn asked_fingerprint(paths: &[PathBuf]) -> String {
    fingerprint(paths.iter().map(|p| p.to_string_lossy()))
}

/// Asks the LLM for the roles of the files no rule matches, when
/// [`RoleRules::unmatched_to_ask`] gives any, adds the rules of its answer to
/// `unmatched_rules` (a pattern already there is not added twice) and saves
/// `rules`. An unparseable answer changes nothing, so the next run asks again.
/// Warns of the files left unclassified.
pub(super) async fn classify_unmatched(
    repo_root: &Path,
    files: &[FileEntry],
    llm: &dyn LlmProvider,
    rules: &mut RoleRules,
) -> Result<(), PipelineError> {
    let mut to_ask = rules.unmatched_to_ask(files);
    if to_ask.is_empty() {
        return Ok(());
    }
    let unmatched_count = to_ask.len();
    to_ask.sort_by_key(|path| (path.components().count(), path.clone()));
    to_ask.truncate(MAX_UNMATCHED_PATHS);
    to_ask.sort();

    let mut prompt = format!("Stack: {}\n\nRules already given:\n", rules.stack);
    for rule in &rules.rules {
        let _ = writeln!(prompt, "{} -> {}", rule.pattern, rule.role.label());
    }
    prompt.push_str("\nFiles no rule matches:\n");
    for path in &to_ask {
        let _ = writeln!(prompt, "- {}", path.display());
    }
    let Some(response) = complete_json::<UnmatchedRolesResponse>(
        llm,
        UNMATCHED_SYSTEM_PROMPT,
        &prompt,
        "roles of the unmatched files",
    )
    .await?
    else {
        return Ok(());
    };

    for rule in response.rules {
        let known = rules
            .unmatched_rules
            .iter()
            .any(|r| r.pattern == rule.pattern);
        if rule.role != FileRole::Unclassified && !known {
            rules.unmatched_rules.push(rule);
        }
    }
    rules.unmatched_asked = Some(asked_fingerprint(&rules.unmatched_by_rules(files)));
    rules.save(repo_root)?;

    let mut promoted = files.to_vec();
    rules.promote_sources(&mut promoted);
    let left = rules
        .classify(&promoted)
        .files_with(FileRole::Unclassified)
        .len();
    if left > 0 {
        tracing::warn!(
            left,
            not_sent = unmatched_count - to_ask.len(),
            "files no role rule matches left unclassified"
        );
    }
    Ok(())
}
