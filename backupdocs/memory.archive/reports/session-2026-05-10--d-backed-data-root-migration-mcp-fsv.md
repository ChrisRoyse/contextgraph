---
namespace: reports
agent_id: codex
timestamp: 2026-05-10T20:08:10Z
status: active
tags: report, d-drive, durable-storage, mcp, fsv
---

# Session Report: 2026-05-10 — D-Backed Data Root Migration And MCP FSV

## Task: Move growing durable ContextGraph data to D, restart MCP on the new binary, research MCP best practices, and prove state by manual readback.
## Outcome: complete

## Summary

Reviewed project memory, MCP config, storage paths, migration scripts, and logs. Researched official MCP lifecycle/transport docs, Claude Code MCP behavior, WSL mounted drive guidance, RocksDB checkpoint guidance, Docker storage location guidance, and XDG base directory conventions. The working design is a fail-closed D-backed data root plus a detached local daemon with a stdio proxy for MCP client compatibility.

Moved existing C/root/home/tmp data into `/mnt/d/contextgraph`, fixed migration logging so invalid JSON cannot hide verification failures, installed the missing real BGE-M3 safetensors asset on D, rebuilt the release MCP binary, and restarted the daemon. The source of truth was read back separately from return values: symlinks, D directory sizes, RocksDB evidence, daemon pid files, port listener, MCP handshake JSON, and MCP `daemon_status` output.

## Files Created or Modified in the Repo

- `.mcp.json` — cgreality now launches the wrapper with D-backed env and reality-loop mode.
- `crates/context-graph-mcp/src/main.rs` — daemon spawn preserves mode and detached daemon skips parent-death signal.
- `scripts/context-graph-mcp-wrapper.sh` — ensures daemon, then starts stdio proxy in configured mode.
- `scripts/ensure_daemon.sh` — starts detached D-backed daemon with D model/storage env.
- `scripts/restart_pipeline_services.sh` — restarts daemon with the same D-backed mode/model/storage env.
- `scripts/migrate_contextgraph_data_to_d.sh` — robust JSON event emission and fail-closed rsync diff reporting.

## Files Written or Updated in ./memory/

- `decisions/2026-05-10--d-backed-durable-data-root-and-mcp-daemon.md`
- `discoveries/2026-05-10--mcp-stdio-restart-and-bge-m3-safetensors.md`
- `journal/2026-05-10--success-d-backed-migration-mcp-daemon-fsv.md`
- `reports/session-2026-05-10--d-backed-data-root-migration-mcp-fsv.md`

## Decisions Made

- Use `/mnt/d/contextgraph` as the durable data root and fail closed if required D paths/assets are missing.
- Use a detached local daemon plus stdio proxy for MCP. This matches MCP stdio lifecycle while avoiding repeated heavyweight reloads.

## Discoveries

- The daemon was dying because parent-death tracking applied to `--daemon-server-only`.
- Daemon child startup lost `--mode reality-loop`, risking the wrong tool surface.
- BGE-M3 safetensors was missing from the D model root; a pinned Hugging Face revision was required.
- Migration summary logging had a Bash/JSON formatting bug; the copy was good, but the summary writer could emit invalid JSON until fixed.

## Failures

- First MCP restart failed because the real safetensors asset was missing. It was fixed by downloading and sha256-verifying the exact model file on D.
- First migration summary failed due invalid JSON emission. It was fixed by using Python `json.dumps` for all migration events.

## Successes

- `claude mcp get cgreality` returned `Status: ✓ Connected`.
- Daemon PID `8450` is live on `127.0.0.1:3100`; both pid files contain `8450`.
- MCP wrapper initialize/tools-list JSON parsed cleanly and listed 195 tools.
- MCP `daemon_status` readback reported `models_state:"ready"`, `pid:8450`, background tasks running, and `graph_builder:true`.
- Existing data now physically resides on D and old home/cache/tmp paths are symlinked to D.

## Tests / Build

- Tests added: none.
- Tests passing: manual FSV passed; cargo fmt/check/build passed for the touched MCP binary; script syntax and JSON checks passed.
- Build status: passing.

## Resume Here Next Session

- Start by reading this report plus the decision/discovery files above.
- Watch out for active client sessions: official MCP stdio clients do not automatically repopulate lost tool registries. If the active Claude Code session still lacks `mcp__cgreality__*` tools, use `/mcp` retry/reconnect after the server-side fix; the daemon is already warm.
- Priority: keep all new durable data paths rooted under `/mnt/d/contextgraph` or fail closed with an explicit error.
- Open question: none for this task.

## Confidence: high

The final state was verified through physical readbacks from the D filesystem, daemon pid files, TCP listener inspection, valid JSON logs, MCP protocol handshake, MCP `daemon_status`, and edge-case failure checks.
