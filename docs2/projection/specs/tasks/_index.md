# Task Index - Multi-Array Teleological Fingerprint System

## Overview

This directory contains all implementation tasks for the **Multi-Array Teleological Fingerprint** architecture with **5-stage optimized retrieval pipeline** and **E13 SPLADE sparse embeddings**.

**System Architecture**: 13-embedder array (E1-E12 dense + E13 SPLADE sparse) with Matryoshka truncation, HNSW indexing, and RRF fusion for high-performance semantic memory retrieval.

> **Performance Targets**: <60ms end-to-end latency @ 1M memories using staged retrieval with early termination.

## Layer Summary

| Layer | Tasks | Purpose | Status |
|-------|-------|---------|--------|
| [Foundation](./foundation/_index.md) | F001-F008 | Data structures, storage, SPLADE indexes (17KB quantized) | 0/8 |
| [Logic](./logic/_index.md) | L001-L008 | Computation, 5-stage retrieval pipeline, RRF fusion | 0/8 |
| [Surface](./surface/_index.md) | S001-S008 | MCP handlers, API, integration tests | 0/8 |

**Total Progress: 0/24 tasks (0%)**

## 5-Stage Retrieval Pipeline

The core innovation is the **5-stage retrieval pipeline** that achieves <60ms latency:

```
Stage 1: Matryoshka 256d pre-filter (E1-E4)     -> 10K candidates
Stage 2: HNSW retrieval per-space               -> 1K candidates
Stage 3: SPLADE sparse rerank (E13)             -> 200 candidates
Stage 4: Full-dimensional rerank                -> 50 candidates
Stage 5: RRF fusion across spaces               -> Final results
```

**Key Optimizations**:
- E13 SPLADE: 17KB quantized storage with top-128 sparse activations
- Matryoshka truncation: 256d/512d/768d/1024d progressive dimensions
- Early termination: Skip stages when confidence threshold met
- RRF fusion: `sum(1 / (k + rank_i))` for robust cross-space aggregation

## Dependency Flow

```
Foundation Layer (F001-F008)
    |
    | Data structures, storage schemas, HNSW config
    v
Logic Layer (L001-L008)
    |
    | Query execution, 5-stage pipeline, RRF fusion
    v
Surface Layer (S001-S008)
    |
    | MCP handlers, API endpoints, integration tests
    v
System Complete
```

## Critical Path

```
F001 -> F003 -> F002 -> F004 -> F008 -> L001 -> L005 -> L007 -> L008 -> S001 -> S006 -> S008
```

**Estimated Duration**: 13-16 implementation cycles

## Quick Links

### Foundation Layer
- [TASK-F001](./foundation/TASK-F001-semantic-fingerprint.md): SemanticFingerprint (12-array)
- [TASK-F002](./foundation/TASK-F002-teleological-fingerprint.md): TeleologicalFingerprint
- [TASK-F005](./foundation/TASK-F005-hnsw-indexes.md): HNSW + SPLADE index configuration

### Logic Layer
- [TASK-L001](./logic/TASK-L001-multi-embedding-query-executor.md): Multi-Embedding Query Executor
- [TASK-L007](./logic/TASK-L007-cross-space-similarity-engine.md): Cross-Space Similarity Engine
- [TASK-L008](./logic/TASK-L008-teleological-retrieval-pipeline.md): 5-Stage Retrieval Pipeline

### Surface Layer
- [TASK-S001](./surface/TASK-S001-mcp-memory-handlers.md): MCP Memory Handlers
- [TASK-S002](./surface/TASK-S002-mcp-search-handlers.md): MCP Search Handlers
- [TASK-S006](./surface/TASK-S006-integration-tests.md): Integration Tests

## Storage Specifications

| Component | Dimensions | Storage | Index Type |
|-----------|------------|---------|------------|
| E1-E4 Dense | 1024d each | 4KB/vec | HNSW + Matryoshka |
| E5-E12 Dense | 384-1024d | 1.5-4KB/vec | HNSW |
| E13 SPLADE | 30522 sparse | 17KB quantized | Inverted + BM25 |
| Total per memory | - | ~50KB | Hybrid |

---

*Task index created: 2026-01-05*
*Architecture: 5-Stage Optimized Retrieval Pipeline*
*Performance: <60ms @ 1M memories*
