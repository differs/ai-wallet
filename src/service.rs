use std::{str::FromStr, sync::Arc};

use async_trait::async_trait;
use ethers_core::{
    abi::{Token, encode},
    types::{
        Address, Eip1559TransactionRequest, NameOrAddress, Signature, U64, U256,
        transaction::{eip712::Eip712, eip712::TypedData, eip2718::TypedTransaction},
    },
    utils::{hash_message, id},
};
use ethers_signers::{LocalWallet, Signer};
use uuid::Uuid;

use crate::{
    error::AppError,
    model::{
        ExecutionMode, MessageEncoding, OperationKind, PolicyDecision, PrepareTransferRequest,
        PrepareTransferResponse, PreparedTransaction, SignIntentRequest, SignIntentResponse,
        SignPayload, SignedArtifact, SimulateTransactionRequest, SimulateTransactionResponse,
        SimulationMode, TransferAsset, VerifyMessageRequest, VerifyMessageResponse,
        VerifyTypedDataRequest, VerifyTypedDataResponse,
    },
};

pub struct AppState {
    pub policy_engine: PolicyEngine,
    pub signer: Arc<dyn SignerGateway>,
    pub simulator: Arc<dyn TransactionSimulator>,
}

#[derive(Default)]
pub struct PolicyEngine;

impl PolicyEngine {
    pub fn evaluate(&self, req: &SignIntentRequest) -> Result<PolicyDecision, AppError> {
        let value = req
            .policy_context
            .max_value_wei
            .parse::<u128>()
            .map_err(|_| {
                AppError::BadRequest(
                    "policy_context.max_value_wei must be a base-10 integer".into(),
                )
            })?;

        if req.chain_id == 0 {
            return Err(AppError::BadRequest("chain_id must be non-zero".into()));
        }

        if req.from_address.trim().is_empty() || req.wallet_id.trim().is_empty() {
            return Err(AppError::BadRequest(
                "wallet_id and from_address are required".into(),
            ));
        }

        match &req.payload {
            SignPayload::TypedData { typed_data } => {
                let parsed: TypedData =
                    serde_json::from_value(typed_data.clone()).map_err(|e| {
                        AppError::BadRequest(format!("invalid typed_data payload: {e}"))
                    })?;
                if parsed.domain.chain_id.is_none() {
                    return Err(AppError::BadRequest(
                        "typed_data.domain.chainId is required".into(),
                    ));
                }
            }
            SignPayload::Transaction { to, .. } => {
                let _ = parse_address(to)?;
            }
            SignPayload::RawMessage { .. } => {}
        }

        if value > 10_000_000_000_000_000u128 {
            return Ok(PolicyDecision::RequiresReview);
        }

        if matches!(req.operation, OperationKind::ContractCall)
            && req.policy_context.purpose.trim().is_empty()
        {
            return Ok(PolicyDecision::Denied);
        }

        Ok(PolicyDecision::Approved)
    }
}

#[derive(Debug, Clone)]
pub struct SignerEnvelope {
    pub request_id: Uuid,
    pub tenant_id: String,
    pub wallet_id: String,
    pub chain_id: u64,
    pub from_address: String,
    pub payload: SignPayload,
}

#[async_trait]
pub trait SignerGateway: Send + Sync {
    async fn sign(&self, envelope: SignerEnvelope) -> Result<SignedArtifact, AppError>;
    fn execution_mode(&self) -> ExecutionMode;
}

pub struct MockSigner;

#[async_trait]
impl SignerGateway for MockSigner {
    async fn sign(&self, envelope: SignerEnvelope) -> Result<SignedArtifact, AppError> {
        let digest = format!(
            "mock:{}:{}:{}:{}:{}",
            envelope.request_id,
            envelope.chain_id,
            envelope.wallet_id,
            envelope.from_address,
            envelope.tenant_id
        );
        Ok(SignedArtifact {
            signature_hex: format!("0x{}", hex::encode(format!("signed:{digest}"))),
            digest_hex: format!("0x{}", hex::encode(digest)),
        })
    }

