use std::sync::Arc;

use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};

use crate::{
    error::AppError,
    model::{
        ApiInfo, PrepareTransferRequest, PrepareTransferResponse, SignIntentRequest,
        SignIntentResponse, SimulateTransactionRequest, SimulateTransactionResponse,
        VerifyMessageRequest, VerifyMessageResponse, VerifyTypedDataRequest,
        VerifyTypedDataResponse,
    },
    service::{
        AppState, handle_sign_intent, prepare_transfer, simulate_transaction, verify_message,
        verify_typed_data,
    },
};

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/info", get(info))
        .route("/v1/evm/verify-message", post(verify_message_handler))
        .route("/v1/evm/verify-typed-data", post(verify_typed_data_handler))
        .route("/v1/evm/prepare-transfer", post(prepare_transfer_handler))
        .route(
            "/v1/evm/simulate-transaction",
            post(simulate_transaction_handler),
        )
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

async fn verify_typed_data_handler(
    Json(req): Json<VerifyTypedDataRequest>,
) -> Result<Json<VerifyTypedDataResponse>, AppError> {
    Ok(Json(verify_typed_data(req)?))
}

async fn prepare_transfer_handler(
    Json(req): Json<PrepareTransferRequest>,
) -> Result<Json<PrepareTransferResponse>, AppError> {
    Ok(Json(prepare_transfer(req)?))
}

async fn sign_intent_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SignIntentRequest>,
) -> Result<Json<SignIntentResponse>, AppError> {
    Ok(Json(handle_sign_intent(&state, req).await?))
}

async fn simulate_transaction_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SimulateTransactionRequest>,
) -> Result<Json<SimulateTransactionResponse>, AppError> {
    Ok(Json(simulate_transaction(&state, req).await?))
}
