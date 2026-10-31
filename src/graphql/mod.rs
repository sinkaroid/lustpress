use crate::error::AppError;
use crate::model::{SearchResponse, VideoResponse};
use crate::state::AppState;
use crate::util::encode_query;
use async_graphql::{Context, EmptyMutation, EmptySubscription, Object, Schema};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::response::IntoResponse;

pub type AppSchema = Schema<QueryRoot, EmptyMutation, EmptySubscription>;

pub fn build_schema(state: AppState) -> AppSchema {
    Schema::build(QueryRoot, EmptyMutation, EmptySubscription)
        .data(state)
        .finish()
}

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn pornhub(&self) -> PornhubQueries {
        PornhubQueries
    }
    async fn xnxx(&self) -> XnxxQueries {
        XnxxQueries
    }
    async fn redtube(&self) -> RedtubeQueries {
        RedtubeQueries
    }
    async fn xvideos(&self) -> XvideosQueries {
        XvideosQueries
    }
    async fn xhamster(&self) -> XhamsterQueries {
        XhamsterQueries
    }
    async fn youporn(&self) -> YoupornQueries {
        YoupornQueries
    }
    async fn eporner(&self) -> EpornerQueries {
        EpornerQueries
    }
    async fn txxx(&self) -> TxxxQueries {
        TxxxQueries
    }
}

// 1. Pornhub
#[derive(Default)]
pub struct PornhubQueries;

#[Object(name = "PornhubQueries")]
impl PornhubQueries {
    async fn get(&self, ctx: &Context<'_>, id: Option<String>) -> Result<VideoResponse, AppError> {
        let state = ctx
            .data::<AppState>()
            .map_err(|e| AppError::Internal(e.message))?;
        let video_id = id.ok_or_else(|| AppError::Validation("id is required".to_string()))?;
        let url = format!("https://www.pornhub.com/view_video.php?viewkey={video_id}");
        let body = state.fetch.fetch_body(&url).await?;
        crate::providers::pornhub::parse::parse_pornhub_get(&body)
    }

    async fn search(
        &self,
        ctx: &Context<'_>,
        key: String,
        page: Option<String>,
    ) -> Result<SearchResponse, AppError> {
        let state = ctx
            .data::<AppState>()
            .map_err(|e| AppError::Internal(e.message))?;
        let p: u32 = page.as_deref().unwrap_or("1").parse().unwrap_or(1);
        let encoded = encode_query(&key);
        let url = if p <= 1 {
            format!("https://www.pornhub.com/video/search?search={encoded}")
        } else {
            format!("https://www.pornhub.com/video/search?search={encoded}&page={p}")
        };
        let body = state.fetch.fetch_body(&url).await?;
        crate::providers::pornhub::parse::parse_pornhub_search(&body, &url)
    }

    async fn random(&self, ctx: &Context<'_>) -> Result<VideoResponse, AppError> {
        let state = ctx
            .data::<AppState>()
            .map_err(|e| AppError::Internal(e.message))?;
        let url = "https://www.pornhub.com/video/random";
        let body = state.fetch.fetch_body(url).await?;
        crate::providers::pornhub::parse::parse_pornhub_get(&body)
    }

    async fn related(
        &self,
        ctx: &Context<'_>,
        id: Option<String>,
    ) -> Result<SearchResponse, AppError> {
        let state = ctx
            .data::<AppState>()
            .map_err(|e| AppError::Internal(e.message))?;
        let video_id = id.ok_or_else(|| AppError::Validation("id is required".to_string()))?;
        let url = format!("https://www.pornhub.com/view_video.php?viewkey={video_id}");
        let body = state.fetch.fetch_body(&url).await?;
        crate::providers::pornhub::parse::parse_pornhub_search(&body, &url)
    }
}

// 2. XNXX
#[derive(Default)]
pub struct XnxxQueries;

#[Object(name = "XnxxQueries")]
impl XnxxQueries {
    async fn get(&self, ctx: &Context<'_>, id: Option<String>) -> Result<VideoResponse, AppError> {
        let state = ctx
            .data::<AppState>()
            .map_err(|e| AppError::Internal(e.message))?;
        let video_id = id.ok_or_else(|| AppError::Validation("id is required".to_string()))?;
        let url = if video_id.starts_with("video") {
            format!("https://www.xnxx.com/{video_id}")
        } else {
            format!("https://www.xnxx.com/video-{video_id}")
        };
        let body = state.fetch.fetch_body(&url).await?;
        crate::providers::xnxx::parse::parse_xnxx_get(&body)
    }

