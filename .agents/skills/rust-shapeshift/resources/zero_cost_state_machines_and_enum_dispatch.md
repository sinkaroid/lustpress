# Zero-Cost State Machines and Enum Dispatch Architecture

In object-oriented and dynamically-typed runtimes, polymorphic dispatch and state machines are typically implemented via runtime interfaces, virtual method tables (vtables), or dynamic subclassing.

In Rust, using dynamic trait objects (`Box<dyn Trait>` or `&dyn Trait`):
1. **Requires Heap Allocation**: Storing heterogeneous implementations in collections requires `Box<dyn Trait>`, adding heap allocation and pointer indirection.
2. **Inhibits Compiler Inlining**: Calls through vtables cannot be inlined by LLVM, disabling loop unrolling and dead-code elimination.
3. **Breaks SIMD Vectorization**: The CPU cannot vectorize operations across unpredictable indirect function pointers.

Using **Compile-Time Enum Dispatch** (`enum_dispatch`) and **Type-State Pattern State Machines**, polymorphic behavior achieves **zero heap allocation, zero pointer chasing, and full LLVM inlining**.

---

## 1. Dynamic Dispatch (`dyn Trait`) vs. Static Enum Dispatch

| Architectural Feature | Dynamic Dispatch (`Box<dyn Trait>`) | Static Enum Dispatch (`enum_dispatch`) |
|---|---|---|
| **Memory Allocation** | Heap (`Box<T>`) | **Stack (Inline discriminated union)** |
| **Pointer Indirection** | 2-word fat pointer (data ptr + vtable ptr) | **1 enum discriminant byte + data** |
| **LLVM Inlining** | No (indirect branch) | **Full inlining permitted** |
| **Branch Prediction** | Indirect call misprediction hazard | **Direct jump table / match statement** |
| **SIMD Auto-Vectorization** | Blocked | **Fully enabled** |

---

## 2. Zero-Allocation Polymorphism with `enum_dispatch`

Add `enum_dispatch` to `Cargo.toml`:
```toml
[dependencies]
enum_dispatch = "0.3"
```

### Eliminating Dynamic Vtables

```rust
use enum_dispatch::enum_dispatch;

// Define the common trait
#[enum_dispatch]
pub trait StreamCodec {
    fn encode(&self, payload: &[u8], output: &mut Vec<u8>);
    fn is_compressed(&self) -> bool;
}

// Concrete Implementations
pub struct RawCodec;
impl StreamCodec for RawCodec {
    #[inline(always)]
    fn encode(&self, payload: &[u8], output: &mut Vec<u8>) {
        output.extend_from_slice(payload);
    }
    #[inline(always)]
    fn is_compressed(&self) -> bool { false }
}

pub struct ZstdCodec {
    pub compression_level: i32,
}
impl StreamCodec for ZstdCodec {
    #[inline(always)]
    fn encode(&self, payload: &[u8], output: &mut Vec<u8>) {
        // Zstd streaming compression logic...
        output.extend_from_slice(payload);
    }
    #[inline(always)]
    fn is_compressed(&self) -> bool { true }
}

// Enum Dispatch replaces Box<dyn StreamCodec>
#[enum_dispatch(StreamCodec)]
pub enum PipelineCodec {
    Raw(RawCodec),
    Zstd(ZstdCodec),
}

pub fn execute_pipeline_stream(codecs: &[PipelineCodec], data: &[u8]) -> Vec<u8> {
    let mut buffer = Vec::with_capacity(data.len());
    
    // ZERO HEAP INDIRECTION: Dispatches via direct compiler-generated match.
    // Inlines completely under LTO release builds.
    for codec in codecs {
        codec.encode(data, &mut buffer);
    }
    
    buffer
}
```

---

## 3. The Typestate Pattern: Zero-Cost Lifecycle State Machines

To prevent illegal lifecycle transitions (e.g. attempting to send data over an unauthenticated or disconnected socket), encode states as **Zero-Sized Types (ZSTs)** into the type system:

```rust
use std::marker::PhantomData;

// Lifecycle States (Zero-Sized Types - consume 0 bytes of memory)
pub struct Disconnected;
pub struct Connecting;
pub struct Authenticated;

pub struct SecureGatewayConnection<State> {
    socket_fd: i32,
    session_token: Option<String>,
    _state: PhantomData<State>,
}

impl SecureGatewayConnection<Disconnected> {
    pub fn new(fd: i32) -> Self {
        Self {
            socket_fd: fd,
            session_token: None,
            _state: PhantomData,
        }
    }

    pub fn connect(self) -> SecureGatewayConnection<Connecting> {
        // Socket connection logic...
        SecureGatewayConnection {
            socket_fd: self.socket_fd,
            session_token: None,
            _state: PhantomData,
        }
    }
}

impl SecureGatewayConnection<Connecting> {
    pub fn authenticate(self, token: String) -> Result<SecureGatewayConnection<Authenticated>, Self> {
        if !token.is_empty() {
            Ok(SecureGatewayConnection {
                socket_fd: self.socket_fd,
                session_token: Some(token),
                _state: PhantomData,
            })
        } else {
            Err(self) // Return connection in Connecting state on auth failure
        }
    }
}

impl SecureGatewayConnection<Authenticated> {
    // This method physically CANNOT be called on Disconnected or Connecting states!
    // Enforced at compile-time with 0 runtime checks.
    pub fn send_secure_payload(&self, payload: &[u8]) -> Result<(), std::io::Error> {
        println!("Sending {} bytes on socket {}", payload.len(), self.socket_fd);
        Ok(())
    }
}
```

---

## 4. Architectural Rules for Zero-Cost State Machines

1. **Replace `Box<dyn Trait>` with `enum_dispatch`**:
   - Whenever the set of polymorphic types is known within the crate/service boundary, use `enum_dispatch` or a manual enum match instead of `Box<dyn Trait>`.
2. **Use ZSTs (`PhantomData`) for State Tracking**:
   - Never use runtime flags like `is_authenticated: bool` or `state: u8` when states can be modeled statically via typestates. Compile-time states have **zero runtime memory footprint and zero branch misprediction penalty**.
3. **Static Size Verification**:
   - Verify that enum variants are balanced using `static_assertions::assert_type_size!`. If one variant is significantly larger (>128 bytes), box only that rare variant to keep the main enum discriminant compact on the stack.
