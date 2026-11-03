# Zero-Syscall I/O & Kernel Bypass with io_uring

## The Problem: The Syscall and Context-Switch Wall

High-concurrency network servers utilizing `epoll` (standard Linux Tokio / Mio) face an inescapable hardware performance ceiling under extreme throughput (100k–500k+ packets/sec). Every network read, write, or socket registration requires:

1. **User-to-Kernel Mode Transitions**: Switching CPU privilege levels via the `syscall` instruction forces CPU pipeline flushes and invalidates Translation Lookaside Buffer (TLB) entries.
2. **KPTI (Kernel Page Table Isolation) Overhead**: Hardware mitigations for Spectre and Meltdown dramatically increase the cost of entering and exiting the Linux kernel.
3. **Repeated Buffer Pinning & Page Faults**: The kernel must validate, pin into RAM, and unpin userspace memory buffers on every single `read()` and `write()` call, triggering continuous memory controller bus traffic.

---

## 1. The io_uring Architectural Paradigm

Unlike `epoll` (which is a readiness notification mechanism), `io_uring` is a **true asynchronous completion queue** operating over two lock-free ring buffers shared directly between userspace and kernel memory:

```text
Userspace Memory ◄══════════════ Shared Ring Buffers ══════════════► Kernel Memory
  │                                                                       │
  ├──► [Submission Queue (SQ)] ──(Pushes I/O Requests: Read/Write)───────►┤
  │                                                                       │
  └──◄ [Completion Queue (CQ)] ◄─(Pops Completed Results with status)─────┤
```

Because both queues reside in shared memory (`mmap`), submitting requests and harvesting completions requires **zero system calls** during steady state.

---

## 2. Kernel Polling: Zero-Syscall I/O (`IORING_SETUP_SQPOLL`)

By configuring the kernel ring with `IORING_SETUP_SQPOLL`, the Linux kernel spawns a dedicated kernel thread that continuously polls the Submission Queue. The application simply writes descriptors to the ring; the kernel consumes them without any userspace interrupt or syscall.

### Production Setup using `io-uring` Crate

```rust
use io_uring::{opcode, types, IoUring};
use std::os::unix::io::AsRawFd;

pub struct ZeroSyscallEngine {
    ring: IoUring,
}

impl ZeroSyscallEngine {
    pub fn new(entries: u32) -> Result<Self, std::io::Error> {
        // Configure kernel polling thread (SQPOLL)
        let ring = IoUring::builder()
            .setup_sqpoll(2000) // Kernel thread sleeps after 2000ms idle
            .build(entries)?;

        Ok(Self { ring })
    }

    /// Submit an asynchronous socket read with ZERO syscalls
    pub unsafe fn submit_read(
        &mut self,
        fd: &std::net::TcpStream,
        buf: &mut [u8],
        user_data: u64,
    ) -> Result<(), std::io::Error> {
        let read_e = opcode::Read::new(
            types::Fd(fd.as_raw_fd()),
            buf.as_mut_ptr(),
            buf.len() as _,
        )
        .build()
        .user_data(user_data);

        // Push directly into shared userspace-kernel ring
        let mut sq = self.ring.submission();
        sq.push(&read_e).map_err(|_| std::io::Error::new(
            std::io::ErrorKind::WouldBlock,
            "Submission queue is full",
        ))?;
        sq.sync(); // Memory barrier to publish to kernel SQPOLL thread

        Ok(())
    }

    /// Harvest completed events without blocking
    pub fn reap_completions<F>(&mut self, mut callback: F)
    where
        F: FnMut(u64, i32), // user_data, result
    {
        let cq = self.ring.completion();
        for cqe in cq {
            callback(cqe.user_data(), cqe.result());
        }
    }
}
```

---

## 3. Pre-Registered Fixed Buffers (`io_uring_register_buffers`)

When buffers are registered upfront with the kernel via `io_uring_register_buffers`, the kernel maps the virtual pages into its page tables once at startup. Subsequent read/write operations bypass address translation, page pinning, and unpinning entirely.

```rust
use io_uring::IoUring;
use libc::iovec;

pub struct RegisteredMemoryPool {
    raw_memory: Vec<u8>,
    iovecs: Vec<iovec>,
}

impl RegisteredMemoryPool {
    pub fn new(ring: &mut IoUring, chunk_count: usize, chunk_size: usize) -> std::io::Result<Self> {
        let total_size = chunk_count * chunk_size;
        let mut raw_memory = vec![0u8; total_size];

        let mut iovecs = Vec::with_capacity(chunk_count);
        for i in 0..chunk_count {
            iovecs.push(iovec {
                iov_base: unsafe { raw_memory.as_mut_ptr().add(i * chunk_size) as *mut _ },
                iov_len: chunk_size,
            });
        }

        // Register buffers permanently with kernel
        ring.submitter().register_buffers(&iovecs)?;

        Ok(Self { raw_memory, iovecs })
    }
}
```

---

## 4. Architectural Selection Matrix: Tokio Epoll vs. io_uring

| Workload Dimension | Standard Tokio (`epoll`) | `io_uring` (`monoio` / `tokio-uring`) |
| :--- | :--- | :--- |
| **QPS Threshold** | Ideal for <100k QPS | Essential for >150k QPS per node |
| **Kernel Overhead** | 15%–25% CPU time in syscall/KPTI | <2% CPU time in kernel transitions |
| **Buffer Ownership** | Borrows slices (`&mut [u8]`) | Requires owned buffers (`Vec<u8>`) across submission/completion |
| **Portability** | Universal (Linux, macOS, Windows) | Strict Linux modern kernel (5.10+, ideally 6.x+) |
| **Disk Storage I/O** | Requires `spawn_blocking` thread pool | True non-blocking asynchronous direct disk I/O (`O_DIRECT`) |

---

## 5. Production Rules for io_uring in Rust

1. **Kernel Version Guard**: Always assert kernel version $\ge$ 5.10 at runtime; older kernels contained security bugs in SQPOLL privilege escalation.
2. **Buffer Pinning Discipline**: Since the kernel accesses memory asynchronously via pointers in the SQ, buffers submitted to `io_uring` must **never be moved, resized, or dropped** until the matching Completion Queue Entry (CQE) is reaped. Use pinned types or pool-allocated owned buffers.
3. **Graceful SQPOLL Fallback**: If the host environment forbids unprivileged `CAP_SYS_ADMIN` or `CAP_SYS_NICE`, catch the initialization error and fall back to normal `io_uring_enter()` submission or standard `epoll`.
