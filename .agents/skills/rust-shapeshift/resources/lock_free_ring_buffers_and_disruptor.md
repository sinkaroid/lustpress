# Lock-Free Circular Ring Buffers and Disruptor Pattern

In concurrent pipelines transferring millions of events, telemetry metrics, or network packets per second, standard synchronization channels (`tokio::sync::mpsc`, `crossbeam_channel`, or `std::sync::mpsc`) introduce noticeable performance bottlenecks:
1. **Dynamic Node Allocations**: Many channel implementations allocate heap memory for queue segments or nodes during sustained bursts.
2. **Lock & Atomic Bus Contention**: Producers and consumers competing on shared atomic cursors cause cache-line bouncing across multi-core CPU architectures.
3. **Latency Jitter**: Memory allocators and thread wakeups create unpredictable p99/p99.99 latency spikes.

The **Disruptor Pattern** and **Fixed-Capacity Lock-Free Ring Buffers** eliminate dynamic allocations entirely by pre-allocating a contiguous circular buffer at startup and using hardware cache-aligned atomic sequence barriers.

---

## 1. Single-Producer Single-Consumer (SPSC) Ring Buffer with `rtrb`

For ultra-low-latency pipelines where a single dedicated reader thread receives messages from a single worker, an SPSC lock-free ring buffer provides sub-nanosecond operations with zero locks.

Add `rtrb` to `Cargo.toml`:
```toml
[dependencies]
rtrb = "0.3"
```

### High-Throughput SPSC Pipeline

```rust
use rtrb::{RingBuffer, Producer, Consumer};
use std::thread;

pub struct TelemetryEvent {
    pub timestamp_ns: u64,
    pub event_id: u32,
    pub metric_value: f64,
}

pub fn create_spsc_telemetry_pipeline(capacity: usize) -> (Producer<TelemetryEvent>, Consumer<TelemetryEvent>) {
    // Allocates fixed memory chunk once at startup. Zero allocations during runtime.
    RingBuffer::new(capacity)
}

pub fn run_pipeline_demo() {
    let (mut producer, mut consumer) = create_spsc_telemetry_pipeline(65_536);

    // Producer Thread
    let producer_handle = thread::spawn(move || {
        for i in 0..1_000_000 {
            let event = TelemetryEvent {
                timestamp_ns: i,
                event_id: (i % 256) as u32,
                metric_value: 42.0,
            };

            // Non-blocking push. If buffer is full, backpressure strategy takes over.
            while let Err(e) = producer.push(event) {
                // Cooperative yield or micro-spin
                std::hint::spin_loop();
            }
        }
    });

    // Consumer Thread
    let consumer_handle = thread::spawn(move || {
        let mut received = 0;
        while received < 1_000_000 {
            if let Ok(event) = consumer.pop() {
                received += 1;
                let _val = event.metric_value;
            } else {
                std::hint::spin_loop();
            }
        }
    });

    producer_handle.join().unwrap();
    consumer_handle.join().unwrap();
}
```

---

## 2. Multi-Producer Multi-Consumer (MPMC) with `crossbeam::queue::ArrayQueue`

When multiple Tokio worker threads or OS threads must push to a shared bounded queue without heap allocations or unbounded memory growth, use `ArrayQueue`.

Add `crossbeam-queue` to `Cargo.toml`:
```toml
[dependencies]
crossbeam-queue = "0.3"
```

```rust
use crossbeam_queue::ArrayQueue;
use std::sync::Arc;

pub struct BoundedEventBroker<T> {
    queue: Arc<ArrayQueue<T>>,
}

impl<T> BoundedEventBroker<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            // Strictly bounded fixed-size heap allocation
            queue: Arc::new(ArrayQueue::new(capacity)),
        }
    }

    /// Pushes event without allocating. Returns Err(event) if at capacity (sheds load).
    #[inline(always)]
    pub fn try_publish(&self, item: T) -> Result<(), T> {
        self.queue.push(item)
    }

    /// Pulls event with zero lock contention
    #[inline(always)]
    pub fn try_consume(&self) -> Option<T> {
        self.queue.pop()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_full(&self) -> bool {
        self.queue.is_full()
    }
}
```

---

## 3. Cache Line Alignment for Atomic Cursors (False Sharing Prevention)

When building custom ring buffers, producers and consumers maintain atomic sequence numbers (head and tail pointers). If the head and tail atomics share the same 64-byte CPU cache line, multi-core writes cause severe CPU cache invalidation loops:

```rust
use std::sync::atomic::{AtomicU64, Ordering};

// CAUTION: False sharing hazard!
// head and tail reside on the same cache line. Every producer write invalidates consumer cache.
pub struct BadCursor {
    pub head: AtomicU64, // Offset 0..8
    pub tail: AtomicU64, // Offset 8..16
}

// CORRECT: Cache line alignment isolates each cursor to its own 64-byte line
#[repr(align(64))]
pub struct CacheAlignedCursor {
    pub cursor: AtomicU64,
}

pub struct TrueDisruptorRing {
    pub head: CacheAlignedCursor, // Occupies dedicated 64B cache line
    pub tail: CacheAlignedCursor, // Occupies dedicated 64B cache line
    pub mask: usize,              // Power-of-two bitmask (capacity - 1)
}

impl TrueDisruptorRing {
    pub fn new(power_of_two_capacity: usize) -> Self {
        assert!(power_of_two_capacity.is_power_of_two(), "Capacity must be power of two");
        Self {
            head: CacheAlignedCursor { cursor: AtomicU64::new(0) },
            tail: CacheAlignedCursor { cursor: AtomicU64::new(0) },
            mask: power_of_two_capacity - 1,
        }
    }

    #[inline(always)]
    pub fn compute_slot_index(&self, sequence: u64) -> usize {
        // Fast bitwise AND replaces expensive modulo operator (sequence % capacity)
        (sequence as usize) & self.mask
    }
}
```

---

## 4. Architectural Rules for Ring Buffer Systems

1. **Power-of-Two Capacity**:
   - Always choose capacities that are powers of two ($2^{16} = 65,536$, $2^{20} = 1,048,576$). This replaces the expensive CPU division/modulo instruction (`% capacity`) with a single-cycle bitwise AND (`& mask`).
2. **Explicit Backpressure Policy**:
   - When the ring is full, make an explicit architectural decision:
     - **Drop Oldest** (for live telemetry and market data feeds).
     - **Fast-Fail / Shed Load** (return HTTP 503 / queue full).
     - **Exponential Backoff** (`std::hint::spin_loop` ➔ `thread::yield_now` ➔ async sleep).
3. **Batch Draining**:
   - Consumers should drain items in batches (e.g. 64–256 items at a time) to amortize atomic synchronization overhead and optimize CPU L1 cache throughput.
