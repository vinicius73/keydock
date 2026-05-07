use std::net::SocketAddr;

use super::error::ConfigError;

pub(crate) const ENV_HTTP_LISTEN: &str = "KEYDOCK_HTTP_LISTEN";
pub(crate) const ENV_HTTP_METRICS_LISTEN: &str = "KEYDOCK_HTTP_METRICS_LISTEN";
pub(crate) const ENV_PATHS_DATA_DIR: &str = "KEYDOCK_PATHS_DATA_DIR";
pub(crate) const ENV_LOG_JSON: &str = "KEYDOCK_LOG_JSON";
pub(crate) const ENV_ROOT_KEY: &str = "KEYDOCK_ROOT_KEY";
pub(crate) const ENV_GC_INTERVAL_SECS: &str = "KEYDOCK_GC_INTERVAL_SECS";
pub(crate) const ENV_RATE_LIMIT_ENABLED: &str = "KEYDOCK_RATE_LIMIT_ENABLED";
pub(crate) const ENV_RATE_LIMIT_REQUESTS_PER_HOUR: &str = "KEYDOCK_RATE_LIMIT_REQUESTS_PER_HOUR";

/// Source of process environment values used by the config loader.
pub trait EnvSource {
    fn get(&self, name: &str) -> Result<Option<String>, ConfigError>;
}

#[derive(Debug, Clone, Copy)]
pub struct ProcessEnv;

impl EnvSource for ProcessEnv {
    fn get(&self, name: &str) -> Result<Option<String>, ConfigError> {
        match std::env::var(name) {
            Ok(value) => Ok(Some(value)),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(std::env::VarError::NotUnicode(_)) => Err(ConfigError::EnvNotUnicode {
                name: name.to_string(),
            }),
        }
    }
}

pub(crate) fn parse_env_value<T>(name: &'static str, value: &str) -> Result<T, ConfigError>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    value.parse().map_err(|e: T::Err| ConfigError::InvalidEnv {
        name: name.to_string(),
        reason: e.to_string(),
    })
}

pub(crate) fn parse_optional_socket_addr(
    name: &'static str,
    value: &str,
) -> Result<Option<SocketAddr>, ConfigError> {
    if value.trim().is_empty() {
        return Ok(None);
    }
    parse_env_value(name, value).map(Some)
}

pub(crate) fn parse_bool_env(name: &'static str, value: &str) -> Result<bool, ConfigError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(ConfigError::InvalidEnv {
            name: name.to_string(),
            reason: "expected true, false, 1, or 0".to_string(),
        }),
    }
}
