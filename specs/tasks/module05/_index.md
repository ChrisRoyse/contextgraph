# Module 05: UTL Integration - Task Index

## Overview

**Module**: 05 - UTL Integration (Unified Temporal Learning)
**Total Tasks**: 57
**Estimated Duration**: 7 weeks
**Created**: 2025-12-31
**Updated**: 2026-01-04
**Version**: 1.2.0

## Task Summary

| Status | Count |
|--------|-------|
| Pending | 57 |
| In Progress | 0 |
| Completed | 0 |
| Blocked | 0 |

## Task Index by Layer

### Initialization Layer (Week 0)

| Task ID | Title | Priority | Hours | Status | Dependencies |
|---------|-------|----------|-------|--------|--------------|
| [M05-T00](M05-T00.md) | Initialize context-graph-utl Crate Structure | critical | 2 | pending | - |

### Foundation Layer (Week 1)

| Task ID | Title | Priority | Hours | Status | Dependencies |
|---------|-------|----------|-------|--------|--------------|
| [M05-T01](M05-T01.md) | Implement UtlConfig and UtlThresholds | high | 2 | pending | M05-T00 |
| [M05-T02](M05-T02.md) | Implement SurpriseConfig | high | 1.5 | pending | M05-T00 |
| [M05-T03](M05-T03.md) | Implement CoherenceConfig | high | 1.5 | pending | M05-T00 |
| [M05-T04](M05-T04.md) | Implement EmotionalConfig | medium | 1.5 | pending | M05-T00 |
| M05-T05 | Implement LifecycleStage Enum | high | 1.5 | pending | M05-T00 |
| M05-T06 | Implement LifecycleLambdaWeights | high | 1.5 | pending | M05-T00 |
| M05-T07 | Implement LifecycleConfig and StageConfig | high | 2 | pending | M05-T05, M05-T06 |
| M05-T08 | Implement JohariQuadrant and SuggestedAction | high | 1.5 | pending | M05-T00 |
| M05-T32 | Implement PhaseConfig Struct | medium | 1 | pending | M05-T00 |
| M05-T33 | Implement JohariConfig Struct | medium | 1 | pending | M05-T00 |

### Logic Layer (Week 2)

| Task ID | Title | Priority | Hours | Status | Dependencies |
|---------|-------|----------|-------|--------|--------------|
| M05-T09 | Implement KL Divergence Computation | high | 2 | pending | M05-T02 |
| M05-T10 | Implement Surprise Computation Methods | high | 2 | pending | M05-T09 |
| M05-T11 | Implement SurpriseCalculator | high | 2.5 | pending | M05-T10 |
| M05-T12 | Implement CoherenceEntry and Window | high | 1.5 | pending | M05-T03 |
| M05-T13 | Implement CoherenceTracker | high | 2.5 | pending | M05-T12 |
| M05-T14 | Implement Structural Coherence (Stub) | medium | 1.5 | pending | M05-T13 |
| M05-T15 | Implement EmotionalState Struct | medium | 1 | pending | M05-T04 |
| M05-T16 | Implement EmotionalWeightCalculator | medium | 2 | pending | M05-T15 |
| M05-T17 | Implement PhaseOscillator | medium | 2 | pending | M05-T32 |
| M05-T31 | Implement Sentiment Lexicon | medium | 2 | pending | M05-T04 |

### Surface Layer (Week 3)

| Task ID | Title | Priority | Hours | Status | Dependencies |
|---------|-------|----------|-------|--------|--------------|
| M05-T18 | Implement JohariClassifier | high | 2 | pending | M05-T08, M05-T33 |
| M05-T19 | Implement LifecycleManager State Machine | high | 3 | pending | M05-T07 |
| M05-T20 | Implement Core UTL Learning Magnitude | critical | 3 | pending | M05-T11, M05-T13, M05-T16, M05-T17 |
| M05-T21 | Implement LearningSignal and UtlState | high | 2 | pending | M05-T18, M05-T19, M05-T20 |
| M05-T22 | Implement UtlProcessor Orchestrator | critical | 4 | pending | M05-T11, M05-T13, M05-T16, M05-T17, M05-T18, M05-T19, M05-T20, M05-T21 |
| M05-T23 | Implement UtlError Enum | high | 1.5 | pending | M05-T00 |
| M05-T30 | Implement SessionContext | medium | 2 | pending | M05-T22 |

### Testing Layer (Week 4)

