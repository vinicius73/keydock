use std::path::PathBuf;

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

    #[error("missing root_key; set root_key in TOML or KEYDOCK_ROOT_KEY in the environment")]
    MissingRootKey,

    #[error("root_key cannot be empty")]
    EmptyRootKey,

    #[error("root_key uses an insecure placeholder value")]
    InsecureRootKeyPlaceholder,

    #[error("environment variable {name} is not valid unicode")]
    EnvNotUnicode { name: String },

    #[error("missing environment variable {name} referenced by root_key")]
    MissingSecretEnv { name: String },

    #[error("invalid environment variable {name}: {reason}")]
    InvalidEnv { name: String, reason: String },

    #[error("failed to read root_key file {path}: {source}")]
    SecretFileIo {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

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
