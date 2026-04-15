#![forbid(unsafe_code)]

use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use keydock_config::{CliError, Command, Config, ServeArgs, ValidatedHttpConfig};
use keydock_fjall::FjallStore;
use keydock_http::build_router;
use keydock_state::AppState;
use keydock_support::clock::SystemClock;
use keydock_usecase::ports::{BucketRepository, KeyRepository};
use metrics_exporter_prometheus::PrometheusBuilder;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

fn main() -> anyhow::Result<()> {
    let command = match keydock_config::parse() {
        Ok(c) => c,
        Err(e) => {
            if matches!(e, CliError::MissingSubcommand) {
                eprintln!("usage: keydock serve [options]\n");
                eprintln!("Run `keydock serve --help` for details.");
            } else {
                eprintln!("{e}");
            }
            std::process::exit(2);
        }
    };

    match command {
        Command::Serve(args) => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .context("tokio runtime")?;
            rt.block_on(async move { serve(args).await })?;
        }
    }

    Ok(())
}

async fn serve(args: ServeArgs) -> anyhow::Result<()> {
    let config = load_merged_config(&args).context("configuration")?;

    init_tracing(config.log_json).context("init tracing")?;

    let prometheus = PrometheusBuilder::new()
        .install_recorder()
        .context("metrics recorder")?;

    std::fs::create_dir_all(&config.paths.data_dir)
        .with_context(|| format!("create data directory {}", config.paths.data_dir.display()))?;

    let store =
        Arc::new(FjallStore::open(&config.paths.data_dir).map_err(|e| anyhow::anyhow!("{e}"))?);
    let buckets: Arc<dyn BucketRepository> = store.clone();
    let keys: Arc<dyn KeyRepository> = store.clone();
    let clock: Arc<dyn keydock_support::Clock> = Arc::new(SystemClock);

    let http = ValidatedHttpConfig::from_config(&config);
    let state = AppState::new(http, env!("CARGO_PKG_VERSION"), clock, buckets, keys);

    let router = build_router(state, prometheus);

    let addr = config.http.listen;
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("bind {addr}"))?;

    tracing::info!(%addr, "listening");

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server")?;

    Ok(())
}

fn load_merged_config(args: &ServeArgs) -> Result<Config, keydock_config::ConfigError> {
    let base = if let Some(path) = &args.config_path {
        Config::load_from_file(path)?
    } else {
        Config::default()
    };

    Ok(base.merge_cli(args.listen, args.data_dir.clone()))
}

fn init_tracing(json: bool) -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let result = if json {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(filter)
            .try_init()
    } else {
        tracing_subscriber::fmt().with_env_filter(filter).try_init()
    };
    result.map_err(|e| anyhow::anyhow!("{e}"))
}

/// For `--config` default when none is passed: optional `keydock.toml` in cwd.
#[allow(dead_code)]
fn default_config_path() -> impl AsRef<Path> {
    std::path::Path::new("keydock.toml")
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install CTRL+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{SignalKind, signal};
        signal(SignalKind::terminate())
            .expect("install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }

    tracing::info!("shutdown signal received");
}
