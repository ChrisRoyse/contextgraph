# TASK-EMB-020: Implement QuantizationRouter

<task_spec id="TASK-EMB-020" version="2.0">

## Metadata

| Field | Value |
|-------|-------|
| **Title** | Implement Per-Embedder QuantizationRouter |
| **Status** | ready |
| **Layer** | logic |
| **Sequence** | 20 |
| **Implements** | REQ-EMB-005 |
| **Depends On** | TASK-EMB-018 (Binary Quantization - COMPLETE) |
| **Blocks** | TASK-EMB-022 (Storage Backend) |
| **Estimated Complexity** | medium |
| **Target Hardware** | RTX 5090, CUDA 13.1, 32GB VRAM |

---

## Critical Context for Implementation

### ABSOLUTE RULES (Constitution AP-007)

1. **NO STUB/MOCK DATA** - All quantization MUST use real algorithms
2. **NO BACKWARDS COMPATIBILITY** - Fail fast with clear error codes
3. **NO FLOAT32 PASSTHROUGH** - Every embedder uses assigned compression
4. **ROBUST ERROR LOGGING** - Every failure logs context for debugging

### Current Codebase State (As of 2026-01-06)

**WHAT EXISTS:**
```
crates/context-graph-embeddings/src/quantization/
├── binary.rs          # ✅ FULLY IMPLEMENTED (620 lines with tests)
├── edge_case_verification.rs  # ✅ Test module
├── mod.rs             # ✅ Module exports
└── types.rs           # ✅ Data structures + marker structs
```

**WHAT DOES NOT EXIST:**
- `router.rs` - **YOU MUST CREATE THIS**
- `pq8.rs` - PQ-8 encoder NOT implemented (marker struct only)
- `float8.rs` - Float8 encoder NOT implemented (marker struct only)

### Implementation Reality

| Quantization Method | Status | Affected ModelIds |
|---------------------|--------|-------------------|
| **Binary** | ✅ COMPLETE in `binary.rs` | E9_Hdc |
| **Float8E4M3** | ❌ NOT IMPLEMENTED | E2, E3, E4, E8, E11 |
| **PQ8** | ❌ NOT IMPLEMENTED | E1, E5, E7, E10 |
| **SparseNative** | ⏭️ PASS-THROUGH (no compression) | E6, E13 |
| **TokenPruning** | ⏭️ OUT OF SCOPE | E12 |

---

## Input Context Files

| Purpose | File Path | What to Read |
|---------|-----------|--------------|
| **Constitution** | `docs2/constitution.yaml` | Section `AP-007`, `embeddings.quantization` |
| **Quantization Types** | `crates/context-graph-embeddings/src/quantization/types.rs` | `QuantizationMethod`, `QuantizedEmbedding`, `QuantizationMetadata` |
| **Binary Implementation** | `crates/context-graph-embeddings/src/quantization/binary.rs` | `BinaryEncoder::quantize()`, `BinaryEncoder::dequantize()`, `BinaryEncoder::hamming_distance()` |
| **Module Exports** | `crates/context-graph-embeddings/src/quantization/mod.rs` | Current exports |
| **Error Types** | `crates/context-graph-embeddings/src/error.rs` | `EmbeddingError` variants |
| **ModelId Enum** | `crates/context-graph-embeddings/src/types/model_id.rs` | All 13 ModelId variants |
| **Tech Spec** | `docs2/codecheck/embeddingissues/TECH-EMB-003-quantization.md` | Full quantization specification |

---

## Prerequisites Checklist

- [x] TASK-EMB-018 completed (Binary Quantization works) - **VERIFIED IN CODEBASE**
- [x] `QuantizationMethod::for_model_id()` exists in types.rs - **VERIFIED AT LINE 57-82**
- [x] `BinaryEncoder` has `quantize()` method - **VERIFIED AT LINE 87 in binary.rs**
- [x] `BinaryEncoder` has `dequantize()` method - **VERIFIED AT LINE 175 in binary.rs**
- [ ] `Float8Encoder` has implementation - **NOT AVAILABLE - MUST ERROR**
- [ ] `PQ8Codebook` has implementation - **NOT AVAILABLE - MUST ERROR**

