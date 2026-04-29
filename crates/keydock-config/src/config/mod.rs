use std::net::SocketAddr;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub(crate) mod env;
pub(crate) mod error;
pub(crate) mod init;
pub(crate) mod loader;
pub(crate) mod secret;

pub use env::{EnvSource, ProcessEnv};
pub use error::{ConfigError, InitError};
pub use init::{CONFIG_FILENAME, write_init_config};
pub use loader::ConfigSources;
pub use secret::LoadedSecret;

/// Full process configuration after file, environment, and CLI layers are resolved.
#[derive(Clone, Serialize)]
pub struct Config {
    pub http: HttpConfig,
    pub paths: PathsConfig,
    /// Development-only toggle for JSON vs pretty logs (ADR: no env as primary; kept for ops convenience).
    pub log_json: bool,
    /// Root key for hashing API credentials at rest (HMAC-SHA256). Override in production.
    pub root_key: LoadedSecret,
    /// Background garbage collection (expired keys in the `data` keyspace).
    pub gc: GcConfig,
    /// HTTP rate limiting (fixed window per client IP).
    pub rate_limit: RateLimitConfig,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("http", &self.http)
            .field("paths", &self.paths)
            .field("log_json", &self.log_json)
            .field("root_key", &"[REDACTED]")
            .field("gc", &self.gc)
            .field("rate_limit", &self.rate_limit)
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

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            listen: default_listen(),
            metrics_listen: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathsConfig {
    /// Embedded database directory.
    pub data_dir: PathBuf,
}

impl Default for PathsConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("./data"),
        }
    }
}

/// TTL sweeper: periodically deletes expired key entries from storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcConfig {
    /// Interval between full scans of the `data` keyspace (seconds).
    #[serde(default = "default_gc_interval_secs")]
    pub interval_secs: u64,
}

fn default_gc_interval_secs() -> u64 {
    60
}

impl Default for GcConfig {
    fn default() -> Self {
        Self {
            interval_secs: default_gc_interval_secs(),
        }
    }
}

/// Fixed-window rate limiting per client IP (HTTP edge).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_requests_per_hour")]
    pub requests_per_hour: u64,
}

fn default_requests_per_hour() -> u64 {
    1000
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            requests_per_hour: default_requests_per_hour(),
        }
    }
}

