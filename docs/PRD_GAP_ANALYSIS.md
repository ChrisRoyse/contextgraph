# Context Graph PRD v6 Gap Analysis Report

**Generated**: 2026-01-18
**PRD Version**: 6.0.0 (Topic-Based Architecture)
**Current Branch**: multistar
**Last Commit**: 782c53f

---

## Executive Summary

The contextgraph project is approximately **85% complete** relative to PRD v6 requirements. Core systems (13-embedder, topic clustering, dream consolidation, UTL learning, storage) are fully implemented. However, several PRD-mandated features remain missing or incomplete:

| Category | Status | Details |
|----------|--------|---------|
| Core Architecture | ✅ Complete | 13-embedder, clustering, storage |
| MCP Tools | ⚠️ Partial | 6 of 15+ PRD-required tools exposed |
| Claude Code Hooks | ⚠️ Partial | 5 of 6 hooks configured (no Stop hook) |
| Skills Framework | ❌ Missing | 0 of 5 skills implemented |
| Test Suite | ⚠️ Broken | 11 compilation errors in MCP tests |

---

## 1. Fully Implemented Components

### 1.1 13-Embedder System ✅
**Location**: `crates/context-graph-embeddings/` + `crates/context-graph-core/src/types/fingerprint/`

All 13 embedders defined and categorized per PRD:

| ID | Name | Dimension | Category | Topic Weight | Status |
|----|------|-----------|----------|--------------|--------|
| E1 | Semantic (Matryoshka) | 1024D | SEMANTIC | 1.0 | ✅ |
| E2 | Temporal-Recent | 512D | TEMPORAL | 0.0 | ✅ |
| E3 | Temporal-Periodic | 512D | TEMPORAL | 0.0 | ✅ |
| E4 | Temporal-Positional | 512D | TEMPORAL | 0.0 | ✅ |
| E5 | Causal | 768D | SEMANTIC | 1.0 | ✅ |
| E6 | Sparse | ~30K | SEMANTIC | 1.0 | ✅ |
| E7 | Code | 1536D | SEMANTIC | 1.0 | ✅ |
| E8 | Graph/GNN | 384D | RELATIONAL | 0.5 | ✅ |
| E9 | HDC | 10K→1024D | STRUCTURAL | 0.5 | ✅ |
| E10 | Multimodal | 768D | SEMANTIC | 1.0 | ✅ |
| E11 | Entity/TransE | 384D | RELATIONAL | 0.5 | ✅ |
| E12 | Late-Interaction | 128D/token | SEMANTIC | 1.0 | ✅ |
| E13 | SPLADE | ~30K sparse | SEMANTIC | 1.0 | ✅ |

**Compliance**:
- ARCH-01: TeleologicalArray is atomic ✅
- ARCH-04: Temporal embedders (E2-E4) excluded from topic detection ✅
- ARCH-05: All 13 embedders required ✅

### 1.2 Memory System ✅
**Location**: `crates/context-graph-core/src/memory/`

| Component | File | Status |
|-----------|------|--------|
| Memory struct | `mod.rs` | ✅ Fully implemented |
| MemorySource enum | `source.rs` | ✅ HookDescription, ClaudeResponse, MDFileChunk |
| ChunkMetadata | `mod.rs` | ✅ file_path, chunk_index, total_chunks, word_offset, char_offset, hash |
| TextChunker | `chunker.rs` | ✅ 200 words, 50 overlap, sentence boundary detection |
| MDFileWatcher | `watcher.rs` | ✅ notify-based, debounced, SHA256 hash tracking |
| MemoryCaptureService | `capture.rs` | ✅ Embedding + storage pipeline |
| SessionManager | `manager.rs` | ✅ Session lifecycle |

**PRD Compliance**:
- 200-word chunks with 50-word (25%) overlap ✅
- Sentence boundary preservation ✅
- SHA256 hash for change detection ✅
- Three memory sources (ARCH-11) ✅

### 1.3 Topic System ✅
**Location**: `crates/context-graph-core/src/clustering/`

| Component | Status | Notes |
|-----------|--------|-------|
| HDBSCAN clustering | ✅ | Batch clustering per space |
| BIRCH CF-trees | ✅ | Online incremental updates |
| TopicSynthesizer | ✅ | Cross-space weighted agreement |
| TopicStability | ✅ | Churn rate, entropy, phases |
| WeightedAgreement | ✅ | Threshold 2.5, max 8.5 |

