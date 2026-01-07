# Comprehensive Fix Plan for Embedding Pipeline

## Executive Summary

The embedding pipeline has 4 root causes preventing model loading:
1. **RC1**: Path format mismatch (`E1_Semantic.safetensors` vs `semantic/model.safetensors`)
2. **RC2**: No skip logic for custom models (Temporal*, Hdc) that don't need weight files
3. **RC3**: Splade directory name mismatch (`splade` vs `splade-v3`)
4. **RC4**: Missing sparse projection file (required for 30K->1536D projection)

---

## FIX PLAN

### Fix 1: [RC1] Path Format Correction

**Problem**: Line 94 in `operations.rs` constructs path as `{model_id}.safetensors` (e.g., `E1_Semantic.safetensors`) but actual files are at `{directory}/model.safetensors` (e.g., `semantic/model.safetensors`).

**File**: `/home/cabdru/contextgraph/crates/context-graph-embeddings/src/warm/loader/operations.rs`

**Current Code (Line 94)**:
```rust
let weight_path = config.model_weights_path.join(format!("{}.safetensors", model_id));
```

**Required Change**:
```rust
// Import ModelId for path resolution
use crate::types::ModelId;

// Replace line 94 with:
let model_enum = ModelId::try_from(model_id).map_err(|_| WarmError::ModelNotRegistered {
    model_id: model_id.to_string(),
})?;
let weight_path = model_enum.model_path(&config.model_weights_path).join("model.safetensors");
```

**Additional Required Changes**:

1. Add conversion method in `crates/context-graph-embeddings/src/types/model_id/conversions.rs`:

```rust
impl TryFrom<&str> for ModelId {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "E1_Semantic" | "semantic" => Ok(Self::Semantic),
            "E2_TemporalRecent" | "temporal_recent" => Ok(Self::TemporalRecent),
            "E3_TemporalPeriodic" | "temporal_periodic" => Ok(Self::TemporalPeriodic),
            "E4_TemporalPositional" | "temporal_positional" => Ok(Self::TemporalPositional),
            "E5_Causal" | "causal" => Ok(Self::Causal),
            "E6_Sparse" | "sparse" => Ok(Self::Sparse),
            "E7_Code" | "code" => Ok(Self::Code),
            "E8_Graph" | "graph" => Ok(Self::Graph),
            "E9_HDC" | "hdc" => Ok(Self::Hdc),
            "E10_Multimodal" | "multimodal" => Ok(Self::Multimodal),
            "E11_Entity" | "entity" => Ok(Self::Entity),
            "E12_LateInteraction" | "late_interaction" => Ok(Self::LateInteraction),
            "E13_Splade" | "splade" => Ok(Self::Splade),
            _ => Err(format!("Unknown model ID: {}", s)),
        }
    }
}
```

**Testing**:
```bash
# Verify path construction
cargo test --package context-graph-embeddings test_model_path_construction

# Integration test
ls ./models/semantic/model.safetensors  # Should exist
```

---

### Fix 2: [RC2] Custom Model Handling (Skip Weight Loading)

**Problem**: `load_single_model()` attempts to load weight files for ALL models, including custom implementations (TemporalRecent, TemporalPeriodic, TemporalPositional, Hdc) that compute embeddings mathematically and have no weight files.

**File**: `/home/cabdru/contextgraph/crates/context-graph-embeddings/src/warm/loader/operations.rs`

**Current Code (Lines 49-151)**: Function always tries to load weights.

**Required Change**: Add early return for custom models BEFORE weight loading.

Insert after line 72 (after getting expected_bytes/expected_dimension):

```rust
// Check if this is a custom model (no weight files needed)
let model_enum = ModelId::try_from(model_id).map_err(|_| WarmError::ModelNotRegistered {
    model_id: model_id.to_string(),
})?;

if model_enum.is_custom() {
    tracing::info!(
        "Model {} is custom implementation (no weight file needed), marking as warm",
        model_id
    );

    // Transition directly: Pending -> Loading -> Validating -> Warm
    // Custom models don't need VRAM allocation for weights (they compute on-the-fly)
    {
        let mut reg = registry
            .write()
            .map_err(|_| WarmError::RegistryLockPoisoned)?;
        reg.start_loading(model_id)?;
        reg.update_progress(model_id, 100, 0)?;
        reg.mark_validating(model_id)?;

        // Create a placeholder handle (no actual VRAM allocation for custom models)
        let handle = ModelHandle::new(0, 0, config.cuda_device_id, 0);
        reg.mark_warm(model_id, handle)?;
    }

    return Ok(());
}
```

**Critical Note**: Custom models should NOT allocate VRAM through this path. They compute embeddings mathematically at inference time and may have their own working memory requirements handled separately.

