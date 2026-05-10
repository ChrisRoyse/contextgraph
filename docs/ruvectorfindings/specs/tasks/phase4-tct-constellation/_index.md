# Phase 4b — TCT-CONSTELLATION Task Index

> **📊 2026-05-10 STATUS — SHIPPED AT PANEL LEVEL.** Full `TctConstellation` + builder + `Gτ` guard + `Verdict::GuardRejected` + Bayesian shrinkage + freshness check + per-embedder threshold calibrator in `crates/context-graph-mejepa-tct/`. MCP tool `mcp__cgreality__mejepa_constellation_inspect` live. `tct_fsv` example passed with RocksDB readback. Per-chunk-type centroids and chunk-level Gτ are pending integration with the now-shipped `context_graph_core::memory::ast` chunk IDs. Authoritative status: ../../implementation-plan.md Phase 4b.

> **Level 4.** Atomic task specifications for the **TCT-CONSTELLATION domain** (Phase 4b cross-cutting: frozen multi-modal centroid set CP, Bayesian shrinkage, threshold calibration, deterministic Gτ guard, L_constellation_centroid hinge primitive, MCP inspector). Authority order: `../../constitution.md` (Level 1) > `../../functional/SPEC-TCT.md` (Level 2) > `../../technical/TECH-TCT.md` (Level 3) > this file (Level 4). Each task is one conceptual change shippable end-to-end with FSV evidence per `docs2/prdtospec.md` §4.

## Phase scope

Implements all 15 functional REQ-TCT-NN requirements (cross-linked in `../_traceability.md` under `### TCT`) and all 7 component contracts (`TctConstellation`, `ConstellationBuilder`, `ThresholdCalibrator`, `GtauChecker`, `ConstellationHingeOp`, `ConstellationStore`, `ViolationRateAggregator`) in `../../technical/TECH-TCT.md`. Foundation -> Logic -> Surface ordering enforced; Iron Rule applies (no task references files created by a later task).

**Cross-cutting integration:** TCT is consumed by Phase 4a (INFERENCE) at Gate 7 via `Verdict::ConstellationGuardRejected`, by Phase 3 (TRAINING) via `L_constellation_centroid`, by SPEC-UTML via `constellation_violation_rate` (ΔΞ predictor-side defense diagnostic), and by Phase 5 (SELF-HEALING) via the rolling-window refit trigger.

**Dependency:** Phase 0 corpus (≥ 2,000 labeled mutation entries) + ≥ 18 of 21 embedders + ≥ 12 of 15 panel slots. Until those are in tree, only the type definitions + Gτ test logic + ConstellationHingeOp can be unit-tested with synthetic panels; full end-to-end builder + threshold calibration require the corpus.

---

## Overview

- **Total tasks:** 12
- **Foundation layer:** 2 tasks (TASK-TCT-001..002)
- **Logic layer:** 8 tasks (TASK-TCT-003..010)
- **Surface layer:** 2 tasks (TASK-TCT-011..012)
- **Estimated walltime:** ~5 days end-to-end (per master plan Phase 4b budget; ~3 days panel-level + ~2 days chunk-level)
- **Crate created:** `crates/context-graph-mejepa-tct/` (NEW)

---

## Dependency DAG

