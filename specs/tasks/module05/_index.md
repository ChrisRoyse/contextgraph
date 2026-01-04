# Module 05: UTL Integration - Task Index

## Overview

**Module**: 05 - UTL Integration (Unified Temporal Learning)
**Total Tasks**: 63
**Estimated Duration**: 7 weeks
**Created**: 2025-12-31
**Updated**: 2026-01-04
**Version**: 1.4.0

## IMPORTANT: Current State

The `context-graph-utl` crate has been **fully implemented** with 369 tests passing.

**Verification:**
```bash
cargo test -p context-graph-utl --lib 2>&1 | tail -3
# Expected: test result: ok. 369 passed; 0 failed;
```

**Git Reference:** `f521803 feat(utl): complete context-graph-utl crate with 453 tests passing`

## Task Summary

| Status | Count |
|--------|-------|
| Completed | 34 |
| Pending (MCP Integration) | 29 |
| In Progress | 0 |
| Blocked | 0 |

## Task Index by Layer

### Initialization Layer (Week 0) - COMPLETE

| Task ID | Title | Priority | Hours | Status | Dependencies |
|---------|-------|----------|-------|--------|--------------|
| [M05-T00](M05-T00.md) | Initialize context-graph-utl Crate Structure | critical | 2 | **COMPLETE** | - |

### Foundation Layer (Week 1) - COMPLETE

| Task ID | Title | Priority | Hours | Status | Dependencies |
|---------|-------|----------|-------|--------|--------------|
| [M05-T01](M05-T01.md) | Implement UtlConfig and UtlThresholds | high | 2 | **COMPLETE** | M05-T00 |
| [M05-T02](M05-T02.md) | Implement SurpriseConfig | high | 1.5 | **COMPLETE** | M05-T00 |
| [M05-T03](M05-T03.md) | Implement CoherenceConfig | high | 1.5 | **COMPLETE** | M05-T00 |
| [M05-T04](M05-T04.md) | Implement EmotionalConfig | medium | 1.5 | **COMPLETE** | M05-T00 |
| M05-T05 | Implement LifecycleStage Enum | high | 1.5 | **COMPLETE** | M05-T00 |
| M05-T06 | Implement LifecycleLambdaWeights | high | 1.5 | **COMPLETE** | M05-T00 |
| M05-T07 | Implement LifecycleConfig and StageConfig | high | 2 | **COMPLETE** | M05-T05, M05-T06 |
| M05-T08 | Implement JohariQuadrant and SuggestedAction | high | 1.5 | **COMPLETE** | M05-T00 |
| M05-T32 | Implement PhaseConfig Struct | medium | 1 | **COMPLETE** | M05-T00 |
| M05-T33 | Implement JohariConfig Struct | medium | 1 | **COMPLETE** | M05-T00 |

**Verification:** All config structs in `crates/context-graph-utl/src/config.rs` (1274 lines)

### Logic Layer (Week 2) - COMPLETE

| Task ID | Title | Priority | Hours | Status | Dependencies |
|---------|-------|----------|-------|--------|--------------|
| M05-T09 | Implement KL Divergence Computation | high | 2 | **COMPLETE** | M05-T02 |
| M05-T10 | Implement Surprise Computation Methods | high | 2 | **COMPLETE** | M05-T09 |
| M05-T11 | Implement SurpriseCalculator | high | 2.5 | **COMPLETE** | M05-T10 |
| M05-T12 | Implement CoherenceEntry and Window | high | 1.5 | **COMPLETE** | M05-T03 |
| M05-T13 | Implement CoherenceTracker | high | 2.5 | **COMPLETE** | M05-T12 |
| M05-T14 | Implement Structural Coherence (Stub) | medium | 1.5 | **COMPLETE** | M05-T13 |
| M05-T15 | Implement EmotionalState Struct | medium | 1 | **COMPLETE** | M05-T04 |
| M05-T16 | Implement EmotionalWeightCalculator | medium | 2 | **COMPLETE** | M05-T15 |
| M05-T17 | Implement PhaseOscillator | medium | 2 | **COMPLETE** | M05-T32 |
| M05-T31 | Implement Sentiment Lexicon | medium | 2 | **COMPLETE** | M05-T04 |

**Verification:**
- `crates/context-graph-utl/src/surprise/` (kl_divergence.rs, calculator.rs, embedding_distance.rs)
- `crates/context-graph-utl/src/coherence/` (window.rs, tracker.rs, structural.rs)
- `crates/context-graph-utl/src/emotional/` (state.rs, calculator.rs, lexicon.rs)
- `crates/context-graph-utl/src/phase/` (oscillator.rs, consolidation.rs)

### Surface Layer (Week 3) - MOSTLY COMPLETE