**Testing**:
```bash
# Verify custom models skip weight loading
RUST_LOG=debug cargo test --package context-graph-embeddings test_custom_model_loading 2>&1 | grep "custom implementation"
```

---

### Fix 3: [RC3] Splade Directory Name Mismatch

**Problem**: `repository.rs` line 48 returns `"splade"` but actual directory is `splade-v3`.

**File**: `/home/cabdru/contextgraph/crates/context-graph-embeddings/src/types/model_id/repository.rs`

**Current Code (Line 48)**:
```rust
Self::Splade => "splade",
```

**Required Change**:
```rust
Self::Splade => "splade-v3",
```

**Verification**:
```bash
ls -la /home/cabdru/contextgraph/models/splade-v3/
# Should contain: pytorch_model.bin, config.json, tokenizer.json, etc.
```

**Additional Note**: The splade-v3 directory currently contains `pytorch_model.bin` but NOT `model.safetensors`. This is a secondary issue - see Fix 5 below for conversion.

**Testing**:
```bash
cargo test --package context-graph-embeddings test_splade_directory_name
```

---

### Fix 4: [RC4] Sparse Model Projection Matrix

**Problem**: Sparse models (E6_Sparse, E13_Splade) output 30K-dimensional sparse vectors but need projection to 1536D dense vectors. The `sparse_projection.safetensors` file is missing.

**Options Analysis**:

| Option | Pros | Cons |
|--------|------|------|
| A: Create projection file | One-time cost, fast inference | Requires training or downloading |
| B: Generate at startup | No file dependency | Slow startup (30K x 1536 matrix) |
| C: Use random projection | Fast, deterministic with seed | Suboptimal quality |

**Recommended**: Option A - Create projection matrix file

**Implementation**:

1. **Create projection matrix generation script**:

**File**: `/home/cabdru/contextgraph/scripts/generate_sparse_projection.py`

```python
#!/usr/bin/env python3
"""Generate sparse-to-dense projection matrix for SPLADE models."""

import numpy as np
import torch
from safetensors.torch import save_file

# Constants from ModelId::dimension() and ModelId::projected_dimension()
SPARSE_DIM = 30522  # SPLADE vocab size
DENSE_DIM = 1536    # Projected dimension

def generate_orthogonal_projection(input_dim: int, output_dim: int, seed: int = 42) -> torch.Tensor:
    """Generate orthogonal projection matrix using QR decomposition."""
    np.random.seed(seed)
    # Generate random matrix
    random_matrix = np.random.randn(output_dim, input_dim).astype(np.float32)
    # QR decomposition for orthogonality
    q, r = np.linalg.qr(random_matrix.T)
    # Take first output_dim columns and transpose
    projection = q[:, :output_dim].T
    return torch.from_numpy(projection.astype(np.float32))

def main():
    print(f"Generating projection matrix: {SPARSE_DIM} -> {DENSE_DIM}")

    projection = generate_orthogonal_projection(SPARSE_DIM, DENSE_DIM)
    print(f"Matrix shape: {projection.shape}")
    print(f"Matrix dtype: {projection.dtype}")

    # Save to both sparse and splade-v3 directories
    for directory in ["models/sparse", "models/splade-v3"]:
        output_path = f"{directory}/sparse_projection.safetensors"
        save_file({"projection": projection}, output_path)
        print(f"Saved to: {output_path}")

if __name__ == "__main__":
    main()
```

2. **Update operations.rs to handle sparse projection loading**:

After loading main weights, add projection loading for sparse models:

```rust
// For sparse models, also load projection matrix
if matches!(model_enum, ModelId::Sparse | ModelId::Splade) {
    let projection_path = model_enum.model_path(&config.model_weights_path)
        .join("sparse_projection.safetensors");

    if !projection_path.exists() {
        return Err(WarmError::WeightFileMissing {
            model_id: model_id.to_string(),
            path: projection_path,
        });
    }

    tracing::info!("Loading sparse projection matrix from {:?}", projection_path);
    // Load projection weights (stored separately, used during inference)
    let (proj_bytes, _, _) = load_weights(&projection_path, &format!("{}_projection", model_id))?;
    tracing::debug!("Loaded {} bytes for sparse projection", proj_bytes.len());
}
```

**Testing**:
```bash
# Generate projection matrices
python scripts/generate_sparse_projection.py

# Verify files exist
ls -la models/sparse/sparse_projection.safetensors
ls -la models/splade-v3/sparse_projection.safetensors
```

---

### Fix 5: [SECONDARY] Convert pytorch_model.bin to model.safetensors

**Problem**: `models/splade-v3/` contains `pytorch_model.bin` but not `model.safetensors`.

**File**: `/home/cabdru/contextgraph/scripts/convert_pytorch_to_safetensors.py`

