# Jemalloc Tuning, Custom Allocators, and Heap Profiling

Under sustained high-concurrency async workloads in Linux containers, the default GNU C library (`glibc`) allocator suffers from arena fragmentation, refusing to release unmapped memory back to the kernel. Switching to `jemalloc` or `mimalloc` and enabling active heap profiling prevents memory bloat and pinpoints allocation leaks.

---

## 1. When to Switch Allocators

| Feature | GNU `glibc` | `tikv-jemallocator` | `mimalloc` |
| :--- | :--- | :--- | :--- |
| **Arena Management** | Dynamic per-thread, fragmentation-prone | Strict bins, decay-based purging | Page-based segment allocator |
| **Long-Running RSS Stability** | Poor without `malloc_trim` | Exceptional under heavy churn | Exceptional |
| **Multi-Thread Scalability** | Moderate | High (thread-cache arena) | Extremely High |
| **Runtime Heap Profiling** | Complex (`valgrind`) | Built-in via `jemalloc_ctl` | Basic |
| **Best For** | Minimal dependencies | Production servers, web backends | CPU-bound batch tasks |

---

## 2. Drop-in `tikv-jemallocator` Configuration

Add `tikv-jemallocator` to `Cargo.toml`:
```toml
[dependencies]
tikv-jemallocator = { version = "0.6", features = ["profiling", "unprefixed_malloc_on_supported_platforms"] }
```

In your `main.rs`:
```rust
#[cfg(not(target_env = "msvc"))]
use tikv_jemallocator::Jemalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;
```

---

## 3. Production Runtime Environment Tuning

Configure jemalloc via the `MALLOC_CONF` environment variable in your `Dockerfile` or container runtime:

```dockerfile
# Active background purging and aggressive page release
ENV MALLOC_CONF="background_thread:true,dirty_decay_ms:1000,muzzy_decay_ms:1000"
```

- `background_thread:true`: Enables dedicated jemalloc background worker threads to sweep and purge unused arenas without stalling application tasks.
- `dirty_decay_ms:1000`: Decays dirty (unused) pages and frees them back to the OS kernel within 1 second.
- `muzzy_decay_ms:1000`: Decays muzzy pages (purged via `MADV_FREE`) within 1 second.

---

## 4. On-Demand Live Heap Dump via `jemalloc-ctl`

You can trigger a live heap profile dump in production via an HTTP admin endpoint without restarting the process:

```rust
use axum::{response::IntoResponse, http::StatusCode};

#[cfg(not(target_env = "msvc"))]
pub async fn dump_heap_profile() -> impl IntoResponse {
    use tikv_jemalloc_ctl::{epoch, prof};
    use std::ffi::CString;

    // Advance epoch to refresh jemalloc internal stats
    let _ = epoch::advance();

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let file_path = format!("/tmp/heap_profile_{timestamp}.prof");
    let c_path = CString::new(file_path.clone()).unwrap();

    // Trigger heap profile dump
    match prof::dump::write(c_path.as_c_str()) {
        Ok(_) => (StatusCode::OK, format!("Heap profile dumped to {file_path}")),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Dump failed: {e}")),
    }
}
```

Analyze the resulting `.prof` file using `jeprof`:
```bash
jeprof --show_bytes --pdf ./target/release/my-service /tmp/heap_profile_1234567.prof > heap.pdf
```

---

## 5. Allocation Profiling in CI via DHAT

For deterministic regression testing in CI without jemalloc overhead, use `dhat` to catch unintended heap churn:

```toml
[dev-dependencies]
dhat = "0.3"
```

In your integration test:
```rust
#[test]
fn test_zero_allocations_on_hot_path() {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::builder().testing().build();

    let payload = b"GET /api/v1/health HTTP/1.1\r\n\r\n";
    parse_request_zero_copy(payload);

    #[cfg(feature = "dhat-heap")]
    {
        let stats = dhat::HeapStats::get();
        // Assert that zero heap allocations occurred during parsing
        assert_eq!(stats.total_blocks, 0, "Expected 0 allocations on hot path!");
    }
}
```

---

## 6. Alternative: `mimalloc` for Low Overhead

If heap profiling is not required and raw single-thread/multi-thread allocator speed is the goal:

```toml
[dependencies]
mimalloc = { version = "0.1", default-features = false }
```

In `main.rs`:
```rust
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
```

---

## 7. Production Checklist

- [ ] Evaluated target platform: default glibc vs `tikv-jemallocator` vs `mimalloc`.
- [ ] Injected `MALLOC_CONF="background_thread:true,dirty_decay_ms:1000,muzzy_decay_ms:1000"` in Docker container.
- [ ] Added live diagnostic heap dump endpoint protected by administrative authorization.
- [ ] Added DHAT heap assertion tests on latency-critical hot paths.