| Task ID | Title | Priority | Hours | Status | Dependencies |
|---------|-------|----------|-------|--------|--------------|
| M05-T18 | Implement JohariClassifier | high | 2 | **COMPLETE** | M05-T08, M05-T33 |
| M05-T19 | Implement LifecycleManager State Machine | high | 3 | **COMPLETE** | M05-T07 |
| M05-T20 | Implement Core UTL Learning Magnitude | critical | 3 | **COMPLETE** | M05-T11, M05-T13, M05-T16, M05-T17 |
| M05-T21 | Implement LearningSignal and UtlState | high | 2 | **COMPLETE** | M05-T18, M05-T19, M05-T20 |
| M05-T22 | Implement UtlProcessor Orchestrator | critical | 4 | **partial** | M05-T11, M05-T13, M05-T16, M05-T17, M05-T18, M05-T19, M05-T20, M05-T21 |
| M05-T23 | Implement UtlError Enum | high | 1.5 | **COMPLETE** | M05-T00 |
| M05-T30 | Implement SessionContext | medium | 2 | pending | M05-T22 |

**Verification:**
- `crates/context-graph-utl/src/johari/` (classifier.rs, retrieval.rs)
- `crates/context-graph-utl/src/lifecycle/` (manager.rs, stage.rs, lambda.rs)
- `crates/context-graph-utl/src/lib.rs` (compute_learning_magnitude)
- `crates/context-graph-utl/src/error.rs`

### Testing Layer (Week 4) - PARTIALLY COMPLETE

| Task ID | Title | Priority | Hours | Status | Dependencies |
|---------|-------|----------|-------|--------|--------------|
| M05-T24 | Implement UtlMetrics and UtlStatus | high | 2 | **COMPLETE** | M05-T05, M05-T06, M05-T08, M05-T22 |
| M05-T25 | Create Integration Tests and Benchmarks | high | 4 | **partial** | M05-T22, M05-T24 |
| M05-T34 | Create config/utl.yaml Configuration File | high | 2 | pending | M05-T01, M05-T02, M05-T03, M05-T04, M05-T32, M05-T33 |
| [M05-T40](M05-T40.md) | Implement UTL Feature Flag Gating | high | 2 | pending | M05-T22 |

**Note:** UtlMetrics is in context-graph-core, re-exported via context-graph-utl.

### Integration Layer (Week 5) - PENDING (MCP Work)

| Task ID | Title | Priority | Hours | Status | Dependencies |
|---------|-------|----------|-------|--------|--------------|
| M05-T26 | Implement utl_status MCP Tool | high | 2.5 | pending | M05-T22, M05-T24 |
| M05-T27 | Implement get_memetic_status UTL Integration | medium | 2 | pending | M05-T26 |
| M05-T28 | Implement CognitivePulse Header | high | 2 | pending | M05-T22, M05-T18 |
| M05-T29 | Implement MemoryNode UTL Extension | high | 2 | pending | M05-T21, M05-T08 |
| M05-T35 | Implement KnowledgeGraph Coherence Integration | medium | 2.5 | pending | M05-T14 |
| M05-T36 | Implement Steering Subsystem Hooks | high | 2.5 | pending | M05-T19, M05-T22 |
| M05-T37 | Implement Johari to Verbosity Tier Mapping | medium | 1.5 | pending | M05-T18, M05-T28 |
| M05-T39 | Implement UtlState Persistence to RocksDB | high | 3 | pending | M05-T21, M05-T19 |
| [M05-T58](M05-T58.md) | Implement UTL Subscribable Pulse Resource | high | 2 | pending | M05-T22, M05-T28 |
| [M05-T59](M05-T59.md) | Implement Priors Vibe Check UTL Integration | medium | 2.5 | pending | M05-T21, M05-T22 |
| [M05-T60](M05-T60.md) | Implement Tool Gating Warning System | high | 2 | pending | M05-T22, M05-T18 |
| [M05-T61](M05-T61.md) | Implement Conflict Alert Detection | high | 2.5 | pending | M05-T22 |

### Extended Integration Layer (Week 6) - PENDING (MCP Work)

| Task ID | Title | Priority | Hours | Status | Dependencies |
|---------|-------|----------|-------|--------|--------------|
| M05-T38 | Implement inject_context UTL Integration | critical | 4 | pending | M05-T18, M05-T22, M05-T37, M05-T28, M05-T60, M05-T61 |
| [M05-T41](M05-T41.md) | Implement Neuromodulation Interface Stubs | medium | 2 | pending | M05-T22, M05-T36 |
| [M05-T42](M05-T42.md) | Implement Entropy/Coherence Threshold Triggers | high | 3 | pending | M05-T22, M05-T28, M05-T62 |
| [M05-T43](M05-T43.md) | Implement UTL-Aware Distillation Mode Selection | medium | 2 | pending | M05-T18, M05-T38 |
| [M05-T44](M05-T44.md) | Implement UTL Resource Endpoints | medium | 3 | pending | M05-T22, M05-T26, M05-T39, M05-T58 |
| M05-T45 | Implement store_memory UTL Validation and Steering Feedback | high | 3 | pending | M05-T22, M05-T19, M05-T36, M05-T59 |
| M05-T46 | Create UTL Chaos and Edge Case Tests | high | 4 | pending | M05-T25, M05-T22 |
| M05-T47 | Implement UTL Validation Test Suite | high | 5 | pending | M05-T25, M05-T22, M05-T19 |
| [M05-T62](M05-T62.md) | Implement Dynamic UTL Thresholds by Lifecycle Stage | high | 2 | pending | M05-T05, M05-T07, M05-T19 |