    async fn search(
        &self,
        ctx: &Context<'_>,
        key: String,
        page: Option<String>,
    ) -> Result<SearchResponse, AppError> {
        let state = ctx
            .data::<AppState>()
            .map_err(|e| AppError::Internal(e.message))?;
        let p: u32 = page.as_deref().unwrap_or("1").parse().unwrap_or(1);
        let encoded = encode_query(&key);
        let url = if p <= 1 {
            format!("https://www.xnxx.com/search/{encoded}")
        } else {
            format!("https://www.xnxx.com/search/{encoded}/{p}")
        };
        let body = state.fetch.fetch_body(&url).await?;
        crate::providers::xnxx::parse::parse_xnxx_search(&body, &url)
    }

    async fn random(&self, ctx: &Context<'_>) -> Result<VideoResponse, AppError> {
        let state = ctx
            .data::<AppState>()
            .map_err(|e| AppError::Internal(e.message))?;
        let home_url = "https://www.xnxx.com/";
        let body = state.fetch.fetch_body(home_url).await?;
        let search_res = crate::providers::xnxx::parse::parse_xnxx_search(&body, home_url)?;
        if search_res.data.is_empty() {
            return Err(AppError::Scraper("No videos found on homepage".to_string()));
        }
        let idx = (chrono::Local::now().timestamp_subsec_nanos() as usize) % search_res.data.len();
        let item = &search_res.data[idx];
        let url = if item.id.starts_with("video") {
            format!("https://www.xnxx.com/{}", item.id)
        } else {
            format!("https://www.xnxx.com/video-{}", item.id)
        };
        let video_body = state.fetch.fetch_body(&url).await?;
        crate::providers::xnxx::parse::parse_xnxx_get(&video_body)
    }

    async fn related(
        &self,
        ctx: &Context<'_>,
        id: Option<String>,
    ) -> Result<SearchResponse, AppError> {
        let state = ctx
            .data::<AppState>()
            .map_err(|e| AppError::Internal(e.message))?;
        let video_id = id.ok_or_else(|| AppError::Validation("id is required".to_string()))?;
        let url = if video_id.starts_with("video") {
            format!("https://www.xnxx.com/{video_id}")
        } else {
            format!("https://www.xnxx.com/video-{video_id}")
        };
        let body = state.fetch.fetch_body(&url).await?;
        crate::providers::xnxx::parse::parse_xnxx_related(&body, &url)
    }
}

// 3. Redtube
#[derive(Default)]
pub struct RedtubeQueries;

#[Object(name = "RedtubeQueries")]
impl RedtubeQueries {
    async fn get(&self, ctx: &Context<'_>, id: Option<String>) -> Result<VideoResponse, AppError> {
        let state = ctx
            .data::<AppState>()
            .map_err(|e| AppError::Internal(e.message))?;
        let video_id = id.ok_or_else(|| AppError::Validation("id is required".to_string()))?;
        let url = format!(
            "https://www.redtube.com/{}",
            video_id.trim_start_matches('/')
        );
        let body = state.fetch.fetch_body(&url).await?;
        crate::providers::redtube::parse::parse_redtube_get(&body)
    }

    async fn search(
        &self,
        ctx: &Context<'_>,
        key: String,
        page: Option<String>,
    ) -> Result<SearchResponse, AppError> {
        let state = ctx
            .data::<AppState>()
            .map_err(|e| AppError::Internal(e.message))?;
        let p: u32 = page.as_deref().unwrap_or("1").parse().unwrap_or(1);
        let encoded = encode_query(&key);
        let url = if p <= 1 {
            format!("https://www.redtube.com/?search={encoded}")
        } else {
            format!("https://www.redtube.com/?search={encoded}&page={p}")
        };
        let body = state.fetch.fetch_body(&url).await?;
        crate::providers::redtube::parse::parse_redtube_search(&body, &url)
    }

