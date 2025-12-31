# Task Traceability Matrix: Module 02 - Core Infrastructure

## Purpose

This matrix ensures every requirement, component, and behavior from the Module 02 specification is covered by at least one atomic task. **Empty "Task ID" columns = INCOMPLETE.**

## Coverage Matrix

### Core Enumerations

| Item | Description | Task ID | File Path | Verified |
|------|-------------|---------|-----------|----------|
| JohariQuadrant | Four quadrant enum (Open, Hidden, Blind, Unknown) | TASK-M02-001 | `crates/context-graph-core/src/types/johari.rs` | ✅ |
| Modality | Content type classification enum | TASK-M02-002 | `crates/context-graph-core/src/types/johari.rs` | ✅ |
| ValidationError | Node validation error types | TASK-M02-004 | `crates/context-graph-core/src/types/memory_node.rs` | ✅ |
| Domain | Marblestone context domain enum | TASK-M02-007 | `crates/context-graph-core/src/marblestone.rs` | ☐ |
| EdgeType | Graph edge relationship types | TASK-M02-009 | `crates/context-graph-core/src/marblestone.rs` | ☐ |
| EmotionalState | UTL emotional state enum | TASK-M02-019 | `crates/context-graph-core/src/pulse.rs` | ☐ |
| SuggestedAction | Cognitive pulse action enum | TASK-M02-020 | `crates/context-graph-core/src/pulse.rs` | ☐ |
| StorageError | Storage operation error types | TASK-M02-025 | `crates/context-graph-storage/src/lib.rs` | ☐ |
| TransitionTrigger | Johari transition trigger types | TASK-M02-012 | `crates/context-graph-core/src/johari.rs` | ☐ |

### Core Structs

| Item | Description | Task ID | File Path | Verified |
|------|-------------|---------|-----------|----------|
| NodeMetadata | Memory node metadata container | TASK-M02-003 | `crates/context-graph-core/src/types/memory_node.rs` | ✅ |
| MemoryNode | Core knowledge node struct | TASK-M02-005 | `crates/context-graph-core/src/types/memory_node.rs` | ☐ |
| NeurotransmitterWeights | Marblestone NT weight struct | TASK-M02-008 | `crates/context-graph-core/src/marblestone.rs` | ☐ |
| GraphEdge | Graph edge with Marblestone fields | TASK-M02-010 | `crates/context-graph-core/src/marblestone.rs` | ☐ |
| CognitivePulse | System cognitive state struct | TASK-M02-021 | `crates/context-graph-core/src/pulse.rs` | ☐ |
| JohariTransition | Quadrant transition record | TASK-M02-012 | `crates/context-graph-core/src/johari.rs` | ☐ |

### Methods / Business Logic

| Item | Description | Task ID | Verified |
|------|-------------|---------|----------|
| JohariQuadrant::is_self_aware() | Check if quadrant is self-aware | TASK-M02-001 | ☐ |
| JohariQuadrant::is_other_aware() | Check if quadrant is other-aware | TASK-M02-001 | ☐ |
| JohariQuadrant::default_retrieval_weight() | Get default retrieval weight | TASK-M02-001 | ☐ |
| JohariQuadrant::column_family() | Get RocksDB column family name | TASK-M02-001 | ☐ |
| Modality::detect() | Auto-detect content modality | TASK-M02-002 | ☐ |
| Modality::file_extensions() | Get file extensions per modality | TASK-M02-002 | ☐ |
| NodeMetadata::add_tag() | Add tag to metadata | TASK-M02-003 | ✅ |
| NodeMetadata::remove_tag() | Remove tag from metadata | TASK-M02-003 | ✅ |
| NodeMetadata::mark_consolidated() | Mark node as consolidated | TASK-M02-003 | ✅ |
| NodeMetadata::mark_deleted() | Soft delete node | TASK-M02-003 | ✅ |
| MemoryNode::new() | Create new memory node | TASK-M02-006 | ☐ |
| MemoryNode::record_access() | Record node access | TASK-M02-006 | ☐ |
| MemoryNode::compute_decay() | Compute Ebbinghaus decay | TASK-M02-006 | ☐ |
| MemoryNode::should_consolidate() | Check consolidation threshold | TASK-M02-006 | ☐ |
| MemoryNode::validate() | Validate node constraints | TASK-M02-006 | ☐ |
| NeurotransmitterWeights::for_domain() | Get domain-specific NT weights | TASK-M02-008 | ☐ |
| NeurotransmitterWeights::compute_effective_weight() | Calculate effective weight | TASK-M02-008 | ☐ |
| GraphEdge::new() | Create new graph edge | TASK-M02-011 | ☐ |
| GraphEdge::get_modulated_weight() | Get NT-modulated weight | TASK-M02-011 | ☐ |
| GraphEdge::apply_steering_reward() | Apply steering reward | TASK-M02-011 | ☐ |
| GraphEdge::record_traversal() | Record edge traversal | TASK-M02-011 | ☐ |
| GraphEdge::is_reliable_shortcut() | Check if reliable amortized shortcut | TASK-M02-011 | ☐ |
| JohariQuadrant::valid_transitions() | Get valid transitions from quadrant | TASK-M02-012 | ☐ |
| CognitivePulse::new() | Create new cognitive pulse | TASK-M02-022 | ☐ |
| CognitivePulse::compute_suggested_action() | Compute suggested action | TASK-M02-022 | ☐ |
| CognitivePulse::update() | Update pulse metrics | TASK-M02-022 | ☐ |
| CognitivePulse::blend() | Blend two pulses | TASK-M02-022 | ☐ |
| EmotionalState::weight_modifier() | Get UTL weight modifier | TASK-M02-019 | ☐ |