**Formula Compliance**:
```
weighted_agreement = Σ(topic_weight_i × is_clustered_i)
Max = 7×1.0 (semantic) + 2×0.5 (relational) + 1×0.5 (structural) = 8.5
Topic threshold: weighted_agreement >= 2.5 ✅
```

### 1.4 Dream System ✅
**Location**: `crates/context-graph-core/src/dream/`

| Phase | File | Status | Implementation |
|-------|------|--------|----------------|
| NREM | `nrem.rs` | ✅ | Hebbian learning replay, Δw_ij = η×φ_i×φ_j |
| REM | `rem.rs` + `hyperbolic_walk.rs` | ✅ | Poincaré ball random walk |
| Triggers | `triggers.rs` | ✅ | entropy > 0.7 AND churn > 0.5 |
| Wake Controller | `wake_controller.rs` | ✅ | <100ms wake latency |
| Amortized | `amortized.rs` | ✅ | 3+ hop paths, 5+ traversals |

**Anti-Pattern Compliance**:
- AP-70: Dream triggers use entropy > 0.7 AND churn > 0.5 ✅
- AP-71: NREM/REM not returning stubs ✅
- AP-72: nrem.rs/rem.rs fully implemented ✅

### 1.5 UTL (Unified Theory of Learning) ✅
**Location**: `crates/context-graph-utl/`

- Learning formula implemented: `L = f((ΔS × ΔC) · w_e · cos φ)`
- Multi-embedder formula: `L_multi = sigmoid(2.0 · (Σ τ_i λ_S · ΔS_i) · (Σ τ_j λ_C · ΔC_j) · w_e · cos φ)`
- Lifecycle phases: Infancy (0-50), Growth (50-500), Maturity (500+) ✅
- ΔS methods per embedder (GMM, KNN, Hamming, Jaccard) ✅
- ΔC formula: 0.4×Connectivity + 0.4×ClusterFit + 0.2×Consistency ✅

### 1.6 Other Fully Implemented Systems

| System | Location | Status |
|--------|----------|--------|
| GWT Workspace | `core/src/gwt/` | ✅ WTA broadcast, state machine |
| Neuromodulation | `core/src/neuromod/` | ✅ DA, 5-HT, NE, ACh |
| ATC Calibration | `core/src/atc/` | ✅ 4 levels, 6 thresholds |
| Retrieval | `core/src/retrieval/` | ✅ 5-stage pipeline |
| Autonomous System | `core/src/autonomous/` | ✅ Drift, pruning, consolidation |
| Storage | `storage/` | ✅ RocksDB + HNSW indexes |
| 5-Layer Architecture | `core/src/layers/` | ✅ L1-L5 nervous system |

---

## 2. Missing/Incomplete Components

### 2.1 Skills Framework ❌ CRITICAL
**Expected Location**: `.claude/skills/*/SKILL.md`
**Status**: Directory exists but is EMPTY

The PRD specifies 5 user-invocable skills:

| Skill | Expected Location | Status |
|-------|-------------------|--------|
| topic-explorer | `.claude/skills/topic-explorer/SKILL.md` | ❌ Missing |
| memory-inject | `.claude/skills/memory-inject/SKILL.md` | ❌ Missing |
| semantic-search | `.claude/skills/semantic-search/SKILL.md` | ❌ Missing |
| dream-consolidation | `.claude/skills/dream-consolidation/SKILL.md` | ❌ Missing |
| curation | `.claude/skills/curation/SKILL.md` | ❌ Missing |

**Impact**: Users cannot invoke context graph functionality via Claude Code `/` commands.

### 2.2 MCP Tools ⚠️ PARTIAL
**Location**: `crates/context-graph-mcp/src/tools/names.rs`

**Currently Exposed (6 tools)**:
- `inject_context` ✅
- `store_memory` ✅
- `get_memetic_status` ✅
- `search_graph` ✅
- `trigger_consolidation` ✅
- `merge_concepts` ✅

**PRD-Required but NOT Exposed**:

| Tool | PRD Section | Status |
|------|-------------|--------|
| `get_topic_portfolio` | 10.2 | ❌ TODO in names.rs |
| `get_topic_stability` | 10.2 | ❌ TODO in names.rs |
| `detect_topics` | 10.2 | ❌ TODO in names.rs |
| `get_divergence_alerts` | 10.2 | ❌ TODO in names.rs |
| `forget_concept` | 10.3 | ❌ TODO in names.rs |
| `boost_importance` | 10.3 | ❌ TODO in names.rs |