    fn execution_mode(&self) -> ExecutionMode {
        ExecutionMode::Mock
    }
}

pub struct LocalDevSigner {
    private_key: Option<String>,
}

impl LocalDevSigner {
    pub fn new(private_key: Option<String>) -> Self {
        Self { private_key }
    }

    fn wallet(&self, chain_id: u64) -> Result<LocalWallet, AppError> {
        let key = self.private_key.as_ref().ok_or_else(|| {
            AppError::SignerUnavailable(
                "AI_WALLET_DEV_PRIVATE_KEY is required for local-dev signer mode".into(),
            )
        })?;

        LocalWallet::from_str(key)
            .map(|wallet| wallet.with_chain_id(chain_id))
            .map_err(|e| AppError::SignerUnavailable(format!("invalid dev private key: {e}")))
    }
}

#[async_trait]
impl SignerGateway for LocalDevSigner {
    async fn sign(&self, envelope: SignerEnvelope) -> Result<SignedArtifact, AppError> {
        let wallet = self.wallet(envelope.chain_id)?;

        let (signature_hex, digest_hex) = match envelope.payload {
            SignPayload::RawMessage { message } => {
                let digest = hash_message(&message);
                let signature = wallet.sign_message(message).await.map_err(|e| {
                    AppError::SignerUnavailable(format!("sign_message failed: {e}"))
                })?;

                (signature.to_string(), format!("0x{}", hex::encode(digest)))
            }
            SignPayload::TypedData { typed_data } => {
                let typed_data: TypedData = serde_json::from_value(typed_data).map_err(|e| {
                    AppError::BadRequest(format!("invalid typed_data payload: {e}"))
                })?;
                let digest = typed_data
                    .encode_eip712()
                    .map_err(|e| AppError::BadRequest(format!("typed data digest failed: {e}")))?;
                let signature = wallet.sign_typed_data(&typed_data).await.map_err(|e| {
                    AppError::SignerUnavailable(format!("sign_typed_data failed: {e}"))
                })?;

                (signature.to_string(), format!("0x{}", hex::encode(digest)))
            }
            SignPayload::Transaction {
                to,
                value_wei,
                data_hex,
                gas_limit,
                max_fee_per_gas_wei,
                max_priority_fee_per_gas_wei,
                nonce,
            } => {
                let tx = TypedTransaction::Eip1559(Eip1559TransactionRequest {
                    from: Some(parse_address(&envelope.from_address)?),
                    to: Some(NameOrAddress::Address(parse_address(&to)?)),
                    gas: Some(U256::from(gas_limit)),
                    value: Some(parse_u256("value_wei", &value_wei)?),
                    data: Some(parse_bytes_hex("data_hex", &data_hex)?.into()),
                    nonce: Some(U256::from(nonce)),
                    max_fee_per_gas: Some(parse_u256("max_fee_per_gas_wei", &max_fee_per_gas_wei)?),
                    max_priority_fee_per_gas: Some(parse_u256(
                        "max_priority_fee_per_gas_wei",
                        &max_priority_fee_per_gas_wei,
                    )?),
                    chain_id: Some(U64::from(envelope.chain_id)),
                    access_list: Default::default(),
                });
                let digest = tx.sighash();
                let signature = wallet.sign_transaction(&tx).await.map_err(|e| {
                    AppError::SignerUnavailable(format!("sign_transaction failed: {e}"))
                })?;

                (signature.to_string(), format!("0x{}", hex::encode(digest)))
            }
        };

        Ok(SignedArtifact {
            signature_hex,
            digest_hex,
        })
    }

    fn execution_mode(&self) -> ExecutionMode {
        ExecutionMode::IsolatedSigner
    }
}

#[async_trait]
pub trait TransactionSimulator: Send + Sync {
    async fn simulate(
        &self,
        req: &SimulateTransactionRequest,
    ) -> Result<SimulateTransactionResponse, AppError>;
}

pub struct StaticSimulator {
    rpc_url: Option<String>,
}

