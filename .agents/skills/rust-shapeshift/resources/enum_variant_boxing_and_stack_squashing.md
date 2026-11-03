# Enum Variant Boxing & Stack Squashing

In Rust systems architecture, an `enum`'s in-memory layout is determined by its largest variant:
$$\text{size\_of::<Enum>()} = \max(\text{size\_of::<Variant}_i\text{>()}) + \text{discriminant} + \text{alignment padding}$$

When a single oversized variant is added to a protocol, domain event, or state machine, **every single instance of that enum—even zero-sized unit variants—allocates that maximum size on the stack**.

In high-concurrency Tokio applications with 50,000+ active connections or recursive async task trees, oversized enums bloat the compiler-generated `Future` state machines, multiplying idle RAM consumption into tens of megabytes.

This reference guide documents how to squash enum footprints using variant boxing, compile-time size guards, and zero-cost pattern matching.

---

## 1. The Async Task Multiplication Trap

Consider a typical message event enum in a gateway or websocket server:

```rust
use std::mem::size_of;

pub struct PingPayload; // 0 bytes

pub struct MessagePayload {
    pub raw_buffer: [u8; 1024], // 1024 bytes!
    pub header_len: usize,
}

// NAIVE: Size of Event is 1024 + 8 + 8 (discriminant + padding) = 1040 bytes!
pub enum NaiveEvent {
    Ping(PingPayload),
    Heartbeat,
    Data(MessagePayload),
}

assert_eq!(size_of::<NaiveEvent>(), 1040);
```

### The Async Consequence
In Tokio, when an async task holds `NaiveEvent` across an `.await` boundary:
```rust
async fn process_event(event: NaiveEvent) {
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    match event { /* ... */ }
}
```
The compiler captures `NaiveEvent` inside the generator state machine:
- 50,000 idle tasks $\times$ 1,040 bytes $\approx$ **52 MB of RAM** consumed by empty padding while waiting on I/O.
- Even if 99.9% of messages are small `Heartbeat` or `Ping` packets, memory is allocated as if every message is a full 1KB payload.

---

## 2. The Solution: Boxing Oversized Variants

By wrapping variants exceeding 128 bytes in a `Box<T>`, the variant's stack footprint collapses into a single 8-byte heap pointer:

```rust
use std::mem::size_of;

pub struct MessagePayload {
    pub raw_buffer: [u8; 1024],
    pub header_len: usize,
}

// SQUASHED: Maximum variant size is now 8 bytes (Box pointer)
pub enum SquashedEvent {
    Ping,
    Heartbeat,
    Data(Box<MessagePayload>), // 8-byte pointer on 64-bit targets
}

// Size is 8 (pointer) + 8 (discriminant tag + padding) = 16 bytes!
const _: () = assert!(size_of::<SquashedEvent>() == 16);
```

### Memory Savings Comparison
| Metric | Naive Enum | Squashed (Boxed) Enum | Savings |
|---|---|---|---|
| **Single Enum Size** | 1,040 bytes | 16 bytes | **98.4% reduction** |
| **50,000 Tasks (Heartbeat)** | 52.0 MB | 0.8 MB | **51.2 MB reclaimed** |
| **L1 Data Cache (32KB)** | Holds ~30 events | Holds ~2,000 events | **66x cache density** |

Heap allocation is now triggered **only when the rare large variant is instantiated**, leaving the high-frequency ping/heartbeat paths 100% zero-allocation.

---

## 3. Pattern Matching and Moving Out of Box

Rust allows matching directly on boxed payloads without double-referencing:

```rust
pub fn handle_event(event: SquashedEvent) {
    match event {
        SquashedEvent::Ping => {
            // 0 heap, 0 pointer dereference
        }
        SquashedEvent::Heartbeat => {
            // 0 heap, 0 pointer dereference
        }
        SquashedEvent::Data(boxed_payload) => {
            // boxed_payload is Box<MessagePayload>
            // Unbox value if needed, or process via deref
            let payload: MessagePayload = *boxed_payload;
            println!("Processed header len: {}", payload.header_len);
        }
    }
}
```

---

## 4. Compile-Time Guard with Static Assertions

To prevent teammates or AI agents from accidentally adding unboxed bulky structs into the enum in the future, enforce a static size assertion:

```rust
pub enum ServiceCommand {
    ReloadConfig,
    DrainConnections,
    UpdateSession(Box<SessionUpdate>),
}

// Fails compilation if any future refactor pushes the enum above 24 bytes!
const _: () = assert!(
    std::mem::size_of::<ServiceCommand>() <= 24,
    "ServiceCommand exceeds 24-byte memory budget! Box large variants."
);
```

---

## 5. Architectural Checklist for Enums

1. Run `cargo clippy -- -W clippy::large_enum_variant` to find unboxed variants.
2. If the size difference between the smallest and largest variant exceeds **128 bytes**, box the largest variant.
3. For recursive data structures (trees, ASTs, JSON representations), always box child enum nodes.
4. Enforce `const _: () = assert!(size_of::<MyEnum>() <= BUDGET);` for core protocol types.
