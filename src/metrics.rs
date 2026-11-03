use axum::{
    body::Body,
    extract::{MatchedPath, Request},
    http::header::USER_AGENT,
    middleware::Next,
    response::{IntoResponse, Response},
};
use dashmap::DashMap;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// Global metrics data registry
pub struct RequestMetric {
    pub count: AtomicU64,
    pub duration_sum_micros: AtomicU64,
}

// HTTP request statistics grouped by "method=\"...\",path=\"...\",status=\"...\""
pub static HTTP_REQUESTS: LazyLock<DashMap<String, RequestMetric>> = LazyLock::new(DashMap::new);

// Active in-flight requests grouped by method
pub static HTTP_IN_FLIGHT: LazyLock<DashMap<String, AtomicU64>> = LazyLock::new(DashMap::new);

// Global process and system stats
pub static METRICS_START_TIME: AtomicU64 = AtomicU64::new(0);
pub static METRICS_RSS: AtomicU64 = AtomicU64::new(0);
pub static METRICS_VSZ: AtomicU64 = AtomicU64::new(0);
pub static METRICS_LAG: AtomicU64 = AtomicU64::new(0); // represented as micro-seconds
pub static METRICS_CPU_TOTAL: AtomicU64 = AtomicU64::new(0); // represented as micro-seconds
pub static METRICS_DISK_READ: AtomicU64 = AtomicU64::new(0);
pub static METRICS_DISK_WRITE: AtomicU64 = AtomicU64::new(0);

pub static SYSTEM_TOTAL_MEM: AtomicU64 = AtomicU64::new(0);
pub static SYSTEM_USED_MEM: AtomicU64 = AtomicU64::new(0);

pub fn init_metrics() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    METRICS_START_TIME.store(now, Ordering::Relaxed);

    // Spawn background task to collect system statistics every 15 seconds
    tokio::spawn(async move {
        let mut sys = sysinfo::System::new();
        let pid = sysinfo::Pid::from(std::process::id() as usize);
        let mut last_cpu_total_secs = 0.0;

        // Initial refresh
        sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), false);
        sys.refresh_memory();
        SYSTEM_TOTAL_MEM.store(sys.total_memory(), Ordering::Relaxed);
        SYSTEM_USED_MEM.store(sys.used_memory(), Ordering::Relaxed);
        if let Some(proc) = sys.process(pid) {
            METRICS_RSS.store(proc.memory(), Ordering::Relaxed);
            METRICS_VSZ.store(proc.virtual_memory(), Ordering::Relaxed);
        }

        loop {
            tokio::time::sleep(Duration::from_secs(15)).await;

            // Trim freed heap pages back to the kernel OS (glibc arena release)
            crate::util::trim_memory();

            // Event loop lag measurement (scheduler lag)
            let start = Instant::now();
            tokio::spawn(async move {
                let lag = start.elapsed();
                let lag_micros = lag.as_micros() as u64;
                METRICS_LAG.store(lag_micros, Ordering::Relaxed);
            });

            // Refresh process & system memory stats (only current process)
            sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), false);
            sys.refresh_memory();

            SYSTEM_TOTAL_MEM.store(sys.total_memory(), Ordering::Relaxed);
            SYSTEM_USED_MEM.store(sys.used_memory(), Ordering::Relaxed);

            if let Some(proc) = sys.process(pid) {
                METRICS_RSS.store(proc.memory(), Ordering::Relaxed);
                METRICS_VSZ.store(proc.virtual_memory(), Ordering::Relaxed);

                let disk_usage = proc.disk_usage();
                METRICS_DISK_READ.store(disk_usage.total_read_bytes, Ordering::Relaxed);
                METRICS_DISK_WRITE.store(disk_usage.total_written_bytes, Ordering::Relaxed);

                // Accumulate CPU usage
                let cpu_pct = proc.cpu_usage() as f64;
                let cpu_delta = (cpu_pct / 100.0) * 15.0;
                last_cpu_total_secs += cpu_delta;

                let cpu_micros = (last_cpu_total_secs * 1_000_000.0) as u64;
                METRICS_CPU_TOTAL.store(cpu_micros, Ordering::Relaxed);
            }
        }
    });
}

