# Task Traceability Matrix: Module 04 - Knowledge Graph

```yaml
metadata:
  module_id: "module-04"
  module_name: "Knowledge Graph"
  version: "1.1.0"
  created: "2026-01-02"
  total_requirements: 28
  total_tasks: 33
  coverage_status: "complete"
```

---

## Purpose

This matrix ensures every requirement, component, and behavior from the Module 04 specifications (SPEC-GRAPH-004, TECH-GRAPH-004) is covered by at least one atomic task. **Empty "Task ID" columns indicate incomplete coverage and MUST be addressed.**

This document serves as the verification layer between:
1. **Requirements** (REQ-KG-*) - What must be implemented
2. **Tasks** (M04-T*) - How it will be implemented
3. **Verification** - Proof of completion

---

## Coverage Matrices

### Data Models / Entities

| Item | Description | Task ID | Verified |
|------|-------------|---------|----------|
| IndexConfig | FAISS IVF-PQ configuration (nlist, nprobe, pq_segments, pq_bits) | M04-T01 | [ ] |
| Vector1536 | 1536D embedding vector re-export from core | M04-T01a | [ ] |
| HyperbolicConfig | Poincare ball parameters (dim=64, curvature=-1.0) | M04-T02 | [ ] |
| CurvatureValidation | Validation ensuring curvature < 0 | M04-T02a | [ ] |
| ConeConfig | Entailment cone parameters (aperture, decay) | M04-T03 | [ ] |
| PoincarePoint | 64D point in Poincare ball with coords: [f32; 64] | M04-T04 | [ ] |
| PoincareBall | Mobius algebra operations (add, distance, exp_map, log_map) | M04-T05 | [ ] |
| EntailmentCone | Cone struct with apex, aperture, aperture_factor, depth | M04-T06 | [ ] |
| ContainmentLogic | O(1) IS-A containment check algorithm | M04-T07 | [ ] |
| GraphError | Comprehensive error enum (17+ variants) | M04-T08 | [ ] |
| ErrorConversions | From<rocksdb::Error> and other conversions | M04-T08a | [ ] |
| FaissFFI | C bindings to FAISS library | M04-T09 | [ ] |
| GpuResources | RAII wrapper for GPU resources | M04-T09 | [ ] |
| FaissGpuIndex | GPU index wrapper with train/search/add methods | M04-T10 | [ ] |
| SearchResult | Query results with ids, distances, k, num_queries | M04-T11 | [ ] |
| ColumnFamilies | RocksDB CF definitions (adjacency, hyperbolic, cones) | M04-T12 | [ ] |
| GraphStorage | RocksDB backend for graph data | M04-T13 | [ ] |
| StorageMigrations | Schema versioning and migration support | M04-T13a | [ ] |
| NeurotransmitterWeights | Marblestone NT struct (excitatory, inhibitory, modulatory) | M04-T14 | [ ] |
| NTWeightValidation | Range validation [0,1] for NT weights | M04-T14a | [ ] |
| GraphEdge | Edge with 13 Marblestone fields | M04-T15 | [ ] |
| EdgeType | Enum: Semantic, Temporal, Causal, Hierarchical, CONTRADICTS | M04-T15, M04-T26 | [ ] |
| Domain | Enum: Code, Legal, Medical, Creative, Research, General | M04-T15 | [ ] |
| BfsResult | BFS traversal result with nodes, edges, depth_counts | M04-T16 | [ ] |
| BfsParams | BFS parameters (max_depth, max_nodes, edge_types, domain_filter) | M04-T16 | [ ] |
| AStarHeuristic | Hyperbolic heuristic for A* traversal | M04-T17a | [ ] |
| DomainSearchResult | Domain-aware search result with modulated_score | M04-T19 | [ ] |
| ContradictionResult | Contradiction detection result with type and confidence | M04-T21 | [ ] |
| ContradictionType | Enum: DirectOpposition, LogicalInconsistency, TemporalConflict, CausalConflict | M04-T21 | [ ] |
| GpuMemoryManager | VRAM budget tracking and allocation | M04-T28 | [ ] |

---

### Requirements to Tasks Mapping

