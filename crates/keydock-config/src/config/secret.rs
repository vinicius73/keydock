use std::fs;
use std::path::{Path, PathBuf};

use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use tracing::instrument;

use super::env::EnvSource;
use super::error::ConfigError;

const INSECURE_ROOT_KEY_PLACEHOLDER: &str =
    "REPLACE_WITH_SECRET_FROM_keydock_init_OR_LONG_RANDOM_VALUE";

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum RawRootKey {
    Inline(String),
    Env(String),
    File(PathBuf),
}

impl std::fmt::Debug for RawRootKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Inline(_) => f.write_str("Inline([REDACTED])"),
            Self::Env(name) => write!(f, "Env({name:?})"),
            Self::File(path) => write!(f, "File({path:?})"),
        }
    }
}

impl<'de> Deserialize<'de> for RawRootKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = toml::Value::deserialize(deserializer)?;
        match value {
            toml::Value::String(value) => Ok(Self::Inline(value)),
            toml::Value::Table(table) => {
                parse_root_key_table(table).map_err(serde::de::Error::custom)
            }
            other => Err(serde::de::Error::custom(format!(
                "root_key must be a string or table, got {}",
                other.type_str()
            ))),
        }
    }
}

fn parse_root_key_table(table: toml::Table) -> Result<RawRootKey, String> {
    let mut source = None;
    for (key, value) in table {
        if source.is_some() {
            return Err("root_key table must contain exactly one source".to_string());
        }

        source = Some(match (key.as_str(), value) {
            ("value", toml::Value::String(value)) => RawRootKey::Inline(value),
            ("env", toml::Value::String(name)) => RawRootKey::Env(name),
            ("file", toml::Value::String(path)) => RawRootKey::File(PathBuf::from(path)),
            ("value" | "env" | "file", _) => {
                return Err(format!("root_key.{key} must be a string"));
            }
            (unknown, _) => {
                return Err(format!(
                    "unknown root_key source `{unknown}`; supported: `value`, `env`, `file`"
                ));
            }
        });
    }

    source.ok_or_else(|| "root_key table must contain exactly one source".to_string())
}

pub(crate) fn resolve_root_key(
    source: Option<RawRootKey>,
    config_dir: Option<&Path>,
    env: &dyn EnvSource,
) -> Result<LoadedSecret, ConfigError> {
    let Some(source) = source else {
        return Err(ConfigError::MissingRootKey);
    };

    let value = match source {
        RawRootKey::Inline(value) => value,
        RawRootKey::Env(name) => env
            .get(&name)?
            .ok_or(ConfigError::MissingSecretEnv { name })?,
        RawRootKey::File(path) => {
            let path = resolve_secret_file_path(config_dir, path);
            fs::read_to_string(&path)
                .map_err(|source| ConfigError::SecretFileIo {
                    path: path.clone(),
                    source,
                })?
                .trim_end_matches(['\r', '\n'])
                .to_string()
        }
    };

    validate_root_key(&value)?;
    Ok(LoadedSecret(SecretString::from(value)))
}

fn resolve_secret_file_path(config_dir: Option<&Path>, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        return path;
    }

    match config_dir {
        Some(dir) => dir.join(path),
        None => path,
    }
}

fn validate_root_key(value: &str) -> Result<(), ConfigError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ConfigError::EmptyRootKey);
    }
    if trimmed == INSECURE_ROOT_KEY_PLACEHOLDER {
        return Err(ConfigError::InsecureRootKeyPlaceholder);
    }
    Ok(())
}

/// Secret string from config (never log; use [`LoadedSecret::expose_bytes`] only at startup).
///
/// Serialization intentionally emits the secret so `keydock init` can write a complete
/// local config file with restrictive permissions.
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
