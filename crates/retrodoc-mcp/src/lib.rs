//! Read-only access to the generated documentation for LLM agents (see
//! `issues/mcp_server.md`). The consuming agent is the LLM, so nothing here
//! calls one: the crate reads the artifacts saved by `generate` and the
//! collected Markdown docs, and answers from them deterministically.
//!
//! Delivered so far: the lexical search ([`SearchIndex`]), which the
//! `retrodoc search` command exposes to judge retrieval quality; the MCP
//! tools and server come next.

pub mod bm25;
pub mod corpus;
pub mod search;

pub use corpus::{build_entries, Entry, EntryKind};
pub use search::{Hit, SearchIndex};
