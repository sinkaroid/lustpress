# Streaming File I/O, Archival, and Compression

Loading entire files or large payloads into memory (`Vec<u8>`) during uploads, downloads, or report generation (CSV, JSON, Parquet) creates severe RSS spikes. Realizing extreme memory efficiency requires zero-copy streaming through chunked asynchronous buffers and streaming encoders.

---

## 1. The In-Memory File Buffer Trap

### The Anti-Pattern
```rust
// ANTI-PATTERN: Loads entire 500MB file into heap memory
pub async fn download_and_compress(file_path: &str) -> Result<Vec<u8>, std::io::Error> {
    let uncompressed = tokio::fs::read(file_path).await?; // 500MB RAM allocation!
    let compressed = zstd::encode_all(&uncompressed[..], 3)?; // Another 100MB+ RAM allocation!
    Ok(compressed)
}
```

---

## 2. Streaming Compression with `async-compression`

Pipe data chunk by chunk through `async-compression` using bounded chunk sizes:

```rust
use async_compression::tokio::write::ZstdEncoder;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub async fn stream_compress_file(
    src_path: &str,
    dst_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut reader = tokio::fs::File::open(src_path).await?;
    let writer = tokio::fs::File::create(dst_path).await?;

    // Wrap async writer in streaming Zstd encoder (level 3)
    let mut encoder = ZstdEncoder::new(writer);

    // Fixed 64KB stack/heap chunk buffer (constant memory usage)
    let mut buffer = [0u8; 64 * 1024];

    loop {
        let bytes_read = reader.read(&mut buffer).await?;
        if bytes_read == 0 {
            break; // EOF
        }
        encoder.write_all(&buffer[..bytes_read]).await?;
    }

    encoder.shutdown().await?;
    Ok(())
}
```

---

## 3. Streaming HTTP Responses in Axum

Stream large CSV exports or binary payloads directly from database/disk to the HTTP client using `axum::body::Body::from_stream`:

```rust
use axum::{
    body::Body,
    response::{IntoResponse, Response},
    http::header,
};
use bytes::Bytes;
use tokio_stream::Stream;

pub async fn export_large_csv_stream() -> Response {
    let stream = async_stream::stream! {
        // Yield CSV headers
        yield Ok::<Bytes, std::io::Error>(Bytes::from_static(b"id,username,email\n"));

        // Yield rows chunk-by-chunk with flat memory
        for i in 0..100_000 {
            let row = format!("{i},user_{i},user_{i}@example.com\n");
            yield Ok(Bytes::from(row));
        }
    };

    Response::builder()
        .header(header::CONTENT_TYPE, "text/csv")
        .header(header::CONTENT_DISPOSITION, "attachment; filename=\"export.csv\"")
        .body(Body::from_stream(stream))
        .unwrap()
}
```

---

## 4. Production Checklist

- [ ] Replaced all instances of `tokio::fs::read` on large files with `tokio::fs::File` and fixed chunk buffers (`64KB`).
- [ ] Switched in-memory compression (`zstd::encode_all`) to streaming encoders (`ZstdEncoder` / `GzipEncoder`).
- [ ] Configured HTTP download/export endpoints to return chunked `Body::from_stream`.
- [ ] Multipart file uploads stream directly to disk or S3 without intermediate RAM accumulation.
