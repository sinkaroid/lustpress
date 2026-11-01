# Rust Redis Connection & Pooling Pitfalls: The 3-Hour Debugging Battle Scar

Connecting to Redis in asynchronous Rust (via `redis` and `deadpool-redis`) operates on lazy initialization semantics that differ significantly from synchronous drivers. Engineers frequently encounter silent connection failures, unverified lazy pools, feature flag mismatches, and credential parsing subtleties that surface only under production load.

---

## 1. The 5 Traps That Break Redis in Rust

### Trap 1: The "Lazy Connection" Illusion
- Calling `redis::Client::open(url)` or `deadpool_redis::Config::create_pool(...)` **does not connect to Redis**. It only parses the string syntax!
- The server starts up, logs `"Redis client initialized"`, and appears healthy. Then, the first user request calls `pool.get().await`, times out, or fails silently.
- **The Fix**: Always run an active asynchronous `PING` check immediately during startup inside a spawned task.

### Trap 2: The `tokio-comp` Feature Flag Omission
- If you add `redis = "0.27"` without async runtime features, calling async commands fails to compile, or deadpool deadlocks Tokio threads.
- **The Fix**: Strictly require `tokio-comp`:
  ```toml
  redis = { version = "0.27", features = ["tokio-comp"] }
  deadpool-redis = "0.18" # or "0.23"
  ```

### Trap 3: Auth URL Formatting (`redis://` vs `rediss://`)
- Redis with password but no username requires an empty user before the colon:
  `redis://:yourpassword@127.0.0.1:6379`
- If you write `redis://yourpassword@127.0.0.1:6379`, Redis interprets `yourpassword` as the username with NO password, causing `NOAUTH Authentication required`.
- Redis with ACL username:
  `redis://username:yourpassword@127.0.0.1:6379`
- TLS-enabled Redis (AWS ElastiCache / Redis Cloud) **requires** `rediss://` and the `tls-rustls` feature in `redis`.

### Trap 4: Plaintext Password Leakage in Production Logs
- Printing the raw `REDIS_URL` in logs exposes production passwords.
- **The Fix**: Implement a deterministic URL masker that hides passwords while keeping scheme, host, and port visible.

### Trap 5: Hard Crashing on Redis Outages
- If Redis is down, an application should **degrade gracefully**, not crash or refuse all traffic.
- **The Fix**: Hybrid Cache Backend. Automatically fall back to bounded in-memory `moka` cache if Redis fails to connect.

---

## 2. The Production-Tested Hybrid Cache Pattern (`jandapress`)