```mermaid
graph TD
    subgraph Foundation [Foundation Layer]
        T001[TASK-TCT-001<br/>Public data models<br/>TctConstellation, CorpusProvenance,<br/>MutationCategory, OracleOutcome,<br/>Language, EntityType, EmbedderId,<br/>ChunkId, GtauViolation, GtauOutput]
        T002[TASK-TCT-002<br/>Internal data models<br/>ShrinkageOrigin, ChunkGtauOutput,<br/>ConstellationHingeOutput,<br/>VerdictGuardRejected, PredictorOutput,<br/>RollingWindow, TctError]
    end

    subgraph Logic [Logic Layer]
        T003[TASK-TCT-003<br/>ConstellationBuilder<br/>per-category, per-language,<br/>outcome centroid computation]
        T004[TASK-TCT-004<br/>Per-chunk-type centroids<br/>+ Bayesian shrinkage on N&lt;50]
        T005[TASK-TCT-005<br/>ThresholdCalibrator<br/>per-embedder + per-chunk-type τ_m<br/>FPR ≤ 5%]
        T006[TASK-TCT-006<br/>GtauChecker<br/>O(N · d_max) deterministic test]
        T007[TASK-TCT-007<br/>ConstellationHingeOp<br/>L_constellation_centroid loss]
        T008[TASK-TCT-008<br/>ConstellationStore<br/>CF_MEJEPA_CONSTELLATION FROZEN]
        T009[TASK-TCT-009<br/>Max-age policy<br/>MEJEPA_INFER_GTAU_STALE_CONSTELLATION]
        T010[TASK-TCT-010<br/>ViolationRateAggregator<br/>rolling-window for ΔΞ]
    end

    subgraph Surface [Surface Layer]
        T011[TASK-TCT-011<br/>mcp__cgreality__<br/>mejepa_constellation_inspect]
        T012[TASK-TCT-012<br/>FSV harness<br/>4 evidence files]
    end

    %% Foundation -> Logic
    T001 --> T003
    T001 --> T004
    T001 --> T006
    T001 --> T007
    T001 --> T010
    T002 --> T003
    T002 --> T004
    T002 --> T005
    T002 --> T006
    T002 --> T007
    T002 --> T008
    T002 --> T009
    T002 --> T010

    %% Logic chain
    T003 --> T004
    T004 --> T005
    T005 --> T008
    T003 --> T006
    T004 --> T006
    T005 --> T006
    T003 --> T007
    T004 --> T007
    T008 --> T009
    T006 --> T010

    %% Logic -> Surface
    T008 --> T011
    T009 --> T011
    T006 --> T012
    T007 --> T012
    T008 --> T012
    T010 --> T012
```

---

## Execution order (sequential)

| # | Task ID | Title | Layer | Depends on | Status |
|---|---|---|---|---|---|
| 1 | TASK-TCT-001 | Public data models (TctConstellation, CorpusProvenance, MutationCategory, OracleOutcome, Language, EntityType, EmbedderId, ChunkId, GtauViolation, GtauOutput) | foundation | (cross-domain shipped) | ready |
| 2 | TASK-TCT-002 | Internal data models + error (ShrinkageOrigin, ChunkGtauOutput, ConstellationHingeOutput, VerdictGuardRejected, PredictorOutput, RollingWindow, TctError) | foundation | 001 | ready |
| 3 | TASK-TCT-003 | ConstellationBuilder: per-category × 21 (168) + per-language × 8 × 21 (1,512) + outcome × 21 (84) centroids; L2-normalized | logic | 001, 002 | blocked (needs corpus ≥ 2,000) |
| 4 | TASK-TCT-004 | Per-chunk-type centroids: 8 × 13 × 11 × 21 = 24,024 cells; Bayesian shrinkage on N&lt;50; fail-closed N&lt;5 | logic | 003 | blocked |
| 5 | TASK-TCT-005 | ThresholdCalibrator: per-embedder τ_m on held-out KnownGood; FPR ≤ 5%; per-chunk-type sharper when N≥50 | logic | 004 | blocked |
| 6 | TASK-TCT-006 | GtauChecker: gtau_check + gtau_check_chunks; O(N · d_max) deterministic; per-chunk localization | logic | 003, 004, 005 | blocked |
| 7 | TASK-TCT-007 | ConstellationHingeOp: L_constellation_centroid hinge; centroid + embedder DETACHED gradient | logic | 003, 004 | blocked |
| 8 | TASK-TCT-008 | ConstellationStore: CF_MEJEPA_CONSTELLATION persist + load FROZEN; key=(version_id, corpus_sha) | logic | 005 | blocked |
| 9 | TASK-TCT-009 | Constellation max-age policy (default 90 days); operator override audit-logged | logic | 008 | blocked |
| 10 | TASK-TCT-010 | ViolationRateAggregator: rolling-window constellation_violation_rate; Option::None on N&lt;30 | logic | 006 | blocked |
| 11 | TASK-TCT-011 | mcp__cgreality__mejepa_constellation_inspect MCP tool | surface | 008, 009 | blocked |
| 12 | TASK-TCT-012 | FSV harness (constellation-evidence, gtau-evidence, threshold-calibration-evidence, chunk-level-rejection-evidence) | surface | 006, 007, 008, 010 | blocked |

