# Numeric Safety & Saturating Arithmetic

In production services, integer overflow, uncontrolled bit truncation via `as` casts, and IEEE 754 floating-point comparisons represent critical runtime hazards.

In debug builds, unhandled integer overflow triggers a process panic. In release builds, integer overflow wraps silently via two's complement. In high-frequency metrics pipelines, financial engines, and token-bucket rate limiters, unhandled numeric edge cases lead to negative balances, infinite rate-limit allowances, or unexpected process aborts.

This reference guide documents zero-cost numeric disciplines: explicit overflow handling (`saturating_*`, `checked_*`, `wrapping_*`), safe narrowing via `TryFrom`, and deterministic floating-point ordering.

---

## 1. The Four Arithmetic Intents

Instead of relying on ambiguous operators (`+`, `-`, `*`), production services must declare explicit mathematical intent:

| Method | Behavior on Overflow | Best Production Use Case |
|---|---|---|
| **`saturating_*`** | Clamps at `MAX` or `MIN` (never overflows) | **Rate limiters, metrics counters, gauge tracking** |
| **`checked_*`** | Returns `None` on overflow | **Financial ledgers, memory buffer pre-sizing** |
| **`wrapping_*`** | Explicitly wraps modulo $2^N$ | **Hash algorithms, ring buffers, sequence numbers** |
| **`overflowing_*`** | Returns `(value, has_overflowed)` | **Low-level crypto primitives, multi-word math** |

### Code Example: Production Token Bucket
```rust
pub struct TokenBucket {
    pub available_tokens: u64,
    pub max_capacity: u64,
}

impl TokenBucket {
    pub fn refill(&mut self, added: u64) {
        // ZERO-PANIC: Automatically caps at max_capacity without overflow risk
        self.available_tokens = self
            .available_tokens
            .saturating_add(added)
            .min(self.max_capacity);
    }

    pub fn try_consume(&mut self, cost: u64) -> bool {
        // ZERO-PANIC: Returns None if cost > available_tokens
        if let Some(remaining) = self.available_tokens.checked_sub(cost) {
            self.available_tokens = remaining;
            true
        } else {
            false
        }
    }
}
```

### Hardware Efficiency of Saturating Arithmetic
Modern x86_64 and ARM64 CPUs implement hardware saturation instructions. Rust's `saturating_add` and `saturating_sub` compile down to **1–2 machine instructions** (e.g. `add` + conditional move on x86, or native saturating instructions on ARM), with **zero conditional panic branches** in the generated assembly.

---

## 2. Eliminating the Silent Truncation Trap of `as`

Using `as` for narrowing numeric conversions silently drops the upper bits, creating severe data corruption bugs:

```rust
// HAZARD: 65,537 exceeds u16::MAX (65,535)
let large_port: u32 = 65_537;
let port: u16 = large_port as u16; // SILENT TRUNCATION! port is now 1!
assert_eq!(port, 1);
```

### The Solution: Safe Narrowing with `TryFrom`
Use `TryFrom` / `TryInto` to detect narrowing failures at runtime without panicking:

```rust
use std::convert::TryFrom;

pub fn parse_port(val: u32) -> Result<u16, &'static str> {
    u16::try_from(val).map_err(|_| "Port number exceeds 65535 boundary")
}

assert!(parse_port(65_537).is_err());
assert_eq!(parse_port(8080).unwrap(), 8080);
```

For widening conversions (e.g. `u16` to `u32`), always use `u32::from(val)`, which is guaranteed to be lossless and verified at compile time.

---

## 3. Floating-Point Comparisons & Ordering

Because IEEE 754 floating-point numbers include `NaN` (Not a Number) and negative zero (`-0.0`), floats do not implement `Eq` or `Ord`:

```rust
let x = f64::NAN;
assert!(!(x == x)); // NaN is not equal to itself!
```

### Safe Equality Check with Epsilon
Never compare floating-point numbers with `==`. Compare within an acceptable tolerance:

```rust
pub fn approx_equal(a: f64, b: f64, epsilon: f64) -> bool {
    (a - b).abs() < epsilon
}

assert!(approx_equal(0.1 + 0.2, 0.3, 1e-10));
```

### Safe Sorting with `f64::total_cmp`
Calling standard `.sort()` on `Vec<f64>` fails to compile because floats do not implement `Ord`. Using naive `.partial_cmp().unwrap()` panics in production if a slice contains `NaN`.

Use `f64::total_cmp`, which enforces a total ordering across all IEEE 754 values (including `NaN` and `-0.0`):

```rust
pub fn sort_latencies(latencies: &mut [f64]) {
    // ZERO-PANIC: total_cmp handles NaN and infinities predictably
    latencies.sort_by(|a, b| a.total_cmp(b));
}
```

---

## 4. Architectural Checklist for Numeric Safety

1. Enforce `#![warn(clippy::cast_possible_truncation, clippy::cast_sign_loss)]` to catch unsafe `as` casts.
2. Use `saturating_*` for token buckets, rate limiters, gauges, and queue counters.
3. Use `checked_*` for monetary balances, slice indexing math, and memory allocations.
4. Always use `TryFrom` for narrowing integer casts.
5. Sort floating-point metrics using `.sort_by(f64::total_cmp)`.
