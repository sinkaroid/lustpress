# Production Memory Leak Diagnostics and eBPF Profiling

A common misconception among developers transitioning to Rust is that the borrow checker makes memory leaks impossible. In reality, the Rust compiler guarantees **memory safety** (preventing data races, double-frees, and use-after-free), but **not the absence of memory leaks**.

Services running in long-term production can leak memory through:
1. **Reference Cycles**: `Rc<RefCell<T>>` or `Arc<Mutex<T>>` cycles that never drop their refcount to zero.
2. **Unbounded Collections**: HashMaps, DashMaps, or unbounded queues that continuously insert keys without eviction policies.
3. **Leaked Allocations**: Deliberate or accidental `Box::leak`, `std::mem::forget`, or unreleased arena pages.
4. **FFI Allocation Boundaries**: C libraries (`libssl`, `librdkafka`, `sqlite3`) failing to free buffers across FFI boundaries.
5. **Glibc Arena Fragmentation**: Unreturned memory chunks kept in glibc thread arenas.

---

## 1. Diagnostics Tooling Matrix

| Tool | When to Use | Performance Overhead | Production Safe? |
|---|---|---|---|
| **AddressSanitizer / LSan** | CI / Local testing | ~2x CPU, ~2x RAM | No (requires custom compiler flag) |
| **Valgrind Massif** | Local reproduction of heap peaks | 10x–20x slowdown | No (local only) |
| **`tikv-jemalloc-ctl`** | Live heap snapshot dumps on demand | Negligible (<1%) | **Yes (Safe for production)** |
| **eBPF (`bcc memleak`)** | Kernel-level leak tracing on running PIDs | Minimal (~1%–2%) | **Yes (Zero binary changes needed)** |

---

## 2. CI & Local Leak Detection: LLVM LeakSanitizer (LSan)

Compile the binary with LeakSanitizer enabled using nightly Rust:

```bash
# Enable address and leak sanitizers
RUSTFLAGS="-Zsanitizer=leak" cargo +nightly test --target x86_64-unknown-linux-gnu
```

If memory is allocated and orphaned without being reclaimed before process exit, LSan prints the exact backtrace:

```text
=================================================================
==12345==ERROR: LeakSanitizer: detected memory leaks
Direct leak of 1048576 byte(s) in 1 object(s) allocated from:
    #0 0x55d7f1 in malloc
    #1 0x55df2a in alloc::alloc::alloc
    #2 0x55e100 in my_service::cache::leak_buffer
```

---

## 3. Production Heap Profiling with `jemalloc-ctl`

When running in Kubernetes or bare-metal Linux with `tikv-jemallocator`, configure jemalloc profiling at compile and runtime to dump live heap snapshots without restarting the service.

Add `tikv-jemalloc-ctl` to `Cargo.toml`:
```toml
[dependencies]
tikv-jemalloc-ctl = "0.6"
```

Configure `_MALLOC_CONF` in the Dockerfile:
```dockerfile
ENV MALLOC_CONF="prof:true,prof_active:true,prof_prefix:jeprof.out"
```

### In-Process Trigger for Memory Heap Dumps

Expose an internal administrative endpoint or listen for `SIGUSR1` to dump an allocation profile:

```rust
use tikv_jemalloc_ctl::{epoch, stats};
use std::ffi::CString;

pub fn trigger_heap_dump(filename: &str) -> Result<(), String> {
    let c_str = CString::new(filename).map_err(|e| e.to_string())?;

    unsafe {
        // Advance jemalloc internal epoch to refresh metrics
        epoch::advance().map_err(|e| format!("Epoch advance failed: {e:?}"))?;

        // Write heap profile to disk
        tikv_jemalloc_ctl::raw::write(
            b"prof.dump\0",
            c_str.as_ptr() as *const std::ffi::c_void,
        ).map_err(|e| format!("Heap dump failed: {e:?}"))?;
    }

    Ok(())
}

pub fn read_process_allocated_bytes() -> usize {
    let _ = epoch::advance();
    stats::allocated::read().unwrap_or(0)
}
```

Inspect the generated profile locally using `jeprof`:
```bash
jeprof --show_bytes --pdf ./target/release/my_service jeprof.out.1234.0.mheap > leak_report.pdf
```

---

## 4. Zero-Overhead Live Production Tracing with eBPF (`memleak`)

In production environments where binaries cannot be recompiled or restarted, attach the BCC / bpftrace `memleak` tool directly to the container's PID via the host Linux kernel:

```bash
# Trace outstanding allocations for PID 4512 larger than 1MB over 30 seconds
sudo /usr/share/bcc/tools/memleak -p 4512 --older 30000 --top 10
```

### Output: Stack Traces with Unfreed Allocations

```text
Attaching to malloc/free syscalls for PID 4512...
[09:42:15] Top 2 stacks with outstanding allocations:
8388608 bytes in 1 allocations from stack
    my_service::state::unbounded_cache_insert+0x42 [my_service]
    my_service::handlers::process_request+0x18a [my_service]
    tokio::runtime::task::core::run+0x85 [my_service]
```

This immediately pinpoints the exact function responsible for unbounded memory retention without touching application code.
