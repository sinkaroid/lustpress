use crate::cache::CacheService;
use crate::error::AppError;
use regex::Regex;
use reqwest::header::{
    ACCEPT, ACCEPT_LANGUAGE, COOKIE, HeaderMap, HeaderValue, REFERER, USER_AGENT,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info};

pub const BROWSER_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

pub fn least_factor(n: i64) -> i64 {
    if n <= 1 {
        return 1;
    }
    if n % 2 == 0 {
        return 2;
    }
    if n % 3 == 0 {
        return 3;
    }
    if n % 5 == 0 {
        return 5;
    }
    let m = (n as f64).sqrt() as i64;
    let mut i = 7;
    while i <= m {
        if n % i == 0 {
            return i;
        }
        if n % (i + 4) == 0 {
            return i + 4;
        }
        if n % (i + 6) == 0 {
            return i + 6;
        }
        if n % (i + 10) == 0 {
            return i + 10;
        }
        if n % (i + 12) == 0 {
            return i + 12;
        }
        if n % (i + 16) == 0 {
            return i + 16;
        }
        if n % (i + 22) == 0 {
            return i + 22;
        }
        if n % (i + 24) == 0 {
            return i + 24;
        }
        i += 30;
    }
    n
}

use std::sync::LazyLock;

static RE_COMMENT: LazyLock<Regex> = LazyLock::new(|| match Regex::new(r"/\*[\s\S]*?\*/") {
    Ok(r) => r,
    Err(e) => panic!("Invalid regex: {e}"),
});
static RE_P: LazyLock<Regex> = LazyLock::new(|| match Regex::new(r"var p=(\d+);") {
    Ok(r) => r,
    Err(e) => panic!("Invalid regex: {e}"),
});
static RE_S: LazyLock<Regex> = LazyLock::new(|| match Regex::new(r"var s=(\d+);") {
    Ok(r) => r,
    Err(e) => panic!("Invalid regex: {e}"),
});
static RE_EXTRA: LazyLock<Regex> = LazyLock::new(|| {
    match Regex::new(r#"document\.cookie="KEY="\+n\+"\*"\+p/n\+":(?:\+s\+:|")(\d+):1"#) {
        Ok(r) => r,
        Err(e) => panic!("Invalid regex: {e}"),
    }
});
static RE_EXTRA2: LazyLock<Regex> = LazyLock::new(|| match Regex::new(r#":(\d+):1;path=/;"#) {
    Ok(r) => r,
    Err(e) => panic!("Invalid regex: {e}"),
});
static RE_GO: LazyLock<Regex> =
    LazyLock::new(
        || match Regex::new(r"function go\(\)\s*\{([\s\S]+?)n=leastFactor\(p\);") {
            Ok(r) => r,
            Err(e) => panic!("Invalid regex: {e}"),
        },
    );
static RE_IFELSE: LazyLock<Regex> = LazyLock::new(|| {
    match Regex::new(r"if\(\(s>>(\d+)\)&1\)p(\+|-)=(\d+)\*(\d+);elsep(\+|-)=(\d+)\*(\d+);") {
        Ok(r) => r,
        Err(e) => panic!("Invalid regex: {e}"),
    }
});
static RE_ADJ: LazyLock<Regex> = LazyLock::new(|| match Regex::new(r"p(\+|-)=(\d+);") {
    Ok(r) => r,
    Err(e) => panic!("Invalid regex: {e}"),
});

pub fn solve_ph_challenge(html: &str) -> Result<String, String> {
    // Only isolate the script tag containing leastFactor (~1-2 KB), avoid cloning the entire 2 MB HTML
    let target_block = if let Some(pos) = html.find("leastFactor") {
        let start = html[..pos]
            .rfind("<script")
            .unwrap_or(pos.saturating_sub(1000));
        let end = html[pos..]
            .find("</script>")
            .map(|p| pos + p + 9)
            .unwrap_or_else(|| (pos + 2000).min(html.len()));
        &html[start..end]
    } else {
        html
    };

    let clean = RE_COMMENT.replace_all(target_block, "");

    let p_cap = RE_P
        .captures(&clean)
        .ok_or("Challenge variable p not found")?;
    let s_cap = RE_S
        .captures(&clean)
        .ok_or("Challenge variable s not found")?;

    let mut p: i64 = p_cap[1]
        .parse()
        .map_err(|e: std::num::ParseIntError| e.to_string())?;
    if !(0..=100_000_000_000).contains(&p) {
        return Err(format!("Challenge parameter p={p} exceeds safety limit"));
    }
    let s: i64 = s_cap[1]
        .parse()
        .map_err(|e: std::num::ParseIntError| e.to_string())?;

    let extra = if let Some(cap) = RE_EXTRA.captures(&clean) {
        cap[1].to_string()
    } else {
        RE_EXTRA2
            .captures(&clean)
            .map(|c| c[1].to_string())
            .unwrap_or_else(|| "1".to_string())
    };

    if let Some(go_cap) = RE_GO.captures(&clean) {
        let body = go_cap[1].replace(|c: char| c.is_whitespace(), "");

        let mut body_without_ifs = body.clone();
        for cap in RE_IFELSE.captures_iter(&body) {
            let shift: u32 = cap[1].parse().unwrap_or(0);
            let cond = ((s >> shift) & 1) != 0;
            if cond {
                let sign = &cap[2];
                let v1: i64 = cap[3].parse().unwrap_or(0);
                let v2: i64 = cap[4].parse().unwrap_or(0);
                let val = v1 * v2;
                if sign == "+" {
                    p += val;
                } else {
                    p -= val;
                }
            } else {
                let sign = &cap[5];
                let v1: i64 = cap[6].parse().unwrap_or(0);
                let v2: i64 = cap[7].parse().unwrap_or(0);
                let val = v1 * v2;
                if sign == "+" {
                    p += val;
                } else {
                    p -= val;
                }
            }
            body_without_ifs = body_without_ifs.replace(&cap[0], "");
        }

        for cap in RE_ADJ.captures_iter(&body_without_ifs) {
            let sign = &cap[1];
            let val: i64 = cap[2].parse().unwrap_or(0);
            if sign == "+" {
                p += val;
            } else {
                p -= val;
            }
        }
    }

    let n = least_factor(p);
    if n == 0 {
        return Err("leastFactor produced 0".into());
    }
    Ok(format!("KEY={}*{}:{}:{}:1", n, p / n, s, extra))
}

