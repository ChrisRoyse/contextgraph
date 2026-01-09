# Task Breakdown - Teleological Array System Refactoring

## Overview

This document provides a phased implementation plan for refactoring the system to use teleological arrays (13-embedder arrays) as the fundamental storage and comparison unit. The refactor includes removing the broken manual North Star system, implementing autonomous goal discovery, integrating with Claude Code hooks for self-learning, and adding entry-point discovery search.

**Target Architecture Flow:**
```
Memory Injection (MCP) → Autonomous Embedding (13 models) → Teleological Array Storage
    → Entry-Point Discovery (any of 13 spaces) → Full Array Comparison (apples to apples)
    → Autonomous Goal Emergence (clustering)
```

**Total Estimated Complexity**: High
**Estimated Timeline**: 10-14 weeks
**Primary Crates Affected**:
- `context-graph-mcp` - MCP handlers, tools, and hooks integration
- `context-graph-core` - Core types, alignment, and autonomous subsystems
- `context-graph-storage` - Storage layer and indexing
- `context-graph-embeddings` - Embedding generation

---

## Dependency Graph

```
Phase 1 (North Star Removal)
    │
    v
Phase 2 (Teleological Array Storage)
    │
    ├───────────────────┬───────────────────┐
    v                   v                   v
Phase 3 (Search)    Phase 4 (Hooks)    Phase 7 (Claude Code Hook Implementation)
    │                   │                   │
    │                   │                   v
    │                   │              Phase 8 (Skills Development)
    │                   │                   │
    │                   │                   v
    │                   │              Phase 9 (Subagent Creation)
    │                   │                   │
    └─────────┬─────────┴───────────────────┘
              v
Phase 5 (Self-Learning Feedback)
              │
              v
Phase 6 (Testing and Validation)
              │
              v
Phase 10 (Integration Testing - Hooks/Skills/Subagents)
```

---

## Phase 1: Remove Manual North Star System

**Goal**: Eliminate the broken manual North Star system that uses single-embedding goals which cannot be meaningfully compared to 13-embedder teleological arrays.

**Duration**: 1-2 weeks
**Dependencies**: None (can start immediately)
**Risk Level**: Medium (breaking changes to MCP API)

### Task 1.1: Remove Protocol Constants

**File**: `crates/context-graph-mcp/src/protocol.rs`

**Work**:
- Remove `PURPOSE_NORTH_STAR_ALIGNMENT` constant
- Remove `NORTH_STAR_UPDATE` constant
- Keep `PURPOSE_DRIFT_CHECK` (will be refactored in Phase 5)
- Add `DEPRECATED_METHOD` error code for graceful transition

**Acceptance Criteria**:
- Protocol constants no longer exist
- Code compiles without these constants

### Task 1.2: Remove Dispatch Routes

**File**: `crates/context-graph-mcp/src/handlers/core.rs`

**Work**:
- Remove dispatch for `PURPOSE_NORTH_STAR_ALIGNMENT`
- Remove dispatch for `NORTH_STAR_UPDATE`
- Add temporary deprecation error responses during transition period
- Log deprecation warnings for any calls to removed endpoints

**Acceptance Criteria**:
- Dispatch routes removed
- Deprecation errors returned for old endpoints
- Logs capture any legacy usage

### Task 1.3: Remove Handler Implementations

**File**: `crates/context-graph-mcp/src/handlers/purpose.rs`

**Work**:
- Remove `handle_north_star_alignment` function
- Remove `handle_north_star_update` function
- Remove associated helper functions
- Keep `handle_drift_check` but mark for refactor with TODO comment

**Acceptance Criteria**:
- Handler implementations removed
- No references to removed functions
- drift_check handler preserved with refactor marker

### Task 1.4: Remove Broken Projection Code

**File**: `crates/context-graph-core/src/alignment/calculator.rs`

**Work**:
- Remove `project_embedding` function
- Remove all uses of projection in alignment calculation
- Add placeholder for teleological array comparison (implemented in Phase 2)
- Remove dimension reduction logic

**Acceptance Criteria**:
- `project_embedding` function deleted
- No dimension projection in codebase
- `rg "project_embedding" --type rust` returns nothing

### Task 1.5: Update GoalNode Structure

**File**: `crates/context-graph-core/src/purpose.rs`

**Work**:
- Remove `embedding: Vec<f32>` field from `GoalNode`
- Add `teleological_array: Option<TeleologicalArray>` field
- Update `GoalNode` constructors to use teleological arrays
- Update serialization/deserialization
- Deprecate `north_star()` constructor, add `autonomous_goal()` constructor

**Acceptance Criteria**:
- GoalNode uses TeleologicalArray
- Old embedding field removed
- Serialization works correctly

### Task 1.6: Update Tests for Removed Functionality

**Files**:
- `crates/context-graph-mcp/src/handlers/tests/north_star.rs`
- `crates/context-graph-mcp/src/handlers/tests/purpose.rs`

**Work**:
- Delete `north_star.rs` test file entirely
- Remove north star tests from `purpose.rs`
- Add placeholder tests for new teleological array-based operations

**Acceptance Criteria**:
- All north star tests removed
- Test suite passes
- No dead test code

### Task 1.7: Documentation Update

**Work**:
- Update `contextprd.md` to remove north star manual creation references
- Add migration guide for users of deprecated endpoints
- Document why manual north star was removed (apples-to-oranges problem)

**Acceptance Criteria**:
- Documentation reflects new architecture
- Migration path documented

---

## Phase 2: Implement Teleological Array Storage

**Goal**: Implement storage layer that treats teleological arrays (13-embedder arrays) as atomic units, with per-embedder indexing for efficient search.

**Duration**: 2-3 weeks
**Dependencies**: Phase 1 (Tasks 1.4, 1.5)
**Risk Level**: Medium (core data model change)

### Task 2.1: Define TeleologicalArray Type

**File**: `crates/context-graph-core/src/teleology/array.rs` (NEW)

**Work**:
- Create `TeleologicalArray` struct with 13-entry fixed array
- Define `EmbedderOutput` enum (Dense, Sparse, TokenLevel)
- Implement `SparseVector` type for SPLADE embeddings
- Implement serialization/deserialization (MessagePack, Bincode)
- Add metadata fields (id, source_content, created_at)
- Implement `Default`, `Clone`, `Debug` traits

**Acceptance Criteria**:
- Type compiles and is usable
- Serialization round-trips correctly
- Memory layout is efficient

### Task 2.2: Define Embedder Enumeration

**File**: `crates/context-graph-core/src/teleology/embedder.rs` (NEW)

**Work**:
- Create `Embedder` enum with all 13 variants (Semantic through SpladeKeyword)
- Implement `expected_dims()` method returning `EmbedderDims`
- Implement `index()` method (0-12)
- Add iteration support (`Embedder::all()`)
- Implement `EmbedderMask` bitmask type for embedder selection
- Implement `EmbedderGroup` for predefined groupings

**Acceptance Criteria**:
- All 13 embedders defined
- Dimension lookup works
- Groups (Temporal, Lexical, etc.) work correctly

### Task 2.3: Define Comparison Types

**File**: `crates/context-graph-core/src/teleology/comparison.rs` (NEW)

