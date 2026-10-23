//! What the rules pass leaves out on a judgement of the LLM, kept so that a doubtful case can be
//! checked: the features set aside as describing tests (they get no rule, so a wrong answer loses
//! all of them) and the rules dropped as noise, each with its reason. Saved as
//! `.retrodoc/cache/rules-set-aside.yaml` and listed in the debt report. The rules merged because
//! several features state them are not here: `rules-stated.yaml` still holds every copy.

use serde::{Deserialize, Serialize};

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
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.features.is_empty() && self.rules.is_empty()
    }

    /// What was set aside for the feature `slug`.
    pub(super) fn of_feature(&self, slug: &str) -> Self {
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

    pub(super) fn extend(&mut self, other: Self) {
        self.features.extend(other.features);
        self.rules.extend(other.rules);
    }
}
