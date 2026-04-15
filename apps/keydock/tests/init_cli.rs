//! CLI integration tests for `keydock init`.

use std::process::{Command, Stdio};

use keydock_config::Config;
use pretty_assertions::assert_eq;

fn keydock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_keydock")
}

#[test]
fn init_creates_config_and_second_run_fails_without_force() {
    let dir = tempfile::tempdir().expect("tempdir");
    let instance = dir.path().join("kd");

    let status = Command::new(keydock_bin())
        .args(["init", instance.to_str().expect("utf8 path")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("spawn keydock init");
    assert_eq!(status.success(), true, "first init should succeed");

    let config_path = instance.join("keydock.toml");
    assert_eq!(config_path.is_file(), true);
    let loaded = Config::load_from_file(&config_path).expect("parse config");
    assert_eq!(
        loaded.paths.data_dir,
        instance.join("data").canonicalize().unwrap()
    );

    let status2 = Command::new(keydock_bin())
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

    assert_eq!(
        Command::new(keydock_bin())
            .args(["init", instance.to_str().expect("utf8 path")])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("spawn")
            .success(),
        true
    );

    let config_path = instance.join("keydock.toml");
    let first = Config::load_from_file(&config_path).expect("load");

    let status = Command::new(keydock_bin())
        .args(["init", "--force", instance.to_str().expect("utf8 path")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("spawn");
    assert_eq!(status.success(), true);

    let second = Config::load_from_file(&config_path).expect("load");
    assert_ne!(
        first.root_key.expose_bytes(),
        second.root_key.expose_bytes(),
        "force should regenerate root_key"
    );
}

#[test]
fn init_requires_directory_argument() {
    let status = Command::new(keydock_bin())
        .arg("init")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("spawn");
    assert_eq!(status.success(), false);
}

#[test]
fn missing_subcommand_prints_usage() {
    let output = Command::new(keydock_bin())
        .output()
        .expect("spawn keydock with no args");
    assert_eq!(output.status.success(), false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        stderr.contains("serve") && stderr.contains("init"),
        true,
        "stderr should mention subcommands: {stderr}"
    );
}
