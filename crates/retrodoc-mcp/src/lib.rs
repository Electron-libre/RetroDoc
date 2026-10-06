//! Read-only access to the generated documentation for LLM agents (see
//! `issues/mcp_server.md`). The consuming agent is the LLM, so nothing here
//! calls one: the crate reads the artifacts saved by `generate` and the
//! collected Markdown docs, and answers from them deterministically.
//!
//! [`Docs`] holds the loaded artifacts and answers the five tools in
//! Markdown; [`serve_stdio`] puts them behind an MCP server (`retrodoc mcp`).
//! The lexical search ([`SearchIndex`]) is also exposed alone by `retrodoc
//! search`, to judge retrieval quality.

pub mod bm25;
pub mod corpus;
pub mod freshness;
pub mod search;
pub mod server;
pub mod tools;

pub use corpus::{build_entries, Entry, EntryKind};
pub use search::{Hit, SearchIndex};
pub use server::{serve_stdio, DocsServer, McpError};
pub use tools::Docs;
