//! The read-only, LLM-free MCP server over the generated documentation (ADR 0016): the read-only
//! tools of `retrodoc-docs` behind the `rmcp` SDK, over stdio (`retrodoc mcp`). What the tools
//! answer lives in `retrodoc-docs` (ADR 0054), shared with the `ask` agent.

pub mod server;

pub use server::{serve_stdio, DocsServer, McpError};