    async fn random(&self, ctx: &Context<'_>) -> Result<VideoResponse, AppError> {
        let state = ctx
            .data::<AppState>()
            .map_err(|e| AppError::Internal(e.message))?;
        let home_url = "https://www.redtube.com/";
        let body = state.fetch.fetch_body(home_url).await?;
        let search_res = crate::providers::redtube::parse::parse_redtube_search(&body, home_url)?;
        if search_res.data.is_empty() {
            return Err(AppError::Scraper("No videos found on homepage".to_string()));
        }
        let idx = (chrono::Local::now().timestamp_subsec_nanos() as usize) % search_res.data.len();
        let item = &search_res.data[idx];
        let url = format!("https://www.redtube.com/{}", item.id);
        let video_body = state.fetch.fetch_body(&url).await?;
        crate::providers::redtube::parse::parse_redtube_get(&video_body)
    }

    async fn related(
        &self,
        ctx: &Context<'_>,
        id: Option<String>,
    ) -> Result<SearchResponse, AppError> {
        let state = ctx
            .data::<AppState>()
            .map_err(|e| AppError::Internal(e.message))?;
        let video_id = id.ok_or_else(|| AppError::Validation("id is required".to_string()))?;
        let url = format!(
            "https://www.redtube.com/{}",
            video_id.trim_start_matches('/')
        );
        let body = state.fetch.fetch_body(&url).await?;
        crate::providers::redtube::parse::parse_redtube_search(&body, &url)
    }
}

// 4. Xvideos
#[derive(Default)]
pub struct XvideosQueries;

#[Object(name = "XvideosQueries")]
impl XvideosQueries {
    async fn get(&self, ctx: &Context<'_>, id: Option<String>) -> Result<VideoResponse, AppError> {
        let state = ctx
            .data::<AppState>()
            .map_err(|e| AppError::Internal(e.message))?;
        let video_id = id.ok_or_else(|| AppError::Validation("id is required".to_string()))?;
        let clean = video_id.trim_start_matches('/');
        let url = if clean.starts_with("video") {
            format!("https://www.xvideos.com/{clean}")
        } else {
            format!("https://www.xvideos.com/video{clean}")
        };
        let body = state.fetch.fetch_body(&url).await?;
        crate::providers::xvideos::parse::parse_xvideos_get(&body)
    }

    async fn search(
        &self,
        ctx: &Context<'_>,
        key: String,
        page: Option<String>,
    ) -> Result<SearchResponse, AppError> {
        let state = ctx
            .data::<AppState>()
            .map_err(|e| AppError::Internal(e.message))?;
        let p: u32 = page.as_deref().unwrap_or("1").parse().unwrap_or(1);
        let encoded = encode_query(&key);
        let url = if p <= 1 {
            format!("https://www.xvideos.com/?k={encoded}")
        } else {
            format!("https://www.xvideos.com/?k={encoded}&p={p}")
        };
        let body = state.fetch.fetch_body(&url).await?;
        crate::providers::xvideos::parse::parse_xvideos_search(&body, &url)
    }

    async fn random(&self, ctx: &Context<'_>) -> Result<VideoResponse, AppError> {
        let state = ctx
            .data::<AppState>()
            .map_err(|e| AppError::Internal(e.message))?;
        let home_url = "https://www.xvideos.com/";
        let body = state.fetch.fetch_body(home_url).await?;
        let search_res = crate::providers::xvideos::parse::parse_xvideos_search(&body, home_url)?;
        if search_res.data.is_empty() {
            return Err(AppError::Scraper("No videos found on homepage".to_string()));
        }
        let idx = (chrono::Local::now().timestamp_subsec_nanos() as usize) % search_res.data.len();
        let item = &search_res.data[idx];
        let clean = item.id.trim_start_matches('/');
        let url = if clean.starts_with("video") {
            format!("https://www.xvideos.com/{clean}")
        } else {
            format!("https://www.xvideos.com/video{clean}")
        };
        let video_body = state.fetch.fetch_body(&url).await?;
        crate::providers::xvideos::parse::parse_xvideos_get(&video_body)
    }

    async fn related(
        &self,
        ctx: &Context<'_>,
        id: Option<String>,
    ) -> Result<SearchResponse, AppError> {
        let state = ctx
            .data::<AppState>()
            .map_err(|e| AppError::Internal(e.message))?;
        let video_id = id.ok_or_else(|| AppError::Validation("id is required".to_string()))?;
        let clean = video_id.trim_start_matches('/');
        let url = if clean.starts_with("video") {
            format!("https://www.xvideos.com/{clean}")
        } else {
            format!("https://www.xvideos.com/video{clean}")
        };
        let body = state.fetch.fetch_body(&url).await?;
        crate::providers::xvideos::parse::parse_xvideos_related(&body, &url)
    }
}

