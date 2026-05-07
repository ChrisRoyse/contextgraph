---
namespace: context
agent_id: agent-00-coordinator
timestamp: 2026-04-14T16:30:00Z
status: active
tags: mission, fsv, training-export, verification
---

# Mission: Full State Verification of Training-Data Export Implementation

## What Was Built Today
Per `tasks/final_fsv_report.md` (the claims under test):
- **Phase 1 + 5**: `TrainingRecord` v2 + `TemporalLabels` + `CF_TRAINING_RECORDS` + 4 MCP tools
- **Phase 2**: `Constellation` compiler + `CF_CONSTELLATIONS` + `CF_CONSTELLATION_BY_SELECTOR` + 5 MCP tools
- **Phase 3**: Contrastive pair miner + `CF_CONTRASTIVE_PAIRS` + `CF_CONTRASTIVE_BY_KIND` + `CF_CONTRASTIVE_BY_ANCHOR` + 4 MCP tools
- **Phase 4**: Tucker-1 core decomposition (CPU) as field on TrainingRecord
- **Phase 6**: `context-graph-cli export training-corpus` with Parquet output
- **6 new CFs**, **13 new MCP tools**, **105 new tests**, claimed **5,473/5,473 passing**

## Verified Numbers (Coordinator)
- Binary rebuilt 2026-04-14 11:10 (574 MB)
- Running MCP PID 26048 started 11:38 (loads the new binary)
- `tools/names.rs` contains 71 `pub const ... &str` declarations
- `TOTAL_COLUMN_FAMILIES = 57` in `storage/column_families.rs:396`
- All 6 new CF constants present in `teleological/column_families.rs`
- Deferred tool list (after search) includes all 9 Phase-2/3 tool names

## Verification Scope (This Pipeline)
1. **Spec grounding** — read all 15 specs + borisflow + SMAP protocol
2. **Source claim verification** — every function/tool claimed in FSV report must exist in code
3. **Live MCP verification** — handshake + tools/list returns 71 distinct names
4. **Physical DB verification** — store→export→read-back cycle with synthetic data, CF-level inspection
5. **Math identity audit** — group_alignments[] math must match spec on synthetic fixtures
6. **Edge case audit** — ≥3 cases with before/after state per new tool category
7. **Code-simplifier review** — second pass on all new modules
8. **Test suite** — `cargo test --workspace` must be fully green, no mocks

## Hard Rules
- NO workarounds or fallbacks. Fail fast with logging.
- NO mock data. Synthetic means known-input/known-output with real code paths.
- NO tests that pass in a broken state. If test expectations are hardcoded numbers, verify the numbers match reality not the other way around.
- Personally verify every subagent claim.
- No backwards compatibility tolerance.

## Source of Truth
RocksDB at `~/.contextgraph/data/` (or `{storage}/` per caller) — specifically the 6 new CFs plus `CF_FINGERPRINTS`, `CF_CONTENT`, `CF_SOURCE_METADATA`.

## Canonical Math Identities to Verify
- `group_alignments[5] == topic_profile[5]` (Implementation = E6 alone)
- `group_alignments[4] == (topic_profile[9] + topic_profile[10]) / 2` (Qualitative = avg(E10,E11))
- `group_alignments[0] == (topic_profile[0] + topic_profile[11] + topic_profile[12]) / 3` (Factual = avg(E1,E12,E13))
- Cross-correlations length = 78 = C(13,2)
- Welford stats converge to naive mean/stddev within 1e-5
- Reservoir sample p50/p95 within ±0.03 on uniform[0,1] stream
- Tucker Frobenius reconstruction error < 0.15 on rank-2 synthetic
