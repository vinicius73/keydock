#![forbid(unsafe_code)]

//! Process configuration: TOML + CLI overrides.

pub mod cli;
pub mod config;

pub use cli::{CliError, Command, InitArgs, ServeArgs, parse};
pub use config::{
    CONFIG_FILENAME, Config, ConfigError, GcConfig, HttpConfig, InitError, LoadedSecret,
    PathsConfig, ValidatedHttpConfig, write_init_config,
};
