//! Domains (docs/ARCHITECTURE.md §4 step 3): the business domains of the application, saved as the intermediate
//! `domains.yaml` artifact, the input of the features pass. Distinct from
//! `retrodoc_core::model::Domain`, the final rendering model populated once features and use cases are
//! attached.
//!
//! `generate` builds them from the surface (`from_surface`, ADR 0020) or from the bounded contexts
//! (`contexts::domains_from_contexts`, ADR 0029). The clustering of the directory summaries of the removed
//! `files` path, with its coverage of every file, is gone (ADR 0035): the files no behaviour reaches are
//! reported as technical support instead (`behaviour_coverage`).

pub(crate) use retrodoc_core::artifacts::domains::{DomainCluster, DomainMap};

#[cfg(test)]
mod corrections_tests;
mod from_surface;
mod prompt;
#[cfg(test)]
mod tests;

use self::prompt::layer_named_domains;

pub use self::from_surface::build_domains_from_surface;

pub(crate) use retrodoc_core::artifacts::domains::UNCATEGORIZED_SLUG;
