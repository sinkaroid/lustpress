# Zero-Cost Typestates, Compile-Time Domain Proofs & Buffer Recycling

## The Problem: Runtime Defensive Overhead and Heap Churn

High-throughput systems written with traditional object-oriented habits suffer from two compounding performance drains:

1. **Defensive Runtime Branching**: Repeated checks across internal function boundaries (e.g., verifying if a connection is authenticated, if an ID is valid, or if a payload is within size bounds). Every `if !conn.is_authed()` branch consumes branch predictor slots and adds latency.
2. **Ephemeral Allocation Churn**: Continuously allocating new strings, buffers, and vectors inside request loops (`format!("key:{}", id)`, `Vec::new()`, `payload.to_owned()`), leading to heap fragmentation and cache invalidations.

---

## 1. "Parse, Don't Validate" & Zero-Cost Domain Types

Validation checks an input, returns a boolean or unit `Ok(())`, and discards the proof. Downstream functions are forced to re-check or trust invariants without compiler guarantees.

Parsing inspects the input once and transforms it into a distinct type whose existence proves the invariant holds.

### Anti-Pattern: Validating with Plain Strings

```rust
// BAD: Every downstream method must wonder if `email` is valid
pub fn send_notification(email: &str, message: &str) -> Result<(), AppError> {
    if !email.contains('@') {
        return Err(AppError::InvalidEmail);
    }
    // ...
    Ok(())
}
```

### Production Pattern: Constructive Domain Parsing

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ValidatedEmail(String);

impl ValidatedEmail {
    pub fn parse(raw: String) -> Result<Self, DomainError> {
        if raw.contains('@') && raw.len() <= 254 {
            Ok(Self(raw))
        } else {
            Err(DomainError::InvalidEmail)
        }
    }

    #[inline(always)]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// Downstream functions accept the proof; ZERO runtime validation branches required
pub fn send_notification(email: &ValidatedEmail, message: &str) {
    // Guaranteed by the type system at zero runtime cost
}
```

---

## 2. Affine Typestates: Eliminating Runtime State Flags

Instead of maintaining a struct with mutable state flags (`is_connected: bool`, `is_authenticated: bool`) that must be checked on every socket operation, model connection progression as distinct compile-time states using phantom types.

State transitions consume `self` by value, making illegal states structurally impossible to represent.

### Production Pattern: Compile-Time Connection Pipeline

```rust
use std::marker::PhantomData;

// State markers (zero-sized types)
pub struct Disconnected;
pub struct Handshaking;
pub struct Established;

pub struct Connection<State> {
    stream: std::net::TcpStream,
    _marker: PhantomData<State>,
}

impl Connection<Disconnected> {
    pub fn new(stream: std::net::TcpStream) -> Self {
        Self {
            stream,
            _marker: PhantomData,
        }
    }

