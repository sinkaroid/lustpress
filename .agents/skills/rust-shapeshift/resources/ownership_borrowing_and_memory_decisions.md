# Ownership, Borrowing, Visibility & Memory Decision Matrix

In Rust systems architecture, memory safety and extreme runtime efficiency are direct consequences of the **Ownership and Borrow Checker Permission Model**. Novice developers and unguided AI agents frequently resort to `.clone()` as a crutch to bypass compiler borrow errors, introducing unnecessary heap churn, latency spikes, and CPU cache misses.

This reference guide provides the authoritative **Decision Matrix** for placing data on the Stack vs Heap, mastering the Borrow Checker's permission mechanics, and applying strict visibility encapsulation.

---

## 1. The Borrow Checker Permission Model (R / W / O)

At any given point in execution, the Rust compiler tracks three fundamental permissions for every variable and memory path:

- **Read (`R`)**: The data can be read, copied, or referenced.
- **Write (`W`)**: The data can be mutated or overwritten.
- **Own (`O`)**: The owner can move, transfer, or deallocate the memory.

```text
       Variable Binding: let mut x = String::from("data");
       [ Permissions: R + W + O ]
             |
             +---> Shared Borrow: let y = &x;
             |     x: [ R (W and O SUSPENDED) ]
             |     y: [ R + O (points to x) ]
             |
             +---> Exclusive Borrow: let z = &mut x;
                   x: [ NONE (R, W, and O completely REVOKED) ]
                   z: [ R + W + O (exclusive access to x's target) ]
```

### The Universal Rule: Aliasing XOR Mutability
Memory can have:
- **Unlimited simultaneous Readers (`&T`)** with zero Writers, OR
- **Exactly ONE Writer (`&mut T`)** with zero Readers.

Never both simultaneously.

---

## 2. Eliminating the `.clone()` Anti-Pattern

When an AI agent or developer writes `.clone()`, it is often an architectural smell indicating a failure to understand ownership transfer or borrowing.

### Anti-Pattern 1: Cloning to pass data into a function that only reads it
```rust
// ❌ WRONG: Allocates a new String on the heap just to read its length
fn print_length(s: String) {
    println!("Length: {}", s.len());
}
print_length(name.clone());

// ✅ CORRECT: Borrow as a zero-cost slice (&str)
fn print_length(s: &str) {
    println!("Length: {}", s.len());
}
print_length(&name);
```

### Anti-Pattern 2: Cloning struct fields in getters
```rust
// ❌ WRONG: Clones internal state, forcing heap allocation on every call
pub struct Config {
    database_url: String,
}
impl Config {
    pub fn database_url(&self) -> String {
        self.database_url.clone()
    }
}

// ✅ CORRECT: Return a borrowed reference (&str)
impl Config {
    pub fn database_url(&self) -> &str {
        &self.database_url
    }
}
```

### When is `.clone()` acceptable?
1. Cloning primitives or small `Copy` types (`u32`, `bool`, `[u8; 16]`).
2. Cloning an `Arc<T>` pointer (only increments an atomic counter; 0 heap allocation).
3. Explicit, intentional creation of an independent, mutable copy when mutation must not affect the original.

---

## 3. The Memory Placement Decision Matrix: Where Should Data Live?

Use this decision matrix when defining structs, function signatures, and state:

```text
                                  Does the data outlive the current scope?
                                                /          \
                                              NO            YES
                                             /                \
                             Borrow as `&T` or `&str`          Is it shared across multiple
                             (Zero allocations, stack)         threads or Tokio tasks?
                                                                 /                 \
                                                               YES                  NO
                                                               /                     \
                                                     Use `Arc<T>` (Read-only)       Is the size known at
                                                     or `Arc<RwLock<T>>` (Mut)      compile-time & small (<2KB)?
                                                                                      /              \
                                                                                    YES               NO
                                                                                    /                   \
                                                                            Store DIRECT in      Is it a short string
                                                                            struct on Stack      (<= 24 bytes)?
                                                                            (Zero indirection)    /          \
                                                                                                YES           NO
                                                                                                /               \
                                                                                      Use `CompactString`    Use `Box<T>` / `Box<str>`
                                                                                      (Inlined on stack)     (Explicit Heap, lean Future)
```

### Detailed Placement Guidelines:

