# Bounded Caching, Client Pooling & Telemetry Cardinality

---

## 1. Bounded In-Memory Caching with `moka`

Dynamic languages often store caches in plain objects or maps (`const cache = {}`), which grow without bounds until the process crashes with Out-Of-Memory (OOM).

In Rust, use `moka::future::Cache` to enforce hard memory and eviction bounds:

```rust
use moka::future::Cache;
use std::time::Duration;

#[derive(Clone)]
pub struct CacheService {
    // Stores raw responses bounded by count and time
    cache: Cache<String, Vec<u8>>,
}

impl CacheService {
    pub fn new() -> Self {
        let cache = Cache::builder()
            // Strict bound: never hold more than 10,000 entries
            .max_capacity(10_000)
            // Time to live: entries expire after 1 hour
            .time_to_live(Duration::from_secs(3600))
            // Time to idle: entries unaccessed for 15 mins are evicted
            .time_to_idle(Duration::from_secs(900))
            .build();

        Self { cache }
    }

    pub async fn get(&self, key: &str) -> Option<Vec<u8>> {
        self.cache.get(key).await
    }

    pub async fn set(&self, key: &str, val: Vec<u8>) {
        self.cache.insert(key.to_string(), val).await;
    }
}
```

---

## 2. Singleton `reqwest::Client` in Shared State

Creating `reqwest::Client` per-request leaks sockets, wastes TLS handshakes, and bloats memory. Instantiate one client in `AppState` and share it via `Arc`:

```rust
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use std::time::Duration;

pub fn build_http_client(user_agent: &str) -> reqwest::Client {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_str(user_agent).unwrap());

    reqwest::Client::builder()
        .default_headers(headers)
        .pool_idle_timeout(Duration::from_secs(90))
        .pool_max_idle_per_host(10)
        .tcp_keepalive(Duration::from_secs(60))
        .timeout(Duration::from_secs(15))
        .gzip(true)
        .deflate(true)
        .build()
        .expect("Failed to build HTTP client")
}
```

---

## 3. Protecting Against Metric Cardinality Explosion

When exporting Prometheus metrics, tracking dynamic paths like `/search?q=rust&user=123` creates a new hashmap entry for every unique URL, quickly causing gigabytes of RAM usage.

Use `axum::extract::MatchedPath` to strictly normalize route templates:

```rust
use axum::{body::Body, extract::MatchedPath, http::Request, middleware::Next, response::Response};

pub async fn telemetry_middleware(req: Request<Body>, next: Next) -> Response {
    let method = req.method().to_string();
    let raw_path = req.uri().path().to_string();

    // Prevent /metrics endpoint from tracking itself
    if raw_path == "/metrics" {
        return next.run(req).await;
    }

    // STRICT route normalization: Only record matched templates (e.g. /video/:id)
    // NEVER record query strings or raw parameters!
    let matched_path = req
        .extensions()
        .get::<MatchedPath>()
        .map(|mp| mp.as_str().to_string())
        .unwrap_or_else(|| "/unmatched".to_string());

    let label_key = format!("method=\"{method}\",path=\"{matched_path}\"");
    // Increment metric counter safely bounded by finite routes...

    next.run(req).await
}
```
