# Zero-Cost Iterators & Pipeline Inlining

In high-throughput Rust services, data transformation loops (filtering HTTP headers, parsing delimiter streams, normalizing metrics) frequently fall victim to **intermediate allocation bloat**.

Developers migrating from languages like Python, JavaScript, or Java commonly chain `.collect::<Vec<_>>()` at intermediate stages of a pipeline. Each `.collect()` forces a heap allocation, dynamic vector resizing, and redundant cache-line churn before the next transformation even begins.

This reference guide documents how to construct purely lazy, zero-allocation iterator pipelines that LLVM compiles down to single-pass vectorized machine loops.

---

## 1. The Intermediate `.collect()` Anti-Pattern

### Naive Implementation (Multiple Allocations & Cache Misses)
```rust
// ANTI-PATTERN: Allocates 2 heap vectors for a simple pipeline!
pub fn process_metrics_naive(raw: &[i64]) -> Vec<i64> {
    // Allocation #1: intermediate filtered vector
    let positive: Vec<i64> = raw.iter().copied().filter(|&x| x > 0).collect();

    // Allocation #2: final mapped vector
    let doubled: Vec<i64> = positive.into_iter().map(|x| x * 2).collect();

    doubled
}
```

### Zero-Allocation Lazy Inlining (Zero Heap Overhead)
```rust
// ZERO-COST: Evaluated in a single pass directly into the final container
pub fn process_metrics_optimal(raw: &[i64]) -> Vec<i64> {
    raw.iter()
        .copied()
        .filter(|&x| x > 0)
        .map(|x| x * 2)
        .collect() // Exactly ONE allocation with pre-sized capacity when possible
}
```

In the optimal version, Rust's iterator combinators (`filter`, `map`, `take`) are zero-sized types (ZSTs) or small stack wrappers. LLVM fuses them together and inlines the entire pipeline into a single assembly loop, eliminating all intermediate heap allocations.

---

## 2. In-Place Allocation Recycling with `.drain()`

When processing queues or batches inside worker loops, dropping and recreating a `Vec` causes continuous allocator churn. Use `.drain(..)` to consume elements while **preserving the underlying allocated capacity**:

```rust
pub struct BatchWorker {
    buffer: Vec<String>,
}

impl BatchWorker {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(capacity),
        }
    }

    /// Processes items using an iterator and leaves `self.buffer` empty
    /// with zero deallocations.
    pub fn flush_and_process<F>(&mut self, mut processor: F)
    where
        F: FnMut(&str),
    {
        // .drain(..) produces an iterator that moves elements out of the buffer
        // and restores len to 0 while keeping capacity intact!
        for item in self.buffer.drain(..) {
            processor(&item);
        }

        // Buffer capacity remains completely intact for the next request cycle!
        debug_assert_eq!(self.buffer.len(), 0);
        debug_assert!(self.buffer.capacity() > 0);
    }
}
```

---

## 3. Monomorphized `impl Iterator` vs `Box<dyn Iterator>`

When returning an iterator from a function or struct method, avoid dynamic trait objects (`Box<dyn Iterator>`), which introduce heap allocation and vtable indirect call penalties:

```rust
// BAD: Triggers heap allocation for the Box + dynamic vtable dispatch on every .next()
pub fn active_tokens_dyn<'a>(tokens: &'a [str]) -> Box<dyn Iterator<Item = &'a str> + 'a> {
    Box::new(tokens.iter().copied().filter(|s| !s.is_empty()))
}

// GOOD: Zero heap allocation, fully inlined by LLVM at the call site
pub fn active_tokens_impl<'a>(tokens: &'a [str]) -> impl Iterator<Item = &'a str> + 'a {
    tokens.iter().copied().filter(|s| !s.is_empty())
}
```

| Trait Form | Heap Allocations | Function Call Overhead | LLVM Inlining |
|---|---|---|---|
| `Box<dyn Iterator>` | 1 heap allocation (`Box`) | Indirect pointer jump | Prevented (virtual call) |
| `impl Iterator` | **0 heap allocations** | **0 overhead** | **Fully inlined & vectorized** |

---

## 4. Batch Ingestion with `Extend` and Pre-Sizing

When appending multiple elements to an existing collection, avoid repeated `.push()` calls in a loop. Use `.extend()`, which leverages `Iterator::size_hint()` to pre-allocate capacity in a single resize:

```rust
pub fn ingest_records(target: &mut Vec<u32>, source: impl IntoIterator<Item = u32>) {
    // Leverages TrustedLen specialization in stdlib for single-reallocation growth
    target.extend(source);
}
```

---

## 5. Architectural Checklist for Iterators

1. Never chain `.collect()` in the middle of a processing pipeline. Keep iterators lazy until the terminal consumer.
2. In hot worker loops, use `buffer.drain(..)` or `buffer.clear()` to reuse pre-allocated capacity.
3. Return `impl Iterator<Item = T>` instead of `Box<dyn Iterator<Item = T>>`.
4. Prefer iterators over manual indexing (`for i in 0..slice.len()`) to automatically eliminate bounds checks (`BCE`).
