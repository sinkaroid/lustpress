# Headless-Free Anti-Bot Handling & Single-Flight Cookie Bootstrapping

Offloading anti-bot challenges, age verification gates, or dynamic cookies to headless browser instances (such as Chromium) consumes **300MB to 1GB of RAM per instance**, utterly defeating memory efficiency and low-latency throughput goals. In native high-performance Rust, these challenges are solved directly in code via single-flight synchronization and reverse-engineered cryptographic or mathematical algorithms.

This playbook demonstrates how to eliminate headless browsers entirely using:
1. **Algorithmic Challenge Solvers**: Reverse-engineering the challenge directly in Rust.
2. **Single-Flight Cookie Bootstrapping**: Using `Arc<RwLock<...>>` to ensure only one worker thread authenticates, while all concurrent requests wait and reuse the acquired cookie.
3. **Chunked Early-Break Response Streaming**: Reading only the header script without buffering full pages.
4. **Target Page Challenge Invalidation & Retry**: Automatically re-authenticating if an expired session token is rejected mid-flight.

---

## 1. Single-Flight Cookie Cache with `RwLock`

Avoid the "thundering herd" problem where 100 concurrent inbound requests all trigger redundant authentication requests simultaneously:

```rust
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AuthService {
    client: reqwest::Client,
    // Holds the cached session cookie and the timestamp it was acquired
    session_cookie: Arc<RwLock<Option<(String, Instant)>>>,
}

impl AuthService {
    pub async fn get_valid_cookie(&self) -> Result<String, anyhow::Error> {
        // 1. Fast path: Read lock check (non-blocking for concurrent readers)
        {
            let guard = self.session_cookie.read().await;
            if let Some((cookie, timestamp)) = guard.as_ref() {
                if timestamp.elapsed() < Duration::from_secs(3600) {
                    return Ok(cookie.clone());
                }
            }
        }

        // 2. Slow path: Acquire write lock for single-flight bootstrapping
        let mut guard = self.session_cookie.write().await;
        
        // Double-check: Another thread might have acquired the cookie while we waited for the write lock
        if let Some((cookie, timestamp)) = guard.as_ref() {
            if timestamp.elapsed() < Duration::from_secs(3600) {
                return Ok(cookie.clone());
            }
        }

        tracing::info!("Bootstrapping session cookies via algorithmic challenge solver...");
        let fresh_cookie = self.bootstrap_session_cookies().await?;

        // Cache the newly resolved cookie
        *guard = Some((fresh_cookie.clone(), Instant::now()));

        // Release temporary heap pages
        crate::util::trim_memory();

        Ok(fresh_cookie)
    }
}
```

---

## 2. Chunked Early-Break Streaming for Challenge Tokens

Do not download 2MB of HTML and ads when the challenge token is in the first `<script>` tag inside `<head>`:

```rust
use reqwest::Response;

pub async fn extract_challenge_script(mut res: Response) -> Result<String, anyhow::Error> {
    let mut text = String::new();

    // Stream 8KB chunks
    while let Ok(Some(chunk)) = res.chunk().await {
        if let Ok(s) = std::str::from_utf8(&chunk) {
            text.push_str(s);
        }

        // Break immediately once the target challenge block has arrived
        if text.contains("challengeFunction") && text.contains("</script>") {
            break;
        }

        // Hard cap: Never read more than 64KB for a challenge token
        if text.len() >= 65_536 {
            break;
        }
    }

    // Explicitly drop response to release the TCP socket back to the pool
    drop(res);

    Ok(text)
}
```

---

## 3. Detecting Challenges on Target Pages with Automatic Retry

If upstream invalidates the session cookie unexpectedly, invalidate the lock and perform one transparent retry:

```rust
impl AuthService {
    pub async fn fetch_with_auto_reauth(&self, url: &str) -> Result<Vec<u8>, anyhow::Error> {
        let cookie = self.get_valid_cookie().await?;
        let res = self.client.get(url).header("Cookie", &cookie).send().await?;
        let bytes = res.bytes().await?.to_vec();

        // Check if the upstream returned the challenge page instead of data
        if String::from_utf8_lossy(&bytes).contains("leastFactor") {
            tracing::warn!("Session cookie invalidated by upstream. Re-authenticating...");
            
            // Invalidate current cookie
            {
                let mut guard = self.session_cookie.write().await;
                *guard = None;
            }

            // Re-acquire fresh cookie
            let fresh_cookie = self.get_valid_cookie().await?;

            // Retry request
            let retry_res = self.client.get(url).header("Cookie", fresh_cookie).send().await?;
            let retry_bytes = retry_res.bytes().await?.to_vec();

            crate::util::trim_memory();
            return Ok(retry_bytes);
        }

        crate::util::trim_memory();
        Ok(bytes)
    }
}
```