---

## Scope

### In Scope

1. Create `crates/context-graph-embeddings/src/quantization/router.rs`
2. Implement `QuantizationRouter` struct with:
   - `new()` constructor
   - `method_for(ModelId)` - returns quantization method
   - `quantize(ModelId, &[f32])` - dispatches to correct encoder
   - `dequantize(ModelId, &QuantizedEmbedding)` - reverses quantization
   - `can_quantize(ModelId)` - returns whether encoder is implemented
3. Full support for Binary quantization (E9_Hdc)
4. Clear error returns for unimplemented methods (PQ8, Float8)
5. Pass-through handling for SparseNative (E6, E13)
6. Error handling for TokenPruning (E12 - out of scope)

### Out of Scope

- Implementing PQ8 quantization (separate task)
- Implementing Float8 quantization (separate task)
- Token pruning for E12 (separate task)
- Codebook loading/training for PQ8

---

## Definition of Done

### File to Create: `router.rs`

**Location:** `crates/context-graph-embeddings/src/quantization/router.rs`

```rust
//! QuantizationRouter - Routes embeddings to Constitutional quantization methods.
//!
//! # Constitution Alignment (AP-007)
//!
//! Every embedder MUST use its assigned quantization method:
//! - E1, E5, E7, E10: PQ-8 (32x compression)
//! - E2, E3, E4, E8, E11: Float8 E4M3 (4x compression)
//! - E9: Binary (32x compression) ← IMPLEMENTED
//! - E6, E13: Sparse native (pass-through)
//! - E12: Token pruning (handled separately)
//!
//! # CRITICAL: No Float32 Storage
//! Float32 passthrough is FORBIDDEN. All embeddings MUST be quantized.

use super::binary::BinaryEncoder;
use super::types::{QuantizationMetadata, QuantizationMethod, QuantizedEmbedding};
use crate::error::EmbeddingError;
use crate::types::ModelId;

/// Error code for router failures.
pub const EMB_E020_ROUTER: &str = "EMB-E020";

/// Routes embeddings to their Constitutional quantization method.
///
/// # Current Implementation Status
///
/// | Method | Status | Error if Unavailable |
/// |--------|--------|----------------------|
/// | Binary | ✅ Implemented | N/A |
/// | Float8 | ❌ Not Implemented | `QuantizerNotImplemented` |
/// | PQ8 | ❌ Not Implemented | `QuantizerNotImplemented` |
/// | Sparse | ⏭️ Pass-through | N/A |
/// | TokenPruning | ⏭️ Out of scope | `UnsupportedMethod` |
#[derive(Debug, Default)]
pub struct QuantizationRouter {
    /// Binary encoder (stateless, always available).
    binary_encoder: BinaryEncoder,
}

impl QuantizationRouter {
    /// Create a new QuantizationRouter.
    ///
    /// # Returns
    /// Router instance ready for Binary quantization.
    /// PQ8 and Float8 will return errors until implemented.
    pub fn new() -> Self {
        log::info!(
            "[{}] QuantizationRouter initialized. Binary: READY, Float8: NOT_IMPL, PQ8: NOT_IMPL",
            EMB_E020_ROUTER
        );
        Self {
            binary_encoder: BinaryEncoder::default(),
        }
    }

    /// Get quantization method for a model.
    ///
    /// This delegates to `QuantizationMethod::for_model_id()` from types.rs.
    #[must_use]
    pub fn method_for(&self, model_id: ModelId) -> QuantizationMethod {
        QuantizationMethod::for_model_id(model_id)
    }

    /// Check if quantization is available for a model.
    ///
    /// # Returns
    /// `true` if the encoder for this model's method is implemented.
    #[must_use]
    pub fn can_quantize(&self, model_id: ModelId) -> bool {
        match self.method_for(model_id) {
            QuantizationMethod::Binary => true, // ✅ Implemented
            QuantizationMethod::SparseNative => true, // Pass-through
            QuantizationMethod::Float8E4M3 => false, // ❌ Not implemented
            QuantizationMethod::PQ8 => false, // ❌ Not implemented
            QuantizationMethod::TokenPruning => false, // Out of scope
        }
    }

    /// Quantize embedding using Constitutional method for the model.
    ///
    /// # Arguments
    /// * `model_id` - Which embedder this embedding is from
    /// * `embedding` - Raw f32 embedding vector
    ///
    /// # Returns
    /// * `Ok(QuantizedEmbedding)` - Compressed embedding
    /// * `Err(EmbeddingError)` - If encoder not implemented or quantization fails
    ///
    /// # CRITICAL: No Float32 Output
    /// This function ALWAYS quantizes. There is no "pass-through" mode for dense vectors.
    ///
    /// # Errors
    /// * `EmbeddingError::QuantizerNotImplemented` - Float8/PQ8 not yet available
    /// * `EmbeddingError::InvalidInput` - Dimension mismatch or invalid data
    pub fn quantize(
        &self,
        model_id: ModelId,
        embedding: &[f32],
    ) -> Result<QuantizedEmbedding, EmbeddingError> {
        let method = self.method_for(model_id);

        log::debug!(
            "[{}] Quantizing {:?} with method {:?}, dim={}",
            EMB_E020_ROUTER,
            model_id,
            method,
            embedding.len()
        );

        match method {
            QuantizationMethod::Binary => {
                // E9_Hdc: Binary quantization (32x compression)
                self.binary_encoder
                    .quantize(embedding, 0.0) // threshold=0.0 for sign-based
                    .map_err(|e| {
                        log::error!(
                            "[{}] Binary quantize failed for {:?}: {}",
                            EMB_E020_ROUTER,
                            model_id,
                            e
                        );
                        EmbeddingError::QuantizationFailed {
                            model_id,
                            reason: e.to_string(),
                        }
                    })
            }

            QuantizationMethod::Float8E4M3 => {
                // E2, E3, E4, E8, E11: Float8 NOT IMPLEMENTED
                log::error!(
                    "[{}] Float8 quantization NOT IMPLEMENTED for {:?}. \
                     Affected models: TemporalRecent, TemporalPeriodic, TemporalPositional, Graph, Entity",
                    EMB_E020_ROUTER,
                    model_id
                );
                Err(EmbeddingError::QuantizerNotImplemented {
                    model_id,
                    method: "Float8E4M3".to_string(),
                })
            }

            QuantizationMethod::PQ8 => {
                // E1, E5, E7, E10: PQ-8 NOT IMPLEMENTED
                log::error!(
                    "[{}] PQ8 quantization NOT IMPLEMENTED for {:?}. \
                     Affected models: Semantic, Causal, Code, Multimodal",
                    EMB_E020_ROUTER,
                    model_id
                );
                Err(EmbeddingError::QuantizerNotImplemented {
                    model_id,
                    method: "PQ8".to_string(),
                })
            }

            QuantizationMethod::SparseNative => {
                // E6, E13: Sparse vectors use native format
                // Return error because sparse should not be passed as dense f32
                log::warn!(
                    "[{}] SparseNative method called with dense f32 for {:?}. \
                     Sparse embeddings should use dedicated sparse storage.",
                    EMB_E020_ROUTER,
                    model_id
                );
                Err(EmbeddingError::InvalidInput {
                    model_id,
                    reason: "Sparse embeddings should not use dense quantization path".to_string(),
                })
            }

            QuantizationMethod::TokenPruning => {
                // E12: Token pruning handled separately
                log::error!(
                    "[{}] TokenPruning is out of scope for QuantizationRouter. \
                     E12_LateInteraction requires dedicated token pruning logic.",
                    EMB_E020_ROUTER,
                    model_id
                );
                Err(EmbeddingError::UnsupportedOperation {
                    model_id,
                    operation: "TokenPruning".to_string(),
                })
            }
        }
    }

    /// Dequantize embedding back to f32.
    ///
    /// # Arguments
    /// * `model_id` - Which embedder this embedding is from
    /// * `quantized` - Quantized embedding to decompress
    ///
    /// # Returns
    /// * `Ok(Vec<f32>)` - Reconstructed embedding (may have precision loss)
    /// * `Err(EmbeddingError)` - If decoder not implemented or dequantization fails
    pub fn dequantize(
        &self,
        model_id: ModelId,
        quantized: &QuantizedEmbedding,
    ) -> Result<Vec<f32>, EmbeddingError> {
        log::debug!(
            "[{}] Dequantizing {:?} with method {:?}",
            EMB_E020_ROUTER,
            model_id,
            quantized.method
        );

        match quantized.method {
            QuantizationMethod::Binary => {
                self.binary_encoder.dequantize(quantized).map_err(|e| {
                    log::error!(
                        "[{}] Binary dequantize failed for {:?}: {}",
                        EMB_E020_ROUTER,
                        model_id,
                        e
                    );
                    EmbeddingError::DequantizationFailed {
                        model_id,
                        reason: e.to_string(),
                    }
                })
            }

            QuantizationMethod::Float8E4M3 => {
                log::error!(
                    "[{}] Float8 dequantization NOT IMPLEMENTED for {:?}",
                    EMB_E020_ROUTER,
                    model_id
                );
                Err(EmbeddingError::QuantizerNotImplemented {
                    model_id,
                    method: "Float8E4M3".to_string(),
                })
            }

            QuantizationMethod::PQ8 => {
                log::error!(
                    "[{}] PQ8 dequantization NOT IMPLEMENTED for {:?}",
                    EMB_E020_ROUTER,
                    model_id
                );
                Err(EmbeddingError::QuantizerNotImplemented {
                    model_id,
                    method: "PQ8".to_string(),
                })
            }

            QuantizationMethod::SparseNative | QuantizationMethod::TokenPruning => {
                Err(EmbeddingError::UnsupportedOperation {
                    model_id,
                    operation: format!("{:?}", quantized.method),
                })
            }
        }
    }

    /// Get expected compressed size in bytes for a model.
    #[must_use]
    pub fn expected_size(&self, model_id: ModelId, original_dim: usize) -> usize {
        match self.method_for(model_id) {
            QuantizationMethod::Binary => (original_dim + 7) / 8, // 1 bit per element
            QuantizationMethod::Float8E4M3 => original_dim,       // 1 byte per element
            QuantizationMethod::PQ8 => 8,                         // Always 8 bytes
            QuantizationMethod::SparseNative => original_dim * 8, // Approximate
            QuantizationMethod::TokenPruning => original_dim * 2, // ~50%
        }
    }
}

/// Constitutional quantization assignments for all 13 embedders.
pub const QUANTIZATION_ASSIGNMENTS: &[(ModelId, QuantizationMethod, &str, bool)] = &[
    (ModelId::Semantic, QuantizationMethod::PQ8, "E1: 32x compression", false),
    (ModelId::TemporalRecent, QuantizationMethod::Float8E4M3, "E2: 4x compression", false),
    (ModelId::TemporalPeriodic, QuantizationMethod::Float8E4M3, "E3: 4x compression", false),
    (ModelId::TemporalPositional, QuantizationMethod::Float8E4M3, "E4: 4x compression", false),
    (ModelId::Causal, QuantizationMethod::PQ8, "E5: 32x compression", false),
    (ModelId::Sparse, QuantizationMethod::SparseNative, "E6: native sparse", true),
    (ModelId::Code, QuantizationMethod::PQ8, "E7: 32x compression", false),
    (ModelId::Graph, QuantizationMethod::Float8E4M3, "E8: 4x compression", false),
    (ModelId::Hdc, QuantizationMethod::Binary, "E9: 32x compression", true), // ✅ IMPLEMENTED
    (ModelId::Multimodal, QuantizationMethod::PQ8, "E10: 32x compression", false),
    (ModelId::Entity, QuantizationMethod::Float8E4M3, "E11: 4x compression", false),
    (ModelId::LateInteraction, QuantizationMethod::TokenPruning, "E12: ~50% reduction", false),
    (ModelId::Splade, QuantizationMethod::SparseNative, "E13: native sparse", true),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Test: Router initializes successfully.
    #[test]
    fn test_router_new() {
        let router = QuantizationRouter::new();
        assert!(router.can_quantize(ModelId::Hdc));
    }

    /// Test: All 13 ModelIds have method assignments.
    #[test]
    fn test_all_model_ids_have_method() {
        let router = QuantizationRouter::new();
        let all_models = ModelId::all();
        assert_eq!(all_models.len(), 13, "Expected 13 ModelId variants");

        for model_id in all_models {
            let method = router.method_for(*model_id);
            // Just verify it returns without panic
            let _ = method.compression_ratio();
        }
    }

    /// Test: Binary quantization works for E9_Hdc.
    #[test]
    fn test_binary_quantization_e9_hdc() {
        let router = QuantizationRouter::new();

        // Create test embedding (1024 dimensions for HDC)
        let embedding: Vec<f32> = (0..1024)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect();

        let result = router.quantize(ModelId::Hdc, &embedding);
        assert!(result.is_ok(), "Binary quantization should succeed for Hdc");

        let quantized = result.unwrap();
        assert_eq!(quantized.method, QuantizationMethod::Binary);
        assert_eq!(quantized.original_dim, 1024);
        // Binary: 1024 bits = 128 bytes
        assert_eq!(quantized.data.len(), 128);
    }

    /// Test: Dequantize reverses binary quantization.
    #[test]
    fn test_binary_round_trip() {
        let router = QuantizationRouter::new();

        // Create bipolar test vector
        let original: Vec<f32> = (0..512)
            .map(|i| if i % 3 == 0 { 1.0 } else { -1.0 })
            .collect();

        let quantized = router.quantize(ModelId::Hdc, &original).unwrap();
        let reconstructed = router.dequantize(ModelId::Hdc, &quantized).unwrap();

        // Binary reconstruction produces +1/-1 values
        assert_eq!(reconstructed.len(), original.len());
        for (orig, recon) in original.iter().zip(reconstructed.iter()) {
            if *orig > 0.0 {
                assert_eq!(*recon, 1.0);
            } else {
                assert_eq!(*recon, -1.0);
            }
        }
    }

    /// Test: PQ8 returns QuantizerNotImplemented error.
    #[test]
    fn test_pq8_not_implemented() {
        let router = QuantizationRouter::new();
        let embedding = vec![0.5f32; 1024];

        let result = router.quantize(ModelId::Semantic, &embedding);
        assert!(result.is_err());

        match result.unwrap_err() {
            EmbeddingError::QuantizerNotImplemented { model_id, method } => {
                assert_eq!(model_id, ModelId::Semantic);
                assert_eq!(method, "PQ8");
            }
            e => panic!("Expected QuantizerNotImplemented, got {:?}", e),
        }
    }

    /// Test: Float8 returns QuantizerNotImplemented error.
    #[test]
    fn test_float8_not_implemented() {
        let router = QuantizationRouter::new();
        let embedding = vec![0.5f32; 768];

        let result = router.quantize(ModelId::TemporalRecent, &embedding);
        assert!(result.is_err());

        match result.unwrap_err() {
            EmbeddingError::QuantizerNotImplemented { model_id, method } => {
                assert_eq!(model_id, ModelId::TemporalRecent);
                assert_eq!(method, "Float8E4M3");
            }
            e => panic!("Expected QuantizerNotImplemented, got {:?}", e),
        }
    }

    /// Test: SparseNative returns InvalidInput for dense vectors.
    #[test]
    fn test_sparse_rejects_dense() {
        let router = QuantizationRouter::new();
        let embedding = vec![0.1f32; 30522];

        let result = router.quantize(ModelId::Sparse, &embedding);
        assert!(result.is_err());

        match result.unwrap_err() {
            EmbeddingError::InvalidInput { model_id, .. } => {
                assert_eq!(model_id, ModelId::Sparse);
            }
            e => panic!("Expected InvalidInput, got {:?}", e),
        }
    }

    /// Test: can_quantize returns correct availability.
    #[test]
    fn test_can_quantize() {
        let router = QuantizationRouter::new();

        // Binary: available
        assert!(router.can_quantize(ModelId::Hdc));

        // Sparse: available (pass-through)
        assert!(router.can_quantize(ModelId::Sparse));
        assert!(router.can_quantize(ModelId::Splade));

        // PQ8: not available
        assert!(!router.can_quantize(ModelId::Semantic));
        assert!(!router.can_quantize(ModelId::Causal));
        assert!(!router.can_quantize(ModelId::Code));
        assert!(!router.can_quantize(ModelId::Multimodal));

        // Float8: not available
        assert!(!router.can_quantize(ModelId::TemporalRecent));
        assert!(!router.can_quantize(ModelId::Graph));
        assert!(!router.can_quantize(ModelId::Entity));

        // TokenPruning: not available
        assert!(!router.can_quantize(ModelId::LateInteraction));
    }

    /// Test: expected_size calculations.
    #[test]
    fn test_expected_size() {
        let router = QuantizationRouter::new();

        // Binary: 1 bit per element
        assert_eq!(router.expected_size(ModelId::Hdc, 1024), 128); // 1024/8

        // Float8: 1 byte per element
        assert_eq!(router.expected_size(ModelId::Graph, 768), 768);

        // PQ8: always 8 bytes
        assert_eq!(router.expected_size(ModelId::Semantic, 1024), 8);
    }
}
```

