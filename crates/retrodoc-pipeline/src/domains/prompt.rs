use super::{DomainMap, UNCATEGORIZED_SLUG};

/// Words that name a technical layer rather than a business domain.
pub(super) const LAYER_WORDS: &[&str] = &[
    "presentation",
    "presenter",
    "presenters",
    "service",
    "services",
    "model",
    "models",
    "controller",
    "controllers",
    "view",
    "views",
    "helper",
    "helpers",
    "util",
    "utils",
    "utility",
    "utilities",
    "infrastructure",
    "api",
    "job",
    "jobs",
    "layer",
    "lib",
    "core",
    "common",
    "shared",
    "misc",
    "other",
];

/// Slugs of the domains whose name is made only of layer
/// words (`presentation-layer`, `services`): the technical split the risks of PRODUCT.md
/// warns about. Reported, not repaired.
pub(super) fn layer_named_domains(map: &DomainMap) -> Vec<String> {
    let is_layer = |slug: &str| {
        slug.split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .all(|w| LAYER_WORDS.contains(&w.to_lowercase().as_str()))
    };
    let mut found = Vec::new();
    for domain in &map.domains {
        if domain.slug != UNCATEGORIZED_SLUG && is_layer(&domain.slug) {
            found.push(domain.slug.clone());
        }
    }
    found
}
