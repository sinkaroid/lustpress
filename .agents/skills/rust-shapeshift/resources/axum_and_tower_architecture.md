# Production Axum 0.8 Architecture & Tower Middleware

---

## 1. Project Directory Layout

```text
src/
├── main.rs            # Entrypoint, signal trapping, graceful shutdown
├── state.rs           # Shared AppState (Client, Cache, Config)
├── config.rs          # Environment loading via dotenvy
├── error.rs           # Domain errors (thiserror) & Axum IntoResponse
├── util.rs            # trim_memory() & formatting helpers
├── metrics.rs         # Telemetry middleware & Prometheus export
├── routes/
│   ├── mod.rs         # Router assembly & middleware stacking
│   ├── rest.rs        # REST endpoint handlers
│   └── status.rs      # Health check & system diagnostics
└── services/          # Business logic, scrapers, DB integration
```

---

## 2. Server Entrypoint & Graceful Shutdown (`src/main.rs`)

```rust
use axum::Router;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::signal;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod config;
mod error;
mod metrics;
mod routes;
mod state;
mod util;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "info,app=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cfg = config::Config::from_env()?;
    let state = Arc::new(state::AppState::new(&cfg)?);

    let app = routes::create_router(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], cfg.port));
    tracing::info!("Server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("Server shut down cleanly.");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("Received Ctrl+C, draining connections..."),
        _ = terminate => tracing::info!("Received SIGTERM, draining connections..."),
    }
}
```

---

## 3. Centralized Domain Error Mapping (`src/error.rs`)

```rust
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Upstream service returned error status {status} for {url}")]
    Upstream {
        status: reqwest::StatusCode,
        url: String,
    },

    #[error("Internal system error: {0}")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::Validation(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Upstream { status, url } => {
                tracing::warn!(%status, %url, "Upstream error encountered");
                (StatusCode::BAD_GATEWAY, "Upstream service error".to_string())
            }
            AppError::Internal(err) => {
                tracing::error!(%err, "Internal failure");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string())
            }
        };

        let body = Json(json!({
            "success": false,
            "error": message,
            "status": status.as_u16(),
        }));

        (status, body).into_response()
    }
}
```
