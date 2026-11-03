# Zero-Copy Serde & Streaming Serialization

In web APIs, microservices, and scraper pipelines, JSON serialization and deserialization is the single largest consumer of CPU cycles and allocator bandwidth. The default pattern of deserializing into owned `String` fields forces thousands of tiny heap allocations per request, completely defeating the zero-cost promise of Rust.

This reference guide documents how to eliminate allocation churn during serialization and achieve zero-copy JSON parsing.

---

## 1. Zero-Copy Deserialization with `#[serde(borrow)]`

When deserializing a payload that is stored in an in-memory buffer (e.g., `bytes::Bytes`), there is no need to allocate new `String` objects on the heap. You can borrow string slices (`&'a str`) directly from the input buffer:

```toml
# Cargo.toml
[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
bytes = "1.7"
```

```rust
use serde::Deserialize;

// ❌ WRONG: Allocates 4 separate heap strings per record
#[derive(Deserialize)]
pub struct NaiveRecord {
    pub id: String,
    pub title: String,
    pub category: String,
    pub author: String,
}

// ✅ CORRECT: ZERO heap allocations (borrows directly from the raw byte buffer)
#[derive(Deserialize)]
pub struct ZeroCopyRecord<'a> {
    #[serde(borrow)]
    pub id: &'a str,
    #[serde(borrow)]
    pub title: &'a str,
    #[serde(borrow)]
    pub category: &'a str,
    #[serde(borrow)]
    pub author: &'a str,
}
```

### Consumption Pattern:
```rust
pub fn parse_upstream_payload(raw_bytes: &[u8]) -> Result<ZeroCopyRecord<'_>, AppError> {
    // serde_json::from_slice parses directly from bytes without double UTF-8 allocation
    let record: ZeroCopyRecord = serde_json::from_slice(raw_bytes)
        .map_err(|e| AppError::Validation(e.to_string()))?;
    
    Ok(record)
}
```

> **Rule**: When parsing JSON from bytes, always prefer `serde_json::from_slice` over `std::str::from_utf8` + `serde_json::from_str`. `from_slice` validates UTF-8 and parses JSON in a single unified pass.

---

## 2. Avoiding Dynamic `serde_json::Value`

`serde_json::Value` represents JSON as a dynamic recursive tree of enum nodes. Every key and string value in `serde_json::Value` is an allocated `String`, and every object is a `Map<String, Value>`.

Parsing a 1MB JSON document into `serde_json::Value` consumes **5MB to 8MB of RAM** and triggers tens of thousands of individual heap allocations.

```rust
// ❌ WRONG: Dynamic value causes massive allocator churn
let val: serde_json::Value = serde_json::from_slice(&bytes)?;
let title = val["data"]["item"]["title"].as_str().unwrap_or_default();

// ✅ CORRECT: Strongly typed minimal extraction struct
#[derive(Deserialize)]
struct MinimalItem<'a> {
    #[serde(borrow)]
    title: &'a str,
}

#[derive(Deserialize)]
struct MinimalData<'a> {
    #[serde(borrow)]
    item: MinimalItem<'a>,
}

#[derive(Deserialize)]
struct MinimalWrapper<'a> {
    #[serde(borrow)]
    data: MinimalData<'a>,
}

let wrapper: MinimalWrapper = serde_json::from_slice(&bytes)?;
let title = wrapper.data.item.title; // 0 heap allocations!
```

Serde automatically ignores all other fields in the payload without parsing or allocating them.

---

## 3. Streaming Response Serialization (Zero Intermediate Allocation)

The typical Axum JSON response pattern:
```rust
// ❌ Allocates an intermediate String or Vec<u8> containing the full JSON payload
axum::Json(large_data)
```

For large JSON payloads (e.g. returning 10,000 items), allocating a giant contiguous 10MB `String` stresses the allocator. Instead, stream the serialized chunks directly to the response socket using a reusable `bytes::BytesMut` buffer:

```rust
use axum::{
    body::Body,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use bytes::BytesMut;

pub fn stream_json_response<T: serde::Serialize>(data: &T) -> Result<Response, AppError> {
    let mut buffer = BytesMut::with_capacity(32 * 1024);
    let mut writer = (&mut buffer).writer();

    // Serialize directly into the buffer writer
    serde_json::to_writer(&mut writer, data)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "application/json".parse().unwrap());

    Ok((StatusCode::OK, headers, Body::from(buffer.freeze())).into_response())
}
```

---

## 4. Hardware SIMD Acceleration for Giant Payloads

For systems ingesting massive JSON streams (> 50 MB/s), standard byte-by-byte deserializers hit CPU memory bus bottlenecks. Use **`simd-json`** to utilize AVX2/NEON vector instructions:

```toml
# Cargo.toml (Optional for heavy batch ingest pipelines)
[dependencies]
simd-json = "0.14"
```

```rust
// Parses inplace using SIMD vector instructions; up to 3x faster on x86_64
pub fn parse_with_simd(mut raw_bytes: Vec<u8>) -> Result<MyStruct, AppError> {
    simd_json::from_slice(&mut raw_bytes).map_err(|e| AppError::Validation(e.to_string()))
}
```

---

## 5. Serde Performance Checklist

1. Are transient deserializers using `#[serde(borrow)]` with `&'a str` instead of `String`?
2. Has `serde_json::Value` been replaced with typed structs that extract only necessary fields?
3. Is `serde_json::from_slice` used directly on `&[u8]` buffers?
4. Are large outbound payloads serialized directly into reusable `BytesMut` writers?