pub async fn metrics_handler() -> impl IntoResponse {
    let rss = METRICS_RSS.load(Ordering::Relaxed);
    let vsz = METRICS_VSZ.load(Ordering::Relaxed);
    let lag_secs = METRICS_LAG.load(Ordering::Relaxed) as f64 / 1_000_000.0;
    let cpu_secs = METRICS_CPU_TOTAL.load(Ordering::Relaxed) as f64 / 1_000_000.0;
    let uptime = METRICS_START_TIME.load(Ordering::Relaxed);
    let disk_read = METRICS_DISK_READ.load(Ordering::Relaxed);
    let disk_write = METRICS_DISK_WRITE.load(Ordering::Relaxed);

    let system_total_mem = SYSTEM_TOTAL_MEM.load(Ordering::Relaxed);
    let system_used_mem = SYSTEM_USED_MEM.load(Ordering::Relaxed);
    let system_free_mem = system_total_mem.saturating_sub(system_used_mem);

    // Tokio runtime stats
    let handle = tokio::runtime::Handle::current();
    let tokio_metrics = handle.metrics();
    let tokio_workers = tokio_metrics.num_workers();
    let tokio_queue_depth = tokio_metrics.global_queue_depth();

    let mut out = String::new();

    // 1. Process CPU / RAM / Disk / VSZ
    out.push_str("# HELP process_resident_set_size_bytes Resident Set Size — total physical RAM used by the process\n");
    out.push_str("# TYPE process_resident_set_size_bytes gauge\n");
    out.push_str(&format!("process_resident_set_size_bytes {}\n\n", rss));

    out.push_str("# HELP process_virtual_memory_bytes Virtual Memory Size — total virtual memory size of the process\n");
    out.push_str("# TYPE process_virtual_memory_bytes gauge\n");
    out.push_str(&format!("process_virtual_memory_bytes {}\n\n", vsz));

    out.push_str(
        "# HELP process_start_time_seconds Start time of the process since unix epoch (seconds)\n",
    );
    out.push_str("# TYPE process_start_time_seconds gauge\n");
    out.push_str(&format!("process_start_time_seconds {}\n\n", uptime));

    out.push_str("# HELP process_cpu_seconds_total Total user + system CPU time spent (seconds)\n");
    out.push_str("# TYPE process_cpu_seconds_total counter\n");
    out.push_str(&format!("process_cpu_seconds_total {:.6}\n\n", cpu_secs));

    out.push_str("# HELP process_disk_read_bytes_total Total disk read bytes\n");
    out.push_str("# TYPE process_disk_read_bytes_total counter\n");
    out.push_str(&format!("process_disk_read_bytes_total {}\n\n", disk_read));

    out.push_str("# HELP process_disk_written_bytes_total Total disk written bytes\n");
    out.push_str("# TYPE process_disk_written_bytes_total counter\n");
    out.push_str(&format!(
        "process_disk_written_bytes_total {}\n\n",
        disk_write
    ));

    out.push_str("# HELP eventloop_lag_seconds Event loop lag — delay of the event loop for synchronous blocking detection\n");
    out.push_str("# TYPE eventloop_lag_seconds gauge\n");
    out.push_str(&format!("eventloop_lag_seconds {:.6}\n\n", lag_secs));

    // 2. System RAM Stats
    out.push_str("# HELP system_total_memory_bytes Total physical system memory (bytes)\n");
    out.push_str("# TYPE system_total_memory_bytes gauge\n");
    out.push_str(&format!(
        "system_total_memory_bytes {}\n\n",
        system_total_mem
    ));

    out.push_str("# HELP system_used_memory_bytes Used physical system memory (bytes)\n");
    out.push_str("# TYPE system_used_memory_bytes gauge\n");
    out.push_str(&format!("system_used_memory_bytes {}\n\n", system_used_mem));

    out.push_str("# HELP system_free_memory_bytes Free physical system memory (bytes)\n");
    out.push_str("# TYPE system_free_memory_bytes gauge\n");
    out.push_str(&format!("system_free_memory_bytes {}\n\n", system_free_mem));

    // 3. Tokio Runtime Stats
    out.push_str("# HELP tokio_worker_threads Number of Tokio worker threads\n");
    out.push_str("# TYPE tokio_worker_threads gauge\n");
    out.push_str(&format!("tokio_worker_threads {}\n\n", tokio_workers));

    out.push_str("# HELP tokio_global_queue_depth Global Tokio scheduler task queue depth\n");
    out.push_str("# TYPE tokio_global_queue_depth gauge\n");
    out.push_str(&format!(
        "tokio_global_queue_depth {}\n\n",
        tokio_queue_depth
    ));

    // 4. HTTP Labeled Metrics
    out.push_str("# HELP http_requests_in_flight Number of HTTP requests currently being processed (active)\n");
    out.push_str("# TYPE http_requests_in_flight gauge\n");
    for item in HTTP_IN_FLIGHT.iter() {
        let method = item.key();
        let val = item.value().load(Ordering::Relaxed);
        out.push_str(&format!(
            "http_requests_in_flight{{method=\"{}\"}} {}\n",
            method, val
        ));
    }
    out.push('\n');

    out.push_str("# HELP http_requests_total Total number of HTTP requests processed\n");
    out.push_str("# TYPE http_requests_total counter\n");
    for item in HTTP_REQUESTS.iter() {
        let label_key = item.key();
        let val = item.value().count.load(Ordering::Relaxed);
        out.push_str(&format!("http_requests_total{{{}}} {}\n", label_key, val));
    }
    out.push('\n');

    out.push_str("# HELP http_request_duration_seconds Total request duration in seconds\n");
    out.push_str("# TYPE http_request_duration_seconds counter\n");
    for item in HTTP_REQUESTS.iter() {
        let label_key = item.key();
        let sum_secs =
            item.value().duration_sum_micros.load(Ordering::Relaxed) as f64 / 1_000_000.0;
        let count = item.value().count.load(Ordering::Relaxed);
        out.push_str(&format!(
            "http_request_duration_seconds_sum{{{}}} {:.6}\n",
            label_key, sum_secs
        ));
        out.push_str(&format!(
            "http_request_duration_seconds_count{{{}}} {}\n",
            label_key, count
        ));
    }

    (
        [("Content-Type", "text/plain; version=0.0.4; charset=utf-8")],
        out,
    )
}

