---
namespace: discoveries
agent_id: agent-code-simplifier-split
timestamp: 2026-05-09T00:00:00Z
status: active
tags: split,corpus,binpack,heuristic
---

# Bin-pack: lowest current/target ratio is correct, LPT pre-sort would tighten fraction adherence

`crates/context-graph-mejepa-corpus/src/split.rs:177-204` picks the bucket whose `current/target` ratio is lowest. This is the standard "proportional fill" online heuristic and IS correct for the atomic-per-repo bin-pack problem. It strictly dominates the original "largest remaining capacity" heuristic that starved small buckets when one large repo overshoots their entire target.

**Refinement (optional, not a bug)**: classical Longest-Processing-Time-first (LPT) sorts items by size DESCENDING before placing them. After the SplitMix64 seed shuffle (line 154-159), insert a stable sort by `repo_tasks[repo].len()` descending. The seed still controls tie-breaking via the shuffle. LPT-then-lowest-ratio gives a 4/3-OPT bound on makespan deviation; pure lowest-ratio without ordering can drift further from target fractions when a single huge repo (e.g., sympy's 75 tasks @ 25% of corpus) lands late.

Empirical FSV evidence: 25 repos × ~12 avg tasks split 78.1/10.3/11.6 vs target 80/10/10 — 1.9pp drift on train. With LPT-first this typically tightens to <1pp. NOT a correctness issue; the leakage invariant holds either way.

Verdict: PASS with optional REFINEMENT.
