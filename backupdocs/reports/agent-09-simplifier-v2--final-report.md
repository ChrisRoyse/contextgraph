---
namespace: reports
agent_id: agent-09-simplifier-v2
timestamp: 2026-04-14T17:30:00Z
status: complete
tags: smap, final-report, code-simplifier, constellation, fix-pass-review
depends_on:
  - agent-08-code-simplifier--concern-1-constellation-centroids
  - agent-08-code-simplifier--concern-2-rebuild-orphan
---

# Agent-09 Code-Simplifier V2 — SMAP Final Report

## Mandate
Review the three files touched in the coordinator's post-agent-08 fix pass:

1. `crates/context-graph-storage/src/teleological/rocksdb_store/constellation.rs` — atomic orphan-swap in `store_constellation` (lines 58-142).
2. `crates/context-graph-core/src/constellation/types.rs` — `SILENT-ZERO WARNING` docstrings + new `Constellation::has_topic_profile_signal` helper (lines 53-100).
3. `crates/context-graph-storage/tests/constellation_integration.rs` — two new regression tests appended (tests 11 + 12).

Answer three targeted questions and flag findings only (no edits).

## Verdict matrix

| Question | Verdict | Severity |
|----------|---------|----------|
| **Q1.** Is the probe-then-delete-in-same-batch the simplest correct form? | **Yes.** The two guard conditions (`bytes.len() == 16` and `bytes.as_slice() != new_id_bytes`) are **not redundant** — they defend against distinct failure modes (malformed index value vs. degenerate same-UUID self-cancel). One nit flagged about the `len != 16` branch silently self-healing. | nit |
| **Q2.** Is the `has_topic_profile_signal` docstring accurate, and is `cross_correlation_centroid` a more sensitive probe? | **Partial.** The helper picks the correct field — checking `topic_profile_centroid` is strictly equivalent to checking `cross_correlation_centroid` because both Welford accumulators share the same `stored_topic_profile = Some(_)` gate in `ConstellationAccumulator::observe`. The 13D probe is cheaper. But the docstring asserts a narrower cause ("stored_topic_profile was None for every member") than the predicate actually detects (could also be "every member's profile was itself all-zero"). | low |
| **Q3.** Do the two new tests cover the failure modes completely? | **Mostly.** The two branches of the probe (`prior != new` and `prior == new`) are covered. The three-way rebuild chain A→B→C is the obvious missing case — trivially passes today, but a cheap regression shield for the "secondary-index drift" class of bug this fix is meant to prevent. | low |

**No high- or medium-severity findings.** The implementation is correct, the helper is wired right, and the test set is adequate for the invariant under test.

## Findings (memory/discoveries written)

1. **nit** — `memory/discoveries/agent-09-simplifier-v2--atomic-swap-guard-logs-corruption.md`
   The `Ok(_) => None` arm swallows a malformed (non-16-byte) secondary-index value without logging. `find_constellation_by_selector` hard-errors on the same condition. Inconsistent fault-detection surface; defer.

2. **low** — `memory/discoveries/agent-09-simplifier-v2--has-topic-profile-signal-docstring-narrow.md`
   The helper's docstring names only one of two causes for an all-zero centroid. Diff is a 6-line docstring rewrite; also contains the cross-check proving `cross_correlation_centroid` is not a more sensitive probe (both derive from the same gate).

3. **low** — `memory/discoveries/agent-09-simplifier-v2--test-coverage-three-way-rebuild-chain.md`
   Add a `rebuild_chain_a_b_c_leaves_only_c` test — ~15 LOC regression shield against a future refactor that caches or short-circuits the index probe.

## Cross-checks performed
- Inspected `compiler.rs::ConstellationAccumulator::observe` (lines 200-249) to verify all three centroid Welford accumulators share the `if let Some(prof)` gate. Confirmed.
- Inspected `compiler.rs::finalize` (lines 269-333) to verify `topic_centroid` / `group_centroid` / `cc_centroid` are all read from the same Welford family, so zero-count produces all-zero vectors for all three uniformly.
- Inspected the type of `new_id_bytes` (`[u8; 16]` from `*c.id.as_bytes()`) — `bytes.as_slice() != new_id_bytes` is well-formed because `PartialEq<[u8; 16]>` is implemented for `[u8]` via the standard library's array/slice interop.
- Verified `selector_index_key` length (`SELECTOR_INDEX_KEY_LEN = 17`) matches the `bytes.len() == 16` *value* check (keys are 17, values/UUIDs are 16 — not the same invariant, both correctly distinguished).
- Confirmed the coordinator's claim that live test run was 12/12 green by reading the file: tests 1-10 were pre-existing, 11 + 12 are the new additions under review.

## Memory files written
- `/home/cabdru/contextgraph/memory/discoveries/agent-09-simplifier-v2--atomic-swap-guard-logs-corruption.md`
- `/home/cabdru/contextgraph/memory/discoveries/agent-09-simplifier-v2--has-topic-profile-signal-docstring-narrow.md`
- `/home/cabdru/contextgraph/memory/discoveries/agent-09-simplifier-v2--test-coverage-three-way-rebuild-chain.md`
- `/home/cabdru/contextgraph/memory/reports/agent-09-simplifier-v2--final-report.md` (this file)

## One-sentence verdict
Both fixes are **correct and minimal**; no high/medium issues; three low-severity polish items flagged (one docstring tightening, one observability nit, one regression test) — all optional and none blocking.
