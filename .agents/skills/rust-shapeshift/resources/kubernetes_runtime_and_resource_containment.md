# Kubernetes Runtime and Resource Containment

Deploying Rust services to Kubernetes (K8s) introduces distinct orchestrator-level constraints that do not exist in standalone Docker. Misconfigurations in cgroups, worker thread counts, memory limits, probe design, or termination signals frequently cause unexpected CPU throttling, `OOMKilled (Exit Code 137)` terminations, and `502 Bad Gateway` errors during rolling deployments.

---

## 1. The CFS Quota & Tokio Thread Explosion Trap

### The Problem
In Kubernetes, pods define CPU limits using CFS (Completely Fair Scheduler) quotas (e.g., `resources.limits.cpu: "1"` or `"500m"`). However, the physical worker node may possess 64 or 128 physical vCPUs.

By default, `tokio::runtime::Builder::new_multi_thread()` or `#[tokio::main]` calls `std::thread::available_parallelism()`. In many Linux kernel / container configurations, this reads the **node's total CPU cores (128)** rather than the pod's quota. Spawning 128 Tokio worker threads inside a 1-core container causes:
1. Massive per-thread stack allocation (128 × thread stack size + per-thread glibc arenas).
2. Catastrophic context-switching thrashing.
3. Severe CFS bandwidth throttling, driving latency from milliseconds into seconds.

### The Solution: Quota-Aware Runtime Sizing
Explicitly cap Tokio worker threads based on container CPU limits or environment variables:

```rust
use std::env;

fn build_optimized_runtime() -> tokio::runtime::Runtime {
    // Read from K8s downward API env (e.g., CONTAINER_CPU_LIMIT=2) or fallback
    let worker_threads = env::var("TOKIO_WORKER_THREADS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or_else(|| {
            // Alternatively clamp available_parallelism to realistic container quota
            std::thread::available_parallelism()
                .map(|p| p.get().clamp(1, 4)) // Never spawn unbounded threads on host
                .unwrap_or(2)
        });

    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(worker_threads)
        .max_blocking_threads(16) // Prevent unbounded blocking pool
        .enable_all()
        .build()
        .expect("Failed to initialize Tokio runtime")
}

fn main() {
    let rt = build_optimized_runtime();
    rt.block_on(async_main());
}

async fn async_main() {
    // Application logic here
}
```

In your Kubernetes Deployment YAML, inject the thread count via Downward API:
```yaml
spec:
  containers:
  - name: my-service
    resources:
      requests:
        cpu: "500m"
        memory: "64Mi"
      limits:
        cpu: "1"
        memory: "128Mi"
    env:
    - name: TOKIO_WORKER_THREADS
      value: "2"
```

---

## 2. Preventing `OOMKilled` (Exit Code 137)

### Cgroups Memory Limit vs RSS
When a container exceeds `resources.limits.memory`, the Linux kernel cgroups OOM-killer immediately dispatches `SIGKILL (Exit Code 137)`. Unlike garbage-collected runtimes, Rust does not perform GC sweeps under memory pressure.

### Best Practice: Guaranteed QoS & Threshold Trimming
1. **Match Requests and Limits (Guaranteed QoS)**: In memory-critical services, set `requests.memory == limits.memory`. This prevents the K8s node from overcommitting memory and evicting your pod when other noisy neighbor pods spike.
2. **Internal High-Watermark Trim**: If running with `glibc`, trigger `libc::malloc_trim(0)` when memory reaches 80% of container limits. If running with `tikv-jemallocator`, ensure `dirty_decay_ms:1000,muzzy_decay_ms:1000` is active.

```yaml
resources:
  requests:
    cpu: "1"
    memory: "128Mi"
  limits:
    cpu: "1"
    memory: "128Mi" # Guaranteed QoS: lowest eviction priority
```

---

## 3. Zero-Downtime Deployment & Graceful SIGTERM Handling

