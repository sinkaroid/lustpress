# Message Queue Consumer Backpressure and Stream Ingestion

In event-driven architectures (Kafka, RabbitMQ, Redis Streams, AWS SQS), consumer workers frequently pull messages faster than the processing pipeline (database persistence, external APIs) can process them. Without hard-bounded prefetch limits and backpressure channels, message buffers grow unboundedly until the Linux kernel terminates the container via OOM.

---

## 1. The Consumer Prefetch OOM Trap

By default, message brokers try to maximize throughput by streaming hundreds or thousands of unacknowledged messages into the consumer application's memory buffers:

- **RabbitMQ**: Without `basic_qos(prefetch_count)`, RabbitMQ delivers all queue messages into client RAM.
- **Kafka**: Without tuning `queued.max.messages.kbytes` or `fetch.max.bytes`, `librdkafka` buffers tens of thousands of messages in memory.
- **Tokio Worker Pool**: Spawning `tokio::spawn` per incoming message creates an unbounded task queue.

---

## 2. Decoupled Pipeline with Bounded Channels

Decouple the message ingestion loop from the worker execution pool using a strictly bounded `tokio::sync::mpsc::channel`. When workers fall behind, the channel fills up and the ingestion loop naturally pauses at `.send().await`, preventing memory inflation:

```rust
use tokio::sync::mpsc;
use std::sync::Arc;

pub struct QueueMessage {
    pub id: u64,
    pub payload: bytes::Bytes,
}

pub async fn run_bounded_consumer_pipeline<F, Fut>(
    concurrency: usize,
    process_fn: F,
) where
    F: Fn(QueueMessage) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    // Bound the internal channel to 2x worker concurrency
    let (tx, mut rx) = mpsc::channel::<QueueMessage>(concurrency * 2);
    let process_fn = Arc::new(process_fn);

    // Spawn fixed pool of async workers
    for _ in 0..concurrency {
        let mut rx = rx.clone(); // Or single worker draining rx
        let process_fn = Arc::clone(&process_fn);

        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                process_fn(msg).await;
            }
        });
    }

    // Ingestion loop: pauses automatically when channel is saturated
    loop {
        let raw_msg = fetch_next_broker_message().await;
        
        // BACKPRESSURE APPLIED HERE:
        // If all workers are busy and channel capacity is reached,
        // this .send().await suspends execution and halts broker pulling.
        if tx.send(raw_msg).await.is_err() {
            break; // Pipeline shut down
        }
    }
}

async fn fetch_next_broker_message() -> QueueMessage {
    // Fetches from Kafka / RabbitMQ / SQS
    QueueMessage { id: 1, payload: bytes::Bytes::from_static(b"data") }
}
```

---

## 3. RabbitMQ Prefetch Tuning (`lapin`)

In RabbitMQ AMQP consumers using `lapin`, always set `basic_qos` before consuming:

```rust
use lapin::{options::BasicQosOptions, Channel};

pub async fn configure_rabbitmq_channel(channel: &Channel, worker_concurrency: u16) -> Result<(), lapin::Error> {
    // Only prefetch enough messages to keep workers busy
    channel.basic_qos(
        worker_concurrency * 2, // prefetch_count
        BasicQosOptions::default(),
    ).await?;

    Ok(())
}
```

---

## 4. Kafka (`rdkafka`) Memory Containment

Configure `librdkafka` client properties to bound internal queue buffers:

```rust
use rdkafka::config::ClientConfig;

pub fn create_kafka_consumer_config() -> ClientConfig {
    let mut config = ClientConfig::new();
    config
        .set("bootstrap.servers", "localhost:9092")
        .set("group.id", "my-consumer-group")
        .set("enable.auto.commit", "false")
        // Limit internal queue buffer size to 16MB (default can be 1GB+)
        .set("queued.max.messages.kbytes", "16384")
        // Limit max fetch size per partition to 1MB
        .set("fetch.message.max.bytes", "1048576")
        .set("fetch.max.bytes", "10485760"); // Max 10MB per fetch request

    config
}
```

---

## 5. Production Checklist

- [ ] Bounded internal Tokio MPSC channels between consumer and worker pool.
- [ ] Set `basic_qos` prefetch limit on RabbitMQ channels.
- [ ] Configured `queued.max.messages.kbytes` on Kafka consumers to prevent GBs of buffered records.
- [ ] Offloaded heavy CPU decoding/parsing to `rayon` or `spawn_blocking` to avoid stalling the message ACK loop.
- [ ] Ensured offset commits only happen after processing has completed, preventing silent message loss during task drainage.