| Task ID | Title | Priority | Hours | Status | Dependencies |
|---------|-------|----------|-------|--------|--------------|
| M05-T24 | Implement UtlMetrics and UtlStatus | high | 2 | pending | M05-T05, M05-T06, M05-T08, M05-T22 |
| M05-T25 | Create Integration Tests and Benchmarks | high | 4 | pending | M05-T22, M05-T24 |
| M05-T34 | Create config/utl.yaml Configuration File | high | 2 | pending | M05-T01, M05-T02, M05-T03, M05-T04, M05-T32, M05-T33 |
| [M05-T40](M05-T40.md) | Implement UTL Feature Flag Gating | high | 2 | pending | M05-T22 |

### Integration Layer (Week 5)

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

### Extended Integration Layer (Week 6)

| Task ID | Title | Priority | Hours | Status | Dependencies |
|---------|-------|----------|-------|--------|--------------|
| M05-T38 | Implement inject_context UTL Integration | critical | 4 | pending | M05-T18, M05-T22, M05-T37, M05-T28 |
| [M05-T41](M05-T41.md) | Implement Neuromodulation Interface Stubs | medium | 2 | pending | M05-T22, M05-T36 |
| [M05-T42](M05-T42.md) | Implement Entropy/Coherence Threshold Triggers | high | 3 | pending | M05-T22, M05-T28 |
| [M05-T43](M05-T43.md) | Implement UTL-Aware Distillation Mode Selection | medium | 2 | pending | M05-T18, M05-T38 |
| [M05-T44](M05-T44.md) | Implement UTL Resource Endpoints | medium | 3 | pending | M05-T22, M05-T26, M05-T39 |
| M05-T45 | Implement store_memory UTL Validation and Steering Feedback | high | 3 | pending | M05-T22, M05-T19, M05-T36 |
| M05-T46 | Create UTL Chaos and Edge Case Tests | high | 4 | pending | M05-T25, M05-T22 |
| M05-T47 | Implement UTL Validation Test Suite | high | 5 | pending | M05-T25, M05-T22, M05-T19 |

### Completion Layer (Week 7)

| Task ID | Title | Priority | Hours | Status | Dependencies |
|---------|-------|----------|-------|--------|--------------|
| M05-T48 | Implement Salience Update Algorithm | high | 2 | pending | M05-T21, M05-T29 |
| M05-T49 | Implement UTL Composite Loss Function | medium | 2.5 | pending | M05-T20, M05-T21 |
| M05-T50 | Implement Predictive Coding Interface Stubs | medium | 2 | pending | M05-T22 |
| M05-T51 | Implement Active Inference Interface Stubs | medium | 2 | pending | M05-T22, M05-T21 |
| M05-T52 | Migrate and Re-export Core UTL Types | high | 3 | pending | M05-T00, M05-T21 |
| M05-T53 | Implement UTL-Aware search_graph Integration | medium | 2.5 | pending | M05-T18, M05-T22 |
| M05-T54 | Implement get_graph_manifest UTL Section | medium | 2 | pending | M05-T22, M05-T24 |
| [M05-T55](M05-T55.md) | Implement Hyperbolic Entailment Interface Stubs | low | 2 | pending | M05-T22 |
| [M05-T56](M05-T56.md) | Create API Documentation for Module 5 Public Types | medium | 3 | pending | M05-T22, M05-T24 |
| [M05-T57](M05-T57.md) | Create Performance Benchmark CI/CD Integration | high | 3 | pending | M05-T25 |

## Execution Order

### Phase 0: Initialization (Day 1-2)
1. M05-T00: Initialize context-graph-utl crate structure

### Phase 1: Foundation Types (Week 1)
1. M05-T01: UtlConfig and UtlThresholds
2. M05-T02: SurpriseConfig
3. M05-T03: CoherenceConfig
4. M05-T04: EmotionalConfig
5. M05-T05: LifecycleStage enum
6. M05-T06: LifecycleLambdaWeights
7. M05-T07: LifecycleConfig and StageConfig
8. M05-T08: JohariQuadrant and SuggestedAction
9. M05-T32: PhaseConfig struct
10. M05-T33: JohariConfig struct

### Phase 2: Component Logic (Week 2)
1. M05-T09: KL Divergence computation
2. M05-T10: Surprise computation methods
3. M05-T11: SurpriseCalculator
4. M05-T12: CoherenceEntry and window
5. M05-T13: CoherenceTracker
6. M05-T14: Structural coherence (stub)
7. M05-T15: EmotionalState struct
8. M05-T16: EmotionalWeightCalculator
9. M05-T17: PhaseOscillator
10. M05-T31: Sentiment lexicon

