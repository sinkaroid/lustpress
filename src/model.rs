use async_graphql::SimpleObject;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
#[graphql(name = "VideoData")]
pub struct VideoDetails {
    pub title: String,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    pub duration: String,
    pub views: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<String>,
    pub uploaded: String,
    pub upvoted: Option<String>,
    pub downvoted: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    pub models: Vec<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
#[graphql(name = "VideoResult")]
pub struct VideoResponse {
    pub success: bool,
    pub data: VideoDetails,
    pub source: String,
    pub assets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
#[graphql(name = "SearchItem")]
pub struct SearchItem {
    pub link: String,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub views: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uploader: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
#[graphql(name = "SearchResult")]
pub struct SearchResponse {
    pub success: bool,
    pub data: Vec<SearchItem>,
    pub source: String,
}
