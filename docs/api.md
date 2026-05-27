# API Contract

## `GET /healthz`

Returns `200 OK` with `ok`.

## `GET /v1/info`

Returns service metadata.

Example response:

```json
{
  "service": "ai-wallet",
  "version": "0.1.0",
  "supported_chains": ["ethereum", "base", "arbitrum", "optimism", "bsc"],
  "isolation_model": "api gateway + isolated signer worker + policy engine"
}
```

## `POST /v1/evm/verify-message`

Verifies an EIP-191 message signature and recovers the signer address.

Request:

```json
{
  "chain_id": 1,
  "expected_address": "0x8f3a20f605217d87DcC2f1F7c36c08f007550961",
  "message": "hello ai-wallet",
  "signature_hex": "0xc4827b54c87c595f4395e8dba581616b12fb5f49b191d87cd4d00a616361bcbe46e7108c0828a661571a74fedd0faec8ae09ee7cbbd5e2015cb5e21e2a63d4831b",
  "encoding": "eip191"
}
```

Response:

```json
{
  "valid": true,
  "recovered_address": "0x8f3a20f605217d87dcc2f1f7c36c08f007550961",
  "chain_id": 1,
  "encoding": "eip191"
}
```

## `POST /v1/evm/sign-intent`

Accepts an EVM sign or payment intent after policy evaluation.

Request:

```json
{
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
}
```

Approved response:

```json
{
  "request_id": "uuid",
  "decision": "approved",
  "execution_mode": "mock",
  "signed_artifact": {
    "signature_hex": "0x...",
    "digest_hex": "0x..."
  },
  "created_at": "2026-05-28T01:00:00Z"
}
```

Review-required response:

```json
{
  "request_id": "uuid",
  "decision": "requires_review",
  "execution_mode": "mock",
  "signed_artifact": null,
  "created_at": "2026-05-28T01:00:00Z"
}
```

## Error model

Errors are returned as:

```json
{
  "error": "bad request: invalid signature: ..."
}
```