---

## Files to Create

| File Path | Description |
|-----------|-------------|
| `crates/context-graph-embeddings/src/quantization/router.rs` | QuantizationRouter implementation |

## Files to Modify

| File Path | Change |
|-----------|--------|
| `crates/context-graph-embeddings/src/quantization/mod.rs` | Add `pub mod router;` and export `QuantizationRouter` |
| `crates/context-graph-embeddings/src/error.rs` | Add error variants if not present: `QuantizerNotImplemented`, `QuantizationFailed`, `DequantizationFailed`, `UnsupportedOperation` |

### mod.rs Changes

```rust
// ADD to crates/context-graph-embeddings/src/quantization/mod.rs

pub mod router;

// ADD to exports:
pub use router::QuantizationRouter;
```

### error.rs Changes (if not present)

```rust
// ADD these variants to EmbeddingError enum if they don't exist:

#[error("[{model_id:?}] Quantizer not implemented: {method}")]
QuantizerNotImplemented {
    model_id: ModelId,
    method: String,
},

#[error("[{model_id:?}] Quantization failed: {reason}")]
QuantizationFailed {
    model_id: ModelId,
    reason: String,
},

#[error("[{model_id:?}] Dequantization failed: {reason}")]
DequantizationFailed {
    model_id: ModelId,
    reason: String,
},

#[error("[{model_id:?}] Unsupported operation: {operation}")]
UnsupportedOperation {
    model_id: ModelId,
    operation: String,
},

#[error("[{model_id:?}] Invalid input: {reason}")]
InvalidInput {
    model_id: ModelId,
    reason: String,
},
```

