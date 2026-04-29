//! CLI integration tests for `keydock init` and startup configuration.

use std::fs;
use std::process::{Command, Stdio};

use keydock_config::Config;
use pretty_assertions::{assert_eq, assert_ne};

fn keydock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_keydock")
}

fn keydock_command() -> Command {
    let mut command = Command::new(keydock_bin());
    for name in [
        "KEYDOCK_HTTP_LISTEN",
        "KEYDOCK_HTTP_METRICS_LISTEN",
        "KEYDOCK_PATHS_DATA_DIR",
        "KEYDOCK_LOG_JSON",
        "KEYDOCK_ROOT_KEY",
        "KEYDOCK_GC_INTERVAL_SECS",
        "KEYDOCK_RATE_LIMIT_ENABLED",
        "KEYDOCK_RATE_LIMIT_REQUESTS_PER_HOUR",
    ] {
        command.env_remove(name);
    }
    command
}

#[test]
fn init_creates_config_and_second_run_fails_without_force() {
    let dir = tempfile::tempdir().expect("tempdir");
    let instance = dir.path().join("kd");

    let status = keydock_command()
        .args(["init", instance.to_str().expect("utf8 path")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("spawn keydock init");
    assert_eq!(status.success(), true, "first init should succeed");

    let config_path = instance.join("keydock.toml");
    assert_eq!(
        config_path.is_file(),
        true,
        "config file should exist after init"
    );
    let loaded = Config::load_from_file(&config_path).expect("parse config");
    assert_eq!(
        loaded.paths.data_dir,
        instance.join("data").canonicalize().unwrap()
    );

    let status2 = keydock_command()
        .args(["init", instance.to_str().expect("utf8 path")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("spawn keydock init again");
    assert_eq!(
        status2.success(),
        false,
        "second init without --force should fail"
    );
}

#[test]
fn init_force_overwrites_existing_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let instance = dir.path().join("kd2");

    let first_status = keydock_command()
        .args(["init", instance.to_str().expect("utf8 path")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("spawn");
    assert_eq!(first_status.success(), true, "first init should succeed");

    let config_path = instance.join("keydock.toml");
    let first = Config::load_from_file(&config_path).expect("load");

    let status = keydock_command()
        .args(["init", "--force", instance.to_str().expect("utf8 path")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("spawn");
    assert_eq!(status.success(), true, "init --force should succeed");

    let second = Config::load_from_file(&config_path).expect("load");
    assert_ne!(
        first.root_key.expose_bytes(),
        second.root_key.expose_bytes(),
        "force should regenerate root_key"
    );
}

#[test]
fn init_requires_directory_argument() {
    let status = keydock_command()
        .arg("init")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("spawn");
    assert_eq!(
        status.success(),
        false,
        "init without directory arg should fail"
    );
}

#[test]
fn missing_subcommand_prints_usage() {
    let output = keydock_command()
        .output()
        .expect("spawn keydock with no args");
    assert_eq!(
        output.status.success(),
        false,
        "missing subcommand should fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        stderr.contains("serve") && stderr.contains("init"),
        true,
        "stderr should mention subcommands: {stderr}"
    );
}

#[test]
fn serve_without_root_key_fails_before_startup() {
    let output = keydock_command()
        .arg("serve")
        .output()
        .expect("spawn keydock serve");

    assert_eq!(
        output.status.success(),
        false,
        "serve without root_key should fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        stderr.contains("missing root_key"),
        true,
        "stderr should explain missing root_key: {stderr}"
    );
}

#[test]
fn serve_accepts_root_key_from_environment() {
    let dir = tempfile::tempdir().expect("tempdir");
    let data_path = dir.path().join("data-file");
    fs::write(&data_path, "not a directory").expect("write data path file");

    let output = keydock_command()
        .args([
            "serve",
            "--data-dir",
            data_path.to_str().expect("utf8 path"),
        ])
        .env("KEYDOCK_ROOT_KEY", "env-secret-for-test")
        .output()
        .expect("spawn keydock serve");

    assert_eq!(
        output.status.success(),
        false,
        "serve should fail when data-dir is not a directory"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        stderr.contains("create data directory"),
        true,
        "stderr should reach post-config startup: {stderr}"
    );
    assert_eq!(stderr.contains("env-secret-for-test"), false);
}