| Requirement ID | Description | Task IDs | Verified |
|----------------|-------------|----------|----------|
| REQ-KG-001 | FAISS IVF-PQ index creation | M04-T01, M04-T09, M04-T10 | [ ] |
| REQ-KG-002 | Index dimension 1536 | M04-T01 | [ ] |
| REQ-KG-003 | nlist=16384 for IVF partitioning | M04-T01 | [ ] |
| REQ-KG-004 | nprobe=128 for search quality | M04-T01, M04-T10 | [ ] |
| REQ-KG-005 | PQ64x8 compression | M04-T01 | [ ] |
| REQ-KG-006 | min_train_vectors = 256 * nlist | M04-T01, M04-T10 | [ ] |
| REQ-KG-007 | GPU index transfer | M04-T10 | [ ] |
| REQ-KG-008 | Incremental add_with_ids | M04-T10 | [ ] |
| REQ-KG-040 | Edge weight modulation | M04-T15, M04-T22 | [ ] |
| REQ-KG-041 | Edge confidence scoring | M04-T15 | [ ] |
| REQ-KG-042 | Edge domain tagging | M04-T15 | [ ] |
| REQ-KG-043 | Amortized shortcuts | M04-T15 | [ ] |
| REQ-KG-044 | Steering reward tracking | M04-T15 | [ ] |
| REQ-KG-050 | Poincare ball model (64D) | M04-T02, M04-T04 | [ ] |
| REQ-KG-051 | Mobius operations (add, distance, exp/log maps) | M04-T05 | [ ] |
| REQ-KG-052 | Entailment cones with aperture decay | M04-T03, M04-T06 | [ ] |
| REQ-KG-053 | O(1) containment check | M04-T07 | [ ] |
| REQ-KG-054 | max_norm < 1.0 boundary enforcement | M04-T02, M04-T04 | [ ] |
| REQ-KG-060 | Semantic k-NN search | M04-T18 | [ ] |
| REQ-KG-061 | Graph traversal (BFS/DFS) | M04-T16, M04-T17, M04-T17a | [ ] |
| REQ-KG-062 | Entailment hierarchy query | M04-T20 | [ ] |
| REQ-KG-063 | Contradiction detection | M04-T21, M04-T26 | [ ] |
| REQ-KG-064 | Search filters (importance, johari, created_after, agent_id) | M04-T18 | [ ] |
| REQ-KG-065 | Marblestone neurotransmitter integration | M04-T14, M04-T14a, M04-T15, M04-T19, M04-T22 | [ ] |
| REQ-KG-TEST | No mock FAISS - real GPU index required | M04-T25 | [ ] |

---

### Spec Section to Task Mapping

| Spec Reference | Section Description | Task IDs | Verified |
|----------------|---------------------|----------|----------|
| TECH-GRAPH-004 Section 2 | FAISS IVF-PQ Configuration | M04-T01, M04-T01a | [ ] |
| TECH-GRAPH-004 Section 3.1 | FAISS FFI Bindings | M04-T09 | [ ] |
| TECH-GRAPH-004 Section 3.2 | FaissGpuIndex Implementation | M04-T10, M04-T11 | [ ] |
| TECH-GRAPH-004 Section 4 | RocksDB Column Families | M04-T12 | [ ] |
| TECH-GRAPH-004 Section 4.1 | Marblestone Edge Fields | M04-T14, M04-T15 | [ ] |
| TECH-GRAPH-004 Section 4.2 | GraphStorage Backend | M04-T13, M04-T13a | [ ] |
| TECH-GRAPH-004 Section 5 | Hyperbolic Configuration | M04-T02, M04-T02a | [ ] |
| TECH-GRAPH-004 Section 5.1 | PoincarePoint Definition | M04-T04 | [ ] |
| TECH-GRAPH-004 Section 5.2 | Mobius Operations | M04-T05 | [ ] |
| TECH-GRAPH-004 Section 6 | Entailment Cones | M04-T03, M04-T06, M04-T07 | [ ] |
| TECH-GRAPH-004 Section 7.1 | BFS Traversal | M04-T16 | [ ] |
| TECH-GRAPH-004 Section 7.2 | DFS Traversal | M04-T17 | [ ] |
| TECH-GRAPH-004 Section 8 | Query Operations | M04-T18, M04-T19, M04-T20, M04-T21, M04-T22 | [ ] |
| TECH-GRAPH-004 Section 9 | Error Handling | M04-T08, M04-T08a | [ ] |
| TECH-GRAPH-004 Section 10.1 | Poincare CUDA Kernel | M04-T23 | [ ] |
| TECH-GRAPH-004 Section 10.2 | Cone CUDA Kernel | M04-T24 | [ ] |
| TECH-GRAPH-004 Section 11 | Integration Tests | M04-T25 | [ ] |

