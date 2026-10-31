use crate::error::AppError;
use crate::model::{SearchItem, SearchResponse, VideoDetails, VideoResponse};
use crate::util::second_to_minute;
use regex::Regex;
use scraper::{Html, Selector};
use serde::Deserialize;
use std::sync::LazyLock;

static SEL_OG_URL: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("meta[property='og:url']").unwrap());
static SEL_OG_TITLE: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("meta[property='og:title']").unwrap());
static SEL_OG_IMAGE: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("meta[property='og:image']").unwrap());
static SEL_OG_DURATION: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("meta[property='og:duration']").unwrap());
static SEL_VIEWS: LazyLock<Selector> = LazyLock::new(|| Selector::parse("span.metadata").unwrap());
static SEL_RATING: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("span.rating-total-txt").unwrap());
static SEL_JSON_LD: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("script[type='application/ld+json']").unwrap());
static SEL_VOTE_ACTIONS: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("span.vote-actions span.value").unwrap());
static SEL_TAGS: LazyLock<Selector> = LazyLock::new(|| Selector::parse("a.is-keyword").unwrap());
static SEL_MODELS: LazyLock<Selector> = LazyLock::new(|| Selector::parse("a.is-pornstar").unwrap());
static SEL_EMBED_INPUT: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("input#copy-video-embed").unwrap());

static SEL_SEARCH_BLOCK: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("div.mozaique.cust-nb-cols div.thumb").unwrap());
static SEL_SEARCH_THUMB_UNDER: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("div.thumb-under").unwrap());
static SEL_SEARCH_A: LazyLock<Selector> = LazyLock::new(|| Selector::parse("a").unwrap());
static SEL_SEARCH_IMG: LazyLock<Selector> = LazyLock::new(|| Selector::parse("img").unwrap());
static SEL_SEARCH_DURATION: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("span.duration").unwrap());

