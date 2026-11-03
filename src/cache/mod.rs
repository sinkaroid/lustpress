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
    pub fn new(max_capacity: u64, ttl_hours: u64, redis_url: Option<&str>) -> Result<Self, String> {
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
                    Self::create_in_memory_backend(max_capacity, ttl)
                }
            }
        } else {
            info!(
                "No REDIS_URL configured. Initializing bounded in-memory Moka cache backend (max capacity: {max_capacity} entries)."
            );
            Self::create_in_memory_backend(max_capacity, ttl)
        };

        Ok(Self {
            backend,
            ttl,
            prefix: "lustpress:rs:".to_string(),
        })
    }

    fn create_in_memory_backend(max_capacity: u64, ttl: Duration) -> CacheBackend {
        // Bounded to 1000 entries max to prevent TinyLFU sketch pre-allocation bloat (matching jandapress)
        let cache = MokaCache::builder()
            .max_capacity(max_capacity)
            .time_to_live(ttl)
            .build();
        CacheBackend::InMemory(cache)
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
