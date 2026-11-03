# Concurrency Primitives & Lock Contention Elimination

In multi-threaded asynchronous systems, synchronization primitives are a frequent source of hidden latency, CPU core throttling, and allocator churn. Developers commonly default to `tokio::sync::Mutex` indiscriminately or misuse standard library locks, causing thread pool deadlocks, excessive OS futex syscalls, and CPU cache line thrashing.

This reference guide establishes the rules for selecting and tuning concurrency primitives for sub-microsecond synchronization.

---

## 1. The Concurrency Selection Matrix

```text
                                Is the lock held across an `.await` boundary?
                                              /               \
                                            YES                NO
                                           /                     \
                              Use `tokio::sync::Mutex`    Is the access read-heavy (>95% reads)?
                              (Task-yielding async lock)          /                  \
                                                                YES                   NO
                                                               /                        \
                                                  Is state updated rarely?       Is it a single numeric
                                                    /                 \          counter or flag?
                                                  YES                  NO            /          \
                                                  /                     \          YES           NO
                                           Use `arc_swap`         Use `parking_lot`   Use `Atomic*`  Use `parking_lot`
                                           (Wait-free reads)      `RwLock`            (Lock-free)    `Mutex`
```

---

## 2. In-Memory Synchronization: `parking_lot` vs `std::sync` vs `tokio::sync`

### The Hidden Penalty of `tokio::sync::Mutex`
- `tokio::sync::Mutex` is an **asynchronous lock**. Acquiring it requires heap allocation for task queue nodes and triggers Tokio task state swaps.
- **Rule**: If a critical section does not perform I/O and executes in $< 10\ \mu\text{s}$, **NEVER** use `tokio::sync::Mutex`.

### The `parking_lot` Advantage
`parking_lot` replaces standard library synchronization with spin-wait optimization and inline storage:
- `std::sync::Mutex`: Consumes **40 bytes** on Linux, relies directly on OS futexes.
- `parking_lot::Mutex`: Consumes **1 byte**, avoids syscalls for uncontended locks, and is immune to priority inversion.

```toml
# Cargo.toml
[dependencies]
parking_lot = "0.12"
```

```rust
use parking_lot::{Mutex, RwLock};

pub struct SafeCache {
    // 1-byte overhead, ultra-fast uncontended lock
    metrics: Mutex<CacheMetrics>,
    // Reader-writer lock optimized for concurrent lookups
    entries: RwLock<HashMap<CompactString, CacheEntry>>,
}

impl SafeCache {
    pub fn record_hit(&self) {
        // Fast synchronous lock: drops immediately, never held across .await
        let mut m = self.metrics.lock();
        m.hits += 1;
    }
}
```

> **CRITICAL WARNING**: Never hold any synchronous lock guard (`parking_lot::MutexGuard` or `std::sync::MutexGuard`) across an `.await` point. Doing so locks the underlying OS thread, starving Tokio worker threads.

---

## 3. Wait-Free Read-Heavy State with `arc-swap`

When shared state (such as routing tables, rate-limit policies, or upstream configurations) is read hundreds of thousands of times per second but updated infrequently (e.g., via background polling or webhook), even an `RwLock` degrades performance due to atomic reader-count increment contention.

Use **`arc-swap`** for zero-lock, wait-free reads:

```toml
# Cargo.toml
[dependencies]
arc-swap = "1.7"
```

```rust
use arc_swap::ArcSwap;
use std::sync::Arc;

pub struct DynamicRouteConfig {
    pub upstream_url: CompactString,
    pub timeout_ms: u64,
}

pub struct GatewayState {
    // Atomic pointer swap: readers never lock, writers atomically replace pointer
    config: ArcSwap<DynamicRouteConfig>,
}

impl GatewayState {
    pub fn get_upstream(&self) -> Arc<DynamicRouteConfig> {
        // Wait-free read: loads Arc pointer in sub-nanosecond time
        self.config.load_full()
    }

    pub fn update_config(&self, new_config: DynamicRouteConfig) {
        // Atomic store: replaces pointer, old readers safely drain via Arc
        self.config.store(Arc::new(new_config));
    }
}
```

---

## 4. Lock-Free Counters & Atomic Memory Ordering

For concurrent metrics, IDs, and liveness flags, eliminate locks entirely using hardware atomics. Avoid the performance penalty of `Ordering::SeqCst` unless strict total ordering is mathematically required.

```rust
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub struct TelemetryCounters {
    total_requests: AtomicU64,
    active_connections: AtomicU64,
    is_shutting_down: AtomicBool,
}

impl TelemetryCounters {
    pub fn inc_request(&self) {
        // Relaxed ordering: No cross-thread memory synchronization required.
        // Increments local CPU cache without emitting expensive memory bus lock instructions.
        self.total_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn mark_shutdown(&self) {
        // Release ordering: Ensures all prior writes are visible to other threads
        self.is_shutting_down.store(true, Ordering::Release);
    }

    pub fn is_terminating(&self) -> bool {
        // Acquire ordering: Synchronizes with the Release store
        self.is_shutting_down.load(Ordering::Acquire)
    }
}
```

### Memory Ordering Quick Reference:
| Ordering | Hardware Cost | Typical Use Case |
|---|---|---|
| **`Relaxed`** | Lowest (Single instruction) | Monotonic counters, Prometheus metrics, stats. |
| **`Acquire` / `Release`** | Low (Cache synchronization barrier) | Thread handoffs, termination flags, lock-free queues. |
| **`SeqCst`** | Highest (Full memory fence across all CPU cores) | Strict multi-variable consensus (rarely needed). |

---

## 5. Concurrency Audit Checklist

1. Is `tokio::sync::Mutex` used ONLY when lock guards must be held across `.await` points?
2. Are in-memory, synchronous locks using `parking_lot` instead of `std::sync`?
3. Are read-heavy configurations ($>95\%$ reads) decoupled using `ArcSwap`?
4. Are concurrent metrics using `Atomic*` with `Ordering::Relaxed`?
5. Has Clippy verified that no synchronous mutex guard crosses an `.await` boundary?
