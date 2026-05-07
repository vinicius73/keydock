#![forbid(unsafe_code)]

//! Process configuration: TOML + environment + CLI overrides.

pub mod cli;
pub mod config;

pub use cli::{CliError, Command, InitArgs, ServeArgs, parse};
pub use config::{
    CONFIG_FILENAME, Config, ConfigError, ConfigSources, EnvSource, GcConfig, HttpConfig,
    InitError, LoadedSecret, PathsConfig, ProcessEnv, RateLimitConfig, write_init_config,
};
