---
namespace: reports
agent_id: agent-code-simplifier-split
timestamp: 2026-05-09T00:00:00Z
status: active
tags: review,split,mejepa-corpus,code-simplifier
---

# Code-simplifier review: mejepa-corpus split module

## Scope
- `crates/context-graph-mejepa-corpus/src/split.rs` (~380 lines)
- `crates/context-graph-mejepa-corpus/src/split_tests.rs` (4 tests)
- `crates/context-graph-mejepa-corpus/examples/split_fsv.rs` (~270 lines)

## Verdict
**PASS with 4 REFINEMENTS recommended; 0 BLOCKERS.**

The bin-pack fix (lowest current/target ratio) is correct and resolves the original starvation bug at root cause. The leakage invariant is enforced both constructively (atomic per-repo assignment) and verifiably (`cross_cut_weights`). Determinism is correct (BTreeMap iteration + SplitMix64 Fisher-Yates). Fail-closed surface for empty corpus, NaN, bad-sum fractions, zero-target buckets, and degenerate splits is solid.

## Counts
- PASS: 1 (overall correctness)
- REFINEMENT-RECOMMENDED: 4
- BLOCKER: 0

## Top-3 actionable refinements

### 1. Delete or implement the `verify_split_via_mincut` reference
**File:** `split.rs:24-26`
**Issue:** Preamble promises a function that does not exist.
**Fix (cheapest):** delete the three-line sentence "Optionally re-run Stoer-Wagner ... `verify_split_via_mincut`."
**Fix (faithful):** add a thin adapter that builds a tiny matrix per bucket pair and calls `context_graph_mincut::stoer_wagner` to assert cut-weight = 0. Doc 09's mincut prescription is degenerate on the literal bipartite (one star per repo → disconnected); the adapter would run mincut on the *task-similarity* graph (edge weight 1 iff tasks share a repo) and confirm the cut weight is 0 by construction.
**Rationale:** code-vs-comment drift erodes trust; a doc lie is a future bug source.

### 2. Reject duplicate `task_id` across different repos
**File:** `split.rs:131-148` (the grouping loop)
**Issue:** `("foo","repo_a")` + `("foo","repo_b")` silently emits one task_id under two buckets, breaking the "each task once" invariant. FSV's HashSet coalesces them and passes.
**Fix:** add a `BTreeSet<&str>` of seen task_ids; on collision return `MutationError::invalid("entries[i].task_id", "duplicate task_id `{}`", "task_ids must be globally unique")`.
**Rationale:** cheap, closes a real silent-failure path, takes ~6 lines.

### 3. LPT pre-sort before lowest-ratio bin-pack
**File:** `split.rs:154-209`
**Issue:** Pure proportional-fill without size-ordering can let a late-arriving large repo (e.g. sympy's 75 tasks) drift fraction adherence. Empirical FSV: 78.1/10.3/11.6 vs target 80/10/10 (1.9pp train drift).
**Fix:** after the SplitMix64 shuffle, do `repos.sort_by(|a,b| repo_tasks[b].len().cmp(&repo_tasks[a].len()))`. The shuffle stays as the deterministic tie-breaker between equal-sized repos.
**Rationale:** classical 4/3-OPT bound for makespan; tightens drift to <1pp; does not affect leakage correctness.

## Bonus refinement (4th)
**File:** `examples/split_fsv.rs:83-94`
Add `assert!(!a.task_id.contains(',') && !a.repo.contains(','))` at CSV write time. Free. Makes the "no commas in SWE-bench identifiers" assumption explicit and fails fast if a future synthetic mutation suffix violates it.

## SMAP discoveries written
- `memory/discoveries/2026-05-09--split-binpack-lpt-ordering.md`
- `memory/discoveries/2026-05-09--split-stoer-wagner-faithfulness.md`
- `memory/discoveries/2026-05-09--split-edge-cases-and-csv-hardening.md`

## Build/test status
Not re-verified (READ-ONLY review). User reports 45 tests green and FSV at `/tmp/contextgraph-mejepa-corpus-fsv/` shows leakage-free split with cross_cut_weights = [0,0,0]. Consistent with code reading.
