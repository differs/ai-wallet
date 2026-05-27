# ai-wallet

Rust implementation skeleton for an AI-facing wallet control plane. The service exposes EVM wallet operations as auditable APIs, while keeping real key usage behind an isolated signer boundary.

## Goals

- Let AI agents access assets only through constrained APIs instead of raw private keys.
- Wrap payment, message signing, and future transaction signing into policy-controlled requests.
- Keep real signing inside an isolated execution environment with narrow, attestable interfaces.
- Start with EVM chains and a Rust codebase.

## Current scope

- `POST /v1/evm/verify-message`: verify an EIP-191 message signature against an expected address.
- `POST /v1/evm/verify-typed-data`: verify an EIP-712 typed-data signature.
- `POST /v1/evm/prepare-transfer`: build canonical native or ERC-20 transfers.
- `POST /v1/evm/simulate-transaction`: validate and pre-simulate transaction execution.
- `POST /v1/evm/sign-intent`: submit a signing or payment intent to the policy engine and signer gateway.
- `GET /v1/info`: inspect supported chains and deployment model.
- `GET /healthz`: health check.

This repository now ships two signer modes:

- `mock`: deterministic fake signatures for API and policy testing.
- `local-dev`: dev-only signer backed by `AI_WALLET_DEV_PRIVATE_KEY`, shaped like the future isolated signer boundary.

## Architecture

See [docs/architecture.md](/home/de/works/ai-wallet/docs/architecture.md) for the system design and [docs/api.md](/home/de/works/ai-wallet/docs/api.md) for the API contract.

## Run

```bash
cargo run
```

Server default:

- Bind address: `127.0.0.1:8080`

Environment:

- `AI_WALLET_BIND=127.0.0.1:8080`
- `AI_WALLET_SIGNER_MODE=mock|local-dev`
- `AI_WALLET_DEV_PRIVATE_KEY=0x...`
- `AI_WALLET_RPC_URL=https://...`

## Test

```bash
cargo test
```

## Example requests

Verify an EIP-191 signature:

```bash
curl -X POST http://127.0.0.1:8080/v1/evm/verify-message \
  -H 'content-type: application/json' \
  -d '{
    "chain_id": 1,
    "expected_address": "0x8f3a20f605217d87DcC2f1F7c36c08f007550961",
    "message": "hello ai-wallet",
    "signature_hex": "0xc4827b54c87c595f4395e8dba581616b12fb5f49b191d87cd4d00a616361bcbe46e7108c0828a661571a74fedd0faec8ae09ee7cbbd5e2015cb5e21e2a63d4831b",
    "encoding": "eip191"
  }'
```

Verify typed data:

```bash
curl -X POST http://127.0.0.1:8080/v1/evm/verify-typed-data \
  -H 'content-type: application/json' \
  -d '{
    "chain_id": 1,
    "expected_address": "0x8f3a20f605217d87DcC2f1F7c36c08f007550961",
    "typed_data": {
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
    },
    "signature_hex": "0x..."
  }'
```

Submit a sign intent:

```bash
curl -X POST http://127.0.0.1:8080/v1/evm/sign-intent \
  -H 'content-type: application/json' \
  -d '{
    "tenant_id": "ops",
    "wallet_id": "hot-wallet-1",
    "chain_id": 1,
    "from_address": "0x90f8bf6a479f320ead074411a4b0e7944ea8c9c1",
    "operation": "transfer_native",
    "payload": {
      "kind": "transaction",
      "to": "0xffcf8fdee72ac11b5c542428b35eef5769c409f0",
      "value_wei": "1000000000000000",
      "data_hex": "0x",
      "gas_limit": 21000,
      "max_fee_per_gas_wei": "30000000000",
      "max_priority_fee_per_gas_wei": "1000000000",
      "nonce": 7
    },
    "policy_context": {
      "actor": "treasury-agent",
      "purpose": "market-making rebalance",
      "source_ip": "127.0.0.1",
      "idempotency_key": "rebalance-20260528-001",
      "max_value_wei": "1000000000000000"
    }
  }'
```

Prepare an ERC-20 transfer:

```bash
curl -X POST http://127.0.0.1:8080/v1/evm/prepare-transfer \
  -H 'content-type: application/json' \
  -d '{
    "chain_id": 1,
    "from_address": "0x8f3a20f605217d87DcC2f1F7c36c08f007550961",
    "to_address": "0x1111111111111111111111111111111111111111",
    "asset": {
      "kind": "erc20",
      "token_address": "0x2222222222222222222222222222222222222222",
      "decimals": 18
    },
    "amount": "1000000000000000000",
    "nonce": 9
  }'
```

## Next implementation steps

1. Replace `local-dev` signer with a real enclave/HSM RPC transport plus attestation or mTLS.
2. Add live RPC-backed simulation, nonce sourcing, and fee estimation.
3. Persist audit logs and request state in Postgres.
4. Add broadcast workers and reconciliation.
5. Add caller authentication and tenant-scoped authorization.