---

## Validation Criteria

### Must Pass

- [ ] `cargo check -p context-graph-embeddings` passes
- [ ] `cargo test -p context-graph-embeddings quantization::router -- --nocapture` passes
- [ ] All 13 ModelIds have method assignments
- [ ] Binary quantization works for E9_Hdc
- [ ] PQ8/Float8 return `QuantizerNotImplemented` error
- [ ] SparseNative rejects dense vectors
- [ ] No warnings about unused code

### Must NOT

- [ ] No stub/mock data anywhere
- [ ] No Float32 passthrough
- [ ] No panics on invalid input (return errors)

---

## Full State Verification Protocol

### Source of Truth

| Verification Point | Command/Location |
|--------------------|------------------|
| Router module exists | `ls crates/context-graph-embeddings/src/quantization/router.rs` |
| Module exported | `grep "pub mod router" crates/context-graph-embeddings/src/quantization/mod.rs` |
| Compilation | `cargo build -p context-graph-embeddings 2>&1` |
| Tests pass | `cargo test -p context-graph-embeddings quantization::router 2>&1` |

### Execute & Inspect

Run these commands and verify outputs:

```bash
# 1. Verify file created
ls -la crates/context-graph-embeddings/src/quantization/router.rs
# EXPECTED: File exists with non-zero size

# 2. Verify module export
grep -n "pub mod router" crates/context-graph-embeddings/src/quantization/mod.rs
# EXPECTED: Line showing "pub mod router;"

# 3. Verify public export
grep -n "pub use router::QuantizationRouter" crates/context-graph-embeddings/src/quantization/mod.rs
# EXPECTED: Line showing the export

# 4. Build succeeds
cargo build -p context-graph-embeddings 2>&1 | tail -5
# EXPECTED: "Finished" or "Compiling" messages, NO errors

# 5. Run tests
cargo test -p context-graph-embeddings quantization::router -- --nocapture 2>&1
# EXPECTED: "test result: ok" with all tests passing
```

