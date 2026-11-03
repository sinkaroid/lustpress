use crate::error::AppError;
use crate::model::{SearchItem, SearchResponse, VideoDetails, VideoResponse};
use crate::util::static_selector;
use regex::Regex;
use scraper::{Html, Selector};
use serde::Deserialize;
use std::sync::LazyLock;

static SEL_CANONICAL: LazyLock<Selector> =
    LazyLock::new(|| static_selector("link[rel='canonical']"));
static SEL_OG_TITLE: LazyLock<Selector> =
    LazyLock::new(|| static_selector("meta[property='og:title']"));
static SEL_OG_IMAGE: LazyLock<Selector> =
    LazyLock::new(|| static_selector("meta[property='og:image']"));
static SEL_INITIALS: LazyLock<Selector> = LazyLock::new(|| static_selector("#initials-script"));
static SEL_DATE_CONTAINER: LazyLock<Selector> =
    LazyLock::new(|| static_selector("div.entity-info-container__date"));

static SEL_SEARCH_LINKS: LazyLock<Selector> =
    LazyLock::new(|| static_selector("a.video-thumb__image-container"));
static SEL_SEARCH_VIEWS: LazyLock<Selector> =
    LazyLock::new(|| static_selector("div.video-thumb-views"));
static SEL_SEARCH_DURATION: LazyLock<Selector> =
    LazyLock::new(|| static_selector("span[data-role='video-duration']"));
static SEL_SEARCH_IMG: LazyLock<Selector> = LazyLock::new(|| static_selector("img"));

static RE_DURATION: LazyLock<Regex> =
    LazyLock::new(|| match Regex::new(r#""duration"\s*:\s*(\d+)"#) {
        Ok(r) => r,
        Err(e) => panic!("Invalid regex: {e}"),
    });
static RE_VIEWS: LazyLock<Regex> = LazyLock::new(|| match Regex::new(r#""views"\s*:\s*(\d+)"#) {
    Ok(r) => r,
    Err(e) => panic!("Invalid regex: {e}"),
});

