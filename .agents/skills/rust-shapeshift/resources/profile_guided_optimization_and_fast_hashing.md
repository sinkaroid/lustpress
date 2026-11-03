# Profile-Guided Optimization (PGO) and Fast Hashers

Standard release builds (`cargo build --release`) optimize code based on static compiler heuristics. Profile-Guided Optimization (PGO) informs LLVM of the actual execution frequency of branches, loops, and function calls from real production workloads, boosting runtime throughput by 10% to 30% without changing a single line of application code.

---

## 1. The PGO Workflow

1. **Instrument**: Build the binary with instrumentation hooks that record branch counters.
2. **Profile**: Run representative production-like workloads (load tests, stress benchmarks).
3. **Merge**: Combine profile counters with `llvm-profdata`.
4. **Optimize**: Rebuild using the merged profile data.

### Automated PGO Shell Pipeline
```bash
#!/bin/bash
set -e

PGO_DIR="/tmp/pgo-data"
rm -rf "$PGO_DIR"
mkdir -p "$PGO_DIR"

# Step 1: Instrumented Build
echo "Building instrumented binary..."
RUSTFLAGS="-Cprofile-generate=$PGO_DIR" cargo build --release

# Step 2: Run representative load
echo "Running profiling workloads..."
./target/release/my-backend --benchmark-mode &
PID=$!
sleep 1
# Execute realistic HTTP/socket benchmark
wrk -t4 -c100 -d30s http://localhost:8080/api/v1/bench
kill -SIGTERM $PID
wait $PID 2>/dev/null || true

# Step 3: Merge profile data
echo "Merging profile counters..."
llvm-profdata merge -o "$PGO_DIR/merged.profdata" "$PGO_DIR"

# Step 4: Rebuild fully optimized binary
echo "Compiling final PGO-optimized binary..."
RUSTFLAGS="-Cprofile-use=$PGO_DIR/merged.profdata -Cllvm-args=-pgo-warn-missing-function" \
    cargo build --release

echo "Success! PGO binary located at target/release/my-backend"
```

---

## 2. Hasher Selection Matrix: SipHash vs `ahash` vs `FxHashMap`

`std::collections::HashMap` uses `SipHash-1-3` by default to defend against HashDoS algorithmic attacks. However, SipHash is 2x to 4x slower than non-cryptographic hashers. Choosing the correct hasher depends on key origin:

| Hasher | Crate | DoS-Resistant | Relative Speed | Ideal Use Case |
| :--- | :--- | :--- | :--- | :--- |
| **SipHash-1-3** | `std::collections` | Yes (Cryptographic) | Baseline (1x) | Untrusted external user input (headers, query params) |
| **`AHashMap`** | `ahash` | Yes (Per-process seed) | **2.5x faster** | General-purpose high-throughput replacement |
| **`FxHashMap`** | `rustc-hash` | No (Deterministic) | **4x faster** | Internal compiler IDs, integer keys, pointer handles |

```rust
use ahash::AHashMap;
use rustc_hash::FxHashMap;

// High-speed general-purpose map (safe for general web traffic)
pub type FastMap<K, V> = AHashMap<K, V>;

// Ultra-fast map for internal u32/u64 IDs (trusted keys only)
pub type InternalIdMap<V> = FxHashMap<u64, V>;
```

---

## 3. Post-Link Binary Optimization with LLVM BOLT

For maximum instruction cache locality, combine PGO with LLVM BOLT (Binary Optimization and Layout Tool):

```bash
# Reorder basic blocks and layout functions by hotness
llvm-bolt target/release/my-backend \
    -o target/release/my-backend.bolt \
    -data=perf.data \
    -reorder-blocks=ext-tsp \
    -reorder-functions=hfsort
```

---

## 4. Production Checklist

- [ ] Automated PGO script integrated into release CI pipeline.
- [ ] Replaced internal integer `HashMap` instances with `FxHashMap` or `AHashMap`.
- [ ] Preserved default `SipHash` on maps directly exposed to unbounded external user keys.
- [ ] Profile workloads match realistic traffic shapes rather than artificial micro-benchmarks.
