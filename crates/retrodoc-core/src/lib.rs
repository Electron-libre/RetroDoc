//! `retrodoc-core`: shared domain model, `retrodoc.toml` configuration and the layout of the cache
//! the pipeline writes and the readers of the docs read.

pub mod artifacts;
pub mod bm25;
pub mod cache;
pub mod config;
pub mod model;
pub mod reconcile;
pub mod report;
pub mod text;

// The moved modules name core by its crate name, as every other crate does.
extern crate self as retrodoc_core;

pub use config::{Config, ConfigError, CONFIG_FILE_NAME};
