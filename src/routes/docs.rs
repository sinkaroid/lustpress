use axum::{
    http::header::CONTENT_TYPE,
    response::{Html, IntoResponse, Response},
};
pub const OPENAPI_JSON: &str = include_str!("../../resources/swagger.json");

pub async fn openapi_json_handler() -> Response {
    (
        [(CONTENT_TYPE, "application/json; charset=utf-8")],
        OPENAPI_JSON,
    )
        .into_response()
}

pub async fn swagger_ui_handler() -> Html<&'static str> {
    Html(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>LustPress API Playground</title>
    <link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5.11.0/swagger-ui.css" />
    <style>body { margin: 0; padding: 0; } .swagger-ui .topbar { display: none; }</style>
</head>
<body>
    <div id="swagger-ui"></div>
    <script src="https://unpkg.com/swagger-ui-dist@5.11.0/swagger-ui-bundle.js"></script>
    <script src="https://unpkg.com/swagger-ui-dist@5.11.0/swagger-ui-standalone-preset.js"></script>
    <script>
        window.onload = () => {
            window.ui = SwaggerUIBundle({
                url: "/docs/json",
                dom_id: '#swagger-ui',
                deepLinking: true,
                presets: [SwaggerUIBundle.presets.apis, SwaggerUIStandalonePreset],
                plugins: [SwaggerUIBundle.plugins.DownloadUrl],
            });
        };
    </script>
</body>
</html>"#,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_openapi_spec_loaded() {
        let parsed: serde_json::Value =
            serde_json::from_str(OPENAPI_JSON).expect("valid swagger json");
        assert_eq!(parsed["openapi"], "3.0.3");
        assert_eq!(parsed["info"]["title"], "Lustpress API");
        assert!(parsed["paths"]["/pornhub/search"].is_object());
    }

    #[tokio::test]
    async fn test_swagger_ui_html() {
        let html = swagger_ui_handler().await;
        assert!(html.0.contains("SwaggerUIBundle"));
        assert!(html.0.contains("/docs/json"));
    }
}
