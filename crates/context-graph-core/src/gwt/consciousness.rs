//! Consciousness Equation: C(t) = I(t) × R(t) × D(t)
//!
//! Implements the core consciousness computation as specified in Constitution v4.0.0
//! Section gwt.consciousness_equation (lines 313-321).
//!
//! ## Formula
//!
//! ```text
//! C(t) = r(t) × σ(MetaUTL.predict_accuracy) × H(PurposeVector)
//! ```
//!
//! Where:
//! - **I(t)** = r(t): Kuramoto order parameter (integration across embeddings)
//! - **R(t)** = σ(MetaUTL.predict_accuracy): Sigmoid of meta-learning accuracy (self-reflection)
//! - **D(t)** = H(PurposeVector) normalized: Shannon entropy of purpose vector (differentiation)
//! - **C(t)** ∈ [0,1]: Final consciousness level

use crate::error::{CoreError, CoreResult};

/// Computes consciousness level from three independent factors
#[derive(Debug, Clone)]
pub struct ConsciousnessCalculator;

/// Metrics for consciousness computation
#[derive(Debug, Clone)]
pub struct ConsciousnessMetrics {
    /// Integration (Kuramoto order parameter r)
    pub integration: f32,
    /// Self-reflection (sigmoid of meta-UTL accuracy)
    pub reflection: f32,
    /// Differentiation (normalized Shannon entropy)
    pub differentiation: f32,
    /// Final consciousness level C(t)
    pub consciousness: f32,
    /// Individual component strengths for debugging
    pub component_analysis: ComponentAnalysis,
}

#[derive(Debug, Clone)]
pub struct ComponentAnalysis {
    /// Is integration sufficient for consciousness?
    pub integration_sufficient: bool,
    /// Is reflection sufficient for consciousness?
    pub reflection_sufficient: bool,
    /// Is differentiation sufficient for consciousness?
    pub differentiation_sufficient: bool,
    /// Which component is the limiting factor
    pub limiting_factor: LimitingFactor,
}

#[derive(Debug, Clone, Copy)]
pub enum LimitingFactor {
    Integration,
    Reflection,
    Differentiation,
    None,
}

impl ConsciousnessCalculator {
    /// Create a new consciousness calculator
    pub fn new() -> Self {
        Self
    }

    /// Compute full consciousness equation C(t) = I(t) × R(t) × D(t)
    ///
    /// # Arguments
    /// - `kuramoto_r`: Kuramoto order parameter ∈ [0,1] (integration)
    /// - `meta_accuracy`: Meta-UTL prediction accuracy ∈ [0,1] (reflection)
    /// - `purpose_vector`: 13D purpose alignment vector
    ///
    /// # Returns
    /// - Consciousness level ∈ [0,1]
    pub fn compute_consciousness(
        &self,
        kuramoto_r: f32,
        meta_accuracy: f32,
        purpose_vector: &[f32; 13],
    ) -> CoreResult<f32> {
        // Validate inputs
        if !(0.0..=1.0).contains(&kuramoto_r) {
            return Err(CoreError::ValidationError {
                field: "kuramoto_r".to_string(),
                message: format!("out of range [0,1]: {}", kuramoto_r),
            });
        }
        if !(0.0..=1.0).contains(&meta_accuracy) {
            return Err(CoreError::ValidationError {
                field: "meta_accuracy".to_string(),
                message: format!("out of range [0,1]: {}", meta_accuracy),
            });
        }

        // I(t) = Kuramoto order parameter
        let integration = kuramoto_r;

        // R(t) = σ(meta_accuracy) via sigmoid
        // For simplicity, using logistic sigmoid: σ(x) = 1/(1+e^(-x))
        // Map [0,1] → [-2,2] → sigmoid → [0.118, 0.881]
        // For conscious range, we scale: if accuracy is high, reflection is high
        let reflection = self.sigmoid(meta_accuracy * 4.0 - 2.0);

        // D(t) = H(PurposeVector) normalized
        let differentiation = self.normalized_purpose_entropy(purpose_vector)?;

        // C(t) = I(t) × R(t) × D(t)
        let consciousness = integration * reflection * differentiation;

        Ok(consciousness.clamp(0.0, 1.0))
    }

