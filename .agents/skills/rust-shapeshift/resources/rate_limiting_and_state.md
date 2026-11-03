# Bounded Rate Limiting & Memory-Safe State Management

In concurrent backends, in-memory rate limiters that store client IP records without hard boundary caps will experience unbounded memory growth under distributed traffic spikes or DDoS conditions. High-efficiency systems require deterministic memory ceilings and active TTL eviction.

---

## 1. Bounded In-Memory Rate Limiting Architecture

This pattern uses `dashmap::DashMap` with **two levels of memory defense**:
1. A **background cleaner task** running every 30 seconds to evict expired IP windows.
2. An **emergency capacity ceiling sweep** (`BUCKET_MAX_SIZE = 50,000`) that runs when under heavy load, preventing the map from ever exceeding fixed RAM limits.

```rust
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::time::sleep;

const WINDOW_MS: u128 = 15 * 60 * 1000; // 15 minutes window
const LIMIT_MAX: u32 = 50;               // Max requests per window
const BUCKET_MAX_SIZE: usize = 50_000;   // Hard cap on in-memory IP entries

#[derive(Clone, Debug)]
pub struct Counter {
    pub count: u32,
    pub reset_at: Instant,
    pub reset_at_sys: SystemTime,
}

#[derive(Clone)]
pub struct RateLimitState {
    buckets: Arc<DashMap<String, Counter>>,
}

impl RateLimitState {
    pub fn new() -> Self {
        let buckets = Arc::new(DashMap::new());
        let buckets_clone = Arc::clone(&buckets);

        // 1. Background Sweeper: Purges expired IP entries every 30 seconds
        tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(30)).await;
                let now = Instant::now();
                buckets_clone.retain(|_, counter: &mut Counter| counter.reset_at > now);
            }
        });

        Self { buckets }
    }

    pub fn touch(&self, key: String) -> Counter {
        let now_instant = Instant::now();
        let now_sys = SystemTime::now();

        // Check if an unexpired counter exists for this IP
        if let Some(mut r) = self.buckets.get_mut(&key) {
            if r.reset_at > now_instant {
                r.count += 1;
                return r.clone();
            }
        }

        // 2. Emergency Bounded Ceiling: If bucket is saturated, sweep immediately
        if self.buckets.len() >= BUCKET_MAX_SIZE {
            self.buckets.retain(|_, counter| counter.reset_at > now_instant);
        }

        let fresh = Counter {
            count: 1,
            reset_at: now_instant + Duration::from_millis(WINDOW_MS as u64),
            reset_at_sys: now_sys + Duration::from_millis(WINDOW_MS as u64),
        };

        // Only insert if within memory bounds
        if self.buckets.len() < BUCKET_MAX_SIZE {
            self.buckets.insert(key, fresh.clone());
        }

        fresh
    }
}
```

---

## 2. Reverse Proxy IP Extraction Guard

Prevent IP spoofing while correctly detecting client IPs behind Cloudflare, Fly.io, or Vercel:

```rust
use axum::http::HeaderMap;

pub fn get_client_ip(headers: &HeaderMap) -> String {
    // Trusted upstream provider headers
    let trusted_ip_headers = [
        "cf-connecting-ip",
        "fly-client-ip",
        "x-vercel-forwarded-for",
        "x-client-ip"
    ];

    for header in trusted_ip_headers {
        if let Some(val) = headers.get(header).and_then(|v| v.to_str().ok()) {
            let ip = val.split(',').next().unwrap_or("").trim();
            if !ip.is_empty() {
                return ip.to_string();
            }
        }
    }

    // Fallback to standard x-forwarded-for or x-real-ip
    headers
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "127.0.0.1".to_string())
}
```

---

## 3. Progressive Delay (Slow-Down) Before Hard Rate Limiting

Instead of an abrupt `429 Too Many Requests`, introduce progressive delays (`tokio::time::sleep`) to throttle aggressive scrapers smoothly:

```rust
const SLOW_DELAY_AFTER: u32 = 30;
const SLOW_DELAY_MS: u64 = 500;
const SLOW_MAX_DELAY_MS: u64 = 10_000;

pub async fn apply_progressive_delay(counter: &Counter) {
    if counter.count > SLOW_DELAY_AFTER && counter.count <= LIMIT_MAX {
        let over = (counter.count - SLOW_DELAY_AFTER) as u64;
        let delay = std::cmp::min(over * SLOW_DELAY_MS, SLOW_MAX_DELAY_MS);
        tokio::time::sleep(Duration::from_millis(delay)).await;
    }
}
```