```python
#!/usr/bin/env python3
"""Convert PyTorch model files to SafeTensors format."""

import sys
from pathlib import Path
import torch
from safetensors.torch import save_file

def convert(input_path: Path, output_path: Path):
    print(f"Loading: {input_path}")
    state_dict = torch.load(input_path, map_location="cpu")

    # Handle nested state dicts
    if "state_dict" in state_dict:
        state_dict = state_dict["state_dict"]
    elif "model_state_dict" in state_dict:
        state_dict = state_dict["model_state_dict"]

    print(f"Tensors: {len(state_dict)}")
    print(f"Saving: {output_path}")
    save_file(state_dict, str(output_path))
    print("Done!")

if __name__ == "__main__":
    for model_dir in ["models/splade-v3"]:
        input_path = Path(model_dir) / "pytorch_model.bin"
        output_path = Path(model_dir) / "model.safetensors"
        if input_path.exists() and not output_path.exists():
            convert(input_path, output_path)
```

**Testing**:
```bash
python scripts/convert_pytorch_to_safetensors.py
ls -la models/splade-v3/model.safetensors
```

---

## Implementation Order

| Order | Fix | Dependencies | Risk | Effort |
|-------|-----|--------------|------|--------|
| 1 | Fix 2: Custom Model Skip | None | Low | 30 min |
| 2 | Fix 3: Splade Directory | None | Low | 5 min |
| 3 | Fix 1: Path Format | Fix 3 (for Splade path) | Medium | 1 hour |
| 4 | Fix 5: Convert Splade | None (can run parallel) | Low | 10 min |
| 5 | Fix 4: Sparse Projection | Fix 1 (path must work first) | Medium | 45 min |

**Rationale**:
- Fix 2 first: Immediately eliminates 4 failure points (Temporal*, Hdc)
- Fix 3 is trivial one-line change
- Fix 1 is the core path fix affecting 9 pretrained models
- Fix 5 can run in parallel while coding Fix 1
- Fix 4 depends on path resolution working

---

## Regression Risk Analysis

### High Risk
- **Model path changes**: Any change to path construction affects ALL 9 pretrained models
- **Mitigation**: Add comprehensive path tests before/after changes

### Medium Risk
- **Custom model placeholder handle**: Using `ModelHandle::new(0, 0, ...)` for custom models
- **Mitigation**: Ensure inference paths check for zero VRAM pointer and handle appropriately

### Low Risk
- **Splade directory rename**: Only affects E13_Splade model
- **Sparse projection loading**: Only affects E6_Sparse and E13_Splade

### Breaking Changes to Avoid
- Do NOT change `EMBEDDING_MODEL_IDS` strings (used throughout codebase)
- Do NOT change `ModelId` enum variant names
- Do NOT change expected_bytes in registry (affects VRAM budgeting)

---

## Verification Checklist

After implementing all fixes:

```bash
# 1. Verify all model directories exist
for dir in semantic temporal causal sparse code graph hdc multimodal entity late-interaction splade-v3; do
    ls -la models/$dir/
done

# 2. Verify safetensors files exist for pretrained models
for dir in semantic causal sparse code graph multimodal entity late-interaction splade-v3; do
    ls -la models/$dir/model.safetensors || echo "MISSING: $dir/model.safetensors"
done

# 3. Verify sparse projection files exist
ls -la models/sparse/sparse_projection.safetensors
ls -la models/splade-v3/sparse_projection.safetensors

# 4. Run unit tests
cargo test --package context-graph-embeddings

# 5. Run integration test (requires CUDA)
cargo test --package context-graph-embeddings --features cuda test_warm_load_all_models
```

---

## Files Modified Summary

| File | Changes |
|------|---------|
| `crates/context-graph-embeddings/src/warm/loader/operations.rs` | Path fix, custom model skip, projection loading |
| `crates/context-graph-embeddings/src/types/model_id/repository.rs` | `splade` -> `splade-v3` |
| `crates/context-graph-embeddings/src/types/model_id/conversions.rs` | Add `TryFrom<&str>` impl |
| `scripts/generate_sparse_projection.py` | NEW: Generate projection matrices |
| `scripts/convert_pytorch_to_safetensors.py` | NEW: Convert model formats |
| `models/sparse/sparse_projection.safetensors` | NEW: Projection matrix |
| `models/splade-v3/sparse_projection.safetensors` | NEW: Projection matrix |
| `models/splade-v3/model.safetensors` | NEW: Converted from pytorch_model.bin |

---

## Success Criteria

1. All 13 models transition from `Pending` -> `Warm` state
2. No `WarmError::WeightFileMissing` errors
3. No `WarmError::ModelNotRegistered` errors
4. Custom models (E2, E3, E4, E9) skip weight loading correctly
5. Sparse models (E6, E13) load projection matrices successfully
6. All tests pass: `cargo test --package context-graph-embeddings`

---

*Generated by Architecture Agent | Agent #3/5 | 2026-01-07*
