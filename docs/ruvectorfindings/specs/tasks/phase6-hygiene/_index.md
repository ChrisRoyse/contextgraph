# Phase 6 — HYGIENE Task Index

> **📊 2026-05-10 STATUS — partial.** `context-graph-tier-compression` (8/7/5/3-bit on-disk format) and `context-graph-mincut` (Stoer-Wagner) crates shipped; CF integration into ME-JEPA panel storage, witness-chain Merkle compression of >90d segments, nightly GC, 115 GB hard quota with emergency eviction, and `me_jepa::ops::*` MCP wrappers (REQ-HYG-03..07) still pending. Authoritative status: ../../implementation-plan.md Phase 6.

> **Level 4.** Atomic task specifications for the **HYGIENE domain** (Phase 6: tier compression integration, Stoer-Wagner mincut leakage scoring, witness-chain Merkle compression, nightly 8-step GC pass, hard quota enforcement, `me_jepa::ops::*` MCP-callable interface). Authority order: `../../constitution.md` (Level 1) > `../../functional/SPEC-HYGIENE.md` (Level 2) > `../../technical/TECH-HYGIENE.md` (Level 3) > this file (Level 4). Each task is one conceptual change shippable end-to-end with FSV evidence per `docs2/prdtospec.md` §4.

## Phase scope

Implements all 10 functional REQ-HYG-NN requirements (cross-linked in `../_traceability.md` under `### HYGIENE`) and all 12 component contracts in `../../technical/TECH-HYGIENE.md`. Foundation → Logic → Surface ordering is enforced; no task references files created by a later task.

**Reuse callout** (per TECH-HYGIENE `<implementation_notes category="reuse">`):
- Tier compression encoder/decoder reuses SHIPPED `crates/context-graph-tier-compression/` (✅; 21 tests; FSV at `/tmp/contextgraph-tier-compression-fsv/{blob_8,blob_7,blob_5,blob_3}.bin`; cosines 0.99999/0.99997/0.99960/0.99192). No new 8/7/5/3-bit quantizer is written. Frozen 1-bit tier is a NEW separate `CGTF` magic in `tier_frozen.rs` to keep the shipped crate's contract unchanged.
- Stoer-Wagner mincut for repo-atomic leakage scoring in step 3 evict reuses SHIPPED `crates/context-graph-mincut/` (✅; dense O(V³) variant; fail-closed on negative weights / NaN / disconnected graphs). Acceptable simplification for v1.0: mincut leakage penalty may be skipped on the first impl cut and added after FSV stress run is green.
- Merkle compression reuses SHIPPED `crates/context-graph-witness/src/cert.rs` (✅; `MerkleTree::from_leaves`, `MERKLE_LEAF_TAG`, `MERKLE_NODE_TAG`). No new Merkle implementation.

**Cross-cutting deferral callout**:
- `heal_detect_drift` and `heal_retrain_if_needed` ship as fail-closed `OpsError::CrossCuttingDeferred { spec: "SPEC-SELF-HEALING" }` shims; bodies land in Phase 5. The MCP shim contract is permanent; the body changes when Phase 5 ships.

## Crate layout

New crate `crates/context-graph-mejepa-hygiene/` (per TECH-HYGIENE `<crate_layout>`). Files under 500 lines. Source files carry the constitution provenance comment.

## Dependency Graph

