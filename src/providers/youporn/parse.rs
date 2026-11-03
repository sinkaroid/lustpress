use crate::error::AppError;
use crate::model::{SearchItem, SearchResponse, VideoDetails, VideoResponse};
use crate::util::second_to_minute;
use scraper::{Html, Selector};
use std::sync::LazyLock;

static SEL_CANONICAL: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("link[rel='canonical']").unwrap());
static SEL_OG_TITLE: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("meta[property='og:title']").unwrap());
static SEL_OG_IMAGE: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("meta[property='og:image']").unwrap());
static SEL_OG_DURATION: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("meta[property='video:duration']").unwrap());
static SEL_PUBLISH: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("span.publishedDate").unwrap());
static SEL_VIEWS: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("span.tm_infoValue").unwrap());
static SEL_RATING: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("span.tm_rating_percent").unwrap());
static SEL_TAGS: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse("div.js_scrollableContent a.bubble-porntag, a[data-espnode='category_tag'], a[data-espnode='porntag_tag']").unwrap()
});
static SEL_MODELS: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("#metaDataPornstarInfo a.tm_pornstar_link span").unwrap());

static SEL_SEARCH_CARDS: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("article.video-box.pc").unwrap());
static SEL_SEARCH_LINK: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("a.tm_video_link").unwrap());
static SEL_SEARCH_TITLE: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("a.tm_video_title span").unwrap());
static SEL_SEARCH_IMG: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("img.thumb-image").unwrap());
static SEL_SEARCH_DURATION: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("div.tm_video_duration span").unwrap());
static SEL_SEARCH_VIEWS: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(".view-rating-container .info-views").unwrap());

pub fn parse_youporn_get(body: &[u8]) -> Result<VideoResponse, AppError> {
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

    let id = link
        .replace("https://www.youporn.com/watch/", "")
        .trim_end_matches('/')
        .to_string();

    let title = document
        .select(&SEL_OG_TITLE)
        .next()
        .and_then(|el| el.value().attr("content"))
        .unwrap_or("None")
        .trim()
        .to_string();

    if id.contains("video-inactive") || title == "None" || link.contains("video-inactive") {
        return Err(AppError::Scraper("Video not found or inactive".to_string()));
    }

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

    let publish = document
        .select(&SEL_PUBLISH)
        .next()
        .map(|el| {
            el.text()
                .collect::<String>()
                .replace("Published on", "")
                .trim()
                .to_string()
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "None".to_string());

    let views = document
        .select(&SEL_VIEWS)
        .next()
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "0".to_string());

    let rating = document
        .select(&SEL_RATING)
        .next()
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "0".to_string());

    let video = format!("https://www.youporn.com/embed/{id}");

    let tags: Vec<String> = document
        .select(&SEL_TAGS)
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let models: Vec<String> = document
        .select(&SEL_MODELS)
        .map(|el| {
            el.text()
                .collect::<String>()
                .replace(',', "")
                .trim()
                .to_string()
        })
        .filter(|s| !s.is_empty())
        .collect();

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
            upvoted: Some("None".to_string()),
            downvoted: Some("None".to_string()),
            channel: None,
            models,
            tags,
        },
        source: link,
        assets: vec![video, image],
    })
}

pub fn parse_youporn_search(body: &[u8], url: &str) -> Result<SearchResponse, AppError> {
    let html_str = String::from_utf8_lossy(body);
    let end_pos = html_str
        .find("<footer")
        .or_else(|| html_str.find("id=\"footer\""))
        .unwrap_or(html_str.len());
    let document = Html::parse_document(&html_str[..end_pos]);

    let mut items = Vec::new();

    for el in document.select(&SEL_SEARCH_CARDS) {
        let link_rel = el
            .select(&SEL_SEARCH_LINK)
            .next()
            .and_then(|a| a.value().attr("href"))
            .unwrap_or_default();

        let id = link_rel.split('/').nth(2).unwrap_or_default().to_string();

        let title = el
            .select(&SEL_SEARCH_TITLE)
            .next()
            .map(|t| t.text().collect::<String>().trim().to_string());

        let image = el
            .select(&SEL_SEARCH_IMG)
            .next()
            .and_then(|img| {
                img.value()
                    .attr("data-src")
                    .or_else(|| img.value().attr("data-poster"))
                    .or_else(|| img.value().attr("src"))
            })
            .map(|s| s.to_string());

        let duration = el
            .select(&SEL_SEARCH_DURATION)
            .next()
            .map(|t| t.text().collect::<String>().trim().to_string());

        let views = el
            .select(&SEL_SEARCH_VIEWS)
            .next()
            .map(|t| {
                let txt = t.text().collect::<String>();
                txt.replace("Views:", "")
                    .replace("views:", "")
                    .trim()
                    .to_string()
            })
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "None".to_string());

        let video = format!("https://www.youporn.com/embed/{id}");

        items.push(SearchItem {
            link: format!("https://www.youporn.com{link_rel}"),
            id,
            title,
            image,
            duration,
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
