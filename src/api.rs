use std::sync::Arc;

use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};

use crate::{
    error::AppError,
    model::{
        ApiInfo, SignIntentRequest, SignIntentResponse, VerifyMessageRequest, VerifyMessageResponse,
    },
    service::{AppState, handle_sign_intent, verify_message},
};

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/info", get(info))
        .route("/v1/evm/verify-message", post(verify_message_handler))
        .route("/v1/evm/sign-intent", post(sign_intent_handler))
        .with_state(state)
}

async fn healthz() -> &'static str {
    "ok"
}

async fn info() -> Json<ApiInfo> {
    Json(ApiInfo {
        service: "ai-wallet",
        version: env!("CARGO_PKG_VERSION"),
        supported_chains: vec!["ethereum", "base", "arbitrum", "optimism", "bsc"],
        isolation_model: "api gateway + isolated signer worker + policy engine",
    })
}

async fn verify_message_handler(
    Json(req): Json<VerifyMessageRequest>,
) -> Result<Json<VerifyMessageResponse>, AppError> {
    Ok(Json(verify_message(req)?))
}

async fn sign_intent_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SignIntentRequest>,
) -> Result<Json<SignIntentResponse>, AppError> {
    Ok(Json(handle_sign_intent(&state, req)?))
}
