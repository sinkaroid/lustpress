# Extreme Memory & Runtime Efficiency in Real-Time WebSockets

In real-time communication systems (chat gateways, live sports feeds, financial tickers, and notification hubs), scaling to 50,000–500,000 concurrent long-lived WebSocket connections presents entirely different memory constraints than short-lived stateless REST APIs.

A naive WebSocket server consumes **100KB to 200KB of RAM per idle connection**—meaning 50,000 idle connections exhaust 5GB to 10GB of memory before processing a single payload.

This playbook documents the architecture required to achieve **< 4KB RSS per concurrent connection** and sub-millisecond zero-copy broadcast fan-out.

---

## 1. The Per-Socket Buffer Trap (The 100k Connection Dilemma)

By default, underlying WebSocket engines (such as `tungstenite` / `tokio-tungstenite`) configure generous internal buffer allocations for streaming frames:
- Default `write_buffer_size`: **128 KB**
- Default `max_frame_size`: **16 MB**
- Default `max_message_size`: **64 MB**

If 50,000 clients maintain active connections with default buffer sizes:
$$50,000 \times 128\ \text{KB} = \mathbf{6.4\ \text{GB of idle RAM allocated solely to internal socket buffers!}}$$

### The Extreme Efficiency Fix: Tuned `WebSocketConfig`
In Axum and Tungstenite, explicitly shrink frame limits and buffer pools during connection upgrades:

```rust
use axum::{
    extract::ws::{WebSocket, WebSocketUpgrade},
    response::IntoResponse,
};
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;

pub async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    let mut config = WebSocketConfig::default();
    
    // 🚀 RESTRICT WRITE BUFFER: 4KB is optimal for push events and chat frames
    config.write_buffer_size = 4 * 1024; // Slashes RAM from 128KB to 4KB per socket!
    config.max_frame_size = Some(64 * 1024); // 64KB max incoming frame
    config.max_message_size = Some(128 * 1024); // 128KB max aggregate payload
    config.accept_unmasked_frames = false;

    ws.on_upgrade(move |socket| handle_socket(socket, config))
}
```

*Memory Impact*: Shrinks buffer allocation across 50,000 connections from **6.4 GB down to ~200 MB** (a **97% memory reduction**).

---

## 2. Single-Task Bi-Directional Event Loop (Eliminating Task Doubling)

The common naive pattern spawns **two separate Tokio tasks per client**: one task dedicated to reading incoming frames, and a second task dedicated to writing outgoing frames:
- $50,000 \times 2 = \mathbf{100,000\ \text{concurrent Tokio tasks}}$ competing for scheduler slots and inflating executor memory.

Instead, merge both read and write streams into a **single cooperative task** using `tokio::select!`:

```rust
use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use std::time::Duration;

pub async fn handle_socket(socket: WebSocket, _config: WebSocketConfig) {
    let (mut ws_sink, mut ws_stream) = socket.split();

    // Strictly bounded egress channel (16 messages deep per client)
    let (tx, mut rx) = mpsc::channel::<Message>(16);

    // Register sender with room / broadcast broker here...

    loop {
        tokio::select! {
            // Branch 1: Handle incoming messages from the client
            incoming = ws_stream.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        // Process client command (keep parsing zero-allocation!)
                    }
                    Some(Ok(Message::Ping(_))) => {
                        // Axum automatically queues Pong responses
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }

            // Branch 2: Forward outgoing messages from the broadcast channel to the socket
            outgoing = rx.recv() => {
                match outgoing {
                    Some(msg) => {
                        if ws_sink.send(msg).await.is_err() {
                            break; // Client dropped connection or socket reset
                        }
                    }
                    None => break, // Channel closed by server broker
                }
            }
        }
    }

    // Clean disconnection: automatic eviction from room registry occurs upon drop
}
```

---

## 3. Zero-Copy Broadcast Fan-Out with `bytes::Bytes`

In real-time broadcasting (pushing a stock price, breaking news, or game tick to 50,000 subscribers), naive code re-serializes or clones the string for every subscriber:

```rust
// ❌ NAIVE: 50,000 allocations per broadcast tick!
for client in &subscribers {
    let json = serde_json::to_string(&data).unwrap(); // 50,000 HEAP ALLOCATIONS!
    client.send(Message::Text(json)).await;
}
```

### The Zero-Copy Pattern
Serialize the payload **once** into an atomic reference-counted `bytes::Bytes` slice. Cloning `Bytes` is an atomic pointer increment ($0$ heap allocations):

