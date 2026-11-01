# Bounds Check Elimination (BCE) and Zero-Cost Memory Patterns

Rust guarantees memory safety by verifying slice boundaries at runtime. In latency-sensitive hot loops (such as byte parsing, cryptography, hashing, and numeric calculation), runtime bounds checking introduces hidden conditional branches that prevent the compiler from generating vectorized SIMD instructions.

---

## 1. Bounds Check Elimination (BCE)

### The Anti-Pattern
Indexed iteration forces Rust to generate branch checks on every loop iteration:

```rust
// ANTI-PATTERN: Emits bounds check on every index access
pub fn sum_pairs(a: &[f64], b: &[f64]) -> f64 {
    let mut sum = 0.0;
    for i in 0..a.len() {
        sum += a[i] * b[i]; // Rust generates a panic branch check for b[i]!
    }
    sum
}
```

### The Solution: Length-Proving Iterators
Using `.zip()`, `.chunks_exact()`, or a single up-front slice assertion allows LLVM to mathematically prove that bounds cannot be exceeded, stripping all runtime checks:

```rust
// OPTIMIZED: Zero bounds checks emitted in assembly
pub fn sum_pairs_bce(a: &[f64], b: &[f64]) -> f64 {
    // Proves length to LLVM up front
    assert!(a.len() == b.len(), "Slices must have equal lengths");

    a.iter()
        .zip(b.iter())
        .map(|(&x, &y)| x * y)
        .sum()
}
```

### Chunked Vectorization with `.chunks_exact()`
When processing fixed-size byte groups, `.chunks_exact()` guarantees slice sizes to LLVM at compile time:

```rust
pub fn process_4byte_words(bytes: &[u8]) {
    let chunks = bytes.chunks_exact(4);
    let remainder = chunks.remainder();

    for chunk in chunks {
        // LLVM knows chunk is EXACTLY 4 bytes: 0 runtime bounds checks!
        let word = u32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        process_word(word);
    }

    for &byte in remainder {
        process_single_byte(byte);
    }
}
```

---

## 2. Eliminating the "Zeroing Tax" with `MaybeUninit`

Writing `let mut buffer = vec![0u8; 65536];` forces the CPU to write 65,536 zero bytes to RAM. When the buffer is immediately passed to a socket or disk read (`stream.read(&mut buffer)`), those zeroes are overwritten within microseconds, wasting CPU bandwidth.

### Use Uninitialized Capacity Safely
Use `bytes::BytesMut` or `std::mem::MaybeUninit` to allocate without zeroing memory:

```rust
use bytes::BytesMut;

pub fn allocate_socket_read_buffer(capacity: usize) -> BytesMut {
    // Allocates memory without paying the zero-initialization tax
    BytesMut::with_capacity(capacity)
}
```

---

## 3. Extracting Fields with `std::mem::take` and `std::mem::replace`

When transitioning state in an `&mut self` method, cloning fields to satisfy the borrow checker wastes heap allocations:

```rust
pub struct Session {
    pub buffer: Vec<u8>,
}

impl Session {
    pub fn flush_and_reset(&mut self) -> Vec<u8> {
        // std::mem::take replaces self.buffer with Vec::new() (0 alloc)
        // and returns the original buffer with its allocated capacity!
        std::mem::take(&mut self.buffer)
    }
}
```

---

## 4. Fixed-Capacity Stack Arrays with `ArrayVec`

When a vector has a known maximum size (e.g. up to 16 tags or 8 IP addresses), do not allocate on the heap. Use `ArrayVec` to keep all elements directly on the stack:

```rust
use arrayvec::ArrayVec;

pub fn collect_local_headers() -> ArrayVec<&'static str, 8> {
    let mut headers = ArrayVec::<&'static str, 8>::new();
    headers.push("Content-Type");
    headers.push("Authorization");
    // Memory is 100% stack-allocated: 0 heap pointer indirection
    headers
}
```

---

## 5. Production Checklist

- [ ] Replaced indexed slice access (`slice[i]`) with `.iter()`, `.zip()`, or `.chunks_exact()`.
- [ ] Eliminated zero-fill buffers (`vec![0; N]`) on network/file read targets.
- [ ] Used `std::mem::take` or `std::mem::replace` to avoid cloning during struct state shifts.
- [ ] Substituted small fixed-bound heap vectors with stack-allocated `ArrayVec`.