**Work**:
- Create `ComparisonType` enum (SingleEmbedder, EmbedderGroup, WeightedFull, MatrixStrategy)
- Create `SearchMatrix` type (13x13 weights)
- Create `ComparisonResult` struct with per-embedder scores
- Implement predefined matrices (identity, semantic_focused, temporal_aware, etc.)
- Add matrix validation (weights sum to 1.0, non-negative diagonal)

**Acceptance Criteria**:
- All comparison types defined
- Predefined matrices work
- Validation catches invalid configurations

### Task 2.4: Implement Similarity Functions

**Files**:
- `crates/context-graph-core/src/teleology/similarity/dense.rs` (NEW)
- `crates/context-graph-core/src/teleology/similarity/sparse.rs` (NEW)
- `crates/context-graph-core/src/teleology/similarity/token_level.rs` (NEW)

**Work**:
- Implement `DenseSimilarity` (cosine, dot_product, euclidean)
- Implement SIMD-accelerated cosine for x86_64
- Implement `SparseSimilarity` (sparse dot, sparse cosine, Jaccard)
- Implement `TokenLevelSimilarity` (MaxSim, symmetric MaxSim)
- Add dimension validation in all functions

**Acceptance Criteria**:
- All similarity functions implemented
- SIMD version benchmarks 2-4x faster
- Correct results verified by tests

### Task 2.5: Implement TeleologicalComparator

**File**: `crates/context-graph-core/src/teleology/comparator.rs` (NEW)

**Work**:
- Create `TeleologicalComparator` struct
- Implement `compare()` method routing to correct similarity function
- Apply search matrices and weights correctly
- Return detailed per-embedder scores
- Implement `BatchComparator` for parallel comparisons

**Acceptance Criteria**:
- Comparator works with all ComparisonTypes
- Per-embedder scores are accurate
- Batch comparison uses Rayon parallelism

### Task 2.6: Storage Trait Definition

**File**: `crates/context-graph-storage/src/teleological/store.rs` (NEW)

**Work**:
- Define `TeleologicalArrayStore` async trait
- Methods: `store`, `store_batch`, `retrieve`, `retrieve_batch`, `delete`, `count`, `exists`, `stats`
- Define `IndexedTeleologicalStore` extended trait with search methods
- Define `SearchResult`, `SearchFilter` types
- Define `StorageStats`, `IndexStats` types

**Acceptance Criteria**:
- Trait compiles and is object-safe
- All methods have sensible signatures
- Error types defined

### Task 2.7: Per-Embedder Index Structure

**File**: `crates/context-graph-storage/src/teleological/index.rs` (NEW)

**Work**:
- Define `EmbedderIndexConfig` for per-embedder settings
- Define `IndexType` enum (HNSW, Inverted, TokenLevel)
- Define `HnswConfig` with m, ef_construction, ef_search
- Define `InvertedIndexConfig` for sparse embeddings
- Implement `EmbedderIndex` trait for polymorphic indices

**Acceptance Criteria**:
- Index types support all embedding formats
- Configuration is flexible
- Trait enables swappable implementations

### Task 2.8: RocksDB Implementation

**File**: `crates/context-graph-storage/src/teleological/rocksdb_store.rs` (NEW)

**Work**:
- Implement `TeleologicalArrayStore` for RocksDB
- Create column families: arrays, metadata, indices
- Implement batch operations with WriteBatch for atomicity
- Add transaction support for consistency
- Implement all 13 per-embedder indices
- Handle index updates on store/delete

**Acceptance Criteria**:
- Store/retrieve works correctly
- Batch operations are atomic
- Indices stay synchronized

### Task 2.9: Index Manager

**File**: `crates/context-graph-storage/src/teleological/index_manager.rs` (NEW)

**Work**:
- Create `IndexManager` to coordinate 13 indices
- Implement parallel indexing (add to all 13 simultaneously)
- Handle index updates and deletions
- Support incremental index builds
- Implement background index optimization

**Acceptance Criteria**:
- Manager coordinates all indices
- Parallel operations work correctly
- No deadlocks or race conditions

### Task 2.10: Module Integration

**Files**:
- `crates/context-graph-core/src/teleology/mod.rs` (NEW)
- `crates/context-graph-storage/src/teleological/mod.rs` (NEW)

**Work**:
- Create teleology modules in both crates
- Re-export all public types
- Add to crate lib.rs files
- Update workspace dependencies

**Acceptance Criteria**:
- Modules compile and export correctly
- Types are accessible from crate roots

---

## Phase 3: Implement Entry-Point Discovery Search

**Goal**: Implement the search engine that supports all comparison types and the 5-stage retrieval pipeline described in the architecture.

**Duration**: 2 weeks
**Dependencies**: Phase 2 (all tasks)
**Risk Level**: Medium

### Task 3.1: Single Embedder Search

**File**: `crates/context-graph-storage/src/teleological/search/single.rs` (NEW)

**Work**:
- Implement `SingleEmbedderSearch` struct
- Search on single embedder index
- Support top-k retrieval with similarity threshold
- Validate query embedding matches expected type
- Return ranked results with scores

**Acceptance Criteria**:
- Single-embedder search works for all 13 embedders
- Top-k results are correctly ranked
- Threshold filtering works

### Task 3.2: Embedder Group Search

**File**: `crates/context-graph-storage/src/teleological/search/group.rs` (NEW)

**Work**:
- Implement `EmbedderGroupSearch` struct
- Search multiple indices from group
- Aggregate scores with equal weights
- Support all predefined groups (Temporal, Lexical, etc.)
- Return combined results with per-embedder breakdown

**Acceptance Criteria**:
- Group search works for all groups
- Score aggregation is correct
- Per-embedder breakdown available

### Task 3.3: Weighted Full Array Search

**File**: `crates/context-graph-storage/src/teleological/search/weighted.rs` (NEW)

**Work**:
- Implement `WeightedFullSearch` struct
- Search all 13 indices with custom weights
- Implement `ParallelStrategy` enum (FullParallel, Staged, Sequential)
- Score fusion with configurable weights
- Re-ranking pass for final results

**Acceptance Criteria**:
- Weighted search works with any weight configuration
- Parallel execution provides speedup
- Staged search enables early termination

### Task 3.4: Matrix Strategy Search

**File**: `crates/context-graph-storage/src/teleological/search/matrix.rs` (NEW)

**Work**:
- Implement `MatrixStrategySearch` struct
- Apply 13x13 matrix weights
- Support cross-embedder correlation analysis (off-diagonal)
- Optimized execution planning based on matrix structure
- Cache hot query patterns

**Acceptance Criteria**:
- Matrix search applies weights correctly
- Correlation analysis works when enabled
- Execution is optimized

### Task 3.5: Search Engine Facade

**File**: `crates/context-graph-storage/src/teleological/search/engine.rs` (NEW)

**Work**:
- Create `TeleologicalSearchEngine` struct
- Route queries to appropriate search strategy
- Support query planning and optimization
- Unified result format with `SearchResults`
- Implement `SearchQueryBuilder` fluent API

**Acceptance Criteria**:
- Facade routes all query types correctly
- Builder API is ergonomic
- Results are consistently formatted

### Task 3.6: 5-Stage Retrieval Pipeline

**File**: `crates/context-graph-storage/src/teleological/search/pipeline.rs` (NEW)

**Work**:
- Implement Stage 1: SPLADE sparse pre-filter
- Implement Stage 2: Matryoshka 128D fast ANN
- Implement Stage 3: Multi-space RRF rerank
- Implement Stage 4: Teleological alignment filter
- Implement Stage 5: Late interaction MaxSim
- Configure pipeline stages per query type

