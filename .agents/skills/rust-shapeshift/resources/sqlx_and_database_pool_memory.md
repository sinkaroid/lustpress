# SQLx and Database Connection Pool Memory Optimization

Database queries and unmanaged connection pools are among the most frequent sources of sudden RSS spikes and out-of-memory (OOM) kills in long-running Rust backend services.

---

## 1. The `.fetch_all()` Heap Explosion Trap

### The Problem
Calling `.fetch_all()` buffers the entire SQL result set into memory before deserializing rows into your target structs. When querying large tables, batch jobs, or audit logs, a single query can pull 50,000 to 500,000 rows, consuming hundreds of megabytes or gigabytes of heap RAM:

```rust
// ANTI-PATTERN: Pulls all 100,000 rows into heap memory at once
pub async fn export_users(pool: &PgPool) -> Result<Vec<User>, sqlx::Error> {
    sqlx::query_as!(User, "SELECT id, email, created_at FROM users")
        .fetch_all(pool) // DANGER: Unbounded heap allocation!
        .await
}
```

### The Solution: Streaming with `.fetch()`
Use `.fetch()` to stream rows one by one asynchronously via `futures_util::StreamExt`. Memory consumption remains completely flat (<5MB) regardless of whether the table contains 1,000 rows or 10,000,000 rows.

```rust
use futures_util::StreamExt;
use sqlx::PgPool;

pub async fn stream_users_to_sink<W>(
    pool: &PgPool,
    mut sink: W,
) -> Result<(), Box<dyn std::error::Error>>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut rows = sqlx::query_as!(User, "SELECT id, email, created_at FROM users")
        .fetch(pool); // Streams row-by-row with minimal buffer

    while let Some(user_res) = rows.next().await {
        let user = user_res?;
        // Process or serialize individual user immediately
        tokio::io::AsyncWriteExt::write_all(
            &mut sink,
            format!("{},{}\n", user.id, user.email).as_bytes(),
        ).await?;
    }

    Ok(())
}
```

---

## 2. Bounded Chunked Batching

When streaming individual rows causes excessive I/O transactions, batch them into strictly bounded chunks using `itertools` or custom slice chunking without reallocating vectors:

```rust
use futures_util::StreamExt;
use sqlx::PgPool;

pub async fn process_in_bounded_batches(
    pool: &PgPool,
    batch_size: usize,
) -> Result<(), sqlx::Error> {
    let mut stream = sqlx::query_as!(User, "SELECT id, email, created_at FROM users")
        .fetch(pool);

    let mut batch = Vec::with_capacity(batch_size);

    while let Some(row) = stream.next().await {
        batch.push(row?);
        if batch.len() >= batch_size {
            flush_batch(&batch).await;
            batch.clear(); // Retains allocated capacity, 0 re-allocations
        }
    }

    if !batch.is_empty() {
        flush_batch(&batch).await;
    }

    Ok(())
}

async fn flush_batch(batch: &[User]) {
    // Process bounded batch in-place
}
```

---

## 3. Database Connection Pool Sizing Formula

### Sizing Myth vs Reality
Developers often assign `max_connections = 50` or `100` per instance. In a Kubernetes deployment with 20 pods, 20 pods × 100 connections = 2,000 connections to PostgreSQL. PostgreSQL forks a backend process for every connection, consuming 10–20MB of server RAM per idle connection and thrashing CPU cache.

### The PostgreSQL Optimal Sizing Formula
```text
max_connections = (cpu_cores * 2) + effective_spindle_count
```
For modern cloud SSD instances:
```text
Pool Size = (Available CPU Cores * 2) + 1
```

For a 4-core container:
- Set `max_connections(10)`
- Set `min_connections(2)`
- Configure aggressive idle reap timeouts:

```rust
use sqlx::postgres::{PgPoolOptions, PgConnectOptions};
use std::time::Duration;

pub async fn create_optimized_pool(database_url: &str) -> Result<sqlx::PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(10) // 10 connections can saturate 10,000 req/s in async Rust
        .min_connections(2)
        .acquire_timeout(Duration::from_secs(3))
        .idle_timeout(Duration::from_secs(60))
        .max_lifetime(Duration::from_secs(1800))
        .connect(database_url)
        .await
}
```

---

## 4. Prepared Statement Cache Containment

By default, SQLx caches prepared statements on each connection. If your application constructs dynamic SQL queries with variable clauses or filters, the statement cache can grow unboundedly, leaking memory over days of runtime.

### Enforce Statement Cache Limits
```rust
use sqlx::postgres::PgConnectOptions;
use std::str::FromStr;

pub fn configure_connect_options(database_url: &str) -> Result<PgConnectOptions, sqlx::Error> {
    let mut opts = PgConnectOptions::from_str(database_url)?;
    
    // Limit prepared statement cache per connection to prevent unbounded growth
    opts = opts.statement_cache_capacity(256);
    
    Ok(opts)
}
```

---

## 5. Production Checklist

- [ ] Replaced all `.fetch_all()` calls on unbounded tables with `.fetch()` streaming.
- [ ] Bounded chunk buffers use `.clear()` instead of re-instantiating `Vec::new()`.
- [ ] Database pool `max_connections` is capped according to core counts (typically 5–15 per pod).
- [ ] Set `acquire_timeout` to fail fast (<3s) rather than letting Tokio worker tasks queue up indefinitely.
- [ ] Hardened `statement_cache_capacity` on dynamic SQL workloads.
