//! The reading of the generated documentation, without any LLM call (ADR 0054): the crate reads the
//! artifacts saved by `generate` and the collected Markdown docs, and answers from them
//! deterministically.
//!
//! [`GeneratedDocs`] holds the loaded artifacts as typed data; [`Docs`] answers the seven read-only
//! tools from it in Markdown, and [`catalog`] gives these tools as data and a call by name, so the
//! MCP server of `retrodoc-mcp` and the `ask` agent of `retrodoc-agent` (ADR 0047) offer the same
//! tools; [`Docs::source_named`] and [`Docs::warnings_of`] check the sources of an answer. The
//! lexical search ([`SearchIndex`]) is also exposed alone by `retrodoc search`, to judge retrieval
//! quality. The stores of the human reviews ([`reviews`]) and edits ([`edits`]), the run history
//! ([`history`]), the debt view ([`debt`]) and the audit ([`audit`]) serve the web interface and
//! the CLI.

pub mod audit;
pub mod catalog;
pub mod corpus;
pub mod debt;
pub mod edits;
pub mod freshness;
pub mod history;
pub mod read_model;
pub mod report;
pub mod reviews;
pub mod search;
pub mod source;
pub mod store;
pub mod tools;

pub use catalog::{read_tools, CallError, ReadTool};
pub use corpus::{build_entries, Entry, EntryKind};
pub use read_model::{GeneratedDocs, Sections, Uncovered};
pub use report::ReportInputs;
pub use retrodoc_ingest::ExistingDoc;
pub use search::{Found, Hit, SearchIndex};
pub use tools::{Docs, Source, SourceKind, Warning};
