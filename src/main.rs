#![deny(clippy::unwrap_used, clippy::expect_used)]

use lustpress::config::Config;
use lustpress::routes::create_router;
use lustpress::state::AppState;
use std::error::Error;
use tower_http::cors::CorsLayer;
use tracing::info;

fn main() -> Result<(), Box<dyn Error>> {
    // Disable Transparent Huge Pages (THP) on Linux to prevent 2MB memory lock-in
    #[cfg(target_os = "linux")]
    unsafe {
        libc::prctl(libc::PR_SET_THP_DISABLE, 1, 0, 0, 0);
    }

    let worker_threads = std::env::var("TOKIO_WORKER_THREADS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(2);

    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(worker_threads)
        .enable_all()
        .build()?
        .block_on(async_main())
}

async fn async_main() -> Result<(), Box<dyn Error>> {
    // 1. Load .env if present
    dotenvy::dotenv().ok();

    // 2. Parse configuration
    let config = match Config::from_env() {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("Configuration Error: {err}");
            std::process::exit(1);
        }
    };

    // 3. Initialize structured logging
    let default_filter = if config.enable_debug_log {
        "lustpress=debug,info"
    } else {
        &config.log_level
    };
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_filter));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    info!("Starting lustpress v{}", env!("CARGO_PKG_VERSION"));

    // 4. Initialize Prometheus metrics collector
    lustpress::metrics::init_metrics();

    // 5. Initialize shared state
    let state = match AppState::new(config.clone()).await {
        Ok(s) => s,
        Err(err) => {
            tracing::error!("Failed to initialize AppState: {err}");
            std::process::exit(1);
        }
    };

    // 5b. Verify Redis connection status asynchronously (matching jandapress)
    if config.redis_url.is_some() {
        let cache_clone = state.cache.clone();
        tokio::spawn(async move {
            match cache_clone.test_connection().await {
                Ok(_) => {
                    info!("Redis cache connection verified successfully! Backend status: Connected")
                }
                Err(err) => tracing::warn!(
                    "Redis connection verification failed: {err}. Falling back to In-Memory operations."
                ),
            }
        });
    }

    // 5c. Spawn periodic in-memory cache maintenance task (pruning expired TTL entries)
    let cache_maint = state.cache.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            cache_maint.run_pending_tasks().await;
            lustpress::util::trim_memory();
        }
    });

    // 5. Build router with CORS
    let app = create_router(state).layer(CorsLayer::permissive());

    // 6. Bind listener
    let addr = config.bind_addr();
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(err) => {
            tracing::error!("Failed to bind to {addr}: {err}");
            std::process::exit(1);
        }
    };

    info!("Lustpress is running at http://{addr}");

    // 7. Graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error>)?;

    info!("Server shutdown completed gracefully.");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .unwrap_or_else(|e| tracing::warn!("Failed to install Ctrl+C handler: {e}"));
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(e) => {
                tracing::warn!("Failed to install SIGTERM handler: {e}");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C, starting graceful shutdown...");
        },
        _ = terminate => {
            info!("Received SIGTERM, starting graceful shutdown...");
        },
    }
}