**Acceptance Criteria**:
- Full pipeline achieves <60ms at 1M memories
- Each stage filters appropriately
- Stage skipping works for specialized queries

### Task 3.7: Correlation Analysis

**File**: `crates/context-graph-storage/src/teleological/search/correlation.rs` (NEW)

**Work**:
- Implement `CorrelationAnalysis` struct
- Compute 13x13 correlation matrix between embedder similarities
- Detect patterns (ConsensusHigh, TemporalSemanticAlign, etc.)
- Compute coherence score
- Identify outlier embedders

**Acceptance Criteria**:
- Correlation matrix computed correctly
- Patterns detected accurately
- Insights extraction works

### Task 3.8: MCP Search Tool Updates

**File**: `crates/context-graph-mcp/src/handlers/memory.rs`

**Work**:
- Add `comparison_type` parameter to `memory/search`
- Add `search_matrix` parameter for matrix strategies
- Support all comparison types in search
- Return per-embedder similarity scores
- Add `memory/compare` tool for direct array comparison

**Acceptance Criteria**:
- Search tool accepts comparison parameters
- Per-embedder breakdown returned
- Compare tool works for array-to-array comparison

---

## Phase 4: Claude Code Hooks Integration (Core)

**Goal**: Integrate with Claude Code hooks system to enable automatic learning from operations, session management, and neural pattern training.

**Duration**: 1-2 weeks
**Dependencies**: Phase 2 (Tasks 2.1-2.5)
**Risk Level**: Low-Medium

### Task 4.1: Define Hook Protocol

**File**: `crates/context-graph-mcp/src/hooks/protocol.rs` (NEW)

**Work**:
- Define `HookEvent` enum (PreTask, PostTask, PreEdit, PostEdit, SessionStart, SessionEnd)
- Define `HookPayload` struct with context data
- Define `HookResponse` for feedback
- Implement JSON serialization for MCP compatibility

**Acceptance Criteria**:
- Protocol matches Claude Code hooks specification
- Serialization works correctly
- Types are extensible

### Task 4.2: Implement Pre-Task Hook Handler

**File**: `crates/context-graph-mcp/src/hooks/pre_task.rs` (NEW)

**Work**:
- Handle `hooks/pre-task` MCP call
- Extract task description from payload
- Load relevant context from teleological array store
- Return context recommendations
- Log task start for trajectory tracking

**Acceptance Criteria**:
- Pre-task hook processes correctly
- Context loading works
- Trajectory logging functional

### Task 4.3: Implement Post-Task Hook Handler

**File**: `crates/context-graph-mcp/src/hooks/post_task.rs` (NEW)

**Work**:
- Handle `hooks/post-task` MCP call
- Record task outcome (success/failure)
- Extract patterns from task execution
- Store trajectory step in learning system
- Update task-related teleological arrays

**Acceptance Criteria**:
- Post-task hook processes correctly
- Outcomes recorded
- Patterns extracted

### Task 4.4: Implement Session Hooks

**File**: `crates/context-graph-mcp/src/hooks/session.rs` (NEW)

**Work**:
- Handle `hooks/session-start` - initialize session context
- Handle `hooks/session-end` - consolidate session learnings
- Handle `hooks/session-restore` - reload previous session state
- Persist session state to teleological store
- Export session metrics

**Acceptance Criteria**:
- Session lifecycle managed correctly
- State persists across sessions
- Metrics exportable

### Task 4.5: Implement Edit Hooks

**File**: `crates/context-graph-mcp/src/hooks/edit.rs` (NEW)

**Work**:
- Handle `hooks/pre-edit` - validate edit context
- Handle `hooks/post-edit` with `--train-patterns` flag
- Extract code patterns from edits
- Store edit patterns as teleological arrays
- Link edits to task context

**Acceptance Criteria**:
- Edit hooks process correctly
- Patterns trainable from edits
- Context linked correctly

### Task 4.6: Hook Dispatch Integration

**File**: `crates/context-graph-mcp/src/handlers/core.rs`

**Work**:
- Add dispatch routes for all hook endpoints
- Route to appropriate hook handlers
- Handle hook errors gracefully
- Log hook execution for debugging

**Acceptance Criteria**:
- All hooks dispatchable
- Errors handled gracefully
- Logging comprehensive

### Task 4.7: Background Workers Integration

**File**: `crates/context-graph-mcp/src/hooks/workers.rs` (NEW)

**Work**:
- Define `BackgroundWorker` trait
- Implement worker dispatch system
- Support 12 worker types (ultralearn, optimize, audit, etc.)
- Priority-based worker scheduling
- Worker status monitoring

**Acceptance Criteria**:
- Workers dispatchable
- Priority scheduling works
- Status queryable

---

## Phase 5: Self-Learning Feedback Loops

**Goal**: Implement autonomous goal discovery, drift detection, and self-learning capabilities that replace the manual North Star system.

**Duration**: 2-3 weeks
**Dependencies**: Phases 2, 3, 4
**Risk Level**: Medium-High (core autonomous behavior)

### Task 5.1: Trajectory Tracking

**File**: `crates/context-graph-core/src/autonomous/trajectory.rs` (NEW)

**Work**:
- Define `Trajectory` struct (sequence of steps)
- Define `TrajectoryStep` with task, input, output, reward
- Store trajectories as teleological arrays
- Implement trajectory search by similarity
- Compute trajectory-level metrics

**Acceptance Criteria**:
- Trajectories tracked correctly
- Storage efficient
- Search by similarity works

### Task 5.2: Verdict Judgment System

**File**: `crates/context-graph-core/src/autonomous/verdict.rs` (NEW)

**Work**:
- Define `Verdict` enum (Success, Failure, Partial)
- Implement automatic verdict detection from outcomes
- Store verdicts with trajectories
- Compute success rates by pattern
- Enable learning from failures

**Acceptance Criteria**:
- Verdicts assigned correctly
- Failure patterns trackable
- Statistics computeable

### Task 5.3: Goal Discovery Pipeline

**File**: `crates/context-graph-core/src/autonomous/discovery.rs` (NEW)

**Work**:
- Create `GoalDiscoveryPipeline` struct
- Implement K-means clustering for teleological arrays
- Compute cluster centroids as goal candidates
- Score candidates by coherence and size
- Generate goal descriptions from clusters
- Assign goal levels (NorthStar, Strategic, Tactical, Immediate)

**Acceptance Criteria**:
- Clustering works correctly
- Centroids are valid teleological arrays
- Goal levels assigned appropriately

### Task 5.4: Drift Detection with Arrays

**File**: `crates/context-graph-core/src/autonomous/drift.rs` (NEW)

**Work**:
- Create `TeleologicalDriftDetector` struct
- Implement per-embedder drift analysis
- Classify drift levels (None, Low, Medium, High, Critical)
- Track drift trends over time
- Generate drift recommendations
- Store drift history

**Acceptance Criteria**:
- Drift detected correctly
- Per-embedder breakdown available
- Trends trackable

### Task 5.5: Alignment Calculator Refactor

**File**: `crates/context-graph-core/src/alignment/calculator.rs`

**Work**:
- Update `GoalAlignmentCalculator` trait to use teleological arrays
- Use `TeleologicalComparator` for all comparisons
- Remove any remaining projection logic
- Support configurable comparison strategies
- Return per-embedder alignment scores

