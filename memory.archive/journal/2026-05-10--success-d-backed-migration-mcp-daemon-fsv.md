---
namespace: journal
agent_id: codex
timestamp: 2026-05-10T20:08:10Z
status: active
tags: success, d-drive, migration, mcp, fsv
---

# Success: D-Backed Migration And MCP Daemon FSV

## What Happened

Migrated existing ContextGraph durable data from root/home/tmp locations into `/mnt/d/contextgraph`, replaced old source paths with symlinks, fixed the migration script JSON logging bug, and restarted MCP through a D-backed daemon/proxy design.

## Verification

- Migration summary: `/mnt/d/contextgraph/logs/migration-20260510T194920Z.json`
- Migration JSONL: `/mnt/d/contextgraph/logs/migration-20260510T194920Z.jsonl`
- MCP wrapper handshake output: `/mnt/d/contextgraph/logs/mcp-wrapper-fsv-20260510T200445Z.jsonl`
- MCP daemon status output: `/mnt/d/contextgraph/logs/mcp-daemon-status-fsv-20260510T200509Z.jsonl`
- Manual learning RocksDB FSV log: `/mnt/d/contextgraph/logs/manual-fsv-learning-20260510T192654Z.log`

## Physical State Readback

- `/home/cabdru/.contextgraph -> /mnt/d/contextgraph/user-data`
- `/home/cabdru/.cache/contextgraph-mejepa-corpus -> /mnt/d/contextgraph/cache/mejepa-corpus`
- `/home/cabdru/.cache/contextgraph-swebench -> /mnt/d/contextgraph/cache/contextgraph-swebench`
- `/home/cabdru/.cache/cgreality -> /mnt/d/contextgraph/state/cgreality`
- `/home/cabdru/.cache/contextgraph-docker-config -> /mnt/d/contextgraph/cache/contextgraph-docker-config`
- `/tmp/contextgraph-mejepa-train-fsv -> /mnt/d/contextgraph/migrated/tmp/contextgraph-mejepa-train-fsv`

D physical data readback showed `/mnt/d/contextgraph/user-data` at 106G, ME-JEPA corpus cache at 1.7G, SWE-bench cache at 471M, migrated tmp data at 64M, and D drive with about 3.9T available.

## Edge Cases

- Idempotent daemon ensure: before and after PID stayed `8450`, process count stayed one, pid files stayed `8450`.
- Invalid mode: `--mode invalid --daemon-server-only --daemon-port 3999` exited with rc 1 and did not bind the port.
- Missing model path: isolated D-backed storage and nonexistent model dir failed closed with a clear "Models directory not found" error and no listener.

## Build And Syntax

- `cargo fmt --all -- --check`: pass
- `cargo check -p context-graph-mcp --bin context-graph-mcp`: pass
- `cargo build -p context-graph-mcp --bin context-graph-mcp --release`: pass
- `bash -n` on modified scripts: pass
- `.mcp.json` parsed with `jq`: pass