### Three Edge Cases (MANDATORY)

**Edge Case 1: Empty embedding**
```rust
#[test]
fn test_edge_empty_embedding() {
    let router = QuantizationRouter::new();
    let empty: Vec<f32> = vec![];

    let result = router.quantize(ModelId::Hdc, &empty);
    // Should succeed but produce empty output OR fail gracefully
    // Verify it does NOT panic
    match result {
        Ok(q) => assert_eq!(q.original_dim, 0),
        Err(_) => {} // Error is acceptable
    }
}
```

**Edge Case 2: Maximum dimension**
```rust
#[test]
fn test_edge_max_dimension() {
    let router = QuantizationRouter::new();
    // 65536 dimensions (2^16) - large but realistic
    let large: Vec<f32> = (0..65536).map(|i| (i as f32).sin()).collect();

    let result = router.quantize(ModelId::Hdc, &large);
    assert!(result.is_ok());
    let q = result.unwrap();
    assert_eq!(q.data.len(), 65536 / 8); // 8192 bytes
}
```

**Edge Case 3: All same value**
```rust
#[test]
fn test_edge_all_same_value() {
    let router = QuantizationRouter::new();
    // All zeros should produce all-zero bits
    let all_zeros = vec![0.0f32; 256];

    let result = router.quantize(ModelId::Hdc, &all_zeros);
    assert!(result.is_ok());
    let q = result.unwrap();

    // With threshold=0.0, all zeros → all 0 bits
    assert!(q.data.iter().all(|&b| b == 0));
}
```