**Note**: The code comment in `names.rs` (lines 21-27) explicitly marks these as TODO items per PRD v6 Section 10.

### 2.3 Claude Code Hooks ⚠️ PARTIAL
**Location**: `.claude/settings.json` + `.claude/hooks/`

**Configured Hooks (5)**:
| Hook | Script | Timeout | Status |
|------|--------|---------|--------|
| SessionStart | `session_start.sh` | 5000ms | ✅ |
| SessionEnd | `session_end.sh` | 30000ms | ✅ |
| PreToolUse | `pre_tool_use.sh` | 100ms | ✅ |
| PostToolUse | `post_tool_use.sh` | 3000ms | ✅ |
| UserPromptSubmit | `user_prompt_submit.sh` | 2000ms | ✅ |

**Missing Hook**:
| Hook | PRD Reference | Status |
|------|---------------|--------|
| Stop | PRD 9.1, CLAUDE.md line 700-704 | ❌ Not configured |

**Impact**: Claude responses at session stop may not be captured as `ClaudeResponse` memories.

### 2.4 Test Suite ⚠️ BROKEN
**Location**: `crates/context-graph-mcp/src/handlers/tests/`

**11 Compilation Errors** in test files referencing deleted modules:

```
error[E0432]: unresolved import `crate::handlers::MetaUtlTracker`
error[E0432]: unresolved import `crate::handlers::gwt_providers`
error[E0599]: no function `new` on type `Handlers`
```

**Affected Files**:
- `mod.rs`
- `task_emb_024_verification.rs`
- `manual_fsv_verification.rs`

**Root Cause**: Commit `fab0622` removed handler modules for PRD v6 compliance but did not update test imports.

**Impact**: `cargo test` fails compilation. Release build succeeds.

### 2.5 Documentation Mismatch ⚠️
**File**: `CLAUDE.md` (lines 479-510)

The CLAUDE.md file lists 30+ MCP tools as available:
- 5 topic tools
- 4 retrieval tools
- 2 clustering tools
- 3 memory tools
- 3 adaptive tools
- 2 dream tools
- 5 maintenance tools

**Reality**: Only 6 tools are exposed via MCP.

**Impact**: Documentation misleads users about available functionality.

---

## 3. PRD vs Implementation Matrix

### 3.1 PRD Section 9: Hook Integration

| Requirement | PRD Section | Status |
|-------------|-------------|--------|
| SessionStart hook | 9.1 | ✅ |
| UserPromptSubmit hook | 9.1 | ✅ |
| PreToolUse hook | 9.1 | ✅ |
| PostToolUse hook | 9.1 | ✅ |
| Stop hook | 9.1 | ❌ Missing |
| SessionEnd hook | 9.1 | ✅ |
| Shell scripts call context-graph-cli | 9.2 | ✅ |

### 3.2 PRD Section 10: MCP Tools

| Tool Category | PRD Tools | Implemented | Exposed |
|---------------|-----------|-------------|---------|
| Core | 4 | 4 | 4 |
| Topic | 4 | 4 (internal) | 0 |
| Curation | 3 | 1 | 1 |
| **Total** | **11** | **9** | **5** |

### 3.3 PRD Section 2: Memory Schema

| Field | PRD Spec | Implementation | Status |
|-------|----------|----------------|--------|
| id | UUID | UUID | ✅ |
| content | String | String (max 10,000 chars) | ✅ |
| source | Enum | MemorySource enum | ✅ |
| created_at | Timestamp | DateTime<Utc> | ✅ |
| session_id | String | String | ✅ |
| teleological_array | [E1..E13] | TeleologicalArray | ✅ |
| chunk_metadata | Option | Option<ChunkMetadata> | ✅ |

### 3.4 PRD Section 5: Multi-Space Clustering

| Requirement | PRD Spec | Status |
|-------------|----------|--------|
| HDBSCAN batch clustering | Per space | ✅ |
| BIRCH online updates | CF-trees | ✅ |
| min_cluster_size | 3 | ✅ |
| silhouette_threshold | 0.3 | ✅ |
| Topic threshold | weighted_agreement >= 2.5 | ✅ |
| Temporal exclusion | E2-E4 weight = 0.0 | ✅ |

### 3.5 PRD Section 12: Performance Budgets