static RE_THUMB_SLIDE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"html5player\.setThumbSlideBig\((?:'|")?(.*?)(?:'|")?\)"#).unwrap()
});
static RE_THUMB_169: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"html5player\.setThumbUrl169\((?:'|")?(.*?)(?:'|")?\)"#).unwrap()
});
static RE_VIDEO_URL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"html5player\.setVideoUrlHigh\((?:'|")?(.*?)(?:'|")?\)"#).unwrap()
});
static RE_VIDEO_RELATED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?s)var video_related\s*=\s*(\[.*?\]);"#).unwrap());
static RE_IFRAME_SRC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"src="([^"]+)""#).unwrap());

#[derive(Debug, Deserialize)]
pub struct XnxxRelatedRaw {
    pub u: String,
    pub t: String,
    pub i: String,
    pub d: String,
    pub n: String,
    pub r: String,
    pub id: serde_json::Value,
}

pub fn parse_xnxx_get(body: &[u8]) -> Result<VideoResponse, AppError> {
    let html_str = String::from_utf8_lossy(body);
    let end_pos = html_str
        .find("<footer")
        .or_else(|| html_str.find("id=\"footer\""))
        .unwrap_or(html_str.len());
    let document = Html::parse_document(&html_str[..end_pos]);

    let link = document
        .select(&SEL_OG_URL)
        .next()
        .and_then(|el| el.value().attr("content"))
        .unwrap_or("None")
        .to_string();

    let id = link.split(".com/").nth(1).unwrap_or("None").to_string();

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

    let duration_sec: u64 = document
        .select(&SEL_OG_DURATION)
        .next()
        .and_then(|el| el.value().attr("content"))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let duration = second_to_minute(duration_sec);

    let views_raw = document
        .select(&SEL_VIEWS)
        .next()
        .map(|el| el.text().collect::<String>())
        .unwrap_or_default();
    let views = views_raw
        .split('-')
        .nth(2)
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "None".to_string());

    let rating = document
        .select(&SEL_RATING)
        .next()
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "None".to_string());

    let mut publish = "None".to_string();
    if let Some(script) = document.select(&SEL_JSON_LD).next() {
        let text: String = script.text().collect();
        if let Some(pos) = text.find("uploadDate") {
            let slice = &text[pos..];
            if let Some(val) = slice.split('"').nth(2) {
                publish = val.replace(',', "").trim().to_string();
            }
        }
    }

    let vote_actions: Vec<String> = document
        .select(&SEL_VOTE_ACTIONS)
        .map(|el| el.text().collect::<String>().trim().to_string())
        .collect();

    let upvote = vote_actions
        .first()
        .cloned()
        .unwrap_or_else(|| "None".to_string());
    let downvote = vote_actions
        .get(1)
        .cloned()
        .unwrap_or_else(|| "None".to_string());

    let thumbnail = RE_THUMB_SLIDE
        .captures(&html_str)
        .map(|c| c[1].trim_matches('\'').trim_matches('"').to_string())
        .unwrap_or_else(|| "None".to_string());

    let bigimg = RE_THUMB_169
        .captures(&html_str)
        .map(|c| c[1].trim_matches('\'').trim_matches('"').to_string())
        .unwrap_or_else(|| "None".to_string());

    let video_url = RE_VIDEO_URL
        .captures(&html_str)
        .map(|c| c[1].trim_matches('\'').trim_matches('"').to_string())
        .unwrap_or_else(|| "None".to_string());

    let embed = document
        .select(&SEL_EMBED_INPUT)
        .next()
        .and_then(|el| el.value().attr("value"))
        .and_then(|val| RE_IFRAME_SRC.captures(val))
        .map(|c| c[1].to_string())
        .unwrap_or_else(|| "None".to_string());

    let tags: Vec<String> = document
        .select(&SEL_TAGS)
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let models: Vec<String> = document
        .select(&SEL_MODELS)
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let assets: Vec<String> = vec![embed, thumbnail, bigimg, video_url]
        .into_iter()
        .filter(|s| s != "None" && !s.is_empty())
        .collect();

    Ok(VideoResponse {
        success: true,
        data: VideoDetails {
            title,
            id,
            image: Some(image),
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
        assets,
    })
}

pub fn parse_xnxx_search(body: &[u8], url: &str) -> Result<SearchResponse, AppError> {
    let html_str = String::from_utf8_lossy(body);
    let end_pos = html_str
        .find("<footer")
        .or_else(|| html_str.find("id=\"footer\""))
        .unwrap_or(html_str.len());
    let document = Html::parse_document(&html_str[..end_pos]);

    let mut titles_durations = Vec::new();
    for el in document.select(&SEL_SEARCH_THUMB_UNDER) {
        let title = el
            .select(&SEL_SEARCH_A)
            .next()
            .and_then(|a| a.value().attr("title"))
            .unwrap_or("None")
            .to_string();

        let duration = el
            .select(&SEL_SEARCH_DURATION)
            .next()
            .map(|d| d.text().collect::<String>().trim().to_string())
            .unwrap_or_else(|| "None".to_string());

        titles_durations.push((title, duration));
    }

    let mut items = Vec::new();
    let thumbs: Vec<_> = document.select(&SEL_SEARCH_BLOCK).collect();

    for (i, el) in thumbs.iter().enumerate() {
        let href = el
            .select(&SEL_SEARCH_A)
            .next()
            .and_then(|a| a.value().attr("href"))
            .unwrap_or("None");

        if href.contains("THUMBNUM") || !href.contains("/video") {
            continue;
        }

        let img_el = el.select(&SEL_SEARCH_IMG).next();
        let video_id = img_el
            .and_then(|img| img.value().attr("data-videoid"))
            .unwrap_or("");

        let image = img_el
            .and_then(|img| {
                img.value()
                    .attr("data-src")
                    .or_else(|| img.value().attr("src"))
            })
            .unwrap_or("None")
            .to_string();

        let (title, duration) = titles_durations
            .get(i)
            .cloned()
            .unwrap_or_else(|| ("None".to_string(), "None".to_string()));

        let video = format!("https://www.xnxx.com/embedframe/{video_id}");

        items.push(SearchItem {
            link: format!("https://www.xnxx.com{href}"),
            id: href.to_string(),
            title: Some(title),
            image: Some(image),
            duration: Some(duration),
            rating: None,
            views: None,
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

pub fn parse_xnxx_related(body: &[u8], url: &str) -> Result<SearchResponse, AppError> {
    let html_str = String::from_utf8_lossy(body);

    let related_json = RE_VIDEO_RELATED
        .captures(&html_str)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str())
        .ok_or_else(|| AppError::Scraper("No result found".to_string()))?;

    let raw_list: Vec<XnxxRelatedRaw> = serde_json::from_str(related_json)
        .map_err(|e| AppError::Scraper(format!("Failed to parse related videos JSON: {e}")))?;

    if raw_list.is_empty() {
        return Err(AppError::Scraper("No result found".to_string()));
    }

    let items: Vec<SearchItem> = raw_list
        .into_iter()
        .map(|rel| {
            let vid_str = match &rel.id {
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::String(s) => s.clone(),
                _ => "".to_string(),
            };

            let trimmed_id = rel.u.trim_matches('/').to_string();

            SearchItem {
                link: format!("https://www.xnxx.com{}", rel.u),
                id: trimmed_id,
                title: Some(rel.t),
                image: Some(rel.i),
                duration: Some(rel.d),
                rating: None,
                views: Some(format!("{}, {}", rel.n, rel.r)),
                uploader: None,
                video: Some(format!("https://www.xnxx.com/embedframe/{vid_str}")),
            }
        })
        .collect();

    Ok(SearchResponse {
        success: true,
        data: items,
        source: url.to_string(),
    })
}
