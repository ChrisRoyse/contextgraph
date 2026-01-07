# SHERLOCK HOLMES CASE FILE #3: EMBEDDING PIPELINE & MODEL LOADING

**Case ID:** SHERLOCK-03-EMBEDDING-PIPELINE
**Date:** 2026-01-07
**Investigator:** Agent #3 of Sherlock Investigation Swarm
**Swarm ID:** swarm_1767796376357_fpuy9v94e

---

## EXECUTIVE SUMMARY

**VERDICT: ALL 13 EMBEDDING MODELS ARE IMPLEMENTED - NO STUBS IN PRODUCTION CODE**

The investigation reveals that all 13 embedding models specified in the constitution are FULLY IMPLEMENTED with real inference code. The "stub" directories observed earlier (hdc, hyperbolic, temporal) are CORRECT - these are CUSTOM models that do not require pretrained weights.

---

## EVIDENCE LOG

### 1. Model Directory Analysis

| Directory | Size | Type | Status |
|-----------|------|------|--------|
| `/models/semantic/` | 6.3GB | E1 Pretrained (e5-large-v2) | PRESENT |
| `/models/multimodal/` | 6.4GB | E10 Pretrained (CLIP) | PRESENT |
| `/models/contextual/` | 3.6GB | Auxiliary (all-mpnet-base-v2) | PRESENT |
| `/models/causal/` | 2.7GB | E5 Pretrained (Longformer) | PRESENT |
| `/models/code/` | 2.4GB | E7 Pretrained (CodeT5p) | PRESENT |
| `/models/late-interaction/` | 1.3GB | E12 Pretrained (ColBERT) | PRESENT |
| `/models/entity/` | 932MB | E11 Pretrained (MiniLM) | PRESENT |
| `/models/sparse/` | 926MB | E6 Pretrained (SPLADE) | PRESENT |
| `/models/graph/` | 846MB | E8 Pretrained (MiniLM) | PRESENT |
| `/models/splade-v3/` | 419MB | E13 Pretrained (SPLADE v3) | PRESENT |
| `/models/hdc/` | 4KB | E9 CUSTOM (no weights needed) | CORRECT |
| `/models/hyperbolic/` | 4KB | Not in 13-model spec | N/A |
| `/models/temporal/` | 4KB | E2-E4 CUSTOM (no weights needed) | CORRECT |

**Total Disk Usage:** ~25.8GB

### 2. Model Implementation Status

All 13 models from ModelId enum are FULLY IMPLEMENTED:

| ModelId | Type | Implementation File | Status |
|---------|------|---------------------|--------|
| E1 Semantic | Pretrained | `models/pretrained/semantic/model.rs` | FULL |
| E2 TemporalRecent | Custom | `models/custom/temporal_recent/model.rs` | FULL |
| E3 TemporalPeriodic | Custom | `models/custom/temporal_periodic/model.rs` | FULL |
| E4 TemporalPositional | Custom | `models/custom/temporal_positional/model.rs` | FULL |
| E5 Causal | Pretrained | `models/pretrained/causal/model.rs` | FULL |
| E6 Sparse | Pretrained | `models/pretrained/sparse/model.rs` | FULL |
| E7 Code | Pretrained | `models/pretrained/code/model.rs` | FULL |
| E8 Graph | Pretrained | `models/pretrained/graph/mod.rs` | FULL |
| E9 Hdc | Custom | `models/custom/hdc/model.rs` | FULL |
| E10 Multimodal | Pretrained | `models/pretrained/multimodal/model.rs` | FULL |
| E11 Entity | Pretrained | `models/pretrained/entity/model.rs` | FULL |
| E12 LateInteraction | Pretrained | `models/pretrained/late_interaction/model.rs` | FULL |
| E13 Splade | Pretrained | `models/pretrained/sparse/model.rs` (reuses E6) | FULL |

### 3. Memory Estimates (FP32)

Source: `/crates/context-graph-embeddings/src/traits/model_factory/memory.rs`

| Model | Memory Estimate | Notes |
|-------|-----------------|-------|
| E1 Semantic | 1.40 GB | Largest dense model |
| E2 TemporalRecent | 15 MB | Smallest (custom) |
| E3 TemporalPeriodic | 15 MB | Smallest (custom) |
| E4 TemporalPositional | 15 MB | Smallest (custom) |
| E5 Causal | 650 MB | Longformer |
| E6 Sparse | 550 MB | SPLADE + MLM head |
| E7 Code | 550 MB | CodeT5p |
| E8 Graph | 120 MB | MiniLM |
| E9 Hdc | 60 MB | Binary hypervectors |
| E10 Multimodal | 1.60 GB | CLIP (largest) |
| E11 Entity | 120 MB | MiniLM |
| E12 LateInteraction | 450 MB | ColBERT |
| E13 Splade | 550 MB | SPLADE v3 |

