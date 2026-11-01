# Stack Inlining & Heap Allocator Churn Elimination

In high-throughput services and scrapers, indiscriminate use of `String` and `Vec<T>` inside Data Transfer Objects (DTOs), ASTs, and cache models causes thousands of tiny heap allocations and deallocations per second. This triggers CPU L1/L2 cache misses, increases glibc arena fragmentation, and unnecessarily inflates process RSS.

This playbook establishes the **Zero-Allocation Type Discipline** required for sub-50MB RSS baseline operations.

---

## 1. The Hidden Cost of `std::string::String`

A standard Rust `String` consumes:
- **24 bytes on the stack** (8-byte pointer, 8-byte length, 8-byte capacity).
- **A separate dynamic heap allocation** storing the actual character bytes, often over-allocated due to growth factor heuristics.

```rust
// ❌ WRONG: Naive Model with 5 Heap Allocations per record
pub struct ItemRecord {
    pub id: String,        // Heap alloc for 8-character ID
    pub title: String,     // Heap alloc
    pub duration: String,  // Heap alloc for "14:20" (5 bytes!)
    pub views: String,     // Heap alloc for "1.4M" (4 bytes!)
    pub status: String,    // Heap alloc for "active" (6 bytes!)
}
```

Under 1,000 requests per second returning 50 items each, this naive struct generates **250,000 heap allocations per second**.

---

## 2. Inlining Short Strings with `compact_str`

For strings that are typically $\le 24$ bytes on 64-bit systems (UUIDs, IDs, durations, ratings, status codes, country codes), use **`compact_str::CompactString`**.

- **$\le 24$ bytes**: Stored **100% on the stack** directly inside the 24-byte footprint. **Zero heap allocation**.
- **$> 24$ bytes**: Gracefully falls back to a single heap allocation.

```toml
# Cargo.toml
[dependencies]
compact_str = { version = "0.8", features = ["serde"] }
```

```rust
// ✅ CORRECT: Zero-Heap Allocation for short metadata
use compact_str::CompactString;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ItemRecord {
    pub id: CompactString,        // 0 heap alloc (stored in stack)
    pub duration: CompactString,  // 0 heap alloc (stored in stack)
    pub views: CompactString,     // 0 heap alloc (stored in stack)
    pub status: CompactString,    // 0 heap alloc (stored in stack)
    pub title: Box<str>,          // Trimmed heap allocation (no excess capacity)
}
```

---

## 3. Discarding Excess Capacity with `Box<str>`

When strings exceed 24 bytes and must live on the heap (such as article titles, descriptions, or URLs), never store them as `String` inside long-lived cache models or completed response records:

```rust
// ❌ String retains excess capacity buffer (24 bytes stack + heap buffer)
let title: String = parse_title();

// ✅ Box<str> shrinks stack size to 16 bytes (ptr + len) and shrinks heap to exact length
let title: Box<str> = parse_title().into_boxed_str();
```

### Memory Footprint Comparison:
| Type | Stack Size (64-bit) | Heap Allocation | Excess Capacity Kept? |
|---|---|---|---|
| `String` | 24 bytes | Heap | Yes (capacity padding) |
| `Box<str>` | 16 bytes | Heap | **No** (exact byte slice) |
| `CompactString` ($\le 24$B) | 24 bytes | **0 bytes** (inlined) | No |
| `Cow<'static, str>` | 24 bytes | **0 bytes** (if borrowed) | No |

---

## 4. Inlining Small Lists with `SmallVec`

Avoid allocating a `Vec` for small, bounded collections like tags, categories, or HTTP headers:

```toml
# Cargo.toml
[dependencies]
smallvec = { version = "1.13", features = ["serde"] }
```

```rust
use smallvec::SmallVec;
use compact_str::CompactString;

// Inlines up to 4 tags directly on the stack without allocating a Vec buffer
pub type InlineTags = SmallVec<[CompactString; 4]>;

#[derive(Debug, Clone, serde::Serialize)]
pub struct VideoMetadata {
    pub id: CompactString,
    pub title: Box<str>,
    pub tags: InlineTags, // 0 heap allocations if <= 4 tags!
}
```

---

## 5. Zero-Copy String Borrowing with `Cow<'a, str>`

When generating responses or transforming strings that may or may not need modification, use `std::borrow::Cow<'a, str>`:

```rust
use std::borrow::Cow;

pub fn normalize_slug(input: &str) -> Cow<'_, str> {
    if input.contains(' ') {
        // Only allocates heap when mutation is strictly necessary
        Cow::Owned(input.replace(' ', "-"))
    } else {
        // Zero-allocation borrowed reference
        Cow::Borrowed(input)
    }
}
```

---

## 6. Checklist: Designing Memory-Optimal Structs

1. Are short strings ($\le 24$ bytes) typed as `CompactString`?
2. Are long immutable strings stored as `Box<str>` instead of `String`?
3. Are small collections ($\le 4$ items) stored as `SmallVec` instead of `Vec`?
4. Are constant headers or static error messages typed as `&'static str` or `Cow<'static, str>`?
5. Has `cargo check` verified that types serialize seamlessly with `serde`?
