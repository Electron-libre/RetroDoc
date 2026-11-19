//! The artifacts of `.retrodoc/cache/` the readers of the generated docs load, each with its `load` and
//! `save` (ADR 0051): the pipeline computes and saves them, the readers read them.

pub mod coverage;
pub mod domains;
pub mod glossary;
pub mod scope;
pub mod source_hashes;

pub use coverage::BehaviourCoverage;
pub use domains::{DomainCluster, DomainMap};
pub use glossary::{Entity, Glossary, MergedEntity};
pub use scope::Scope;
pub use source_hashes::SourceHashes;
