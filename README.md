# ai-wallet

Rust implementation skeleton for an AI-facing wallet control plane. The service exposes EVM wallet operations as auditable APIs, while keeping real key usage behind an isolated signer boundary.

## Goals

- Let AI agents access assets only through constrained APIs instead of raw private keys.
- Wrap payment, message signing, and future transaction signing into policy-controlled requests.
- Keep real signing inside an isolated execution environment with narrow, attestable interfaces.
- Start with EVM chains and a Rust codebase.

## Current scope

- `POST /v1/evm/verify-message`: verify an EIP-191 message signature against an expected address.
- `POST /v1/evm/sign-intent`: submit a signing or payment intent to the policy engine and signer gateway.
- `GET /v1/info`: inspect supported chains and deployment model.
- `GET /healthz`: health check.

This repository currently ships a `MockSigner` so the API shape, policy flow, and isolation boundary can be validated before wiring a real enclave/HSM signer.

## Architecture

See [docs/architecture.md](/home/de/works/ai-wallet/docs/architecture.md) for the system design and [docs/api.md](/home/de/works/ai-wallet/docs/api.md) for the API contract.

## Run

```bash
cargo run
```

Server default:

- Bind address: `127.0.0.1:8080`

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

## Next implementation steps

1. Replace `MockSigner` with a remote signer running inside Nitro Enclave, SGX, gVisor microVM, or an HSM-backed service.
2. Add EIP-712 typed-data verification and signing.
3. Add transaction simulation, nonce management, and broadcast submission workers.
4. Persist audit logs and request state in Postgres.
5. Add mTLS or request signing between API gateway and signer worker.