### Evidence of Success Logs

After implementation, run this command to collect evidence:

```bash
cd /home/cabdru/contextgraph
echo "=== TASK-EMB-020 VERIFICATION ===" > /tmp/task-emb-020-evidence.log
echo "" >> /tmp/task-emb-020-evidence.log

echo "### File Exists ###" >> /tmp/task-emb-020-evidence.log
ls -la crates/context-graph-embeddings/src/quantization/router.rs >> /tmp/task-emb-020-evidence.log 2>&1
echo "" >> /tmp/task-emb-020-evidence.log

echo "### Module Exported ###" >> /tmp/task-emb-020-evidence.log
grep -n "router" crates/context-graph-embeddings/src/quantization/mod.rs >> /tmp/task-emb-020-evidence.log 2>&1
echo "" >> /tmp/task-emb-020-evidence.log

echo "### Build Output ###" >> /tmp/task-emb-020-evidence.log
cargo build -p context-graph-embeddings 2>&1 | tail -10 >> /tmp/task-emb-020-evidence.log
echo "" >> /tmp/task-emb-020-evidence.log

echo "### Test Results ###" >> /tmp/task-emb-020-evidence.log
cargo test -p context-graph-embeddings quantization::router 2>&1 >> /tmp/task-emb-020-evidence.log
echo "" >> /tmp/task-emb-020-evidence.log

echo "### Public API Exported ###" >> /tmp/task-emb-020-evidence.log
grep -r "QuantizationRouter" crates/context-graph-embeddings/src/lib.rs >> /tmp/task-emb-020-evidence.log 2>&1

cat /tmp/task-emb-020-evidence.log
```

