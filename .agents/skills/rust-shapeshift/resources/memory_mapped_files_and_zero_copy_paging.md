# Memory-Mapped Files and Zero-Copy Paging Architecture

When applications process large binary datasets, disk caches, persistent logs, or static web assets, using standard filesystem APIs like `tokio::fs::read` or `std::fs::File::read_to_end` forces the OS to copy entire files into userspace heap buffers (`Vec<u8>`). This spikes process Resident Set Size (RSS), strains the allocator, and risks triggering Linux OOMKilled (Exit Code 137).

Memory-mapped file I/O delegates paging to the Linux Virtual Memory Subsystem and Page Cache, achieving **sub-megabyte memory footprints** even when processing multi-gigabyte files.

---

## 1. How Memory Mapping Prevents RSS Bloat

Standard read vs. memory-mapped I/O:

| Metric / Behavior | Standard `File::read_to_end` | Memory Mapping (`memmap2`) |
|---|---|---|
| **Userspace Heap Allocation** | Full file size allocated in heap `Vec<u8>` | **0 bytes** allocated on heap |
| **OS Page Cache Duplication** | 2x (Kernel page cache + userspace buffer) | **1x** (Userspace directly maps page cache) |
| **Memory Eviction Under Pressure** | None (OS cannot free userspace heap) | **Automatic** (Kernel drops clean pages) |
| **Paging Granularity** | Eager bulk copy | **On-demand** (4KB page faults as touched) |
| **OOM Risk** | High for files > available RAM | **Zero** (Pages flushed/evicted dynamically) |

---

## 2. Zero-Copy File Inspection with `memmap2`

Add `memmap2` to `Cargo.toml`:
```toml
[dependencies]
memmap2 = "0.9"
```

### Read-Only Zero-Copy Memory Map with Kernel Advice

```rust
use memmap2::{Mmap, MmapOptions, Advice};
use std::fs::File;
use std::path::Path;
use std::io::{Result, Error, ErrorKind};

pub struct PagedFileStorage {
    file: File,
    mmap: Mmap,
}

impl PagedFileStorage {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let file = File::open(path)?;
        let metadata = file.metadata()?;
        
        if metadata.len() == 0 {
            return Err(Error::new(ErrorKind::InvalidData, "Cannot mmap zero-length file"));
        }

        // Safety: File is opened read-only and no concurrent writes are made
        // by this process. Underlying file descriptor must not be truncated externally.
        let mmap = unsafe { MmapOptions::new().map(&file)? };

        // Kernel hint: Sequential reading optimization
        #[cfg(target_os = "linux")]
        let _ = mmap.advise(Advice::Sequential);

        Ok(Self { file, mmap })
    }

    /// Read a subslice with zero heap allocation
    #[inline(always)]
    pub fn read_slice(&self, offset: usize, length: usize) -> Option<&[u8]> {
        self.mmap.get(offset..offset + length)
    }

    /// Advise kernel that a memory range is no longer needed in physical RAM
    pub fn release_range_from_ram(&self, offset: usize, length: usize) {
        #[cfg(target_os = "linux")]
        let _ = self.mmap.advise_range(Advice::DontNeed, offset, length);
    }

    pub fn total_bytes(&self) -> usize {
        self.mmap.len()
    }
}
```

---

## 3. High-Performance Zero-Copy Static Asset Serving (Axum 0.8)

In web services, serving static files via `tokio::fs::read` loads file contents into per-request heap buffers. Under high concurrent traffic, 100 concurrent requests for a 10MB file consume 1GB of RSS.

Using `memmap2` allows hundreds of concurrent requests to borrow directly from the same shared kernel page cache:

```rust
use axum::{
    extract::{Path as AxumPath, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use memmap2::Mmap;
use std::collections::HashMap;
use std::fs::File;
use std::sync::Arc;

pub struct SharedAssetCache {
    mapped_assets: HashMap<String, Arc<Mmap>>,
}

impl SharedAssetCache {
    pub fn new() -> Self {
        Self {
            mapped_assets: HashMap::new(),
        }
    }

    pub fn preload_asset(&mut self, route_key: &str, file_path: &str) -> std::io::Result<()> {
        let file = File::open(file_path)?;
        // Safety: Read-only memory map for immutable server assets
        let mmap = unsafe { Mmap::map(&file)? };
        self.mapped_assets.insert(route_key.to_string(), Arc::new(mmap));
        Ok(())
    }
}

pub async fn serve_mmap_asset(
    AxumPath(asset_name): AxumPath<String>,
    State(cache): State<Arc<SharedAssetCache>>,
) -> Result<impl IntoResponse, StatusCode> {
    let mmap = cache.mapped_assets.get(&asset_name).ok_or(StatusCode::NOT_FOUND)?;

    // Zero userspace copying: bytes are streamed directly from shared OS pages
    let bytes = mmap.as_ref();
    let body_slice = bytes.to_vec(); // Or wrap in bytes::Bytes::from_static if static lifetime

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "application/octet-stream".parse().unwrap());
    headers.insert(header::CONTENT_LENGTH, mmap.len().into());

    Ok((headers, body_slice))
}
```

---

## 4. Safety Invariants & Mitigation

1. **External File Truncation (SIGBUS Defense)**:
   - If another process on Linux truncates an open mapped file while your service reads beyond the new EOF, the kernel generates a `SIGBUS` signal.
   - **Mitigation**: Exclusively map files owned by the service process, enforce read-only filesystem permissions, or verify file size via `file.metadata()?.len()`.

2. **Writable Memory Maps (`MmapMut`)**:
   - For Write-Ahead Logs (WAL) or shared ring buffers, use `MmapMut::map_mut(&file)`.
   - Call `mmap.flush_async()?` or `mmap.flush()?` to persist dirty pages deterministically without blocking the Tokio reactor loop.