| Type | Allocation Location | Best Use Case | Anti-Pattern Warning |
|---|---|---|---|
| **Direct Struct Field** (`u64`, `bool`, `CompactString`) | **Stack / Inlined** | Fixed-size metadata, IDs, small state objects. | Do not put arrays > 4KB direct on the stack; risks Tokio task stack overflow. |
| **`Box<T>`** | **Heap (Single Owner)** | Large payload buffers, recursive types, keeping Tokio `Future` state machine sizes small. | Do not use for small primitives like `Box<u32>` (adds pointer indirection for no benefit). |
| **`Arc<T>`** | **Heap (Thread-Safe Shared)** | Long-lived shared singletons (`reqwest::Client`, Redis pools, global caches). | Do not wrap tiny ephemeral data in `Arc`; atomic reference counting has non-zero CPU cost. |
| **`&'a T` / `&str`** | **Borrowed (0 Allocation)** | Reading data within a single request or lexical lifetime. | Do not force lifetimes onto structs that must live in async task boundaries (`'static` required). |
| **`Cow<'a, str>`** | **Dual (Borrowed or Heap)** | Data that is mostly read as-is, but occasionally requires normalization or escaping. | Do not use if the data is *always* mutated or *never* mutated. |

---

## 4. Keeping Tokio `Future` State Machines Small

Every local variable held across an `.await` point becomes part of the generated compiler state machine struct:

```rust
// ❌ WRONG: Inflates Future size to >128KB!
async fn handle_request() {
    let large_buffer = [0u8; 128 * 1024]; // 128 KB on the stack!
    tokio::time::sleep(std::time::Duration::from_millis(10)).await; // Held across await!
    process_buffer(&large_buffer);
}

// ✅ CORRECT: Keep the Future footprint tiny (8 bytes pointer)
async fn handle_request() {
    let large_buffer = vec![0u8; 128 * 1024].into_boxed_slice(); // Heap-allocated Box
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    process_buffer(&large_buffer);
}
```

---

## 5. Visibility & Encapsulation Discipline

Exposing public struct fields (`pub`) breaks encapsulation, prevents zero-cost invariants, and prevents memory layout optimizations.

### Rule 1: Scoped Visibility
Use the narrowest possible visibility modifier:
- **`pub(crate)`**: Visible anywhere inside the current crate, but hidden from external consumers.
- **`pub(super)`**: Visible only to the immediate parent module.
- **Private (no modifier)**: Default. Visible only within the declaring module.
- **`pub`**: Reserved strictly for the public public-facing API surface.

```rust
pub struct ServerConfig {
    // ❌ WRONG: Blanket pub allows callers to invalidate invariants
    // pub bind_address: String,

    // ✅ CORRECT: Crate-internal read/write, external immutable access
    pub(crate) bind_address: CompactString,
    worker_threads: usize,
}

impl ServerConfig {
    pub fn bind_address(&self) -> &str {
        &self.bind_address
    }
}
```

### Rule 2: The Newtype Pattern for Zero-Cost Invariants
Never pass raw primitive strings for domain identifiers (e.g. `user_id: String` vs `video_id: String`):

```rust
use compact_str::CompactString;

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct VideoId(pub(crate) CompactString);

impl VideoId {
    pub fn new(raw: &str) -> Result<Self, AppError> {
        if raw.len() < 3 || raw.len() > 32 {
            return Err(AppError::Validation("Invalid VideoId length".into()));
        }
        Ok(Self(CompactString::new(raw)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
```
*Zero-Cost*: `VideoId` has the exact same memory layout and performance as `CompactString` (0 runtime overhead), but guarantees type safety at compile time.

---

## 6. Compile-Time Typestate Pattern

Enforce access permissions and valid state transitions at compile-time without storing runtime status enums or boolean flags:

```rust
use std::marker::PhantomData;

// Zero-sized state markers (0 bytes runtime overhead!)
pub struct Draft;
pub struct Published;

pub struct Article<State> {
    title: Box<str>,
    content: Box<str>,
    _state: PhantomData<State>,
}

impl Article<Draft> {
    pub fn new(title: &str, content: &str) -> Self {
        Self {
            title: title.into(),
            content: content.into(),
            _state: PhantomData,
        }
    }

    // Permission transition: Can only publish a Draft
    pub fn publish(self) -> Article<Published> {
        Article {
            title: self.title,
            content: self.content,
            _state: PhantomData,
        }
    }
}

impl Article<Published> {
    // Only published articles have a URL method!
    pub fn public_url(&self) -> String {
        format!("/articles/{}", self.title)
    }
}
```

---

## 7. Architecture Checklist

1. Are function arguments accepting borrowed references (`&str`, `&[T]`) instead of owned types (`String`, `Vec<T>`) whenever ownership transfer is unnecessary?
2. Has `.clone()` been audited and eliminated on hot paths?
3. Are large structs (>2KB) stored via `Box<T>` to keep Tokio `Future` sizes small?
4. Are struct fields kept private or scoped with `pub(crate)`?
5. Are domain primitives wrapped in zero-cost Newtypes to enforce validation invariants?