**Status legend:** `ready` (can start now); `blocked` (waits on Phase 0 corpus + Phase 1b/c embedders + panel slots).

---

## Parallel-execution batches

- **Batch F.1** (foundation, parallel-safe): TASK-TCT-001 alone.
- **Batch F.2** (foundation, depends on F.1): TASK-TCT-002.
- **Batch L.1** (logic, depends on F.2 + Phase 0 corpus + ≥18 embedders): TASK-TCT-003 (panel-level builder).
- **Batch L.2** (logic, depends on L.1): TASK-TCT-004 (chunk-level + shrinkage).
- **Batch L.3** (logic, depends on L.2): TASK-TCT-005 (threshold calibration).
- **Batch L.4** (logic, depends on L.3): TASK-TCT-006, TASK-TCT-007, TASK-TCT-008 — can ship in parallel after centroids + thresholds finalized.
- **Batch L.5** (logic, depends on L.4): TASK-TCT-009 (max-age, after store), TASK-TCT-010 (rate aggregator, after Gτ).
- **Batch S.1** (surface, depends on L.4 + L.5): TASK-TCT-011, TASK-TCT-012 — both consume the full set.

---

## Critical path

```
TASK-TCT-001 -> TASK-TCT-002 -> TASK-TCT-003 -> TASK-TCT-004 -> TASK-TCT-005 -> TASK-TCT-006 -> TASK-TCT-008 -> TASK-TCT-012
```

The longest single-task is TASK-TCT-004 (per-chunk-type centroids with Bayesian shrinkage on 24,024 cells). Anchor scheduling on the corpus availability + TASK-TCT-005 calibration (FPR ≤ 5% gate must hold).

---

## Cross-domain dependencies

- **CORPUS (Phase 0)**: provides ≥ 2,000 labeled (panel, mutation_category, oracle_outcome, language, entity_type, chunk_panels) tuples consumed by TASK-TCT-003/004. The mutation operators + Docker oracle bridge are shipped; full 300×8 corpus generation is pending.
- **EMBEDDERS (Phase 1b)**: provides 21 frozen embedders + per-embedder weight SHA-256 digests consumed by TASK-TCT-001 (CorpusProvenance.embedder_versions) + TASK-TCT-008 (provenance check at load).
- **INSTRUMENTS (Phase 1c)**: provides Panel + 15 slot encoders consumed by TASK-TCT-006 / 007 (panel.slot_for(embedder)).
- **AST-CHUNKING (Phase 1a)**: provides ChunkId structure (file_sha, language, entity_type, line_start, line_end, parent_chain_hash) consumed by TASK-TCT-001 / 006 (chunk-level Gτ).
- **SOLVER (shipped)**: provides cosine-similarity primitive (used in gtau and hinge); also provides BLAKE3 for parent_chain_hash.
- **STORAGE (shipped)**: provides RocksDB CF wrappers consumed by TASK-TCT-008 / 010.
- **WITNESS (shipped)**: provides ed25519 segment signing for constellation provenance.
- **INFERENCE (Phase 4a)**: consumes the ConstellationGuard trait + Verdict::GuardRejected variant; before TCT ships, Phase 4a uses MockedAlwaysApprove + provenance_warning per `crates/context-graph-mejepa/src/compiler.rs::ConstellationGuard`.