---

### Performance Targets

| Target | Metric | Task ID | Verified |
|--------|--------|---------|----------|
| FAISS k=10 search | <5ms (nprobe=128, 10M vectors) | M04-T10, M04-T25 | [ ] |
| FAISS k=100 search | <10ms (nprobe=128, 10M vectors) | M04-T10, M04-T18, M04-T25 | [ ] |
| Poincare distance (CPU) | <10us (single pair) | M04-T05, M04-T25 | [ ] |
| Poincare distance (GPU) | <1ms (1K x 1K batch) | M04-T23, M04-T25, M04-T29 | [ ] |
| Cone containment (CPU) | <50us (single check) | M04-T07, M04-T25 | [ ] |
| Cone containment (GPU) | <2ms (1K x 1K batch) | M04-T24, M04-T25, M04-T29 | [ ] |
| BFS depth=6 | <100ms (10M nodes) | M04-T16, M04-T25 | [ ] |
| Domain-aware search | <10ms (k=10, 10M vectors) | M04-T19, M04-T25 | [ ] |
| Entailment query | <1ms (per cone check) | M04-T20, M04-T25 | [ ] |

---

### Memory Budget Targets

| Component | Budget | Task ID | Verified |
|-----------|--------|---------|----------|
| FAISS GPU index (10M vectors) | 8GB VRAM | M04-T10, M04-T28 | [ ] |
| Hyperbolic coordinates (10M nodes) | 2.5GB | M04-T04, M04-T13 | [ ] |
| Entailment cones (10M nodes) | 2.7GB | M04-T06, M04-T13 | [ ] |
| RocksDB cache | 8GB RAM | M04-T13 | [ ] |
| Total VRAM (RTX 5090) | 24GB | M04-T28, M04-T29 | [ ] |

---

### Error States

| Error | Condition | Task ID | Verified |
|-------|-----------|---------|----------|
| FaissIndexCreation | Index factory fails | M04-T08, M04-T09, M04-T10 | [ ] |
| FaissTrainingFailed | Training with insufficient data | M04-T08, M04-T10 | [ ] |
| FaissSearchFailed | Search on untrained index | M04-T08, M04-T10, M04-T18 | [ ] |
| FaissAddFailed | Add vectors to invalid index | M04-T08, M04-T10 | [ ] |
| IndexNotTrained | Operation requires trained index | M04-T08, M04-T10 | [ ] |
| InsufficientTrainingData | provided < min_train_vectors | M04-T08, M04-T10 | [ ] |
| GpuResourceAllocation | GPU memory allocation fails | M04-T08, M04-T09, M04-T28 | [ ] |
| GpuTransferFailed | CPU to GPU transfer fails | M04-T08, M04-T10 | [ ] |
| StorageOpen | RocksDB open fails | M04-T08, M04-T13 | [ ] |
| Storage | General RocksDB error | M04-T08, M04-T08a, M04-T13 | [ ] |
| ColumnFamilyNotFound | Missing CF | M04-T08, M04-T12, M04-T13 | [ ] |
| CorruptedData | Deserialization fails | M04-T08, M04-T13 | [ ] |
| VectorIdMismatch | ID collision or missing | M04-T08, M04-T10 | [ ] |
| InvalidConfig | Configuration validation fails | M04-T08, M04-T01, M04-T02a | [ ] |
| NodeNotFound | Node lookup fails | M04-T08, M04-T13, M04-T16 | [ ] |
| EdgeNotFound | Edge lookup fails | M04-T08, M04-T13, M04-T15 | [ ] |
| InvalidHyperbolicPoint | norm >= 1.0 | M04-T08, M04-T04, M04-T05 | [ ] |

---

### Edge Cases

