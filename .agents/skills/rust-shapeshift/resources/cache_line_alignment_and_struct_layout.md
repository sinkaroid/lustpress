# Cache-Line Alignment, Struct Layout & False Sharing Elimination

In high-concurrency systems running on multi-core processors, extreme efficiency depends on how hardware interacts with CPU caches (L1/L2/L3) and memory buses. Poor struct design causes two major performance drains:
1. **Struct Padding Bloat**: Memory wasted on unused alignment bytes across millions of cache/struct instances.
2. **False Sharing (Cache-Line Bouncing)**: Independent CPU cores invalidating each other's L1 cache lines when mutating adjacent variables, collapsing multi-core scalability by up to 80%.

This reference guide details the hardware-level layout optimizations needed to eliminate these bottlenecks.

---

## 1. Struct Field Ordering & Padding Elimination

Rust aligns struct fields according to their natural boundary (e.g., an 8-byte `u64` must reside at a memory address divisible by 8). When fields of varying sizes are declared haphazardly, the compiler must insert invisible **padding bytes**.

### Bad vs Optimal Layout:
```rust
// ❌ UNOPTIMIZED: Consumes 24 bytes due to padding gaps
pub struct UnoptimizedRecord {
    pub active: bool,   // 1 byte  + 7 BYTES PADDING
    pub id: u64,        // 8 bytes
    pub flags: u8,      // 1 byte  + 7 BYTES PADDING
}                       // Total: 24 bytes!

// ✅ OPTIMIZED: Consumes 16 bytes (33% memory reduction)
pub struct OptimizedRecord {
    pub id: u64,        // 8 bytes (offset 0)
    pub active: bool,   // 1 byte  (offset 8)
    pub flags: u8,      // 1 byte  (offset 9)
                        // + 6 bytes trailing padding (offset 10..16)
}                       // Total: 16 bytes!
```

If an in-memory cache stores 1,000,000 items, the unoptimized layout wastes **8 Megabytes of RAM** entirely on dead padding bytes.

### The Struct Ordering Rule
Always declare struct fields in descending order of alignment size:
1. **8-byte types**: `u64`, `i64`, `f64`, pointers, `usize`, `Box<T>`, `Arc<T>`
2. **4-byte types**: `u32`, `i32`, `f32`, `char`
3. **2-byte types**: `u16`, `i16`
4. **1-byte types**: `u8`, `i8`, `bool`
5. **Zero-sized types**: `PhantomData<T>`

---

## 2. Eliminating Multi-Threaded False Sharing

Modern 64-bit processors (x86_64 and ARM64) do not fetch single bytes from RAM; they fetch **64-byte cache lines**.

When two threads running on Core 0 and Core 1 update two independent atomic counters located within the same 64-byte cache line:
```text
[ Core 0: writes counter_a ] ---> [ 64-Byte Cache Line ] <--- [ Core 1: writes counter_b ]
                                         |
                   💥 CACHE INVALIDATION STORM (False Sharing)
         Both cores fight over ownership of the single 64-byte line!
```

Throughput plummets as CPU cores stall waiting for memory bus synchronization.

### The Fix: `#[repr(align(64))]`

Pad hot, concurrently mutated variables so that each variable resides on its own dedicated 64-byte cache line:

```rust
use std::sync::atomic::{AtomicU64, Ordering};

// Force compiler to align struct to a full 64-byte hardware cache line boundary
#[repr(align(64))]
pub struct CachePaddedCounter {
    pub value: AtomicU64,
}

impl CachePaddedCounter {
    pub const fn new(initial: u64) -> Self {
        Self {
            value: AtomicU64::new(initial),
        }
    }
}

// Multi-worker telemetry struct immune to false sharing
pub struct WorkerStats {
    // Each worker counter is isolated on its own independent cache line
    pub worker_0_processed: CachePaddedCounter,
    pub worker_1_processed: CachePaddedCounter,
    pub worker_2_processed: CachePaddedCounter,
    pub worker_3_processed: CachePaddedCounter,
}

impl WorkerStats {
    pub fn record_job(&self, worker_id: usize) {
        match worker_id {
            0 => self.worker_0_processed.value.fetch_add(1, Ordering::Relaxed),
            1 => self.worker_1_processed.value.fetch_add(1, Ordering::Relaxed),
            2 => self.worker_2_processed.value.fetch_add(1, Ordering::Relaxed),
            3 => self.worker_3_processed.value.fetch_add(1, Ordering::Relaxed),
            _ => {}
        };
    }
}
```

---

## 3. Representation Attributes Reference

| Attribute | Purpose | Safe in Production? |
|---|---|---|
| `#[repr(Rust)]` | Default. Compiler may reorder fields arbitrarily to minimize padding. | **YES** |
| `#[repr(C)]` | Enforces exact C-ABI layout and declaration order. Essential for FFI or exact struct layout audits. | **YES** |
| `#[repr(transparent)]` | Guarantees single-field struct has identical memory layout and ABI as its inner type (Newtype pattern). | **YES** |
| `#[repr(align(N))]` | Sets minimum alignment boundary (e.g., `align(64)` for cache lines). | **YES** |
| `#[repr(packed)]` | Removes all padding bytes. | **CAUTION**: Unaligned field access causes CPU hardware faults or severe instruction penalties on ARM. Avoid unless required by hardware protocols. |

---

## 4. Hardware Alignment Checklist

1. Are high-volume cached structs ordered from largest alignment (8B) to smallest (1B)?
2. Has `std::mem::size_of::<T>()` been audited against theoretical minimum field sizes?
3. Are multi-threaded hot atomic counters padded to 64 bytes using `#[repr(align(64))]`?
4. Are `Newtype` wrappers marked with `#[repr(transparent)]`?
