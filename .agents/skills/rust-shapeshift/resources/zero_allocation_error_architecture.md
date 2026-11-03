# Zero-Allocation Error Architecture & Result Size Optimization

In high-throughput services, error handling is frequently overlooked as a source of memory bloat. Two primary hazards compromise performance:
1. **The `Result<T, E>` Stack Bloat**: The memory size of `Result<T, E>` is determined by `max(sizeof(T), sizeof(E)) + alignment`. A bloated error enum forces every single function call returning `Result` to allocate massive stack frames, even when the `Ok` branch is taken 99.9% of the time.
2. **Hot-Path Error Heap Churn**: Creating error instances using `format!` or `String::from` triggers heap allocations on every validation failure, rate limit breach, or missing parameter. Under high traffic or bot fuzzing, error generation alone degrades allocator performance.

This reference guide documents how to construct lean, zero-allocation error hierarchies.

---

## 1. The Hidden Stack Cost of Large Error Enums

Consider this unoptimized domain error:

```rust
// ❌ WRONG: Bloats sizeof(AppError) to 128+ bytes!
pub enum BloatedError {
    NotFound(&'static str),                 // 16 bytes
    Validation(String),                     // 24 bytes
    Database(diesel::result::Error),        // 32 bytes
    UpstreamNetwork(reqwest::Error),        // 120+ bytes!
}
```

Because `UpstreamNetwork` is 120+ bytes:
- `std::mem::size_of::<BloatedError>()` is **~128 bytes**.
- Any function returning `Result<u32, BloatedError>` consumes **136 bytes on the stack** on every call frame!
- Deep call stacks multiply this footprint across thousands of concurrent Tokio tasks, increasing cache misses.

---

## 2. Boxing Rare Error Payloads

Errors from upstream libraries (e.g., `reqwest::Error`, `redis::RedisError`) are exceptional events. They should never dictate the baseline stack size of your domain `Result`.

**The Fix**: Box large or rare error variants:

```rust
// ✅ CORRECT: Compact Error Enum (sizeof <= 24 bytes)
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Not Found: {0}")]
    NotFound(&'static str), // 16 bytes

    #[error("Validation Error: {0}")]
    Validation(compact_str::CompactString), // 24 bytes (0 heap alloc for <=24B)

    #[error("Rate Limit Exceeded")]
    RateLimited, // 0 bytes

    // 🚀 Box large errors: shrinks enum variant footprint to an 8-byte pointer!
    #[error("Upstream HTTP Error: {0}")]
    Upstream(Box<reqwest::Error>), // 8 bytes

    #[error("Database Error: {0}")]
    Database(Box<deadpool_redis::PoolError>), // 8 bytes
}
```

By boxing large variants, `std::mem::size_of::<AppError>()` drops from **128 bytes to 24 bytes**, reducing stack frame overhead by over **80%** across every function in the application.

---

## 3. Eliminating Dynamic `format!` on Error Paths

Never format dynamic error strings on high-frequency error paths (e.g., input validation, authentication, rate limiting):

```rust
// ❌ WRONG: Allocates a new String on the heap on EVERY failed request
return Err(AppError::Validation(format!("Parameter '{}' is missing or invalid", key)));

// ✅ CORRECT: Zero-allocation static context or CompactString
use compact_str::format_compact;

// Path A: Pure static string (0 heap allocations)
return Err(AppError::NotFound("Video resource not found"));

// Path B: Dynamic string under 24 bytes (inlined on stack; 0 heap allocations)
return Err(AppError::Validation(format_compact!("Invalid ID: {id}")));
```

---

## 4. Zero-Allocation Axum `IntoResponse` Mapping

Map domain errors directly into HTTP responses without converting messages into dynamic JSON strings:

```rust
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Serialize)]
struct ErrorResponseBody<'a> {
    success: bool,
    error: &'a str,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, *msg),
            AppError::Validation(msg) => (StatusCode::BAD_REQUEST, msg.as_str()),
            AppError::RateLimited => (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded. Please slow down."),
            AppError::Upstream(_) => (StatusCode::BAD_GATEWAY, "Upstream provider error"),
            AppError::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal persistence error"),
        };

        // Borrowed response body: zero string allocation during JSON serialization
        let body = Json(ErrorResponseBody {
            success: false,
            error: message,
        });

        (status, body).into_response()
    }
}
```

---

## 5. Error Optimization Checklist

1. Has `std::mem::size_of::<AppError>()` been verified to be $\le 32$ bytes?
2. Are external library errors (`reqwest::Error`, `diesel::result::Error`) wrapped in `Box<...>`?
3. Are frequent error messages constructed using `&'static str` or `format_compact!` instead of standard `format!`?
4. Does `IntoResponse` serialize error payloads using borrowed `&str` references?
