use std::net::SocketAddr;
use std::path::PathBuf;

use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse config file {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
}

fn default_root_key() -> LoadedSecret {
    LoadedSecret(SecretString::from(format!(
        "{}-{}",
        uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4()
    )))
}

/// Full process configuration as loaded from file (before CLI merge).
#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub http: HttpConfig,
    pub paths: PathsConfig,
    /// Development-only toggle for JSON vs pretty logs (ADR: no env as primary; kept for ops convenience).
    #[serde(default)]
    pub log_json: bool,
    /// Root key for hashing API credentials at rest (HMAC-SHA256). Override in production.
    #[serde(default = "default_root_key")]
    pub root_key: LoadedSecret,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("http", &self.http)
            .field("paths", &self.paths)
            .field("log_json", &self.log_json)
            .field("root_key", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    #[serde(default = "default_listen")]
    pub listen: SocketAddr,

    /// Optional separate scrape listener; when `None`, metrics share the main HTTP server.
    #[serde(default)]
    pub metrics_listen: Option<SocketAddr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathsConfig {
    /// Embedded database directory.
    pub data_dir: PathBuf,
}

fn default_listen() -> SocketAddr {
    "127.0.0.1:8080"
        .parse()
        .expect("default listen address must parse")
}

impl Default for Config {
    fn default() -> Self {
        Self {
            http: HttpConfig {
                listen: default_listen(),
                metrics_listen: None,
            },
            paths: PathsConfig {
                data_dir: PathBuf::from("./data"),
            },
            log_json: false,
            root_key: default_root_key(),
        }
    }
}

impl Config {
    pub fn load_from_file(path: &std::path::Path) -> Result<Self, ConfigError> {
        let raw = std::fs::read_to_string(path).map_err(|source| ConfigError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        toml::from_str(&raw).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })
    }

    /// Merges CLI overrides (second wins over file), ADR precedence: file then CLI.
    pub fn merge_cli(mut self, listen: Option<SocketAddr>, data_dir: Option<PathBuf>) -> Self {
        if let Some(addr) = listen {
            self.http.listen = addr;
        }
        if let Some(dir) = data_dir {
            self.paths.data_dir = dir;
        }
        self
    }
}

/// HTTP-relevant, validated view of configuration (no secrets by default).
#[derive(Debug, Clone)]
pub struct ValidatedHttpConfig {
    pub listen: SocketAddr,
    pub metrics_listen: Option<SocketAddr>,
    pub log_json: bool,
}

impl ValidatedHttpConfig {
    pub fn from_config(cfg: &Config) -> Self {
        Self {
            listen: cfg.http.listen,
            metrics_listen: cfg.http.metrics_listen,
            log_json: cfg.log_json,
        }
    }
}

/// Secret string from config (never log; use [`LoadedSecret::expose_bytes`] only at startup).
#[derive(Clone)]
pub struct LoadedSecret(pub SecretString);

impl LoadedSecret {
    /// Raw UTF-8 bytes of the configured secret (for deriving [`keydock_domain::SigningKey`]).
    pub fn expose_bytes(&self) -> Vec<u8> {
        self.0.expose_secret().as_bytes().to_vec()
    }
}

impl Serialize for LoadedSecret {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.0.expose_secret())
    }
}

impl<'de> Deserialize<'de> for LoadedSecret {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(LoadedSecret(SecretString::from(s)))
    }
}

impl std::fmt::Debug for LoadedSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}