**Acceptance Criteria**:
- Calculator uses array comparison
- No projection code remains
- Per-embedder scores returned

### Task 5.6: Autonomous Bootstrap Service

**File**: `crates/context-graph-core/src/autonomous/bootstrap.rs` (NEW)

**Work**:
- Create `AutonomousBootstrapService` struct
- Bootstrap goal hierarchy from stored memories
- Run goal discovery on startup
- Identify North Star from largest coherent cluster
- Build parent-child relationships
- Persist discovered hierarchy

**Acceptance Criteria**:
- Bootstrap creates valid hierarchy
- North Star identified correctly
- Hierarchy persists

### Task 5.7: Refactor drift_check Handler

**File**: `crates/context-graph-mcp/src/handlers/purpose.rs`

**Work**:
- Update `handle_drift_check` to use teleological arrays
- Use `TeleologicalDriftDetector` for analysis
- Return per-embedder drift analysis
- Include drift recommendations
- Remove any projection usage

**Acceptance Criteria**:
- Handler uses new drift detector
- Per-embedder analysis returned
- No projection code

### Task 5.8: Memory Distillation Service

**File**: `crates/context-graph-core/src/autonomous/distillation.rs` (NEW)

**Work**:
- Implement pattern distillation from trajectories
- Extract high-value patterns (reward > threshold)
- Consolidate similar patterns via array comparison
- Prune low-value patterns
- Update goal hierarchy with learned patterns

**Acceptance Criteria**:
- Distillation extracts valuable patterns
- Consolidation reduces redundancy
- Hierarchy updated correctly

### Task 5.9: MCP Autonomous Tools

**File**: `crates/context-graph-mcp/src/handlers/autonomous.rs` (NEW)

**Work**:
- Implement `auto_bootstrap_north_star` - discover goals from data
- Implement `get_alignment_drift` - check drift with array comparison
- Implement `discover_sub_goals` - find sub-goals via clustering
- Implement `get_autonomous_status` - system health and metrics
- All tools use teleological arrays, no single embeddings

**Acceptance Criteria**:
- All tools work correctly
- Return meaningful results
- No single-embedding comparisons

### Task 5.10: Learning Integration with Hooks

**File**: `crates/context-graph-mcp/src/hooks/intelligence.rs` (NEW)

**Work**:
- Implement `hooks/intelligence` for trajectory events
- Handle trajectory-start, trajectory-step, trajectory-end
- Store patterns via pattern-store
- Search patterns via pattern-search
- Compute and return learning statistics

**Acceptance Criteria**:
- Intelligence hooks work
- Patterns stored correctly
- Statistics accurate

---

## Phase 6: Testing and Validation

**Goal**: Ensure all refactored components work correctly and the system maintains quality.

**Duration**: 1-2 weeks (parallel with phases 3-5)
**Dependencies**: Phases 1-5 (incremental)
**Risk Level**: Low

### Task 6.1: Unit Tests for Core Types

**Work**:
- Test `TeleologicalArray` creation, serialization, validation
- Test `Embedder` enum methods
- Test `ComparisonType` and `SearchMatrix`
- Test `EmbedderMask` and `EmbedderGroup`
- Achieve >90% coverage on core types

**Acceptance Criteria**:
- All core types have unit tests
- Edge cases covered
- Coverage >90%

### Task 6.2: Unit Tests for Comparison Engine

**Work**:
- Test dense, sparse, token-level similarities
- Test `TeleologicalComparator` with various matrices
- Test edge cases (empty vectors, dimension mismatches)
- Verify SIMD vs non-SIMD produce same results
- Benchmark comparison performance

**Acceptance Criteria**:
- Similarity functions tested
- Comparator tested with all types
- Performance benchmarked

### Task 6.3: Integration Tests for Storage

**Work**:
- Test store/retrieve cycles
- Test per-embedder index operations
- Test concurrent access (multiple readers/writers)
- Test batch operations atomicity
- Test index rebuild

**Acceptance Criteria**:
- Storage operations reliable
- Concurrency safe
- Indices consistent

### Task 6.4: Integration Tests for Search

**Work**:
- Test single-embedder search
- Test weighted search with various weights
- Test matrix strategy search
- Test 5-stage pipeline
- Benchmark search latency

**Acceptance Criteria**:
- All search types work
- Results correctly ranked
- Latency within targets

### Task 6.5: Integration Tests for Hooks

**Work**:
- Test pre-task and post-task hooks
- Test session lifecycle hooks
- Test edit hooks with pattern training
- Test background worker dispatch
- Verify hook error handling

**Acceptance Criteria**:
- All hooks work correctly
- Errors handled gracefully
- Background workers function

### Task 6.6: E2E Tests for MCP Tools

**Work**:
- Test `memory/store` with teleological arrays
- Test `memory/search` with different comparison types
- Test `memory/compare` operations
- Test autonomous tools (bootstrap, drift, discovery)
- Test hook endpoints

**Acceptance Criteria**:
- All MCP tools work E2E
- Correct JSON-RPC responses
- Error codes appropriate

### Task 6.7: Verify No Projection Code Remains

**Work**:
- Grep codebase for projection-related code
- Verify alignment uses array comparison only
- Check no dimension reduction in comparisons
- Audit all similarity calculations

**Acceptance Criteria**:
- `rg "project_embedding" --type rust` returns nothing
- `rg "dimension.*reduction" --type rust` returns nothing
- All comparisons apples-to-apples

### Task 6.8: Migration Testing

**Work**:
- Test migration of existing TeleologicalFingerprint to TeleologicalArray
- Test GoalNode migration (regenerate embeddings from description)
- Verify data integrity after migration
- Test rollback procedures

**Acceptance Criteria**:
- Migration script works
- Data integrity maintained
- Rollback possible

---

## Phase 7: Claude Code Hook Implementation (Full)

**Goal**: Implement all 10 Claude Code hook types with complete integration into the teleological array system.

**Duration**: 1.5-2 weeks
**Dependencies**: Phase 4 (Core Hooks), Phase 2 (Storage)
**Risk Level**: Medium
**Complexity**: High

### Task 7.1: Create Claude Code Settings Configuration

**Directory**: `.claude/` (NEW)
**File**: `.claude/settings.json`

**Work**:
- Create `.claude/` directory structure
- Define `settings.json` schema with hooks configuration
- Configure all 10 hook types with proper matchers
- Set up hook timeouts and error handling
- Configure hook logging and metrics

**Configuration Structure**:
```json
{
  "hooks": {
    "PreToolCall": [...],
    "PostToolCall": [...],
    "PreFileRead": [...],
    "PostFileRead": [...],
    "PreFileWrite": [...],
    "PostFileWrite": [...],
    "PreBashExec": [...],
    "PostBashExec": [...],
    "SessionStart": [...],
    "SessionEnd": [...]
  }
}
```

**Acceptance Criteria**:
- `.claude/settings.json` created with valid schema
- All 10 hook types configured
- Hooks can be enabled/disabled per type
- Timeouts and error handling configured

### Task 7.2: Implement PreToolCall Hook

**Files**:
- `crates/context-graph-mcp/src/hooks/tool_call.rs` (NEW)
- `.claude/hooks/pre_tool_call.sh` (NEW)

**Work**:
- Create `PreToolCallHandler` in Rust
- Implement context injection before tool execution
- Query teleological store for relevant context
- Return context recommendations to Claude
- Create shell script wrapper for Claude Code integration
- Handle tool filtering (which tools trigger the hook)

