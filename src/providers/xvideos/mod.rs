pub mod parse;

use crate::error::{AppError, SafeQuery};
use crate::model::{SearchResponse, VideoResponse};
use crate::state::AppState;
use crate::util::encode_query;
use axum::{Json, Router, extract::State, routing::get};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct IdQuery {
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub key: String,
    pub page: Option<String>,
}

fn build_xvideos_url(id: &str) -> String {
    let clean = id.trim_start_matches('/');
    if clean.starts_with("video") {
        format!("https://www.xvideos.com/{clean}")
    } else {
        format!("https://www.xvideos.com/video{clean}")
    }
}

pub async fn get_handler(
    State(state): State<AppState>,
    SafeQuery(query): SafeQuery<IdQuery>,
) -> Result<Json<VideoResponse>, AppError> {
    if query.id.is_empty() {
        return Err(AppError::Validation("Parameter id is required".to_string()));
    }
    let url = build_xvideos_url(&query.id);
    let body = state.fetch.fetch_body(&url).await?;
    let res = parse::parse_xvideos_get(&body)?;
    Ok(Json(res))
}

pub async fn search_handler(
    State(state): State<AppState>,
    SafeQuery(query): SafeQuery<SearchQuery>,
) -> Result<Json<SearchResponse>, AppError> {
    if query.key.is_empty() {
        return Err(AppError::Validation(
            "Parameter key is required".to_string(),
        ));
    }
    let page: u32 = query.page.as_deref().unwrap_or("1").parse().unwrap_or(1);
    let encoded_key = encode_query(&query.key);

    let url = if page <= 1 {
        format!("https://www.xvideos.com/?k={encoded_key}")
    } else {
        format!("https://www.xvideos.com/?k={encoded_key}&p={page}")
    };

    let body = state.fetch.fetch_body(&url).await?;
    let res = parse::parse_xvideos_search(&body, &url)?;
    Ok(Json(res))
}

pub async fn related_handler(
    State(state): State<AppState>,
    SafeQuery(query): SafeQuery<IdQuery>,
) -> Result<Json<SearchResponse>, AppError> {
    if query.id.is_empty() {
        return Err(AppError::Validation("Parameter id is required".to_string()));
    }
    let url = build_xvideos_url(&query.id);
    let body = state.fetch.fetch_body(&url).await?;
    let res = parse::parse_xvideos_related(&body, &url)?;
    Ok(Json(res))
}

pub async fn random_handler(
    State(state): State<AppState>,
) -> Result<Json<VideoResponse>, AppError> {
    let home_url = "https://www.xvideos.com/";
    let body = state.fetch.fetch_body(home_url).await?;
    let search_res = parse::parse_xvideos_search(&body, home_url)?;

    if search_res.data.is_empty() {
        return Err(AppError::Scraper("No videos found on homepage".to_string()));
    }

    let idx = (chrono::Local::now().timestamp_subsec_nanos() as usize) % search_res.data.len();
    let random_item = &search_res.data[idx];

    let url = build_xvideos_url(&random_item.id);
    let video_body = state.fetch.fetch_body(&url).await?;
    let res = parse::parse_xvideos_get(&video_body)?;
    Ok(Json(res))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/get", get(get_handler))
        .route("/search", get(search_handler))
        .route("/related", get(related_handler))
        .route("/random", get(random_handler))
}