```rust
use deadpool_redis::{Config as RedisConfig, Pool, Runtime};
use moka::future::Cache as MokaCache;
use std::time::Duration;
use tracing::{info, warn};

#[derive(Clone)]
pub enum CacheBackend {
    InMemory(MokaCache<String, Vec<u8>>),
    Redis(Pool),
}

#[derive(Clone)]
pub struct CacheManager {
    backend: CacheBackend,
    ttl: Duration,
}

/// Safely mask passwords in Redis connection strings before logging
pub fn mask_redis_url(url: &str) -> String {
    if let Some(start_idx) = url.find("://") {
        let scheme = &url[..start_idx + 3];
        let rest = &url[start_idx + 3..];
        if let Some(at_idx) = rest.find('@') {
            let auth_part = &rest[..at_idx];
            let host_part = &rest[at_idx..]; // "@host:port..."
            if let Some(colon_idx) = auth_part.find(':') {
                let username = &auth_part[..colon_idx];
                return format!("{}{}:******{}", scheme, username, host_part);
            } else {
                return format!("{}******{}", scheme, host_part);
            }
        }
    }
    url.to_string()
}

impl CacheManager {
    pub fn new(redis_url: Option<&str>, expire_hours: u64) -> Self {
        let ttl = Duration::from_secs(expire_hours * 3600);

        let backend = if let Some(url) = redis_url {
            info!("Initializing Redis cache backend with url: {}", mask_redis_url(url));
            let mut cfg = RedisConfig::default();
            cfg.url = Some(url.to_string());
            cfg.connection = None;

            // Runtime::Tokio1 is required for Tokio integration
            match cfg.create_pool(Some(Runtime::Tokio1)) {
                Ok(pool) => CacheBackend::Redis(pool),
                Err(err) => {
                    warn!("Failed to create Redis pool: {:?}. Falling back to bounded In-Memory cache.", err);
                    Self::create_in_memory_backend(ttl)
                }
            }
        } else {
            info!("No Redis URL provided. Initializing bounded In-Memory cache backend.");
            Self::create_in_memory_backend(ttl)
        };

        Self { backend, ttl }
    }

    fn create_in_memory_backend(ttl: Duration) -> CacheBackend {
        // Strictly bounded to 1,000 entries to prevent memory growth leaks
        let cache = MokaCache::builder()
            .max_capacity(1000)
            .time_to_live(ttl)
            .build();
        CacheBackend::InMemory(cache)
    }

    /// Actively verify Redis connection with an explicit PING command
    pub async fn test_connection(&self) -> Result<(), String> {
        match &self.backend {
            CacheBackend::InMemory(_) => Ok(()),
            CacheBackend::Redis(pool) => {
                let mut conn = pool.get().await
                    .map_err(|e| format!("Failed to acquire connection from pool: {:?}", e))?;
                
                let pong: String = redis::cmd("PING")
                    .query_async(&mut conn)
                    .await
                    .map_err(|e| format!("PING command failed: {:?}", e))?;

                if pong == "PONG" {
                    Ok(())
                } else {
                    Err(format!("Unexpected PING response: {}", pong))
                }
            }
        }
    }

    pub async fn get(&self, key: &str) -> Option<Vec<u8>> {
        match &self.backend {
            CacheBackend::InMemory(cache) => cache.get(key).await,
            CacheBackend::Redis(pool) => {
                let mut conn = match pool.get().await {
                    Ok(c) => c,
                    Err(err) => {
                        warn!("Failed to get Redis connection from pool: {:?}", err);
                        return None;
                    }
                };

                let cmd = redis::cmd("GET").arg(key).query_async::<Option<Vec<u8>>>(&mut conn).await;
                match cmd {
                    Ok(val) => val,
                    Err(err) => {
                        warn!("Redis GET error: {:?}", err);
                        None
                    }
                }
            }
        }
    }

    pub async fn set(&self, key: &str, value: &[u8]) {
        match &self.backend {
            CacheBackend::InMemory(cache) => {
                cache.insert(key.to_string(), value.to_vec()).await;
            }
            CacheBackend::Redis(pool) => {
                let mut conn = match pool.get().await {
                    Ok(c) => c,
                    Err(err) => {
                        warn!("Failed to get Redis connection from pool: {:?}", err);
                        return;
                    }
                };

                let ttl_secs = self.ttl.as_secs();
                let res: Result<(), redis::RedisError> = redis::cmd("SETEX")
                    .arg(key)
                    .arg(ttl_secs)
                    .arg(value)
                    .query_async(&mut conn)
                    .await;

                if let Err(err) = res {
                    warn!("Redis SETEX error: {:?}", err);
                }
            }
        }
    }
}
```

---

## 3. Asynchronous Non-Blocking Verification in `main.rs`

Never block server bootup on Redis. Verify connectivity asynchronously in the background so the HTTP server can start serving immediately while logging clear status:

```rust
if config.redis_url.is_some() {
    let cache_clone = state.cache.clone();
    tokio::spawn(async move {
        match cache_clone.test_connection().await {
            Ok(_) => tracing::info!("Redis cache connection verified successfully! Backend: Connected"),
            Err(err) => tracing::warn!("Redis verification failed: {}. Falling back to In-Memory cache.", err),
        }
    });
}
```

---

## 4. Self-Hosted Redis Dockerfile for CI / Local Testing

When testing Redis integration locally or in GitHub Actions, avoid unauthenticated default Redis containers. Use a pinned lightweight Redis image with explicit auth:

```dockerfile
FROM redis:8-alpine AS redis

LABEL org.opencontainers.image.title="app-redis"
LABEL org.opencontainers.image.description="Dedicated Redis cache backend"

EXPOSE 6379

# Enforce explicit auth password matching app connection string
CMD ["redis-server", "--requirepass", "app_secret_password"]
```

**Matching `.env` String**:
```env
REDIS_URL=redis://:app_secret_password@127.0.0.1:6379
```
