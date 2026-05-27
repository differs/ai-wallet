use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct ApiInfo {
    pub service: &'static str,
    pub version: &'static str,
    pub supported_chains: Vec<&'static str>,
    pub isolation_model: &'static str,
    pub auth_mode: &'static str,
    pub audit_backend: &'static str,
    pub signer_mode: &'static str,
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
    Eip712,
}

#[derive(Debug, Serialize)]
pub struct VerifyMessageResponse {
    pub valid: bool,
    pub recovered_address: String,
    pub chain_id: u64,
    pub encoding: MessageEncoding,
}

#[derive(Debug, Deserialize)]
pub struct VerifyTypedDataRequest {
    pub chain_id: u64,
    pub expected_address: String,
    pub typed_data: Value,
    pub signature_hex: String,
}

#[derive(Debug, Serialize)]
pub struct VerifyTypedDataResponse {
    pub valid: bool,
    pub recovered_address: String,
    pub chain_id: u64,
    pub encoding: MessageEncoding,
    pub digest_hex: String,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SignPayload {
    RawMessage {
        message: String,
    },
    TypedData {
        typed_data: Value,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SignedArtifact {
    pub signature_hex: String,
    pub digest_hex: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SignerRpcSignRequest {
    pub request_id: Uuid,
    pub tenant_id: String,
    pub wallet_id: String,
    pub chain_id: u64,
    pub from_address: String,
    pub payload: SignPayload,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SignerRpcSignResponse {
    pub signed_artifact: SignedArtifact,
}

#[derive(Debug, Deserialize)]
pub struct PrepareTransferRequest {
    pub chain_id: u64,
    pub from_address: String,
    pub to_address: String,
    pub asset: TransferAsset,
    pub amount: String,
    pub gas_limit: Option<u64>,
    pub max_fee_per_gas_wei: Option<String>,
    pub max_priority_fee_per_gas_wei: Option<String>,
    pub nonce: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransferAsset {
    Native,
    Erc20 { token_address: String, decimals: u8 },
}

#[derive(Debug, Serialize)]
pub struct PrepareTransferResponse {
    pub operation: OperationKind,
    pub transaction: PreparedTransaction,
}

#[derive(Debug, Serialize)]
pub struct PreparedTransaction {
    pub to: String,
    pub value_wei: String,
    pub data_hex: String,
    pub gas_limit: u64,
    pub max_fee_per_gas_wei: String,
    pub max_priority_fee_per_gas_wei: String,
    pub nonce: u64,
}

#[derive(Debug, Deserialize)]
pub struct SimulateTransactionRequest {
    pub chain_id: u64,
    pub from_address: String,
    pub to: String,
    pub value_wei: String,
    pub data_hex: String,
    pub gas_limit: u64,
}

#[derive(Debug, Serialize)]
pub struct SimulateTransactionResponse {
    pub mode: SimulationMode,
    pub success: bool,
    pub estimated_gas: u64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationMode {
    Static,
    Rpc,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BroadcastTransactionRequest {
    pub request_id: Option<Uuid>,
    pub chain_id: u64,
    pub raw_transaction_hex: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BroadcastTransactionResponse {
    pub broadcast_id: Uuid,
    pub status: BroadcastState,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BroadcastStatusResponse {
    pub broadcast_id: Uuid,
    pub request_id: Option<Uuid>,
    pub chain_id: u64,
    pub status: BroadcastState,
    pub tx_hash: Option<String>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BroadcastState {
    Queued,
    Submitted,
    Failed,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListAuditEventsQuery {
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ListAuditEventsResponse {
    pub events: Vec<AuditEvent>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuditEvent {
    pub id: Uuid,
    pub request_id: Option<Uuid>,
    pub tenant_id: Option<String>,
    pub wallet_id: Option<String>,
    pub actor: Option<String>,
    pub action: String,
    pub status: String,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct AuditEventInput {
    pub request_id: Option<Uuid>,
    pub tenant_id: Option<String>,
    pub wallet_id: Option<String>,
    pub actor: Option<String>,
    pub action: String,
    pub status: String,
    pub metadata: Value,
}
