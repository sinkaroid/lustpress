# Production Telemetry & Zero-Dependency Prometheus Metrics

Many Rust applications pull in bloated telemetry crates (`prometheus`, `opentelemetry`) that add megabytes to binary size and create subtle memory leaks through unconstrained label dimensions.

This reference guide provides a **lightweight, zero-dependency Prometheus metrics exporter** that tracks OS process memory, Tokio scheduler health, event loop lag, and HTTP requests with strict cardinality containment.

---

## 1. Single-Process PID-Filtered Sysinfo Collection

Calling `sysinfo::System::new_all()` or refreshing all system processes scans `/proc` for every PID on the machine, wasting CPU and memory.

**Best Practice**: Filter exclusively to the running process's own PID:

```rust
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

pub static METRICS_RSS: AtomicU64 = AtomicU64::new(0);
pub static METRICS_VSZ: AtomicU64 = AtomicU64::new(0);
pub static SYSTEM_TOTAL_MEM: AtomicU64 = AtomicU64::new(0);
pub static SYSTEM_USED_MEM: AtomicU64 = AtomicU64::new(0);

pub fn spawn_metrics_collector() {
    tokio::spawn(async move {
        let mut sys = sysinfo::System::new();
        let pid = sysinfo::Pid::from(std::process::id() as usize);

        loop {
            tokio::time::sleep(Duration::from_secs(15)).await;

            // 1. Actively trim freed glibc heap pages back to the Linux kernel
            crate::util::trim_memory();

            // 2. Measure Tokio Event Loop Lag
            measure_event_loop_lag();

            // 3. Refresh ONLY our own process, NOT the entire system process table!
            sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), false);
            sys.refresh_memory();

            SYSTEM_TOTAL_MEM.store(sys.total_memory(), Ordering::Relaxed);
            SYSTEM_USED_MEM.store(sys.used_memory(), Ordering::Relaxed);

            if let Some(proc) = sys.process(pid) {
                METRICS_RSS.store(proc.memory(), Ordering::Relaxed);
                METRICS_VSZ.store(proc.virtual_memory(), Ordering::Relaxed);
            }
        }
    });
}
```

---

## 2. Tokio Event Loop Lag Measurement

Event loop lag measures the delay between when an async task is scheduled and when a Tokio worker thread actually executes it. Elevated lag indicates synchronous blocking calls on the async reactor:

```rust
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

pub static METRICS_LAG: AtomicU64 = AtomicU64::new(0); // in microseconds

fn measure_event_loop_lag() {
    let start = Instant::now();
    tokio::spawn(async move {
        let lag_micros = start.elapsed().as_micros() as u64;
        METRICS_LAG.store(lag_micros, Ordering::Relaxed);
    });
}
```

---

## 3. Strict Label Cardinality Protection in Axum Middleware

Tracking endpoints like `/search?q=foo` or `/gallery/12345` creates an infinite number of unique labels in metric hash maps, causing gigabytes of RAM bloat.

Always normalize via `axum::extract::MatchedPath`:

```rust
use axum::{
    body::Body,
    extract::{MatchedPath, Request},
    middleware::Next,
    response::Response,
};

pub async fn telemetry_middleware(req: Request<Body>, next: Next) -> Response {
    let raw_path = req.uri().path().to_string();

    // Prevent /metrics endpoint from tracking itself
    if raw_path == "/metrics" {
        return next.run(req).await;
    }

    // STRICT route normalization: Only record matched templates (e.g. /api/v1/items/:id),
    // NEVER raw path or query params (?id=..., ?key=...) to prevent cardinality explosion.
    let matched_path = req
        .extensions()
        .get::<MatchedPath>()
        .map(|mp| mp.as_str().to_string())
        .unwrap_or_else(|| {
            if raw_path == "/" {
                "/".to_string()
            } else {
                "/unmatched".to_string()
            }
        });

    let method = req.method().to_string();
    let start = std::time::Instant::now();

    let res = next.run(req).await;
    let elapsed = start.elapsed();
    let status = res.status().as_u16().to_string();

    // Safely record with finite, bounded label key
    let key = format!("method=\"{method}\",path=\"{matched_path}\",status=\"{status}\"");
    record_request(&key, elapsed);

    // Release unmapped heap pages
    crate::util::trim_memory();

    res
}
```

---

## 4. Zero-Dependency Prometheus Spec 0.0.4 Exporter

Format metrics directly into a standard text payload without external crates:

```rust
use axum::response::IntoResponse;

pub async fn metrics_handler() -> impl IntoResponse {
    let rss = METRICS_RSS.load(Ordering::Relaxed);
    let vsz = METRICS_VSZ.load(Ordering::Relaxed);
    let lag_secs = METRICS_LAG.load(Ordering::Relaxed) as f64 / 1_000_000.0;

    let handle = tokio::runtime::Handle::current();
    let tokio_metrics = handle.metrics();
    let workers = tokio_metrics.num_workers();
    let queue_depth = tokio_metrics.global_queue_depth();

    let mut out = String::with_capacity(4096);

    out.push_str("# HELP process_resident_set_size_bytes Total physical RAM used by the process\n");
    out.push_str("# TYPE process_resident_set_size_bytes gauge\n");
    out.push_str(&format!("process_resident_set_size_bytes {}\n\n", rss));

    out.push_str("# HELP eventloop_lag_seconds Tokio scheduler delay for blocking detection\n");
    out.push_str("# TYPE eventloop_lag_seconds gauge\n");
    out.push_str(&format!("eventloop_lag_seconds {:.6}\n\n", lag_secs));

    out.push_str("# HELP tokio_worker_threads Number of Tokio worker threads\n");
    out.push_str("# TYPE tokio_worker_threads gauge\n");
    out.push_str(&format!("tokio_worker_threads {}\n\n", workers));

    out.push_str("# HELP tokio_global_queue_depth Global Tokio scheduler task queue depth\n");
    out.push_str("# TYPE tokio_global_queue_depth gauge\n");
    out.push_str(&format!("tokio_global_queue_depth {}\n\n", queue_depth));

    ([("Content-Type", "text/plain; version=0.0.4; charset=utf-8")], out)
}
```
