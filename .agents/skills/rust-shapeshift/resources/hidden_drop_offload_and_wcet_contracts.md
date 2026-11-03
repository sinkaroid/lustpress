# Predictable Tail Latency, WCET Contracts & Offloaded Destructors

## The Problem: Latency Spikes in Garbage-Free Rust

A pervasive myth in backend engineering is that eliminating a tracing Garbage Collector automatically guarantees low, predictable latency. In production Rust services, tail latency spikes (p99/p99.99) and jitter frequently occur due to four subtle runtime hazards:

1. **Hidden `Drop` Deallocation Cascades**: Dropping large or deeply nested heap structures (`Vec<T>`, large DOMs, JSON abstract syntax trees, nested `Arc<T>`) causes the current worker thread to synchronously walk thousands of pointers and execute memory deallocations, halting request handling for milliseconds.
2. **Unbounded Compare-And-Swap (CAS) Loops**: Naive lock-free atomic loops spin indefinitely under high CPU concurrency and cache-line contention, starving CPU pipelines.
3. **Async Task Worker Starvation**: Tight polling loops in Tokio tasks consume full executor time slices without yielding, delaying subsequent scheduled tasks.
4. **Hot-Path String Formatting & Logging Allocations**: Macros like `format!`, `println!`, and dynamic tracing spans perform heap allocations and acquire global OS stdout mutexes in the critical execution path.

---

## 1. Path Classification & Worst-Case Execution Time (WCET) Contracts

Strict real-time and low-jitter guidelines must only be applied to latency-critical paths. Segregate components explicitly into execution zones:

```text
Hot / Latency-Critical Path:
  - Packet Rx/Tx, WebSocket message loop, HTTP stream decode, driver rings.
  - Zero heap allocation, zero blocking locks, zero formatting/logging.
  - Constant or bounded iterations (O(1) or O(bounded_N)).

Deferred / Non-Critical Path:
  - Config parsing, metrics exposition, startup initialization, cleanup drain.
  - Dynamic heap allocation allowed, standard HashMaps permitted.
```

### Documenting Formal WCET Contracts

For every latency-sensitive function, declare an explicit WCET contract in rustdoc:

```rust
/// WCET Contract:
/// - Maximum time complexity: O(MAX_BATCH), where MAX_BATCH <= 128.
/// - Allocations: 0 heap allocations during steady-state processing.
/// - Synchronization: Lock-free atomic load/store only; no Mutex or OS syscalls.
/// - Panics: Panic-free. All slice accesses guarded via checked slices.
/// - Logging: Zero format string evaluations or I/O calls.
pub fn process_event_batch<const MAX_BATCH: usize>(
    events: &[RawEvent; MAX_BATCH],
    count: usize,
) -> Result<usize, ProcessingError> {
    let active_slice = events.get(..count).ok_or(ProcessingError::OutOfBounds)?;
    let mut processed = 0;

    for event in active_slice {
        // Deterministic, bounded inner operations
        processed += 1;
    }

    Ok(processed)
}
```

---

## 2. Offloading Destructors: The Background Cleaner Pattern

When a worker thread finishes parsing or manipulating a large data structure (e.g., thousands of scraped records or large gRPC message buffers), dropping it in-place triggers immediate heap deallocation overhead.

Transfer ownership to a dedicated background cleaner channel:

```rust
use tokio::sync::mpsc;
use std::thread;

pub struct AsyncCleaner<T: Send + 'static> {
    tx: mpsc::Sender<T>,
}

impl<T: Send + 'static> AsyncCleaner<T> {
    pub fn new(capacity: usize) -> Self {
        let (tx, mut rx) = mpsc::channel::<T>(capacity);

        // Spawn a dedicated, low-priority background OS thread for deallocations
        thread::Builder::new()
            .name("heap-cleaner".to_string())
            .spawn(move || {
                while let Some(garbage) = rx.blocking_recv() {
                    drop(garbage); // Explicit drop occurs off the worker thread
                }
            })
            .expect("failed to spawn cleaner thread");

        Self { tx }
    }

    /// Non-blocking handoff. If cleaner channel is saturated, falls back to in-place drop.
    pub fn offload(&self, item: T) {
        if let Err(mpsc::error::TrySendError::Full(overflow)) = self.tx.try_send(item) {
            // Saturated cleaner queue: drop in place as backpressure valve
            drop(overflow);
        }
    }
}
```

### Usage in High-Concurrency Request Handlers

