pub mod parse;

use crate::error::{AppError, SafeQuery};
use crate::state::AppState;
use crate::util::encode_query;
use axum::{Json, Router, extract::State, routing::get};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct IdQuery {
    pub id: String,
    pub page: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub key: String,
    pub page: Option<String>,
}

fn get_api_url(video_id: &str) -> Result<String, AppError> {
    let id: u64 = video_id
        .trim()
        .parse()
        .map_err(|_| AppError::Validation("Invalid video id: must be numeric".to_string()))?;

    let million = (id / 1_000_000) * 1_000_000;
    let thousand = (id / 1_000) * 1_000;

    Ok(format!(
        "https://txxx.com/api/json/video/86400/{million}/{thousand}/{id}.json"
    ))
}

fn get_related_api_url(video_id: &str, page: u32, count: u32) -> Result<String, AppError> {
    let id: u64 = video_id
        .trim()
        .parse()
        .map_err(|_| AppError::Validation("Invalid video id: must be numeric".to_string()))?;

    let million = (id / 1_000_000) * 1_000_000;
    let thousand = (id / 1_000) * 1_000;

    Ok(format!(
        "https://txxx.com/api/json/videos_related2/432000/{count}/{million}/{thousand}/{id}.all.{page}.json"
    ))
}

pub async fn get_handler(
    State(state): State<AppState>,
    SafeQuery(query): SafeQuery<IdQuery>,
) -> Result<Json<crate::model::VideoResponse>, AppError> {
    if query.id.is_empty() {
        return Err(AppError::Validation("Parameter id is required".to_string()));
    }
    let api_url = get_api_url(&query.id)?;
    let body = state.fetch.fetch_body(&api_url).await?;
    let res = parse::parse_txxx_get(&body)?;
    Ok(Json(res))
}

pub async fn search_handler(
    State(state): State<AppState>,
    SafeQuery(query): SafeQuery<SearchQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    if query.key.is_empty() {
        return Err(AppError::Validation(
            "Parameter key is required".to_string(),
        ));
    }
    let page: u32 = query.page.as_deref().unwrap_or("1").parse().unwrap_or(1);
    let encoded_key = encode_query(&query.key);

    let api_url = format!(
        "https://txxx.com/api/videos2.php?params=259200/str/relevance/60/search..{page}.all..&s={encoded_key}"
    );

    let body = state.fetch.fetch_body(&api_url).await?;
    let res = parse::parse_txxx_search(&body, page)?;
    Ok(Json(res))
}

pub async fn related_handler(
    State(state): State<AppState>,
    SafeQuery(query): SafeQuery<IdQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    if query.id.is_empty() {
        return Err(AppError::Validation("Parameter id is required".to_string()));
    }
    let page: u32 = query.page.as_deref().unwrap_or("1").parse().unwrap_or(1);
    let api_url = get_related_api_url(&query.id, page, 50)?;

    let body = state.fetch.fetch_body(&api_url).await?;
    let res = parse::parse_txxx_related(&body, &query.id, page)?;
    Ok(Json(res))
}

pub async fn random_handler(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_url = "https://txxx.com/api/json/videos2/14400/str/most-popular/60/..1.all..day.json";
    let body = state.fetch.fetch_body(api_url).await?;
    let res = parse::parse_txxx_random(&body, api_url)?;
    Ok(Json(res))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/get", get(get_handler))
        .route("/search", get(search_handler))
        .route("/related", get(related_handler))
        .route("/random", get(random_handler))
}
