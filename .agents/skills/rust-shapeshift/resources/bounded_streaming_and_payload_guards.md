# Bounded Streaming & Memory Exhaustion Guards

One of the most insidious causes of production out-of-memory (OOM) crashes in backend services is **unbounded payload buffering**. Calling `res.text().await`, `res.bytes().await`, or accepting unconstrained incoming POST bodies forces the process to allocate contiguous heap memory matching the entire payload size.

If an upstream provider returns a 50MB error page, a decompression bomb, or an infinite chunked HTTP stream, a single request can consume hundreds of megabytes of RAM that glibc may not immediately return to the operating system.

---

## 1. Guarding Inbound Axum Requests

Never allow incoming HTTP request bodies to allocate unbounded memory. Axum provides `DefaultBodyLimit` to reject oversized payloads at the socket layer before they reach handlers:

```rust
use axum::{
    extract::DefaultBodyLimit,
    routing::post,
    Router,
};

pub fn configure_payload_limits(app: Router) -> Router {
    app
        // Enforce a strict global limit (e.g., 64KB for standard JSON APIs)
        .layer(DefaultBodyLimit::max(64 * 1024))
        // Or override for specific file-upload endpoints:
        // .route("/upload", post(upload_handler).layer(DefaultBodyLimit::max(10 * 1024 * 1024)))
}
```

---

## 2. Bounded Upstream Response Consumption

When fetching data from third-party APIs or external websites, never buffer the entire response blindly. Use `futures_util::StreamExt` to read the response stream with a hard byte ceiling:

```rust
use bytes::{Bytes, BytesMut};
use futures_util::StreamExt;
use reqwest::Response;

const MAX_SCRAPE_BYTES: usize = 512 * 1024; // 512 KB ceiling

pub async fn read_bounded_body(res: Response) -> Result<Bytes, AppError> {
    // 1. Verify Content-Length header early if available
    if let Some(len) = res.content_length() {
        if len > MAX_SCRAPE_BYTES as u64 {
            return Err(AppError::PayloadTooLarge(format!(
                "Upstream payload exceeds maximum allowance: {len} bytes"
            )));
        }
    }

    // 2. Stream chunks with hard byte accumulator
    let mut stream = res.bytes_stream();
    let mut buffer = BytesMut::with_capacity(32 * 1024); // Start lean with 32KB

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| AppError::Upstream(e.to_string()))?;
        
        if buffer.len() + chunk.len() > MAX_SCRAPE_BYTES {
            // Cut the socket immediately: prevents downloading massive payloads
            return Err(AppError::PayloadTooLarge(
                "Upstream response exceeded 512KB threshold during streaming".into(),
            ));
        }

        buffer.extend_from_slice(&chunk);
    }

    Ok(buffer.freeze())
}
```

---

## 3. Early-Break Streaming for Fast Parsing

When extracting metadata from the beginning of an HTML document or stream, stop reading the HTTP stream as soon as the target content is found:

```rust
pub async fn read_until_delimiter(res: Response, delimiter: &str) -> Result<String, AppError> {
    let mut stream = res.bytes_stream();
    let mut accumulated = String::with_capacity(16 * 1024);

    while let Some(chunk_res) = stream.next().await {
        let chunk = chunk_res.map_err(|e| AppError::Upstream(e.to_string()))?;
        let text = String::from_utf8_lossy(&chunk);
        accumulated.push_str(&text);

        // Terminate download early as soon as the header/target section is captured
        if accumulated.contains(delimiter) {
            break;
        }

        if accumulated.len() > MAX_SCRAPE_BYTES {
            break;
        }
    }

    Ok(accumulated)
}
```

---

## 4. Zero-Copy Media Proxying via `Body::from_stream`

When proxying images, video streams, or binary assets, **do not read the bytes into memory**. Stream them directly from the upstream `reqwest` response into the downstream `axum` response body:

```rust
use axum::{
    body::Body,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response as AxumResponse},
};

pub async fn proxy_asset_handler(
    client: reqwest::Client,
    target_url: &str,
) -> Result<AxumResponse, AppError> {
    let upstream_res = client.get(target_url).send().await.map_err(AppError::from)?;

    let status = StatusCode::from_u16(upstream_res.status().as_u16())
        .unwrap_or(StatusCode::BAD_GATEWAY);

    let mut headers = HeaderMap::new();
    if let Some(content_type) = upstream_res.headers().get(header::CONTENT_TYPE) {
        headers.insert(header::CONTENT_TYPE, content_type.clone());
    }
    if let Some(cache_control) = upstream_res.headers().get(header::CACHE_CONTROL) {
        headers.insert(header::CACHE_CONTROL, cache_control.clone());
    }

    // Direct zero-copy pipe: bytes flow from network socket to client socket
    let body = Body::from_stream(upstream_res.bytes_stream());

    Ok((status, headers, body).into_response())
}
```

---

## 5. Security & Stability Checklist

1. Is `DefaultBodyLimit` applied globally across all Axum routes?
2. Are all third-party HTTP streams guarded with a byte accumulator ceiling?
3. Does proxying use `Body::from_stream` rather than buffering in a `Vec<u8>`?
4. Are oversized responses aborted immediately at the TCP stream level without downloading remainder bytes?
