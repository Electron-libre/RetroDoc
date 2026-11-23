//! The artifacts of `.retrodoc/cache/` the readers of the generated docs load, each with its `load` and
//! `save` (ADR 0051): the pipeline computes and saves them, the readers read them.

pub mod coverage;
pub mod domains;
pub mod estimate;
pub mod glossary;
pub mod items;
pub mod scope;
pub mod set_aside;
pub mod source_hashes;

pub use coverage::BehaviourCoverage;
pub use domains::{DomainCluster, DomainMap};
pub use glossary::{Entity, Glossary, MergedEntity};
pub use items::{
    load_features, load_rules, load_technical_features, load_use_cases, save_features, save_rules,
    save_use_cases,
};
pub use scope::Scope;
pub use set_aside::{AsideFeature, AsideRule, SetAside};
pub use source_hashes::SourceHashes;