**Complexity**: Medium
**Dependencies**: Phase 4.1, Phase 2.6

**Acceptance Criteria**:
- Hook triggers before tool calls
- Context injected from teleological store
- Tool filtering works correctly
- Latency <50ms

### Task 7.3: Implement PostToolCall Hook

**Files**:
- `crates/context-graph-mcp/src/hooks/tool_call.rs` (EXTEND)
- `.claude/hooks/post_tool_call.sh` (NEW)

**Work**:
- Create `PostToolCallHandler` in Rust
- Capture tool execution results
- Store tool outcomes in teleological arrays
- Update trajectory with tool step
- Train patterns from successful tool usages
- Implement result caching for repeated queries

**Complexity**: Medium
**Dependencies**: Task 7.2, Phase 5.1

**Acceptance Criteria**:
- Hook triggers after tool calls
- Results stored correctly
- Patterns trainable from tool usage
- Trajectory updated

### Task 7.4: Implement PreFileRead Hook

**Files**:
- `crates/context-graph-mcp/src/hooks/file_ops.rs` (NEW)
- `.claude/hooks/pre_file_read.sh` (NEW)

**Work**:
- Create `PreFileReadHandler` in Rust
- Check file relevance against current goals
- Preload related file embeddings
- Return file context recommendations
- Handle file pattern matching (*.rs, *.py, etc.)

**Complexity**: Low
**Dependencies**: Phase 2.5

**Acceptance Criteria**:
- Hook triggers before file reads
- File context returned
- Pattern matching works

### Task 7.5: Implement PostFileRead Hook

**Files**:
- `crates/context-graph-mcp/src/hooks/file_ops.rs` (EXTEND)
- `.claude/hooks/post_file_read.sh` (NEW)

**Work**:
- Capture file content after read
- Generate teleological array for file content
- Store file-to-goal relationships
- Track file access patterns
- Build file dependency graph

**Complexity**: Medium
**Dependencies**: Task 7.4, Phase 2.1

**Acceptance Criteria**:
- File content captured
- Teleological arrays generated
- Access patterns tracked

### Task 7.6: Implement PreFileWrite Hook

**Files**:
- `crates/context-graph-mcp/src/hooks/file_ops.rs` (EXTEND)
- `.claude/hooks/pre_file_write.sh` (NEW)

**Work**:
- Validate write against goal alignment
- Check for potential conflicts with existing content
- Compute diff alignment score
- Return write recommendations
- Prevent writes that drift significantly from goals

**Complexity**: High
**Dependencies**: Phase 5.4, Phase 2.5

**Acceptance Criteria**:
- Pre-write validation works
- Alignment checked before write
- Recommendations returned

### Task 7.7: Implement PostFileWrite Hook

**Files**:
- `crates/context-graph-mcp/src/hooks/file_ops.rs` (EXTEND)
- `.claude/hooks/post_file_write.sh` (NEW)

**Work**:
- Store file change as teleological array
- Update file embedding in index
- Track edit patterns for learning
- Trigger background workers for analysis
- Update goal progress metrics

**Complexity**: Medium
**Dependencies**: Task 7.6, Phase 4.7

**Acceptance Criteria**:
- File changes stored
- Embeddings updated
- Workers triggered

### Task 7.8: Implement PreBashExec Hook

**Files**:
- `crates/context-graph-mcp/src/hooks/bash_exec.rs` (NEW)
- `.claude/hooks/pre_bash_exec.sh` (NEW)

**Work**:
- Analyze command intent before execution
- Check command safety against goal context
- Load environment context from teleological store
- Return command recommendations
- Implement command pattern matching

**Complexity**: Medium
**Dependencies**: Phase 4.2

**Acceptance Criteria**:
- Command analysis works
- Safety checks functional
- Context loaded correctly

### Task 7.9: Implement PostBashExec Hook

**Files**:
- `crates/context-graph-mcp/src/hooks/bash_exec.rs` (EXTEND)
- `.claude/hooks/post_bash_exec.sh` (NEW)

**Work**:
- Capture command output and exit code
- Store execution results as teleological arrays
- Learn from successful/failed commands
- Update trajectory with execution step
- Train patterns from command sequences

**Complexity**: Medium
**Dependencies**: Task 7.8, Phase 5.1

**Acceptance Criteria**:
- Output captured correctly
- Results stored
- Patterns trainable

### Task 7.10: Implement SessionStart and SessionEnd Hooks

**Files**:
- `crates/context-graph-mcp/src/hooks/session.rs` (EXTEND)
- `.claude/hooks/session_start.sh` (NEW)
- `.claude/hooks/session_end.sh` (NEW)

**Work**:
- Extend session handlers from Phase 4.4
- Initialize teleological context on session start
- Load previous session state and goals
- Consolidate learnings on session end
- Export session metrics and patterns
- Trigger background consolidation workers

**Complexity**: Medium
**Dependencies**: Phase 4.4, Phase 5.8

**Acceptance Criteria**:
- Sessions initialized correctly
- State persists across sessions
- Metrics exported

### Task 7.11: Hook Registry and Dispatch

**File**: `crates/context-graph-mcp/src/hooks/registry.rs` (NEW)

**Work**:
- Create centralized `HookRegistry` for all 10 hook types
- Implement async hook dispatch with timeout handling
- Support hook chaining (multiple handlers per event)
- Implement hook priority ordering
- Add hook metrics collection
- Create hook testing utilities

**Complexity**: Medium
**Dependencies**: Tasks 7.2-7.10

**Acceptance Criteria**:
- Registry manages all hooks
- Dispatch works correctly
- Chaining and priorities work
- Metrics collected

---

## Phase 8: Skills Development

**Goal**: Create 4-6 core skills that leverage the teleological array system for Claude Code.

**Duration**: 1-2 weeks
**Dependencies**: Phase 7 (Hook Implementation)
**Risk Level**: Low-Medium
**Complexity**: Medium

### Task 8.1: Create Skills Directory Structure

**Directory**: `.claude/skills/` (NEW)

**Work**:
- Create `.claude/skills/` directory
- Define skill manifest format (`skill.yaml` or `skill.json`)
- Create skill template structure
- Implement skill discovery mechanism
- Document skill development guidelines

**Directory Structure**:
```
.claude/skills/
    memory-search/
        skill.yaml
        handler.ts
        tests/
    goal-alignment/
        skill.yaml
        handler.ts
        tests/
    pattern-learning/
        skill.yaml
        handler.ts
        tests/
    context-injection/
        skill.yaml
        handler.ts
        tests/
```

**Acceptance Criteria**:
- Directory structure created
- Manifest format defined
- Discovery mechanism works

### Task 8.2: Implement Memory Search Skill

**Directory**: `.claude/skills/memory-search/` (NEW)

**Work**:
- Create skill manifest with invocation patterns
- Implement handler that wraps teleological search
- Support natural language queries
- Return formatted search results
- Include usage examples and tests

**Skill Capabilities**:
- Search by semantic similarity
- Search by embedder group
- Search with custom matrix
- Return per-embedder breakdowns

**Complexity**: Medium
**Dependencies**: Phase 3.5

**Acceptance Criteria**:
- Skill invocable from Claude
- Search works correctly
- Results formatted properly

### Task 8.3: Implement Goal Alignment Skill

**Directory**: `.claude/skills/goal-alignment/` (NEW)