| Scenario | Expected Behavior | Task ID | Verified |
|----------|-------------------|---------|----------|
| Point at origin (norm=0) | All operations valid | M04-T04, M04-T05 | [ ] |
| Point near boundary (norm > 0.999) | project() rescales to max_norm | M04-T04, M04-T05 | [ ] |
| Degenerate cone (aperture=0) | contains() returns false for all except apex | M04-T06, M04-T07 | [ ] |
| Apex at origin | Special handling for direction | M04-T07, M04-T27 | [ ] |
| Point equals apex | contains() returns true | M04-T07 | [ ] |
| Cyclic graph traversal | visited set prevents infinite loops | M04-T16, M04-T17 | [ ] |
| Empty search results | Return empty Vec, no error | M04-T11, M04-T18 | [ ] |
| Untrained index search | Return empty or error per config | M04-T10, M04-T18 | [ ] |
| FAISS -1 sentinel IDs | Filter from results | M04-T11 | [ ] |
| max_depth = 0 | Return only start node | M04-T16, M04-T17 | [ ] |
| max_nodes = 0 | Return empty result | M04-T16, M04-T17 | [ ] |
| All NT weights = 0 | modulation factor = 0 | M04-T14, M04-T22 | [ ] |
| Conflicting aperture formulas | Use canonical formula from T27 | M04-T07, M04-T27 | [ ] |
| GPU memory exhausted | Graceful error via GpuResourceAllocation | M04-T28 | [ ] |
| Deep stack in DFS | Iterative (not recursive) prevents overflow | M04-T17 | [ ] |
| A* heuristic inadmissible | Hyperbolic heuristic must be admissible | M04-T17a | [ ] |

---

## PRD Alignment Issues (From Analysis Agents)

These issues were identified during analysis and require resolution:

| Issue | Severity | Source | Resolution Task | Status |
|-------|----------|--------|-----------------|--------|
| `context-graph-graph` crate does not exist | CRITICAL | Foundation Analysis | M04-T00 | Pending |
| Vector1536 type not re-exported from core | HIGH | Foundation Analysis | M04-T01a | Pending |
| EntailmentCone.axis field mismatch between specs | MEDIUM | Analysis | M04-T06 clarifies fields | Pending |
| NT formula conflicts (3 different formulas found) | HIGH | Surface Analysis | M04-T27 | Pending |
| EdgeType CONTRADICTS missing | HIGH | Surface Analysis | M04-T26 | Pending |
| VRAM budget discrepancy (20GB vs 24GB) | LOW | Surface Analysis | M04-T28 | Pending |
| Curvature validation not enforced | MEDIUM | Foundation Analysis | M04-T02a | Pending |
| NT weight range [0,1] not enforced | MEDIUM | Logic Analysis | M04-T14a | Pending |
| Storage schema migration missing | MEDIUM | Logic Analysis | M04-T13a | Pending |

---

## Uncovered Items

<!-- List any items without task coverage - these MUST be addressed -->

| Item | Type | Reason | Action Required |
|------|------|--------|-----------------|
| (none) | - | All items covered | - |

**Note:** All 8 new tasks (M04-T00, T01a, T02a, T08a, T13a, T14a, T17a, T26, T27, T28, T29) were added to address gaps identified during analysis. Coverage is now 100%.

---

## Coverage Summary

| Category | Covered | Total | Percentage |
|----------|---------|-------|------------|
| Data Models / Entities | 30 | 30 | 100% |
| Requirements (REQ-KG-*) | 25 | 25 | 100% |
| Spec Sections | 16 | 16 | 100% |
| Performance Targets | 9 | 9 | 100% |
| Memory Budget Targets | 5 | 5 | 100% |
| Error States | 17 | 17 | 100% |
| Edge Cases | 16 | 16 | 100% |
| PRD Alignment Issues | 9 | 9 | 100% (tasks assigned) |

**TOTAL COVERAGE: 100%**

---

## Task to Requirement Quick Reference

This section provides a reverse mapping for implementers:

### M04-T00: Create Crate Structure
- **Implements:** Crate existence (CRITICAL blocker)
- **Required For:** All other tasks

### M04-T01: IndexConfig for FAISS IVF-PQ
- **Implements:** REQ-KG-001, REQ-KG-002, REQ-KG-003, REQ-KG-004, REQ-KG-005, REQ-KG-006
- **Spec Ref:** TECH-GRAPH-004 Section 2

### M04-T01a: Re-export Vector1536
- **Implements:** Vector type availability for graph module
- **Spec Ref:** TECH-GRAPH-004 Section 2

### M04-T02: HyperbolicConfig
- **Implements:** REQ-KG-050, REQ-KG-054
- **Spec Ref:** TECH-GRAPH-004 Section 5

### M04-T02a: Curvature Validation
- **Implements:** REQ-KG-054 (curvature < 0 enforcement)
- **Spec Ref:** TECH-GRAPH-004 Section 5

### M04-T03: ConeConfig
- **Implements:** REQ-KG-052
- **Spec Ref:** TECH-GRAPH-004 Section 6