/// RAII guard ensuring in-flight metric counter is always decremented,
/// even if request future is cancelled or aborted by client (PRD J2 / P1-7)
pub struct InFlightGuard<'a>(&'a str);

impl<'a> Drop for InFlightGuard<'a> {
    fn drop(&mut self) {
        if let Some(counter) = HTTP_IN_FLIGHT.get(self.0) {
            counter.fetch_sub(1, Ordering::Relaxed);
        }
    }
}

pub async fn telemetry_middleware(req: Request<Body>, next: Next) -> Response {
    let method = req.method().clone();
    let method_str = method.to_string();
    let raw_path = req.uri().path().to_string();

    // Prevent /metrics endpoint from tracking itself
    if raw_path == "/metrics" {
        return next.run(req).await;
    }

    // STRICT route normalization: Only use the matched route template (e.g. /pornhub/get),
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

    let headers = req.headers();
    let ip = headers
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim())
        .or_else(|| headers.get("x-real-ip").and_then(|h| h.to_str().ok()))
        .unwrap_or("unknown")
        .to_string();

    let user_agent = headers
        .get(USER_AGENT)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    tracing::debug!(
        "Incoming request: {} {} | IP: {} | UA: {}",
        method,
        raw_path,
        ip,
        user_agent
    );

    // Track active requests count by method via RAII guard
    HTTP_IN_FLIGHT
        .entry(method_str.clone())
        .or_insert_with(|| AtomicU64::new(0))
        .fetch_add(1, Ordering::Relaxed);
    let _guard = InFlightGuard(&method_str);

    let start = Instant::now();
    let res = next.run(req).await;
    let elapsed = start.elapsed();

    let status_code = res.status().as_u16().to_string();

    // Record request count and duration grouped by normalized route labels
    let key = format!(
        "method=\"{}\",path=\"{}\",status=\"{}\"",
        method_str, matched_path, status_code
    );
    let metric = HTTP_REQUESTS.entry(key).or_insert_with(|| RequestMetric {
        count: AtomicU64::new(0),
        duration_sum_micros: AtomicU64::new(0),
    });
    metric.count.fetch_add(1, Ordering::Relaxed);
    metric
        .duration_sum_micros
        .fetch_add(elapsed.as_micros() as u64, Ordering::Relaxed);

    // Actively trim freed glibc heap pages after request AST DOM is dropped
    crate::util::trim_memory();

    res
}
