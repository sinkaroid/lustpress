# Zero-Runtime Minimalist Daemons: The Sub-Megabyte RSS Pattern (The Koburi Model)

A common anti-pattern in modern Rust engineering is blindly introducing `tokio`, `axum`, and multi-threaded async executors into lightweight daemons, monitoring sidecars, CLI utilities, or low-QPS internal services. A multi-threaded async runtime pays a non-trivial memory tax upfront: worker thread stacks, scheduling queues, timer wheels, and glibc arenas consume 10MB to 40MB of RSS just sitting completely idle.

When the target environment is a memory-constrained host (e.g. a 512MB VPS or an edge container with dozens of co-located services), the tool itself must never become part of the memory problem it monitors. Battle-tested in production (e.g. `koburi`), an optimized zero-runtime Rust daemon can run 24/7 with continuous minute-by-minute scraping at **sub-megabyte memory usage (down to ~50KB RSS)**.

---

## 1. The Decision Matrix: Async Runtime vs Plain Synchronous

Before adding a single async dependency, classify the service workload:

| Workload Property | High-Throughput Service (Archetype A) | Minimalist Daemon / Sidecar (Archetype B) |
| :--- | :--- | :--- |
| **Concurrency Target** | 1,000 to 500,000 concurrent sockets | 1 to 5 concurrent connections (polling / low QPS) |
| **Request Cadence** | Continuous stream (>1,000 QPS) | Sporadic (e.g. health checks every 10–30s) |
| **Runtime Selection** | **Tokio multi-thread** / Axum | **Zero Async Runtime** (`std::net` / `std::thread`) |
| **Idle Memory Budget** | 20MB – 50MB RSS | **Sub-megabyte (<1MB, down to ~50KB RSS)** |
| **Connection Strategy** | Persistent connection pools | On-demand ephemeral sockets |
| **Linking Target** | `glibc` with jemalloc/malloc_trim | `x86_64-unknown-linux-musl` static linking |

---

## 2. Rule 1: No Async Runtime for Low-QPS Daemons

If a daemon serves one or two endpoints hit on a polling interval (e.g., container metrics, node health probes, local proxying), use `std::net::TcpListener` with a synchronous accept loop. 

A single-threaded or sequential accept loop pays **0 bytes** of executor overhead at idle:

```rust
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

pub fn run_sub_5mb_server(bind_addr: &str) -> std::io::Result<()> {
    let listener = TcpListener::bind(bind_addr)?;
    println!("Minimalist daemon listening on {bind_addr} (Zero Async Runtime)");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                // Handled sequentially with 0 thread stack allocation
                if let Err(e) = handle_connection(stream) {
                    eprintln!("Connection error: {e}");
                }
            }
            Err(e) => eprintln!("Accept error: {e}"),
        }
    }

    Ok(())
}

fn handle_connection(mut stream: TcpStream) -> std::io::Result<()> {
    // Fixed 1KB stack buffer: 0 heap allocations
    let mut buffer = [0u8; 1024];
    let bytes_read = stream.read(&mut buffer)?;

    if bytes_read > 0 && buffer.starts_with(b"GET /stats") {
        let stats_json = collect_metrics_snapshot();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            stats_json.len(),
            stats_json
        );
        stream.write_all(response.as_bytes())?;
    } else {
        stream.write_all(b"HTTP/1.1 404 NOT FOUND\r\nConnection: close\r\n\r\n")?;
    }

    // Socket automatically dropped and closed here: 0 lingering socket RAM
    Ok(())
}

fn collect_metrics_snapshot() -> &'static str {
    "{\"status\":\"healthy\",\"uptime\":3600}"
}
```

---

## 3. Rule 2: On-Demand Ephemeral Connections Over Pooling

In high-QPS systems, connection pooling saves handshake latency. But in low-cadence daemons (hitting a local Docker socket `/var/run/docker.sock` once every 15 seconds), holding an open connection pool is an anti-pattern:
- A pooled connection reserves kernel TCP/socket buffers.
- It can go stale or break mid-request.

**Best Practice**: Connect on demand, execute request, close immediately:

```rust
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::io::{Read, Write};

#[cfg(unix)]
pub fn query_docker_stats_on_demand() -> std::io::Result<String> {
    // 1. Open on-demand socket
    let mut socket = UnixStream::connect("/var/run/docker.sock")?;

    // 2. Write HTTP request to daemon
    socket.write_all(b"GET /containers/json HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")?;

    // 3. Read response into bounded buffer
    let mut response = String::with_capacity(4096);
    socket.read_to_string(&mut response)?;

    // 4. Socket closed on exit: 0 idle connection RAM held
    Ok(response)
}
```

---

## 4. Rule 3: `musl` Static Linking to Eliminate Allocator Arenas

Dynamic linking against Linux `glibc` pulls in glibc's multi-threaded arena allocator by default, adding baseline memory overhead. Compiling against `musl` statically bundles a lean, simple allocator with zero dynamic linker resolution cost:

```bash
# Build fully static Linux binary
cargo build --release --target x86_64-unknown-linux-musl
strip target/x86_64-unknown-linux-musl/release/my-daemon
```

Verify compliance:
```bash
ldd target/x86_64-unknown-linux-musl/release/my-daemon
# Output must be: "not a dynamic executable"
```

Configure `Cargo.toml` for size and dead-code elimination:
```toml
[profile.release]
opt-level = "z"
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true
```

---

## 5. Rule 4: Zero Dependencies Discipline (Stdlib-First)

Every external crate introduced can bring hidden threads, global statics, or background memory allocation. For minimalist daemons:
- Use `std::net`, `std::os::unix::net`, `std::io`, `std::thread`.
- Hand-roll minimal HTTP response headers and JSON serialization if endpoints are simple.
- Keep `Cargo.toml` `[dependencies]` strictly empty or limited to zero-dependency core crates.

---

## 6. Verification Checklist

- [ ] Evaluated project archetype: Is async concurrency actually needed, or is sequential/blocking I/O sufficient?
- [ ] Confirmed 0 Tokio dependencies in `Cargo.toml` for low-QPS daemons.
- [ ] Idle RSS verified with `ps -o rss -p $(pgrep <bin>)` to be **sub-megabyte (down to ~50KB in continuous production)**.
- [ ] Static `musl` binary produced with `x86_64-unknown-linux-musl`.
- [ ] Ephemeral sockets closed immediately after processing (`Connection: close`).
