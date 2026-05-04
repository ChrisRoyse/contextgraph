# Hardening Review

## Closed Gap

The all-phase review found a cross-phase API contract hole: the
machine-readable API surface mixed routes served by `blackbox server serve`
with command-only or future HTTP endpoint shapes. That could mislead an
operator, SDK generator, or integration test into calling an endpoint that the
router did not implement.

The root cause was a single `endpoints` registry field carrying two different
meanings: shipped HTTP routes and planned HTTP-facing equivalents for existing
CLI capabilities.

## Hardened Contract

`blackbox api surface [--ledger PATH]` now separates the surfaces:

- `endpoints` contains only routes served by the Phase 17 HTTP router.
- `served_http_endpoints` is the explicit served-route list with route ids,
  authentication requirements, ledger record kind, and `implemented=true`.
- `planned_http_endpoints` contains command-only or future HTTP shapes with
  `implemented=false`, a CLI command equivalent, source phase, and
  `expected_http_status_until_implemented=404`.
- `api_surface_policy` states that served-route proof comes from the HTTP
  router plus readback, not from command return values alone.

This keeps the system fail-closed: if a route is not actually wired into the
server, it is not advertised as available.

## Verified Source Of Truth

The source of truth for this hardening is:

- API registry JSON emitted by `blackbox api surface`.
- Real HTTP responses from `blackbox server serve`.
- SQLite `ledger_records` readback, especially `http_request_audit` rows.

The cross-phase FSV test starts the real server, sends real requests, verifies
served endpoints return 200/401 as appropriate, verifies planned endpoints
return 404, and then reads SQLite directly to prove the resulting state.

## Current Proof

Automated proof:

```bash
cargo test --package context-graph-cli --test blackbox_cross_phase_api_surface_hardening_fsv -- --nocapture
cargo test --package context-graph-cli --test blackbox_phase17_http_server_fsv -- --nocapture
cargo check --package context-graph-cli
```

Manual proof:

- `POST /v1/runs` persisted one `agent_run` and one successful
  `http_request_audit` row.
- Authenticated `GET /v1/dashboard/summary` returned the persisted run count
  and added a 200 audit row.
- Unauthenticated `GET /v1/dashboard/summary` returned
  `HTTP_AUTH_MISSING` and added a 401 audit row.
- Planned `POST /v1/sdk/events` returned 404 and did not mutate the ledger.
- Planned `POST /v1/replay` returned 404 and did not mutate the ledger.

Final manual SQLite state:

```text
agent_run|1
http_request_audit|3
2|POST /v1/runs|200|
3|GET /v1/dashboard/summary|200|
4|GET /v1/dashboard/summary|401|HTTP_AUTH_MISSING
```
