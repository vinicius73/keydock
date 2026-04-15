use std::net::SocketAddr;
use std::path::PathBuf;

use lexopt::prelude::*;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("missing subcommand (expected `serve`)")]
    MissingSubcommand,

    #[error(transparent)]
    Parse(#[from] lexopt::Error),

    #[error("invalid value for {0}: {1}")]
    InvalidValue(&'static str, String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Serve(ServeArgs),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServeArgs {
    pub config_path: Option<PathBuf>,
    pub listen: Option<SocketAddr>,
    pub data_dir: Option<PathBuf>,
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
            Short('h') | Long("help") => {
                print_serve_help();
                std::process::exit(0);
            }
            arg => return Err(CliError::Parse(arg.unexpected())),
        }
    }

    Ok(args)
}

fn print_root_help() {
    println!(
        "\
keydock — multi-tenant key-value HTTP service

Usage:
  keydock serve [options]

Run `keydock serve --help` for serve options.
"
    );
}

fn print_serve_help() {
    println!(
        "\
keydock serve

Options:
  -c, --config <PATH>   Path to TOML config file
      --listen <ADDR>     Socket address to listen on (overrides config)
      --data-dir <PATH>   Data directory (overrides config)
  -h, --help            Show this help
"
    );
}
