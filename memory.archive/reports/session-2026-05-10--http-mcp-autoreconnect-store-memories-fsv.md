---
namespace: reports
created: 2026-05-10T21:04:10Z
updated: 2026-05-10T21:04:10Z
status: active
tags: report, mcp, streamable-http, store_memories, fsv, d-drive
---

# Session Report: 2026-05-10 - HTTP MCP Autoreconnect And store_memories FSV

## Task: Fix active MCP reconnect/tool-registry root cause and prove D-backed durable storage works through real MCP writes.
## Outcome: complete

## Summary

Reviewed the resumed D-backed MCP work and official MCP/Claude Code docs. The durable data root was already on `/mnt/d/contextgraph`, and the project had already switched `cgreality` to a Streamable HTTP endpoint backed by a user systemd service. The remaining live failure was that `store_memories` over HTTP hit `cluster_manager insert failed ... Dimension mismatch: expected 768, actual 0`.

The source tree already contained the disabled-embedder topology fix in HEAD `b163bddb`, but the running release binary had been built before that commit. Rebuilt the release binary, restarted `contextgraph-mcp-http.service`, and verified Claude Code reports `cgreality` as connected over HTTP. Then ran manual FSV against the real D-backed RocksDB, with exact `ldb` content CF readback and edge-case count checks.

## Files Created or Modified in the Repo

- `memory/decisions/2026-05-10--cgreality-mcp-uses-streamable-http-systemd.md` - records HTTP systemd MCP decision.
- `memory/discoveries/2026-05-10--store-memories-cluster-disabled-embedder-dimension-root-cause.md` - records disabled E5/E11 root cause.
- `memory/journal/2026-05-10--failure-http-mcp-fsv-harness-ldb-and-binary-grep.md` - records verification harness failures.
- `memory/journal/2026-05-10--success-http-mcp-autoreconnect-store-memories-fsv.md` - records final successful FSV.
- `memory/reports/session-2026-05-10--http-mcp-autoreconnect-store-memories-fsv.md` - this report.

## Files Written or Updated in ./memory/

- `decisions/2026-05-10--cgreality-mcp-uses-streamable-http-systemd.md`
- `discoveries/2026-05-10--store-memories-cluster-disabled-embedder-dimension-root-cause.md`
- `journal/2026-05-10--failure-http-mcp-fsv-harness-ldb-and-binary-grep.md`
- `journal/2026-05-10--success-http-mcp-autoreconnect-store-memories-fsv.md`
- `reports/session-2026-05-10--http-mcp-autoreconnect-store-memories-fsv.md`

## Decisions Made

- Keep `cgreality` on Streamable HTTP through `contextgraph-mcp-http.service` because Claude Code reconnects HTTP servers automatically and stdio servers do not.
- Keep D-backed storage fail-closed: the systemd service requires the binary, `/mnt/d/contextgraph`, and D-backed model assets before it starts.

## Discoveries

- The live `store_memories` failure was caused by a stale release binary, not by missing source changes. HEAD already had disabled E5/E11 gated out of clustering and indexes.
- Verification harnesses must use `0x` UUID hex with `ldb --key_hex get`.
- Raw RocksDB grep output can contain binary bytes; verification scripts must decode safely or use filename-only grep.

## Failures

- FSV `20260510T210035Z` failed at exact `ldb get` because UUID hex lacked the required `0x` prefix.
- FSV `20260510T210214Z` failed after exact `ldb get` because binary grep output was decoded as strict UTF-8.

## Successes

- Release binary rebuilt from `b163bddb` with SHA-256 `2b189bbb1e0829fd3b8994918d11ab2ab1229d1e4f6872c5461112fc4c1f56f9`.
- `contextgraph-mcp-http.service` is enabled, active, and listening on `127.0.0.1:3100` and `127.0.0.1:3101` with PID `38908`.
- `claude mcp get cgreality` reports `Status: connected`, `Type: http`, `URL: http://127.0.0.1:3101/mcp`.
- Final FSV passed at `/mnt/d/contextgraph/logs/http-mcp-fsv-20260510T210327Z.json`.
- Exact content CF readback proved fingerprint `c97e655d-1018-4a44-9551-a9d8334979aa` contains token `FSV_STORE_MEMORIES_DDRIVE_HTTP_20260510T210327Z_PID38908`.

## Tests / Build

- `cargo fmt --all -- --check` passed.
- `cargo test -p context-graph-core test_insert_single_memory -- --nocapture` passed.
- `cargo test -p context-graph-core test_updates_status -- --nocapture` passed.
- `cargo test -p context-graph-mcp server::http_transport::tests -- --nocapture` passed.
- `cargo test -p context-graph-mcp --test cli_args -- --nocapture` passed.
- `cargo check -p context-graph-mcp --bin context-graph-mcp` passed.
- `cargo build -p context-graph-mcp --bin context-graph-mcp --release` passed.

## Resume Here Next Session

- Start from this report if continuing MCP transport/storage verification.
- The service and `.mcp.json` are already HTTP based. Do not revert to stdio unless a user explicitly asks.
- The final FSV JSON is the strongest evidence artifact: `/mnt/d/contextgraph/logs/http-mcp-fsv-20260510T210327Z.json`.
- Watch for old log lines before `2026-05-10T20:56:10Z`; the pre-rebuild daemon logged the dimension mismatch before the new binary was started.

## Confidence: high

The final state was verified through live HTTP MCP protocol calls, systemd status, TCP listeners, PID file agreement, `claude mcp get`, exact RocksDB content CF readback, RocksDB CF counts, search readback, and four edge-case no-mutation checks.
