use crate::state::AppState;
use crate::util::format_locale_date;
use axum::{Json, extract::State};
use serde::Serialize;

#[derive(Serialize)]
pub struct RootStatusResponse {
    pub success: bool,
    pub playground: &'static str,
    pub endpoint: &'static str,
    pub date: String,
    pub rss: String,
    pub heap: &'static str,
    pub server: String,
    pub version: &'static str,
}

pub async fn root_handler(State(state): State<AppState>) -> Json<RootStatusResponse> {
    let server_location = state.get_server_location().await;
    let date = format_locale_date();

    let rss_bytes = crate::metrics::METRICS_RSS.load(std::sync::atomic::Ordering::Relaxed);
    let rss = if rss_bytes > 0 {
        format!("{:.1} MB", (rss_bytes as f64) / (1024.0 * 1024.0))
    } else {
        "0.0 MB".to_string()
    };

    Json(RootStatusResponse {
        success: true,
        playground: "https://sinkaroid.github.io/lustpress",
        endpoint: "https://github.com/sinkaroid/lustpress/blob/master/README.md#routing",
        date,
        rss,
        heap: "0/0 MB", // Explicitly documented zero per PRD D5
        server: server_location,
        version: env!("CARGO_PKG_VERSION"),
    })
}