### Storage Operations

| Item | Description | Task ID | Verified |
|------|-------------|---------|----------|
| serialize_node() | Serialize MemoryNode to bytes | TASK-M02-014 | ☐ |
| deserialize_node() | Deserialize bytes to MemoryNode | TASK-M02-014 | ☐ |
| serialize_edge() | Serialize GraphEdge to bytes | TASK-M02-014 | ☐ |
| deserialize_edge() | Deserialize bytes to GraphEdge | TASK-M02-014 | ☐ |
| serialize_embedding() | Serialize embedding vector | TASK-M02-014 | ☐ |
| deserialize_embedding() | Deserialize embedding vector | TASK-M02-014 | ☐ |
| get_column_family_descriptors() | Get all CF descriptors | TASK-M02-015 | ☐ |
| RocksDbMemex::open() | Open database connection | TASK-M02-016 | ☐ |
| RocksDbMemex::store_node() | Store memory node | TASK-M02-017 | ☐ |
| RocksDbMemex::get_node() | Retrieve memory node | TASK-M02-017 | ☐ |
| RocksDbMemex::update_node() | Update existing node | TASK-M02-017 | ☐ |
| RocksDbMemex::delete_node() | Delete node (soft) | TASK-M02-017 | ☐ |
| RocksDbMemex::store_edge() | Store graph edge | TASK-M02-018 | ☐ |
| RocksDbMemex::get_edge() | Retrieve graph edge | TASK-M02-018 | ☐ |
| RocksDbMemex::get_edges_from() | Get outgoing edges | TASK-M02-018 | ☐ |
| RocksDbMemex::get_edges_to() | Get incoming edges | TASK-M02-018 | ☐ |
| get_nodes_by_quadrant() | Query nodes by Johari quadrant | TASK-M02-023 | ☐ |
| get_nodes_by_tag() | Query nodes by tag | TASK-M02-023 | ☐ |
| get_nodes_in_time_range() | Query nodes by time range | TASK-M02-023 | ☐ |
| store_embedding() | Store embedding vector | TASK-M02-024 | ☐ |
| get_embedding() | Retrieve embedding vector | TASK-M02-024 | ☐ |
| batch_get_embeddings() | Batch retrieve embeddings | TASK-M02-024 | ☐ |

### Column Families (RocksDB)

| Column Family | Purpose | Task ID | Verified |
|---------------|---------|---------|----------|
| nodes | Primary node storage | TASK-M02-015 | ☐ |
| edges | Graph edge storage | TASK-M02-015 | ☐ |
| embeddings | Embedding vector storage | TASK-M02-015 | ☐ |
| metadata | Node metadata storage | TASK-M02-015 | ☐ |
| johari_open | Open quadrant index | TASK-M02-015 | ☐ |
| johari_hidden | Hidden quadrant index | TASK-M02-015 | ☐ |
| johari_blind | Blind quadrant index | TASK-M02-015 | ☐ |
| johari_unknown | Unknown quadrant index | TASK-M02-015 | ☐ |
| temporal | Temporal index | TASK-M02-015 | ☐ |
| tags | Tag index | TASK-M02-015 | ☐ |
| sources | Source index | TASK-M02-015 | ☐ |
| system | System metadata | TASK-M02-015 | ☐ |

### Traits / Interfaces