| Metric | PRD Target | Status |
|--------|------------|--------|
| All 13 embed | <35ms | ⚠️ Untested |
| Per-space HNSW | <2ms | ⚠️ Untested |
| inject_context P95 | <40ms | ⚠️ Untested |
| Cluster update (BIRCH) | <5ms | ⚠️ Untested |
| Dream wake | <100ms | ✅ Implemented |

**Note**: Performance benchmarks exist but have not been run against PRD targets.

---

## 4. Remediation Plan

### 4.1 Critical Priority (Blocks Basic Operation)

| Task | Effort | Description |
|------|--------|-------------|
| **Create 5 Skills** | 2-3 days | Implement SKILL.md files for topic-explorer, memory-inject, semantic-search, dream-consolidation, curation |
| **Add Stop Hook** | 1 hour | Configure Stop hook in `.claude/settings.json` + create `stop.sh` script |
| **Fix Test Compilation** | 2-4 hours | Remove or update broken test files referencing deleted modules |

### 4.2 High Priority (PRD Compliance)

| Task | Effort | Description |
|------|--------|-------------|
| **Expose Topic MCP Tools** | 1 day | Add handlers for get_topic_portfolio, get_topic_stability, detect_topics, get_divergence_alerts |
| **Expose Curation MCP Tools** | 4 hours | Add handlers for forget_concept, boost_importance |
| **Update CLAUDE.md** | 2 hours | Remove references to non-exposed tools OR add all tools back |

### 4.3 Medium Priority (Quality)

| Task | Effort | Description |
|------|--------|-------------|
| Run Performance Benchmarks | 1 day | Validate against PRD Section 12 targets |
| Add Integration Tests | 2-3 days | End-to-end tests for hook → CLI → MCP flow |
| Documentation Review | 1 day | Ensure all docs match current implementation |

---

## 5. Files Requiring Changes

### 5.1 New Files Needed

```
.claude/skills/topic-explorer/SKILL.md
.claude/skills/memory-inject/SKILL.md
.claude/skills/semantic-search/SKILL.md
.claude/skills/dream-consolidation/SKILL.md
.claude/skills/curation/SKILL.md
.claude/hooks/stop.sh
```

### 5.2 Files Requiring Updates

```
.claude/settings.json              # Add Stop hook configuration
crates/context-graph-mcp/src/tools/names.rs    # Uncomment TODO tools
crates/context-graph-mcp/src/handlers/mod.rs   # Add new tool handlers
crates/context-graph-mcp/src/handlers/tests/*  # Fix or remove broken tests
CLAUDE.md                          # Update tool list to match reality
```

---

## 6. Verification Checklist

After remediation, verify:

- [ ] `cargo build --release` succeeds (currently ✅)
- [ ] `cargo test` succeeds (currently ❌)
- [ ] All 6 hooks configured in `.claude/settings.json`
- [ ] All 5 skills exist in `.claude/skills/`
- [ ] All 11 PRD Section 10 tools exposed via MCP
- [ ] CLAUDE.md tool list matches exposed tools
- [ ] Performance benchmarks pass PRD targets

---

## Appendix A: Architecture Compliance Summary

| Rule | Description | Status |
|------|-------------|--------|
| ARCH-01 | TeleologicalArray atomic | ✅ |
| ARCH-02 | No cross-embedder comparison | ✅ |
| ARCH-03 | Autonomous operation | ✅ |
| ARCH-04 | Temporal excluded from topics | ✅ |
| ARCH-05 | All 13 embedders required | ✅ |
| ARCH-06 | MCP tools only for memory ops | ✅ |
| ARCH-07 | Native Claude Code hooks | ✅ |
| ARCH-08 | CUDA GPU required (prod) | ✅ |
| ARCH-09 | Topic threshold 2.5 | ✅ |
| ARCH-10 | Divergence uses semantic only | ✅ |
| ARCH-11 | Three memory sources | ✅ |

## Appendix B: Anti-Pattern Compliance Summary

| Rule | Description | Status |
|------|-------------|--------|
| AP-02 | No cross-embedder comparison | ✅ |
| AP-50 | Native hooks only | ✅ |
| AP-60 | Temporal excluded from topics | ✅ |
| AP-61 | Topic threshold 2.5 | ✅ |
| AP-62 | Divergence uses semantic only | ✅ |
| AP-70 | Dream trigger conditions | ✅ |
| AP-71 | No NREM/REM stubs | ✅ |
| AP-72 | nrem.rs/rem.rs implemented | ✅ |

---

*Report generated by analyzing PRD v6, CLAUDE.md, and full codebase exploration.*