**Work**:
- Create skill for checking goal alignment
- Query current goals from teleological store
- Compute alignment scores for given content
- Return alignment recommendations
- Support different comparison strategies

**Skill Capabilities**:
- Check content alignment with North Star
- Check alignment with sub-goals
- Get alignment breakdown by embedder
- Suggest improvements for better alignment

**Complexity**: Medium
**Dependencies**: Phase 5.5

**Acceptance Criteria**:
- Alignment checking works
- Per-embedder scores returned
- Recommendations provided

### Task 8.4: Implement Pattern Learning Skill

**Directory**: `.claude/skills/pattern-learning/` (NEW)

**Work**:
- Create skill for explicit pattern storage
- Accept pattern descriptions and examples
- Generate teleological arrays for patterns
- Store with proper indexing
- Support pattern retrieval and search

**Skill Capabilities**:
- Store new patterns
- Search existing patterns
- Get pattern statistics
- Consolidate similar patterns

**Complexity**: Medium
**Dependencies**: Phase 5.8

**Acceptance Criteria**:
- Patterns storable via skill
- Search and retrieval work
- Statistics available

### Task 8.5: Implement Context Injection Skill

**Directory**: `.claude/skills/context-injection/` (NEW)

**Work**:
- Create skill for manual context injection
- Accept context content and metadata
- Generate teleological array for context
- Store in appropriate namespace
- Link to current session/goals

**Skill Capabilities**:
- Inject arbitrary context
- Tag context with metadata
- Link to goals/tasks
- Search injected contexts

**Complexity**: Low
**Dependencies**: Phase 2.8

**Acceptance Criteria**:
- Context injection works
- Metadata stored correctly
- Search works

### Task 8.6: Implement Drift Check Skill

**Directory**: `.claude/skills/drift-check/` (NEW)

**Work**:
- Create skill for on-demand drift checking
- Wrap teleological drift detector
- Return per-embedder drift analysis
- Provide actionable recommendations
- Support trend visualization data

**Skill Capabilities**:
- Check current drift from goals
- Get per-embedder breakdown
- Get drift history/trends
- Get recommendations

**Complexity**: Medium
**Dependencies**: Phase 5.4

**Acceptance Criteria**:
- Drift checking works
- Recommendations provided
- Trends available

### Task 8.7: Skill Loader and Manager

**File**: `crates/context-graph-mcp/src/skills/loader.rs` (NEW)

**Work**:
- Implement skill discovery from `.claude/skills/`
- Parse skill manifests
- Validate skill configurations
- Load skill handlers
- Route skill invocations to handlers
- Manage skill lifecycle

**Complexity**: Medium
**Dependencies**: Tasks 8.2-8.6

**Acceptance Criteria**:
- Skills discovered automatically
- Manifests parsed correctly
- Invocations routed properly

---

## Phase 9: Subagent Creation

**Goal**: Create custom subagents that leverage the teleological array system for specialized tasks.

**Duration**: 1-2 weeks
**Dependencies**: Phase 7 (Hooks), Phase 8 (Skills)
**Risk Level**: Medium
**Complexity**: High

### Task 9.1: Create Subagent Directory Structure

**Directory**: `.claude/agents/` (NEW)

**Work**:
- Create `.claude/agents/` directory
- Define agent manifest format (`agent.yaml`)
- Create agent template structure
- Implement agent discovery mechanism
- Document agent development guidelines

**Directory Structure**:
```
.claude/agents/
    goal-tracker/
        agent.yaml
        prompts/
            system.md
            instructions.md
        tools/
        tests/
    context-curator/
        agent.yaml
        prompts/
        tools/
        tests/
    pattern-miner/
        agent.yaml
        prompts/
        tools/
        tests/
```

**Acceptance Criteria**:
- Directory structure created
- Manifest format defined
- Discovery mechanism works

### Task 9.2: Implement Goal Tracker Subagent

**Directory**: `.claude/agents/goal-tracker/` (NEW)

**Work**:
- Create agent manifest with capabilities
- Write system prompt for goal tracking behavior
- Implement goal monitoring loop
- Track goal progress and drift
- Generate periodic goal reports
- Trigger alerts on significant drift

**Agent Capabilities**:
- Monitor goal alignment continuously
- Track drift trends over time
- Generate progress reports
- Alert on critical drift
- Suggest goal refinements

**Complexity**: High
**Dependencies**: Phase 5.4, Phase 8.3

**Acceptance Criteria**:
- Agent monitors goals correctly
- Drift alerts work
- Reports generated

### Task 9.3: Implement Context Curator Subagent

**Directory**: `.claude/agents/context-curator/` (NEW)

**Work**:
- Create agent manifest with capabilities
- Write system prompt for context curation behavior
- Implement automatic context organization
- Curate and prune low-value contexts
- Merge similar contexts
- Maintain context quality metrics

**Agent Capabilities**:
- Organize contexts by goal relevance
- Prune stale/low-value contexts
- Merge duplicate/similar contexts
- Report context health
- Suggest context improvements

**Complexity**: High
**Dependencies**: Phase 2.8, Phase 5.8

**Acceptance Criteria**:
- Agent curates contexts correctly
- Pruning works safely
- Merging preserves value

### Task 9.4: Implement Pattern Miner Subagent

**Directory**: `.claude/agents/pattern-miner/` (NEW)

**Work**:
- Create agent manifest with capabilities
- Write system prompt for pattern mining behavior
- Implement background pattern discovery
- Cluster similar trajectories
- Extract high-value patterns
- Report pattern insights

**Agent Capabilities**:
- Discover patterns from trajectories
- Cluster similar patterns
- Rank patterns by value
- Generate pattern reports
- Suggest pattern applications

**Complexity**: High
**Dependencies**: Phase 5.1, Phase 5.3

**Acceptance Criteria**:
- Agent discovers patterns correctly
- Clustering works
- Reports generated

### Task 9.5: Implement Learning Coach Subagent

**Directory**: `.claude/agents/learning-coach/` (NEW)

**Work**:
- Create agent manifest with capabilities
- Write system prompt for learning guidance
- Analyze learning trajectories
- Identify learning gaps
- Suggest learning opportunities
- Track skill development

**Agent Capabilities**:
- Analyze learning progress
- Identify skill gaps
- Suggest learning paths
- Track improvements
- Generate learning reports

**Complexity**: Medium
**Dependencies**: Phase 5.2, Phase 5.10

**Acceptance Criteria**:
- Agent provides useful guidance
- Gaps identified correctly
- Suggestions actionable

### Task 9.6: Subagent Orchestrator

**File**: `crates/context-graph-mcp/src/agents/orchestrator.rs` (NEW)

**Work**:
- Create `SubagentOrchestrator` for managing subagents
- Implement agent lifecycle management (start, stop, restart)
- Handle inter-agent communication via teleological store
- Coordinate agent tasks and priorities
- Manage agent resource allocation
- Implement agent health monitoring

**Complexity**: High
**Dependencies**: Tasks 9.2-9.5

**Acceptance Criteria**:
- Orchestrator manages all agents
- Lifecycle management works
- Inter-agent communication works

### Task 9.7: Subagent MCP Integration

**File**: `crates/context-graph-mcp/src/handlers/agents.rs` (NEW)