// 5. xHamster
#[derive(Default)]
pub struct XhamsterQueries;

#[Object(name = "XhamsterQueries")]
impl XhamsterQueries {
    async fn get(&self, ctx: &Context<'_>, id: Option<String>) -> Result<VideoResponse, AppError> {
        let state = ctx
            .data::<AppState>()
            .map_err(|e| AppError::Internal(e.message))?;
        let video_id = id.ok_or_else(|| AppError::Validation("id is required".to_string()))?;
        let url = format!(
            "https://xhamster.com/videos/{}",
            video_id.trim_start_matches('/')
        );
        let body = state.fetch.fetch_body(&url).await?;
        crate::providers::xhamster::parse::parse_xhamster_get(&body)
    }

    async fn search(
        &self,
        ctx: &Context<'_>,
        key: String,
        page: Option<String>,
    ) -> Result<SearchResponse, AppError> {
        let state = ctx
            .data::<AppState>()
            .map_err(|e| AppError::Internal(e.message))?;
        let p: u32 = page.as_deref().unwrap_or("1").parse().unwrap_or(1);
        let encoded = encode_query(&key);
        let url = if p <= 1 {
            format!("https://xhamster.com/search/{encoded}")
        } else {
            format!("https://xhamster.com/search/{encoded}?page={p}")
        };
        let body = state.fetch.fetch_body(&url).await?;
        crate::providers::xhamster::parse::parse_xhamster_search(&body, &url)
    }

    async fn random(&self, ctx: &Context<'_>) -> Result<VideoResponse, AppError> {
        let state = ctx
            .data::<AppState>()
            .map_err(|e| AppError::Internal(e.message))?;
        let newest_url = "https://xhamster.com/newest";
        let body = state.fetch.fetch_body(newest_url).await?;
        let search_res =
            crate::providers::xhamster::parse::parse_xhamster_search(&body, newest_url)?;
        if search_res.data.is_empty() {
            return Err(AppError::Scraper(
                "No videos found on newest page".to_string(),
            ));
        }
        let idx = (chrono::Local::now().timestamp_subsec_nanos() as usize) % search_res.data.len();
        let item = &search_res.data[idx];
        let url = format!("https://xhamster.com/videos/{}", item.id);
        let video_body = state.fetch.fetch_body(&url).await?;
        crate::providers::xhamster::parse::parse_xhamster_get(&video_body)
    }

    async fn related(
        &self,
        ctx: &Context<'_>,
        id: Option<String>,
    ) -> Result<SearchResponse, AppError> {
        let state = ctx
            .data::<AppState>()
            .map_err(|e| AppError::Internal(e.message))?;
        let video_id = id.ok_or_else(|| AppError::Validation("id is required".to_string()))?;
        let url = format!("https://xhamster.com/{}", video_id.trim_start_matches('/'));
        let body = state.fetch.fetch_body(&url).await?;
        crate::providers::xhamster::parse::parse_xhamster_search(&body, &url)
    }
}

// 6. YouPorn
#[derive(Default)]
pub struct YoupornQueries;

#[Object(name = "YoupornQueries")]
impl YoupornQueries {
    async fn get(&self, ctx: &Context<'_>, id: Option<String>) -> Result<VideoResponse, AppError> {
        let state = ctx
            .data::<AppState>()
            .map_err(|e| AppError::Internal(e.message))?;
        let video_id = id.ok_or_else(|| AppError::Validation("id is required".to_string()))?;
        let url = format!(
            "https://www.youporn.com/watch/{}",
            video_id.trim_start_matches('/')
        );
        let body = state.fetch.fetch_body(&url).await?;
        crate::providers::youporn::parse::parse_youporn_get(&body)
    }

    async fn search(
        &self,
        ctx: &Context<'_>,
        key: String,
        page: Option<String>,
    ) -> Result<SearchResponse, AppError> {
        let state = ctx
            .data::<AppState>()
            .map_err(|e| AppError::Internal(e.message))?;
        let p: u32 = page.as_deref().unwrap_or("1").parse().unwrap_or(1);
        let encoded = encode_query(&key);
        let url = if p <= 1 {
            format!("https://www.youporn.com/search/?query={encoded}")
        } else {
            format!("https://www.youporn.com/search/?query={encoded}&page={p}")
        };
        let body = state.fetch.fetch_body(&url).await?;
        crate::providers::youporn::parse::parse_youporn_search(&body, &url)
    }

