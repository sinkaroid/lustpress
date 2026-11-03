# NUMA Architecture, HugePages & TLB Cache Saturation

## The Problem: Silicon Topography & The Memory Bus Penalty

On multi-socket servers or modern multi-chiplet processors (AMD EPYC, Intel Xeon, Ampere Altra), physical RAM is not a uniform pool. It is partitioned across multiple **NUMA (Non-Uniform Memory Access) Nodes**, where each CPU socket or core complex directly controls a local bank of memory channels:

1. **Remote NUMA Interconnect Latency**: When a thread executing on CPU Socket 0 accesses data stored in RAM connected to Socket 1, the memory request must traverse the inter-socket bus (AMD Infinity Fabric / Intel UPI). Remote memory access incurs **2x to 3x higher latency** and consumes scarce inter-socket bandwidth.
2. **Translation Lookaside Buffer (TLB) Thrashing**: Standard Linux virtual memory uses 4 KB pages. When an application manages large buffers (e.g., a 1 GB packet buffer, cache, or database page pool), the CPU must map **262,144 page entries**. The CPU's L1/L2 TLB (which can hold only 64–1,500 entries) is perpetually thrashed, forcing the hardware MMU to perform slow 4-level page table walks in RAM.
3. **Paging Jitter**: Under memory pressure, the Linux kernel page daemon (`kswapd`) can swap out active pages or rebalance memory zones, causing latency spikes.

---

## 1. NUMA Node Topology in Rust

Before allocating large shared structures, discover the host hardware NUMA nodes and bind threads to memory controllers co-located with their physical CPU socket:

```rust
#[cfg(target_os = "linux")]
pub fn get_current_numa_node() -> Option<usize> {
    let mut cpu: u32 = 0;
    let mut node: u32 = 0;
    
    // Use vDSO getcpu syscall (zero kernel transition)
    let ret = unsafe {
        libc::syscall(
            libc::SYS_getcpu,
            &mut cpu as *mut u32,
            &mut node as *mut u32,
            std::ptr::null_mut::<libc::c_void>(),
        )
    };

    if ret == 0 {
        Some(node as usize)
    } else {
        None
    }
}
```

### Allocating NUMA-Local Memory with `libnuma`

When preallocating large pools or ring buffers, bind the memory physical pages to the current thread's NUMA node using `mbind`:

```rust
#[cfg(target_os = "linux")]
pub unsafe fn allocate_numa_local(size_bytes: usize, numa_node: usize) -> Result<*mut u8, std::io::Error> {
    // 1. Allocate anonymous memory via mmap
    let ptr = libc::mmap(
        std::ptr::null_mut(),
        size_bytes,
        libc::PROT_READ | libc::PROT_WRITE,
        libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
        -1,
        0,
    );

    if ptr == libc::MAP_FAILED {
        return Err(std::io::Error::last_os_error());
    }

    // 2. Bind memory pages strictly to the target NUMA node
    let mut nodemask: libc::c_ulong = 1 << numa_node;
    let ret = libc::mbind(
        ptr,
        size_bytes,
        libc::MPOL_BIND, // Strict policy: fail rather than allocate remote memory
        &mut nodemask,
        std::mem::size_of::<libc::c_ulong>() as libc::c_ulong * 8,
        0,
    );

    if ret != 0 {
        libc::munmap(ptr, size_bytes);
        return Err(std::io::Error::last_os_error());
    }

    Ok(ptr as *mut u8)
}
```

---

## 2. HugePages (2 MB & 1 GB Pages): Eliminating TLB Misses

Replacing standard 4 KB pages with **2 MB HugePages** reduces page table entries by a factor of **512x**, allowing massive memory buffers to fit entirely inside the CPU's hardware TLB cache:

| Metric | Standard 4 KB Page | 2 MB HugePage |
| :--- | :--- | :--- |
| **Page Table Walk Depth** | 4 levels (PML4 $\to$ PDP $\to$ PD $\to$ PT) | **3 levels** (bypasses Page Table) |
| **TLB Coverage (1,024 entries)** | 4 MB of RAM | **2 GB of RAM** |
| **Address Translation Time** | 20–40 ns on TLB miss | **<1 ns (Hot TLB Hit)** |

### Allocating HugePages via `madvise` (Transparent HugePages - THP)

For application buffers that grow dynamically, advise the Linux kernel to back the virtual memory range with 2 MB transparent huge pages:

```rust
pub unsafe fn advise_hugepages(ptr: *mut u8, len: usize) -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        // Require memory address to be aligned to 2MB boundary
        let ret = libc::madvise(ptr as *mut _, len, libc::MADV_HUGEPAGE);
        if ret != 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}
```

### Direct Allocation via `MAP_HUGETLB` (Explicit 2MB HugePages)

For dedicated ring buffers and frame pools:

```rust
#[cfg(target_os = "linux")]
pub unsafe fn allocate_explicit_hugepages(size_bytes: usize) -> Result<*mut u8, std::io::Error> {
    // Round up size to 2MB boundary
    let page_size = 2 * 1024 * 1024;
    let aligned_size = (size_bytes + page_size - 1) & !(page_size - 1);

    let ptr = libc::mmap(
        std::ptr::null_mut(),
        aligned_size,
        libc::PROT_READ | libc::PROT_WRITE,
        libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_HUGETLB,
        -1,
        0,
    );

    if ptr == libc::MAP_FAILED {
        return Err(std::io::Error::last_os_error());
    }

    Ok(ptr as *mut u8)
}
```

---

## 3. Physical Page Pinning with `mlockall`

To eliminate latency spikes caused by minor page faults or Linux page eviction, lock the entire process address space into physical RAM:

```rust
pub fn lock_process_memory() -> Result<(), std::io::Error> {
    #[cfg(target_os = "linux")]
    unsafe {
        // MCL_CURRENT: Lock all currently mapped pages
        // MCL_FUTURE:  Lock all pages mapped in the future
        let ret = libc::mlockall(libc::MCL_CURRENT | libc::MCL_FUTURE);
        if ret != 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}
```

*Note: Requires `ulimit -l unlimited` or the Linux `CAP_IPC_LOCK` capability.*

---

## 4. Production Architectural Checklist for Silicon Topography

1. **NUMA Binding with Thread Affinity**: Pair every `core_affinity::set_for_current` call with NUMA-local memory allocation (`MPOL_BIND`). Never allow a worker thread on Socket 1 to allocate buffers on Socket 0.
2. **Pre-Fault Buffers at Startup**: Always write a single byte to every 2 MB page during the initialization phase (`ptr.write_volatile(0)`) to trigger kernel page allocation upfront, ensuring zero fault overhead during runtime.
3. **Verify via Linux Procfs**: Inspect `/proc/<pid>/numa_maps` to ensure zero pages are mapped on remote NUMA nodes.
