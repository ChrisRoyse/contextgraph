---
namespace: discoveries
agent_id: agent-04-coder-analyzer
timestamp: 2026-04-17T16:50:00Z
status: active
severity: high
tags: daemon, rocksdb, constellation, phase-b, pwd
---

# Daemon on port 3100 was using wrong RocksDB dir (empty 1.9MB) — constellation not found

## What Happened
When the analyzer probed `score_against_constellation` with the Sonnets anchor
`18ef2460-9ab6-44fc-8d7e-8d7513174ac8`, the daemon returned
`Constellation ... not found`.

Investigation:
- `ss -tlnp` → port 3100 held by PID 2346870 `./target/release/context-graph-mcp --daemon-server-only --daemon-port 3100`.
- `/proc/2346870/fd/*` symlinks → the daemon's open RocksDB files were all under `/home/cabdru/contextgraph/target/release/contextgraph_data` (only 1.9MB, effectively empty).
- The REAL database lives at `/home/cabdru/contextgraph/contextgraph_data` (3.0GB, contains the calibrated Sonnets constellation).
- Cause: `context-graph-mcp` resolves `contextgraph_data` relative to the CWD at spawn time. The daemon was started with `cwd=/home/cabdru/contextgraph/target/release` instead of `/home/cabdru/contextgraph`.

## Fix
Stop the wrong-path daemon and restart from `/home/cabdru/contextgraph`:

```
kill <pid-on-3100>
cd /home/cabdru/contextgraph
./target/release/context-graph-mcp --daemon-server-only --daemon-port 3100 &
```

Then verify `list_constellations` returns `total > 0` before running the analyzer.

## Why It Matters
The analyzer's entire `style_profile_vector` field (the 13-embedder cosine to the
Sonnets centroid) depends on the daemon being pointed at the correct RocksDB.
Running on the empty DB would have caused EVERY record to emit an analyzer_error
or — worse — produce a nonsense profile vector if the constellation had stale
centroids.

## Related
- ./memory/discoveries/coordinator--constellation-anchors-validated.md (Sonnets UUID)
- Tags: daemon, rocksdb, constellation, pwd
