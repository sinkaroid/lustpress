# Hardware Resource Budgets, Concurrency Modeling & Loom Verification

## The Problem: Unbounded Resource Exhaustion & Flaky Concurrency Tests

Async frameworks like Tokio make it easy to spawn millions of tasks, but without upfront physical resource accounting, production services frequently collapse under load due to:

1. **Unbounded Channel & Buffer Growth**: An unexpected slow consumer causes in-memory queues to grow until the Linux OOM-killer terminates the process.
2. **Flaky Concurrency Tests**: Verifying multithreaded logic or custom lock-free atomics with `thread::sleep(Duration::from_millis(10))` provides false confidence; race conditions only trigger under specific OS thread scheduling permutations in production.
3. **Hidden Async Task Poll Latencies**: Tasks that accidentally perform CPU-heavy serialization or uninstrumented blocking I/O stall the Tokio cooperative scheduler.

---

## 1. Upfront Physical Resource Budgeting

Before writing a single line of network code, define the **Worst-Case RSS Ceiling** mathematically:

$$\text{RSS}_{\text{max}} = (\text{Max Connections} \times \text{Buffer}_{\text{per\_conn}}) + \text{Queue Budgets} + \text{Cache Budgets} + \text{Arena Overhead}$$

### Production Resource Specification Table

| Resource Dimension | Strict Limit | Backpressure / Rejection Policy |
| :--- | :--- | :--- |
| **Concurrent TCP Connections** | 10,000 max | `tokio::net::TcpListener` gated via `tokio::sync::Semaphore(10_000)`. Immediate RST if saturated. |
| **Per-Connection Read Buffer** | 64 KB max (`bytes::BytesMut`) | If payload exceeds 64 KB, disconnect with HTTP 413 Payload Too Large. |
| **In-Flight Request Channel** | Bounded: 1,024 items | Use `mpsc::channel(1024)`. If full, return HTTP 503 Load Shedding. |
| **Global LRU Cache** | 256 MB max (`moka::future::Cache`) | Evicts LRU entries automatically; memory-bounded via weighted entries. |
| **Allocator Arena Overhead** | `MALLOC_ARENA_MAX=2` | Limits glibc per-thread arenas to 2, preventing phantom RSS bloat. |

### Calculated Worst-Case Footprint

$$\text{Active Network Memory} = 10{,}000 \times (64\text{ KB rx} + 64\text{ KB tx}) = 1.28\text{ GB}$$
$$\text{Total Safe RSS Ceiling} = 1.28\text{ GB} + 256\text{ MB cache} + 128\text{ MB runtime/stack} \approx 1.66\text{ GB}$$

Deploy Kubernetes limits with at least 25% safety headroom:
```yaml
resources:
  requests:
    memory: "512Mi"
    cpu: "500m"
  limits:
    memory: "2Gi"
    cpu: "2000m"
```

---

## 2. Formal Concurrency Verification with Loom

Never test atomic state machines or custom synchronization primitives with random sleeps. Use **Loom** for deterministic execution-schedule permutation testing.

### Structuring Code for Loom and Standard Runtimes

Abstract synchronization types so that Loom replaces `std::sync` during test runs:

```rust
#[cfg(loom)]
mod sync {
    pub use loom::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    pub use loom::sync::Arc;
    pub use loom::thread;
}

#[cfg(not(loom))]
mod sync {
    pub use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    pub use std::sync::Arc;
    pub use std::thread;
}

use sync::*;
```

### Writing a Loom Exhaustive Permutation Test

Loom repeatedly intercepts thread context switches, atomic operations, and memory barriers to test every possible interleaving:

```rust
pub struct SpinLock {
    locked: AtomicBool,
}

impl SpinLock {
    pub fn new() -> Self {
        Self { locked: AtomicBool::new(false) }
    }

    pub fn try_lock(&self) -> bool {
        // Atomic flag acquisition with Acquire semantics
        !self.locked.swap(true, Ordering::Acquire)
    }

    pub fn unlock(&self) {
        // Release publication
        self.locked.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(loom)]
    fn loom_verify_spinlock_mutual_exclusion() {
        loom::model(|| {
            let lock = Arc::new(SpinLock::new());
            let lock2 = lock.clone();

            let handle = loom::thread::spawn(move || {
                if lock2.try_lock() {
                    // Critical section
                    lock2.unlock();
                }
            });

            if lock.try_lock() {
                // Critical section
                lock.unlock();
            }

            handle.join().unwrap();
        });
    }
}
```

Run the model test with:
```bash
RUSTFLAGS="--cfg loom" cargo test --test loom_model --release
```

If an ordering is too weak (e.g. `Ordering::Relaxed` where `Ordering::Acquire` is required), Loom produces an exact execution trace proving the race condition.

---

## 3. Tokio Task Poll Diagnostics & Starvation Detection

In production async services, tasks that spend more than 1–2 milliseconds in a single `poll()` invocation cause latency jitter for other multiplexed futures.

### Enabling Tokio Runtime Tracing & Metrics

Configure the Tokio runtime to track task execution metrics in diagnostic builds:

```rust
#[cfg(feature = "tokio-console")]
pub fn init_runtime_diagnostics() {
    console_subscriber::init();
}

pub fn create_instrumented_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .metrics_poll_count_histogram() // Track poll duration distributions
        .build()
        .expect("failed to build tokio runtime")
}
```

### Inspecting Runtime Health with `tokio-console`

Run your service with `RUSTFLAGS="--cfg tokio_unstable"` and connect:

```bash
tokio-console http://127.0.0.1:6669
```

Watch for:
1. **Long Poll Times**: Tasks showing poll durations > 1ms. Identify the offending function and offload it to `tokio::task::spawn_blocking` or `rayon`.
2. **High Busy/Idle Ratio**: Indicates that workers are saturated with computation rather than waiting on non-blocking I/O.
3. **Woken Without Progress**: Tasks scheduled frequently by poorly configured timers or channels without actual work ready.
