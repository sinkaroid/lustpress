pub mod parse;

pub use parse::clean_eporner_id;

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

fn build_eporner_api_get_url(id: &str) -> String {
    let clean_id = parse::clean_eporner_id(id);
    format!("https://www.eporner.com/api/v2/video/id/?id={clean_id}&format=json")
}

pub async fn get_handler(
    State(state): State<AppState>,
    SafeQuery(query): SafeQuery<IdQuery>,
) -> Result<Json<VideoResponse>, AppError> {
    if query.id.is_empty() {
        return Err(AppError::Validation("Parameter id is required".to_string()));
    }
    let url = build_eporner_api_get_url(&query.id);
    let body = state.fetch.fetch_body(&url).await?;
    let res = parse::parse_eporner_get(&body, &url)?;
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

    let api_url = format!(
        "https://www.eporner.com/api/v2/video/search/?query={encoded_key}&page={page}&per_page=30&format=json"
    );

    let web_url = if page <= 1 {
        format!("https://www.eporner.com/search/{encoded_key}/")
    } else {
        format!("https://www.eporner.com/search/{encoded_key}/{page}/")
    };

    let body = state.fetch.fetch_body(&api_url).await?;
    let res = parse::parse_eporner_search(&body, &web_url)?;
    Ok(Json(res))
}

pub async fn related_handler(
    State(state): State<AppState>,
    SafeQuery(query): SafeQuery<IdQuery>,
) -> Result<Json<SearchResponse>, AppError> {
    if query.id.is_empty() {
        return Err(AppError::Validation("Parameter id is required".to_string()));
    }
    let clean_id = parse::clean_eporner_id(&query.id);
    let info_url = format!("https://www.eporner.com/api/v2/video/id/?id={clean_id}&format=json");
    let body = state.fetch.fetch_body(&info_url).await?;
    let video_info = parse::parse_eporner_video_info(&body)?;

    let keyword = video_info
        .keywords
        .as_deref()
        .and_then(|k| {
            k.split(',')
                .map(|s| s.trim())
                .find(|s| !s.is_empty() && s.len() > 2)
        })
        .unwrap_or("porn");

    let encoded_key = encode_query(keyword);
    let search_url = format!(
        "https://www.eporner.com/api/v2/video/search/?query={encoded_key}&per_page=30&format=json"
    );
    let web_url = format!("https://www.eporner.com/video-{clean_id}/");
    let search_body = state.fetch.fetch_body(&search_url).await?;
    let res = parse::parse_eporner_search(&search_body, &web_url)?;
    Ok(Json(res))
}

pub async fn random_handler(
    State(state): State<AppState>,
) -> Result<Json<VideoResponse>, AppError> {
    let weekly_api_url =
        "https://www.eporner.com/api/v2/video/search/?order=top-weekly&per_page=30&format=json";
    let weekly_web_url = "https://www.eporner.com/cat/all/SORT-top-weekly/";
    let body = state.fetch.fetch_body(weekly_api_url).await?;
    let search_res = parse::parse_eporner_search(&body, weekly_web_url)?;

    if search_res.data.is_empty() {
        return Err(AppError::Scraper("No weekly top videos found".to_string()));
    }

    let idx = (chrono::Local::now().timestamp_subsec_nanos() as usize) % search_res.data.len();
    let random_item = &search_res.data[idx];

    let url = build_eporner_api_get_url(&random_item.id);
    let video_body = state.fetch.fetch_body(&url).await?;
    let res = parse::parse_eporner_get(&video_body, &url)?;
    Ok(Json(res))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/get", get(get_handler))
        .route("/search", get(search_handler))
        .route("/related", get(related_handler))
        .route("/random", get(random_handler))
}
