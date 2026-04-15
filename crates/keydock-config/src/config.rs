use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;
use tracing::instrument;

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

/// Filename written by [`write_init_config`] inside the instance directory.
pub const CONFIG_FILENAME: &str = "keydock.toml";

#[derive(Debug, Error)]
pub enum InitError {
    #[error("instance path is not a directory: {path}")]
    NotADirectory { path: PathBuf },

    #[error("config file already exists: {path} (use --force to overwrite)")]
    AlreadyExists { path: PathBuf },

    #[error("failed to serialize config to TOML: {0}")]
    Serialize(#[from] toml::ser::Error),

    #[error("failed to initialize instance at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to canonicalize data directory {path}: {source}")]
    Canonicalize {
        path: PathBuf,
        #[source]
        source: std::io::Error,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metrics_listen: Option<SocketAddr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathsConfig {
    /// Embedded database directory.
    pub data_dir: PathBuf,
}

fn default_listen() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 8080))
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
    #[instrument(skip_all, fields(path = %path.display()))]
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
    #[instrument(skip_all, fields(has_listen = listen.is_some(), has_data_dir = data_dir.is_some()))]
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

/// Creates `<instance_dir>/data`, writes `<instance_dir>/keydock.toml` with defaults and a new
/// `root_key`, and returns the config file path.
///
/// `paths.data_dir` is stored as an **absolute** path so `keydock serve -c …` works from any cwd.
#[instrument(skip_all, fields(instance_dir = %instance_dir.display(), force = force))]
pub fn write_init_config(instance_dir: &Path, force: bool) -> Result<PathBuf, InitError> {
    use std::fs;
    use std::io::Write;

    if instance_dir.exists() && !instance_dir.is_dir() {
        return Err(InitError::NotADirectory {
            path: instance_dir.to_path_buf(),
        });
    }

    fs::create_dir_all(instance_dir).map_err(|e| InitError::Io {
        path: instance_dir.to_path_buf(),
        source: e,
    })?;

    let config_path = instance_dir.join(CONFIG_FILENAME);
    if config_path.exists() && !force {
        return Err(InitError::AlreadyExists { path: config_path });
    }

    let data_dir_path = instance_dir.join("data");
    fs::create_dir_all(&data_dir_path).map_err(|e| InitError::Io {
        path: data_dir_path.clone(),
        source: e,
    })?;

    let data_dir_canonical =
        data_dir_path
            .canonicalize()
            .map_err(|source| InitError::Canonicalize {
                path: data_dir_path.clone(),
                source,
            })?;

    let config = Config {
        http: HttpConfig {
            listen: default_listen(),
            metrics_listen: None,
        },
        paths: PathsConfig {
            data_dir: data_dir_canonical,
        },
        log_json: false,
        root_key: default_root_key(),
    };

    let toml_str = toml::to_string_pretty(&config)?;

    let mut tmp = tempfile::NamedTempFile::new_in(instance_dir).map_err(|e| InitError::Io {
        path: instance_dir.to_path_buf(),
        source: e,
    })?;
    tmp.write_all(toml_str.as_bytes())
        .map_err(|e| InitError::Io {
            path: tmp.path().to_path_buf(),
            source: e,
        })?;
    tmp.flush().map_err(|e| InitError::Io {
        path: tmp.path().to_path_buf(),
        source: e,
    })?;
    tmp.as_file().sync_all().map_err(|e| InitError::Io {
        path: tmp.path().to_path_buf(),
        source: e,
    })?;

    tmp.persist(&config_path).map_err(|e| InitError::Io {
        path: config_path.clone(),
        source: e.error,
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&config_path, fs::Permissions::from_mode(0o600)).map_err(|e| {
            InitError::Io {
                path: config_path.clone(),
                source: e,
            }
        })?;
    }

    Ok(config_path)
}

/// HTTP-relevant, validated view of configuration (no secrets by default).
#[derive(Debug, Clone)]
pub struct ValidatedHttpConfig {
    pub listen: SocketAddr,
    pub metrics_listen: Option<SocketAddr>,
    pub log_json: bool,
}

impl ValidatedHttpConfig {
    #[instrument(skip_all, name = "ValidatedHttpConfig::from_config")]
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
    #[instrument(skip_all, name = "LoadedSecret::expose_bytes")]
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

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

    #[test]
    fn write_init_config_creates_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let instance = dir.path().join("instance");
        let config_path = write_init_config(&instance, false).expect("write_init_config");
        assert_eq!(config_path, instance.join(CONFIG_FILENAME));

        let loaded = Config::load_from_file(&config_path).expect("load");
        assert_eq!(loaded.http.listen, default_listen());
        assert_eq!(loaded.http.metrics_listen, None);
        assert_eq!(loaded.log_json, false);
        assert_eq!(
            loaded.paths.data_dir,
            instance.join("data").canonicalize().unwrap()
        );
    }

    #[test]
    fn write_init_config_fails_when_exists_without_force() {
        let dir = tempfile::tempdir().expect("tempdir");
        let instance = dir.path().join("instance");
        write_init_config(&instance, false).expect("first write");
        let err = write_init_config(&instance, false).unwrap_err();
        assert!(matches!(err, InitError::AlreadyExists { .. }));
    }

    #[rstest]
    #[case(true)]
    #[case(false)]
    fn write_init_config_force_overwrites(#[case] first_force: bool) {
        let dir = tempfile::tempdir().expect("tempdir");
        let instance = dir.path().join("instance");
        write_init_config(&instance, first_force).expect("first write");
        let path = write_init_config(&instance, true).expect("second write with force");
        let loaded = Config::load_from_file(&path).expect("load");
        assert!(!loaded.root_key.expose_bytes().is_empty());
    }
}