---

## Coverage roll-up

Every functional REQ-TCT-NN (15 total) and every component contract (7 in TECH-TCT) maps to at least one task in this index. Coverage tracking is in `../_traceability.md`.

| REQ | Title | Task ID(s) |
|---|---|---|
| REQ-TCT-01 | TctConstellation data type with all centroid maps + thresholds + provenance | TASK-TCT-001 |
| REQ-TCT-02 | ConstellationBuilder with per-cell mean + L2 normalize + provenance inputs | TASK-TCT-003 |
| REQ-TCT-03 | Bayesian shrinkage on N&lt;50; fail-closed N&lt;5 | TASK-TCT-004 |
| REQ-TCT-04 | ThresholdCalibrator FPR ≤ 5% per cell with ≥ 30 samples | TASK-TCT-005 |
| REQ-TCT-05 | CF_MEJEPA_CONSTELLATION RocksDB persistence FROZEN | TASK-TCT-008 |
| REQ-TCT-06 | lookup_centroid with shrinkage fallback | TASK-TCT-004 |
| REQ-TCT-07 | constellation_hinge primitive (centroid + embedder detached) | TASK-TCT-007 |
| REQ-TCT-08 | gtau_check (panel-level) deterministic O(N · d_max) | TASK-TCT-006 |
| REQ-TCT-09 | gtau_check_chunks (per-chunk localization) | TASK-TCT-006 |
| REQ-TCT-10 | Verdict::GuardRejected variant; CF_MEJEPA_GUARD_DECISIONS | TASK-TCT-002 (variant) + TASK-TCT-006 (emission) |
| REQ-TCT-11 | Max-age policy + operator override audit-logged | TASK-TCT-009 |
| REQ-TCT-12 | Provenance mismatch check at load | TASK-TCT-008 |
| REQ-TCT-13 | constellation_violation_rate rolling-window | TASK-TCT-010 |
| REQ-TCT-14 | FSV harness 4 evidence files | TASK-TCT-012 |
| REQ-TCT-15 | mcp__cgreality__mejepa_constellation_inspect | TASK-TCT-011 |

| Tech component | Layer | Task ID |
|---|---|---|
| TctConstellation | foundation/logic | TASK-TCT-001, TASK-TCT-009 |
| ConstellationBuilder | logic | TASK-TCT-003, TASK-TCT-004 |
| ThresholdCalibrator | logic | TASK-TCT-005 |
| GtauChecker | logic | TASK-TCT-006 |
| ConstellationHingeOp | logic | TASK-TCT-007 |
| ConstellationStore | logic | TASK-TCT-008 |
| ViolationRateAggregator | logic | TASK-TCT-010 |
| McpInspectTool | surface | TASK-TCT-011 |

---

## Authority cross-reference

- Functional spec: `../../functional/SPEC-TCT.md`
- Technical spec: `../../technical/TECH-TCT.md`
- Constitution: `../../constitution.md` (`<tct_invariants>`)
- Master traceability: `../_traceability.md`
- Cross-cutting consumer: `../phase4-inference/_index.md` (Gate 7 ConstellationGuardRejected); `../phase3-training/` (L_constellation_centroid loss surface integrator); `../utml-learning-formula/` (constellation_violation_rate ΔΞ consumer)
- Successor phase: `../phase5-self-healing/` (rolling-window refit trigger)
- Architectural sources: `docs/ruvectorfindings/13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md` (FULL); `docs/ruvectorfindings/14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md` §5, §8.1; `docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md` §5.3
- Paper provenance: Royse, C. (April 2026). *Teleological Constellation Training: Multi-Modal Embedding Decomposition...* DOI 10.13140/RG.2.2.24370.57288