    /// Transition to Handshaking state
    pub fn begin_handshake(mut self) -> Result<Connection<Handshaking>, std::io::Error> {
        // Perform initial protocol exchange
        Ok(Connection {
            stream: self.stream,
            _marker: PhantomData,
        })
    }
}

impl Connection<Handshaking> {
    /// Transition to Established state upon valid auth token
    pub fn authenticate(mut self, token: &[u8]) -> Result<Connection<Established>, ProtocolError> {
        // Verify credentials
        Ok(Connection {
            stream: self.stream,
            _marker: PhantomData,
        })
    }
}

impl Connection<Established> {
    /// ONLY callable when the connection is provably established.
    /// Eliminates all `if !is_authed` checks!
    pub fn send_data(&mut self, payload: &[u8]) -> Result<(), std::io::Error> {
        use std::io::Write;
        self.stream.write_all(payload)
    }
}
```

---

## 3. Exhaustive Matching on Domain Enums

Never use wildcard arms (`_ =>`) when matching domain enums under your application's control. Wildcards hide unhandled variants when new protocol features are added.

```rust
pub enum ProtocolOp {
    Ping,
    Subscribe { channel_id: u64 },
    Publish { channel_id: u64, payload_len: u32 },
}

// BAD: Wildcard quietly ignores unhandled operations
// match op {
//     ProtocolOp::Ping => handle_ping(),
//     _ => handle_other(),
// }

// GOOD: Exhaustive match enforces complete handling at compile time
pub fn dispatch_op(op: ProtocolOp) {
    match op {
        ProtocolOp::Ping => handle_ping(),
        ProtocolOp::Subscribe { channel_id } => handle_subscribe(channel_id),
        ProtocolOp::Publish { channel_id, payload_len } => handle_publish(channel_id, payload_len),
    }
}
```

---

## 4. Buffer Recycling & Allocation Reuse

### `clone_from` for Reusing Existing Capacities

When a persistent worker must replace its state with newly received data, calling `.clone()` or `.to_owned()` releases the existing allocation and asks the heap allocator for a new buffer.

Using `clone_from` reuses the existing capacity in-place without allocator syscalls:

```rust
pub struct WorkerBuffer {
    pub scratch: String,
}

impl WorkerBuffer {
    pub fn update(&mut self, incoming: &str) {
        // Reuses the internal capacity of `self.scratch` if it fits!
        self.scratch.clone_from(&incoming.to_string());
        
        // Even better: clear and push without intermediate allocation
        self.scratch.clear();
        self.scratch.push_str(incoming);
    }
}
```

### In-Place String Formatting via `write!`

Avoid `format!` on the hot path. Use `std::fmt::Write` directly on a reusable buffer:

```rust
use std::fmt::Write;

pub fn build_cache_key(buffer: &mut String, prefix: &str, user_id: u64, tenant_id: u32) {
    buffer.clear(); // Keep preallocated capacity
    // Zero heap allocation! Writes directly into existing memory.
    let _ = write!(buffer, "{}:{}:{}", prefix, tenant_id, user_id);
}
```

---

## 5. Ring Buffer Slab Pooling for Network Frames

In high-concurrency packet and framing loops, avoid allocating a `Vec<u8>` per frame. Preallocate a bounded object pool using a lock-free queue (`crossbeam::queue::ArrayQueue`):

```rust
use crossbeam::queue::ArrayQueue;
use std::sync::Arc;

pub struct BufferPool {
    pool: Arc<ArrayQueue<Vec<u8>>>,
    buffer_capacity: usize,
}

impl BufferPool {
    pub fn new(pool_size: usize, buffer_capacity: usize) -> Self {
        let queue = ArrayQueue::new(pool_size);
        for _ in 0..pool_size {
            let _ = queue.push(Vec::with_capacity(buffer_capacity));
        }
        Self {
            pool: Arc::new(queue),
            buffer_capacity,
        }
    }

    pub fn acquire(&self) -> PooledBuffer {
        let buf = self.pool.pop().unwrap_or_else(|| Vec::with_capacity(self.buffer_capacity));
        PooledBuffer {
            buffer: Some(buf),
            pool: self.pool.clone(),
        }
    }
}

pub struct PooledBuffer {
    buffer: Option<Vec<u8>>,
    pool: Arc<ArrayQueue<Vec<u8>>>,
}

impl std::ops::Deref for PooledBuffer {
    type Target = Vec<u8>;
    fn deref(&self) -> &Self::Target {
        self.buffer.as_ref().unwrap()
    }
}

impl std::ops::DerefMut for PooledBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.buffer.as_mut().unwrap()
    }
}

impl Drop for PooledBuffer {
    fn drop(&mut self) {
        if let Some(mut buf) = self.buffer.take() {
            buf.clear(); // Retain capacity for next borrower
            let _ = self.pool.push(buf);
        }
    }
}
```

With `PooledBuffer`, frames borrow preallocated buffers and return them to the lock-free pool on drop without involving the OS heap allocator.
