# Phase 17 State And Invariants

Phase 17 ships the first executable HTTP surface for BlackBoxFix. It is a
single-ledger, single-writer server intended for local/shared deployment behind
operator-controlled TLS and network policy.

## Shipped Surface

Command:

```bash
blackbox server serve \
  --ledger PATH \
  [--bind 127.0.0.1:7607] \
  [--api-key-env BLACKBOX_API_KEY]
```

Connector:

- `blackbox_http_server_v1`

Ledger record kind:

- `http_request_audit`

Phase 17 bumps the logical ledger schema to `LEDGER_SCHEMA_VERSION = 11`. The
physical SQLite table shape is unchanged, but old schema-version ledgers are
wiped on `blackbox init`; there is no migration shim.

## Source Of Truth

Primary source of truth:

- SQLite `ledger_records` rows in the user-provided ledger;
- SQLite `blackbox_metadata` count/tip rows;
- real HTTP request/response traffic to the server process.

The server writes a `http_request_audit` row for every protected request after
it verifies the ledger and acquires the writer lock. The audit row records
route id, tenant/audit scope, status code, error code, request body hash,
response body hash, bind address, body limit, and single-writer state.

## Served Endpoints

```text
GET  /v1/health
POST /v1/runs
GET  /v1/runs/{run_id}
POST /v1/runs/{run_id}/steps
POST /v1/tool-calls
GET  /v1/dashboard/summary
GET  /v1/dashboard/runs
GET  /v1/dashboard/runs/{run_id}/timeline
GET  /v1/dashboard/evidence
GET  /v1/dashboard/verifier-status
GET  /v1/dashboard/errors
GET  /v1/dashboard/exports
GET  /v1/dashboard/pilot-packages
GET  /v1/dashboard/risk-assessments
```

Protected endpoints require `Authorization: Bearer <api-key>` and
`X-Blackbox-Tenant`. POST endpoints also require
`Content-Type: application/json`.

## Invariants

- API keys are read only from the configured environment variable; they are
  never accepted as a command-line argument.
- API keys must be at least 32 non-whitespace characters.
- Protected routes fail closed without a valid bearer token.
- Request body tenant must equal `X-Blackbox-Tenant`.
- Tenant-scoped dashboard reads filter physical ledger rows before building
  read models.
- Non-loopback binds are refused unless the operator provides both
  `--allow-remote-bind` and `--tls-terminated-by NAME`.
- POST request bodies are limited to 64 KiB.
- Unknown JSON fields are rejected.
- HTTP-created runs persist `runtime.framework="http_server"`.
- Every protected request, including failures, appends one
  `http_request_audit` row.

## Error Codes

| Code | Meaning |
|---|---|
| `HTTP_API_KEY_MISSING` | configured API key env var is absent |
| `HTTP_API_KEY_INVALID` | configured API key is too short or malformed |
| `HTTP_AUTH_MISSING` | request omitted bearer auth |
| `HTTP_AUTH_SCHEME_INVALID` | request used a non-bearer auth scheme |
| `HTTP_AUTH_INVALID` | bearer key hash did not match |
| `HTTP_TENANT_HEADER_MISSING` | protected request omitted tenant scope |
| `HTTP_TENANT_MISMATCH` | request body tenant did not match header tenant |
| `HTTP_CONTENT_TYPE_REQUIRED` | POST body omitted content type |
| `HTTP_CONTENT_TYPE_INVALID` | POST body was not `application/json` |
| `HTTP_JSON_REQUIRED` | POST body was empty |
| `HTTP_JSON_INVALID` | POST body did not parse as JSON |
| `HTTP_JSON_OBJECT_REQUIRED` | POST JSON body was not an object |
| `HTTP_REQUEST_SCHEMA_INVALID` | POST JSON failed strict schema validation |
| `HTTP_BODY_TOO_LARGE` | POST body exceeded the 64 KiB limit |
| `HTTP_REMOTE_BIND_REFUSED` | server was asked to bind a non-loopback address |
| `HTTP_TLS_TERMINATOR_REQUIRED` | remote bind lacked TLS terminator evidence |

## Automated FSV

```bash
cargo test --package context-graph-cli --test blackbox_phase17_http_server_fsv -- --nocapture
```

The test starts a real server process, uses real HTTP requests, and opens the
SQLite file directly for proof. It verifies:

- happy-path `POST /v1/runs` persists one `agent_run` and one audit row;
- `POST /v1/runs/{run_id}/steps` persists one `agent_step` and one audit row;
- `POST /v1/tool-calls` persists one redacted `tool_call` and one audit row;
- `GET /v1/dashboard/summary` returns tenant-scoped data and creates an audit
  row;
- invalid auth creates no business row and creates a 401 audit row;
- tenant mismatch creates no business row and creates a 400 audit row;
- oversized body creates no business row and creates a 413 audit row.

## Manual FSV Checklist

For manual verification, print direct SQLite state before and after every
trigger:

```sql
SELECT record_kind, COUNT(*)
FROM ledger_records
GROUP BY record_kind
ORDER BY record_kind;
```

Inspect the latest audit row:

```sql
SELECT sequence,
       tenant_id,
       json_extract(payload_json, '$.request.route_id'),
       json_extract(payload_json, '$.response.status_code'),
       json_extract(payload_json, '$.response.error_code')
FROM ledger_records
WHERE record_kind='http_request_audit'
ORDER BY sequence DESC
LIMIT 1;
```

Happy-path expected final state after one run, one step, one tool call, and
one dashboard read:

- `agent_run=1`
- `agent_step=1`
- `tool_call=1`
- `http_request_audit=4`

Edge cases must include invalid API key, tenant mismatch, and oversized JSON
body. Each edge case must leave `agent_run` unchanged and increment
`http_request_audit` by exactly one with the expected status/error code.
