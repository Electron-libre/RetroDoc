//! The business domains of the application, saved as `.retrodoc/cache/domains.yaml` (docs/ARCHITECTURE.md
//! §4 step 3), each with the resources, entities and files it holds. Distinct from
//! [`crate::model::Domain`], the final rendering model populated once features and use cases are attached.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::cache::{load_yaml, save_yaml, Artifact, CacheError};
use crate::model::Feature;

/// The slug of the domain that holds what no other domain took.
pub const UNCATEGORIZED_SLUG: &str = "uncategorized";

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
    pub fn save(&self, repo_root: &Path) -> Result<(), CacheError> {
        save_yaml(&Artifact::Domains.path(repo_root), self)
    }

    /// Loads a previously saved `domains.yaml`. Missing or unreadable:
    /// `None` (first run), not an error.
    #[must_use]
    pub fn load(repo_root: &Path) -> Option<Self> {
        load_yaml(&Artifact::Domains.path(repo_root))
    }

    /// The domains the docs publish: those at least one of `features` belongs to, the
    /// "uncategorized" bucket left out (EDM).
    pub fn published<'a>(
        &'a self,
        features: &[Feature],
    ) -> impl Iterator<Item = &'a DomainCluster> + 'a {
        let held = held_slugs(features);
        self.without_uncategorized()
            .filter(move |d| held.contains(&d.slug))
    }

    /// The domains no feature of `features` belongs to, the "uncategorized" bucket left out: the
    /// docs leave them out and the debt report lists them (EDM).
    pub fn featureless<'a>(
        &'a self,
        features: &[Feature],
    ) -> impl Iterator<Item = &'a DomainCluster> + 'a {
        let held = held_slugs(features);
        self.without_uncategorized()
            .filter(move |d| !held.contains(&d.slug))
    }

    /// The source files of the "uncategorized" bucket: code no domain holds.
    pub fn uncategorized_paths(&self) -> impl Iterator<Item = &PathBuf> {
        self.domains
            .iter()
            .filter(|d| d.slug == UNCATEGORIZED_SLUG)
            .flat_map(|d| &d.paths)
    }

    /// Every domain but the "uncategorized" bucket.
    pub fn without_uncategorized(&self) -> impl Iterator<Item = &DomainCluster> {
        self.domains.iter().filter(|d| d.slug != UNCATEGORIZED_SLUG)
    }
}

/// The slugs of the domains `features` belong to.
fn held_slugs(features: &[Feature]) -> BTreeSet<String> {
    features.iter().map(|f| f.domain_slug.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_old_domain_map_with_sub_domains_loads_and_is_saved_without_them() {
        let dir = tempfile::tempdir().unwrap();
        let path = Artifact::Domains.path(dir.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            "domains:\n- slug: billing\n  name: Billing\n  description: Bills.\n  paths: [app/invoice.rb]\n  \
             sub_domains:\n  - slug: payment\n    name: Payment\n    description: Pays.\n    paths: [app/pay.rb]\n",
        )
        .unwrap();

        let map = DomainMap::load(dir.path()).expect("an old domains.yaml still loads");
        assert_eq!(map.domains[0].slug, "billing");
        map.save(dir.path()).unwrap();
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(!saved.contains("sub_domains"), "{saved}");
    }
}
