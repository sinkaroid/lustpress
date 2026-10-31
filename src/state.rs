use crate::cache::CacheService;
use crate::config::Config;
use crate::fetch::FetchService;
use serde::Serialize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Serialize, Clone)]
pub struct GeoServerInfo {
    pub ip: String,
    pub country: String,
    pub region: String,
    pub city: String,
}

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub cache: Arc<CacheService>,
    pub fetch: Arc<FetchService>,
    geo_cache: Arc<RwLock<Option<(String, Instant)>>>,
}

impl AppState {
    pub async fn new(config: Config) -> Result<Self, String> {
        let cache = CacheService::new(
            config.cache_max_capacity,
            config.expire_cache_hours,
            config.redis_url.as_deref(),
        )?;

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::USER_AGENT,
            reqwest::header::HeaderValue::from_str(&config.user_agent)
                .map_err(|e| e.to_string())?,
        );

        let http_client = reqwest::Client::builder()
            .default_headers(headers)
            .connect_timeout(Duration::from_secs(config.upstream_connect_timeout_secs))
            .timeout(Duration::from_secs(config.upstream_timeout_secs))
            .redirect(reqwest::redirect::Policy::limited(5))
            .pool_max_idle_per_host(2)
            .pool_idle_timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| format!("Failed to initialize reqwest client: {e}"))?;

        let fetch = FetchService::new(http_client, cache.clone(), config.user_agent.clone());

        Ok(Self {
            config: Arc::new(config),
            cache: Arc::new(cache),
            fetch: Arc::new(fetch),
            geo_cache: Arc::new(RwLock::new(None)),
        })
    }

    /// Cache-first Geo IP info (Fixes J1 and N3: 30 min success TTL, fast failure)
    pub async fn get_server_location(&self) -> String {
        {
            let guard = self.geo_cache.read().await;
            if let Some((loc, _)) = guard
                .as_ref()
                .filter(|(_, t)| t.elapsed() < Duration::from_secs(1800))
            {
                return loc.clone();
            }
        }

        let mut guard = self.geo_cache.write().await;
        if let Some((loc, _)) = guard
            .as_ref()
            .filter(|(_, t)| t.elapsed() < Duration::from_secs(1800))
        {
            return loc.clone();
        }

        // Fetch outside lock
        let loc = match self
            .fetch
            .client()
            .get("https://ipwho.is/")
            .timeout(Duration::from_secs(3))
            .send()
            .await
        {
            Ok(resp) => {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    let city = json["city"].as_str().unwrap_or("Unknown");
                    let country = json["country"].as_str().unwrap_or("Unknown");
                    format!("{}, {}", city, country)
                } else {
                    "Unknown, Unknown".to_string()
                }
            }
            Err(_) => "Unknown, Unknown".to_string(),
        };

        *guard = Some((loc.clone(), Instant::now()));
        loc
    }
}
