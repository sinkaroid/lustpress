# SIMD Vectorization and Hardware-Accelerated Byte Operations

When processing high-volume text, network byte streams, binary protocols, or numeric arrays, standard scalar loops (`for b in slice`) underutilize modern CPU architectures. Modern x86_64 and ARM64 CPUs feature 128-bit, 256-bit, and 512-bit vector registers (AVX2, AVX-512, NEON) capable of evaluating 16 to 64 bytes per CPU cycle without allocating heap memory.

---

## 1. Zero-Allocation Fast Delimiter Searching with `memchr`

Standard byte scanning (`slice.iter().position(|&b| b == b'\n')`) checks bytes one by one. The `memchr` crate uses hand-tuned assembly and SIMD vector registers to scan 32 to 64 bytes in a single instruction:

```rust
use memchr::{memchr, memchr2, memmem};

pub fn find_http_header_end(raw_http: &[u8]) -> Option<usize> {
    // Searches for \r\n\r\n using vectorized two-way substring matching
    memmem::find(raw_http, b"\r\n\r\n")
}

pub fn parse_line_endings_fast(buffer: &[u8]) -> Vec<&[u8]> {
    let mut lines = Vec::new();
    let mut start = 0;

    // memchr scans up to 64 bytes per instruction via AVX-512 / AVX2 / NEON
    while let Some(pos) = memchr(b'\n', &buffer[start..]) {
        let end = start + pos;
        lines.push(&buffer[start..end]);
        start = end + 1;
    }

    if start < buffer.len() {
        lines.push(&buffer[start..]);
    }

    lines
}
```

---

## 2. High-Throughput UTF-8 Validation with `simdutf8`

Standard `std::str::from_utf8` processes scalar byte sequences with branching state tables. The `simdutf8` crate accelerates UTF-8 validation up to 20 GB/s on modern CPUs by evaluating byte ranges in parallel:

```rust
use simdutf8::basic::from_utf8;

pub fn validate_request_payload_fast(raw_bytes: &[u8]) -> Result<&str, simdutf8::basic::Utf8Error> {
    // Evaluates 16/32 bytes simultaneously using AVX2 or ARM NEON registers
    from_utf8(raw_bytes)
}
```

---

## 3. Portable SIMD for Numeric & Transformation Pipelines

When computing metrics, parsing delimited numbers, or masking WebSocket frame payloads, use portable SIMD or chunked vector intrinsics:

```rust
// Fast WebSocket frame unmasking (XOR 4-byte mask over arbitrary byte buffer)
pub fn unmask_websocket_payload(payload: &mut [u8], mask: [u8; 4]) {
    // Construct 32-bit repeated pattern
    let mask_u32 = u32::from_ne_bytes(mask);
    let mask_u64 = ((mask_u32 as u64) << 32) | (mask_u32 as u64);

    let (prefix, middle, suffix) = unsafe { payload.align_to_mut::<u64>() };

    // 1. Process unaligned prefix scalar bytes
    let mut mask_idx = 0;
    for byte in prefix.iter_mut() {
        *byte ^= mask[mask_idx % 4];
        mask_idx += 1;
    }

    // 2. Process aligned chunks 8 bytes at a time (64 bits per CPU op)
    for word in middle.iter_mut() {
        *word ^= mask_u64;
    }

    // 3. Process remaining scalar trailing bytes
    mask_idx = (payload.len() - suffix.len()) % 4;
    for byte in suffix.iter_mut() {
        *byte ^= mask[mask_idx % 4];
        mask_idx += 1;
    }
}
```

---

## 4. Compiler Target Flags for Vectorization

Ensure the Rust compiler targets modern instruction sets in release builds by adding CPU target flags to `.cargo/config.toml`:

```toml
[target.'cfg(all(target_arch = "x86_64", target_os = "linux"))']
rustflags = ["-C", "target-cpu=x86-64-v3"] # Enables AVX, AVX2, BMI2, FMA
```
> Setting `target-cpu=x86-64-v3` enables the compiler to automatically vectorize loops into AVX2 instructions across your entire binary without writing manual assembly.

---

## 5. Production Checklist

- [ ] Replaced scalar `.position()` with `memchr` or `memmem` on packet/string slicing.
- [ ] Switched high-throughput UTF-8 parsing to `simdutf8`.
- [ ] Configured `target-cpu=x86-64-v3` (or equivalent microarchitecture) in `.cargo/config.toml` for container deployment.
- [ ] Replaced scalar bit-twiddling on bulk payloads with 64-bit or vector aligned word slicing.
