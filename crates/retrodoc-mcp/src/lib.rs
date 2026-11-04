//! Read-only access to the generated documentation for LLM agents (see
//! `issues/done/mcp_server.md`). The consuming agent is the LLM, so nothing here
//! calls one: the crate reads the artifacts saved by `generate` and the
//! collected Markdown docs, and answers from them deterministically.
//!
//! [`GeneratedDocs`] holds the loaded artifacts as typed data; [`Docs`] answers
//! the seven tools from it in
//! Markdown; [`serve_stdio`] puts them behind an MCP server (`retrodoc mcp`).
//! The lexical search ([`SearchIndex`]) is also exposed alone by `retrodoc
//! search`, to judge retrieval quality.

pub mod corpus;
pub mod debt;
pub mod freshness;
pub mod read_model;
pub mod search;
pub mod server;
pub mod source;
pub mod tools;

pub use corpus::{build_entries, Entry, EntryKind};
pub use read_model::GeneratedDocs;
pub use retrodoc_ingest::ExistingDoc;
pub use search::{Found, Hit, SearchIndex};
pub use server::{serve_stdio, DocsServer, McpError};
pub use tools::Docs;
