use std::sync::Arc;

use ethers_core::{
    types::{Address, Signature},
    utils::hash_message,
};
use uuid::Uuid;

use crate::{
    error::AppError,
    model::{
        ExecutionMode, OperationKind, PolicyContext, PolicyDecision, SignIntentRequest,
        SignIntentResponse, SignedArtifact, VerifyMessageRequest, VerifyMessageResponse,
    },
};

pub struct AppState {
    pub policy_engine: PolicyEngine,
    pub signer: Arc<dyn SignerGateway>,
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

        if value > 10_000_000_000_000_000u128 {
            return Ok(PolicyDecision::RequiresReview);
        }

        if matches!(req.operation, OperationKind::ContractCall)
            && req.policy_context.purpose.is_empty()
        {
            return Ok(PolicyDecision::Denied);
        }

        Ok(PolicyDecision::Approved)
    }
}

pub trait SignerGateway: Send + Sync {
    fn sign(&self, request_id: Uuid, req: &SignIntentRequest) -> Result<SignedArtifact, AppError>;
    fn execution_mode(&self) -> ExecutionMode;
}

pub struct MockSigner;

impl SignerGateway for MockSigner {
    fn sign(&self, request_id: Uuid, req: &SignIntentRequest) -> Result<SignedArtifact, AppError> {
        let digest = format!(
            "mock:{}:{}:{}:{}",
            request_id, req.chain_id, req.wallet_id, req.policy_context.idempotency_key
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

pub fn handle_sign_intent(
    state: &AppState,
    req: SignIntentRequest,
) -> Result<SignIntentResponse, AppError> {
    let request_id = Uuid::new_v4();
    let decision = state.policy_engine.evaluate(&req)?;

    let signed_artifact = match decision {
        PolicyDecision::Approved => Some(state.signer.sign(request_id, &req)?),
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

fn parse_address(value: &str) -> Result<Address, AppError> {
    value
        .parse()
        .map_err(|e| AppError::BadRequest(format!("invalid address `{value}`: {e}")))
}

#[allow(dead_code)]
fn _validate_policy_context(_context: &PolicyContext) -> Result<(), AppError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use ethers_core::types::Signature;

    use super::*;
    use crate::model::{MessageEncoding, PolicyContext, SignPayload};

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
    fn signature_parser_accepts_hex() {
        let parsed: Signature = "0x7740dced0f085f955f97d6eed95f1dd4cb631651b570a9fd547b10411ff784b91f4fd44b58f74af8df52e8a0eec08f050bbcdf2d8d7dbf9d4a81098053ec1dc01c"
            .parse()
            .expect("parse signature");
        assert_eq!(parsed.v, 28);
    }
}
