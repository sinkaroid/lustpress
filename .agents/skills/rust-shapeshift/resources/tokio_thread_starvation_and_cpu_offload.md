# Tokio Thread Starvation & CPU Workload Offloading

Tokio is a **cooperative asynchronous runtime**, not a preemptive one. Unlike runtimes with preemptive schedulers that can interrupt long-running tasks, a Tokio worker thread executes an asynchronous task continuously until that task voluntarily yields control at an `.await` boundary.

If a worker thread runs heavy, synchronous CPU-bound logic (DOM parsing, large regex scans, decompression, or cryptography), that thread is **blocked**. The I/O reactor on that thread cannot process incoming TCP sockets, leading to severe latency spikes, socket timeouts, and degraded p99 metrics.

---

## 1. The Cooperative Scheduling Trap

Tokio defaults to creating worker threads equal to the number of logical CPU cores (`num_cpus`):

```text
Incoming HTTP Request ---> [ Tokio Worker 1 ] (Runs synchronous CPU work for 25ms)
Incoming Health Check ---> [ Tokio Worker 1 ] (STARVED! Waits 25ms behind CPU work)
```

If 4 concurrent requests trigger synchronous parsing on a 4-core server, **100% of Tokio worker threads are blocked**. All other connections, including Kubernetes liveness probes (`/healthz`) and Prometheus scrapers (`/metrics`), stall.

### Rule of Thumb
Any synchronous CPU operation expected to take **more than 100 µs to 1 ms** MUST NOT run directly on a Tokio worker thread.

---

## 2. Offloading with `tokio::task::spawn_blocking`

For synchronous operations that take a few milliseconds (e.g., parsing a pruned HTML document or decoding a large payload), offload execution to Tokio's dedicated blocking threadpool:

```rust
use axum::{extract::State, response::IntoResponse, Json};
use std::sync::Arc;

pub async fn parse_document_handler(
    State(state): State<Arc<AppState>>,
    raw_body: String,
) -> Result<impl IntoResponse, AppError> {
    // Prune the raw string on the async thread (nanosecond slice)
    let pruned = crate::pruning::slice_html(&raw_body);
    let pruned_owned = pruned.to_string();

    // 🚀 OFFLOAD CPU-BOUND PARSING TO BLOCKING POOL
    let parsed_items = tokio::task::spawn_blocking(move || {
        crate::parser::extract_cards(&pruned_owned)
    })
    .await
    .map_err(|join_err| AppError::Internal(format!("Blocking task panicked: {join_err}")))?;

    Ok(Json(parsed_items))
}
```

### Why this protects throughput:
- Tokio's blocking threadpool dynamically spins up to 512 threads dedicated purely to synchronous blocking tasks.
- Tokio worker threads remain **100% responsive** to I/O events, TCP handshakes, and immediate async handlers.

---

## 3. High-Throughput Parallelism with `rayon`

When a single request requires intensive CPU processing across multiple chunks (e.g., parsing 50 large provider manifests or running parallel batch transformations), `rayon` provides work-stealing parallelism:

```toml
# Cargo.toml
[dependencies]
rayon = "1.10"
```

```rust
use rayon::prelude::*;

pub async fn batch_process_items(raw_chunks: Vec<String>) -> Vec<ProcessedCard> {
    tokio::task::spawn_blocking(move || {
        raw_chunks
            .par_iter()
            .map(|chunk| crate::parser::parse_single_chunk(chunk))
            .collect()
    })
    .await
    .unwrap_or_default()
}
```

> **Warning**: Never call Tokio's async `.await` inside a `rayon` parallel iterator. Rayon is strictly for synchronous CPU computations.

---

## 4. Measuring Event Loop Health

To verify that worker threads are never starved, continuously measure Tokio event loop lag:

```rust
use std::time::{Duration, Instant};

pub fn spawn_event_loop_monitor() {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(500));
        loop {
            let start = Instant::now();
            interval.tick().await;
            let elapsed = start.elapsed();

            // If the 500ms timer takes >510ms, worker threads are experiencing lag
            let lag = elapsed.saturating_sub(Duration::from_millis(500));
            if lag > Duration::from_millis(10) {
                tracing::warn!(
                    lag_ms = lag.as_millis(),
                    "Tokio worker thread starvation detected!"
                );
            }
        }
    });
}
```

---

## 5. Summary Checklist

| Task Type | Execution Strategy | Target Threadpool |
|---|---|---|
| Network I/O (`reqwest`, `redis`, TCP) | `.await` | Tokio Worker Pool |
| Fast string slicing ($\le 100$ µs) | Direct inline execution | Tokio Worker Pool |
| Medium DOM / JSON parsing (1ms–15ms) | `tokio::task::spawn_blocking` | Tokio Blocking Pool |
| Massive batch CPU calculations | `rayon` inside `spawn_blocking` | Rayon Work-Stealing Pool |
