//! The reference of one repository: what the product really does, written by hand (or from its user
//! docs), that the generated docs are compared with.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::PipelineError;

/// `benchmark/<repo>/reference.yaml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reference {
    /// Public repository, e.g. its GitHub URL.
    pub repository: String,
    /// The commit the reference was written for: the benchmark clones this one.
    pub commit: String,
    /// What the product is for, in two sentences.
    pub purpose: String,
    pub actors: Vec<String>,
    pub domains: Vec<ReferenceDomain>,
    /// A sample of the business rules the product enforces, one sentence each.
    #[serde(default)]
    pub rules: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReferenceDomain {
    pub name: String,
    pub description: String,
    pub features: Vec<ReferenceFeature>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReferenceFeature {
    pub name: String,
    pub description: String,
    /// A sample of the use cases of the feature, by name (not exhaustive).
    #[serde(default)]
    pub use_cases: Vec<String>,
}

impl Reference {
    /// Reads and checks a reference file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file can't be read or parsed, or if it is unusable (see
    /// [`Reference::check`]).
    pub fn load(path: &Path) -> Result<Self, PipelineError> {
        let raw = std::fs::read_to_string(path).map_err(|source| PipelineError::ArtifactIo {
            path: path.to_path_buf(),
            source,
        })?;
        let reference: Self =
            serde_yaml::from_str(&raw).map_err(|error| PipelineError::InvalidReference {
                path: path.to_path_buf(),
                reason: error.to_string(),
            })?;
        reference.check(path)?;
        Ok(reference)
    }

    /// A reference without domains, or with a domain without features, or with an empty name can't be
    /// scored: say so rather than report a recall of 0.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::InvalidReference`] naming the first problem found.
    pub fn check(&self, path: &Path) -> Result<(), PipelineError> {
        let invalid = |reason: String| PipelineError::InvalidReference {
            path: path.to_path_buf(),
            reason,
        };
        if self.commit.trim().is_empty() {
            return Err(invalid("`commit` is empty".into()));
        }
        if self.domains.is_empty() {
            return Err(invalid("no domain".into()));
        }
        for domain in &self.domains {
            if domain.name.trim().is_empty() {
                return Err(invalid("a domain has no name".into()));
            }
            if domain.features.is_empty() {
                return Err(invalid(format!("domain `{}` has no feature", domain.name)));
            }
            if domain.features.iter().any(|f| f.name.trim().is_empty()) {
                return Err(invalid(format!(
                    "a feature of domain `{}` has no name",
                    domain.name
                )));
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn feature_count(&self) -> usize {
        self.domains.iter().map(|d| d.features.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const YAML: &str = "
repository: https://github.com/example/shop
commit: abc123
purpose: Sells things. Ships them.
actors: [Customer, Clerk]
domains:
  - name: Ordering
    description: Placing orders.
    features:
      - name: Checkout
        description: Pay for a cart.
        use_cases: [Pay by card]
rules:
  - An order needs at least one item.
";

    fn load(yaml: &str) -> Result<Reference, PipelineError> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("reference.yaml");
        std::fs::write(&path, yaml).unwrap();
        Reference::load(&path)
    }

    #[test]
    fn loads_a_reference() {
        let reference = load(YAML).unwrap();
        assert_eq!(reference.commit, "abc123");
        assert_eq!(reference.actors, ["Customer", "Clerk"]);
        assert_eq!(reference.feature_count(), 1);
        assert_eq!(reference.domains[0].features[0].use_cases, ["Pay by card"]);
        assert_eq!(reference.rules.len(), 1);
    }

    #[test]
    fn rules_and_use_cases_are_optional() {
        let yaml = YAML
            .replace("        use_cases: [Pay by card]\n", "")
            .replace("rules:\n  - An order needs at least one item.\n", "");
        let reference = load(&yaml).unwrap();
        assert_eq!(reference.rules, Vec::<String>::new());
        assert_eq!(
            reference.domains[0].features[0].use_cases,
            Vec::<String>::new()
        );
    }

    #[test]
    fn refuses_a_reference_that_cannot_be_scored() {
        let no_domain = YAML.split("domains:").next().unwrap().to_string() + "domains: []\n";
        let error = load(&no_domain).unwrap_err().to_string();
        assert!(error.contains("no domain"), "{error}");

        let no_feature = YAML.replace(
            "    features:\n      - name: Checkout\n        description: Pay for a cart.\n        use_cases: [Pay by card]\n",
            "    features: []\n",
        );
        let error = load(&no_feature).unwrap_err().to_string();
        assert!(error.contains("`Ordering` has no feature"), "{error}");

        let error = load(&YAML.replace("commit: abc123", "commit: ''"))
            .unwrap_err()
            .to_string();
        assert!(error.contains("`commit` is empty"), "{error}");
    }

    #[test]
    fn a_missing_or_malformed_file_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(Reference::load(&dir.path().join("nope.yaml")).is_err());
        assert!(load("domains: 3").is_err());
    }
}
