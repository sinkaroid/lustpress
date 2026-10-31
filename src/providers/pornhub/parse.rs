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
static SEL_OG_VIDEO: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("meta[property='og:video:url']").unwrap());
static SEL_VIEWS: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("div.views > span.count").unwrap());
static SEL_RATING: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("div.ratingPercent > span.percent").unwrap());
static SEL_VIDEO_INFO: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("div.videoInfo").unwrap());
static SEL_UPVOTE: LazyLock<Selector> = LazyLock::new(|| Selector::parse("span.votesUp").unwrap());
static SEL_DOWNVOTE: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("span.votesDown").unwrap());
static SEL_TAGS: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("div.video-info-row a").unwrap());
static SEL_MODELS: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("div.pornstarsWrapper.js-pornstarsWrapper a").unwrap());

static SEL_SEARCH_WRAP: LazyLock<Selector> = LazyLock::new(|| Selector::parse("div.wrap").unwrap());
static SEL_SEARCH_A: LazyLock<Selector> = LazyLock::new(|| Selector::parse("a").unwrap());
static SEL_SEARCH_IMG: LazyLock<Selector> = LazyLock::new(|| Selector::parse("img").unwrap());
static SEL_SEARCH_DURATION: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("var.duration").unwrap());
static SEL_SEARCH_VIEWS: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("div.videoDetailsBlock span.views").unwrap());

pub fn parse_pornhub_get(body: &[u8]) -> Result<VideoResponse, AppError> {
    let html_str = String::from_utf8_lossy(body);
    let end_pos = html_str
        .find("id=\"cmtWrapper\"")
        .or_else(|| html_str.find("class=\"recommendedVideos\""))
        .or_else(|| html_str.find("<footer"))
        .unwrap_or(html_str.len());
    let document = Html::parse_document(&html_str[..end_pos]);

    let link = document
        .select(&SEL_CANONICAL)
        .next()
        .and_then(|el| el.value().attr("href"))
        .unwrap_or("None")
        .to_string();

    let id = link.split('=').nth(1).unwrap_or("None").to_string();

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
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "None".to_string());

    let video_info = document
        .select(&SEL_VIDEO_INFO)
        .next()
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "None".to_string());

    let upvote = document
        .select(&SEL_UPVOTE)
        .next()
        .and_then(|el| el.value().attr("data-rating"))
        .unwrap_or("None")
        .to_string();

    let downvote = document
        .select(&SEL_DOWNVOTE)
        .next()
        .and_then(|el| el.value().attr("data-rating"))
        .unwrap_or("None")
        .to_string();

    let video = document
        .select(&SEL_OG_VIDEO)
        .next()
        .and_then(|el| el.value().attr("content"))
        .unwrap_or("None")
        .to_string();

    let mut raw_tags: Vec<String> = document
        .select(&SEL_TAGS)
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if !raw_tags.is_empty() {
        raw_tags.remove(0); // Match JS tags.shift()
    }
    let tags: Vec<String> = raw_tags
        .into_iter()
        .filter(|t| t != "Suggest" && t != " Suggest")
        .collect();

    let models: Vec<String> = document
        .select(&SEL_MODELS)
        .map(|el| el.text().collect::<String>().trim().to_string())
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
            uploaded: video_info,
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

pub fn parse_pornhub_search(body: &[u8], url: &str) -> Result<SearchResponse, AppError> {
    let html_str = String::from_utf8_lossy(body);
    let is_related = url.contains("view_video.php");

    // Do not prune at pagination for view_video.php (?related) since comment
    // pagination or tab controls may precede recommended video containers (div.wrap).
    let end_pos = if is_related {
        html_str
            .find("<footer")
            .or_else(|| html_str.find("class=\"footer-wrap\""))
            .unwrap_or(html_str.len())
    } else {
        html_str
            .find("class=\"page_numbers\"")
            .or_else(|| html_str.find("class=\"pagination\""))
            .or_else(|| html_str.find("<footer"))
            .or_else(|| html_str.find("class=\"footer-wrap\""))
            .unwrap_or(html_str.len())
    };
    let document = Html::parse_document(&html_str[..end_pos]);

    let mut items = Vec::new();

    for el in document.select(&SEL_SEARCH_WRAP) {
        let link_el = el.select(&SEL_SEARCH_A).next();
        let link_rel = link_el
            .and_then(|a| a.value().attr("href"))
            .unwrap_or_default();
        let title = link_el
            .and_then(|a| a.value().attr("title"))
            .unwrap_or("None")
            .to_string();

        if link_rel.contains("javascript:void(0)") {
            continue;
        }

        let id = link_rel.split('=').nth(1).unwrap_or("None").to_string();

        let image = el
            .select(&SEL_SEARCH_IMG)
            .next()
            .and_then(|img| img.value().attr("src"))
            .unwrap_or("None")
            .to_string();

        if image.starts_with("data:image") {
            continue;
        }

        let duration = el
            .select(&SEL_SEARCH_DURATION)
            .next()
            .map(|d| d.text().collect::<String>().trim().to_string())
            .unwrap_or_else(|| "None".to_string());

        let views = el
            .select(&SEL_SEARCH_VIEWS)
            .next()
            .map(|v| v.text().collect::<String>().trim().to_string())
            .unwrap_or_else(|| "None".to_string());

        let video = format!("https://www.pornhub.com/embed/{id}");

        items.push(SearchItem {
            link: format!("https://www.pornhub.com{link_rel}"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pornhub_search_with_pagination() {
        let mock_html = r##"
        <html>
        <body>
            <div class="wrap">
                <a href="/view_video.php?viewkey=ph12345" title="Sample Search Video"></a>
                <img src="https://di.phncdn.com/1.jpg" />
                <var class="duration">12:34</var>
                <div class="videoDetailsBlock"><span class="views">50K</span></div>
            </div>
            <div class="page_numbers"><span>1</span><a href="#">2</a></div>
            <div class="huge-unneeded-tags">Should be pruned</div>
            <footer>Footer</footer>
        </body>
        </html>
        "##;
        let res = parse_pornhub_search(
            mock_html.as_bytes(),
            "https://www.pornhub.com/video/search?search=test",
        )
        .expect("Search parse should succeed");
        assert_eq!(res.data.len(), 1);
        assert_eq!(res.data[0].id, "ph12345");
        assert_eq!(res.data[0].title.as_deref(), Some("Sample Search Video"));
    }

    #[test]
    fn test_parse_pornhub_related_with_comments_pagination() {
        // view_video.php may have comment pagination before related video cards
        let mock_html = r##"
        <html>
        <body>
            <div class="video-container">Player</div>
            <div id="cmtWrapper">
                <div class="pagination">Comment page 1, 2, 3</div>
            </div>
            <div class="relatedVideos">
                <div class="wrap">
                    <a href="/view_video.php?viewkey=phrelated99" title="Related Video 1"></a>
                    <img src="https://di.phncdn.com/related.jpg" />
                    <var class="duration">08:15</var>
                    <div class="videoDetailsBlock"><span class="views">120K</span></div>
                </div>
            </div>
            <footer>Footer</footer>
        </body>
        </html>
        "##;
        let res = parse_pornhub_search(
            mock_html.as_bytes(),
            "https://www.pornhub.com/view_video.php?viewkey=phmain123",
        )
        .expect("Related parse should not be broken by comment pagination");
        assert_eq!(res.data.len(), 1);
        assert_eq!(res.data[0].id, "phrelated99");
        assert_eq!(res.data[0].title.as_deref(), Some("Related Video 1"));
    }
}
