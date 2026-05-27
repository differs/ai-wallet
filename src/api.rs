use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    middleware,
    routing::{get, post},
};
use uuid::Uuid;

use crate::{
    auth::auth_middleware,
    error::AppError,
    model::{
        ApiInfo, BroadcastStatusResponse, BroadcastTransactionRequest,
        BroadcastTransactionResponse, ListAuditEventsQuery, ListAuditEventsResponse,
        PrepareTransferRequest, PrepareTransferResponse, SignIntentRequest, SignIntentResponse,
        SimulateTransactionRequest, SimulateTransactionResponse, VerifyMessageRequest,
        VerifyMessageResponse, VerifyTypedDataRequest, VerifyTypedDataResponse,
    },
    service::{
        AppState, handle_sign_intent, prepare_transfer, simulate_transaction, verify_message,
        verify_typed_data,
    },
};

pub fn router(state: Arc<AppState>) -> Router {
    let authenticated_routes = Router::new()
        .route("/v1/evm/verify-message", post(verify_message_handler))
        .route("/v1/evm/verify-typed-data", post(verify_typed_data_handler))
        .route("/v1/evm/prepare-transfer", post(prepare_transfer_handler))
        .route(
            "/v1/evm/simulate-transaction",
            post(simulate_transaction_handler),
        )
        .route("/v1/evm/sign-intent", post(sign_intent_handler))
        .route(
            "/v1/evm/broadcast-transaction",
            post(broadcast_transaction_handler),
        )
        .route(
            "/v1/evm/broadcast-status/{broadcast_id}",
            get(broadcast_status_handler),
        )
        .route("/v1/audit-events", get(list_audit_events_handler))
        .layer(middleware::from_fn_with_state(
            Arc::clone(&state),
            auth_middleware,
        ));

    Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/info", get(info))
        .merge(authenticated_routes)
        .with_state(state)
}

async fn healthz() -> &'static str {
    "ok"
}

async fn info(State(state): State<Arc<AppState>>) -> Json<ApiInfo> {
    Json(ApiInfo {
        service: "ai-wallet",
        version: env!("CARGO_PKG_VERSION"),
        supported_chains: vec!["ethereum", "base", "arbitrum", "optimism", "bsc"],
        isolation_model: "api gateway + isolated signer worker + policy engine",
        auth_mode: state.api_auth.mode_name(),
        audit_backend: state.audit.backend_name(),
        signer_mode: match state.signer.execution_mode() {
            crate::model::ExecutionMode::Mock => "mock",
            crate::model::ExecutionMode::IsolatedSigner => "isolated-signer",
        },
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

async fn broadcast_transaction_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<BroadcastTransactionRequest>,
) -> Result<Json<BroadcastTransactionResponse>, AppError> {
    let response = state.broadcast.enqueue(req.clone()).await?;
    let _ = state
        .audit
        .record(crate::model::AuditEventInput {
            request_id: req.request_id,
            tenant_id: None,
            wallet_id: None,
            actor: None,
            action: "broadcast_transaction".into(),
            status: "queued".into(),
            metadata: serde_json::json!({
                "broadcast_id": response.broadcast_id,
                "chain_id": req.chain_id,
            }),
        })
        .await?;
    Ok(Json(response))
}

async fn broadcast_status_handler(
    State(state): State<Arc<AppState>>,
    Path(broadcast_id): Path<Uuid>,
) -> Result<Json<BroadcastStatusResponse>, AppError> {
    Ok(Json(state.broadcast.get_status(broadcast_id).await?))
}

async fn list_audit_events_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListAuditEventsQuery>,
) -> Result<Json<ListAuditEventsResponse>, AppError> {
    let limit = query.limit.unwrap_or(50).min(200) as usize;
    Ok(Json(ListAuditEventsResponse {
        events: state.audit.list(limit).await?,
    }))
}