| Item | Description | Task ID | Verified |
|------|-------------|---------|----------|
| Memex | Storage abstraction trait | TASK-M02-026 | ☐ |
| Default for JohariQuadrant | Default implementation | TASK-M02-001 | ☐ |
| Default for Modality | Default implementation | TASK-M02-002 | ☐ |
| Default for MemoryNode | Default implementation | TASK-M02-006 | ☐ |
| Default for NeurotransmitterWeights | Default implementation | TASK-M02-008 | ☐ |
| Default for Domain | Default implementation | TASK-M02-007 | ☐ |
| Default for EmotionalState | Default implementation | TASK-M02-019 | ☐ |
| Default for SuggestedAction | Default implementation | TASK-M02-020 | ☐ |
| Default for CognitivePulse | Default implementation | TASK-M02-022 | ☐ |

### Validation Rules

| Rule | Constraint | Task ID | Verified |
|------|------------|---------|----------|
| Embedding dimension | Must be 1536 | TASK-M02-006 | ☐ |
| Importance range | [0.0, 1.0] | TASK-M02-006 | ☐ |
| Emotional valence range | [-1.0, 1.0] | TASK-M02-006 | ☐ |
| Content size limit | ≤1MB | TASK-M02-006 | ☐ |
| Embedding normalization | Must be normalized | TASK-M02-006 | ☐ |
| NT weights range | [0.0, 1.0] each | TASK-M02-008 | ☐ |
| Steering reward range | [-1.0, 1.0] | TASK-M02-011 | ☐ |
| Pulse metrics range | [0.0, 1.0] | TASK-M02-022 | ☐ |

### Marblestone Integration

| Feature | Description | Task ID | Verified |
|---------|-------------|---------|----------|
| Domain-aware NT weights | Context-specific neurotransmitter profiles | TASK-M02-008 | ☐ |
| Excitatory/Inhibitory/Modulatory | Three-weight NT system | TASK-M02-008 | ☐ |
| Amortized shortcuts | Learned shortcut edges | TASK-M02-010, TASK-M02-011 | ☐ |
| Steering reward | [-1,1] reward signal | TASK-M02-010, TASK-M02-011 | ☐ |
| Modulated weight calculation | NT-adjusted edge weights | TASK-M02-011 | ☐ |
| Domain enum | Code/Legal/Medical/Creative/Research/General | TASK-M02-007 | ☐ |

### Error States

| Error | Condition | Task ID | Verified |
|-------|-----------|---------|----------|
| InvalidEmbeddingDimension | Embedding ≠ 1536 dimensions | TASK-M02-004 | ✅ |
| OutOfBounds | Importance/valence outside valid range | TASK-M02-004 | ✅ |
| ContentTooLarge | Content exceeds 1MB limit | TASK-M02-004 | ✅ |
| EmbeddingNotNormalized | Embedding vector not normalized | TASK-M02-004 | ✅ |
| OpenFailed | Database open failure | TASK-M02-025 | ☐ |
| ColumnFamilyNotFound | Missing column family | TASK-M02-025 | ☐ |
| SerializationFailed | Serialization error | TASK-M02-025 | ☐ |
| DeserializationFailed | Deserialization error | TASK-M02-025 | ☐ |
| NotFound | Entity not found | TASK-M02-025 | ☐ |
| WriteFailed | Write operation failure | TASK-M02-025 | ☐ |
| ReadFailed | Read operation failure | TASK-M02-025 | ☐ |
| IndexCorrupted | Index corruption detected | TASK-M02-025 | ☐ |

### Test Coverage

| Test Category | Description | Task ID | Verified |
|---------------|-------------|---------|----------|
| JohariQuadrant unit tests | All enum methods | TASK-M02-001 | ☐ |
| Modality unit tests | Detection and extensions | TASK-M02-002 | ☐ |
| NodeMetadata unit tests | Tag and lifecycle operations | TASK-M02-003 | ✅ |
| ValidationError unit tests | Error display and context | TASK-M02-004 | ✅ |
| MemoryNode unit tests | All struct methods | TASK-M02-006 | ☐ |
| NeurotransmitterWeights unit tests | Domain profiles and calculations | TASK-M02-008 | ☐ |
| GraphEdge unit tests | Edge operations and Marblestone | TASK-M02-011 | ☐ |
| Johari transition tests | Valid transition logic | TASK-M02-012 | ☐ |
| Serialization tests | Round-trip serialization | TASK-M02-014 | ☐ |
| Column family tests | CF creation and options | TASK-M02-015 | ☐ |
| RocksDB backend tests | Open/close lifecycle | TASK-M02-016 | ☐ |
| Node CRUD tests | Store/get/update/delete | TASK-M02-017 | ☐ |
| Edge CRUD tests | Edge operations | TASK-M02-018 | ☐ |
| Pulse unit tests | All pulse methods | TASK-M02-022 | ☐ |
| Index operation tests | Secondary index queries | TASK-M02-023 | ☐ |
| Embedding storage tests | Vector operations | TASK-M02-024 | ☐ |
| Memex trait tests | Trait implementation | TASK-M02-026 | ☐ |
| Integration tests | End-to-end workflows | TASK-M02-027 | ☐ |
| Doc tests | Example code in documentation | TASK-M02-028 | ☐ |

