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
        "https://www.pornhub.com/view_video.php?viewkey={}",
        query.id
    );
    let body = state.fetch.fetch_body(&url).await?;
    let res = parse::parse_pornhub_get(&body)?;
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
        format!("https://www.pornhub.com/video/search?search={encoded_key}")
    } else {
        format!("https://www.pornhub.com/video/search?search={encoded_key}&page={page}")
    };

    let body = state.fetch.fetch_body(&url).await?;
    let res = parse::parse_pornhub_search(&body, &url)?;
    Ok(Json(res))
}

pub async fn related_handler(
    State(state): State<AppState>,
    SafeQuery(query): SafeQuery<IdQuery>,
) -> Result<Json<SearchResponse>, AppError> {
    if query.id.is_empty() {
        return Err(AppError::Validation("Parameter id is required".to_string()));
    }
    let url = format!(
        "https://www.pornhub.com/view_video.php?viewkey={}",
        query.id
    );
    let body = state.fetch.fetch_body(&url).await?;
    let res = parse::parse_pornhub_search(&body, &url)?;
    Ok(Json(res))
}

pub async fn random_handler(
    State(state): State<AppState>,
) -> Result<Json<VideoResponse>, AppError> {
    let url = "https://www.pornhub.com/video/random";
    let body = state.fetch.fetch_body(url).await?;
    let res = parse::parse_pornhub_get(&body)?;
    Ok(Json(res))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/get", get(get_handler))
        .route("/search", get(search_handler))
        .route("/related", get(related_handler))
        .route("/random", get(random_handler))
}
