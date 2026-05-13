---
namespace: discoveries
created: 2026-05-10T21:04:10Z
updated: 2026-05-10T21:04:10Z
status: resolved
severity: high
tags: store_memories, clustering, e5, e11, embeddings, fsv
---

# store_memories Cluster Dimension Root Cause

## What Happened

The first HTTP FSV reached the real `store_memories` handler and failed in the batch sidecar write:

```text
cluster_manager insert failed for 99e5cd0c-f089-4d4d-a376-ca3e6a43fd55: Dimension mismatch: expected 768, actual 0
```

The source root cause was the disabled embedder topology. E5 causal and E11 entity are intentionally disabled and can produce empty vectors, but the clustering and index paths previously still treated disabled slots as live spaces in some paths. That let an empty vector reach a 768-dimensional cluster/index contract.

HEAD contains the source fix in commit `b163bddb31b7d671cb096e23dd30788e61498756`: disabled E5/E11 are gated out of clustering, topic synthesis, HNSW population, and direct E11 embedding fails closed.

The running service had been built before that commit. Rebuilding the release binary and restarting the systemd service was required before the fix affected MCP.

## Why It Matters

The failure was not a storage capacity issue and not an HTTP transport bug. It was an active-topology contract bug between the embedding provider, fingerprint shape, cluster manager, and storage indexes.

The correct behavior is:

- Disabled E5/E11 slots stay addressable for old persisted records.
- Disabled E5/E11 do not participate in live clustering, search weights, HNSW indexes, or direct embedding calls.
- Enabled slots still fail closed on dimension mismatch.

## Verification

After rebuilding and restarting, manual FSV passed through the live HTTP MCP endpoint and the real D-backed RocksDB:

- Evidence JSON: `/mnt/d/contextgraph/logs/http-mcp-fsv-20260510T210327Z.json`
- Evidence log: `/mnt/d/contextgraph/logs/http-mcp-fsv-20260510T210327Z.log`
- Final stored fingerprint: `c97e655d-1018-4a44-9551-a9d8334979aa`
- Exact content CF readback contained token: `FSV_STORE_MEMORIES_DDRIVE_HTTP_20260510T210327Z_PID38908`

## Related

- See also: ./memory/decisions/2026-05-10--cgreality-mcp-uses-streamable-http-systemd.md
- See also: ./memory/reports/session-2026-05-10--http-mcp-autoreconnect-store-memories-fsv.md
- Tags: store_memories, clustering, embeddings, E5, E11
