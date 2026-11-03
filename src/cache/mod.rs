use deadpool_redis::redis::AsyncCommands;
use deadpool_redis::{Config as RedisConfig, Pool as RedisPool, Runtime};
use moka::future::Cache as MokaCache;
use std::time::Duration;
use tracing::{debug, info, warn};

fn sanitize_redis_url(url: &str) -> String {
    if let Ok(mut parsed) = url::Url::parse(url) {
        if parsed.password().is_some() {
            let _ = parsed.set_password(Some("*****"));
        }
        parsed.to_string()
    } else {
        "redis://[redacted]".to_string()
    }
}

#[derive(Clone)]
pub enum CacheBackend {
    InMemory(MokaCache<String, Vec<u8>>),
    Redis(RedisPool),
}

#[derive(Clone)]
pub struct CacheService {
    backend: CacheBackend,
    ttl: Duration,
    prefix: String,
}

impl CacheService {
    pub fn new(max_bytes: u64, ttl_hours: u64, redis_url: Option<&str>) -> Result<Self, String> {
        let ttl = Duration::from_secs(ttl_hours * 3600);

        let backend = if let Some(url) = redis_url {
            info!(
                "Initializing Redis cache backend with url: {}",
                sanitize_redis_url(url)
            );
            let cfg = RedisConfig::from_url(url);
            match cfg.create_pool(Some(Runtime::Tokio1)) {
                Ok(pool) => CacheBackend::Redis(pool),
                Err(err) => {
                    warn!(
                        "Failed to construct Redis pool configuration: {err}. Falling back to in-memory Moka cache."
                    );
                    Self::create_in_memory_backend(max_bytes, ttl)
                }
            }
        } else {
            info!(
                "No REDIS_URL configured. Initializing byte-bounded in-memory Moka cache backend (byte budget: {} bytes, max ~{:.1} MB).",
                max_bytes,
                (max_bytes as f64) / (1024.0 * 1024.0)
            );
            Self::create_in_memory_backend(max_bytes, ttl)
        };

        Ok(Self {
            backend,
            ttl,
            prefix: "lustpress:rs:".to_string(),
        })
    }

    fn create_in_memory_backend(max_bytes: u64, ttl: Duration) -> CacheBackend {
        // Weight in KiB (1 unit = 1024 bytes) to prevent Moka W-TinyLFU FrequencySketch
        // from pre-allocating an 8-16 MB table on startup for a 16M entry count.
        // For 16 MiB budget: max_capacity is 16,384 KiB -> sketch table is only 8 KB.
        let max_kib = max_bytes.div_ceil(1024).max(1);
        let cache = MokaCache::builder()
            .weigher(|k: &String, v: &Vec<u8>| {
                let total_bytes = v.len() + k.len() + 64;
                total_bytes.div_ceil(1024) as u32
            })
            .max_capacity(max_kib)
            .initial_capacity(16)
            .time_to_live(ttl)
            .build();
        CacheBackend::InMemory(cache)
    }

    pub async fn run_pending_tasks(&self) {
        if let CacheBackend::InMemory(cache) = &self.backend {
            cache.run_pending_tasks().await;
        }
    }

    pub async fn test_connection(&self) -> Result<(), String> {
        match &self.backend {
            CacheBackend::InMemory(_) => Ok(()),
            CacheBackend::Redis(pool) => {
                let mut conn = pool
                    .get()
                    .await
                    .map_err(|e| format!("Failed to get Redis connection: {e}"))?;
                let pong: String = deadpool_redis::redis::cmd("PING")
                    .query_async(&mut conn)
                    .await
                    .map_err(|e| format!("PING command failed: {e}"))?;
                if pong == "PONG" {
                    Ok(())
                } else {
                    Err(format!("Unexpected PING response: {pong}"))
                }
            }
        }
    }

    pub async fn get(&self, key: &str) -> Option<Vec<u8>> {
        if key.contains("/random") {
            return None;
        }

        match &self.backend {
            CacheBackend::InMemory(cache) => {
                let bytes = cache.get(key).await;
                if bytes.is_some() {
                    debug!("[CACHE HIT] {key}");
                }
                bytes
            }
            CacheBackend::Redis(pool) => {
                let mut conn = match pool.get().await {
                    Ok(c) => c,
                    Err(err) => {
                        warn!("Failed to get Redis connection from pool: {err}");
                        return None;
                    }
                };

                let redis_key = format!("{}{}", self.prefix, key);
                let result: Result<Option<Vec<u8>>, _> = conn.get(&redis_key).await;
                match result {
                    Ok(bytes) => {
                        if bytes.is_some() {
                            debug!("[CACHE HIT - REDIS] {key}");
                        }
                        bytes
                    }
                    Err(err) => {
                        warn!("Redis GET error: {err}");
                        None
                    }
                }
            }
        }
    }

    pub async fn set(&self, key: &str, value: Vec<u8>) {
        if key.contains("/random") {
            return;
        }

        debug!("[CACHE SET] {key} ({} bytes)", value.len());

        match &self.backend {
            CacheBackend::InMemory(cache) => {
                cache.insert(key.to_string(), value).await;
            }
            CacheBackend::Redis(pool) => {
                let mut conn = match pool.get().await {
                    Ok(c) => c,
                    Err(err) => {
                        warn!("Failed to get Redis connection from pool: {err}");
                        return;
                    }
                };

                let redis_key = format!("{}{}", self.prefix, key);
                let ttl_secs = self.ttl.as_secs();
                let res: Result<(), _> = conn.set_ex(&redis_key, value, ttl_secs).await;
                if let Err(err) = res {
                    warn!("Redis SETEX error: {err}");
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_byte_weighted_cache_eviction() {
        // Budget: 1000 bytes
        let cache = CacheService::new(1000, 1, None).unwrap();

        // Inserting 600 bytes payload
        cache.set("https://example.com/1", vec![1u8; 600]).await;
        assert!(cache.get("https://example.com/1").await.is_some());

        // Inserting another 600 bytes payload (exceeds 1000 bytes)
        cache.set("https://example.com/2", vec![2u8; 600]).await;
        cache.run_pending_tasks().await;

        // Since 600 + 600 + overhead > 1000, one entry must be evicted
        let has_1 = cache.get("https://example.com/1").await.is_some();
        let has_2 = cache.get("https://example.com/2").await.is_some();
        assert!(
            !(has_1 && has_2),
            "Byte weighted cache should evict when budget exceeded"
        );
    }

    #[tokio::test]
    async fn test_random_urls_never_cached() {
        let cache = CacheService::new(10_000, 1, None).unwrap();
        cache.set("https://example.com/random", vec![1, 2, 3]).await;
        assert!(cache.get("https://example.com/random").await.is_none());
    }
}
