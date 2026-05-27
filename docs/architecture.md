# Architecture

## Problem statement

An AI agent should never receive raw private keys or direct host access to a production wallet. Instead, it should call a narrow API that:

- validates the request payload,
- evaluates policy and spending constraints,
- records an immutable audit trail,
- forwards approved requests to an isolated signer runtime,
- returns only the signed artifact or transaction result.

## Trust boundaries

### 1. AI client

- Sends signed or authenticated API requests.
- Does not handle private keys.
- Receives only policy-approved outputs.

### 2. Wallet API gateway

- Authenticates tenants, agents, and service identities.
- Normalizes EVM requests into canonical sign intents.
- Performs simulation, policy checks, idempotency checks, and logging.
- Has no access to raw signing material.

### 3. Isolated signer

- Owns or proxies access to the actual signing key.
- Runs in a separate trust domain:
  - AWS Nitro Enclave,
  - SGX/TDX,
  - Firecracker/gVisor-based microVM,
  - HSM-backed dedicated signer service.
- Accepts only a minimal binary/API protocol from the gateway.
- Returns signatures, never private keys.

### 4. Broadcast worker

- Takes an approved signed transaction and submits it to an RPC endpoint.
- Manages retries and chain-specific error handling.

## Core request flow

1. AI client calls `POST /v1/evm/sign-intent`.
2. Gateway authenticates the caller and verifies request freshness.
3. Gateway canonicalizes the action into an internal `SignIntent`.
4. Policy engine checks:
   - allowed chain,
   - allowed wallet,
   - max transfer value,
   - operation type,
   - destination allowlist/denylist,
   - contract call selectors,
   - time window,
   - required human review threshold.
5. If needed, gateway simulates the transaction before approval.
6. Gateway forwards the canonical digest to the isolated signer over mTLS or attested channel.
7. Isolated signer signs and returns the signature or raw signed transaction.
8. Gateway records an audit event and optionally hands off to a broadcaster.

## API-first wallet model

The API should expose business-safe primitives, not raw key operations. Recommended operation groups:

- `verify-message`: verify agent-provided proofs or user acknowledgements.
- `sign-intent`: request policy-approved signing.
- `prepare-transfer`: build a canonical EVM transfer request.
- `simulate-transaction`: estimate outcome before signing.
- `broadcast-transaction`: submit to RPC only after approval.
- `get-audit-events`: inspect who asked for what, when, and why.

## Security controls

### Authentication

- External callers: API key + HMAC request signing, or OAuth2 client credentials, or mTLS.
- Internal components: SPIFFE/SPIRE or mTLS service identities.

Current implementation:

- HMAC request authentication on `/v1/*`.
- Signer RPC over HTTPS with mutual TLS.

### Authorization

- Tenant-scoped wallets.
- Per-agent operation scopes such as `wallet.verify`, `wallet.sign_message`, `wallet.transfer`.
- Fine-grained policy by chain, wallet, token, contract selector, and spend limit.

### Isolation

- Keep signing keys outside the API gateway process.
- Disable outbound internet access from the signer except to attestation/HSM dependencies.
- Use short-lived credentials and rotate signer certificates.

### Auditing

- Write immutable request, decision, and signature metadata.
- Persist request payload digests, not sensitive plaintext if avoidable.
- Include `tenant_id`, `wallet_id`, `actor`, `source_ip`, and `idempotency_key`.

Current implementation:

- Memory-backed audit in development.
- Postgres-backed audit persistence when `DATABASE_URL` is configured.

### Replay protection

- Require idempotency keys.
- Enforce timestamp windows or nonces on client requests.
- Reject duplicate intents unless explicitly retried.

## EVM-specific design notes

- Use EIP-155 chain IDs to prevent cross-chain replay.
- Support both EIP-191 and EIP-712.
- Treat contract calls differently from transfers:
  - decode selector and arguments,
  - maintain allowlists,
  - simulate before approval.
- Support hosted and external nonce management.

## Suggested deployment topology

- `wallet-api`: Rust Axum service exposed behind an API gateway.
- `policy-engine`: in-process first, external policy service later if needed.
- `signer-worker`: isolated runtime with narrow signing RPC.
- `audit-db`: Postgres for request and decision metadata.
- `queue`: optional NATS/Kafka/SQS for async signing and broadcasting.

## Delivery phases

### Phase 1

- EVM message verification.
- Mock signing gateway.
- Policy engine and API contract.
- Audit schema design.

### Phase 2

- Isolated signer protocol abstraction with a local-dev signer adapter.
- EIP-712 verification and typed-data signing flow.
- Native and ERC-20 transfer preparation.
- Static transaction simulation with an RPC-ready extension point.

### Phase 3

- HMAC API authentication.
- Signer RPC over mTLS with a dedicated `signer-worker`.
- Postgres audit persistence.
- Async broadcast worker and status tracking.