impl StaticSimulator {
    pub fn new(rpc_url: Option<String>) -> Self {
        Self { rpc_url }
    }
}

#[async_trait]
impl TransactionSimulator for StaticSimulator {
    async fn simulate(
        &self,
        req: &SimulateTransactionRequest,
    ) -> Result<SimulateTransactionResponse, AppError> {
        let _ = parse_address(&req.from_address)?;
        let _ = parse_address(&req.to)?;
        let _ = parse_u256("value_wei", &req.value_wei)?;
        let data = parse_bytes_hex("data_hex", &req.data_hex)?;

        let estimated_gas = if data.is_empty() {
            21_000
        } else {
            21_000 + (data.len() as u64 * 16)
        };

        let mut warnings = Vec::new();
        if req.gas_limit < 21_000 {
            warnings.push("gas_limit is below the minimum for a basic EVM transfer".into());
        }
        if !data.is_empty() && req.gas_limit < 50_000 {
            warnings.push("contract call has low gas_limit for non-empty calldata".into());
        }
        if self.rpc_url.is_none() {
            warnings.push(
                "AI_WALLET_RPC_URL is not set; using static simulation instead of live RPC".into(),
            );
        }

        Ok(SimulateTransactionResponse {
            mode: if self.rpc_url.is_some() {
                SimulationMode::Rpc
            } else {
                SimulationMode::Static
            },
            success: req.gas_limit >= estimated_gas,
            estimated_gas,
            warnings,
        })
    }
}

pub fn verify_message(req: VerifyMessageRequest) -> Result<VerifyMessageResponse, AppError> {
    let signature: Signature = req
        .signature_hex
        .parse()
        .map_err(|e| AppError::BadRequest(format!("invalid signature: {e}")))?;
    let expected = parse_address(&req.expected_address)?;
    let digest = hash_message(req.message);
    let recovered = signature
        .recover(digest)
        .map_err(|e| AppError::BadRequest(format!("failed to recover signer: {e}")))?;

    Ok(VerifyMessageResponse {
        valid: recovered == expected,
        recovered_address: format!("{recovered:#x}"),
        chain_id: req.chain_id,
        encoding: req.encoding,
    })
}

pub fn verify_typed_data(req: VerifyTypedDataRequest) -> Result<VerifyTypedDataResponse, AppError> {
    let signature: Signature = req
        .signature_hex
        .parse()
        .map_err(|e| AppError::BadRequest(format!("invalid signature: {e}")))?;
    let expected = parse_address(&req.expected_address)?;
    let typed_data: TypedData = serde_json::from_value(req.typed_data)
        .map_err(|e| AppError::BadRequest(format!("invalid typed_data payload: {e}")))?;
    let digest = typed_data
        .encode_eip712()
        .map_err(|e| AppError::BadRequest(format!("typed data digest failed: {e}")))?;
    let recovered = signature
        .recover(digest)
        .map_err(|e| AppError::BadRequest(format!("failed to recover signer: {e}")))?;

    Ok(VerifyTypedDataResponse {
        valid: recovered == expected,
        recovered_address: format!("{recovered:#x}"),
        chain_id: req.chain_id,
        encoding: MessageEncoding::Eip712,
        digest_hex: format!("0x{}", hex::encode(digest)),
    })
}

