# gRPC, Tonic, and HTTP/2 Memory Tuning

gRPC services implemented with Tonic run on top of HTTP/2 (`hyper`). Under high traffic bursts or large streaming transfers, default HTTP/2 flow-control window sizes and buffering mechanisms cause memory to balloon across open streams.

---

## 1. HTTP/2 Flow-Control Window Sizing

### The Problem
HTTP/2 uses stream-level and connection-level flow control windows. The default window sizes can buffer megabytes of frames per active stream. At 10,000 concurrent RPC streams, unmetered window sizes can consume gigabytes of memory buffering frames before the server application has consumed them.

### The Solution: Tuned Flow Control
Bound initial stream and connection windows on `Server::builder()`:

```rust
use tonic::transport::Server;
use std::time::Duration;

pub async fn run_grpc_server(addr: std::net::SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
    Server::builder()
        // Limit individual stream window to 64KB (prevents per-stream heap bloat)
        .initial_stream_window_size(Some(64 * 1024))
        // Limit connection-wide window to 512KB across all concurrent streams
        .initial_connection_window_size(Some(512 * 1024))
        // Limit maximum frame size to 16KB (HTTP/2 minimum default)
        .max_frame_size(Some(16 * 1024))
        // Limit concurrent streams per HTTP/2 connection
        .max_concurrent_streams(Some(1024))
        // HTTP/2 Keepalive to reclaim dead connections
        .http2_keepalive_interval(Some(Duration::from_secs(30)))
        .http2_keepalive_timeout(Some(Duration::from_secs(5)))
        // Add services...
        .serve(addr)
        .await?;

    Ok(())
}
```

---

## 2. Zero-Copy Byte Streaming in gRPC

Avoid materializing intermediate `Vec<u8>` arrays during streaming RPCs. Use `prost::bytes::Bytes` which uses reference-counted slice slicing (`.slice()`) without copying underlying buffers:

```rust
use tonic::{Request, Response, Status};
use prost::bytes::Bytes;
use tokio_stream::Stream;
use tokio_stream::wrappers::ReceiverStream;
use tokio::sync::mpsc;

pub struct StreamService;

#[tonic::async_trait]
impl my_proto::stream_service_server::StreamService for StreamService {
    type StreamDataStream = ReceiverStream<Result<my_proto::DataChunk, Status>>;

    async fn stream_data(
        &self,
        request: Request<my_proto::StreamRequest>,
    ) -> Result<Response<Self::StreamDataStream>, Status> {
        // Strictly bounded channel buffer (prevents producer from running away)
        let (tx, rx) = mpsc::channel(32);

        tokio::spawn(async move {
            let chunk_data: Bytes = Bytes::from_static(b"OPTIMIZED_ZERO_COPY_PAYLOAD");

            for _ in 0..1000 {
                let msg = my_proto::DataChunk {
                    // Slices Bytes without memory copy or allocation
                    payload: chunk_data.slice(..),
                };

                // Propagate backpressure: await yields if client is slow reading
                if tx.send(Ok(msg)).await.is_err() {
                    break; // Client disconnected or cancelled
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}
```

---

## 3. Tonic Client Channel Pooling & Bounded Buffers

Do not create a new `Channel` per outbound gRPC call. Use a shared, pooled channel with configured concurrency and window bounds:

```rust
use tonic::transport::{Channel, Endpoint};
use std::time::Duration;

pub async fn create_optimized_channel(dst: &'static str) -> Result<Channel, tonic::transport::Error> {
    Endpoint::from_static(dst)
        .timeout(Duration::from_secs(5))
        .connect_timeout(Duration::from_secs(3))
        .initial_stream_window_size(Some(64 * 1024))
        .initial_connection_window_size(Some(512 * 1024))
        .tcp_nodelay(true)
        .connect()
        .await
}
```

---

## 4. Production Checklist

- [ ] Enforced `initial_stream_window_size(64 * 1024)` on Tonic Server.
- [ ] Enforced `initial_connection_window_size(512 * 1024)` on Tonic Server.
- [ ] Bounded streaming MPSC channels to low numbers (e.g. 16–64) for backpressure propagation.
- [ ] Protobuf messages use `bytes::Bytes` rather than `Vec<u8>` for binary data payloads.
- [ ] Tonic client channels are created as shared singletons inside `Arc<AppState>`.