    /// Compute full metrics including component analysis
    pub fn compute_metrics(
        &self,
        kuramoto_r: f32,
        meta_accuracy: f32,
        purpose_vector: &[f32; 13],
    ) -> CoreResult<ConsciousnessMetrics> {
        let integration = kuramoto_r;
        let reflection = self.sigmoid(meta_accuracy * 4.0 - 2.0);
        let differentiation = self.normalized_purpose_entropy(purpose_vector)?;
        let consciousness = (integration * reflection * differentiation).clamp(0.0, 1.0);

        // Analyze limiting factors (threshold for "sufficient" is 0.5)
        let integration_sufficient = integration >= 0.5;
        let reflection_sufficient = reflection >= 0.5;
        let differentiation_sufficient = differentiation >= 0.5;

        let limiting_factor = match (
            integration_sufficient,
            reflection_sufficient,
            differentiation_sufficient,
        ) {
            (false, _, _) => LimitingFactor::Integration,
            (_, false, _) => LimitingFactor::Reflection,
            (_, _, false) => LimitingFactor::Differentiation,
            _ => LimitingFactor::None,
        };

        Ok(ConsciousnessMetrics {
            integration,
            reflection,
            differentiation,
            consciousness,
            component_analysis: ComponentAnalysis {
                integration_sufficient,
                reflection_sufficient,
                differentiation_sufficient,
                limiting_factor,
            },
        })
    }

    /// Logistic sigmoid function: σ(x) = 1/(1+e^(-x))
    fn sigmoid(&self, x: f32) -> f32 {
        (1.0 / (1.0 + (-x).exp())).clamp(0.0, 1.0)
    }

    /// Compute normalized Shannon entropy of purpose vector
    ///
    /// H(V) = -Σᵢ pᵢ log₂(pᵢ) where pᵢ = |Vᵢ| / Σⱼ|Vⱼ|
    /// Normalized to [0,1] where max is log₂(13) ≈ 3.7
    fn normalized_purpose_entropy(&self, purpose_vector: &[f32; 13]) -> CoreResult<f32> {
        // Convert to probability distribution
        let sum: f32 = purpose_vector.iter().map(|v| v.abs()).sum();

        if sum <= 1e-6 {
            // Empty vector → no differentiation
            return Ok(0.0);
        }

        // Compute Shannon entropy
        let mut entropy = 0.0;
        for value in purpose_vector {
            let p = (value.abs() / sum).clamp(1e-6, 1.0);
            entropy -= p * p.log2();
        }

        // Normalize to [0,1] by dividing by maximum entropy log₂(13) ≈ 3.7
        let max_entropy = 13.0_f32.log2();
        let normalized = (entropy / max_entropy).clamp(0.0, 1.0);

        Ok(normalized)
    }
}

impl Default for ConsciousnessCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consciousness_equation_high_all_factors() {
        let calc = ConsciousnessCalculator::new();
        let purpose_vec = [1.0; 13]; // Uniform distribution

        let c = calc.compute_consciousness(0.9, 0.9, &purpose_vec).unwrap();
        // r=0.9, σ(0.9*4-2)=σ(1.6)≈0.832, H_norm≈0.91
        // C ≈ 0.9 × 0.832 × 0.91 ≈ 0.676
        assert!(c > 0.6 && c < 0.8, "Got {}", c);
    }

    #[test]
    fn test_consciousness_equation_low_integration() {
        let calc = ConsciousnessCalculator::new();
        let purpose_vec = [1.0; 13];

        let c = calc.compute_consciousness(0.2, 0.9, &purpose_vec).unwrap();
        // Low Kuramoto should kill consciousness
        assert!(c < 0.4);
    }

    #[test]
    fn test_consciousness_equation_zero_purpose_vector() {
        let calc = ConsciousnessCalculator::new();
        let purpose_vec = [0.0; 13];

        let c = calc.compute_consciousness(0.9, 0.9, &purpose_vec).unwrap();
        // Zero entropy → consciousness = 0
        assert!(c < 0.01);
    }

    #[test]
    fn test_consciousness_bounds() {
        let calc = ConsciousnessCalculator::new();
        let purpose_vec = [1.0; 13];

        let c = calc.compute_consciousness(1.0, 1.0, &purpose_vec).unwrap();
        assert!(c >= 0.0 && c <= 1.0);
        assert!(c > 0.8); // Should be quite high
    }

    #[test]
    fn test_metrics_limiting_factor_analysis() {
        let calc = ConsciousnessCalculator::new();
        let purpose_vec = [1.0; 13];

        let metrics = calc
            .compute_metrics(0.1, 0.9, &purpose_vec)
            .unwrap();
        // Integration is the bottleneck
        matches!(metrics.component_analysis.limiting_factor, LimitingFactor::Integration);
        assert!(!metrics.component_analysis.integration_sufficient);
    }
}