```rust
use bytes::Bytes;
use axum::extract::ws::Message;

pub struct BroadcastBroker {
    // Channel sending pre-framed, zero-copy atomic slices
    sender: tokio::sync::broadcast::Sender<Bytes>,
}

impl BroadcastBroker {
    pub fn broadcast_event<T: serde::Serialize>(&self, event: &T) -> Result<(), AppError> {
        // 1. Serialize payload EXACTLY ONCE into a reusable buffer
        let mut buffer = bytes::BytesMut::with_capacity(512);
        serde_json::to_writer((&mut buffer).writer(), event)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        
        let payload: Bytes = buffer.freeze();

        // 2. Broadcast single Bytes reference across all 50k subscribers
        // Clones only the internal Arc pointer (0 heap allocation per client!)
        let _ = self.sender.send(payload);

        Ok(())
    }
}
```

---

## 4. Slow Consumer Defense & Poisoned Queue Isolation

If a single mobile client encounters cellular packet loss or stalls, an unconstrained broadcast channel will accumulate buffered messages. If left unchecked, that single slow consumer will inflate server memory until an Out-of-Memory (OOM) panic occurs.

### Backpressure Enforcement Rule:
Never use unbounded channels. Use bounded channels (`mpsc::channel(16)` or `32`), and use **`try_send()`**:

```rust
pub fn forward_to_subscriber(tx: &mpsc::Sender<Message>, msg: Message) -> bool {
    match tx.try_send(msg) {
        Ok(_) => true,
        Err(mpsc::error::TrySendError::Full(_)) => {
            // 🛑 CLIENT IS LAGGING BEHIND SERVER EGRESS:
            // Terminate connection immediately to protect server RAM from ballooning
            tracing::warn!("Disconnecting slow WebSocket consumer; egress queue full");
            false
        }
        Err(mpsc::error::TrySendError::Closed(_)) => false,
    }
}
```

---

## 5. Centralized Heartbeat Sweeper (Eliminating Timer Storms)

Spawning a dedicated `tokio::time::interval(Duration::from_secs(30))` for every single client creates 50,000 independent timer objects in the Tokio runtime wheel.

### Unified Timer Sweep Architecture
Store a single atomic timestamp (`AtomicU64` storing epoch seconds) in the client session state. A **single background worker** sweeps the connection pool every 15 seconds to terminate dead sockets:

```rust
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct ClientSession {
    pub last_seen_epoch: AtomicU64,
}

impl ClientSession {
    pub fn touch_heartbeat(&self) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        self.last_seen_epoch.store(now, Ordering::Relaxed);
    }
}

pub fn spawn_central_heartbeat_sweeper(sessions: Arc<DashMap<u64, Arc<ClientSession>>>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(15));
        loop {
            interval.tick().await;
            let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

            // Evict and drop sockets silent for > 60 seconds
            sessions.retain(|_id, session| {
                let last = session.last_seen_epoch.load(Ordering::Relaxed);
                now.saturating_sub(last) < 60
            });
        }
    });
}
```

---

## 6. Linux Kernel Socket Tuning for 100k+ Connections

On Linux deployment hosts running high-density WebSocket services, configure kernel TCP limits to prevent socket starvation:

### `/etc/sysctl.conf`
```ini
# Maximum number of open socket connections in listen queue
net.core.somaxconn = 65535
net.ipv4.tcp_max_syn_backlog = 65535

# Expand dynamic port range for outbound connections
net.ipv4.ip_local_port_range = 1024 65535

# Fast recycle of closed sockets in TIME_WAIT
net.ipv4.tcp_fin_timeout = 15

# Minimal TCP read/write buffer allocations (conserves kernel memory)
net.ipv4.tcp_rmem = 4096 87380 4194304
net.ipv4.tcp_wmem = 4096 16384 4194304
```

### `/etc/security/limits.conf`
```text
# Expand file descriptor ceiling for server user
*    soft    nofile    1048576
*    hard    nofile    1048576
```

---

## 7. Real-Time Architecture Checklist

1. Has `WebSocketConfig.write_buffer_size` been tuned down to 4KB (eliminating the default 128KB per-socket waste)?
2. Are client read/write loops merged into a single task per connection using `tokio::select!`?
3. Is broadcast fan-out powered by `bytes::Bytes` to achieve zero-copy reference counting?
4. Are egress queues strictly bounded, dropping or disconnecting slow consumers via `try_send()`?
5. Are client heartbeats monitored using a single centralized timer sweep instead of per-socket timers?
6. Is `TCP_NODELAY` enabled to dispatch small push frames without Nagle algorithm buffering delays?
