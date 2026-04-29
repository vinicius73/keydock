use std::net::SocketAddr;
use std::path::PathBuf;

use lexopt::prelude::*;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("missing subcommand (expected one of: `serve`, `init`)")]
    MissingSubcommand,

    #[error(transparent)]
    Parse(#[from] lexopt::Error),

    #[error("invalid value for {0}: {1}")]
    InvalidValue(&'static str, String),

    #[error("missing directory argument for `init`")]
    MissingInitDirectory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Serve(ServeArgs),
    Init(InitArgs),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServeArgs {
    pub config_path: Option<PathBuf>,
    pub listen: Option<SocketAddr>,
    pub data_dir: Option<PathBuf>,
    /// When `true`, `KEYDOCK_*` environment overrides are skipped.
    /// TOML-declared indirection (`root_key = { env/file }`) is unaffected.
    pub no_env: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitArgs {
    pub dir: PathBuf,
    pub force: bool,
}

/// Parses CLI into a structured command (minimal surface for the composition root).
pub fn parse() -> Result<Command, CliError> {
    let mut parser = lexopt::Parser::from_env();

    let sub = match parser.next()? {
        None => return Err(CliError::MissingSubcommand),
        Some(Long("help")) | Some(Short('h')) => {
            print_root_help();
            std::process::exit(0);
        }
        Some(Value(v)) => v.string()?,
        Some(arg) => return Err(CliError::Parse(arg.unexpected())),
    };

    match sub.as_str() {
        "serve" => Ok(Command::Serve(parse_serve(&mut parser)?)),
        "init" => Ok(Command::Init(parse_init(&mut parser)?)),
        other => Err(CliError::InvalidValue("subcommand", other.to_string())),
    }
}

fn parse_serve(parser: &mut lexopt::Parser) -> Result<ServeArgs, CliError> {
    let mut args = ServeArgs::default();

    while let Some(arg) = parser.next()? {
        match arg {
            Short('c') | Long("config") => {
                let path: PathBuf = parser.value()?.parse()?;
                args.config_path = Some(path);
            }
            Long("listen") => {
                let s: String = parser.value()?.parse()?;
                let addr: SocketAddr = s
                    .parse()
                    .map_err(|e| CliError::InvalidValue("listen", format!("{s}: {e}")))?;
                args.listen = Some(addr);
            }
            Long("data-dir") => {
                let path: PathBuf = parser.value()?.parse()?;
                args.data_dir = Some(path);
            }
            Long("no-env") => {
                args.no_env = true;
            }
            Short('h') | Long("help") => {
                print_serve_help();
                std::process::exit(0);
            }
            arg => return Err(CliError::Parse(arg.unexpected())),
        }
    }

    Ok(args)
}

fn parse_init(parser: &mut lexopt::Parser) -> Result<InitArgs, CliError> {
    let mut force = false;
    let mut dir: Option<PathBuf> = None;

    while let Some(arg) = parser.next()? {
        match arg {
            Long("force") => {
                force = true;
            }
            Short('h') | Long("help") => {
                print_init_help();
                std::process::exit(0);
            }
            Value(v) => {
                let s = v.string()?;
                if dir.is_some() {
                    return Err(CliError::InvalidValue(
                        "init",
                        format!("unexpected extra argument: {s}"),
                    ));
                }
                dir = Some(PathBuf::from(s));
            }
            arg => return Err(CliError::Parse(arg.unexpected())),
        }
    }

    let dir = dir.ok_or(CliError::MissingInitDirectory)?;
    Ok(InitArgs { dir, force })
}

fn print_root_help() {
    println!(
        "\
keydock — multi-tenant key-value HTTP service

Usage:
  keydock serve [options]
  keydock init [options] <DIR>

Run `keydock serve --help` or `keydock init --help` for details.
"
    );
}

fn print_serve_help() {
    println!(
        "\
keydock serve

Options:
  -c, --config <PATH>   Path to TOML config file
      --listen <ADDR>   Socket address to listen on (overrides config and env)
      --data-dir <PATH> Data directory (overrides config and env)
      --no-env          Skip KEYDOCK_* environment overrides.
                        TOML-declared indirection (root_key = {{ env/file }}) is unaffected.
  -h, --help            Show this help

Environment (applied after config file, before CLI flags; skipped with --no-env):
  KEYDOCK_ROOT_KEY                      Root secret (required if not set in TOML)
  KEYDOCK_HTTP_LISTEN                   Bind address (e.g. 0.0.0.0:8080)
  KEYDOCK_HTTP_METRICS_LISTEN           Dedicated Prometheus scrape address
  KEYDOCK_PATHS_DATA_DIR                Data directory path
  KEYDOCK_LOG_JSON                      Emit JSON logs (true/false)
  KEYDOCK_GC_INTERVAL_SECS              GC sweep interval in seconds
  KEYDOCK_RATE_LIMIT_ENABLED            Enable rate limiting (true/false)
  KEYDOCK_RATE_LIMIT_REQUESTS_PER_HOUR  Requests per IP per hour
"
    );
}

fn print_init_help() {
    println!(
        "\
keydock init

Creates a new keydock.toml under <DIR> (and <DIR>/data) with default settings and a generated root_key.

Arguments:
  <DIR>                 Instance directory (created if missing)

Options:
      --force           Overwrite an existing keydock.toml
  -h, --help            Show this help
"
    );
}