**Work**:
- Create MCP handlers for subagent operations
- Implement `agents/list` - list available agents
- Implement `agents/start` - start an agent
- Implement `agents/stop` - stop an agent
- Implement `agents/status` - get agent status
- Implement `agents/communicate` - send message to agent

**Complexity**: Medium
**Dependencies**: Task 9.6

**Acceptance Criteria**:
- All MCP handlers work
- Agent control functional
- Status reporting accurate

---

## Phase 10: Integration Testing (Hooks/Skills/Subagents)

**Goal**: Comprehensive testing of the Claude Code integration components working together.

**Duration**: 1-2 weeks
**Dependencies**: Phases 7, 8, 9
**Risk Level**: Low
**Complexity**: Medium

### Task 10.1: Hook Integration Tests

**File**: `crates/context-graph-mcp/tests/hooks_integration.rs` (NEW)

**Work**:
- Test all 10 hook types end-to-end
- Test hook chaining scenarios
- Test hook error handling
- Test hook timeouts
- Test hook metrics collection
- Verify hooks work with Claude Code

**Acceptance Criteria**:
- All hooks tested E2E
- Error handling verified
- Performance benchmarked

### Task 10.2: Skills Integration Tests

**File**: `crates/context-graph-mcp/tests/skills_integration.rs` (NEW)

**Work**:
- Test skill discovery and loading
- Test each skill's functionality
- Test skill invocation from Claude
- Test skill error handling
- Test skill-to-hook integration
- Verify skills work with teleological store

**Acceptance Criteria**:
- All skills tested
- Invocation works
- Store integration verified

### Task 10.3: Subagent Integration Tests

**File**: `crates/context-graph-mcp/tests/agents_integration.rs` (NEW)

**Work**:
- Test agent lifecycle management
- Test inter-agent communication
- Test agent-to-hook integration
- Test agent-to-skill integration
- Test agent orchestration
- Verify agents work with teleological store

**Acceptance Criteria**:
- All agents tested
- Lifecycle management verified
- Communication works

### Task 10.4: Full System Integration Tests

**File**: `crates/context-graph-mcp/tests/full_integration.rs` (NEW)

**Work**:
- Test complete workflows involving hooks, skills, and agents
- Test session lifecycle with all components
- Test learning feedback loops
- Test goal alignment across components
- Test performance under load
- Test error recovery scenarios

**Acceptance Criteria**:
- Full workflows tested
- All components work together
- Performance acceptable

### Task 10.5: Claude Code Compatibility Tests

**Directory**: `tests/claude_code/` (NEW)

**Work**:
- Create test scripts that simulate Claude Code interactions
- Test `.claude/settings.json` configuration
- Test skill invocation patterns
- Test agent invocation patterns
- Verify JSON-RPC compatibility
- Test with real Claude Code (if available)

**Acceptance Criteria**:
- Claude Code compatibility verified
- Configuration works
- Invocations function correctly

### Task 10.6: Performance Benchmarks

**File**: `crates/context-graph-mcp/benches/integration_bench.rs` (NEW)

**Work**:
- Benchmark hook execution latency
- Benchmark skill invocation latency
- Benchmark agent response time
- Benchmark teleological store operations with hooks
- Compare against performance targets
- Generate performance report

**Performance Targets**:
- Hook execution: <50ms
- Skill invocation: <100ms
- Agent response: <500ms
- Store operations: <10ms

**Acceptance Criteria**:
- All benchmarks pass targets
- Performance report generated
- Regressions detectable

---

## Implementation Order Summary

| Phase | Duration | Parallel Possible | Critical Path | Claude Code |
|-------|----------|-------------------|---------------|-------------|
| Phase 1: North Star Removal | 1-2 weeks | No | Yes | No |
| Phase 2: Storage | 2-3 weeks | No | Yes | No |
| Phase 3: Search | 2 weeks | Yes (with 4, 7) | Yes | No |
| Phase 4: Hooks (Core) | 1-2 weeks | Yes (with 3, 7) | No | Partial |
| Phase 5: Self-Learning | 2-3 weeks | No | Yes | No |
| Phase 6: Testing | 1-2 weeks | Yes (incremental) | No | No |
| Phase 7: Hook Implementation | 1.5-2 weeks | Yes (with 3) | No | **Yes** |
| Phase 8: Skills Development | 1-2 weeks | Yes (after 7) | No | **Yes** |
| Phase 9: Subagent Creation | 1-2 weeks | Yes (after 8) | No | **Yes** |
| Phase 10: Integration Testing | 1-2 weeks | No | No | **Yes** |

**Total Timeline**: 10-14 weeks

---

## Claude Code Directory Structure Summary

After all phases, the `.claude/` directory structure will be:

```
.claude/
    settings.json                    # Main configuration with hooks
    hooks/
        pre_tool_call.sh             # PreToolCall hook script
        post_tool_call.sh            # PostToolCall hook script
        pre_file_read.sh             # PreFileRead hook script
        post_file_read.sh            # PostFileRead hook script
        pre_file_write.sh            # PreFileWrite hook script
        post_file_write.sh           # PostFileWrite hook script
        pre_bash_exec.sh             # PreBashExec hook script
        post_bash_exec.sh            # PostBashExec hook script
        session_start.sh             # SessionStart hook script
        session_end.sh               # SessionEnd hook script
    skills/
        memory-search/
            skill.yaml
            handler.ts
            tests/
        goal-alignment/
            skill.yaml
            handler.ts
            tests/
        pattern-learning/
            skill.yaml
            handler.ts
            tests/
        context-injection/
            skill.yaml
            handler.ts
            tests/
        drift-check/
            skill.yaml
            handler.ts
            tests/
    agents/
        goal-tracker/
            agent.yaml
            prompts/
                system.md
                instructions.md
            tools/
            tests/
        context-curator/
            agent.yaml
            prompts/
            tools/
            tests/
        pattern-miner/
            agent.yaml
            prompts/
            tools/
            tests/
        learning-coach/
            agent.yaml
            prompts/
            tools/
            tests/
```

---

## Risk Mitigation

### Breaking Changes
- Existing stored memories may need migration
- MCP tool schemas change (add deprecation period)
- Goal alignment API changes completely

### Migration Strategy
1. Keep old code paths during transition (feature flag `legacy_north_star`)
2. Migrate existing data to new format with script
3. Run both systems in parallel for validation
4. Remove old code paths after validation period

### Rollback Plan
1. Feature flags allow disabling new code
2. Database migrations have reverse scripts
3. Old tool endpoints return deprecation warnings before removal
4. Git tags mark pre-refactor stable points

### Claude Code Integration Risks
- Hook latency may impact Claude Code responsiveness
- Skill failures should not break Claude Code
- Subagent errors should be isolated
- Mitigation: Extensive timeout handling and graceful degradation

---

## Success Criteria

### Phase 1: North Star Removal
- [ ] No `north_star_update` or `north_star_alignment` handlers
- [ ] No `project_embedding` function
- [ ] GoalNode uses TeleologicalArray
- [ ] Tests pass without north star code

### Phase 2: Teleological Array Storage
- [ ] TeleologicalArray type defined with 13 embedders
- [ ] Storage trait implemented for RocksDB
- [ ] Per-embedder indices working
- [ ] Batch operations atomic

### Phase 3: Entry-Point Discovery Search
- [ ] All search strategies implemented
- [ ] 5-stage pipeline achieves latency targets
- [ ] MCP search tools updated
- [ ] Correlation analysis working