    async fn random(&self, ctx: &Context<'_>) -> Result<VideoResponse, AppError> {
        let state = ctx
            .data::<AppState>()
            .map_err(|e| AppError::Internal(e.message))?;
        let home_url = "https://www.youporn.com/";
        let body = state.fetch.fetch_body(home_url).await?;
        let search_res = crate::providers::youporn::parse::parse_youporn_search(&body, home_url)?;
        if search_res.data.is_empty() {
            return Err(AppError::Scraper("No videos found on homepage".to_string()));
        }
        let idx = (chrono::Local::now().timestamp_subsec_nanos() as usize) % search_res.data.len();
        let item = &search_res.data[idx];
        let url = format!("https://www.youporn.com/watch/{}", item.id);
        let video_body = state.fetch.fetch_body(&url).await?;
        crate::providers::youporn::parse::parse_youporn_get(&video_body)
    }

    async fn related(
        &self,
        ctx: &Context<'_>,
        id: Option<String>,
    ) -> Result<SearchResponse, AppError> {
        let state = ctx
            .data::<AppState>()
            .map_err(|e| AppError::Internal(e.message))?;
        let video_id = id.ok_or_else(|| AppError::Validation("id is required".to_string()))?;
        let url = format!(
            "https://www.youporn.com/watch/{}",
            video_id.trim_start_matches('/')
        );
        let body = state.fetch.fetch_body(&url).await?;
        crate::providers::youporn::parse::parse_youporn_search(&body, &url)
    }
}

// 7. Eporner
#[derive(Default)]
pub struct EpornerQueries;

#[Object(name = "EpornerQueries")]
impl EpornerQueries {
    async fn get(&self, ctx: &Context<'_>, id: Option<String>) -> Result<VideoResponse, AppError> {
        let state = ctx
            .data::<AppState>()
            .map_err(|e| AppError::Internal(e.message))?;
        let video_id = id.ok_or_else(|| AppError::Validation("id is required".to_string()))?;
        let clean_id = crate::providers::eporner::clean_eporner_id(&video_id);
        let url = format!("https://www.eporner.com/api/v2/video/id/?id={clean_id}&format=json");
        let body = state.fetch.fetch_body(&url).await?;
        crate::providers::eporner::parse::parse_eporner_get(&body, &url)
    }

    async fn search(
        &self,
        ctx: &Context<'_>,
        key: String,
        page: Option<String>,
    ) -> Result<SearchResponse, AppError> {
        let state = ctx
            .data::<AppState>()
            .map_err(|e| AppError::Internal(e.message))?;
        let p: u32 = page.as_deref().unwrap_or("1").parse().unwrap_or(1);
        let encoded = encode_query(&key);
        let api_url = format!(
            "https://www.eporner.com/api/v2/video/search/?query={encoded}&page={p}&per_page=30&format=json"
        );
        let web_url = if p <= 1 {
            format!("https://www.eporner.com/search/{encoded}/")
        } else {
            format!("https://www.eporner.com/search/{encoded}/{p}/")
        };
        let body = state.fetch.fetch_body(&api_url).await?;
        crate::providers::eporner::parse::parse_eporner_search(&body, &web_url)
    }

    async fn random(&self, ctx: &Context<'_>) -> Result<VideoResponse, AppError> {
        let state = ctx
            .data::<AppState>()
            .map_err(|e| AppError::Internal(e.message))?;
        let weekly_api_url =
            "https://www.eporner.com/api/v2/video/search/?order=top-weekly&per_page=30&format=json";
        let weekly_web_url = "https://www.eporner.com/cat/all/SORT-top-weekly/";
        let body = state.fetch.fetch_body(weekly_api_url).await?;
        let search_res =
            crate::providers::eporner::parse::parse_eporner_search(&body, weekly_web_url)?;
        if search_res.data.is_empty() {
            return Err(AppError::Scraper("No weekly top videos found".to_string()));
        }
        let idx = (chrono::Local::now().timestamp_subsec_nanos() as usize) % search_res.data.len();
        let item = &search_res.data[idx];
        let clean_id = crate::providers::eporner::clean_eporner_id(&item.id);
        let url = format!("https://www.eporner.com/api/v2/video/id/?id={clean_id}&format=json");
        let video_body = state.fetch.fetch_body(&url).await?;
        crate::providers::eporner::parse::parse_eporner_get(&video_body, &url)
    }