#[derive(Clone)]
pub struct FetchService {
    client: reqwest::Client,
    cache: CacheService,
    ph_cookies: Arc<RwLock<Option<(String, Instant)>>>,
    default_ua: String,
}

impl FetchService {
    pub fn new(client: reqwest::Client, cache: CacheService, default_ua: String) -> Self {
        Self {
            client,
            cache,
            ph_cookies: Arc::new(RwLock::new(None)),
            default_ua,
        }
    }

    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    pub async fn get_pornhub_cookies(&self) -> Result<String, AppError> {
        // Read lock check
        {
            let guard = self.ph_cookies.read().await;
            if let Some((cookies, _)) = guard
                .as_ref()
                .filter(|(_, t)| t.elapsed() < Duration::from_secs(3600))
            {
                return Ok(cookies.clone());
            }
        }

        // Write lock for single-flight bootstrap
        let mut guard = self.ph_cookies.write().await;
        if let Some((cookies, _)) = guard
            .as_ref()
            .filter(|(_, t)| t.elapsed() < Duration::from_secs(3600))
        {
            return Ok(cookies.clone());
        }

        info!("Bootstrapping PornHub cookies...");
        let base_url = "https://www.pornhub.com/";
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(BROWSER_UA));
        headers.insert(
            ACCEPT,
            HeaderValue::from_static(
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            ),
        );
        headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));

        let res = self
            .client
            .get(base_url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to connect to PornHub: {e}")))?;

        let mut cookie_parts = Vec::new();
        for cookie_header in res.headers().get_all(reqwest::header::SET_COOKIE) {
            if let Some(part) = cookie_header
                .to_str()
                .ok()
                .and_then(|c| c.split(';').next())
            {
                cookie_parts.push(part.to_string());
            }
        }
        let initial_cookies = cookie_parts.join("; ");
        let mut text = String::new();
        let mut stream_res = res;
        while let Ok(Some(chunk)) = stream_res.chunk().await {
            if let Ok(s) = std::str::from_utf8(&chunk) {
                text.push_str(s);
            }
            if text.contains("leastFactor") && text.contains("</script>") {
                break;
            }
            if text.len() >= 65_536 {
                break;
            }
        }
        drop(stream_res);

        let final_cookies = if text.contains("leastFactor") {
            info!("Solving Pornhub Challenge at index...");
            let challenge_cookie = solve_ph_challenge(&text)
                .map_err(|e| AppError::Internal(format!("Failed to solve PH challenge: {e}")))?;
            format!("{}; {}; age_verified=1", initial_cookies, challenge_cookie)
        } else {
            if initial_cookies.is_empty() {
                "age_verified=1".to_string()
            } else {
                format!("{}; age_verified=1", initial_cookies)
            }
        };

        *guard = Some((final_cookies.clone(), Instant::now()));
        Ok(final_cookies)
    }

    pub async fn fetch_body(&self, url: &str) -> Result<Vec<u8>, AppError> {
        if let Some(cached) = self.cache.get(url).await {
            return Ok(cached);
        }

        let is_pornhub = url.contains("pornhub.com");
        let mut req = self.client.get(url);

        if is_pornhub {
            let cookies = self.get_pornhub_cookies().await?;
            req = req
                .header(USER_AGENT, BROWSER_UA)
                .header(COOKIE, cookies)
                .header(REFERER, "https://www.pornhub.com/")
                .header(
                    ACCEPT,
                    "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
                )
                .header(ACCEPT_LANGUAGE, "en-US,en;q=0.9");
        } else {
            req = req.header(USER_AGENT, &self.default_ua);
        }

        debug!("[FETCH] {url}");
        let res = req
            .send()
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        if !res.status().is_success() {
            return Err(AppError::Upstream {
                status: res.status(),
                url: url.to_string(),
            });
        }

        let bytes = res
            .bytes()
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?
            .to_vec();

        // Check if Pornhub challenge appeared on target video page (zero-allocation byte window check)
        let has_challenge = bytes.windows(11).any(|w| w == b"leastFactor");
        if is_pornhub && has_challenge {
            info!("Pornhub challenge on target page. Re-authenticating...");
            // Invalidate cookies
            {
                let mut guard = self.ph_cookies.write().await;
                *guard = None;
            }
            let new_cookies = self.get_pornhub_cookies().await?;
            let retry_res = self
                .client
                .get(url)
                .header(USER_AGENT, BROWSER_UA)
                .header(COOKIE, new_cookies)
                .header(REFERER, "https://www.pornhub.com/")
                .send()
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;

            let retry_bytes = retry_res
                .bytes()
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?
                .to_vec();
            self.cache.set(url, retry_bytes.clone()).await;
            return Ok(retry_bytes);
        }

        self.cache.set(url, bytes.clone()).await;
        Ok(bytes)
    }
}
