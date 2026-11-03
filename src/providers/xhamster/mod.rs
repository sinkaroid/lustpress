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

pub async fn get_handler(
    State(state): State<AppState>,
    SafeQuery(query): SafeQuery<IdQuery>,
) -> Result<Json<VideoResponse>, AppError> {
    if query.id.is_empty() {
        return Err(AppError::Validation("Parameter id is required".to_string()));
    }
    let url = format!(
        "https://xhamster.com/videos/{}",
        query.id.trim_start_matches('/')
    );
    let body = state.fetch.fetch_body(&url).await?;
    let res = parse::parse_xhamster_get(&body)?;
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
        format!("https://xhamster.com/search/{encoded_key}")
    } else {
        format!("https://xhamster.com/search/{encoded_key}?page={page}")
    };

    let body = state.fetch.fetch_body(&url).await?;
    let res = parse::parse_xhamster_search(&body, &url)?;
    Ok(Json(res))
}

pub async fn related_handler(
    State(state): State<AppState>,
    SafeQuery(query): SafeQuery<IdQuery>,
) -> Result<Json<SearchResponse>, AppError> {
    if query.id.is_empty() {
        return Err(AppError::Validation("Parameter id is required".to_string()));
    }
    let clean_id = query.id.trim_start_matches('/');
    let url = if clean_id.starts_with("videos/") {
        format!("https://xhamster.com/{clean_id}")
    } else {
        format!("https://xhamster.com/videos/{clean_id}")
    };
    let body = state.fetch.fetch_body(&url).await?;
    let res = parse::parse_xhamster_search(&body, &url)?;
    Ok(Json(res))
}

pub async fn random_handler(
    State(state): State<AppState>,
) -> Result<Json<VideoResponse>, AppError> {
    let newest_url = "https://xhamster.com/newest";
    let body = state.fetch.fetch_body(newest_url).await?;
    let search_res = parse::parse_xhamster_search(&body, newest_url)?;

    if search_res.data.is_empty() {
        return Err(AppError::Scraper(
            "No videos found on newest page".to_string(),
        ));
    }

    let idx = (chrono::Local::now().timestamp_subsec_nanos() as usize) % search_res.data.len();
    let random_item = &search_res.data[idx];

    let url = format!("https://xhamster.com/videos/{}", random_item.id);
    let video_body = state.fetch.fetch_body(&url).await?;
    let res = parse::parse_xhamster_get(&video_body)?;
    Ok(Json(res))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/get", get(get_handler))
        .route("/search", get(search_handler))
        .route("/related", get(related_handler))
        .route("/random", get(random_handler))
}
