//! `retrodoc-core`: shared domain model, `retrodoc.toml` configuration and the layout of the cache
//! the pipeline writes and the readers of the docs read.

pub mod artifacts;
pub mod cache;
pub mod config;
pub mod model;
pub mod text;

pub use config::{Config, ConfigError, CONFIG_FILE_NAME};
