# Reqwest Client & TCP Socket Pooling: Eliminating Network Memory Leaks

In native network services, instantiating an HTTP client per request creates new TCP connection pools, triggers repeated TLS handshakes, and leaks OS socket descriptors, resulting in severe memory bloat and eventual port exhaustion under load. In Rust, HTTP clients must be engineered as long-lived, bounded singletons.

---

## 1. The Singleton Client Architecture

Always instantiate a **single `reqwest::Client`** at application startup, store it inside `Arc<AppState>`, and reuse it across all handlers and background tasks:

```rust
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use std::time::Duration;

pub fn create_shared_http_client(user_agent: &str) -> reqwest::Client {
    let mut default_headers = HeaderMap::new();
    default_headers.insert(
        USER_AGENT,
        HeaderValue::from_str(user_agent).unwrap_or_else(|_| HeaderValue::from_static("RustBackend/1.0")),
    );

    reqwest::Client::builder()
        .default_headers(default_headers)
        // 1. Connection Pool Bounding
        .pool_idle_timeout(Duration::from_secs(90))
        .pool_max_idle_per_host(10)
        // 2. TCP Keepalive
        .tcp_keepalive(Duration::from_secs(60))
        // 3. Global Hard Timeout
        .timeout(Duration::from_secs(15))
        // 4. Native Compression (saves network memory buffers)
        .gzip(true)
        .deflate(true)
        .build()
        .expect("Failed to build shared HTTP client")
}
```

---

## 2. Tiered Timeouts: Never Let Ancillary Calls Block Workers

A common mistake is using the same 15-second timeout for everything. If an ancillary request (like GeoIP lookup or third-party status probe) hangs, it pins the Tokio worker thread context for 15 seconds.

### The Fast-Fail Ancillary Pattern:
Override timeouts on ancillary requests so they fail fast without impacting core traffic:

```rust
// Core Scraper Request (Needs longer timeout for large pages)
let body = client
    .get(target_url)
    .send() // Uses default 15s timeout
    .await?;

// Ancillary Metadata Request (Must fail fast in 3 seconds!)
let location = match client
    .get("https://ipwho.is/")
    .timeout(Duration::from_secs(3)) // Explicit fast timeout
    .send()
    .await
{
    Ok(res) if res.status().is_success() => parse_location(res).await,
    _ => "Unknown".to_string(), // Fail gracefully without holding resources
};
```

---

## 3. Selective Caching Policy (Preventing Cache Bloat)

Never cache responses blindly. Certain endpoints (such as `/random` galleries or ephemeral queries) produce infinite unique keys that flood your cache and evict hot entries:

```rust
pub async fn fetch_body(&self, url: &str) -> Result<Vec<u8>, AppError> {
    // 1. Check cache first
    if let Some(cached) = self.cache.get(url).await {
        return Ok(cached);
    }

    // 2. EXCLUDE random / volatile endpoints from cache insertion!
    if url.contains("/random") {
        let res = self.client.get(url).send().await?;
        return Ok(res.bytes().await?.to_vec());
    }

    // 3. Cache deterministic endpoints
    let res = self.client.get(url).send().await?;
    let bytes = res.bytes().await?.to_vec();
    self.cache.set(url, &bytes).await;

    Ok(bytes)
}
```

---

## 4. Chunked Stream Early-Break (Socket Lifecycle Control)

When verifying challenge tokens or inspecting page headers, never read the full multi-megabyte body with `res.text().await`.

Stream chunks and explicitly drop the response:

```rust
use reqwest::Response;

pub async fn check_upstream_challenge(mut res: Response) -> Result<bool, reqwest::Error> {
    let mut buffer = String::new();

    while let Ok(Some(chunk)) = res.chunk().await {
        if let Ok(s) = std::str::from_utf8(&chunk) {
            buffer.push_str(s);
        }

        // Target found early? Break immediately!
        if buffer.contains("challenge_token") {
            // Drop response immediately to return socket to pool
            drop(res);
            return Ok(true);
        }

        // Safety cap: Never buffer more than 64KB for a verification check
        if buffer.len() >= 65_536 {
            break;
        }
    }

    drop(res);
    Ok(false)
}
```
