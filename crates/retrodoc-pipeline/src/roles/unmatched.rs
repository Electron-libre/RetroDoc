//! Second roles call (LNG-16 to LNG-19): the paths no rule of `roles.yaml`
//! matches, alone (no content), get glob rules that apply to those files only.
//! They are saved in `roles-unmatched.yaml`, hand-editable, with the paths the
//! call was given and left unclassified: a path is sent once, so a rerun with
//! no new unmatched file calls nothing. A call sends at most
//! [`MAX_UNMATCHED_PATHS`] paths, the shallowest; the others go to the next run.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use retrodoc_ingest::FileEntry;
use retrodoc_llm::LlmProvider;
use serde::{Deserialize, Serialize};

use super::{FileRole, RoleRule, RoleRules};
use crate::artifact::{save_yaml, Artifact};
use crate::error::PipelineError;
use crate::response::complete_json;

/// Paths a call sends, the shallowest first.
pub(crate) const MAX_UNMATCHED_PATHS: usize = 400;

const UNMATCHED_SYSTEM_PROMPT: &str = concat!(
    "You are completing the file role rules of a software repository: the files listed match none \
of its rules. From their paths alone, give glob rules that assign them a role. ",
    role_definitions!(),
    "Patterns use glob syntax relative to the repo root, e.g. \"app/blueprints/**\" or \
\"**/*.coffee\". They apply to every file the rules of the repository do not match, now and later: \
name these files as narrowly as their kind allows, never a catch-all such as \"**\". Leave out a \
file you cannot place. Reply with ONLY a single JSON object, no prose and no Markdown code fence, \
matching this shape: {\"rules\":[{\"pattern\":\"...\",\"role\":\"view\"}]}."
);

/// `roles-unmatched.yaml`: the roles of the files no rule of `roles.yaml`
/// matches.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UnmatchedRoles {
    /// Applied to the files no rule of `roles.yaml` matches, only.
    #[serde(default)]
    pub rules: Vec<RoleRule>,
    /// The paths the call was given and left unclassified: not sent again.
    #[serde(default)]
    pub left: BTreeSet<PathBuf>,
}

impl UnmatchedRoles {
    /// # Errors
    ///
    /// Returns an error if the cache folder or file can't be written.
    pub fn save(&self, repo_root: &Path) -> Result<(), PipelineError> {
        save_yaml(&Artifact::RolesUnmatched.path(repo_root), self)
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct UnmatchedRolesResponse {
    #[serde(default)]
    rules: Vec<RoleRule>,
}

impl RoleRules {
    /// The paths the second call would be given, the shallowest first: the
    /// files no rule matches, neither of `roles.yaml` nor of
    /// `roles-unmatched.yaml`, the call was not given yet. None when the rules
    /// of `roles.yaml` are empty (an unusable answer, not a repository to
    /// classify path by path).
    #[must_use]
    pub fn unmatched_to_ask(&self, files: &[FileEntry]) -> Vec<PathBuf> {
        if self.rules.is_empty() {
            return Vec::new();
        }
        let mut files = files.to_vec();
        self.promote_sources(&mut files);
        let mut paths: Vec<PathBuf> = self
            .classify(&files)
            .roles
            .into_iter()
            .filter(|(path, role)| {
                *role == FileRole::Unclassified && !self.unmatched.left.contains(path)
            })
            .map(|(path, _)| path)
            .collect();
        paths.sort_by_key(|path| (path.components().count(), path.clone()));
        paths
    }
}

/// Asks the LLM for the roles of the files [`RoleRules::unmatched_to_ask`]
/// gives (at most [`MAX_UNMATCHED_PATHS`]), adds the rules of its answer (a
/// pattern already there is not added twice) and records the paths it left
/// unclassified, then saves `roles-unmatched.yaml`. Best effort: a failed call
/// or an unusable answer is warned of and changes nothing, so the next run asks
/// again. Warns of the files left unclassified.
pub(super) async fn classify_unmatched(
    repo_root: &Path,
    files: &[FileEntry],
    llm: &dyn LlmProvider,
    rules: &mut RoleRules,
) {
    let mut sent = rules.unmatched_to_ask(files);
    if sent.is_empty() {
        return;
    }
    let not_sent = sent.len().saturating_sub(MAX_UNMATCHED_PATHS);
    sent.truncate(MAX_UNMATCHED_PATHS);

    let mut prompt = format!("Stack: {}\n\nRules already given:\n", rules.stack);
    for rule in &rules.rules {
        let _ = writeln!(prompt, "{} -> {}", rule.pattern, rule.role.label());
    }
    prompt.push_str("\nFiles no rule matches:\n");
    for path in &sent {
        let _ = writeln!(prompt, "- {}", path.display());
    }
    let response = match complete_json::<UnmatchedRolesResponse>(
        llm,
        UNMATCHED_SYSTEM_PROMPT,
        &prompt,
        "roles of the unmatched files",
    )
    .await
    {
        Ok(Some(response)) => response,
        Ok(None) => return,
        Err(err) => {
            tracing::warn!(error = %err, "the roles of the unmatched files could not be asked, they stay unclassified");
            return;
        }
    };

    for rule in response.rules {
        let known = rules
            .unmatched
            .rules
            .iter()
            .any(|r| r.pattern == rule.pattern);
        if rule.role != FileRole::Unclassified && !known {
            rules.unmatched.rules.push(rule);
        }
    }
    let mut promoted = files.to_vec();
    rules.promote_sources(&mut promoted);
    let map = rules.classify(&promoted);
    rules.unmatched.left.extend(
        sent.into_iter()
            .filter(|path| map.roles.get(path) == Some(&FileRole::Unclassified)),
    );
    if let Err(err) = rules.unmatched.save(repo_root) {
        tracing::warn!(error = %err, "the roles of the unmatched files could not be saved");
    }

    let left = map.files_with(FileRole::Unclassified).len();
    if left > 0 {
        tracing::warn!(
            left,
            not_sent,
            "files no role rule matches left unclassified"
        );
    }
}
