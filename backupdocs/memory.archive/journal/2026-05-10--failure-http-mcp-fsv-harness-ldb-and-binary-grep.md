---
namespace: journal
created: 2026-05-10T21:04:10Z
updated: 2026-05-10T21:04:10Z
status: resolved
severity: low
tags: fsv, harness, ldb, grep, manual-testing
---

# Failure: HTTP MCP FSV Harness Readback Issues

## What Happened

Two manual FSV harness attempts failed after `store_memories` had already successfully written data:

1. `/mnt/d/contextgraph/logs/http-mcp-fsv-20260510T210035Z.log` failed because `ldb --key_hex get` requires UUID keys to be prefixed with `0x`.
2. `/mnt/d/contextgraph/logs/http-mcp-fsv-20260510T210214Z.log` failed because raw `grep -R -a` returned binary bytes that Python tried to decode as UTF-8.

These were harness defects, not service defects. Both runs physically wrote synthetic memories, which is why later FSV counts started above zero.

## Why It Matters

Future FSV scripts that read RocksDB directly should:

- Use `0x` before UUID hex keys when calling `ldb --key_hex get`.
- Decode subprocess output with `errors="replace"` or grep filename-only with `grep -l` when scanning RocksDB files.
- Treat verification harness bugs separately from service behavior, while still recording any durable data they created.

## Resolution

The final FSV run used `0x{uuid.hex}` for exact content CF readback and binary-safe command decoding. It passed and wrote:

- `/mnt/d/contextgraph/logs/http-mcp-fsv-20260510T210327Z.log`
- `/mnt/d/contextgraph/logs/http-mcp-fsv-20260510T210327Z.json`

## Related

- See also: ./memory/journal/2026-05-10--success-http-mcp-autoreconnect-store-memories-fsv.md
- Tags: fsv, ldb, rocksdb, verification