**Total VRAM Required (FP32):** ~6.1 GB
**Total VRAM Required (FP16):** ~3.0 GB
**Total VRAM Required (INT8):** ~1.5 GB

### 4. Compilation Verification

```
cargo check --package context-graph-embeddings --features cuda
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.66s
```

**Result:** Code compiles successfully with CUDA feature enabled.

### 5. Key Code Paths Verified

- **ModelFactory:** `/crates/context-graph-embeddings/src/models/factory/default_factory.rs`
  - Creates all 13 models via `create_model()`
  - Distinguishes pretrained vs custom models
  - Memory estimates available via `estimate_memory()`

- **WarmLoader Pipeline:** `/crates/context-graph-embeddings/src/warm/integration/pipeline.rs`
  - Loads all 12 models into VRAM at startup (12, not 13 - E13 Splade is optional)
  - Validates each model with test inference
  - Fail-fast on ANY error (exit codes 101-110)

- **ModelId Enum:** `/crates/context-graph-embeddings/src/types/model_id/core.rs`
  - All 13 model IDs defined (E1-E13)
  - Dimension, projected_dimension, is_custom, is_pretrained methods
  - Latency budgets per constitution

---

## GREP SCAN FOR STUBS/TODOS

**Command:** `grep -ri "todo!|unimplemented!|panic!|stub" models/`

**Result:** NO `todo!()` or `unimplemented!()` found in production code. Only `panic!()` in test assertions which is appropriate.

The word "stub" appears in comments explaining:
- Feature flag docs: "Without the feature, models use stub implementations for testing"
- AP-007 compliance notes: "No stub data in prod"

These are DOCUMENTATION, not actual stubs.

---

## CRITICAL FINDINGS

### 1. CUDA Feature MANDATORY

The crate enforces CUDA requirement at compile time:
```
error: [EMB-E001] CUDA_UNAVAILABLE: The 'cuda' feature MUST be enabled.
```

This is CORRECT behavior per Constitution AP-007.

### 2. Custom Models Don't Need Weight Files

Models E2, E3, E4, E9 are mathematical/algorithmic:
- TemporalRecent: Exponential decay (pure math)
- TemporalPeriodic: Fourier basis (pure math)
- TemporalPositional: Sinusoidal PE (pure math)
- HDC: Hypervector operations (binary vectors)

The empty `models/hdc/`, `models/temporal/` directories are CORRECT.

### 3. E13 Splade Reuses E6 Architecture

`SparseModel::new_splade()` creates a model with same architecture as E6 but reports `ModelId::Splade`. Uses different model path: `prithivida_Splade_PP_en_v1`.

### 4. Projection Matrices Required

Sparse models (E6, E13) require projection matrices:
- `/models/sparse/projection.safetensors` (30522D -> 1536D)
- AP-007 enforced: NO hash fallback allowed

---

## VERDICT

**STATUS: INNOCENT**

All 13 embedding models specified in the constitution are FULLY IMPLEMENTED:
- 9 pretrained models with real HuggingFace weights
- 4 custom models with mathematical implementations
- No stubs, todos, or unimplemented code in production paths

**Confidence: HIGH**

---

## GUIDANCE FOR NEXT AGENTS

### For Agent #4 (cudarc Compatibility):
- Codebase uses Candle, NOT cudarc directly
- CUDA feature enables real GPU inference
- Focus verification on `crates/context-graph-embeddings/src/gpu/` module

### For Agent #5 (Integration):
- WarmLoader loads 12 models by default (E13 Splade optional)
- Total VRAM for all models: ~6.1GB FP32 / ~3.0GB FP16
- Pipeline exit codes: 101-110 for different failure modes

---

## MEMORY LOCATIONS STORED

Key: `sherlock/agent3/embedding-findings`
Key: `sherlock/agent3/model-status-summary`

---

## CHAIN OF CUSTODY

| Timestamp | Action | Verified By |
|-----------|--------|-------------|
| 2026-01-07 | Model directories enumerated | HOLMES |
| 2026-01-07 | Implementation files verified | HOLMES |
| 2026-01-07 | Memory estimates documented | HOLMES |
| 2026-01-07 | Compilation verified with --features cuda | HOLMES |
| 2026-01-07 | Grep scan for stubs completed | HOLMES |

---

**CASE CLOSED**

*"When you have eliminated the impossible, whatever remains, however improbable, must be the truth."*
