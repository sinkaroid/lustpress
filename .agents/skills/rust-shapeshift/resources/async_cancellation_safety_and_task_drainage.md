# Async Cancellation Safety & Graceful Task Drainage

In asynchronous Rust (Tokio), futures are driven to completion cooperatively. When an async operation is cancelled—such as when a client aborts an HTTP request, a timeout fires, or a branch loses the race in `tokio::select!`—the losing future is **immediately dropped** at its current `.await` point.

If the dropped future was executing a non-atomic operation (such as reading an exact number of bytes, sending a multi-step database transaction, or popping a job from a queue), state is permanently corrupted or lost.

---

## 1. The Anatomy of a Cancellation Bug

Consider this naive timeout pattern:

```rust
// ❌ WRONG: Non-cancellation-safe data corruption
tokio::select! {
    res = socket.read_exact(&mut header_buf) => {
        // If read_exact reads 3 out of 8 bytes before timeout fires...
    }
    _ = tokio::time::sleep(Duration::from_millis(50)) => {
        // ...The remaining 5 bytes are lost! Socket state is permanently desynchronized!
    }
}
```

When `tokio::time::sleep` completes first, `socket.read_exact` is dropped. The 3 bytes already read from the OS buffer are discarded with the future. Any subsequent read on `socket` starts at byte 4, resulting in payload corruption.

---

## 2. Cancellation Safety Reference Matrix

| Asynchronous Operation | Cancellation Safe? | Failure Mode if Cancelled |
|---|---|---|
| `tokio::time::sleep` | **YES** | Timer stops, no side effects. |
| `tokio::sync::mpsc::Receiver::recv` | **YES** | Item stays in queue or is returned; never lost. |
| `tokio::sync::broadcast::Receiver::recv` | **YES** | Message remains in ring buffer. |
| `tokio::io::AsyncReadExt::read` | **YES** | Bytes read are already in your buffer. |
| `tokio::io::AsyncReadExt::read_exact` | **NO** | Partial reads are discarded. |
| `tokio::io::AsyncWriteExt::write_all` | **NO** | Partial writes leave the recipient with corrupted data. |
| `tokio::sync::Mutex::lock` | **YES** | Waiter is removed from queue cleanly. |
| `deadpool::managed::Pool::get` | **YES** | Connection checkout is clean or cancelled. |
| Multi-step DB/Redis Transactions | **NO** | Intermediate states persist without commit. |

---

## 3. Pattern: Making Unsafe Writes Safe

When sending framed payloads or executing multi-step writes that must not be interrupted mid-stream, encapsulate the operation in a dedicated task or decouple cancellation:

```rust
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

pub struct SafeWriter {
    sender: mpsc::Sender<bytes::Bytes>,
}

impl SafeWriter {
    pub fn new<W: AsyncWriteExt + Unpin + Send + 'static>(mut writer: W) -> Self {
        let (tx, mut rx) = mpsc::channel::<bytes::Bytes>(256);

        // Dedicated writer task: CANNOT be cancelled by downstream HTTP client drops
        tokio::spawn(async move {
            while let Some(chunk) = rx.recv().await {
                if let Err(e) = writer.write_all(&chunk).await {
                    tracing::error!("Background write error: {e}");
                    break;
                }
            }
        });

        Self { sender: tx }
    }

    pub async fn send_chunk(&self, chunk: bytes::Bytes) -> Result<(), AppError> {
        self.sender.send(chunk).await.map_err(|_| AppError::Internal("Writer closed".into()))
    }
}
```

---

## 4. Graceful Task Drainage with `JoinSet` and `CancellationToken`

Never use unbounded `tokio::spawn` for background tasks without tracking their lifecycle. Fire-and-forget tasks get abruptly killed on SIGTERM, corrupting state and dropping in-flight writes.

Use **`tokio::task::JoinSet`** coupled with **`tokio_util::sync::CancellationToken`**:

```toml
# Cargo.toml
[dependencies]
tokio-util = "0.7"
```

```rust
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use std::time::Duration;

pub struct WorkerPool {
    token: CancellationToken,
    tasks: JoinSet<()>,
}

impl WorkerPool {
    pub fn new() -> Self {
        Self {
            token: CancellationToken::new(),
            tasks: JoinSet::new(),
        }
    }

    pub fn spawn_worker(&mut self, worker_id: u32) {
        let cancel = self.token.clone();
        self.tasks.spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        // 🛑 Clean shutdown signal received: finish atomic work and exit
                        tracing::info!(worker_id, "Worker gracefully draining...");
                        break;
                    }
                    _ = tokio::time::sleep(Duration::from_millis(500)) => {
                        // Perform atomic work batch
                    }
                }
            }
        });
    }

    pub async fn shutdown(mut self) {
        // 1. Broadcast shutdown signal to all workers
        self.token.cancel();

        // 2. Wait for all workers to finish their current work (with timeout)
        let drain_timeout = tokio::time::timeout(Duration::from_secs(10), async {
            while let Some(res) = self.tasks.join_next().await {
                if let Err(e) = res {
                    tracing::error!("Worker join error: {e}");
                }
            }
        });

        if drain_timeout.await.is_err() {
            tracing::warn!("Shutdown timed out; aborting lingering background tasks");
            self.tasks.shutdown().await;
        }
    }
}
```

---

## 5. Cancellation Safety Checklist

1. Are all branches inside `tokio::select!` verified against the Cancellation Safety Matrix?
2. Are `read_exact` and `write_all` operations shielded from arbitrary drop cancellations?
3. Are background tasks tracked inside a `JoinSet` rather than detached `tokio::spawn` calls?
4. Does the shutdown handler broadcast a `CancellationToken` and drain background tasks cleanly before exit?
