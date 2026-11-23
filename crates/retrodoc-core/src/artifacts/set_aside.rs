//! What the rules pass leaves out on a judgement of the LLM, kept so that a doubtful case can be
//! checked: the features set aside as describing tests (they get no rule, so a wrong answer loses
//! all of them) and the rules dropped as noise, each with its reason. Saved as
//! `.retrodoc/cache/rules-set-aside.yaml` and listed in the debt report. The rules merged because
//! several features state them are not here: `rules-stated.yaml` still holds every copy.
//! A rule dropped as noise is kept whole, so that `keep_rules` of `retrodoc.toml` puts it back
//! without asking the LLM again.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::cache::{load_yaml, save_yaml, Artifact, CacheError};
use crate::model::BusinessRule;

/// A feature that got no rule because it describes how the application is tested.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AsideFeature {
    pub slug: String,
    pub name: String,
    pub description: String,
    /// Why, in words.
    pub reason: String,
}

/// A rule dropped by the judgement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AsideRule {
    /// The slug it had (`<feature-slug>-<n>`).
    pub slug: String,
    pub feature: String,
    pub statement: String,
    /// The class the LLM gave it: `storage`, `screen`, `test` or `technical`.
    pub class: String,
    /// The rule whole, with its evidence, so that `keep_rules` can put it back with no call. `None`
    /// in a file saved before it was kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<BusinessRule>,
}

/// Everything the pass set aside, by feature.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SetAside {
    #[serde(default)]
    pub features: Vec<AsideFeature>,
    #[serde(default)]
    pub rules: Vec<AsideRule>,
}

impl SetAside {
    /// What the last run of the rules pass set aside. Missing or unreadable: `None`.
    #[must_use]
    pub fn load(repo_root: &Path) -> Option<Self> {
        load_yaml(&Artifact::RulesSetAside.path(repo_root))
    }

    /// # Errors
    ///
    /// Returns an error if the file can't be written.
    pub fn save(&self, repo_root: &Path) -> Result<(), CacheError> {
        save_yaml(&Artifact::RulesSetAside.path(repo_root), self)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.features.is_empty() && self.rules.is_empty()
    }

    /// What was set aside for the feature `slug`.
    #[must_use]
    pub fn of_feature(&self, slug: &str) -> Self {
        Self {
            features: self
                .features
                .iter()
                .filter(|f| f.slug == slug)
                .cloned()
                .collect(),
            rules: self
                .rules
                .iter()
                .filter(|r| r.feature == slug)
                .cloned()
                .collect(),
        }
    }

    /// Drops the features the user put back (`keep_features`): they are not set aside, even while
    /// their rules are not stated yet.
    pub fn forget_features(&mut self, keep_features: &[String]) {
        self.features.retain(|f| !keep_features.contains(&f.slug));
    }

    pub fn extend(&mut self, other: Self) {
        self.features.extend(other.features);
        self.rules.extend(other.rules);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saved_set_aside_loads_back() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(SetAside::load(dir.path()), None);
        let aside = SetAside {
            features: vec![AsideFeature {
                slug: "run-tests".into(),
                name: "Run tests".into(),
                description: "Runs the test suite.".into(),
                reason: "describes the tests".into(),
            }],
            rules: Vec::new(),
        };
        aside.save(dir.path()).unwrap();
        assert_eq!(SetAside::load(dir.path()), Some(aside));
    }
}