### M04-T04: PoincarePoint
- **Implements:** REQ-KG-050, REQ-KG-054
- **Spec Ref:** TECH-GRAPH-004 Section 5.1

### M04-T05: PoincareBall Mobius Operations
- **Implements:** REQ-KG-051
- **Spec Ref:** TECH-GRAPH-004 Section 5.2
- **Performance:** <10us per distance computation

### M04-T06: EntailmentCone Struct
- **Implements:** REQ-KG-052, REQ-KG-053
- **Spec Ref:** TECH-GRAPH-004 Section 6

### M04-T07: Containment Logic
- **Implements:** REQ-KG-053
- **Spec Ref:** TECH-GRAPH-004 Section 6
- **Performance:** <50us per containment check

### M04-T08: GraphError Enum
- **Implements:** Error handling for all operations
- **Spec Ref:** TECH-GRAPH-004 Section 9

### M04-T08a: Error Conversions
- **Implements:** From traits for RocksDB and other errors
- **Spec Ref:** TECH-GRAPH-004 Section 9

### M04-T09: FAISS FFI Bindings
- **Implements:** REQ-KG-001 (partial)
- **Spec Ref:** TECH-GRAPH-004 Section 3.1

### M04-T10: FaissGpuIndex Wrapper
- **Implements:** REQ-KG-001 through REQ-KG-008
- **Spec Ref:** TECH-GRAPH-004 Section 3.2
- **Performance:** <5ms for k=10 search on 10M vectors

### M04-T11: SearchResult Struct
- **Implements:** REQ-KG-060 (support)
- **Spec Ref:** TECH-GRAPH-004 Section 3.2

### M04-T12: Column Families
- **Implements:** Storage infrastructure
- **Spec Ref:** TECH-GRAPH-004 Section 4

### M04-T13: GraphStorage Backend
- **Implements:** Storage for all graph data
- **Spec Ref:** TECH-GRAPH-004 Section 4.2

### M04-T13a: Storage Migrations
- **Implements:** Schema versioning and upgrades
- **Spec Ref:** TECH-GRAPH-004 Section 4.2

### M04-T14: NeurotransmitterWeights
- **Implements:** REQ-KG-065
- **Spec Ref:** TECH-GRAPH-004 Section 4.1

### M04-T14a: NT Weight Validation
- **Implements:** REQ-KG-065 (range enforcement)
- **Spec Ref:** TECH-GRAPH-004 Section 4.1

### M04-T15: GraphEdge with Marblestone
- **Implements:** REQ-KG-040 through REQ-KG-044, REQ-KG-065
- **Spec Ref:** TECH-GRAPH-004 Section 4.1

### M04-T16: BFS Graph Traversal
- **Implements:** REQ-KG-061
- **Spec Ref:** TECH-GRAPH-004 Section 7.1
- **Performance:** <100ms for depth=6 on 10M node graph

### M04-T17: DFS Graph Traversal
- **Implements:** REQ-KG-061
- **Spec Ref:** TECH-GRAPH-004 Section 7.2

### M04-T17a: A* Hyperbolic Traversal
- **Implements:** REQ-KG-061 (advanced)
- **Spec Ref:** TECH-GRAPH-004 Section 7 (extension)

### M04-T18: Semantic Search
- **Implements:** REQ-KG-060, REQ-KG-064
- **Spec Ref:** TECH-GRAPH-004 Section 8
- **Performance:** <10ms for k=100 on 10M vectors

### M04-T19: Domain-Aware Search (Marblestone)
- **Implements:** REQ-KG-065
- **Spec Ref:** TECH-GRAPH-004 Section 8
- **Performance:** <10ms for k=10 on 10M vectors

### M04-T20: Entailment Query
- **Implements:** REQ-KG-062
- **Spec Ref:** TECH-GRAPH-004 Section 8
- **Performance:** <1ms per cone check

### M04-T21: Contradiction Detection
- **Implements:** REQ-KG-063
- **Spec Ref:** TECH-GRAPH-004 Section 8

### M04-T22: get_modulated_weight
- **Implements:** REQ-KG-065
- **Spec Ref:** TECH-GRAPH-004 Section 8

### M04-T23: Poincare Distance CUDA Kernel
- **Implements:** GPU acceleration for hyperbolic distance
- **Spec Ref:** TECH-GRAPH-004 Section 10.1
- **Performance:** <1ms for 1K x 1K distance matrix

