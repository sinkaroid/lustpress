use crate::error::AppError;
use crate::model::{SearchItem, SearchResponse, VideoDetails, VideoResponse};
use crate::util::second_to_minute;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct EpornerThumb {
    pub size: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub src: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct EpornerApiVideo {
    pub id: String,
    pub title: Option<String>,
    pub keywords: Option<String>,
    pub views: Option<serde_json::Value>,
    pub rate: Option<serde_json::Value>,
    pub url: Option<String>,
    pub added: Option<String>,
    pub length_sec: Option<u64>,
    pub length_min: Option<String>,
    pub embed: Option<String>,
    pub default_thumb: Option<EpornerThumb>,
    pub thumbs: Option<Vec<EpornerThumb>>,
}

#[derive(Debug, Deserialize)]
pub struct EpornerSearchApiResponse {
    pub count: Option<u32>,
    pub total_count: Option<u32>,
    pub total_pages: Option<u32>,
    pub videos: Option<Vec<EpornerApiVideo>>,
}

fn value_to_string(v: Option<&serde_json::Value>) -> Option<String> {
    match v {
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        Some(serde_json::Value::Number(n)) => Some(n.to_string()),
        _ => None,
    }
}

fn parse_rate_to_f64(v: Option<&serde_json::Value>) -> Option<f64> {
    match v {
        Some(serde_json::Value::Number(n)) => n.as_f64(),
        Some(serde_json::Value::String(s)) => s.parse::<f64>().ok(),
        _ => None,
    }
}

pub fn clean_eporner_id(raw: &str) -> &str {
    let mut s = raw.trim();
    if let Some(rest) = s.strip_prefix("https://www.eporner.com/") {
        s = rest;
    }
    if let Some(rest) = s.strip_prefix("http://www.eporner.com/") {
        s = rest;
    }
    if let Some(rest) = s.strip_prefix("hd-porn/") {
        s = rest;
    }
    if let Some(rest) = s.strip_prefix("video-") {
        s = rest;
    }
    s.split('/').next().unwrap_or(s).trim()
}

pub fn parse_eporner_video_info(body: &[u8]) -> Result<EpornerApiVideo, AppError> {
    if body.is_empty() || body == b"[]" {
        return Err(AppError::Scraper("Video not found".to_string()));
    }

    serde_json::from_slice::<EpornerApiVideo>(body)
        .map_err(|_| AppError::Scraper("Video not found".to_string()))
}

pub fn parse_eporner_get(body: &[u8], _url: &str) -> Result<VideoResponse, AppError> {
    let video = parse_eporner_video_info(body)?;

    let clean_id = &video.id;
    let full_id = format!("video-{clean_id}");
    let title = video.title.clone().unwrap_or_else(|| "None".to_string());

    let image = video
        .default_thumb
        .as_ref()
        .and_then(|t| t.src.clone())
        .or_else(|| {
            video
                .thumbs
                .as_ref()
                .and_then(|ts| ts.first())
                .and_then(|t| t.src.clone())
        });

    let duration = video
        .length_sec
        .map(second_to_minute)
        .or_else(|| video.length_min.clone())
        .unwrap_or_else(|| "0min, 0sec".to_string());

    let views = value_to_string(video.views.as_ref()).unwrap_or_else(|| "0".to_string());

    // Convert 5-star rate into 100% percentage scale matching legacy format (e.g. 4.57 -> "91.40")
    let rating =
        parse_rate_to_f64(video.rate.as_ref()).map(|r| format!("{:.2}", (r / 5.0) * 100.0));
    let uploaded = video.added.clone().unwrap_or_else(|| "None".to_string());

    let tags: Vec<String> = video
        .keywords
        .as_deref()
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let source = video
        .url
        .clone()
        .unwrap_or_else(|| format!("https://www.eporner.com/{full_id}/"));

    let embed = video
        .embed
        .clone()
        .unwrap_or_else(|| format!("https://www.eporner.com/embed/{clean_id}/"));

    let image_asset = image.clone().unwrap_or_else(|| "None".to_string());

    Ok(VideoResponse {
        success: true,
        data: VideoDetails {
            title,
            id: full_id,
            image,
            duration,
            views,
            rating,
            uploaded,
            upvoted: Some("0".to_string()),
            downvoted: Some("0".to_string()),
            channel: None,
            models: Vec::new(),
            tags,
        },
        source,
        assets: vec![embed, image_asset],
    })
}

pub fn parse_eporner_search(body: &[u8], url: &str) -> Result<SearchResponse, AppError> {
    if body.is_empty() || body == b"[]" {
        return Err(AppError::Scraper("No result found".to_string()));
    }

    let resp: EpornerSearchApiResponse = serde_json::from_slice(body)
        .map_err(|e| AppError::Scraper(format!("Invalid API response: {e}")))?;

    let videos = resp.videos.unwrap_or_default();
    if videos.is_empty() {
        return Err(AppError::Scraper("No result found".to_string()));
    }

    let mut items = Vec::with_capacity(videos.len());

    for video in videos {
        let clean_id = &video.id;
        let id = format!("video-{clean_id}");
        let link = video
            .url
            .clone()
            .unwrap_or_else(|| format!("https://www.eporner.com/{id}/"));
        let title = video.title;
        let image = video
            .default_thumb
            .as_ref()
            .and_then(|t| t.src.clone())
            .or_else(|| {
                video
                    .thumbs
                    .as_ref()
                    .and_then(|ts| ts.first())
                    .and_then(|t| t.src.clone())
            });
        let duration = video.length_min;
        // Search rating formatted as percentage e.g. "91%" matching legacy
        let rating =
            parse_rate_to_f64(video.rate.as_ref()).map(|r| format!("{:.0}%", (r / 5.0) * 100.0));
        let views = value_to_string(video.views.as_ref());
        let embed = video
            .embed
            .clone()
            .unwrap_or_else(|| format!("https://www.eporner.com/embed/{clean_id}/"));

        items.push(SearchItem {
            link,
            id,
            title,
            image,
            duration,
            rating,
            views,
            uploader: None,
            video: Some(embed),
        });
    }

    Ok(SearchResponse {
        success: true,
        data: items,
        source: url.to_string(),
    })
}

pub fn parse_eporner_related(body: &[u8], url: &str) -> Result<SearchResponse, AppError> {
    parse_eporner_search(body, url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_eporner_id() {
        assert_eq!(clean_eporner_id("video-pPAyWiJwOFY"), "pPAyWiJwOFY");
        assert_eq!(clean_eporner_id("pPAyWiJwOFY"), "pPAyWiJwOFY");
        assert_eq!(
            clean_eporner_id("video-pPAyWiJwOFY/test-slug/"),
            "pPAyWiJwOFY"
        );
        assert_eq!(
            clean_eporner_id("hd-porn/pPAyWiJwOFY/test-slug/"),
            "pPAyWiJwOFY"
        );
        assert_eq!(
            clean_eporner_id("https://www.eporner.com/video-pPAyWiJwOFY/test-slug/"),
            "pPAyWiJwOFY"
        );
    }

    #[test]
    fn test_parse_eporner_get_valid() {
        let mock_json = r#"{
            "id": "pPAyWiJwOFY",
            "title": "Sample Cosplay Video",
            "keywords": "cosplay, anime, lingerie",
            "views": 1500,
            "rate": "4.85",
            "url": "https://www.eporner.com/video-pPAyWiJwOFY/sample/",
            "added": "2026-10-31 16:57:58",
            "length_sec": 152,
            "length_min": "2:32",
            "embed": "https://www.eporner.com/embed/pPAyWiJwOFY/",
            "default_thumb": {
                "size": "medium",
                "src": "https://static.eporner.com/thumb.jpg"
            }
        }"#;

        let res = parse_eporner_get(mock_json.as_bytes(), "https://example.com").unwrap();
        assert_eq!(res.data.id, "video-pPAyWiJwOFY");
        assert_eq!(res.data.title, "Sample Cosplay Video");
        assert_eq!(res.data.views, "1500");
        assert_eq!(res.data.rating.as_deref(), Some("97.00"));
        assert_eq!(res.data.upvoted.as_deref(), Some("0"));
        assert_eq!(res.data.downvoted.as_deref(), Some("0"));
        assert_eq!(res.data.tags, vec!["cosplay", "anime", "lingerie"]);
        assert_eq!(res.assets[0], "https://www.eporner.com/embed/pPAyWiJwOFY/");
    }

    #[test]
    fn test_parse_eporner_search_valid() {
        let mock_json = r#"{
            "count": 1,
            "videos": [
                {
                    "id": "abc12345",
                    "title": "Cosplay Action",
                    "views": 25000,
                    "rate": "5.00",
                    "url": "https://www.eporner.com/video-abc12345/cosplay-action/",
                    "length_min": "12:34",
                    "embed": "https://www.eporner.com/embed/abc12345/",
                    "default_thumb": {
                        "src": "https://static.eporner.com/thumb.jpg"
                    }
                }
            ]
        }"#;

        let res = parse_eporner_search(mock_json.as_bytes(), "https://example.com").unwrap();
        assert_eq!(res.data.len(), 1);
        assert_eq!(res.data[0].id, "video-abc12345");
        assert_eq!(res.data[0].title.as_deref(), Some("Cosplay Action"));
        assert_eq!(res.data[0].rating.as_deref(), Some("100%"));
        assert_eq!(res.data[0].views.as_deref(), Some("25000"));
    }

    #[test]
    fn test_parse_eporner_empty_search() {
        let mock_json = r#"{"count":0,"videos":[]}"#;
        let err = parse_eporner_search(mock_json.as_bytes(), "https://example.com").unwrap_err();
        match err {
            AppError::Scraper(msg) => assert_eq!(msg, "No result found"),
            _ => panic!("Expected Scraper error"),
        }
    }

    #[test]
    fn test_parse_eporner_get_not_found() {
        let err = parse_eporner_get(b"[]", "https://example.com").unwrap_err();
        match err {
            AppError::Scraper(msg) => assert_eq!(msg, "Video not found"),
            _ => panic!("Expected Scraper error"),
        }
    }
}
