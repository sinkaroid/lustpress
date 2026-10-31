use axum::{
    Json,
    extract::{FromRequestParts, Query, rejection::QueryRejection},
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use serde::Serialize;
use thiserror::Error;

#[derive(Serialize, Debug, Clone)]
pub struct ErrorResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Error, Debug)]
pub enum AppError {
    #[error("{0}")]
    NotFound(String),

    #[error("{0}")]
    Validation(String),

    #[error("{0}")]
    Scraper(String),

    #[error("Upstream error: {status} from {url}")]
    Upstream { status: StatusCode, url: String },

    #[error("Internal server error: {0}")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::Validation(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            // D2: Scraper errors map to 400 for legacy compatibility
            AppError::Scraper(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            // D3: Upstream non-2xx returns 502 Bad Gateway
            AppError::Upstream { status: _, url } => {
                tracing::warn!(url = %url, "Upstream returned non-2xx");
                (
                    StatusCode::BAD_GATEWAY,
                    "Upstream service failure".to_string(),
                )
            }
            AppError::Internal(msg) => {
                tracing::error!(error = %msg, "Internal server error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal Server Error".to_string(),
                )
            }
        };

        let body = Json(ErrorResponse {
            success: false,
            message,
        });

        (status, body).into_response()
    }
}

/// Fallback handler for unmatched routes (ensures 404 is always JSON, never plain text)
pub async fn fallback_404_handler() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            success: false,
            message: "Not Found".to_string(),
        }),
    )
}

/// Custom Query extractor that converts any query parsing rejection into JSON (never plain text)
pub struct SafeQuery<T>(pub T);

impl<T, S> FromRequestParts<S> for SafeQuery<T>
where
    T: serde::de::DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match Query::<T>::from_request_parts(parts, state).await {
            Ok(Query(value)) => Ok(SafeQuery(value)),
            Err(rejection) => Err(AppError::Validation(match rejection {
                QueryRejection::FailedToDeserializeQueryString(err) => err.to_string(),
                _ => "Invalid query parameters".to_string(),
            })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_envelope_format() {
        let err = AppError::Scraper("No result found".to_string());
        let res = err.into_response();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_fallback_404_json() {
        let (status, Json(body)) = fallback_404_handler().await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(!body.success);
        assert_eq!(body.message, "Not Found");
    }
}
