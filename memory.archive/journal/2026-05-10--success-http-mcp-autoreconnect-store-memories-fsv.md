---
namespace: journal
created: 2026-05-10T21:04:10Z
updated: 2026-05-10T21:04:10Z
status: active
tags: success, mcp, streamable-http, store_memories, fsv, d-drive
---

# Success: HTTP MCP Autoreconnect and store_memories FSV

## What Happened

Rebuilt `/home/cabdru/contextgraph/target/release/context-graph-mcp` from HEAD `b163bddb31b7d671cb096e23dd30788e61498756`, restarted `contextgraph-mcp-http.service`, and verified Claude Code sees `cgreality` as an HTTP MCP server.

The final release binary evidence:

- Path: `/home/cabdru/contextgraph/target/release/context-graph-mcp`
- Mtime: `2026-05-10 15:54:30 -0500`
- Size: `78974080`
- SHA-256: `2b189bbb1e0829fd3b8994918d11ab2ab1229d1e4f6872c5461112fc4c1f56f9`

The final manual FSV passed:

- Log: `/mnt/d/contextgraph/logs/http-mcp-fsv-20260510T210327Z.log`
- JSON: `/mnt/d/contextgraph/logs/http-mcp-fsv-20260510T210327Z.json`
- Stored fingerprint: `c97e655d-1018-4a44-9551-a9d8334979aa`
- Stored token: `FSV_STORE_MEMORIES_DDRIVE_HTTP_20260510T210327Z_PID38908`

## Source of Truth Readback

The FSV used these source-of-truth locations:

- MCP endpoint: `http://127.0.0.1:3101/mcp`
- Health endpoint: `http://127.0.0.1:3101/healthz`
- RocksDB: `/mnt/d/contextgraph/storage/contextgraph-rocksdb`
- Content CF exact readback via `ldb --key_hex get 0xc97e655d10184a449551a9d8334979aa`
- PID files: `/mnt/d/contextgraph/state/locks/context-graph-daemon.pid` and `/mnt/d/contextgraph/storage/contextgraph-rocksdb/mcp.pid`

Final counts in the D-backed RocksDB source of truth:

```json
{
  "audit_by_target": 6,
  "audit_log": 6,
  "content": 4,
  "embedding_registry": 4,
  "fingerprints": 4,
  "source_metadata": 4
}
```

## Edge Cases Verified

- Empty `store_memories.items=[]` returned `[VALIDATION_ERROR -32602] items must contain at least one memory` and did not change RocksDB counts.
- Oversized `store_memories` batch of 65 returned `[VALIDATION_ERROR -32602] items must contain at most 64 memories, got 65` and did not change RocksDB counts.
- Invalid HTTP `Origin: http://evil.example` returned HTTP 403 and did not change RocksDB counts.
- Invalid HTTP `Accept: application/json` returned HTTP 406 and did not change RocksDB counts.

## Related

- See also: ./memory/discoveries/2026-05-10--store-memories-cluster-disabled-embedder-dimension-root-cause.md
- Tags: mcp, fsv, store_memories, streamable-http
