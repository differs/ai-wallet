use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct ApiInfo {
    pub service: &'static str,
    pub version: &'static str,
    pub supported_chains: Vec<&'static str>,
    pub isolation_model: &'static str,
}

#[derive(Debug, Deserialize)]
pub struct VerifyMessageRequest {
    pub chain_id: u64,
    pub expected_address: String,
    pub message: String,
    pub signature_hex: String,
    #[serde(default = "default_message_encoding")]
    pub encoding: MessageEncoding,
}

fn default_message_encoding() -> MessageEncoding {
    MessageEncoding::Eip191
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageEncoding {
    Eip191,
}

#[derive(Debug, Serialize)]
pub struct VerifyMessageResponse {
    pub valid: bool,
    pub recovered_address: String,
    pub chain_id: u64,
    pub encoding: MessageEncoding,
}

#[derive(Debug, Deserialize)]
pub struct SignIntentRequest {
    pub tenant_id: String,
    pub wallet_id: String,
    pub chain_id: u64,
    pub from_address: String,
    pub operation: OperationKind,
    pub payload: SignPayload,
    pub policy_context: PolicyContext,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    TransferNative,
    TransferErc20,
    ContractCall,
    TypedData,
    RawMessage,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SignPayload {
    RawMessage {
        message: String,
    },
    Transaction {
        to: String,
        value_wei: String,
        data_hex: String,
        gas_limit: u64,
        max_fee_per_gas_wei: String,
        max_priority_fee_per_gas_wei: String,
        nonce: u64,
    },
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PolicyContext {
    pub actor: String,
    pub purpose: String,
    pub source_ip: String,
    pub idempotency_key: String,
    pub max_value_wei: String,
}

#[derive(Debug, Serialize)]
pub struct SignIntentResponse {
    pub request_id: Uuid,
    pub decision: PolicyDecision,
    pub execution_mode: ExecutionMode,
    pub signed_artifact: Option<SignedArtifact>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyDecision {
    Approved,
    Denied,
    RequiresReview,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    Mock,
    IsolatedSigner,
}

#[derive(Debug, Serialize)]
pub struct SignedArtifact {
    pub signature_hex: String,
    pub digest_hex: String,
}
