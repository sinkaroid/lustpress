#![deny(clippy::unwrap_used, clippy::expect_used)]

use lustpress::config::Config;
use lustpress::routes::create_router;
use lustpress::state::AppState;
use std::error::Error;
use tower_http::cors::CorsLayer;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
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
