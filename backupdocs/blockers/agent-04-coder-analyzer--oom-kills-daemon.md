---
namespace: blockers
agent_id: agent-04-coder-analyzer
timestamp: 2026-04-17T16:55:00Z
status: blocked
severity: critical
tags: oom, daemon, phase-b, voiceagent, ram, wsl2
---

# Phase B blocked: daemon cannot stay alive due to host RAM exhaustion

## What I Tried
1. Verified the analyzer script is correct, deps installed (`pronouncing`, `spacy`, `en_core_web_sm`), syntax OK.
2. Discovered the daemon claimed to be on port 3100 (PID 2346870) was pointing at the WRONG RocksDB dir (`target/release/contextgraph_data`, 1.9MB, empty). Real DB is at `/home/cabdru/contextgraph/contextgraph_data` (3.0GB, has the Sonnets constellation). Killed that daemon. See `./memory/discoveries/agent-04-coder-analyzer--daemon-wrong-db-path.md`.
3. Started the canonical daemon via `scripts/ensure_daemon.sh`, which exports `CONTEXT_GRAPH_STORAGE_PATH=/home/cabdru/contextgraph/contextgraph_data` and launches `context-graph-mcp --daemon-server-only --daemon-port 3100`. The script reported the daemon "ready on port 3100 (took 45s)".
4. Within seconds of becoming ready, the daemon dies. `ss -tlnp | grep 3100` returns empty. The listener process (PID 2375580) is gone.

## Root Cause
Kernel page allocation failure under severe RAM pressure. From `/var/log/kern.log`:
```
2026-04-17T11:50:10 kernel: context-graph-m: page allocation failure: order:4,
  mode:0x40c40(GFP_NOFS|__GFP_COMP), nodemask=(null), cpuset=init.scope,
  mems_allowed=0
CPU: 1 PID: 2578693 Comm: context-graph-m Not tainted
  6.6.87.2-microsoft-standard-WSL2
```

Host RAM state at 11:55 UTC-ish:
```
Mem:  120GB total  /  120GB used  /  0 free
Swap:  32GB total  /   32GB used  /  0 free
```

Top consumers (100% not mine to kill):
- 4x `/home/cabdru/miniconda3/bin/python -m voiceagent listen --voice boris`
  using 31GB + 31GB + 30GB + 15GB RSS = ~107GB out of 120GB.

The Context Graph daemon needs ~20GB host RAM (embedder models mmap'd via
llama.cpp into GPU + host tensors + RocksDB block cache + usearch HNSW indexes).
There is no room for it. WSL2 kernel responds with page-allocation failures
which kill the daemon immediately after it finishes initial model load.

This is NOT a bug in the daemon — the supervisor would rebind and keep it
alive if memory were available.

## Impact on Phase B
The mission brief's non-negotiable rule: *"If the daemon becomes unreachable
mid-run, exit with clear message — do NOT swap to a mock scorer."*

`style_profile_vector` (the 13-float per-embedder cosine to the Sonnets
centroid) is the whole point of Phase B's daemon round-trip — it's what tells
Phase C's DPO pair builder which variants are stylistically close to real
Shakespeare. Without it, Phase C's scoring is content-only again, and the
whole resurrection collapses back into the same failure that motivated the
rebuild.

I can compute all pure-CPU fields (meter, devices, register, vocabulary,
sentence_shape, semantic_topic) without the daemon. But I CANNOT emit valid
records that satisfy the locked schema. Emitting `style_profile_vector: null`
for 7,925 records would silently disable Phase C's register-aware scoring —
exactly the kind of "mock fallback" the mission forbids.

## What the Operator Needs To Do
Free host RAM before Phase B can proceed. Options:
1. Stop the `voiceagent listen` processes (PIDs 1186576, 1212064, 8419,
   1609164). They are Python ML processes, not my work to kill.
2. Or run Phase B on a different host with ≥40GB free RAM.
3. Or (least preferred) rebuild the daemon with smaller embedder footprint
   (drop E6/E13 SPLADE sparse indexes — they use several GB each).

Once RAM is free, restart the daemon via
`bash /home/cabdru/contextgraph/scripts/ensure_daemon.sh`, confirm with
`ss -tlnp | grep 3100`, then rerun:
```
python3 scripts/shakespeare_stylistic_analyzer.py --limit 10 --output /tmp/tags_smoke.jsonl
python3 scripts/shakespeare_stylistic_analyzer.py --output ~/.contextgraph/training/shakespeare_stylistic_tags.jsonl
```

The analyzer is idempotent and resumable — if it dies partway through, just
rerun the same command.

## Related
- `./memory/discoveries/agent-04-coder-analyzer--daemon-wrong-db-path.md`
- `./scripts/ensure_daemon.sh`
- `./crates/context-graph-mcp/src/main.rs` line 1792+ (headless daemon mode)
- Tags: oom, daemon, voiceagent, ram, wsl2, blocker