```rust
pub async fn handle_batch_request(
    cleaner: &AsyncCleaner<Vec<LargeRecord>>,
    payload: Vec<u8>,
) -> Result<(), AppError> {
    // Heavy transformation produces heap-allocated structure
    let records: Vec<LargeRecord> = parse_and_process(&payload)?;

    // Hot path completes here. Hand off deallocation to the background worker:
    cleaner.offload(records);

    Ok(())
}
```

---

## 3. Bounded CAS Retry Loops vs. Spin Contention

Unbounded atomic loops create pathological latency spikes under high multicore contention due to cache-coherency ping-ponging.

### Anti-Pattern: Unbounded Compare-And-Swap Loop

```rust
use std::sync::atomic::{AtomicU64, Ordering};

// BAD: Can spin millions of times under lock contention
pub fn increment_counter_unbounded(counter: &AtomicU64) {
    let mut current = counter.load(Ordering::Relaxed);
    loop {
        match counter.compare_exchange_weak(
            current,
            current + 1,
            Ordering::AcqRel,
            Ordering::Relaxed,
        ) {
            Ok(_) => break,
            Err(actual) => current = actual,
        }
    }
}
```

### Production Pattern: Bounded Retry with Hardware Pause

```rust
use std::sync::atomic::{AtomicU64, Ordering};
use std::hint::spin_loop;

pub const MAX_CAS_RETRIES: usize = 32;

#[derive(Debug, thiserror::Error)]
pub enum AtomicContentionError {
    #[error("Exceeded maximum CAS retries due to thread contention")]
    Saturated,
}

pub fn increment_counter_bounded(counter: &AtomicU64) -> Result<(), AtomicContentionError> {
    let mut current = counter.load(Ordering::Relaxed);

    for _ in 0..MAX_CAS_RETRIES {
        match counter.compare_exchange_weak(
            current,
            current.saturating_add(1),
            Ordering::AcqRel,
            Ordering::Relaxed,
        ) {
            Ok(_) => return Ok(()),
            Err(actual) => {
                current = actual;
                spin_loop(); // Emits PAUSE instruction on x86_64 to reduce power & pipeline contention
            }
        }
    }

    Err(AtomicContentionError::Saturated)
}
```

---

## 4. Async Cooperative Poll Bounds (Preventing Task Starvation)

Tokio uses cooperative scheduling, but tasks draining queues via `while let Some(...)` loops can consume excessive runtime time slices without yielding.

### Anti-Pattern: Monopolizing Worker Thread

```rust
// BAD: Runs indefinitely if queue is populated faster than drained
async fn process_inbound_stream(mut rx: tokio::sync::mpsc::Receiver<Frame>) {
    while let Some(frame) = rx.recv().await {
        handle_frame(frame);
    }
}
```

### Production Pattern: Bounded Iteration with Explicit Yield

```rust
use tokio::task::yield_now;

pub const MAX_DRAIN_PER_POLL: usize = 64;

pub async fn process_inbound_stream_bounded(mut rx: tokio::sync::mpsc::Receiver<Frame>) {
    'outer: loop {
        for _ in 0..MAX_DRAIN_PER_POLL {
            match rx.try_recv() {
                Ok(frame) => handle_frame(frame),
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                    // Await next item asynchronously
                    match rx.recv().await {
                        Some(frame) => handle_frame(frame),
                        None => break 'outer,
                    }
                    break;
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break 'outer,
            }
        }

        // Voluntarily yield current task back to executor run-queue
        yield_now().await;
    }
}
```

---

## 5. Hot-Path Ring Buffer Logging (Deferred Formatting)

Dynamic log formatting (`format!("{}: {}", id, msg)`) allocates temporary strings and takes file descriptor write locks. Replace hot-path logging with lock-free, fixed-capacity event enums.

```rust
#[derive(Clone, Copy, Debug)]
pub enum FastEvent {
    RequestReceived { id: u64, timestamp_ns: u64 },
    CacheHit { id: u64 },
    RateLimited { client_ip: [u8; 4] },
}

/// Fixed-capacity, lock-free SPSC ring buffer for telemetry
pub struct EventRecorder<const CAP: usize> {
    queue: rtrb::Producer<FastEvent>,
}

impl<const CAP: usize> EventRecorder<CAP> {
    #[inline(always)]
    pub fn record(&mut self, event: FastEvent) {
        // Non-blocking, zero-allocation write. Drops oldest if full.
        let _ = self.queue.push(event);
    }
}
```

Formatting and serialization to JSON or text logs are performed solely by an asynchronous consumer reading from the consumer end (`rtrb::Consumer`), keeping the hot path entirely allocation-free.
