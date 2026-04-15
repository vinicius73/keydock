#![forbid(unsafe_code)]

//! Process configuration: TOML + CLI overrides.

pub mod cli;
pub mod config;

pub use cli::{CliError, Command, ServeArgs, parse};
pub use config::{Config, ConfigError, HttpConfig, LoadedSecret, PathsConfig, ValidatedHttpConfig};
