use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use tracing::instrument;

use super::env::{
    ENV_GC_INTERVAL_SECS, ENV_HTTP_LISTEN, ENV_HTTP_METRICS_LISTEN, ENV_LOG_JSON,
    ENV_PATHS_DATA_DIR, ENV_RATE_LIMIT_ENABLED, ENV_RATE_LIMIT_REQUESTS_PER_HOUR, ENV_ROOT_KEY,
    EnvSource, ProcessEnv, parse_bool_env, parse_env_value, parse_optional_socket_addr,
};
use super::error::ConfigError;
use super::secret::{RawRootKey, resolve_root_key};
use super::{Config, GcConfig, HttpConfig, PathsConfig, RateLimitConfig};

#[derive(Debug, Clone)]
pub struct ConfigSources<'a> {
    pub config_path: Option<&'a Path>,
    pub listen: Option<SocketAddr>,
    pub data_dir: Option<PathBuf>,
    /// When `false`, `KEYDOCK_*` environment overrides are not applied.
    /// TOML-declared indirection (`root_key = { env/file }`) is unaffected.
    pub apply_env: bool,
}

#[derive(Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    #[serde(default)]
    http: HttpConfig,
    #[serde(default)]
    paths: PathsConfig,
    #[serde(default)]
    log_json: bool,
    #[serde(default)]
    root_key: Option<RawRootKey>,
    #[serde(default)]
    gc: GcConfig,
    #[serde(default)]
    rate_limit: RateLimitConfig,
}

impl RawConfig {
    fn merge_env(&mut self, env: &dyn EnvSource) -> Result<(), ConfigError> {
        if let Some(value) = env.get(ENV_HTTP_LISTEN)? {
            self.http.listen = parse_env_value(ENV_HTTP_LISTEN, &value)?;
        }
        if let Some(value) = env.get(ENV_HTTP_METRICS_LISTEN)? {
            self.http.metrics_listen = parse_optional_socket_addr(ENV_HTTP_METRICS_LISTEN, &value)?;
        }
        if let Some(value) = env.get(ENV_PATHS_DATA_DIR)? {
            self.paths.data_dir = PathBuf::from(value);
        }
        if let Some(value) = env.get(ENV_LOG_JSON)? {
            self.log_json = parse_bool_env(ENV_LOG_JSON, &value)?;
        }
        if let Some(value) = env.get(ENV_ROOT_KEY)? {
            self.root_key = Some(RawRootKey::Inline(value));
        }
        if let Some(value) = env.get(ENV_GC_INTERVAL_SECS)? {
            self.gc.interval_secs = parse_env_value(ENV_GC_INTERVAL_SECS, &value)?;
        }
        if let Some(value) = env.get(ENV_RATE_LIMIT_ENABLED)? {
            self.rate_limit.enabled = parse_bool_env(ENV_RATE_LIMIT_ENABLED, &value)?;
        }
        if let Some(value) = env.get(ENV_RATE_LIMIT_REQUESTS_PER_HOUR)? {
            self.rate_limit.requests_per_hour =
                parse_env_value(ENV_RATE_LIMIT_REQUESTS_PER_HOUR, &value)?;
        }
        Ok(())
    }

    fn merge_cli(&mut self, listen: Option<SocketAddr>, data_dir: Option<PathBuf>) {
        if let Some(addr) = listen {
            self.http.listen = addr;
        }
        if let Some(dir) = data_dir {
            self.paths.data_dir = dir;
        }
    }

    fn resolve(
        self,
        config_dir: Option<&Path>,
        env: &dyn EnvSource,
    ) -> Result<Config, ConfigError> {
        let root_key = resolve_root_key(self.root_key, config_dir, env)?;
        Ok(Config {
            http: self.http,
            paths: self.paths,
            log_json: self.log_json,
            root_key,
            gc: self.gc,
            rate_limit: self.rate_limit,
        })
    }
}

impl Config {
    /// Loads a TOML file and resolves `root_key` indirection declared inside that file,
    /// using the real process environment for `root_key = { env = "..." }` indirection.
    ///
    /// Does not apply process-wide `KEYDOCK_*` overrides. Use
    /// [`Config::load_from_sources`] for the full runtime precedence chain.
    pub fn load_from_file(path: &Path) -> Result<Self, ConfigError> {
        Self::load_from_file_with_env(path, &ProcessEnv)
    }

    /// Like [`Config::load_from_file`] but accepts a custom [`EnvSource`] for
    /// resolving `root_key = { env = "..." }` indirection declared in the TOML file.
    ///
    /// Does not apply process-wide `KEYDOCK_*` overrides.
    #[instrument(skip_all, fields(path = %path.display()))]
    pub fn load_from_file_with_env(path: &Path, env: &dyn EnvSource) -> Result<Self, ConfigError> {
        let raw = fs::read_to_string(path).map_err(|source| ConfigError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let raw_config: RawConfig = toml::from_str(&raw).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
        raw_config.resolve(path.parent(), env)
    }

    /// Loads runtime config with precedence: built-in defaults, TOML file,
    /// `KEYDOCK_*` environment variables, then CLI sources.
    #[instrument(skip_all, fields(has_config = sources.config_path.is_some()))]
    pub fn load_from_sources(sources: &ConfigSources<'_>) -> Result<Self, ConfigError> {
        Self::load_from_sources_with_env(sources, &ProcessEnv)
    }

    #[instrument(skip_all, fields(has_config = sources.config_path.is_some()))]
    pub fn load_from_sources_with_env(
        sources: &ConfigSources<'_>,
        env: &dyn EnvSource,
    ) -> Result<Self, ConfigError> {
        let (mut raw_config, config_dir) = match sources.config_path {
            Some(path) => {
                let raw = fs::read_to_string(path).map_err(|source| ConfigError::Io {
                    path: path.to_path_buf(),
                    source,
                })?;
                let config = toml::from_str(&raw).map_err(|source| ConfigError::Parse {
                    path: path.to_path_buf(),
                    source,
                })?;
                (config, path.parent().map(Path::to_path_buf))
            }
            None => (RawConfig::default(), None),
        };

        if sources.apply_env {
            raw_config.merge_env(env)?;
        }
        raw_config.merge_cli(sources.listen, sources.data_dir.clone());
        raw_config.resolve(config_dir.as_deref(), env)
    }
}