pub fn prepare_transfer(req: PrepareTransferRequest) -> Result<PrepareTransferResponse, AppError> {
    if req.chain_id == 0 {
        return Err(AppError::BadRequest("chain_id must be non-zero".into()));
    }
    let _ = parse_address(&req.from_address)?;

    let max_fee_per_gas_wei = req
        .max_fee_per_gas_wei
        .unwrap_or_else(|| "30000000000".into());
    let max_priority_fee_per_gas_wei = req
        .max_priority_fee_per_gas_wei
        .unwrap_or_else(|| "1000000000".into());
    let nonce = req.nonce.unwrap_or(0);

    let (operation, to, value_wei, data_hex, gas_limit) = match req.asset {
        TransferAsset::Native => {
            let to = parse_address(&req.to_address)?;
            let _ = parse_u256("amount", &req.amount)?;
            (
                OperationKind::TransferNative,
                format!("{to:#x}"),
                req.amount,
                "0x".into(),
                req.gas_limit.unwrap_or(21_000),
            )
        }
        TransferAsset::Erc20 {
            token_address,
            decimals: _,
        } => {
            let token = parse_address(&token_address)?;
            let recipient = parse_address(&req.to_address)?;
            let amount = parse_u256("amount", &req.amount)?;
            let selector = &id("transfer(address,uint256)")[..4];
            let encoded = encode(&[Token::Address(recipient), Token::Uint(amount)]);
            let mut calldata = Vec::with_capacity(4 + encoded.len());
            calldata.extend_from_slice(selector);
            calldata.extend_from_slice(&encoded);

            (
                OperationKind::TransferErc20,
                format!("{token:#x}"),
                "0".into(),
                format!("0x{}", hex::encode(calldata)),
                req.gas_limit.unwrap_or(65_000),
            )
        }
    };

    Ok(PrepareTransferResponse {
        operation,
        transaction: PreparedTransaction {
            to,
            value_wei,
            data_hex,
            gas_limit,
            max_fee_per_gas_wei,
            max_priority_fee_per_gas_wei,
            nonce,
        },
    })
}

pub async fn handle_sign_intent(
    state: &AppState,
    req: SignIntentRequest,
) -> Result<SignIntentResponse, AppError> {
    let request_id = Uuid::new_v4();
    let decision = state.policy_engine.evaluate(&req)?;

    let signed_artifact = match decision {
        PolicyDecision::Approved => {
            let envelope = SignerEnvelope {
                request_id,
                tenant_id: req.tenant_id.clone(),
                wallet_id: req.wallet_id.clone(),
                chain_id: req.chain_id,
                from_address: req.from_address.clone(),
                payload: req.payload,
            };
            Some(state.signer.sign(envelope).await?)
        }
        PolicyDecision::Denied | PolicyDecision::RequiresReview => None,
    };

    Ok(SignIntentResponse {
        request_id,
        decision,
        execution_mode: state.signer.execution_mode(),
        signed_artifact,
        created_at: chrono::Utc::now(),
    })
}

pub async fn simulate_transaction(
    state: &AppState,
    req: SimulateTransactionRequest,
) -> Result<SimulateTransactionResponse, AppError> {
    if req.chain_id == 0 {
        return Err(AppError::BadRequest("chain_id must be non-zero".into()));
    }
    state.simulator.simulate(&req).await
}

fn parse_address(value: &str) -> Result<Address, AppError> {
    value
        .parse()
        .map_err(|e| AppError::BadRequest(format!("invalid address `{value}`: {e}")))
}

fn parse_u256(label: &str, value: &str) -> Result<U256, AppError> {
    U256::from_dec_str(value)
        .map_err(|e| AppError::BadRequest(format!("invalid {label} `{value}`: {e}")))
}