```mermaid
graph TD
    subgraph Foundation
        T001[TASK-HYG-001<br/>Crate scaffold + HygieneEnv + categories::BUDGETS]
        T002[TASK-HYG-002<br/>HygieneError taxonomy + .code() arms]
        T003[TASK-HYG-003<br/>Reports + status data models]
        T004[TASK-HYG-004<br/>Tier enum + frequency math + EntryAccessFrequency]
    end
    subgraph Logic
        T005[TASK-HYG-005<br/>tier::encode_for_tier + decode_for_tier]
        T006[TASK-HYG-006<br/>tier_frozen 1-bit CGTF encoder/decoder]
        T007[TASK-HYG-007<br/>tier_demote + tier_promote + per-entry locks]
        T008[TASK-HYG-008<br/>quota::snapshot + degradation_for + score_for_eviction]
        T009[TASK-HYG-009<br/>quota_check_and_evict + Unrecoverable transition]
        T010[TASK-HYG-010<br/>witness_compress::compress_segments + atomic archive]
        T011[TASK-HYG-011<br/>witness_verify_integrity + audit_verify_old_entry]
        T012[TASK-HYG-012<br/>gc step 1 demote]
        T013[TASK-HYG-013<br/>gc step 2 compact_range_cf parallel]
        T014[TASK-HYG-014<br/>gc step 3 evict bottom-1pct + mincut leakage]
        T015[TASK-HYG-015<br/>gc step 4 prune checkpoints + tagged-release shield]
        T016[TASK-HYG-016<br/>gc step 5 prune calibration]
        T017[TASK-HYG-017<br/>gc step 6 compress witness wrapper]
        T018[TASK-HYG-018<br/>gc step 7 verify_integrity + atomic batch commit]
        T019[TASK-HYG-019<br/>gc step 8 report writer 0o600]
        T020[TASK-HYG-020<br/>gc::gc_run_nightly orchestrator + flock + Phase 5 guard]
        T021[TASK-HYG-021<br/>ops module + heal_* CrossCuttingDeferred shims]
    end
    subgraph Surface
        T022[TASK-HYG-022<br/>MCP tool mejepa_gc_run + JSON schema]
        T023[TASK-HYG-023<br/>MCP tool mejepa_quota_status + JSON schema]
        T024[TASK-HYG-024<br/>MCP tool mejepa_witness_compress + JSON schema]
        T025[TASK-HYG-025<br/>CLI stress subcommand on context-graph-mejepa]
        T026[TASK-HYG-026<br/>FSV harness gc_stress + merkle_compression + quota_overflow]
    end

    T001 --> T002
    T001 --> T003
    T001 --> T004
    T002 --> T005
    T003 --> T008
    T003 --> T010
    T003 --> T011
    T003 --> T012
    T003 --> T015
    T003 --> T019
    T004 --> T005
    T004 --> T007
    T005 --> T006
    T005 --> T007
    T006 --> T007
    T007 --> T012
    T008 --> T009
    T008 --> T014
    T008 --> T023
    T009 --> T021
    T010 --> T011
    T010 --> T017
    T011 --> T018
    T011 --> T024
    T012 --> T020
    T013 --> T020
    T014 --> T020
    T015 --> T020
    T016 --> T020
    T017 --> T020
    T018 --> T020
    T019 --> T020
    T020 --> T021
    T021 --> T022
    T021 --> T023
    T021 --> T024
    T022 --> T025
    T023 --> T025
    T024 --> T025
    T020 --> T026
    T010 --> T026
    T009 --> T026
    T025 --> T026
```

## Execution order

