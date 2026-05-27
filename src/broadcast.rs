use std::{collections::HashMap, sync::Arc};

use chrono::Utc;
use reqwest::Client;
use serde_json::json;
use tokio::sync::{RwLock, mpsc};
use uuid::Uuid;

use crate::{
    audit::AuditRepository,
    error::AppError,
    model::{
        AuditEventInput, BroadcastState, BroadcastStatusResponse, BroadcastTransactionRequest,
        BroadcastTransactionResponse,
    },
};

#[derive(Clone)]
pub struct BroadcastService {
    sender: mpsc::Sender<BroadcastJob>,
    statuses: Arc<RwLock<HashMap<Uuid, BroadcastStatusResponse>>>,
}

#[derive(Clone)]
struct BroadcastJob {
    broadcast_id: Uuid,
    request: BroadcastTransactionRequest,
}

impl BroadcastService {
    pub fn new(rpc_url: Option<String>, audit: Arc<dyn AuditRepository>) -> Self {
        let (sender, mut receiver) = mpsc::channel::<BroadcastJob>(128);
        let statuses = Arc::new(RwLock::new(HashMap::<Uuid, BroadcastStatusResponse>::new()));
        let statuses_for_task = Arc::clone(&statuses);

        tokio::spawn(async move {
            let client = Client::new();
            while let Some(job) = receiver.recv().await {
                let outcome = submit_transaction(&client, rpc_url.as_deref(), &job.request).await;
                let mut statuses = statuses_for_task.write().await;
                if let Some(status) = statuses.get_mut(&job.broadcast_id) {
                    status.updated_at = Utc::now();
                    match outcome {
                        Ok(tx_hash) => {
                            status.status = BroadcastState::Submitted;
                            status.tx_hash = Some(tx_hash.clone());
                            status.error = None;
                            let _ = audit
                                .record(AuditEventInput {
                                    request_id: status.request_id,
                                    tenant_id: None,
                                    wallet_id: None,
                                    actor: None,
                                    action: "broadcast_transaction".into(),
                                    status: "submitted".into(),
                                    metadata: json!({
                                        "broadcast_id": status.broadcast_id,
                                        "tx_hash": tx_hash,
                                        "chain_id": status.chain_id,
                                    }),
                                })
                                .await;
                        }
                        Err(err) => {
                            status.status = BroadcastState::Failed;
                            status.tx_hash = None;
                            status.error = Some(err.to_string());
                            let _ = audit
                                .record(AuditEventInput {
                                    request_id: status.request_id,
                                    tenant_id: None,
                                    wallet_id: None,
                                    actor: None,
                                    action: "broadcast_transaction".into(),
                                    status: "failed".into(),
                                    metadata: json!({
                                        "broadcast_id": status.broadcast_id,
                                        "chain_id": status.chain_id,
                                        "error": err.to_string(),
                                    }),
                                })
                                .await;
                        }
                    }
                }
            }
        });

        Self { sender, statuses }
    }

    pub async fn enqueue(
        &self,
        request: BroadcastTransactionRequest,
    ) -> Result<BroadcastTransactionResponse, AppError> {
        let now = Utc::now();
        let broadcast_id = Uuid::new_v4();
        let response = BroadcastStatusResponse {
            broadcast_id,
            request_id: request.request_id,
            chain_id: request.chain_id,
            status: BroadcastState::Queued,
            tx_hash: None,
            error: None,
            created_at: now,
            updated_at: now,
        };
        self.statuses
            .write()
            .await
            .insert(broadcast_id, response.clone());
        self.sender
            .send(BroadcastJob {
                broadcast_id,
                request,
            })
            .await
            .map_err(|e| AppError::Internal(format!("failed to queue broadcast: {e}")))?;

        Ok(BroadcastTransactionResponse {
            broadcast_id,
            status: BroadcastState::Queued,
            created_at: now,
        })
    }

    pub async fn get_status(&self, id: Uuid) -> Result<BroadcastStatusResponse, AppError> {
        self.statuses
            .read()
            .await
            .get(&id)
            .cloned()
            .ok_or_else(|| AppError::BadRequest(format!("unknown broadcast id `{id}`")))
    }
}

async fn submit_transaction(
    client: &Client,
    rpc_url: Option<&str>,
    request: &BroadcastTransactionRequest,
) -> Result<String, AppError> {
    let rpc_url = rpc_url.ok_or_else(|| {
        AppError::Internal("AI_WALLET_RPC_URL is required for broadcast submission".into())
    })?;

    let response = client
        .post(rpc_url)
        .json(&json!({
            "jsonrpc": "2.0",
            "method": "eth_sendRawTransaction",
            "params": [request.raw_transaction_hex],
            "id": 1
        }))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("broadcast RPC request failed: {e}")))?;
    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("broadcast RPC response decode failed: {e}")))?;

    if let Some(result) = json.get("result").and_then(|value| value.as_str()) {
        return Ok(result.to_string());
    }

    Err(AppError::Internal(format!(
        "broadcast RPC returned error: {}",
        json.get("error")
            .cloned()
            .unwrap_or_else(|| json!({"message": "unknown"}))
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::MemoryAuditRepository;

    #[tokio::test]
    async fn enqueue_creates_pending_status() {
        let service = BroadcastService::new(None, Arc::new(MemoryAuditRepository::new()));
        let queued = service
            .enqueue(BroadcastTransactionRequest {
                request_id: None,
                chain_id: 1,
                raw_transaction_hex: "0x02".into(),
            })
            .await
            .expect("enqueue");

        let status = service
            .get_status(queued.broadcast_id)
            .await
            .expect("status");
        assert!(matches!(
            status.status,
            BroadcastState::Queued | BroadcastState::Failed
        ));
    }
}