### Completion Layer (Week 7) - PENDING

| Task ID | Title | Priority | Hours | Status | Dependencies |
|---------|-------|----------|-------|--------|--------------|
| M05-T48 | Implement Salience Update Algorithm | high | 2 | pending | M05-T21, M05-T29 |
| M05-T49 | Implement UTL Composite Loss Function | medium | 2.5 | pending | M05-T20, M05-T21 |
| M05-T50 | Implement Predictive Coding Interface Stubs | medium | 2 | pending | M05-T22 |
| M05-T51 | Implement Active Inference Interface Stubs | medium | 2 | pending | M05-T22, M05-T21 |
| M05-T52 | Migrate and Re-export Core UTL Types | high | 3 | **COMPLETE** | M05-T00, M05-T21 |
| M05-T53 | Implement UTL-Aware search_graph Integration | medium | 2.5 | pending | M05-T18, M05-T22 |
| M05-T54 | Implement get_graph_manifest UTL Section | medium | 2 | pending | M05-T22, M05-T24 |
| [M05-T55](M05-T55.md) | Implement Hyperbolic Entailment Interface Stubs | low | 2 | pending | M05-T22 |
| [M05-T56](M05-T56.md) | Create API Documentation for Module 5 Public Types | medium | 3 | pending | M05-T22, M05-T24 |
| [M05-T57](M05-T57.md) | Create Performance Benchmark CI/CD Integration | high | 3 | pending | M05-T25 |
| [M05-T63](M05-T63.md) | Implement Synthetic Data Seeding Support | medium | 3 | pending | M05-T22, M05-T29 |

## Completed Tasks Summary

The following components exist in `crates/context-graph-utl/`:

| Component | Files | Tests |
|-----------|-------|-------|
| Config | `src/config.rs` (1274 lines) | 14 tests |
| Error | `src/error.rs` | 5 tests |
| Surprise | `src/surprise/*.rs` | 50+ tests |
| Coherence | `src/coherence/*.rs` | 50+ tests |
| Emotional | `src/emotional/*.rs` | 40+ tests |
| Phase | `src/phase/*.rs` | 30+ tests |
| Johari | `src/johari/*.rs` | 40+ tests |
| Lifecycle | `src/lifecycle/*.rs` | 60+ tests |
| Core | `src/lib.rs` | 17 tests |

**Total: 369 passing tests**

## Next Priority Tasks

The remaining tasks are primarily MCP integration work:

1. **M05-T22**: Complete UtlProcessor orchestrator (partially done)
2. **M05-T26-M05-T29**: MCP tool implementations
3. **M05-T38**: inject_context UTL integration (CRITICAL)
4. **M05-T34**: config/utl.yaml file

## Quality Gates

| Gate | Criteria | Status |
|------|----------|--------|
| Crate Initialized | M05-T00 complete, crate compiles | **PASS** |
| Foundation Complete | M05-T01 through M05-T08, M05-T32, M05-T33 pass | **PASS** |
| Logic Complete | M05-T09 through M05-T17, M05-T31 pass | **PASS** |
| Surface Complete | M05-T18 through M05-T24, M05-T30 pass | **PARTIAL** |
| Testing Complete | M05-T25, M05-T34, M05-T40 pass, 90%+ coverage | **PARTIAL** |
| Integration Complete | M05-T26-T29, M05-T35-T37, M05-T39, M05-T58-T61 pass | PENDING |
| Extended Integration | M05-T38, M05-T41-T47, M05-T62 complete | PENDING |
| Module Complete | All 63 tasks complete, benchmarks pass | PENDING |

## Performance Targets

| Operation | Target | P99 Target | Status |
|-----------|--------|------------|--------|
| `compute_learning_magnitude` | <100us | <500us | IMPLEMENTED |
| Surprise (KL) | <5ms | <20ms | IMPLEMENTED |
| Surprise (distance) | <1ms | <5ms | IMPLEMENTED |
| Coherence computation | <5ms | <25ms | IMPLEMENTED |
| Emotional weight | <1ms | <5ms | IMPLEMENTED |
| Phase update | <10us | <50us | IMPLEMENTED |
| Johari classification | <1us | <5us | IMPLEMENTED |

## Critical Constraints (All Implemented)

1. **NO NaN/Infinity**: All UTL computations clamp inputs and validate outputs
2. **Lambda Weight Invariant**: lambda_novelty + lambda_consolidation = 1.0
3. **Lifecycle Transitions**: Infancy -> Growth at 50, Growth -> Maturity at 500
4. **Thread Safety**: All types implement Send + Sync where needed

## Specification References

- `constitution.yaml` lines 148-167 - UTL parameter definitions
- `contextprd.md` Section 2.1 - UTL formula
- `learntheory.md` - Theoretical background

---

*Index updated: 2026-01-04*
*Module: 05 - UTL Integration*
*Version: 1.4.0*
*Implementation status: Core crate complete, MCP integration pending*
