---
namespace: discoveries
agent_id: codex
timestamp: 2026-05-10T20:08:10Z
status: active
tags: mcp, stdio, daemon, bge-m3, safetensors, root-cause
---

# MCP Stdio Restart And BGE-M3 Safetensors

## What Happened

Restarting the MCP service initially failed after moving state to D. The daemon log showed the real failure:

`BgeM3Dense weight load FAILED. error=Safetensors file not found at /mnt/d/cabdru/contextgraph/models/bge-m3-dense/model.safetensors`

The configured D model root was correct, but the real model asset was missing there. The BAAI/bge-m3 main revision currently exposes `pytorch_model.bin` but not `model.safetensors`; the pinned revision `50f9396f75618b3389c1fd1068a1ff58dc7b5b26` contains the required safetensors file.

## Root Causes

- The daemon spawn path lost `--mode reality-loop`, so a D-backed daemon could come up with the wrong tool surface.
- `PR_SET_PDEATHSIG` was applied to `--daemon-server-only`, so a supposedly detached daemon could die when its launcher/proxy exited.
- The old direct stdio MCP config meant reconnects depended on spawning/loading the full server process again.
- D model assets were incomplete; the correct BGE-M3 safetensors file was absent.

## Fixes

- `spawn_daemon_process` now preserves the selected mode.
- `--daemon-server-only` skips parent-death tracking; client-owned stdio/proxy processes still use it.
- `.mcp.json` launches the wrapper, which ensures one warm detached daemon and then starts the stdio proxy.
- The real BGE-M3 safetensors file was downloaded to D and sha256-verified.

## Why It Matters

This addresses the repeated pattern where MCP tools disappear from the active Claude Code session while the underlying server or runtime state may still exist. Official MCP stdio does not give the server a way to repopulate a client registry by itself; the robust server-side answer is to make reconnect cheap, explicit, and backed by a single warm daemon with durable D-rooted state.

## Related

- See also: `memory/decisions/2026-05-10--d-backed-durable-data-root-and-mcp-daemon.md`
- See also: `memory/journal/2026-05-10--success-d-backed-migration-mcp-daemon-fsv.md`
