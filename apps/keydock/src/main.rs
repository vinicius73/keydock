#![forbid(unsafe_code)]

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

#[cfg(unix)]
use tokio::signal::unix::{SignalKind, signal};

use anyhow::Context;
use metrics_exporter_prometheus::PrometheusBuilder;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

use keydock_config::{CliError, Command, Config, ServeArgs, write_init_config};
use keydock_domain::SigningKey;
use keydock_fjall::FjallStore;
use keydock_http::build_router;
use keydock_state::AppState;
use keydock_support::clock::SystemClock;
use keydock_usecase::ports::{BucketRepository, KeyRepository};

fn main() -> anyhow::Result<()> {
    let command = match keydock_config::parse() {
        Ok(c) => c,
        Err(e) => {
            if matches!(e, CliError::MissingSubcommand) {
                eprintln!("usage: keydock <serve | init> ...\n");
                eprintln!("Run `keydock --help` for details.");
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
        Command::Init(args) => {
            init_instance(&args.dir, args.force);
        }
    }

    Ok(())
}

fn init_instance(instance_dir: &std::path::Path, force: bool) {
    match write_init_config(instance_dir, force) {
        Ok(path) => println!("Created {}", path.display()),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    }
}

async fn serve(args: ServeArgs) -> anyhow::Result<()> {
    let config = load_merged_config(&args).context("configuration")?;

    init_tracing(config.log_json).context("init tracing")?;

    let prometheus = PrometheusBuilder::new()
        .install_recorder()
        .context("metrics recorder")?;

    std::fs::create_dir_all(&config.paths.data_dir)
        .with_context(|| format!("create data directory {}", config.paths.data_dir.display()))?;

    let store = Arc::new(
        FjallStore::open(&config.paths.data_dir)
            .with_context(|| format!("open store at {}", config.paths.data_dir.display()))?,
    );
    tracing::info!(
        data_dir = %config.paths.data_dir.display(),
        "fjall storage opened"
    );
    let buckets: Arc<dyn BucketRepository> = store.clone();
    let keys: Arc<dyn KeyRepository> = store.clone();
    let clock: Arc<dyn keydock_support::Clock> = Arc::new(SystemClock);

    let root_key = Arc::new(SigningKey::new(Box::new(config.root_key.expose_bytes())));
    let state = AppState::new(env!("CARGO_PKG_VERSION"), clock, buckets, keys, root_key);

    if config.rate_limit.enabled {
        let tokens = u32::try_from(config.rate_limit.requests_per_hour).unwrap_or(u32::MAX);
        let rule = lazy_limit::RuleConfig::new(lazy_limit::Duration::hours(1), tokens);
        let limiter_config = lazy_limit::LimiterConfig::new(rule);
        lazy_limit::initialize_limiter(limiter_config).await;
    }

    let router = build_router(state, prometheus, config.rate_limit.clone());

    let addr = config.http.listen;
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("bind {addr}"))?;

    tracing::info!(%addr, "listening");

    let gc_cancel = CancellationToken::new();
    let gc_interval_secs = config.gc.interval_secs;
    let sweeper = store.build_gc_sweeper(Duration::from_secs(gc_interval_secs));
    tokio::spawn(sweeper.run(gc_cancel.clone()));
    tracing::info!(
        interval_secs = gc_interval_secs,
        "gc background sweeper spawned"
    );

    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        shutdown_signal().await;
        gc_cancel.cancel();
        tracing::info!("gc cancellation requested");
    })
    .await
    .context("server")?;

    tracing::info!("http server stopped");
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
    result.map_err(|e| anyhow::anyhow!("init tracing subscriber: {e}"))
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::error!(error = %e, "failed to listen for CTRL+C");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal(SignalKind::terminate()) {
            Ok(mut stream) => {
                stream.recv().await;
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to listen for SIGTERM");
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }

    tracing::info!("shutdown signal received");
}
