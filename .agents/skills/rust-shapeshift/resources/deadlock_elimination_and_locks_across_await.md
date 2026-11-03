# Deadlock Elimination & Lock Safety Across `.await` Points

In multi-threaded asynchronous systems powered by Tokio, holding a synchronous lock (`std::sync::Mutex` or `parking_lot::Mutex`) across an `.await` boundary is one of the most destructive architectural bugs in production Rust.

It triggers **Tokio worker thread starvation**, completely stalling the asynchronous reactor and freezing all incoming HTTP/gRPC requests without producing a single panic or error log.

This reference guide documents the mechanics of the `.await` lock hazard, RAII block scoping disciplines, when to use `tokio::sync::Mutex`, and how to replace lock contention with zero-cost channel-based actor patterns.

---

## 1. The Anatomy of an Async Lock Freeze

### The Anti-Pattern (Total Reactor Deadlock)
```rust
use std::sync::Mutex;

pub struct SharedState {
    counter: u64,
}

// HAZARD: Holding a synchronous MutexGuard across an .await point!
pub async fn increment_and_notify_hazard(state: &Mutex<SharedState>) {
    let mut guard = state.lock().unwrap();
    guard.counter += 1;

    // The current OS worker thread is suspended here waiting for network I/O.
    // The synchronous MutexGuard is NOT released!
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    println!("Counter: {}", guard.counter);
    // guard dropped here
}
```

### Why This Freezes Tokio
1. **Worker Thread Exhaustion**: Tokio runtimes typically allocate 1 OS thread per CPU core (e.g. 4 or 8 threads).
2. **Cascading Block**:
   - Task 1 acquires the synchronous lock on Thread A, then hits `.await` and yields.
   - Task 2 runs on Thread B and attempts to acquire `state.lock()`. Because it is a synchronous lock, Thread B **physically blocks the OS thread via a kernel futex syscall**.
   - As more tasks hit the lock, all 4 or 8 Tokio worker threads become physically blocked waiting on the lock.
   - Now, Task 1's timer or network read cannot wake up because **no worker threads are free to run the Tokio scheduler**.
   - The process enters a **permanent, unrecoverable deadlock**.

---

## 2. Pattern A: RAII Block Scoping (Synchronous Work)

If the critical section only involves in-memory data manipulation and requires no async I/O, **confine the lock guard to an explicit synchronous scope** that drops before `.await`:

```rust
use parking_lot::Mutex;

pub struct SharedState {
    counter: u64,
}

pub async fn increment_and_notify_safe(state: &Mutex<SharedState>) {
    // 1. Enter isolated synchronous block
    let new_val = {
        let mut guard = state.lock();
        guard.counter += 1;
        guard.counter // Copy out primitive or clone small data
    }; // <-- guard is GUARANTEED to be dropped here!

    // 2. Perform async operation safely with ZERO locks held
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    println!("Counter updated to: {}", new_val);
}
```

By constraining `guard` to the inner block, the synchronous lock duration is measured in **nanoseconds**, completely eliminating thread starvation.

---

## 3. Pattern B: When to Use `tokio::sync::Mutex`

If you **must** hold a lock across an `.await` point (e.g. exclusive transactional access to an async socket or stream):

```rust
use tokio::sync::Mutex;

pub struct SocketSession {
    stream: tokio::net::TcpStream,
}

// SAFE: tokio::sync::Mutex yields cooperatively to the reactor
pub async fn send_exclusive_packet(session: &Mutex<SocketSession>, data: &[u8]) {
    // .lock().await yields the task if contended, freeing the OS worker thread!
    let mut guard = session.lock().await;

    // Safe to hold across async I/O
    use tokio::io::AsyncWriteExt;
    let _ = guard.stream.write_all(data).await;

    // Lock released automatically when guard drops
}
```

### Selection Matrix: `parking_lot::Mutex` vs `tokio::sync::Mutex`
| Dimension | `parking_lot::Mutex` | `tokio::sync::Mutex` |
|---|---|---|
| **Memory Footprint** | **1 byte** | **64–88 bytes** |
| **Uncontended Lock Speed** | **~10 ns** (CPU atomic) | **~50–80 ns** (async machinery) |
| **Cross-`.await` Safety** | **DEADLOCK HAZARD** | **100% Safe** |
| **Best Production Use Case** | In-memory structs, fast state updates, cache maps | Shared network streams, exclusive async transactions |

---

## 4. Pattern C: Zero-Lock Architecture via Actor Channels

For high-throughput systems, the cleanest way to eliminate locks entirely is **single-writer message passing**:

```rust
use tokio::sync::mpsc;

pub enum StateMsg {
    Increment { respond_to: tokio::sync::oneshot::Sender<u64> },
}

pub struct StateActor {
    counter: u64,
    receiver: mpsc::Receiver<StateMsg>,
}

impl StateActor {
    pub async fn run(mut self) {
        while let Some(msg) = self.receiver.recv().await {
            match msg {
                StateMsg::Increment { respond_to } => {
                    self.counter += 1;
                    let _ = respond_to.send(self.counter);
                }
            }
        }
    }
}
```

Only one task owns the state. All other tasks send messages via bounded channels with zero lock contention.

---

## 5. Architectural Checklist for Lock Safety

1. Enforce `#![deny(clippy::await_holding_lock)]` in `src/lib.rs` and `src/main.rs`.
2. Never call `std::sync::Mutex::lock()` inside an `async fn` without explicit RAII block scoping.
3. Use `parking_lot::Mutex` for sub-microsecond in-memory state updates; never hold it across `.await`.
4. Use `tokio::sync::Mutex` only when exclusive state must be preserved across async I/O boundaries.