### Documentation

| Item | Description | Task ID | Verified |
|------|-------------|---------|----------|
| Public API doc comments | /// comments on all public items | TASK-M02-028 | ☐ |
| basic_storage.rs example | Simple node creation/retrieval | TASK-M02-028 | ☐ |
| marblestone_edges.rs example | Edge creation with NT weights | TASK-M02-028 | ☐ |
| cognitive_pulse.rs example | Pulse generation and interpretation | TASK-M02-028 | ☐ |
| Crate README files | Purpose documentation per crate | TASK-M02-028 | ☐ |

## Uncovered Items

<!-- List any items without task coverage - these MUST be addressed -->

| Item | Type | Reason | Action Required |
|------|------|--------|-----------------|
| (none) | — | — | — |

## Coverage Summary

- **Enumerations:** 9/9 covered (100%)
- **Structs:** 6/6 covered (100%)
- **Methods:** 28/28 covered (100%)
- **Storage Operations:** 22/22 covered (100%)
- **Column Families:** 12/12 covered (100%)
- **Traits:** 9/9 covered (100%)
- **Validation Rules:** 8/8 covered (100%)
- **Marblestone Features:** 6/6 covered (100%)
- **Error States:** 12/12 covered (100%)
- **Test Categories:** 19/19 covered (100%)
- **Documentation:** 5/5 covered (100%)

**TOTAL COVERAGE: 100%** ✅

## Validation Checklist

- [x] All data models have tasks
- [x] All enums have tasks
- [x] All structs have tasks
- [x] All methods have tasks
- [x] All storage operations have tasks
- [x] All column families have tasks
- [x] All traits have tasks
- [x] All validation rules covered in tasks
- [x] All Marblestone features have tasks
- [x] All error states handled in tasks
- [x] All test categories have tasks
- [x] All documentation items have tasks
- [x] Task dependencies form valid DAG (no cycles)
- [x] Layer ordering correct (foundation → logic → surface)
- [x] No item appears without a task assignment

## Spec References

| Spec ID | Description | Tasks Implementing |
|---------|-------------|--------------------|
| TECH-CORE-002 Section 2.1 | MemoryNode specification | TASK-M02-004, TASK-M02-005, TASK-M02-006 |
| TECH-CORE-002 Section 2.2 | JohariQuadrant specification | TASK-M02-001, TASK-M02-012 |
| TECH-CORE-002 Section 2.3 | Metadata specification | TASK-M02-002, TASK-M02-003 |
| TECH-CORE-002 Section 2.4 | CognitivePulse specification | TASK-M02-019, TASK-M02-020, TASK-M02-021, TASK-M02-022 |
| TECH-CORE-002 Section 2.5 | Marblestone specification | TASK-M02-007, TASK-M02-008, TASK-M02-009, TASK-M02-010, TASK-M02-011 |
| TECH-CORE-002 Section 3 | Storage specification | TASK-M02-013 through TASK-M02-026 |
| TECH-CORE-002 Section 3.1 | Column families specification | TASK-M02-015 |
| TECH-CORE-002 Section 3.2 | RocksDB backend specification | TASK-M02-016, TASK-M02-017, TASK-M02-018 |
| TECH-CORE-002 Section 4 | Testing specification | TASK-M02-027 |
| REQ-CORE-001 | Johari quadrant requirements | TASK-M02-001 |
| REQ-CORE-002 | MemoryNode requirements | TASK-M02-005 |
| REQ-CORE-003 | Metadata requirements | TASK-M02-003 |
| REQ-CORE-004 | Memory decay requirements | TASK-M02-006 |
| REQ-CORE-005 | Storage requirements | TASK-M02-015 |
| REQ-CORE-006 | CRUD requirements | TASK-M02-017 |
| REQ-CORE-007 | CognitivePulse requirements | TASK-M02-021 |
| REQ-CORE-008 | Pulse action requirements | TASK-M02-022 |
| Marblestone Integration Spec | Full Marblestone architecture | TASK-M02-007, TASK-M02-008, TASK-M02-010, TASK-M02-011, TASK-M02-018 |

---

*Generated: 2025-12-31*
*Module: 02 - Core Infrastructure*
*Framework: Atomic Task Traceability Matrix v1.0*
