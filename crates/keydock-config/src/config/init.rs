use std::fs;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use secrecy::SecretString;
use tracing::instrument;

use super::error::InitError;
use super::secret::LoadedSecret;
use super::{Config, GcConfig, HttpConfig, PathsConfig, RateLimitConfig};

/// Filename written by [`write_init_config`] inside the instance directory.
pub const CONFIG_FILENAME: &str = "keydock.toml";

/// Builds a Config with default settings, a freshly generated root key, and the given data dir.
///
/// Used exclusively by [`write_init_config`] for local instance bootstrap.
fn build_init_config(data_dir: PathBuf) -> Config {
    let root_key = LoadedSecret(SecretString::from(format!(
        "{}-{}",
        uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4()
    )));
    Config {
        http: HttpConfig::default(),
        paths: PathsConfig { data_dir },
        log_json: false,
        root_key,
        gc: GcConfig::default(),
        rate_limit: RateLimitConfig::default(),
    }
}

/// Creates `<instance_dir>/data`, writes `<instance_dir>/keydock.toml` with defaults and a new
/// `root_key`, and returns the config file path.
///
/// `paths.data_dir` is stored as an **absolute** path so `keydock serve -c …` works from any cwd.
#[instrument(skip_all, fields(instance_dir = %instance_dir.display(), force = force))]
pub fn write_init_config(instance_dir: &Path, force: bool) -> Result<PathBuf, InitError> {
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

    let config = build_init_config(data_dir_canonical);

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
        fs::set_permissions(&config_path, fs::Permissions::from_mode(0o600)).map_err(|e| {
            InitError::Io {
                path: config_path.clone(),
                source: e,
            }
        })?;
    }

    Ok(config_path)
}