### M04-T24: Cone Membership CUDA Kernel
- **Implements:** GPU acceleration for cone containment
- **Spec Ref:** TECH-GRAPH-004 Section 10.2
- **Performance:** <2ms for 1K x 1K membership matrix

### M04-T25: Integration Tests
- **Implements:** REQ-KG-TEST
- **Spec Ref:** TECH-GRAPH-004 Section 11

### M04-T26: EdgeType::CONTRADICTS
- **Implements:** REQ-KG-063 (edge type support)
- **Spec Ref:** TECH-GRAPH-004 Section 4.1

### M04-T27: Fix Formula Conflicts
- **Implements:** Canonical containment formula
- **Spec Ref:** TECH-GRAPH-004 Section 6

### M04-T28: GPU Memory Manager
- **Implements:** VRAM budget tracking
- **Spec Ref:** TECH-GRAPH-004 Section 10

### M04-T29: Benchmark Suite
- **Implements:** All NFR-KG performance requirements
- **Spec Ref:** TECH-GRAPH-004 Section 11

---

## Validation Checklist

Pre-execution validation:

- [x] All data models have tasks
- [x] All DTOs/interfaces have tasks
- [x] All service methods have tasks
- [x] All API endpoints have tasks
- [x] All error states handled in tasks
- [x] All edge cases covered in tasks
- [x] Task dependencies form valid DAG (no cycles)
- [x] Layer ordering correct (foundation -> logic -> surface)
- [x] No item appears without a task assignment
- [x] All REQ-KG-* requirements mapped
- [x] All TECH-GRAPH-004 sections covered
- [x] All performance targets have verification tasks
- [x] All memory budgets have tracking tasks
- [x] PRD alignment issues have resolution tasks

Post-execution validation (per task):

- [ ] All files in `files_to_create` exist
- [ ] All files in `files_to_modify` are updated
- [ ] All `signatures` in definition_of_done are exact matches
- [ ] All `constraints` are satisfied
- [ ] All `verification` steps pass
- [ ] All `test_commands` succeed
- [ ] All `validation_criteria` are met

---

## Dependency-Ordered Task List for Implementers

Execute tasks in this order to ensure all dependencies are satisfied:

### Phase 1: Crate Bootstrap (Week 1)
1. **M04-T00** - Create crate structure (CRITICAL)
2. **M04-T14** - NeurotransmitterWeights (no deps, can parallel with T00)

### Phase 2: Foundation Types (Week 1-2)
3. **M04-T01** - IndexConfig
4. **M04-T02** - HyperbolicConfig
5. **M04-T03** - ConeConfig
6. **M04-T08** - GraphError
7. **M04-T01a** - Vector1536 re-export
8. **M04-T02a** - Curvature validation
9. **M04-T04** - PoincarePoint
10. **M04-T08a** - Error conversions
11. **M04-T05** - PoincareBall Mobius
12. **M04-T06** - EntailmentCone struct
13. **M04-T07** - Containment logic

### Phase 3: Logic Layer (Week 2-3)
14. **M04-T09** - FAISS FFI
15. **M04-T12** - Column families
16. **M04-T14a** - NT weight validation
17. **M04-T10** - FaissGpuIndex
18. **M04-T15** - GraphEdge
19. **M04-T11** - SearchResult
20. **M04-T13** - GraphStorage
21. **M04-T13a** - Storage migrations
22. **M04-T16** - BFS traversal
23. **M04-T17** - DFS traversal
24. **M04-T17a** - A* traversal

### Phase 4: Surface Layer (Week 3-4)
25. **M04-T18** - Semantic search
26. **M04-T19** - Domain-aware search
27. **M04-T20** - Entailment query
28. **M04-T21** - Contradiction detection
29. **M04-T22** - get_modulated_weight
30. **M04-T26** - EdgeType::CONTRADICTS
31. **M04-T27** - Fix formula conflicts
32. **M04-T23** - Poincare CUDA kernel
33. **M04-T24** - Cone CUDA kernel
34. **M04-T28** - GPU memory manager

### Phase 5: Integration (Week 4-5)
35. **M04-T25** - Integration tests
36. **M04-T29** - Benchmark suite

---

*Generated: 2026-01-02*
*Module: 04 - Knowledge Graph*
*Version: 1.1.0*
*Total Tasks: 33*
*Coverage: 100%*
