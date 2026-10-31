use crate::error::AppError;
use crate::model::{VideoDetails, VideoResponse};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct TxxxCategory {
    pub title: String,
}

#[derive(Debug, Deserialize)]
pub struct TxxxStatistics {
    pub viewed: Option<String>,
    pub rating: Option<String>,
    pub likes: Option<serde_json::Value>,
    pub dislikes: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct TxxxChannel {
    pub title: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TxxxVideoRaw {
    pub video_id: serde_json::Value,
    pub title: Option<String>,
    pub dir: Option<String>,
    pub duration: Option<String>,
    pub thumbsrc: Option<String>,
    pub thumb: Option<String>,
    pub post_date: Option<String>,
    pub statistics: Option<TxxxStatistics>,
    pub channel: Option<TxxxChannel>,
    pub categories: Option<HashMap<String, TxxxCategory>>,
    pub tags: Option<HashMap<String, TxxxCategory>>,
    pub models: Option<HashMap<String, TxxxCategory>>,
    pub models_suggested: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct TxxxGetResponse {
    pub video: Option<TxxxVideoRaw>,
}

pub fn parse_txxx_get(bytes: &[u8]) -> Result<VideoResponse, AppError> {
    let raw: TxxxGetResponse = serde_json::from_slice(bytes)
        .map_err(|e| AppError::Scraper(format!("Invalid API response: {e}")))?;

    let video = raw
        .video
        .ok_or_else(|| AppError::Scraper("Invalid API response".to_string()))?;

    let video_id_str = match &video.video_id {
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        _ => "None".to_string(),
    };

    let categories: Vec<String> = video
        .categories
        .unwrap_or_default()
        .into_values()
        .map(|c| c.title)
        .collect();

    let tags: Vec<String> = video
        .tags
        .unwrap_or_default()
        .into_values()
        .map(|t| t.title)
        .collect();

    let models: Vec<String> = {
        let m: Vec<String> = video
            .models
            .unwrap_or_default()
            .into_values()
            .map(|m| m.title)
            .collect();
        if !m.is_empty() {
            m
        } else {
            video.models_suggested.unwrap_or_default()
        }
    };

    let all_tags = [categories, tags].concat();
    let video_dir = video.dir.unwrap_or_default();
    let embed = format!("https://txxx.com/embed/{video_id_str}/");
    let image = video
        .thumbsrc
        .or(video.thumb)
        .unwrap_or_else(|| "None".to_string());

    let (views, rating, upvoted, downvoted) = if let Some(stats) = video.statistics {
        let v = stats.viewed.unwrap_or_else(|| "0".to_string());
        let r = stats.rating.unwrap_or_else(|| "0.00".to_string());
        let up = stats
            .likes
            .map(|l| l.to_string())
            .unwrap_or_else(|| "0".to_string());
        let down = stats
            .dislikes
            .map(|d| d.to_string())
            .unwrap_or_else(|| "0".to_string());
        (v, r, up, down)
    } else {
        (
            "0".to_string(),
            "0.00".to_string(),
            "0".to_string(),
            "0".to_string(),
        )
    };

    let mut assets = vec![embed];
    if image != "None" {
        assets.push(image.clone());
    }

    Ok(VideoResponse {
        success: true,
        data: VideoDetails {
            title: video.title.unwrap_or_else(|| "None".to_string()),
            id: video_id_str.clone(),
            image: Some(image),
            duration: video.duration.unwrap_or_else(|| "None".to_string()),
            views,
            rating: Some(rating),
            uploaded: video.post_date.unwrap_or_else(|| "None".to_string()),
            upvoted: Some(upvoted),
            downvoted: Some(downvoted),
            channel: video.channel.and_then(|c| c.title),
            models,
            tags: all_tags,
        },
        source: format!("https://txxx.com/videos/{video_id_str}/{video_dir}/"),
        assets,
    })
}

#[derive(Debug, Deserialize)]
pub struct TxxxSearchVideoRaw {
    pub video_id: serde_json::Value,
    pub title: Option<String>,
    pub dir: Option<String>,
    pub duration: Option<String>,
    pub video_viewed: Option<serde_json::Value>,
    pub rating: Option<serde_json::Value>,
    pub post_date: Option<String>,
    pub likes: Option<serde_json::Value>,
    pub dislikes: Option<serde_json::Value>,
    pub scr: Option<String>,
    pub categories: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TxxxSearchResponseRaw {
    pub videos: Option<Vec<TxxxSearchVideoRaw>>,
    pub total_count: Option<serde_json::Value>,
    pub pages: Option<serde_json::Value>,
}

pub fn parse_txxx_search(bytes: &[u8], page: u32) -> Result<serde_json::Value, AppError> {
    let raw: TxxxSearchResponseRaw = serde_json::from_slice(bytes)
        .map_err(|e| AppError::Scraper(format!("Invalid search response: {e}")))?;

    let videos = raw.videos.unwrap_or_default();
    let total_count = match raw.total_count {
        Some(serde_json::Value::Number(n)) => n.to_string(),
        Some(serde_json::Value::String(s)) => s,
        _ => videos.len().to_string(),
    };

    let pages = match raw.pages {
        Some(serde_json::Value::Number(n)) => n.as_u64().unwrap_or(1),
        Some(serde_json::Value::String(s)) => s.parse().unwrap_or(1),
        _ => 1,
    };

    let data: Vec<serde_json::Value> = videos
        .into_iter()
        .map(|v| {
            let vid_str = match &v.video_id {
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::String(s) => s.clone(),
                _ => "".to_string(),
            };

            let cats: Vec<&str> = v
                .categories
                .as_deref()
                .map(|c| c.split(',').collect())
                .unwrap_or_default();
            let embed = format!("https://txxx.com/embed/{vid_str}/");

            serde_json::json!({
                "video_id": v.video_id,
                "title": v.title.unwrap_or_default(),
                "dir": v.dir.unwrap_or_default(),
                "duration": v.duration.unwrap_or_default(),
                "views": v.video_viewed,
                "rating": v.rating,
                "uploaded": v.post_date,
                "likes": v.likes,
                "dislikes": v.dislikes,
                "image": v.scr,
                "categories": cats,
                "embed": embed,
            })
        })
        .collect();

    Ok(serde_json::json!({
        "success": true,
        "total_count": total_count,
        "pages": pages,
        "page": page,
        "data": data,
    }))
}

#[derive(Debug, Deserialize)]
pub struct TxxxRelatedVideoRaw {
    pub video_id: serde_json::Value,
    pub title: Option<String>,
    pub scr: Option<String>,
    pub thumb: Option<String>,
    pub duration: Option<String>,
    pub video_viewed: Option<serde_json::Value>,
    pub rating: Option<serde_json::Value>,
    pub username: Option<String>,
    pub display_name: Option<String>,
    pub dir: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TxxxRelatedResponseRaw {
    pub videos: Option<Vec<TxxxRelatedVideoRaw>>,
    pub total_count: Option<serde_json::Value>,
    pub pages: Option<serde_json::Value>,
}

pub fn parse_txxx_related(
    bytes: &[u8],
    id: &str,
    page: u32,
) -> Result<serde_json::Value, AppError> {
    let raw: TxxxRelatedResponseRaw = serde_json::from_slice(bytes)
        .map_err(|e| AppError::Scraper(format!("Invalid related response: {e}")))?;

    let videos = raw.videos.unwrap_or_default();
    let total_count = match raw.total_count {
        Some(serde_json::Value::Number(n)) => n.to_string(),
        Some(serde_json::Value::String(s)) => s,
        _ => "0".to_string(),
    };

    let pages = match raw.pages {
        Some(serde_json::Value::Number(n)) => n.as_u64().unwrap_or(1),
        Some(serde_json::Value::String(s)) => s.parse().unwrap_or(1),
        _ => 1,
    };

    let data: Vec<serde_json::Value> = videos
        .into_iter()
        .map(|v| {
            let vid_str = match &v.video_id {
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::String(s) => s.clone(),
                _ => "".to_string(),
            };

            let img = v.scr.or(v.thumb).unwrap_or_default();
            let uploader = v.username.or(v.display_name).unwrap_or_default();
            let dir = v.dir.unwrap_or_default();

            serde_json::json!({
                "id": vid_str,
                "title": v.title.unwrap_or_default(),
                "image": img,
                "duration": v.duration.unwrap_or_else(|| "None".to_string()),
                "views": v.video_viewed.map(|v| v.to_string()).unwrap_or_else(|| "0".to_string()),
                "rating": v.rating.map(|r| r.to_string()).unwrap_or_else(|| "0".to_string()),
                "uploader": uploader,
                "link": format!("https://txxx.com/videos/{vid_str}/{dir}/"),
                "video": format!("https://txxx.com/embed/{vid_str}/"),
            })
        })
        .collect();

    Ok(serde_json::json!({
        "success": true,
        "total_count": total_count,
        "pages": pages,
        "page": page,
        "data": data,
        "source": format!("https://txxx.com/videos/{id}/"),
    }))
}

pub fn parse_txxx_random(bytes: &[u8], source_url: &str) -> Result<serde_json::Value, AppError> {
    let raw: TxxxSearchResponseRaw = serde_json::from_slice(bytes)
        .map_err(|e| AppError::Scraper(format!("Invalid random response: {e}")))?;

    let videos = raw.videos.unwrap_or_default();
    if videos.is_empty() {
        return Err(AppError::Scraper(
            "No videos returned from upstream".to_string(),
        ));
    }

    // Pick random index
    let idx = (chrono::Local::now().timestamp_subsec_nanos() as usize) % videos.len();
    let v = &videos[idx];

    let vid_str = match &v.video_id {
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        _ => "".to_string(),
    };

    let cats: Vec<&str> = v
        .categories
        .as_deref()
        .map(|c| c.split(',').collect())
        .unwrap_or_default();
    let embed = format!("https://txxx.com/embed/{vid_str}/");

    let data = serde_json::json!({
        "video_id": v.video_id,
        "title": v.title.as_deref().unwrap_or_default(),
        "dir": v.dir.as_deref().unwrap_or_default(),
        "duration": v.duration.as_deref().unwrap_or_default(),
        "views": v.video_viewed,
        "rating": v.rating,
        "uploaded": v.post_date,
        "likes": v.likes,
        "dislikes": v.dislikes,
        "image": v.scr,
        "categories": cats,
        "embed": embed,
    });

    Ok(serde_json::json!({
        "success": true,
        "data": data,
        "source": source_url,
    }))
}
