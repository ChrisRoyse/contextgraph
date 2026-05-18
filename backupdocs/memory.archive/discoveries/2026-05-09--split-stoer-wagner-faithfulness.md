---
namespace: discoveries
agent_id: agent-code-simplifier-split
timestamp: 2026-05-09T00:00:00Z
status: active
tags: split,stoer-wagner,mincut,doc09
---

# Stoer-Wagner connection: faithful in spirit; literal call is degenerate; preamble references a non-existent function

Doc 09 §3.4 (line 258) prescribes "Stoer-Wagner mincut on the (task → repo) bipartite graph". Two issues:

1. **Bipartite-graph claim is degenerate as the author states.** Each SWE-bench Lite task → exactly one repo. The (task, repo) bipartite graph is therefore a forest of stars, one per repo. Stoer-Wagner on this disconnected graph returns `MincutError::GraphDisconnected` (mincut crate lib.rs:58-59). The atomic per-repo bucket assignment in split.rs IS the closed-form mincut answer (cross-bucket edge weight in the task-similarity graph, where weight=1 iff tasks share a repo, is identically zero). `cross_cut_weights` (split.rs:345-371) directly computes this cross-bucket edge sum. Faithful to the doc's INTENT.

2. **Comment on split.rs:26 lies.** Preamble says "callers that want 'an actual mincut ran on this corpus' can use `verify_split_via_mincut`." That function does not exist. Either:
   - Delete the sentence (recommended, cheapest), OR
   - Implement a thin adapter that, for each ordered pair of buckets, builds a tiny n×n matrix where w[i,j]=1 iff bucket-i task-i and bucket-j task-j share a repo (else 0), then calls `context_graph_mincut::stoer_wagner` and asserts the cut weight ≤ 0 (which it always will be for a leakage-free split, since the graph between buckets has no edges).

Recommendation: delete line 24-26 in the preamble, OR ship the adapter. Code currently makes a promise it doesn't keep.

Verdict: REFINEMENT-RECOMMENDED (doc lie).
