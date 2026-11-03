# Compiler Niche Optimizations and Static Size Assertions

Memory bloat often occurs invisibly inside struct definitions and enum layouts. Adding a single field or choosing primitive types naively can double a struct's memory footprint due to alignment padding, wasting tens or hundreds of megabytes across large collections.

---

## 1. Niche Optimization via `NonZero*`

In standard Rust, wrapping a primitive type inside `Option<T>` requires an enum discriminant tag:

```rust
use std::mem::size_of;

// Standard u64: 8 bytes
// Option<u64>: 16 bytes! (1 byte tag + 7 bytes alignment padding)
assert_eq!(size_of::<u64>(), 8);
assert_eq!(size_of::<Option<u64>>(), 16); // 100% space bloat!
```

If an array stores 10,000,000 instances of `Option<u64>`, **80 megabytes of memory is wasted on empty padding**.

### The Solution: Unlock the Niche
When zero is an invalid or unrepresented state (e.g. database IDs, non-zero counts, socket file descriptors), use `std::num::NonZeroU64`. The Rust compiler uses the zero bit-pattern (`0x00...`) to represent `None`, requiring **zero extra bytes**:

```rust
use std::num::NonZeroU64;
use std::mem::size_of;

// Option<NonZeroU64> is EXACTLY 8 bytes (0 byte tag overhead)
assert_eq!(size_of::<Option<NonZeroU64>>(), 8);

#[derive(Clone, Copy)]
pub struct AccountRecord {
    pub account_id: NonZeroU64,
    pub secondary_id: Option<NonZeroU64>, // 8 bytes instead of 16
    pub balance: u64,
}

// Total struct size: 24 bytes (instead of 32 or 40 bytes)
const _: () = assert!(size_of::<AccountRecord>() == 24);
```

---

## 2. Compile-Time Size Assertions (`const _: () = assert!(...)`)

When teams evolve a codebase, someone may unintentionally add a field (e.g. a `String` or `Vec`) to a core event or packet struct. Static size assertions cause the build to fail immediately in CI if a struct exceeds its memory budget:

```rust
pub struct TelemetryPacket {
    pub timestamp: u64,
    pub metric_id: u32,
    pub value: f32,
    pub flags: u8,
}

// Compile error triggers if any field addition or reordering changes the size!
const _: () = assert!(
    std::mem::size_of::<TelemetryPacket>() == 24,
    "TelemetryPacket size changed! Verify memory impact before updating assertion."
);
```

---

## 3. Allocation Buffer Reuse via `.clone_from()`

Calling `dest = src.clone()` inside loops drops `dest`'s allocated heap memory and requests a brand new allocation from the OS allocator:

```rust
// ANTI-PATTERN: Allocator thrashing inside loops
let mut buffer = String::with_capacity(1024);
for record in records {
    buffer = record.data.clone(); // Drops old buffer, allocates new memory!
}

// BEST PRACTICE: Reuses existing capacity with 0 heap allocations
let mut buffer = String::with_capacity(1024);
for record in records {
    buffer.clone_from(&record.data); // Copies bytes directly into existing capacity
}
```

---

## 4. `Box<[T]>` (16 Bytes) vs `Vec<T>` (24 Bytes)

`Vec<T>` maintains three words on the stack: `ptr` (pointer to heap), `cap` (capacity), and `len` (current length) = 24 bytes on 64-bit platforms.

When a collection is immutable and will not grow further (e.g., config tables, parsed query tokens, static payloads), convert it to a boxed slice via `.into_boxed_slice()`:

```rust
use std::mem::size_of;

assert_eq!(size_of::<Vec<u8>>(), 24);
assert_eq!(size_of::<Box<[u8]>>(), 16); // Saves 8 bytes per slice!

pub struct ImmutableConfig {
    pub name: Box<str>,         // 16 bytes (vs 24-byte String)
    pub allowed_ips: Box<[u32]>, // 16 bytes (vs 24-byte Vec<u32>)
}
```

---

## 5. `#[cold]` CPU Instruction Cache Isolation

Errors and emergency fallbacks execute infrequently. Marking fallback functions with `#[cold]` instructs LLVM to relocate their compiled machine code out of the CPU's primary L1 instruction cache lines, keeping the CPU cache clear for the hot path:

```rust
#[cold]
#[inline(never)]
fn handle_unlikely_reconnection_error(err: std::io::Error) {
    tracing::error!("Rare reconnection error: {err}");
}
```

---

## 6. Production Checklist

- [ ] Replaced `Option<u32/u64>` with `Option<NonZeroU32/NonZeroU64>` across core record types.
- [ ] Added `const _: () = assert!(size_of::<T>() == N)` to all structs stored in millions of instances.
- [ ] Switched loop assignments to `.clone_from()` to prevent allocator thrashing.
- [ ] Converted immutable vectors and strings to `Box<[T]>` and `Box<str>`.
- [ ] Tagged rare error recovery routines with `#[cold]`.
