pub mod docs;
pub mod status;

use crate::state::AppState;
use axum::{Router, routing::get};

pub fn create_router(state: AppState) -> Router {
    let mut router = Router::new()
        .route("/", get(status::root_handler))
        .route("/metrics", get(crate::metrics::metrics_handler))
        .route("/docs", get(docs::swagger_ui_handler))
        .route("/docs/json", get(docs::openapi_json_handler))
        .nest("/pornhub", crate::providers::pornhub::routes())
        .nest("/xnxx", crate::providers::xnxx::routes())
        .nest("/redtube", crate::providers::redtube::routes())
        .nest("/xvideos", crate::providers::xvideos::routes())
        .nest("/xhamster", crate::providers::xhamster::routes())
        .nest("/youporn", crate::providers::youporn::routes())
        .nest("/eporner", crate::providers::eporner::routes())
        .nest("/txxx", crate::providers::txxx::routes());

    if state.config.lustpress_graphql {
        let schema = crate::graphql::build_schema(state.clone());
        router = router
            .route(
                "/graphql",
                get(crate::graphql::graphiql).post(crate::graphql::graphql_handler),
            )
            .layer(axum::Extension(schema));
    }

    router
        .layer(axum::middleware::from_fn(
            crate::metrics::telemetry_middleware,
        ))
        .fallback(crate::error::fallback_404_handler)
        .with_state(state)
}
