# Zero-Copy Inter-Process Communication (IPC) and Shared Memory

When microservices or co-located processes (e.g., Rust backend with an Envoy sidecar, a Python ML inference engine, or a local daemon) communicate on the same physical host or Kubernetes pod, using TCP loopback (`127.0.0.1`) wastes CPU on network packetization, TCP checksums, and double-kernel buffer copies. Unix Domain Sockets (UDS) and memory-mapped shared files (`memmap2`) eliminate this overhead.

---

## 1. Unix Domain Sockets (UDS) vs TCP Loopback

| Metric | TCP Loopback (`127.0.0.1`) | Unix Domain Socket (UDS) |
| :--- | :--- | :--- |
| **Network Stack** | Full TCP/IP stack (routing, checksum, ACK) | Kernel socket buffer only (bypasses IP) |
| **Throughput** | ~25–35 GB/s | ~60–80 GB/s |
| **Memory Overhead** | Double socket buffer (send + recv queues) | Single-copy kernel page stream |
| **Security** | Open to all local processes on port | Governed by Linux file permissions |

---

## 2. Axum over Unix Domain Sockets

Serve high-throughput HTTP/1.1 or HTTP/2 APIs over a local `.sock` file:

```rust
#[cfg(unix)]
use tokio::net::UnixListener;
use axum::{routing::get, Router};
use std::path::Path;

#[cfg(unix)]
pub async fn run_uds_server(socket_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(socket_path);
    if path.exists() {
        let _ = std::fs::remove_file(path); // Clean up stale socket file
    }

    let listener = UnixListener::bind(path)?;
    println!("Server listening on UDS: {socket_path}");

    let app = Router::new().route("/ping", get(|| async { "pong" }));

    // Serve directly over UDS
    axum::serve(listener, app).await?;

    Ok(())
}
```

---

## 3. Zero-Copy Shared Memory with `memmap2`

For transferring gigabyte-scale datasets (such as passing tensors to a Python process or streaming video frames) with zero heap copies, use memory-mapped shared files:

```rust
use memmap2::{MmapMut, MmapOptions};
use std::fs::OpenOptions;

pub struct SharedMemoryBuffer {
    mmap: MmapMut,
}

impl SharedMemoryBuffer {
    pub fn create_or_open(path: &str, size_bytes: usize) -> Result<Self, std::io::Error> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)?;

        file.set_len(size_bytes as u64)?;

        let mmap = unsafe { MmapOptions::new().map_mut(&file)? };

        Ok(Self { mmap })
    }

    pub fn write_slice_zero_copy(&mut self, offset: usize, data: &[u8]) -> Result<(), &'static str> {
        if offset + data.len() > self.mmap.len() {
            return Err("Buffer overflow: data exceeds shared memory bounds");
        }

        // Copies directly into shared kernel page cache (0 application heap allocation)
        self.mmap[offset..offset + data.len()].copy_from_slice(data);
        Ok(())
    }

    pub fn read_slice(&self, offset: usize, len: usize) -> Option<&[u8]> {
        self.mmap.get(offset..offset + len)
    }
}
```

---

## 4. Production Checklist

- [ ] Replaced TCP loopback (`127.0.0.1`) with Unix Domain Sockets for sidecar / daemon communication.
- [ ] Stale socket files cleaned up on process startup before binding `UnixListener`.
- [ ] Large multi-megabyte inter-process transfers use memory-mapped files (`memmap2`) instead of JSON over HTTP.
- [ ] File permissions on socket files restricted to the designated service user group.