### The Ingress 502 Race Condition
During rolling updates (`kubectl rollout restart`):
1. K8s removes the Pod IP from the Service Endpoints.
2. Simultaneously, K8s sends `SIGTERM` to the container process.
3. Kube-proxy / Ingress controllers take **1 to 3 seconds** to propagate the route removal across all nodes.
4. If your Rust service exits immediately upon receiving `SIGTERM`, clients whose requests were in-flight or routed during that 2-second window receive `502 Bad Gateway` or `Connection Reset`.

### The Solution: Pre-Stop Sleep & Dual Signal Draining
In Kubernetes, configure a `preStop` hook to delay `SIGTERM` until kube-proxy has deregistered the pod, and handle graceful shutdown in Rust:

```yaml
lifecycle:
  preStop:
    exec:
      command: ["/bin/sh", "-c", "sleep 3"]
```

In Rust (`Axum` shutdown signal):
```rust
use tokio::signal::unix::{signal, SignalKind};

pub async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for Ctrl+C");
    };

    #[cfg(unix)]
    let terminate = async {
        let mut sigterm = signal(SignalKind::terminate())
            .expect("Failed to listen for SIGTERM");
        sigterm.recv().await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Received termination signal. Draining in-flight requests...");
}
```

---

## 4. Probe Architecture (Liveness vs Readiness vs Startup)

Never bind expensive downstream checks (e.g. database ping, Redis cluster scan) to your **Liveness Probe**. If PostgreSQL experiences a temporary 5-second query spike, K8s will consider all Rust pods dead, kill them simultaneously, and trigger a catastrophic cluster-wide crash loop.

### Probe Separation of Concerns

| Probe Type | Purpose | Frequency | Safe Implementation |
| :--- | :--- | :--- | :--- |
| **Startup** | Allows cold start / migrations | 1s | Check schema migration or state init |
| **Liveness** | Checks if process / reactor is alive | 10s | Lightweight HTTP 200 OK (no external DB calls) |
| **Readiness** | Checks if pod can accept incoming traffic | 5s | Check DB pool available connections & Redis ping |

```rust
use axum::{routing::get, Router, http::StatusCode, extract::State};
use std::sync::Arc;

pub fn probe_routes() -> Router<Arc<AppState>> {
    Router::new()
        // Liveness: Zero allocation, purely tests if Tokio event loop responds
        .route("/healthz/live", get(|| async { StatusCode::OK }))
        // Readiness: Verifies database pool has available connections
        .route("/healthz/ready", get(readiness_handler))
}

async fn readiness_handler(State(state): State<Arc<AppState>>) -> StatusCode {
    // Fast check: verify at least 1 connection can be acquired within 500ms
    match tokio::time::timeout(
        std::time::Duration::from_millis(500),
        state.db_pool.acquire(),
    ).await {
        Ok(Ok(_conn)) => StatusCode::OK,
        _ => StatusCode::SERVICE_UNAVAILABLE, // Temporarily stops traffic routing without killing pod
    }
}
```

---

## 5. Security & Read-Only Root Filesystem

Rust compiled binaries do not require root privileges, package managers, or writable filesystems at runtime:

```yaml
securityContext:
  runAsNonRoot: true
  runAsUser: 10001
  runAsGroup: 10001
  readOnlyRootFilesystem: true
  allowPrivilegeEscalation: false
  capabilities:
    drop:
    - ALL
```

---

## 6. Production Checklist

- [ ] Tokio worker threads explicitly bounded (`TOKIO_WORKER_THREADS`) to match container CPU limits.
- [ ] Memory `requests` equal `limits` (Guaranteed QoS) to avoid noisy-neighbor eviction.
- [ ] Ingress 502 race avoided via `preStop: sleep 3` or delayed signal drain.
- [ ] Liveness probe isolated from database/cache dependencies.
- [ ] Readiness probe fails gracefully (`503 Service Unavailable`) without triggering pod restarts.
- [ ] Binary runs as non-root with `readOnlyRootFilesystem: true`.