fn parse_bytes_hex(label: &str, value: &str) -> Result<Vec<u8>, AppError> {
    let trimmed = value.strip_prefix("0x").unwrap_or(value);
    hex::decode(trimmed)
        .map_err(|e| AppError::BadRequest(format!("invalid {label} `{value}`: {e}")))
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use serde_json::json;

    use super::*;
    use crate::model::{PolicyContext, VerifyTypedDataRequest};

    #[test]
    fn verify_known_eip191_signature() {
        let req = VerifyMessageRequest {
            chain_id: 1,
            expected_address: "0x8f3a20f605217d87DcC2f1F7c36c08f007550961".into(),
            message: "hello ai-wallet".into(),
            signature_hex: "0xc4827b54c87c595f4395e8dba581616b12fb5f49b191d87cd4d00a616361bcbe46e7108c0828a661571a74fedd0faec8ae09ee7cbbd5e2015cb5e21e2a63d4831b".into(),
            encoding: MessageEncoding::Eip191,
        };

        let result = verify_message(req).expect("verification succeeds");
        assert!(result.valid);
    }

    #[tokio::test]
    async fn verify_known_eip712_signature() {
        let typed_data = json!({
            "types": {
                "EIP712Domain": [
                    {"name":"name","type":"string"},
                    {"name":"version","type":"string"},
                    {"name":"chainId","type":"uint256"},
                    {"name":"verifyingContract","type":"address"}
                ],
                "Mail": [
                    {"name":"contents","type":"string"}
                ]
            },
            "primaryType": "Mail",
            "domain": {
                "name":"AI Wallet",
                "version":"1",
                "chainId":1,
                "verifyingContract":"0xCcCCccccCCCCcCCCCCCcCcCccCcCCCcCcccccccC"
            },
            "message": {
                "contents":"hello typed data"
            }
        });

        let wallet = LocalWallet::from_str(
            "0x4f3edf983ac636a65a842ce7c78d9aa706d3b113bce036f4f5f1b1b09a2f5b5d",
        )
        .expect("wallet");
        let typed: TypedData =
            serde_json::from_value(typed_data.clone()).expect("typed data parse");
        let signature = wallet
            .sign_typed_data(&typed)
            .await
            .expect("sign typed data");

        let result = verify_typed_data(VerifyTypedDataRequest {
            chain_id: 1,
            expected_address: "0x8f3a20f605217d87DcC2f1F7c36c08f007550961".into(),
            typed_data,
            signature_hex: signature.to_string(),
        })
        .expect("verify typed data");

        assert!(result.valid);
    }

    #[test]
    fn policy_requires_review_for_large_value() {
        let engine = PolicyEngine;
        let req = SignIntentRequest {
            tenant_id: "tenant-a".into(),
            wallet_id: "wallet-1".into(),
            chain_id: 1,
            from_address: "0x90f8bf6a479f320ead074411a4b0e7944ea8c9c1".into(),
            operation: OperationKind::TransferNative,
            payload: SignPayload::RawMessage {
                message: "approve".into(),
            },
            policy_context: PolicyContext {
                actor: "agent".into(),
                purpose: "rebalance".into(),
                source_ip: "127.0.0.1".into(),
                idempotency_key: "abc".into(),
                max_value_wei: "10000000000000001".into(),
            },
        };

        let decision = engine.evaluate(&req).expect("policy evaluation");
        assert!(matches!(decision, PolicyDecision::RequiresReview));
    }

    #[test]
    fn prepare_erc20_transfer_calldata() {
        let result = prepare_transfer(PrepareTransferRequest {
            chain_id: 1,
            from_address: "0x8f3a20f605217d87DcC2f1F7c36c08f007550961".into(),
            to_address: "0x1111111111111111111111111111111111111111".into(),
            asset: TransferAsset::Erc20 {
                token_address: "0x2222222222222222222222222222222222222222".into(),
                decimals: 18,
            },
            amount: "1000000000000000000".into(),
            gas_limit: None,
            max_fee_per_gas_wei: None,
            max_priority_fee_per_gas_wei: None,
            nonce: Some(9),
        })
        .expect("prepare transfer");

        assert!(matches!(result.operation, OperationKind::TransferErc20));
        assert!(result.transaction.data_hex.starts_with("0xa9059cbb"));
        assert_eq!(result.transaction.gas_limit, 65_000);
    }

    #[tokio::test]
    async fn static_simulation_warns_on_low_gas() {
        let simulator = StaticSimulator::new(None);
        let result = simulator
            .simulate(&SimulateTransactionRequest {
                chain_id: 1,
                from_address: "0x8f3a20f605217d87DcC2f1F7c36c08f007550961".into(),
                to: "0x1111111111111111111111111111111111111111".into(),
                value_wei: "0".into(),
                data_hex: "0x".into(),
                gas_limit: 20_000,
            })
            .await
            .expect("simulate");

        assert!(!result.success);
        assert!(matches!(result.mode, SimulationMode::Static));
        assert!(!result.warnings.is_empty());
    }
}
