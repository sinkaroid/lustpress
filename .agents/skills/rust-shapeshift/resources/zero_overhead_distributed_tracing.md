# Zero-Overhead Distributed Tracing and Observability

The `tracing` and `opentelemetry` crates provide world-class distributed observability in Rust. However, naive usage—such as using `#[instrument]` on hot inner loops, formatting dynamic strings into span fields, or performing synchronous logging on Tokio worker threads—can cause 20% to 40% CPU degradation and severe heap churn.

---

## 1. Compile-Time Static Level Filtering

Eliminate disabled trace levels completely from the compiled binary. If `trace!` and `debug!` statements are never enabled in production, remove their branching and metadata overhead at compile time in `Cargo.toml`:

```toml
[dependencies]
tracing = { version = "0.1", features = ["release_max_level_info"] }
log = { version = "0.4", features = ["release_max_level_info"] }
```
> With `release_max_level_info`, the compiler strips every `trace!` and `debug!` call from release builds, reducing binary size and eliminating runtime condition checks.

---

## 2. The `#[instrument]` Anti-Pattern on Hot Paths

### The Trap
Placing `#[instrument]` on functions called thousands of times per second allocates a `Span` struct, initializes span metadata, and registers enter/exit timestamps on every invocation:

```rust
// ANTI-PATTERN: Called 50,000 times per second in a parser or socket loop
#[tracing::instrument(skip(payload))]
pub fn parse_packet(payload: &[u8]) -> Result<Packet, ParseError> {
    // Allocates Span and records timestamps on every packet!
}
```

### The Solution: Selective Root Instrumentation
Only instrument top-level boundaries (HTTP route handlers, RPC entry points, message queue consumer messages). Use zero-allocation scalar fields inside:

```rust
// GOOD: Instrument top-level gateway request only
#[tracing::instrument(
    name = "http_request",
    skip(payload),
    fields(
        req_id = %req_id,
        bytes = payload.len(), // Zero allocation integer primitive
    )
)]
pub async fn handle_gateway_request(req_id: &str, payload: &[u8]) -> Response {
    // Inner utility functions run without tracing overhead
    let packet = parse_packet_fast(payload)?;
    process(packet).await
}
```

---

## 3. Non-Blocking Async Telemetry with `tracing-appender`

Writing logs or traces synchronously to `stdout` or disk blocks Tokio worker threads, stalling async event loops. Decouple logging onto a dedicated background worker thread with a non-blocking bounded queue:

```rust
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, fmt};

pub fn init_zero_overhead_tracing() -> tracing_appender::non_blocking::WorkerGuard {
    // Create non-blocking stdout writer on dedicated background thread
    let (non_blocking, guard) = tracing_appender::non_blocking(std::io::stdout());

    let subscriber = tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(
            fmt::layer()
                .with_writer(non_blocking)
                .with_target(false)
                .with_thread_ids(false) // Saves timestamp/ID formatting overhead
                .compact(),
        );

    subscriber.init();

    // MUST retain guard in main() until shutdown to flush remaining buffered logs
    guard
}
```

---

## 4. Zero-Allocation Span Fields

Avoid formatting dynamic strings into span fields (`format!("{}", user_id)`). Use native borrowing or scalar primitives:

```rust
use tracing::{info_span, Span};

pub fn record_transaction_span(account_id: u64, amount: u64) -> Span {
    // Passes primitives directly to tracing subscriber (0 heap allocations)
    info_span!(
        "account_tx",
        account.id = account_id,
        tx.amount = amount,
    )
}
```

---

## 5. Production Checklist

- [ ] Enabled `release_max_level_info` in `Cargo.toml` to strip debug spans in release builds.
- [ ] Removed all `#[instrument]` macros from loops, parser functions, and hot-path handlers.
- [ ] Wrapped `stdout` / file loggers in `tracing_appender::non_blocking` with dedicated `WorkerGuard`.
- [ ] Span fields avoid `format!` and use borrowed `&str` or numeric types (`u64`, `usize`).