#[derive(Debug, Deserialize)]
pub struct XhamsterTag {
    #[serde(rename = "isTag")]
    pub is_tag: Option<bool>,
    #[serde(rename = "isPornstar")]
    pub is_pornstar: Option<bool>,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct XhamsterRatingModel {
    pub value: Option<serde_json::Value>,
    pub likes: Option<serde_json::Value>,
    pub dislikes: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct XhamsterRatingComponent {
    #[serde(rename = "ratingModel")]
    pub rating_model: Option<XhamsterRatingModel>,
}

#[derive(Debug, Deserialize)]
pub struct XhamsterVideoTagsComponent {
    pub tags: Option<Vec<XhamsterTag>>,
}

#[derive(Debug, Deserialize)]
pub struct XhamsterInitials {
    #[serde(rename = "ratingComponent")]
    pub rating_component: Option<XhamsterRatingComponent>,
    #[serde(rename = "videoTagsComponent")]
    pub video_tags_component: Option<XhamsterVideoTagsComponent>,
}

pub fn parse_xhamster_get(body: &[u8]) -> Result<VideoResponse, AppError> {
    let html_str = String::from_utf8_lossy(body);
    let end_pos = html_str
        .find("<footer")
        .or_else(|| html_str.find("id=\"footer\""))
        .unwrap_or(html_str.len());
    let document = Html::parse_document(&html_str[..end_pos]);

    let link = document
        .select(&SEL_CANONICAL)
        .next()
        .and_then(|el| el.value().attr("href"))
        .unwrap_or("None")
        .to_string();

    let id = link.split('/').nth(4).unwrap_or("None").to_string();

    let title = document
        .select(&SEL_OG_TITLE)
        .next()
        .and_then(|el| el.value().attr("content"))
        .unwrap_or("None")
        .trim()
        .to_string();

    let image = document
        .select(&SEL_OG_IMAGE)
        .next()
        .and_then(|el| el.value().attr("content"))
        .unwrap_or("None")
        .to_string();

    let duration = RE_DURATION
        .captures(&html_str)
        .map(|c| c[1].to_string())
        .unwrap_or_else(|| "None".to_string());

    let views = RE_VIEWS
        .captures(&html_str)
        .map(|c| c[1].to_string())
        .unwrap_or_else(|| "None".to_string());

    let publish = document
        .select(&SEL_DATE_CONTAINER)
        .next()
        .and_then(|el| el.value().attr("data-tooltip"))
        .unwrap_or("None")
        .to_string();

    // Parse initials JSON script
    let mut rating = "None".to_string();
    let mut upvote = "None".to_string();
    let mut downvote = "None".to_string();
    let mut tags = Vec::new();
    let mut models = Vec::new();

    if let Some(script) = document.select(&SEL_INITIALS).next() {
        let text: String = script.text().collect();
        let json_str = text
            .trim()
            .trim_start_matches("window.initials =")
            .trim_start_matches("window.initials=")
            .trim_end_matches(';')
            .trim();

        if let Ok(initials) = serde_json::from_str::<XhamsterInitials>(json_str) {
            if let Some(comp) = initials.rating_component.and_then(|r| r.rating_model) {
                if let Some(val) = comp.value {
                    rating = val.to_string();
                }
                if let Some(likes) = comp.likes {
                    upvote = likes.to_string();
                }
                if let Some(dislikes) = comp.dislikes {
                    downvote = dislikes.to_string();
                }
            }

            if let Some(tag_comp) = initials.video_tags_component.and_then(|v| v.tags) {
                for t in tag_comp {
                    if t.is_tag.unwrap_or(false) {
                        tags.push(t.name.clone());
                    }
                    if t.is_pornstar.unwrap_or(false) {
                        models.push(t.name);
                    }
                }
            }
        }
    }

    let embed_id = link.split('-').next_back().unwrap_or("").trim_matches('/');
    let video = if !embed_id.is_empty() {
        format!("https://xhamster.com/embed/{embed_id}")
    } else {
        "None".to_string()
    };

    Ok(VideoResponse {
        success: true,
        data: VideoDetails {
            title,
            id,
            image: Some(image.clone()),
            duration,
            views,
            rating: Some(rating),
            uploaded: publish,
            upvoted: Some(upvote),
            downvoted: Some(downvote),
            channel: None,
            models,
            tags,
        },
        source: link,
        assets: vec![video, image],
    })
}

pub fn parse_xhamster_search(body: &[u8], url: &str) -> Result<SearchResponse, AppError> {
    let html_str = String::from_utf8_lossy(body);
    let end_pos = html_str
        .find("<footer")
        .or_else(|| html_str.find("id=\"footer\""))
        .unwrap_or(html_str.len());
    let document = Html::parse_document(&html_str[..end_pos]);

    let views_list: Vec<String> = document
        .select(&SEL_SEARCH_VIEWS)
        .map(|el| el.text().collect::<String>().trim().to_string())
        .collect();

    let duration_list: Vec<String> = document
        .select(&SEL_SEARCH_DURATION)
        .map(|el| el.text().collect::<String>().trim().to_string())
        .collect();

    let mut items = Vec::new();

    for (i, el) in document.select(&SEL_SEARCH_LINKS).enumerate() {
        let link_rel = el.value().attr("href").unwrap_or_default();
        let id = link_rel.split('/').nth(4).unwrap_or("None").to_string();

        let img_el = el.select(&SEL_SEARCH_IMG).next();
        let title = img_el
            .and_then(|img| img.value().attr("alt"))
            .unwrap_or("None")
            .to_string();
        let image = img_el
            .and_then(|img| img.value().attr("src"))
            .unwrap_or("None")
            .to_string();

        let duration = duration_list
            .get(i)
            .cloned()
            .unwrap_or_else(|| "None".to_string());
        let views = views_list
            .get(i)
            .cloned()
            .unwrap_or_else(|| "None".to_string());

        let embed_id = link_rel
            .split('-')
            .next_back()
            .unwrap_or("")
            .trim_matches('/');
        let video = format!("https://xhamster.com/embed/{embed_id}");

        items.push(SearchItem {
            link: link_rel.to_string(),
            id,
            title: Some(title),
            image: Some(image),
            duration: Some(duration),
            rating: None,
            views: Some(views),
            uploader: None,
            video: Some(video),
        });
    }

    if items.is_empty() {
        return Err(AppError::Scraper("No result found".to_string()));
    }

    Ok(SearchResponse {
        success: true,
        data: items,
        source: url.to_string(),
    })
}
