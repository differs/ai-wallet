pub mod api;
pub mod config;
pub mod error;
pub mod model;
pub mod service;

use std::sync::Arc;

use axum::Router;
use config::{AppConfig, SignerMode};
use service::{AppState, LocalDevSigner, MockSigner, PolicyEngine, StaticSimulator};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

pub fn build_app(config: AppConfig) -> Router {
    let signer = match config.signer_mode {
        SignerMode::Mock => Arc::new(MockSigner) as _,
        SignerMode::LocalDev => Arc::new(LocalDevSigner::new(config.dev_private_key.clone())) as _,
    };

    let state = Arc::new(AppState {
        policy_engine: PolicyEngine::default(),
        signer,
        simulator: Arc::new(StaticSimulator::new(config.rpc_url.clone())),
    });

    Router::new()
        .merge(api::router(state))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}
