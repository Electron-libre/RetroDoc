//! The names of the passes `generate` tracks (`UsageTracker::in_pass`, `set_pass` in the commands of one
//! pass), shared by the call sites, the list the estimate and the providers are drawn from, the estimate and
//! the provider lookups (`PassProviders::get`, `Run::concurrency`), so a name can't drift from one to the other.

pub const SOURCES: &str = "sources";
pub const BRIEF: &str = "brief";
pub const ROLES: &str = "roles";
pub const BUSINESS_FILES: &str = "business-files";
pub const CONTEXTS: &str = "contexts";
/// The joint reading of the business files, tracked as a pass of its own; it has no provider of its own and
/// calls those of the glossary and the entry points.
pub const BUSINESS_READ: &str = "business-read";
pub const GLOSSARY: &str = "glossary";
pub const ENTRY_POINTS: &str = "entry-points";
pub const ACTORS: &str = "actors";
pub const DOMAINS: &str = "domains";
pub const FEATURES: &str = "features";
pub const USE_CASES: &str = "use-cases";
pub const RULES: &str = "rules";
pub const CONFIDENCE: &str = "confidence";
pub const RECONCILE: &str = "reconcile";