    async fn related(
        &self,
        ctx: &Context<'_>,
        id: Option<String>,
    ) -> Result<SearchResponse, AppError> {
        let state = ctx
            .data::<AppState>()
            .map_err(|e| AppError::Internal(e.message))?;
        let video_id = id.ok_or_else(|| AppError::Validation("id is required".to_string()))?;
        let clean_id = crate::providers::eporner::clean_eporner_id(&video_id);
        let info_url =
            format!("https://www.eporner.com/api/v2/video/id/?id={clean_id}&format=json");
        let body = state.fetch.fetch_body(&info_url).await?;
        let video_info = crate::providers::eporner::parse::parse_eporner_video_info(&body)?;

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
        crate::providers::eporner::parse::parse_eporner_search(&search_body, &web_url)
    }
}

// 8. TXXX
#[derive(Default)]
pub struct TxxxQueries;

#[Object(name = "TxxxQueries")]
impl TxxxQueries {
    async fn get(&self, ctx: &Context<'_>, id: Option<String>) -> Result<VideoResponse, AppError> {
        let state = ctx
            .data::<AppState>()
            .map_err(|e| AppError::Internal(e.message))?;
        let video_id = id.ok_or_else(|| AppError::Validation("id is required".to_string()))?;
        let id_num: u64 = video_id
            .trim()
            .parse()
            .map_err(|_| AppError::Validation("Invalid video id: must be numeric".to_string()))?;
        let million = (id_num / 1_000_000) * 1_000_000;
        let thousand = (id_num / 1_000) * 1_000;
        let api_url =
            format!("https://txxx.com/api/json/video/86400/{million}/{thousand}/{id_num}.json");
        let body = state.fetch.fetch_body(&api_url).await?;
        crate::providers::txxx::parse::parse_txxx_get(&body)
    }

    async fn search(
        &self,
        ctx: &Context<'_>,
        key: String,
        page: Option<String>,
    ) -> Result<serde_json::Value, AppError> {
        let state = ctx
            .data::<AppState>()
            .map_err(|e| AppError::Internal(e.message))?;
        let p: u32 = page.as_deref().unwrap_or("1").parse().unwrap_or(1);
        let encoded = encode_query(&key);
        let api_url = format!(
            "https://txxx.com/api/videos2.php?params=259200/str/relevance/60/search..{p}.all..&s={encoded}"
        );
        let body = state.fetch.fetch_body(&api_url).await?;
        crate::providers::txxx::parse::parse_txxx_search(&body, p)
    }

    async fn random(&self, ctx: &Context<'_>) -> Result<serde_json::Value, AppError> {
        let state = ctx
            .data::<AppState>()
            .map_err(|e| AppError::Internal(e.message))?;
        let api_url =
            "https://txxx.com/api/json/videos2/14400/str/most-popular/60/..1.all..day.json";
        let body = state.fetch.fetch_body(api_url).await?;
        crate::providers::txxx::parse::parse_txxx_random(&body, api_url)
    }

    async fn related(
        &self,
        ctx: &Context<'_>,
        id: Option<String>,
    ) -> Result<serde_json::Value, AppError> {
        let state = ctx
            .data::<AppState>()
            .map_err(|e| AppError::Internal(e.message))?;
        let video_id = id.ok_or_else(|| AppError::Validation("id is required".to_string()))?;
        let id_num: u64 = video_id
            .trim()
            .parse()
            .map_err(|_| AppError::Validation("Invalid video id: must be numeric".to_string()))?;
        let million = (id_num / 1_000_000) * 1_000_000;
        let thousand = (id_num / 1_000) * 1_000;
        let api_url = format!(
            "https://txxx.com/api/json/videos_related2/432000/50/{million}/{thousand}/{id_num}.all.1.json"
        );
        let body = state.fetch.fetch_body(&api_url).await?;
        crate::providers::txxx::parse::parse_txxx_related(&body, &video_id, 1)
    }
}

pub async fn graphql_handler(
    schema: axum::extract::Extension<AppSchema>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}

pub async fn graphiql() -> impl IntoResponse {
    axum::response::Html(
        async_graphql::http::GraphiQLSource::build()
            .endpoint("/graphql")
            .finish(),
    )
}
