//! Domains (docs/ARCHITECTURE.md §4 step 3): the business domains of the application, saved as the intermediate
//! `domains.yaml` artifact, the input of the features pass. Distinct from
//! `retrodoc_core::model::Domain`, the final rendering model populated once features and use cases are
//! attached.
//!
//! `generate` builds them from the surface (`from_surface`, ADR 0020) or from the bounded contexts
//! (`contexts::domains_from_contexts`, ADR 0029). The clustering of the directory summaries of the removed
//! `files` path, with its coverage of every file, is gone (ADR 0035): the files no behaviour reaches are
//! reported as technical support instead (`behaviour_coverage`).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::artifact::{load_yaml, save_yaml, Artifact};
use crate::error::PipelineError;

mod from_surface;
mod prompt;
#[cfg(test)]
mod tests;

use self::prompt::layer_named_domains;

pub use self::from_surface::build_domains_from_surface;

pub(crate) const UNCATEGORIZED_SLUG: &str = "uncategorized";

/// Intermediate artifact (docs/ARCHITECTURE.md §4 step 3): the business domains of the application, each with the
/// resources, entities and files it holds.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DomainMap {
    pub domains: Vec<DomainCluster>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DomainCluster {
    pub slug: String,
    pub name: String,
    pub description: String,
    /// Source files assigned to this domain.
    #[serde(default)]
    pub paths: Vec<PathBuf>,
    /// Domains built from the surface (`domains_from = "surface"`): the
    /// resources (entry points grouped by resource) and the entities placed
    /// here.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resources: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entities: Vec<String>,
    /// Domains built from the bounded contexts (`domains_from = "contexts"`): the entry points of
    /// the domain are those of its `paths`, not those of the resources it names, since a resource
    /// can be served from the files of several contexts.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub by_paths: bool,
    /// With `by_paths`: ids (`<file>#<name>`) of the routes declared in a file of another domain that
    /// this domain serves…
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub routed_in: BTreeSet<String>,
    /// …and of the routes of its own files that another domain (of the map, in the run or not) serves.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub routed_out: BTreeSet<String>,
}

impl DomainMap {
    /// Persists the clustering as `.retrodoc/cache/domains.yaml` (docs/ARCHITECTURE.md
    /// §4 step 3): a human-reviewable intermediate artifact, the basis for
    /// the next pipeline pass (features, docs/ARCHITECTURE.md §4 step 4).
    ///
    /// # Errors
    ///
    /// Returns an error if `.retrodoc/cache/` can't be created, the file
    /// can't be written, or serialization to YAML fails.
    pub fn save(&self, repo_root: &Path) -> Result<(), PipelineError> {
        save_yaml(&Artifact::Domains.path(repo_root), self)?;
        Ok(())
    }

    /// Loads a previously saved `domains.yaml`. Missing or unreadable:
    /// `None` (first run), not an error.
    #[must_use]
    pub fn load(repo_root: &Path) -> Option<Self> {
        load_yaml(&Artifact::Domains.path(repo_root))
    }
}