### Phase 4: Claude Code Hooks Integration (Core)
- [ ] All hook endpoints functional
- [ ] Session lifecycle managed
- [ ] Pattern training from edits
- [ ] Background workers dispatchable

### Phase 5: Self-Learning Feedback Loops
- [ ] Goal discovery clusters arrays correctly
- [ ] Drift detection uses array comparison
- [ ] Alignment calculator refactored
- [ ] Autonomous bootstrap creates hierarchy

### Phase 6: Testing and Validation
- [ ] >80% test coverage on new code
- [ ] All integration tests pass
- [ ] Performance targets met

### Phase 7: Claude Code Hook Implementation
- [ ] All 10 hook types implemented
- [ ] `.claude/settings.json` configured
- [ ] Hook latency <50ms
- [ ] Hook error handling robust

### Phase 8: Skills Development
- [ ] 5+ core skills implemented
- [ ] Skills discoverable and loadable
- [ ] Skills integrate with teleological store
- [ ] Skills invocable from Claude

### Phase 9: Subagent Creation
- [ ] 4+ subagents implemented
- [ ] Subagent orchestration working
- [ ] Inter-agent communication functional
- [ ] MCP handlers for agent control

### Phase 10: Integration Testing
- [ ] All integration tests pass
- [ ] Claude Code compatibility verified
- [ ] Performance benchmarks pass
- [ ] Full system tested E2E

### Overall
- [ ] No projection code in codebase
- [ ] All comparisons apples-to-apples
- [ ] Test coverage >80% on new code
- [ ] Documentation updated
- [ ] Performance targets met
- [ ] Claude Code integration complete

---

## Appendix: Key Files Reference

### Files to Remove
- Protocol constants for north star
- Dispatch routes for north star
- Handler implementations for north star
- Projection functions in alignment
- North star test files

### Files to Create
```
crates/context-graph-core/src/teleology/
    mod.rs
    array.rs
    embedder.rs
    comparison.rs
    comparator.rs
    similarity/
        mod.rs
        dense.rs
        sparse.rs
        token_level.rs

crates/context-graph-core/src/autonomous/
    mod.rs
    discovery.rs
    drift.rs
    bootstrap.rs
    trajectory.rs
    verdict.rs
    distillation.rs

crates/context-graph-storage/src/teleological/
    mod.rs
    store.rs
    index.rs
    rocksdb_store.rs
    index_manager.rs
    search/
        mod.rs
        single.rs
        group.rs
        weighted.rs
        matrix.rs
        engine.rs
        pipeline.rs
        correlation.rs

crates/context-graph-mcp/src/hooks/
    mod.rs
    protocol.rs
    pre_task.rs
    post_task.rs
    session.rs
    edit.rs
    workers.rs
    intelligence.rs
    registry.rs           # NEW - Phase 7
    tool_call.rs          # NEW - Phase 7
    file_ops.rs           # NEW - Phase 7
    bash_exec.rs          # NEW - Phase 7

crates/context-graph-mcp/src/skills/
    mod.rs                # NEW - Phase 8
    loader.rs             # NEW - Phase 8

crates/context-graph-mcp/src/agents/
    mod.rs                # NEW - Phase 9
    orchestrator.rs       # NEW - Phase 9

crates/context-graph-mcp/src/handlers/
    autonomous.rs         # NEW
    agents.rs             # NEW - Phase 9

.claude/
    settings.json         # NEW - Phase 7
    hooks/                # NEW - Phase 7
        pre_tool_call.sh
        post_tool_call.sh
        pre_file_read.sh
        post_file_read.sh
        pre_file_write.sh
        post_file_write.sh
        pre_bash_exec.sh
        post_bash_exec.sh
        session_start.sh
        session_end.sh
    skills/               # NEW - Phase 8
        memory-search/
        goal-alignment/
        pattern-learning/
        context-injection/
        drift-check/
    agents/               # NEW - Phase 9
        goal-tracker/
        context-curator/
        pattern-miner/
        learning-coach/
```

### Files to Modify
- `crates/context-graph-mcp/src/protocol.rs`
- `crates/context-graph-mcp/src/handlers/core.rs`
- `crates/context-graph-mcp/src/handlers/purpose.rs`
- `crates/context-graph-mcp/src/handlers/memory.rs`
- `crates/context-graph-core/src/alignment/calculator.rs`
- `crates/context-graph-core/src/purpose.rs`
- `crates/context-graph-core/src/lib.rs`
- `crates/context-graph-storage/src/lib.rs`

---

## Appendix: Claude Code Hook Types Reference

| Hook Type | Trigger | Use Case |
|-----------|---------|----------|
| PreToolCall | Before any tool execution | Context injection, validation |
| PostToolCall | After tool execution | Result storage, learning |
| PreFileRead | Before reading a file | Context preloading |
| PostFileRead | After reading a file | Content embedding |
| PreFileWrite | Before writing a file | Alignment validation |
| PostFileWrite | After writing a file | Change tracking |
| PreBashExec | Before shell command | Safety checks |
| PostBashExec | After shell command | Result learning |
| SessionStart | Session initialization | Context loading |
| SessionEnd | Session termination | Learning consolidation |

---

## Appendix: Complexity Estimates

### Phase 7 Tasks
| Task | Complexity | Effort (days) |
|------|------------|---------------|
| 7.1 Settings Configuration | Low | 1 |
| 7.2 PreToolCall Hook | Medium | 2 |
| 7.3 PostToolCall Hook | Medium | 2 |
| 7.4 PreFileRead Hook | Low | 1 |
| 7.5 PostFileRead Hook | Medium | 2 |
| 7.6 PreFileWrite Hook | High | 3 |
| 7.7 PostFileWrite Hook | Medium | 2 |
| 7.8 PreBashExec Hook | Medium | 2 |
| 7.9 PostBashExec Hook | Medium | 2 |
| 7.10 Session Hooks | Medium | 2 |
| 7.11 Hook Registry | Medium | 2 |

### Phase 8 Tasks
| Task | Complexity | Effort (days) |
|------|------------|---------------|
| 8.1 Directory Structure | Low | 0.5 |
| 8.2 Memory Search Skill | Medium | 2 |
| 8.3 Goal Alignment Skill | Medium | 2 |
| 8.4 Pattern Learning Skill | Medium | 2 |
| 8.5 Context Injection Skill | Low | 1 |
| 8.6 Drift Check Skill | Medium | 2 |
| 8.7 Skill Loader | Medium | 2 |

### Phase 9 Tasks
| Task | Complexity | Effort (days) |
|------|------------|---------------|
| 9.1 Directory Structure | Low | 0.5 |
| 9.2 Goal Tracker Agent | High | 3 |
| 9.3 Context Curator Agent | High | 3 |
| 9.4 Pattern Miner Agent | High | 3 |
| 9.5 Learning Coach Agent | Medium | 2 |
| 9.6 Subagent Orchestrator | High | 3 |
| 9.7 MCP Integration | Medium | 2 |

### Phase 10 Tasks
| Task | Complexity | Effort (days) |
|------|------------|---------------|
| 10.1 Hook Integration Tests | Medium | 2 |
| 10.2 Skills Integration Tests | Medium | 2 |
| 10.3 Subagent Integration Tests | Medium | 2 |
| 10.4 Full System Tests | High | 3 |
| 10.5 Claude Code Compatibility | Medium | 2 |
| 10.6 Performance Benchmarks | Medium | 2 |
