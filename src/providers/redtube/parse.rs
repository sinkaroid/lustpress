use crate::error::AppError;
use crate::model::{SearchItem, SearchResponse, VideoDetails, VideoResponse};
use crate::util::{second_to_minute, static_selector};
use scraper::{Html, Selector};
use std::sync::LazyLock;

static SEL_CANONICAL: LazyLock<Selector> =
    LazyLock::new(|| static_selector("link[rel='canonical']"));
static SEL_OG_TITLE: LazyLock<Selector> =
    LazyLock::new(|| static_selector("meta[property='og:title']"));
static SEL_OG_IMAGE: LazyLock<Selector> =
    LazyLock::new(|| static_selector("meta[property='og:image']"));
static SEL_OG_DURATION: LazyLock<Selector> =
    LazyLock::new(|| static_selector("meta[property='og:video:duration']"));
static SEL_VIEWS: LazyLock<Selector> = LazyLock::new(|| static_selector("span.video_view_count"));
static SEL_RATING: LazyLock<Selector> =
    LazyLock::new(|| static_selector("div.rating_percent.js_rating_percent"));
static SEL_PUBLISH: LazyLock<Selector> =
    LazyLock::new(|| static_selector("span.video-infobox-date-added"));
static SEL_PLAYER: LazyLock<Selector> =
    LazyLock::new(|| static_selector("meta[name='twitter:player']"));
static SEL_TAGS: LazyLock<Selector> = LazyLock::new(|| {
    static_selector(
        "a.item.video_carousel_item.video_carousel_category, a.item.video_carousel_item.video_carousel_tag",
    )
});
static SEL_MODELS: LazyLock<Selector> =
    LazyLock::new(|| static_selector("div.pornstar-name.pornstarPopupWrapper a"));

static SEL_SEARCH_LINKS: LazyLock<Selector> = LazyLock::new(|| static_selector("a.video_link"));
static SEL_SEARCH_IMG: LazyLock<Selector> = LazyLock::new(|| static_selector("img"));
static SEL_SEARCH_DURATION: LazyLock<Selector> = LazyLock::new(|| static_selector("span.duration"));
static SEL_SEARCH_VIEWS: LazyLock<Selector> = LazyLock::new(|| static_selector("span.video_count"));

pub fn parse_redtube_get(body: &[u8]) -> Result<VideoResponse, AppError> {
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

    let id = link.split('/').nth(3).unwrap_or("None").to_string();

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

    let views = document
        .select(&SEL_VIEWS)
        .next()
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "None".to_string());

    let rating = document
        .select(&SEL_RATING)
        .next()
        .and_then(|el| el.value().attr("data-percent"))
        .map(|p| format!("{p}%"))
        .unwrap_or_else(|| "None".to_string());

    let publish = document
        .select(&SEL_PUBLISH)
        .next()
        .map(|el| {
            el.text()
                .collect::<String>()
                .replace("Published on ", "")
                .trim()
                .to_string()
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "None".to_string());

    let video = document
        .select(&SEL_PLAYER)
        .next()
        .and_then(|el| el.value().attr("content"))
        .unwrap_or("None")
        .to_string();

    let tags: Vec<String> = document
        .select(&SEL_TAGS)
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let mut models: Vec<String> = Vec::new();
    for el in document.select(&SEL_MODELS) {
        let name = el.text().collect::<String>().trim().to_string();
        if !name.is_empty()
            && !name.contains("Subscribe")
            && !name.contains("Rank")
            && !models.contains(&name)
        {
            models.push(name);
        }
    }

    Ok(VideoResponse {
        success: true,
        data: VideoDetails {
            title,
            id,
            image: Some(image.clone()),
            duration,
            views,
            rating: Some(rating.clone()),
            uploaded: publish,
            upvoted: Some(rating),
            downvoted: None,
            channel: None,
            models,
            tags,
        },
        source: link,
        assets: vec![video, image],
    })
}

pub fn parse_redtube_search(body: &[u8], url: &str) -> Result<SearchResponse, AppError> {
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

    let mut items = Vec::new();

    for (i, el) in document.select(&SEL_SEARCH_LINKS).enumerate() {
        let link_rel = el.value().attr("href").unwrap_or_default();
        if link_rel.contains("javascript:void(0)") {
            continue;
        }

        let id = link_rel.split('/').nth(1).unwrap_or("None").to_string();

        let img_el = el.select(&SEL_SEARCH_IMG).next();
        let title = img_el
            .and_then(|img| img.value().attr("alt"))
            .unwrap_or("None")
            .to_string();
        let image = img_el
            .and_then(|img| img.value().attr("data-src"))
            .unwrap_or("None")
            .to_string();

        if image.starts_with("data:image") {
            continue;
        }

        let duration_raw = el
            .select(&SEL_SEARCH_DURATION)
            .next()
            .map(|d| d.text().collect::<String>())
            .unwrap_or_default();

        let duration: String = duration_raw
            .split_whitespace()
            .map(|w| {
                w.chars()
                    .filter(|c| c.is_ascii_digit() || *c == ':')
                    .collect::<String>()
            })
            .filter(|w| w.contains(':'))
            .collect::<Vec<_>>()
            .join(" ");

        let views = views_list
            .get(i)
            .cloned()
            .unwrap_or_else(|| "None".to_string());
        let video = format!("https://embed.redtube.com/?id={id}");

        items.push(SearchItem {
            link: format!("https://www.redtube.com{link_rel}"),
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
