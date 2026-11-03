# Thread-Per-Core & Shared-Nothing Architecture

## The Problem: The Hidden Tax of Work-Stealing Schedulers

Multi-threaded async runtimes (such as standard multi-threaded Tokio) default to a **work-stealing algorithm**. While convenient for general-purpose workloads, work-stealing creates severe hardware bottlenecks under high throughput:

1. **L1/L2 Cache Invalidation & Thrashing**: When Core 1 runs out of work and steals a future from Core 2's deque, the task's working data is pulled across the CPU interconnect bus. The cache lines in Core 2's L1/L2 cache are invalidated via hardware coherence protocols (MESI/MOESI), destroying cache locality.
2. **Atomic Contention on Task Queues**: Work-stealing deques require atomic compare-and-swap operations on head and tail pointers. Under heavy thread counts (32–128 cores), cache-line bouncing on shared run queues degrades instruction execution pipelines.
3. **Cross-Thread Synchronization Overhead**: Sharing state across worker threads forces the use of cross-thread locks (`parking_lot::Mutex`, `tokio::sync::Mutex`) or multi-producer queues, introducing lock contention and tail latency jitter.

---

## 1. The Thread-Per-Core (Shared-Nothing) Philosophy

Inspired by high-performance systems like ScyllaDB, Seastar, Datadog Vector, and high-frequency trading engines, the **Thread-Per-Core (Shared-Nothing)** architecture eliminates cross-core interference entirely:

```text
┌───────────────── CPU Core 0 ─────────────────┐  ┌───────────────── CPU Core 1 ─────────────────┐
│  [OS Thread Pinned to Core 0]                │  │  [OS Thread Pinned to Core 1]                │
│  ├── Local Single-Thread Executor (Current)  │  │  ├── Local Single-Thread Executor (Current)  │
│  ├── Local Memory Arena / Object Pool        │  │  ├── Local Memory Arena / Object Pool        │
│  └── Local Socket File Descriptors (REUSEPORT)│ │  └── Local Socket File Descriptors (REUSEPORT)│
└──────────────────────┬────────────────────────┘  └──────────────────────▲────────────────────────┘
                       │                                                  │
                       └─────────── Lock-Free SPSC Ring Buffer ───────────┘
                                  (Inter-Core Message Handoff)
```

### Core Tenets:
1. **1 Hardware Core = 1 OS Thread = 1 Local Executor**.
2. **Zero Cross-Core Sharing**: Every core manages its own connections, memory arenas, caches, and timers.
3. **Zero Mutexes on Hot Path**: Because memory is owned exclusively by the pinned thread, state mutation uses standard `RefCell` or raw borrowing—**zero atomic synchronization overhead**.
4. **Inter-Core Communication via SPSC Rings**: If Core 0 must message Core 1, it deposits an event into a single-producer single-consumer lock-free queue.

---

## 2. Pinned OS Threads & CPU Affinity in Rust

Bind worker threads strictly to physical CPU cores to prevent the Linux kernel scheduler from migrating threads across cores:

```rust
use std::thread;

pub fn spawn_pinned_worker<F>(core_id: usize, work: F) -> thread::JoinHandle<()>
where
    F: FnOnce() + Send + 'static,
{
    thread::Builder::new()
        .name(format!("worker-core-{}", core_id))
        .spawn(move || {
            // Pin the current OS thread to the designated physical core
            let core_ids = core_affinity::get_core_ids().expect("failed to get core IDs");
            if let Some(core) = core_ids.get(core_id) {
                core_affinity::set_for_current(*core);
            }

            work();
        })
        .expect("failed to spawn worker thread")
}
```

---

## 3. Kernel SO_REUSEPORT Network Partitioning

Instead of accepting TCP connections on a single central listener and delegating sockets across threads, each pinned core opens its own independent `TcpListener` bound to the same port using `SO_REUSEPORT`.

The Linux kernel's network stack distributes inbound SYN packets directly across listeners using a 4-tuple hash:

```rust
use socket2::{Domain, Protocol, Socket, Type};
use std::net::SocketAddr;

pub fn create_reuseport_listener(addr: SocketAddr) -> std::io::Result<std::net::TcpListener> {
    let domain = if addr.is_ipv6() { Domain::IPV6 } else { Domain::IPV4 };
    let socket = Socket::new(domain, Type::STREAM, Some(Protocol::TCP))?;

    // Enable SO_REUSEPORT: kernel distributes incoming connections evenly
    socket.set_reuse_port(true)?;
    socket.set_reuse_address(true)?;
    socket.set_nonblocking(true)?;

    socket.bind(&addr.into())?;
    socket.listen(1024)?;

    Ok(socket.into())
}
```

---

## 4. Single-Threaded Tokio Current-Thread Isolation

Run an independent Tokio `current_thread` runtime on each pinned core:

```rust
pub fn run_thread_per_core_cluster(addr: SocketAddr, num_cores: usize) {
    let mut handles = Vec::new();

    for core_id in 0..num_cores {
        let handle = spawn_pinned_worker(core_id, move || {
            // Local current_thread runtime: NO WORK-STEALING!
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build local runtime");

            rt.block_on(async move {
                let listener = create_reuseport_listener(addr).expect("failed to bind reuseport");
                let listener = tokio::net::TcpListener::from_std(listener).expect("convert listener");

                loop {
                    let (socket, _) = match listener.accept().await {
                        Ok(conn) => conn,
                        Err(_) => continue,
                    };

                    // Spawns locally on this core ONLY: 100% L1/L2 cache locality
                    tokio::task::spawn_local(async move {
                        handle_connection_locally(socket).await;
                    });
                }
            });
        });

        handles.push(handle);
    }

    for h in handles {
        let _ = h.join();
    }
}

async fn handle_connection_locally(mut socket: tokio::net::TcpStream) {
    // Zero atomic overhead: local state mutation without cross-thread locks
}
```

---

## 5. Performance Comparison

| Metric | Work-Stealing Runtime | Thread-Per-Core (Shared-Nothing) |
| :--- | :--- | :--- |
| **L1/L2 Cache Hit Rate** | ~75%–85% under high load | **>98%** (zero cross-core task migration) |
| **Tail Latency (p99.99)** | Millisecond-level jitter spikes | **Deterministic microsecond range** |
| **Core Scalability** | Diminishing returns past 32 cores | **Linear scaling up to 128+ cores** |
| **Memory Allocation** | Global allocator lock contention | Core-local arenas (zero lock contention) |
