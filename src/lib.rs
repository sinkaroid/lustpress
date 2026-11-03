#![recursion_limit = "512"]
#![deny(clippy::unwrap_used, clippy::expect_used)]

pub mod cache;
pub mod config;
pub mod error;
pub mod fetch;
pub mod graphql;
pub mod metrics;
pub mod model;
pub mod providers;
pub mod routes;
pub mod state;
pub mod util;
