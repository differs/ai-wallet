pub mod api;
pub mod error;
pub mod model;
pub mod service;

use std::sync::Arc;

use axum::Router;
use service::{AppState, MockSigner, PolicyEngine};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

pub fn build_app() -> Router {
    let state = Arc::new(AppState {
        policy_engine: PolicyEngine::default(),
        signer: Arc::new(MockSigner),
    });

    Router::new()
        .merge(api::router(state))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}