---

## Manual Verification Checklist

**After implementation, YOU MUST manually verify:**

- [ ] **File physically exists**: Open `crates/context-graph-embeddings/src/quantization/router.rs` in editor
- [ ] **Tests actually ran**: Check test output shows "test_router_new", "test_binary_quantization_e9_hdc", etc.
- [ ] **No warnings**: Build output shows no "warning:" lines for router.rs
- [ ] **Error variants exist**: Check `error.rs` contains `QuantizerNotImplemented` variant
- [ ] **Re-export works**: Run `cargo doc -p context-graph-embeddings --open` and verify `QuantizationRouter` appears

---

## Test Commands

```bash
cd /home/cabdru/contextgraph

# Check compilation
cargo check -p context-graph-embeddings

# Run router tests
cargo test -p context-graph-embeddings quantization::router -- --nocapture

# Run all quantization tests
cargo test -p context-graph-embeddings quantization:: -- --nocapture

# Check for warnings
cargo clippy -p context-graph-embeddings -- -W clippy::all 2>&1 | grep -E "(router|warning)"
```

---

## Pseudo Code Summary

```
router.rs:
  struct QuantizationRouter:
    binary_encoder: BinaryEncoder

  impl QuantizationRouter:
    new() → Self
      Log "Router initialized"
      Return Self { binary_encoder: BinaryEncoder::default() }

    method_for(model_id) → QuantizationMethod
      Delegate to QuantizationMethod::for_model_id()

    can_quantize(model_id) → bool
      Match method_for(model_id):
        Binary | SparseNative → true
        Float8 | PQ8 | TokenPruning → false

    quantize(model_id, embedding) → Result<QuantizedEmbedding, Error>
      Match method_for(model_id):
        Binary → binary_encoder.quantize(embedding, 0.0)
        Float8 → Error(QuantizerNotImplemented)
        PQ8 → Error(QuantizerNotImplemented)
        Sparse → Error(InvalidInput "use sparse path")
        TokenPruning → Error(UnsupportedOperation)

    dequantize(model_id, quantized) → Result<Vec<f32>, Error>
      Match quantized.method:
        Binary → binary_encoder.dequantize(quantized)
        Float8 → Error(QuantizerNotImplemented)
        PQ8 → Error(QuantizerNotImplemented)
        _ → Error(UnsupportedOperation)

    expected_size(model_id, dim) → usize
      Match method_for(model_id):
        Binary → (dim + 7) / 8
        Float8 → dim
        PQ8 → 8
        _ → dim * estimate

  QUANTIZATION_ASSIGNMENTS: Array of (ModelId, Method, Description, IsImplemented)
```

---

## Dependencies Graph

```
TASK-EMB-018 (Binary) ──────────────────┐
        ✅ COMPLETE                      │
                                         ▼
                              TASK-EMB-020 (Router)
                                    │ YOU ARE HERE
                                    ▼
                              TASK-EMB-022 (Storage Backend)
                                    │
                                    ▼
                              [Surface Layer Tasks]
```

---

## Future Work (Out of Scope for This Task)

When PQ8 and Float8 are implemented:
1. Add `pq8_quantizers: HashMap<ModelId, Arc<PQ8Quantizer>>` field
2. Add `float8_encoder: Float8Encoder` field
3. Update `new()` to load codebooks
4. Update `quantize()`/`dequantize()` to use new encoders
5. Update `can_quantize()` to return `true` for these methods

</task_spec>
