---
namespace: discoveries
agent_id: agent-s2-sherlock-vram
timestamp: 2026-04-17T12:18:00Z
status: active
severity: medium
tags: daemon, cli, flags, no-warm, vram
---

# Daemon `--no-warm` defers loading but does NOT selectively disable embedders

## What Happened

Inspection of `crates/context-graph-mcp/src/main.rs` + `crates/context-graph-mcp/src/server/mod.rs`:

Daemon CLI flags:
- `--warm-first` (default): block startup until ALL 13 embedders load into VRAM
- `--no-warm`: don't block, load ALL 13 in background — embeddings fail until ready
- `CONTEXT_GRAPH_WARM_FIRST=0` env var: same effect as --no-warm

**There is NO per-embedder disable flag.** All 13 embedders are loaded regardless.

With `--no-warm`:
- Startup is faster (seconds, not ~20-30 s)
- Embedder calls fail with "not ready" error until background load completes
- Final steady-state VRAM is IDENTICAL (still 17.34 GB)
- **This flag does NOT save VRAM** — it only moves when the load happens

## Why It Matters

Phase G has a 0.4 GB shortfall (20.7 + 11.5 = 32.2 vs 31.8 GB budget). `--no-warm` DOES NOT solve it — we would still eventually load all 13 embedders.

## Actual levers available
1. Quantize Gemma to 4-bit (Phase G): 11.5 → 4.2 GB → total 24.9 GB (6.9 GB headroom) ✓
2. Stagger services (Phase D): daemon DOWN during Gemma load → 0 + 11.5 = 11.5 GB, then daemon UP for scoring → 17.34 GB (14.5 GB headroom) ✓
3. Kill daemon entirely for training phases E/F (already planned) ✓

## Related
- Daemon main.rs flag parsing: lines 157-473
- Server warm initialization: `crates/context-graph-mcp/src/server/mod.rs` lines 300-460
