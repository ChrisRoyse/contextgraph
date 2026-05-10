//! Constitution-defined expected values for dimension tests.
//!
//! These constants serve as the single source of truth for all dimension
//! validation tests. Any mismatch with these values is a critical error.

use context_graph_embeddings::{ModelId, QuantizationMethod};

// =============================================================================
// CONSTITUTION-DEFINED EXPECTED VALUES (Fail-Fast Reference)
// =============================================================================

/// Expected native dimensions for all 15 `ModelId::all()` variants.
/// These are the raw output dimensions from each model before projection.
pub const EXPECTED_NATIVE_DIMS: [(ModelId, usize); 15] = [
    (ModelId::Semantic, 1024),          // E1
    (ModelId::TemporalRecent, 512),     // E2
    (ModelId::TemporalPeriodic, 512),   // E3
    (ModelId::TemporalPositional, 512), // E4
    (ModelId::Causal, 768),             // E5
    (ModelId::Sparse, 30522),           // E6
    (ModelId::Code, 1536),              // E7
    (ModelId::Graph, 1024),             // E8 (e5-large-v2, upgraded from MiniLM 384D)
    (ModelId::Hdc, 10000),              // E9
    (ModelId::Contextual, 768),         // E10
    (ModelId::Entity, 384),             // E11 (legacy MiniLM-L6-v2; production uses Kepler 768D)
    (ModelId::LateInteraction, 128),    // E12
    (ModelId::Splade, 30522),           // E13
    (ModelId::Kepler, 768),             // E11 production (KEPLER RoBERTa-base)
    (ModelId::BgeM3Dense, 1024),        // E14
];

/// Expected projected dimensions for all 15 `ModelId::all()` variants.
/// These are the dimensions used for Multi-Array Storage.
pub const EXPECTED_PROJECTED_DIMS: [(ModelId, usize); 15] = [
    (ModelId::Semantic, 1024),          // E1 - no projection
    (ModelId::TemporalRecent, 512),     // E2 - no projection
    (ModelId::TemporalPeriodic, 512),   // E3 - no projection
    (ModelId::TemporalPositional, 512), // E4 - no projection
    (ModelId::Causal, 768),             // E5 - no projection
    (ModelId::Sparse, 1536),            // E6 - 30K -> 1536
    (ModelId::Code, 1536),              // E7 - native 1536D
    (ModelId::Graph, 1024),             // E8 - e5-large-v2 (upgraded from MiniLM 384D)
    (ModelId::Hdc, 1024),               // E9 - 10K -> 1024
    (ModelId::Contextual, 768),         // E10 - no projection
    (ModelId::Entity, 384),             // E11 - legacy MiniLM (production uses Kepler 768D)
    (ModelId::LateInteraction, 128),    // E12 - no projection
    (ModelId::Splade, 1536),            // E13 - 30K -> 1536
    (ModelId::Kepler, 768),             // E11 production - no projection
    (ModelId::BgeM3Dense, 1024),        // E14 - native 1024D dense head
];

/// Expected quantization methods for all 15 `ModelId::all()` variants.
pub const EXPECTED_QUANTIZATION: [(ModelId, QuantizationMethod); 15] = [
    (ModelId::Semantic, QuantizationMethod::PQ8), // E1
    (ModelId::TemporalRecent, QuantizationMethod::Float8E4M3), // E2
    (ModelId::TemporalPeriodic, QuantizationMethod::Float8E4M3), // E3
    (ModelId::TemporalPositional, QuantizationMethod::Float8E4M3), // E4
    (ModelId::Causal, QuantizationMethod::PQ8),   // E5
    (ModelId::Sparse, QuantizationMethod::SparseNative), // E6
    (ModelId::Code, QuantizationMethod::PQ8),     // E7
    (ModelId::Graph, QuantizationMethod::Float8E4M3), // E8
    (ModelId::Hdc, QuantizationMethod::Binary),   // E9
    (ModelId::Contextual, QuantizationMethod::PQ8), // E10
    (ModelId::Entity, QuantizationMethod::Float8E4M3), // E11
    (ModelId::LateInteraction, QuantizationMethod::TokenPruning), // E12
    (ModelId::Splade, QuantizationMethod::SparseNative), // E13
    (ModelId::Kepler, QuantizationMethod::PQ8),   // E11 production (768D, same as E5/E10)
    (ModelId::BgeM3Dense, QuantizationMethod::PQ8), // E14
];

/// Expected total dimension sum.
/// Updated: E8 upgraded 384->1024 (e5-large-v2); E11 remains legacy MiniLM
/// 384D; Kepler adds 768D; E14 BGE-M3 dense adds 1024D.
pub const EXPECTED_TOTAL_DIMENSION: usize = 13056;

/// Expected `ModelId::all()` count. Production arrays use `ModelId::production()`
/// when they need 14 active models without legacy `Entity`.
pub const EXPECTED_MODEL_COUNT: usize = 15;
