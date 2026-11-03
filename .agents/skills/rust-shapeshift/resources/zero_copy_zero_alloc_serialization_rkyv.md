# Zero-Allocation and Zero-Copy Serialization with rkyv & zerocopy

Standard serialization frameworks (such as `serde_json`, `bincode`, or `rmp-serde`) decode serialized data by traversing buffers, validating individual fields, and allocating concrete Rust structs in stack/heap memory. Even with `#[serde(borrow)]`, pointers must be reconstructed and collection capacities allocated.

In ultra-low-latency gateways, telemetry pipelines, and memory-constrained microservices, deserialization overhead can be eliminated entirely using **zero-copy in-place serialization**:
- **`rkyv`**: Serializes Rust data into a format that can be accessed in-place with **zero decoding time ($0\text{ns}$)** and **zero heap allocation**.
- **`zerocopy`**: Converts raw byte buffers to and from plain-old-data (POD) structs with compile-time layout and alignment guarantees.

---

## 1. Serialization Performance Comparison

| Serialization Framework | Deserialization Speed | Heap Allocations During Read | Schema Flexibility | Zero-Copy Capability |
|---|---|---|---|---|
| **`serde_json`** | Baseline (~50–200 MB/s) | High (Strings, Vecs, Maps) | Dynamic JSON | Slices only with `&'a str` |
| **`bincode`** | Fast (~400–800 MB/s) | Medium (Allocates collections) | Strict Binary | Partial |
| **`rkyv`** | **Instant ($0\text{ns}$ / In-place)** | **0 bytes** | Complex Nested Structs | **100% In-Place Borrowing** |
| **`zerocopy`** | **Instant ($0\text{ns}$ / Transmute)** | **0 bytes** | Flat POD Structs only | **100% Direct Cast** |

---

## 2. In-Place Deserialization with `rkyv`

Add `rkyv` to `Cargo.toml`:
```toml
[dependencies]
rkyv = { version = "0.8", default-features = false, features = ["std", "alloc"] }
```

### Serializing and In-Place Accessing Structs

```rust
use rkyv::{
    access,
    rancor::Error,
    to_bytes,
    Archive,
    Deserialize,
    Serialize,
};

#[derive(Archive, Serialize, Deserialize, Debug, PartialEq)]
#[rkyv(as = Self)]
pub struct PacketTelemetry {
    pub session_id: u64,
    pub timestamp_epoch_ms: u64,
    pub error_flags: u32,
    pub payload_name: String,
    pub sensor_readings: Vec<f32>,
}

pub fn serialize_telemetry_payload(packet: &PacketTelemetry) -> Result<Vec<u8>, Error> {
    // Serializes directly into contiguous byte buffer
    let bytes = to_bytes::<Error>(packet)?;
    Ok(bytes.into_vec())
}

pub fn inspect_telemetry_in_place(buffer: &[u8]) -> Result<(), Error> {
    // ZERO ALLOCATION: Validates boundaries and accesses the archived struct directly
    // in-place inside the raw byte buffer.
    let archived = access::<PacketTelemetry, Error>(buffer)?;

    // Fields can be read directly without unpacking
    assert_eq!(archived.session_id, 1024);
    
    // String and slice data are borrowed directly from buffer pointers:
    let name: &str = &archived.payload_name;
    let readings: &[f32] = &archived.sensor_readings;

    println!("Inspected session {} payload '{}' with {} readings", 
             archived.session_id, name, readings.len());

    Ok(())
}
```

---

## 3. High-Throughput POD Framing with `zerocopy`

For high-density networking protocols (e.g. TCP packet framing, binary IPC headers, or hardware sensor packets), structs with fixed layouts can be safely cast directly from raw bytes using `zerocopy`.

Add `zerocopy` to `Cargo.toml`:
```toml
[dependencies]
zerocopy = { version = "0.8", features = ["derive"] }
```

### Zero-Allocation Header Parsing

```rust
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout, NetworkEndian, U16, U32, U64};

#[derive(FromBytes, IntoBytes, KnownLayout, Immutable, Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct BinaryFrameHeader {
    pub magic: [u8; 4],          // "SHPF"
    pub version: u8,             // Protocol version
    pub flags: u8,               // Control flags
    pub sequence_id: U64<NetworkEndian>,
    pub payload_length: U32<NetworkEndian>,
    pub checksum: U16<NetworkEndian>,
}

impl BinaryFrameHeader {
    pub const MAGIC: [u8; 4] = *b"SHPF";

    #[inline(always)]
    pub fn parse_from_buffer(buffer: &[u8]) -> Option<(&Self, &[u8])> {
        let (header, payload) = Self::ref_from_prefix(buffer).ok()?;
        
        if header.magic != Self::MAGIC {
            return None;
        }

        let expected_len = header.payload_length.get() as usize;
        if payload.len() < expected_len {
            return None; // Incomplete packet
        }

        Some((header, &payload[..expected_len]))
    }
}
```

---

## 4. Architectural Rules for Zero-Copy Data Pipelines

1. **Alignment Verification**:
   - `rkyv` and `zerocopy` require proper memory alignment. When reading byte buffers from network sockets or disk, use aligned buffers (`rkyv::util::AlignedVec`) or enforce `#[repr(C, packed)]` / `#[repr(C, align(N))]`.
2. **Untrusted Input Boundaries**:
   - Always run validation (`rkyv::access` or `zerocopy::FromBytes::ref_from_prefix`) before reading archived structs to ensure buffer bounds and alignment invariants hold. Never use unchecked transmutes.
3. **Cache Storage**:
   - For high-concurrency Redis or RocksDB caches, store byte payloads encoded with `rkyv`. Workers retrieve the bytes and inspect cached structs with **zero deserialization CPU cost and zero heap memory duplication**.
