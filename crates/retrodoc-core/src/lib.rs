//! `retrodoc-core`: shared domain model and `retrodoc.toml` configuration.

pub mod config;
pub mod model;

pub use config::{Config, ConfigError, CONFIG_FILE_NAME};
