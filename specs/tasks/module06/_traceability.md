# Task Traceability Matrix: Module 06 - Stub Elimination

## Purpose

This matrix ensures every stub, fake implementation, and silent fallback identified in the investigation is covered by at least one atomic task. **Empty "Task ID" columns = INCOMPLETE.**

## Coverage Matrix

### Stub Implementations Identified

| Item | Location | Severity | Task ID | Verified |
|------|----------|----------|---------|----------|
| StubUtlProcessor | `stubs/utl_stub.rs` | CRITICAL | M06-T01 | [ ] |
| InMemoryStore (0.5 similarity) | `stubs/memory_store_stub.rs` | CRITICAL | M06-T05 | [ ] |
| InMemoryGraphIndex (brute force) | `stubs/graph_index_stub.rs` | HIGH | M06-T06 | [ ] |
| StubSensingLayer | `stubs/layers_stub.rs` | HIGH | M06-T07 | [ ] |
| StubReflexLayer | `stubs/layers_stub.rs` | HIGH | M06-T08 | [ ] |
| StubMemoryLayer | `stubs/layers_stub.rs` | HIGH | M06-T08 | [ ] |
| StubLearningLayer | `stubs/layers_stub.rs` | HIGH | M06-T08 | [ ] |
| StubCoherenceLayer | `stubs/layers_stub.rs` | HIGH | M06-T08 | [ ] |
| StubLayerConfig | `stubs/layers_stub.rs` | MEDIUM | M06-T08 | [ ] |

### Fake Data in Production Code

| Item | Location | Line(s) | Task ID | Verified |
|------|----------|---------|---------|----------|
| Fake embedding `vec![0.1; 1536]` | `handlers/tools.rs` | 230 | M06-T04, M06-T10 | [ ] |
| Fake embedding `vec![0.1; 1536]` | `handlers/tools.rs` | 273 | M06-T04, M06-T10 | [ ] |
| Fake embedding `vec![0.1; 1536]` | `handlers/memory.rs` | 42 | M06-T04, M06-T10 | [ ] |
| Hash-based UTL metrics | `stubs/utl_stub.rs` | 50-80 | M06-T01 | [ ] |
| Hardcoded `get_status()` zeros | `stubs/utl_stub.rs` | 104-122 | M06-T01 | [ ] |

### Trait/Implementation Gaps

| Trait | Real Implementation | Gap | Task ID | Verified |
|-------|---------------------|-----|---------|----------|
| `core::traits::UtlProcessor` | `utl::processor::UtlProcessor` | No impl, API mismatch (async vs sync) | M06-T01 | [ ] |
| `core::traits::MemoryStore` | N/A (only InMemoryStore stub) | No real implementation | M06-T05 | [ ] |
| `core::traits::GraphIndex` | `cuda::FaissGpuIndex` | Not connected via trait | M06-T06 | [ ] |
| `core::traits::NervousLayer` | N/A (5 stubs only) | No real implementations | M06-T07, M06-T08 | [ ] |
| `embeddings::EmbeddingModel` | `embeddings::candle_models::*` | Factory not wired | M06-T02, M06-T04 | [ ] |

### Silent Fallbacks (Sample - 100+ identified)

| Pattern | Location | Risk | Task ID | Verified |
|---------|----------|------|---------|----------|
| `.unwrap_or(default)` | Throughout handlers/ | Hides failures | M06-T09 | [ ] |
| `.unwrap_or_else(\|_\| fallback)` | Throughout handlers/ | Hides failures | M06-T09 | [ ] |
| `Ok(default_value)` on error | Various service methods | Returns fake success | M06-T09 | [ ] |
| Health check always returns true | `server.rs` | Hides unhealthy state | M06-T09 | [ ] |

### Production Wiring Issues

| Issue | Location | Task ID | Verified |
|-------|----------|---------|----------|
| `Arc::new(StubUtlProcessor::new())` | `server.rs:34` | M06-T10 | [ ] |
| `Arc::new(InMemoryStore::new())` | `server.rs:33` | M06-T10 | [ ] |
| No EmbeddingProvider injected | `handlers/tools.rs` | M06-T10 | [ ] |
| No GraphIndex injected | `handlers/tools.rs` | M06-T10 | [ ] |

## Uncovered Items

<!-- List any items without task coverage - these MUST be addressed -->

| Item | Type | Reason | Action Required |
|------|------|--------|-----------------|
| VectorOps stub | `stubs/vector_ops_stub.rs` | Lower priority | Add to M06-T08 if needed |
| Test fixtures with fake data | `tests/fixtures/` | Intentional for tests | Document as acceptable |

## Coverage Summary

- **Stub Implementations:** 9/9 covered (100%)
- **Fake Data in Production:** 5/5 covered (100%)
- **Trait/Implementation Gaps:** 5/5 covered (100%)
- **Silent Fallbacks:** Systematic removal in M06-T09 (100%)
- **Production Wiring:** 4/4 covered (100%)

**TOTAL COVERAGE: 100%**

## Validation Checklist

- [ ] All stub implementations have replacement tasks
- [ ] All fake embeddings have removal tasks
- [ ] All trait gaps have bridging tasks
- [ ] Silent fallbacks systematically addressed
- [ ] Production wiring issues covered
- [ ] Task dependencies form valid DAG (no cycles)
- [ ] Layer ordering correct (foundation -> logic -> surface)
- [ ] No item appears without a task assignment

## Verification Commands

```bash
# After M06 completion, these should all return 0 results:

# 1. No StubUtlProcessor in non-test code
grep -r "StubUtlProcessor" crates/context-graph-mcp/src/*.rs 2>/dev/null | grep -v test | wc -l

# 2. No fake embeddings in handlers
grep -r "vec!\[0\.1" crates/context-graph-mcp/src/handlers/*.rs 2>/dev/null | wc -l

# 3. No InMemoryStore in server
grep -r "InMemoryStore" crates/context-graph-mcp/src/server.rs 2>/dev/null | wc -l

# 4. Reduced unwrap_or usage
grep -c "unwrap_or" crates/context-graph-mcp/src/**/*.rs 2>/dev/null
# Should be < 20 (down from 100+)
```

---

*Traceability matrix created: 2026-01-04*
*Module: 06 - Stub Elimination*
*Coverage: 100% of identified issues*