pub(crate) fn default_listen() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 8080))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::net::SocketAddr;
    use std::path::{Path, PathBuf};

    use pretty_assertions::assert_eq;
    use rstest::rstest;
    use secrecy::SecretString;

    use super::env::{
        ENV_GC_INTERVAL_SECS, ENV_HTTP_LISTEN, ENV_LOG_JSON, ENV_PATHS_DATA_DIR,
        ENV_RATE_LIMIT_ENABLED, ENV_RATE_LIMIT_REQUESTS_PER_HOUR, ENV_ROOT_KEY,
    };
    use super::secret::LoadedSecret;
    use super::*;

    #[derive(Default)]
    struct MapEnv {
        vars: BTreeMap<String, String>,
    }

    impl MapEnv {
        fn with(mut self, name: &str, value: &str) -> Self {
            self.vars.insert(name.to_string(), value.to_string());
            self
        }
    }

    impl EnvSource for MapEnv {
        fn get(&self, name: &str) -> Result<Option<String>, ConfigError> {
            Ok(self.vars.get(name).cloned())
        }
    }

    fn write_config(dir: &Path, contents: &str) -> PathBuf {
        let path = dir.join(CONFIG_FILENAME);
        fs::write(&path, contents).expect("write test config");
        path
    }

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
        assert_eq!(loaded.gc.interval_secs, 60);
        assert_eq!(loaded.rate_limit.enabled, false);
        assert_eq!(loaded.rate_limit.requests_per_hour, 1000);
    }

    #[test]
    fn write_init_config_fails_when_exists_without_force() {
        let dir = tempfile::tempdir().expect("tempdir");
        let instance = dir.path().join("instance");
        write_init_config(&instance, false).expect("first write");
        let err = write_init_config(&instance, false).unwrap_err();
        assert_eq!(matches!(err, InitError::AlreadyExists { .. }), true);
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
        assert_eq!(loaded.root_key.expose_bytes().is_empty(), false);
    }

    #[rstest]
    #[case::bare_string(r#"root_key = "inline-secret""#)]
    #[case::value_table(r#"root_key = { value = "inline-secret" }"#)]
    fn load_from_file_accepts_root_key_inline_forms(#[case] toml: &str) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_config(dir.path(), toml);

        let loaded = Config::load_from_file(&path).expect("load");

        assert_eq!(loaded.root_key.expose_bytes(), b"inline-secret".to_vec());
    }

    #[test]
    fn load_from_file_accepts_root_key_env_source() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_config(
            dir.path(),
            r#"
root_key = { env = "CUSTOM_ROOT_KEY" }
"#,
        );
        let env = MapEnv::default().with("CUSTOM_ROOT_KEY", "env-secret");

        let loaded = Config::load_from_file_with_env(&path, &env).expect("load");

        assert_eq!(loaded.root_key.expose_bytes(), b"env-secret".to_vec());
    }

    #[test]
    fn load_from_file_accepts_root_key_file_source_relative_to_config_dir() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(dir.path().join("root.secret"), "file-secret\n").expect("write secret");
        let path = write_config(
            dir.path(),
            r#"
root_key = { file = "root.secret" }
"#,
        );

        let loaded = Config::load_from_file(&path).expect("load");

        assert_eq!(loaded.root_key.expose_bytes(), b"file-secret".to_vec());
    }

    #[test]
    fn load_from_sources_applies_env_before_cli() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_config(
            dir.path(),
            r#"
root_key = "file-secret"
log_json = false

[http]
listen = "127.0.0.1:1111"

[paths]
data_dir = "./from-file"

[gc]
interval_secs = 10

[rate_limit]
enabled = false
requests_per_hour = 10
"#,
        );
        let env = MapEnv::default()
            .with(ENV_HTTP_LISTEN, "127.0.0.1:2222")
            .with(ENV_PATHS_DATA_DIR, "./from-env")
            .with(ENV_LOG_JSON, "true")
            .with(ENV_ROOT_KEY, "env-secret")
            .with(ENV_GC_INTERVAL_SECS, "20")
            .with(ENV_RATE_LIMIT_ENABLED, "1")
            .with(ENV_RATE_LIMIT_REQUESTS_PER_HOUR, "30");
        let cli_listen: SocketAddr = "127.0.0.1:3333".parse().expect("socket addr");
        let sources = ConfigSources {
            config_path: Some(&path),
            listen: Some(cli_listen),
            data_dir: Some(PathBuf::from("./from-cli")),
            apply_env: true,
        };

        let loaded = Config::load_from_sources_with_env(&sources, &env).expect("load");

        assert_eq!(loaded.http.listen, cli_listen);
        assert_eq!(loaded.paths.data_dir, PathBuf::from("./from-cli"));
        assert_eq!(loaded.log_json, true);
        assert_eq!(loaded.root_key.expose_bytes(), b"env-secret".to_vec());
        assert_eq!(loaded.gc.interval_secs, 20);
        assert_eq!(loaded.rate_limit.enabled, true);
        assert_eq!(loaded.rate_limit.requests_per_hour, 30);
    }

    #[rstest]
    #[case::empty("root_key = \"\"")]
    #[case::whitespace_only("root_key = \"   \"")]
    fn load_from_file_rejects_empty_root_key(#[case] contents: &str) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_config(dir.path(), contents);

        let err = Config::load_from_file(&path).unwrap_err();

        assert_eq!(matches!(err, ConfigError::EmptyRootKey), true);
    }

    #[test]
    fn load_from_file_rejects_insecure_root_key_placeholder() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_config(
            dir.path(),
            "root_key = \"REPLACE_WITH_SECRET_FROM_keydock_init_OR_LONG_RANDOM_VALUE\"",
        );

        let err = Config::load_from_file(&path).unwrap_err();

        assert_eq!(matches!(err, ConfigError::InsecureRootKeyPlaceholder), true);
    }

    #[test]
    fn load_from_sources_requires_root_key() {
        let env = MapEnv::default();
        let sources = ConfigSources {
            config_path: None,
            listen: None,
            data_dir: None,
            apply_env: true,
        };

        let err = Config::load_from_sources_with_env(&sources, &env).unwrap_err();

        assert_eq!(matches!(err, ConfigError::MissingRootKey), true);
    }

    #[test]
    fn config_debug_redacts_root_key() {
        let config = Config {
            root_key: LoadedSecret(SecretString::from("super-secret")),
            http: HttpConfig::default(),
            paths: PathsConfig::default(),
            log_json: false,
            gc: GcConfig::default(),
            rate_limit: RateLimitConfig::default(),
        };
        let debug = format!("{config:?}");

        assert_eq!(debug.contains("super-secret"), false);
        assert_eq!(debug.contains("[REDACTED]"), true);
    }

    #[test]
    fn load_from_file_rejects_unknown_root_key_table_source() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_config(dir.path(), r#"root_key = { unknown = "x" }"#);

        let err = Config::load_from_file(&path).unwrap_err();

        assert_eq!(matches!(err, ConfigError::Parse { .. }), true);
    }

    #[test]
    fn load_from_sources_no_env_ignores_keydock_overrides() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_config(
            dir.path(),
            r#"
root_key = "file-secret"

[http]
listen = "127.0.0.1:1111"

[paths]
data_dir = "./from-file"
"#,
        );
        let env = MapEnv::default()
            .with(ENV_HTTP_LISTEN, "127.0.0.1:2222")
            .with(ENV_ROOT_KEY, "env-secret");
        let sources = ConfigSources {
            config_path: Some(&path),
            listen: None,
            data_dir: None,
            apply_env: false,
        };

        let loaded = Config::load_from_sources_with_env(&sources, &env).expect("load");

        assert_eq!(
            loaded.http.listen,
            "127.0.0.1:1111".parse::<SocketAddr>().unwrap()
        );
        assert_eq!(loaded.root_key.expose_bytes(), b"file-secret".to_vec());
    }

    #[test]
    fn load_from_sources_no_env_toml_indirection_still_resolves() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_config(dir.path(), r#"root_key = { env = "CUSTOM_KEY" }"#);
        let env = MapEnv::default()
            .with("CUSTOM_KEY", "resolved-secret")
            .with(ENV_ROOT_KEY, "should-be-ignored");
        let sources = ConfigSources {
            config_path: Some(&path),
            listen: None,
            data_dir: None,
            apply_env: false,
        };

        let loaded = Config::load_from_sources_with_env(&sources, &env).expect("load");

        assert_eq!(loaded.root_key.expose_bytes(), b"resolved-secret".to_vec());
    }
}
