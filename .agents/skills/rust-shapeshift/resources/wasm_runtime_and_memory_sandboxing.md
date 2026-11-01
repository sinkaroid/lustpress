# WebAssembly Runtime and Memory Sandboxing

Embedding WebAssembly (Wasm) engines (`wasmtime`) enables safe, multi-tenant user extensibility and plugin systems. However, untrusted guest Wasm modules can allocate linear memory unboundedly until host RAM is exhausted, or enter infinite CPU loops that starve Tokio worker threads.

---

## 1. Hard Linear Memory Ceilings with `ResourceLimiter`

Prevent guest modules from growing their linear memory beyond a strict memory budget (e.g., max 16MB) by implementing `wasmtime::ResourceLimiter`:

```rust
use wasmtime::*;

pub struct PluginState {
    pub max_memory_bytes: usize,
    pub current_memory_bytes: usize,
}

impl ResourceLimiter for PluginState {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> Result<bool, wasmtime::Error> {
        if desired > self.max_memory_bytes {
            // Rejects linear memory allocation: protects host from OOM
            Ok(false)
        } else {
            self.current_memory_bytes = desired;
            Ok(true)
        }
    }

    fn table_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> Result<bool, wasmtime::Error> {
        Ok(desired <= 10_000) // Bound table elements to 10k
    }
}
```

---

## 2. Epoch-Based CPU Interruption and Fuel Metering

Infinite loops in guest Wasm code block the host thread. Use epoch interruption or fuel metering to terminate rogue execution:

```rust
use wasmtime::*;
use std::time::Duration;

pub fn configure_safe_wasm_engine() -> Result<Engine, wasmtime::Error> {
    let mut config = Config::new();
    
    // Enable epoch interruption to interrupt infinite guest loops
    config.epoch_interruption(true);
    
    // Disable multi-memory to contain memory footprint
    config.wasm_multi_memory(false);
    
    Engine::new(&config)
}

pub async fn run_plugin_with_timeout<T>(
    engine: &Engine,
    store: &mut Store<T>,
    timeout: Duration,
) {
    let engine_clone = engine.clone();
    
    // Spawn watchdog to increment epoch when timeout expires
    let watchdog = tokio::spawn(async move {
        tokio::time::sleep(timeout).await;
        engine_clone.increment_epoch();
    });

    // Store configured to trap when epoch expires
    store.set_epoch_deadline(1);

    // Run guest execution...
    watchdog.abort();
}
```

---

## 3. Zero-Copy Guest-Host Memory Slices

Avoid copying data when passing request payloads into the guest Wasm module. Access the guest's linear memory directly as a borrowed byte slice:

```rust
pub fn inspect_guest_memory(
    memory: &Memory,
    store: &impl AsContext,
    ptr: usize,
    len: usize,
) -> Result<&[u8], &'static str> {
    let raw_slice = memory.data(store);
    raw_slice.get(ptr..ptr + len).ok_or("Out of bounds memory access")
}
```

---

## 4. Production Checklist

- [ ] Attached `ResourceLimiter` enforcing hard linear memory ceiling (e.g. 16MB–64MB per instance).
- [ ] Enabled `epoch_interruption(true)` or fuel metering on the `Engine`.
- [ ] Watchdog task terminates runaway execution after deadline.
- [ ] Zero-copy host reads directly access `memory.data()` bounds without vector cloning.