| # | Task ID | Title | Layer | Depends On | Est complexity |
|---|---|---|---|---|---|
| 1 | TASK-HYG-001 | Crate scaffold + HygieneEnv + categories::BUDGETS table | foundation | — | low |
| 2 | TASK-HYG-002 | HygieneError + OpsErrorKind taxonomy + .code() arms | foundation | 001 | low |
| 3 | TASK-HYG-003 | GcReport + EvictReport + CompressReport + ChainIntegrityReport + QuotaStatus + CategoryStatus + TierTransition + EntryAccessFrequency | foundation | 001 | medium |
| 4 | TASK-HYG-004 | Tier enum + thresholds + decay_factor + tier_for_frequency | foundation | 001 | low |
| 5 | TASK-HYG-005 | tier::encode_for_tier + decode_for_tier (CGTC dispatch via tier-compression crate) | logic | 002, 004 | medium |
| 6 | TASK-HYG-006 | tier_frozen 1-bit CGTF magic encoder + decoder + half-bin invariant | logic | 005 | medium |
| 7 | TASK-HYG-007 | tier_demote + tier_promote + per-entry sharded mutex map | logic | 004, 005, 006 | medium |
| 8 | TASK-HYG-008 | quota::snapshot + degradation_for + score_for_eviction (recency exp decay) | logic | 003 | medium |
| 9 | TASK-HYG-009 | quota_check_and_evict + Unrecoverable transition + CF_MEJEPA_QUOTA_STATE write | logic | 008 | medium |
| 10 | TASK-HYG-010 | witness_compress::compress_segments + archive_segment_atomic + Merkle root build | logic | 003 | high |
| 11 | TASK-HYG-011 | witness_verify_integrity + audit_verify_old_entry (Merkle re-verify) | logic | 010 | medium |
| 12 | TASK-HYG-012 | gc_steps::step1_demote::run | logic | 003, 007 | low |
| 13 | TASK-HYG-013 | gc_steps::step2_compact::run (parallel compact_range_cf) | logic | 003 | low |
| 14 | TASK-HYG-014 | gc_steps::step3_evict::run (bottom-1pct + optional mincut leakage) | logic | 008 | medium |
| 15 | TASK-HYG-015 | gc_steps::step4_prune_checkpoints::run + tagged-release shield | logic | 003 | low |
| 16 | TASK-HYG-016 | gc_steps::step5_prune_calibration::run | logic | 003 | low |
| 17 | TASK-HYG-017 | gc_steps::step6_compress_witness::run wrapper | logic | 010 | low |
| 18 | TASK-HYG-018 | gc_steps::step7_verify_integrity::run + atomic batch commit | logic | 011 | medium |
| 19 | TASK-HYG-019 | gc_steps::step8_report::run + 0o600 writer | logic | 003 | low |
| 20 | TASK-HYG-020 | gc::gc_run_nightly orchestrator + flock + Phase 5 mid-promotion guard | logic | 012-019 | medium |
| 21 | TASK-HYG-021 | ops module re-export of 8 primitives + heal_* CrossCuttingDeferred shim | logic | 009, 020 | low |
| 22 | TASK-HYG-022 | MCP tool mejepa_gc_run + dispatch_gc_run + schema (additionalProperties:false; confirm_run required) | surface | 021 | medium |
| 23 | TASK-HYG-023 | MCP tool mejepa_quota_status + dispatch_quota_status + schema | surface | 021 | low |
| 24 | TASK-HYG-024 | MCP tool mejepa_witness_compress + dispatch + threshold_days bounds | surface | 021 | low |
| 25 | TASK-HYG-025 | CLI `stress --entries N --duration D` on context-graph-mejepa bin | surface | 022, 023, 024 | medium |
| 26 | TASK-HYG-026 | FSV harnesses (gc_stress_fsv + merkle_compression_fsv + quota_overflow_fsv) at /tmp/contextgraph-mejepa-hygiene-fsv/ | surface | 020, 010, 009, 025 | medium |

## Status legend

- ready — all dependencies satisfied; can be picked up
- blocked — at least one dependency not yet shipped
- in_progress — actively being worked
- complete — landed + FSV evidence + traceability ticked

All tasks above start in **ready** for the foundation layer; logic and surface tasks are **blocked** until their predecessors land.

## Critical path

```
TASK-HYG-001 → 003 → 004 → 005 → 006 → 007 → 012 → 020 → 021 → 022 → 025 → 026
```

12-step longest chain; parallel batches reduce wall-clock.

## Parallel opportunities

- **Batch 1** (foundation, fully parallel): TASK-HYG-001 first; then 002, 003, 004 in parallel (each only depends on 001).
- **Batch 2** (logic A, after foundation): TASK-HYG-005 → 006 → 007 sequential (tier stack); TASK-HYG-008 (quota math) parallel; TASK-HYG-010 → 011 (witness) parallel to tier stack.
- **Batch 3** (gc steps): TASK-HYG-012, 013, 014, 015, 016, 017, 018, 019 mostly parallel (each depends on its own substrate); 014 needs 008; 017 needs 010; 018 needs 011.
- **Batch 4** (orchestrator + ops): TASK-HYG-020 + 021 sequential.
- **Batch 5** (surface MCP): TASK-HYG-022, 023, 024 fully parallel.
- **Batch 6** (CLI + FSV): TASK-HYG-025 → 026 sequential.

## Phase 6 DoD verification

Final verification command (from TECH-HYGIENE `<verification_protocol>`):

```
cargo run --release -p context-graph-mejepa -- stress --entries 10000 --duration 7d
```

Expected:
- `total_disk_used_gb` ≤ 110
- `gc_passes_completed` == 7
- `witness_chain_compressed: yes`
- `quota_violations` == 0

FSV evidence (mode 0o600 per SEC-MEJEPA-07):
- `/tmp/contextgraph-mejepa-hygiene-fsv/gc-stress-evidence.json`
- `/tmp/contextgraph-mejepa-hygiene-fsv/merkle-compression-evidence.json`
- `/tmp/contextgraph-mejepa-hygiene-fsv/quota-overflow-evidence.json`
