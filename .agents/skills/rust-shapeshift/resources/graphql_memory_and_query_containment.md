# GraphQL Memory & Query Containment: Production Async-GraphQL in Axum

In GraphQL APIs, memory efficiency cannot be guaranteed by standard HTTP body limits alone. A seemingly small 2KB GraphQL query can trigger exponential in-memory execution trees via nested relationships, circular fragments, and unbatched resolver calls (the N+1 query problem).

Without strict schema-level containment, a single recursive GraphQL query can allocate hundreds of thousands of intermediate async futures, blowing process RSS past gigabytes in milliseconds.

This reference guide documents how to build a high-throughput, memory-contained GraphQL server using **`async-graphql`** and **`async-graphql-axum`**.

---

## 1. Schema-Level Depth & Complexity Containment

Never expose an unconstrained GraphQL schema to production traffic. `async-graphql` provides compile-time schema builders with strict mathematical bounds:

```toml
# Cargo.toml
[dependencies]
async-graphql = { version = "7.0", features = ["chrono"] }
async-graphql-axum = "7.0"
```

### Schema Construction with Hard Limits:
```rust
use async_graphql::{EmptyMutation, EmptySubscription, Schema};
use std::sync::Arc;

pub type AppSchema = Schema<QueryRoot, EmptyMutation, EmptySubscription>;

pub fn build_schema(state: Arc<AppState>) -> AppSchema {
    let mut builder = Schema::build(QueryRoot, EmptyMutation, EmptySubscription)
        // 🚀 INJECT SHARED STATE: Zero-cost pointer reference across all resolvers
        .data(state)
        // 🛑 PREVENT RECURSIVE AST EXHAUSTION: Maximum nested selection depth
        .limit_depth(5)
        // 🛑 PREVENT COMPUTATIONAL ATTACKS: Maximum computed complexity ceiling
        .limit_complexity(100);

    // Disable schema introspection in production to protect schema memory and security
    #[cfg(not(debug_assertions))]
    {
        builder = builder.disable_introspection();
    }

    builder.finish()
}
```

### Depth vs Complexity Guard Mechanics:
- **`limit_depth(5)`**: Blocks malicious nested queries like `{ user { posts { author { posts { author { id } } } } } }`.
- **`limit_complexity(100)`**: Calculates field cost weights. If a query requests 50 items and each item expands 10 sub-fields ($50 \times 10 = 500$), the query is aborted at the parser level before resolving a single field.

---

## 2. Solving the N+1 Memory Trap with `DataLoader`

The classic GraphQL memory and latency disaster is the **N+1 Resolver Problem**:
- Querying a list of 50 videos and resolving their uploader profile independently triggers **51 separate network requests/queries**, allocating 50 concurrent futures and duplicate in-memory data structures.

### The Batched Zero-Allocation Solution:
Use `async_graphql::dataloader::DataLoader` to coalesce individual field requests across the entire execution tree into a single vectorized batch query:

```rust
use async_graphql::dataloader::{DataLoader, Loader};
use std::collections::HashMap;
use std::sync::Arc;

pub struct UploaderLoader {
    state: Arc<AppState>,
}

impl Loader<CompactString> for UploaderLoader {
    type Value = UploaderProfile;
    type Error = Arc<AppError>;

    async fn load(
        &self,
        keys: &[CompactString],
    ) -> Result<HashMap<CompactString, Self::Value>, Self::Error> {
        // 🚀 SINGLE VECTORIZED LOOKUP: Fetch all 50 keys in one single roundtrip
        let mut results = HashMap::with_capacity(keys.len());
        let profiles = self.state.fetch_profiles_batch(keys).await?;

        for profile in profiles {
            results.insert(profile.id.clone(), profile);
        }

        Ok(results)
    }
}
```

### Registering DataLoader in Schema State:
```rust
let loader = DataLoader::new(
    UploaderLoader { state: state.clone() },
    tokio::spawn, // Batch executor
).max_batch_size(100);

let schema = Schema::build(QueryRoot, EmptyMutation, EmptySubscription)
    .data(loader)
    .finish();
```

---

## 3. Resolvers with Zero-Copy State Access

Never clone entire application state or configurations inside field resolvers. Extract borrowed references directly from the GraphQL execution context (`&Context<'_>`):

```rust
use async_graphql::{Context, Object, Result};
use std::sync::Arc;

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn video<'ctx>(&self, ctx: &Context<'ctx>, id: String) -> Result<VideoType> {
        // Borrow state from context (0 heap allocations)
        let state = ctx.data::<Arc<AppState>>()?;
        
        let video_data = state.get_video(&id).await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        Ok(VideoType::from(video_data))
    }

    async fn search<'ctx>(
        &self,
        ctx: &Context<'ctx>,
        query: String,
        limit: Option<usize>,
    ) -> Result<Vec<VideoCardType>> {
        let state = ctx.data::<Arc<AppState>>()?;
        
        // Enforce hard ceiling on pagination limit
        let bounded_limit = limit.unwrap_or(20).min(50);
        let items = state.search_videos(&query, bounded_limit).await?;

        Ok(items.into_iter().map(VideoCardType::from).collect())
    }
}
```

---

## 4. Production Axum 0.8 Router Integration

Expose both the POST execution route and an optional GraphiQL playground interface using `async-graphql-axum`:

```rust
use async_graphql::http::GraphiQLSource;
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::{
    extract::State,
    response::{Html, IntoResponse},
    routing::{get, post},
    Router,
};
use std::sync::Arc;

// GraphQL execution handler
pub async fn graphql_handler(
    State(schema): State<AppSchema>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}

// GraphiQL development IDE (disabled in strict production environments)
pub async fn graphiql_ide() -> impl IntoResponse {
    Html(GraphiQLSource::build().endpoint("/graphql").finish())
}

// Router assembly
pub fn configure_graphql_routes(schema: AppSchema) -> Router {
    Router::new()
        .route("/graphql", post(graphql_handler))
        .route("/graphiql", get(graphiql_ide))
        .with_state(schema)
}
```

---

## 5. GraphQL Security & Memory Checklist

1. Is `limit_depth` configured (recommended: 5–7 levels maximum)?
2. Is `limit_complexity` applied to constrain expensive multiply-nested lists?
3. Is schema introspection disabled in production via `.disable_introspection()`?
4. Are recursive relational lookups batched via `DataLoader` to eliminate N+1 allocations?
5. Are all query list limits (`limit: Option<usize>`) clamped to a strict maximum (e.g., `.min(50)`)?
