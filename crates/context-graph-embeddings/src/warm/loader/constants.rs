//! Constants for the warm model loader.
//!
//! Contains model size definitions and VRAM budget constants.

/// One gigabyte in bytes.
pub const GB: usize = 1024 * 1024 * 1024;

/// Default expected dimension for embedding models.
pub const DEFAULT_EMBEDDING_DIMENSION: usize = 768;

/// Expected model sizes in bytes (FP16, from spec).
/// These are approximate sizes for budget planning.
pub const MODEL_SIZES: &[(&str, usize)] = &[
    ("E1_Semantic", 600 * 1024 * 1024),           // 600MB
    ("E2_TemporalRecent", 400 * 1024 * 1024),     // 400MB
    ("E3_TemporalPeriodic", 400 * 1024 * 1024),   // 400MB
    ("E4_TemporalPositional", 350 * 1024 * 1024), // 350MB
    ("E5_Causal", 500 * 1024 * 1024),             // 500MB
    ("E6_Sparse", 450 * 1024 * 1024),             // 450MB
    ("E7_Code", 700 * 1024 * 1024),               // 700MB
    ("E8_Graph", 550 * 1024 * 1024),              // 550MB
    ("E9_HDC", 300 * 1024 * 1024),                // 300MB
    ("E10_Contextual", 800 * 1024 * 1024),        // 800MB
    ("E11_Entity", 450 * 1024 * 1024),            // 450MB
    ("E12_LateInteraction", 600 * 1024 * 1024),   // 600MB
    ("E13_Splade", 450 * 1024 * 1024),            // 450MB (similar to Sparse)
    ("E11_Kepler", 350 * 1024 * 1024),            // 350MB (KEPLER RoBERTa-base 768D)
];
