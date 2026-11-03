# Zero-Panic Architecture & Axum Router Layering

In high-throughput services, an unhandled panic on a Tokio worker thread drops the task, risks corrupting thread-local state, orphans synchronization locks, and degrades long-term process throughput. A robust production service must be mathematically panic-free at compile time.

This reference guide documents the **Zero-Panic Policy**, router recursion management, and correct middleware layer ordering used in high-efficiency production services.

---

## 1. Compile-Time Zero-Panic Enforcement

Enforce zero unhandled crashes at the compiler level across all targets:

### Top of `src/main.rs` and `src/lib.rs`
```rust
#![deny(clippy::unwrap_used, clippy::expect_used)]
```

When Clippy enforces this lint:
- Any developer (or AI agent) attempting to call `.unwrap()` or `.expect()` will fail compilation immediately.
- Forces explicit error conversion and bubbling using `?` or `.ok_or_else(...)`.

### Safe Idiom Replacements:
```rust
// ❌ WRONG: Panics if query parameter or header is missing
let id = link.split('=').nth(1).unwrap();

// ✅ CORRECT: Safe fallback with default or domain error
let id = link.split('=').nth(1).unwrap_or("None").to_string();

// ❌ WRONG: Panics if selector fails
let title = el.select(&SEL_TITLE).next().unwrap().text().collect();

// ✅ CORRECT: Functional Option combinators
let title = el
    .select(&SEL_TITLE)
    .next()
    .map(|el| el.text().collect::<String>().trim().to_string())
    .unwrap_or_else(|| "None".to_string());
```

---

## 2. Macro Recursion Limit for Deeply Nested Routers

As an Axum application grows to dozens of endpoints with nested middleware layers (`.route_layer()`, `.layer()`, `.merge()`), the Rust compiler's default macro expansion depth is exceeded, causing compile errors:
`recursion limit reached while expanding the macro`

### Top of `src/main.rs`
```rust
#![recursion_limit = "512"]
```

---

## 3. Strict Middleware Layer Execution Order

In Axum, middleware layers wrap handlers like an onion. **Layer ordering dictates execution order and memory safety**:

```rust
// 1. Core Service Routes
let scraper_routes = Router::<AppState>::new()
    .route("/provider/get", get(provider_get))
    .route("/provider/search", get(provider_search))
    // INNER LAYERS: Executed closest to handler
    .route_layer(axum::middleware::from_fn_with_state(state.clone(), slow_down_middleware))
    .route_layer(axum::middleware::from_fn_with_state(state.clone(), rate_limiter_middleware))
    // CORS LAYER: Permissive or strict headers
    .layer(tower_http::cors::CorsLayer::permissive());

// 2. Assembly with State
let app = Router::new()
    .merge(scraper_routes.with_state(state.clone()))
    .route("/metrics", get(metrics_handler))
    .route("/doc", get(doc_handler))
    // OUTER LAYER: Telemetry must be outermost to capture ALL requests (including 404s)
    .layer(axum::middleware::from_fn(telemetry_middleware))
    // Fallback 404 handler
    .fallback(not_found_handler);
```

---

## 4. Zero-Disk Static Documentation Serving

Serving Swagger/Scalar OpenAPI documentation via dynamic filesystem reads wastes disk I/O and leaks file descriptors.

**Best Practice**: Embed documentation HTML templates at compile time using string literals:

```rust
use axum::response::{Html, IntoResponse};

const SCALAR_HTML: &str = r#"<!doctype html>
<html>
  <head><title>API Reference</title></head>
  <body>
    <script id="api-reference" data-url="/openapi.json" src="https://cdn.jsdelivr.net/npm/@scalar/api-reference"></script>
  </body>
</html>"#;

pub async fn doc_handler() -> impl IntoResponse {
    Html(SCALAR_HTML)
}
```
Zero disk I/O, zero runtime template engines, zero heap allocation.
