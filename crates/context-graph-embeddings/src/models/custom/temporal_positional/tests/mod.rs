//! Tests for the Temporal-Positional embedding model (E4).
//!
//! Test modules:
//! - `construction` - Model construction and configuration tests
//! - `embedding` - Core embedding and trait implementation tests
//! - `uniqueness` - Uniqueness and determinism tests
//! - `edge_cases` - Edge case handling tests
//! - `verification` - Source of truth and evidence tests
//! - `hybrid` - Hybrid session+position encoding mode tests

mod construction;
mod edge_cases;
mod embedding;
mod hybrid;
mod uniqueness;
mod verification;

/// Helper function to compute cosine similarity between two vectors.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a < f32::EPSILON || norm_b < f32::EPSILON {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}
