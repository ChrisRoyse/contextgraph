---
namespace: discoveries
agent_id: agent-209-task-updates-phase6-hygiene
timestamp: 2026-05-10T00:00:00Z
status: active
tags: tasks, hygiene, phase6, audit
---

# Phase 6 hygiene TASK file updates — 2026-05-10

## What

Prepended the verbatim AI-Agent Implementation Mandate block (from `memory/handoff/agent-208-coordinator--ai-agent-mandate-block-template.md`) and a `## 📋 2026-05-10 STATE AUDIT` H2 section to all 20 `TASK-HYG-NNN.md` files in `docs/ruvectorfindings/specs/tasks/phase6-hygiene/`. The mandate block was inserted between the H1 title and the `<task_spec>` XML block. The audit section follows the mandate and reports either `clean` or `flagged` with specific drift items.

## Pre-work audit findings (verified 2026-05-10)

1. **Crate `crates/context-graph-mejepa-hygiene/` does NOT exist** — every task's "create new crate / new module under it" instruction remains accurate.
2. **Foundation crates already shipped:**
   - `crates/context-graph-tier-compression/src/lib.rs` (shipped 2026-05-09, CGTC 8/7/5/3-bit)
   - `crates/context-graph-mincut/src/lib.rs` (shipped 2026-05-09, Stoer-Wagner)
   - `crates/context-graph-witness/src/lib.rs` (shipped 2026-05-08, cert + Merkle)
3. **Already-declared `CF_MEJEPA_*` consts in `crates/context-graph-mejepa/src/heal/cf.rs`:**
   - `CF_MEJEPA_FISHER_SNAPSHOTS`, `CF_MEJEPA_DRIFT_WINDOW`, `CF_MEJEPA_DRIFT_HISTORY`, `CF_MEJEPA_HEAL_REPORTS`, `CF_MEJEPA_DISTILL_STEPS`, `CF_MEJEPA_ACTIVE_POINTERS`, `CF_MEJEPA_CALIBRATION_HISTORY`, `CF_MEJEPA_WEIGHT_BLOBS`, `CF_MEJEPA_SHIFT_WATERMARK`, `CF_MEJEPA_TRAIN_CERTS`.
4. **CFs newly introduced by phase6-hygiene tasks (not yet in tree):**
   - `CF_MEJEPA_PANEL_META` (HYG-007), `CF_MEJEPA_QUOTA_STATE` (HYG-008), `CF_MEJEPA_WEIGHT_BLOBS_META` (HYG-009/015), `CF_MEJEPA_WITNESS_SEGMENT_META` (HYG-010), `CF_MEJEPA_GC_HISTORY` (HYG-019).

## Per-file SHA + audit table

| File | Pre-SHA | Post-SHA | Audit finding |
|------|---------|----------|---------------|
| TASK-HYG-001.md | d0f65cd3 | 0af4cf3b | clean |
| TASK-HYG-002.md | db19cd83 | 96895060 | clean |
| TASK-HYG-003.md | 4080a1bc | 043c4a1e | clean |
| TASK-HYG-004.md | a8c22496 | eb82af9a | clean |
| TASK-HYG-005.md | 6a4731b3 | 7d0e7f5a | clean (depends on shipped tier-compression crate) |
| TASK-HYG-006.md | 45bd398e | 5750cb0e | clean |
| TASK-HYG-007.md | e8dd5473 | a7f35127 | clean (first declarer of CF_MEJEPA_PANEL_META) |
| TASK-HYG-008.md | b1c481fb | 7c7b9dc6 | clean (first declarer of CF_MEJEPA_QUOTA_STATE) |
| TASK-HYG-009.md | 2dd80c6f | f7d5707a | flagged (CF_MEJEPA_WEIGHT_BLOBS_META is new; sidecar of already-shipped CF_MEJEPA_WEIGHT_BLOBS — must import existing const, not redeclare) |
| TASK-HYG-010.md | 7be3baca | 9bfe1d88 | clean (depends on shipped witness crate; declares new CF_MEJEPA_WITNESS_SEGMENT_META) |
| TASK-HYG-011.md | 16e63575 | cf6e7d79 | clean |
| TASK-HYG-012.md | e09be430 | f90f2cc0 | clean |
| TASK-HYG-013.md | 4aadb539 | bd655c2f | flagged (CF list mixes already-declared CFs CF_MEJEPA_WEIGHT_BLOBS, CF_MEJEPA_TRAIN_CERTS, CF_MEJEPA_SHIFT_WATERMARK with new ones — task must import existing consts; suggest extending compact set to also cover the 7 heal CFs) |
| TASK-HYG-014.md | f222eecc | bdefb8e8 | clean (depends on shipped mincut crate) |
| TASK-HYG-015.md | 622dc5bb | 760761d3 | flagged (redeclares `pub const CF_MEJEPA_WEIGHT_BLOBS` already present in heal/cf.rs:17 — must import, not redeclare; CF_MEJEPA_WEIGHT_BLOBS_META is new and OK) |
| TASK-HYG-016.md | 76678e59 | 79c410a1 | flagged (redeclares `pub const CF_MEJEPA_CALIBRATION_HISTORY` already present in heal/cf.rs:16 — must import, not redeclare; reconcile with existing RetentionPolicy at heal/cf.rs:206-212) |
| TASK-HYG-017.md | 60e092e9 | cee5d7c9 | clean |
| TASK-HYG-018.md | 18fa1a7e | ff0735c3 | clean |
| TASK-HYG-019.md | 859e96b2 | c7a02585 | clean (first declarer of CF_MEJEPA_GC_HISTORY) |
| TASK-HYG-020.md | 47583d7f | 7346427f | clean (Phase 5 self-healing shipped at 679b8efc; SelfHealingHandle trait in HYG-001 keeps dep one-way) |

## Summary

- 20/20 files updated.
- 4 files flagged for drift: TASK-HYG-009, TASK-HYG-013, TASK-HYG-015, TASK-HYG-016 — all stem from the same root cause: the existing `crates/context-graph-mejepa/src/heal/cf.rs` already exports 10 `CF_MEJEPA_*` consts (`CF_MEJEPA_WEIGHT_BLOBS`, `CF_MEJEPA_TRAIN_CERTS`, `CF_MEJEPA_SHIFT_WATERMARK`, `CF_MEJEPA_CALIBRATION_HISTORY`, plus 6 healing-specific), and the hygiene tasks must import these rather than re-`pub const` them.
- 16/20 files audited clean.
- No `<task_spec>` XML bodies were modified; only the title-to-XML boundary received the mandate + audit blocks.
- Foundation crates (tier-compression, mincut, witness) confirmed shipped; tasks referencing them are accurate.

## Recommendation for implementers

Before implementing TASK-HYG-001, decide on the canonical CF-const location. Options:
- (a) Move existing `CF_MEJEPA_*` consts from `crates/context-graph-mejepa/src/heal/cf.rs` into a new `crates/context-graph-mejepa-cf` crate that both `heal` and `hygiene` depend on. Cleanest.
- (b) Make `context-graph-mejepa-hygiene` depend on `context-graph-mejepa` and re-export the heal consts. Simpler but couples hygiene to heal.
- Either way, ZERO `pub const` duplication of names already in heal/cf.rs in the new hygiene crate.
