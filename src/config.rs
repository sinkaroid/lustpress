use std::env;
use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub lustpress_graphql: bool,
    pub redis_url: Option<String>,
    pub expire_cache_hours: u64,
    pub user_agent: String,
    pub cache_max_capacity: u64,
    pub cache_max_bytes: u64,
    pub upstream_connect_timeout_secs: u64,
    pub upstream_timeout_secs: u64,
    pub rate_limit_enabled: bool,
    pub trusted_proxy_header: Option<String>,
    pub log_level: String,
    pub log_client_info: bool,
    pub enable_debug_log: bool,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let port = match env::var("PORT") {
            Ok(val) => val.trim().parse::<u16>().map_err(|_| {
                format!(
                    "Invalid PORT: '{}' must be a valid port number (1-65535)",
                    val
                )
            })?,
            Err(_) => 3000,
        };

        let lustpress_graphql = match env::var("LUSTPRESS_GRAPHQL") {
            Ok(val) => val.trim().to_lowercase() == "true",
            Err(_) => false,
        };

        let redis_url = env::var("REDIS_URL")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let expire_cache_hours = match env::var("EXPIRE_CACHE") {
            Ok(val) => val.trim().parse::<u64>().map_err(|_| {
                format!("Invalid EXPIRE_CACHE: '{}' must be a positive integer", val)
            })?,
            Err(_) => 1,
        };

        let user_agent = env::var("USER_AGENT")
            .unwrap_or_else(|_| format!("lustpress/{}", env!("CARGO_PKG_VERSION")));

        let cache_max_bytes = match env::var("CACHE_MAX_BYTES") {
            Ok(val) => val
                .trim()
                .parse::<u64>()
                .map_err(|_| format!("Invalid CACHE_MAX_BYTES: '{}' must be an integer", val))?,
            Err(_) => match env::var("CACHE_MAX_MB") {
                Ok(val) => {
                    let mb = val.trim().parse::<u64>().map_err(|_| {
                        format!("Invalid CACHE_MAX_MB: '{}' must be an integer", val)
                    })?;
                    mb * 1024 * 1024
                }
                Err(_) => 16 * 1024 * 1024, // Bounded to 16 MiB max byte-budget (PRD P1-2 / P1-4, Issue N2)
            },
        };

        let cache_max_capacity =
            match env::var("CACHE_MAX_ENTRIES").or_else(|_| env::var("CACHE_MAX_CAPACITY")) {
                Ok(val) => {
                    let parsed = val.trim().parse::<u64>().map_err(|_| {
                        format!("Invalid CACHE_MAX_ENTRIES: '{}' must be an integer", val)
                    })?;
                    parsed.min(5000)
                }
                Err(_) => 1000,
            };

        let upstream_connect_timeout_secs = match env::var("UPSTREAM_CONNECT_TIMEOUT_SECS") {
            Ok(val) => val
                .trim()
                .parse::<u64>()
                .map_err(|_| format!("Invalid UPSTREAM_CONNECT_TIMEOUT_SECS: '{}'", val))?,
            Err(_) => 5,
        };

        let upstream_timeout_secs = match env::var("UPSTREAM_TIMEOUT_SECS") {
            Ok(val) => val
                .trim()
                .parse::<u64>()
                .map_err(|_| format!("Invalid UPSTREAM_TIMEOUT_SECS: '{}'", val))?,
            Err(_) => 15,
        };

        let rate_limit_enabled = match env::var("RATE_LIMIT_ENABLED") {
            Ok(val) => val.trim().to_lowercase() == "true",
            Err(_) => false,
        };

        let trusted_proxy_header = env::var("TRUSTED_PROXY_HEADER")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let enable_debug_log = match env::var("ENABLE_DEBUG_LOG") {
            Ok(val) => val.trim().to_lowercase() == "true",
            Err(_) => false,
        };

        let log_level = if enable_debug_log {
            "debug".to_string()
        } else {
            env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string())
        };

        let log_client_info = match env::var("LOG_CLIENT_INFO") {
            Ok(val) => val.trim().to_lowercase() == "true",
            Err(_) => false,
        };

        Ok(Self {
            port,
            lustpress_graphql,
            redis_url,
            expire_cache_hours,
            user_agent,
            cache_max_capacity,
            cache_max_bytes,
            upstream_connect_timeout_secs,
            upstream_timeout_secs,
            rate_limit_enabled,
            trusted_proxy_header,
            log_level,
            log_client_info,
            enable_debug_log,
        })
    }

    pub fn bind_addr(&self) -> SocketAddr {
        SocketAddr::from(([0, 0, 0, 0], self.port))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        temp_env::with_vars(
            [
                ("PORT", None::<&str>),
                ("LUSTPRESS_GRAPHQL", None::<&str>),
                ("ENABLE_DEBUG_LOG", None::<&str>),
                ("USER_AGENT", None::<&str>),
            ],
            || {
                let config = Config::from_env().unwrap();
                assert_eq!(config.port, 3000);
                assert!(!config.lustpress_graphql);
                assert!(!config.enable_debug_log);
                assert_eq!(config.expire_cache_hours, 1);
                assert_eq!(config.cache_max_capacity, 1000);
                assert_eq!(config.cache_max_bytes, 16 * 1024 * 1024);
                let expected = format!("lustpress/{}", env!("CARGO_PKG_VERSION"));
                assert_eq!(config.user_agent, expected);
            },
        );
    }

    #[test]
    fn test_custom_user_agent() {
        temp_env::with_var("USER_AGENT", Some("lustpress-custom/1.0.0"), || {
            let config = Config::from_env().unwrap();
            assert_eq!(config.user_agent, "lustpress-custom/1.0.0");
        });
    }
}