### Phase 3: Surface Layer (Week 3)
1. M05-T18: JohariClassifier
2. M05-T19: LifecycleManager state machine
3. M05-T20: Core UTL learning magnitude
4. M05-T21: LearningSignal and UtlState
5. M05-T22: UtlProcessor orchestrator
6. M05-T23: UtlError enum
7. M05-T30: SessionContext

### Phase 4: Testing (Week 4)
1. M05-T24: UtlMetrics and UtlStatus
2. M05-T25: Integration tests and benchmarks
3. M05-T34: config/utl.yaml creation
4. M05-T40: UTL Feature Flag Gating

### Phase 5: Integration (Week 5)
1. M05-T26: utl_status MCP tool
2. M05-T27: get_memetic_status UTL integration
3. M05-T28: CognitivePulse header
4. M05-T29: MemoryNode UTL extension
5. M05-T35: KnowledgeGraph integration
6. M05-T36: Steering subsystem hooks
7. M05-T37: Johari to verbosity tier mapping
8. M05-T39: UtlState persistence

### Phase 6: Extended Integration (Week 6)
1. M05-T38: inject_context UTL Integration (CRITICAL)
2. M05-T41: Neuromodulation interface stubs
3. M05-T42: Entropy/Coherence threshold triggers
4. M05-T43: UTL-aware distillation mode selection
5. M05-T44: UTL resource endpoints
6. M05-T45: store_memory UTL validation
7. M05-T46: Chaos and edge case tests
8. M05-T47: Validation test suite

### Phase 7: Completion (Week 7)
1. M05-T48: Salience update algorithm
2. M05-T49: UTL composite loss function
3. M05-T50: Predictive coding interface stubs
4. M05-T51: Active inference interface stubs
5. M05-T52: Core UTL type migration
6. M05-T53: UTL-aware search_graph integration
7. M05-T54: get_graph_manifest UTL section
8. M05-T55: Hyperbolic entailment interface stubs
9. M05-T56: API documentation
10. M05-T57: Performance benchmark CI/CD integration

## Quality Gates

| Gate | Criteria | Required For |
|------|----------|--------------|
| Crate Initialized | M05-T00 complete, crate compiles | Phase 1 start |
| Foundation Complete | M05-T01 through M05-T08, M05-T32, M05-T33 pass | Phase 2 start |
| Logic Complete | M05-T09 through M05-T17, M05-T31 pass | Phase 3 start |
| Surface Complete | M05-T18 through M05-T24, M05-T30 pass | Phase 4 start |
| Testing Complete | M05-T25, M05-T34, M05-T40 pass, 90%+ coverage | Phase 5 start |
| Integration Complete | M05-T26 through M05-T29, M05-T35-T37, M05-T39 pass | Phase 6 start |
| Extended Integration | M05-T38, M05-T41-T45 complete, inject_context verified | Phase 7 start |
| Validation Complete | M05-T46, M05-T47 pass, chaos tests green, r > 0.7 | Completion start |
| Module Complete | All 57 tasks complete, benchmarks pass, CI gates green | Module 6 ready |

## Performance Targets

| Operation | Target | P99 Target | Conditions |
|-----------|--------|------------|------------|
| `compute_learning_magnitude` | <100us | <500us | Core equation only |
| Full UTL computation | <10ms | <50ms | All components |
| Surprise (KL) | <5ms | <20ms | 1536D, 50 context |
| Surprise (distance) | <1ms | <5ms | 1536D, 50 context |
| Coherence computation | <5ms | <25ms | 100 window entries |
| Emotional weight | <1ms | <5ms | Text analysis |
| Phase update | <10us | <50us | Simple math |
| Johari classification | <1us | <5us | Two comparisons |

## Critical Constraints

1. **NO NaN/Infinity**: All UTL computations MUST clamp inputs and validate outputs
2. **Lambda Weight Invariant**: lambda_novelty + lambda_consolidation = 1.0
3. **Lifecycle Transitions**: Infancy -> Growth at 50, Growth -> Maturity at 500
4. **Thread Safety**: UtlProcessor must be Send + Sync

## Specification References

- `SPEC-UTL-005` - UTL Technical Specification
- `constitution.yaml` - Project constitution
- `contextgraphprd.md` - Product Requirements Document
- `TECH-UTL-005` - Technical Implementation Guide

---

*Index generated: 2026-01-04*
*Module: 05 - UTL Integration*
*Version: 1.2.0*
