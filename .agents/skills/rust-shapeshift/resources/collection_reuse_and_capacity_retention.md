# Collection Reuse & Capacity Retention

In high-throughput services processing thousands of requests per second, repeatedly instantiating and dropping collections (`Vec`, `String`, `HashMap`) inside request handlers triggers severe **allocator thrashing and heap fragmentation**.

Even though Rust's ownership model automatically drops memory at the end of each scope, delegating allocation lifecycle management to the OS allocator on every request induces latency spikes (p99/p99.9) and swells resident set size (RSS).

This reference guide documents patterns for reusing pre-allocated collection capacities: `clone_from`, `std::mem::take`, capacity-retaining resets (`.clear()`), and stack-only collections with `ArrayVec`.

---

## 1. Allocation-Free Cloning with `clone_from`

When updating a long-lived buffer from an incoming data source, standard `.clone()` discards the destination's existing memory and triggers a fresh `malloc`:

```rust
pub struct SessionCache {
    buffer: Vec<u8>,
}

impl SessionCache {
    // NAIVE: Frees old buffer, calls malloc() for a new buffer on every update!
    pub fn update_naive(&mut self, incoming: &[u8]) {
        self.buffer = incoming.to_vec(); // Allocator churn!
    }

    // OPTIMAL: Reuses self.buffer's pre-allocated capacity!
    pub fn update_optimal(&mut self, incoming: &[u8]) {
        // clone_from reuses self.buffer's memory if incoming.len() <= capacity.
        // Triggers ZERO malloc() calls if capacity is sufficient!
        self.buffer.clone_from(&incoming.to_vec());
    }

    // EVEN BETTER: Direct slice copy
    pub fn update_zero_alloc(&mut self, incoming: &[u8]) {
        self.buffer.clear();
        self.buffer.extend_from_slice(incoming); // 0 allocations if capacity suffices
    }
}
```

### Benchmark Impact
Under 50,000 req/sec benchmarks:
- `self.buffer = incoming.to_vec()`: **50,000 malloc + 50,000 free operations/sec**.
- `self.buffer.clear(); self.buffer.extend_from_slice(...)`: **0 allocations/sec** after initial warm-up.

---

## 2. Extracting Data Without Cloning via `std::mem::take`

When an async task or worker needs to consume data out of a mutable struct reference, developers frequently clone the data to satisfy the borrow checker:

```rust
pub struct RequestContext {
    pub raw_headers: Vec<String>,
}

// ANTI-PATTERN: Clones every string on the heap!
pub fn harvest_headers_naive(ctx: &mut RequestContext) -> Vec<String> {
    ctx.raw_headers.clone() // Allocates duplicate heap nodes!
}

// ZERO-ALLOCATION: Takes ownership and leaves an empty, zero-sized Vec
pub fn harvest_headers_optimal(ctx: &mut RequestContext) -> Vec<String> {
    // std::mem::take moves out the Vec, leaving Vec::new() (a 0-byte ZST on the heap)
    std::mem::take(&mut ctx.raw_headers)
}
```

`std::mem::take(&mut field)` works on any type that implements `Default`. For `Vec<T>`, `String`, and `Option<T>`, `Default::default()` performs **zero heap allocations**.

---

## 3. Stack-Only Collections with `ArrayVec` (0 Heap Guarantee)

When a collection has a known maximum bound (e.g. HTTP header lists, routing segments, socket chunks $\le 32$ items), heap-backed `Vec<T>` wastes memory on heap pointers and indirection.

Use `ArrayVec<T, N>` (from the `arrayvec` crate) to store elements **entirely on the stack**:

```rust
use arrayvec::ArrayVec;

pub struct RouteMatcher {
    // 0 heap allocations, inline stack array of up to 8 segments
    segments: ArrayVec<&'static str, 8>,
}

impl RouteMatcher {
    pub fn parse(path: &'static str) -> Result<Self, &'static str> {
        let mut segments = ArrayVec::new();
        for part in path.split('/').filter(|s| !s.is_empty()) {
            segments.try_push(part).map_err(|_| "Route depth exceeds 8 segments")?;
        }
        Ok(Self { segments })
    }
}
```

### Memory Footprint Comparison
| Type | Location | Heap Malloc | Indirection (Deref) |
|---|---|---|---|
| `Vec<T>` | Stack metadata + Heap buffer | **Yes** (on growth) | Yes (cache miss risk) |
| `SmallVec<[T; N]>` | Stack (first $N$) $\to$ Heap | Conditional | Conditional |
| `ArrayVec<T, N>` | **100% Stack** | **Never (0 heap)** | **No (direct inline)** |

---

## 4. Resetting Worker Containers with `.clear()`

When maintaining worker-local scratch buffers across loop iterations, never drop the container at the end of the iteration:

```rust
pub async fn run_event_loop() {
    // Pre-allocate scratchpad once outside the hot loop
    let mut scratchpad: Vec<u8> = Vec::with_capacity(8192);

    loop {
        // Read incoming frame into scratchpad
        // ...
        
        // At end of iteration: reset length to 0, RETAIN capacity!
        scratchpad.clear();
        debug_assert_eq!(scratchpad.len(), 0);
        debug_assert_eq!(scratchpad.capacity(), 8192); // 8KB remains allocated!
    }
}
```

---

## 5. Architectural Checklist for Collections

1. Use `dst.clone_from(&src)` or `.clear() + .extend()` to reuse pre-allocated capacity.
2. Use `std::mem::take(&mut value)` instead of `.clone()` to extract fields out of mutable references.
3. For collections bounded by small $N$ ($\le 16$), use `ArrayVec<T, N>` to eliminate heap allocations entirely.
4. Keep worker scratchpads alive across loop iterations and clear them with `.clear()`.
