pub mod api;
pub mod audit;
pub mod auth;
pub mod broadcast;
pub mod config;
pub mod error;
pub mod model;
pub mod service;
pub mod signer_rpc;

use std::sync::Arc;

use audit::build_audit_repository;
use auth::HmacAuthVerifier;
use axum::Router;
use broadcast::BroadcastService;
use config::{AppConfig, SignerMode};
use service::{AppState, LocalDevSigner, MockSigner, PolicyEngine, StaticSimulator};
use signer_rpc::RemoteSignerClient;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

pub async fn build_state(config: AppConfig) -> Result<Arc<AppState>, error::AppError> {
    let signer = match config.signer_mode {
        SignerMode::Mock => Arc::new(MockSigner) as _,
        SignerMode::LocalDev => Arc::new(LocalDevSigner::new(config.dev_private_key.clone())) as _,
        SignerMode::RemoteMtls => Arc::new(RemoteSignerClient::new(
            config.signer_url.clone().ok_or_else(|| {
                error::AppError::Internal("AI_WALLET_SIGNER_URL is required".into())
            })?,
            config.signer_client_cert_path.as_deref().ok_or_else(|| {
                error::AppError::Internal("AI_WALLET_SIGNER_CLIENT_CERT_PATH is required".into())
            })?,
            config.signer_client_key_path.as_deref().ok_or_else(|| {
                error::AppError::Internal("AI_WALLET_SIGNER_CLIENT_KEY_PATH is required".into())
            })?,
            config.signer_ca_cert_path.as_deref().ok_or_else(|| {
                error::AppError::Internal("AI_WALLET_SIGNER_CA_CERT_PATH is required".into())
            })?,
        )?) as _,
    };

    let audit = build_audit_repository(config.database_url.as_deref()).await?;
    let broadcast = BroadcastService::new(config.rpc_url.clone(), Arc::clone(&audit));
    let api_auth = HmacAuthVerifier::from_config(
        config.auth_mode,
        config.api_key.clone(),
        config.api_secret.clone(),
        config.auth_max_skew_seconds,
    )?;

    Ok(Arc::new(AppState {
        api_auth,
        policy_engine: PolicyEngine::default(),
        signer,
        simulator: Arc::new(StaticSimulator::new(config.rpc_url.clone())),
        audit,
        broadcast,
    }))
}

pub fn build_app(state: Arc<AppState>) -> Router {
    Router::new()
        .merge(api::router(state))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}
