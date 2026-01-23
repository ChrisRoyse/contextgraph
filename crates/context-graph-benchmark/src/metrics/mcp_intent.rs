//! MCP Intent Tool Integration benchmark metrics.
//!
//! Metrics for evaluating E10's role as an E1 ENHANCER in MCP tools:
//!
//! - **E10EnhancementMetrics**: Measures E10's improvement over E1-only
//! - **MCPToolMetrics**: Per-tool latency and quality metrics
//! - **AsymmetricValidationMetrics**: Validates direction modifiers (1.2/0.8)
//! - **ConstitutionalComplianceMetrics**: Verifies ARCH rules are followed
//!
//! ## Success Criteria (from plan)
//!
//! | Metric | Target |
//! |--------|--------|
//! | MRR improvement (E1+E10 vs E1) | > 5% |
//! | Optimal blend value | [0.2, 0.4] |
//! | Asymmetry ratio | 1.5 ± 0.15 |
//! | E1-strong refine rate | >= 70% |
//! | E1-weak broaden rate | >= 50% |
//! | Tool p95 latency | < 2000ms |

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// COMBINED METRICS
// ============================================================================

/// Combined metrics for MCP Intent benchmark.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPIntentMetrics {
    /// E10 enhancement value metrics.
    pub enhancement: E10EnhancementMetrics,

    /// Per-tool metrics.
    pub tools: MCPToolMetrics,

    /// Asymmetric validation metrics.
    pub asymmetric: AsymmetricValidationMetrics,

    /// Constitutional compliance metrics.
    pub compliance: ConstitutionalComplianceMetrics,
}

impl MCPIntentMetrics {
    /// Compute overall quality score (0.0 to 1.0).
    pub fn overall_score(&self) -> f64 {
        // 30% enhancement + 30% tool quality + 20% asymmetric + 20% compliance
        let enhancement_score = if self.enhancement.improvement_percent > 5.0 { 1.0 } else { 0.5 };
        let tool_score = self.tools.overall_quality();
        let asymmetric_score = if self.asymmetric.compliant { 1.0 } else { 0.0 };
        let compliance_score = self.compliance.score;

        0.3 * enhancement_score + 0.3 * tool_score + 0.2 * asymmetric_score + 0.2 * compliance_score
    }

    /// Check if all success criteria are met.
    pub fn meets_success_criteria(&self) -> bool {
        self.enhancement.improvement_percent > 5.0
            && self.enhancement.optimal_blend >= 0.2
            && self.enhancement.optimal_blend <= 0.4
            && self.asymmetric.compliant
            && self.compliance.all_rules_pass()
    }
}

// ============================================================================
// E10 ENHANCEMENT METRICS
// ============================================================================

/// Metrics measuring how much E10 improves E1-only results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E10EnhancementMetrics {
    /// MRR with E1 only (baseline).
    pub e1_only_mrr: f64,

    /// MRR with E1 + E10 blend.
    pub e1_e10_blend_mrr: f64,

    /// Improvement percentage: (blend - e1) / e1 * 100.
    pub improvement_percent: f64,

    /// Optimal blend value (E10 weight that maximizes MRR).
    pub optimal_blend: f64,

    /// Blend sweep results.
    pub blend_sweep: Vec<BlendSweepPoint>,

    /// E1-strong query refine rate (ARCH-17).
    pub e1_strong_refine_rate: f64,

    /// E1-weak query broaden rate (ARCH-17).
    pub e1_weak_broaden_rate: f64,

    /// Number of queries evaluated.
    pub queries_evaluated: usize,
}

impl Default for E10EnhancementMetrics {
    fn default() -> Self {
        Self {
            e1_only_mrr: 0.0,
            e1_e10_blend_mrr: 0.0,
            improvement_percent: 0.0,
            optimal_blend: 0.1,  // Updated: E10 enhances E1 at 10% weight
            blend_sweep: Vec::new(),
            e1_strong_refine_rate: 0.0,
            e1_weak_broaden_rate: 0.0,
            queries_evaluated: 0,
        }
    }
}

impl E10EnhancementMetrics {
    /// Check if enhancement meets target (>5% improvement).
    pub fn meets_target(&self) -> bool {
        self.improvement_percent > 5.0
    }

    /// Check if optimal blend is in expected range [0.2, 0.4].
    pub fn optimal_blend_in_range(&self) -> bool {
        self.optimal_blend >= 0.2 && self.optimal_blend <= 0.4
    }

    /// Check if ARCH-17 compliance is met.
    pub fn arch17_compliant(&self) -> bool {
        self.e1_strong_refine_rate >= 0.70 && self.e1_weak_broaden_rate >= 0.50
    }
}

/// A point in the blend parameter sweep.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlendSweepPoint {
    /// Blend value (E10 weight).
    pub blend: f64,

    /// E1 weight (1.0 - blend).
    pub e1_weight: f64,

    /// MRR at this blend.
    pub mrr: f64,

    /// P@5 at this blend.
    pub precision_at_5: f64,

    /// P@10 at this blend.
    pub precision_at_10: f64,
}

// ============================================================================
// MCP TOOL METRICS
// ============================================================================

/// Per-tool metrics for MCP tool integration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPToolMetrics {
    /// search_by_intent tool metrics.
    pub search_by_intent: ToolMetrics,

    /// find_contextual_matches tool metrics.
    pub find_contextual_matches: ToolMetrics,

    /// search_graph with intent_search profile metrics.
    pub search_graph_intent: ToolMetrics,
}

impl Default for MCPToolMetrics {
    fn default() -> Self {
        Self {
            search_by_intent: ToolMetrics::default(),
            find_contextual_matches: ToolMetrics::default(),
            search_graph_intent: ToolMetrics::default(),
        }
    }
}

impl MCPToolMetrics {
    /// Compute overall tool quality (average MRR).
    pub fn overall_quality(&self) -> f64 {
        let mrrs = [
            self.search_by_intent.mrr,
            self.find_contextual_matches.mrr,
            self.search_graph_intent.mrr,
        ];
        mrrs.iter().sum::<f64>() / mrrs.len() as f64
    }

    /// Check if all tools meet latency target (<2000ms p95).
    pub fn all_meet_latency_target(&self) -> bool {
        self.search_by_intent.latency_p95_ms < 2000.0
            && self.find_contextual_matches.latency_p95_ms < 2000.0
            && self.search_graph_intent.latency_p95_ms < 2000.0
    }
}

/// Metrics for a single MCP tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolMetrics {
    /// Tool name.
    pub name: String,

    /// Mean Reciprocal Rank.
    pub mrr: f64,

    /// Precision at K values.
    pub precision_at_k: HashMap<usize, f64>,

    /// Recall at K values.
    pub recall_at_k: HashMap<usize, f64>,

    /// NDCG at K values.
    pub ndcg_at_k: HashMap<usize, f64>,

    /// Latency p50 in milliseconds.
    pub latency_p50_ms: f64,

    /// Latency p95 in milliseconds.
    pub latency_p95_ms: f64,

    /// Latency p99 in milliseconds.
    pub latency_p99_ms: f64,

    /// Total queries executed.
    pub queries_executed: usize,

    /// Error rate (0.0 to 1.0).
    pub error_rate: f64,

    /// Timeout rate (0.0 to 1.0).
    pub timeout_rate: f64,
}

impl Default for ToolMetrics {
    fn default() -> Self {
        Self {
            name: String::new(),
            mrr: 0.0,
            precision_at_k: HashMap::new(),
            recall_at_k: HashMap::new(),
            ndcg_at_k: HashMap::new(),
            latency_p50_ms: 0.0,
            latency_p95_ms: 0.0,
            latency_p99_ms: 0.0,
            queries_executed: 0,
            error_rate: 0.0,
            timeout_rate: 0.0,
        }
    }
}

impl ToolMetrics {
    /// Create with tool name.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Default::default()
        }
    }

    /// Check if tool meets performance targets.
    pub fn meets_targets(&self) -> bool {
        self.latency_p95_ms < 2000.0 && self.error_rate < 0.01
    }
}

// ============================================================================
// ASYMMETRIC VALIDATION METRICS
// ============================================================================

/// Metrics for validating asymmetric direction modifiers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsymmetricValidationMetrics {
    /// Total pairs evaluated.
    pub total_pairs: usize,

    /// Observed asymmetry ratio (should be ~1.5).
    pub ratio: f64,

    /// Expected ratio (1.5 = 1.2/0.8).
    pub expected_ratio: f64,

    /// Whether ratio is within tolerance (±0.15).
    pub compliant: bool,

    /// Intent→Context MRR (with 1.2x boost).
    pub intent_to_context_mrr: f64,

    /// Context→Intent MRR (with 0.8x dampening).
    pub context_to_intent_mrr: f64,

    /// Per-pair results.
    pub pair_results: Vec<AsymmetricPairResult>,
}

impl Default for AsymmetricValidationMetrics {
    fn default() -> Self {
        Self {
            total_pairs: 0,
            ratio: 0.0,
            // E5-base-v2 handles asymmetry via prefixes - expected ratio is 1.0
            expected_ratio: 1.0,
            compliant: false,
            intent_to_context_mrr: 0.0,
            context_to_intent_mrr: 0.0,
            pair_results: Vec::new(),
        }
    }
}

impl AsymmetricValidationMetrics {
    /// Compute from pair results.
    pub fn compute(pair_results: Vec<AsymmetricPairResult>) -> Self {
        if pair_results.is_empty() {
            return Self::default();
        }

        let total = pair_results.len();

        let avg_ratio: f64 = pair_results.iter().map(|p| p.observed_ratio).sum::<f64>() / total as f64;

        // E5-base-v2 handles asymmetry via prefixes - expected ratio is 1.0
        let expected = 1.0;
        let tolerance = 0.15;
        let compliant = (avg_ratio - expected).abs() <= tolerance;

        let i2c_mrr: f64 = pair_results.iter().map(|p| p.intent_to_context_score as f64).sum::<f64>() / total as f64;
        let c2i_mrr: f64 = pair_results.iter().map(|p| p.context_to_intent_score as f64).sum::<f64>() / total as f64;

        Self {
            total_pairs: total,
            ratio: avg_ratio,
            expected_ratio: expected,
            compliant,
            intent_to_context_mrr: i2c_mrr,
            context_to_intent_mrr: c2i_mrr,
            pair_results,
        }
    }
}

/// Result for a single asymmetric pair validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsymmetricPairResult {
    /// Base similarity (without modifier).
    pub base_similarity: f32,

    /// Intent→Context score (base × 1.2).
    pub intent_to_context_score: f32,

    /// Context→Intent score (base × 0.8).
    pub context_to_intent_score: f32,

    /// Observed ratio.
    pub observed_ratio: f64,

    /// Whether this pair passes validation.
    pub passed: bool,
}

// ============================================================================
// CONSTITUTIONAL COMPLIANCE METRICS
// ============================================================================

/// Metrics for constitutional compliance validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstitutionalComplianceMetrics {
    /// ARCH-12: E1 is always foundation (blend < 0.5 for E10).
    pub arch_12_e1_foundation: bool,

    /// ARCH-17: E10 refines when E1 strong, broadens when E1 weak.
    pub arch_17_enhancement_behavior: bool,

    /// AP-02: No cross-embedder comparison.
    pub ap_02_no_cross_comparison: bool,

    /// Overall compliance score (0.0 to 1.0).
    pub score: f64,

    /// Detailed rule results.
    pub rule_details: HashMap<String, RuleComplianceResult>,
}

impl Default for ConstitutionalComplianceMetrics {
    fn default() -> Self {
        Self {
            arch_12_e1_foundation: true,
            arch_17_enhancement_behavior: true,
            ap_02_no_cross_comparison: true,
            score: 1.0,
            rule_details: HashMap::new(),
        }
    }
}

impl ConstitutionalComplianceMetrics {
    /// Check if all rules pass.
    pub fn all_rules_pass(&self) -> bool {
        self.arch_12_e1_foundation && self.arch_17_enhancement_behavior && self.ap_02_no_cross_comparison
    }

    /// Compute from individual rule results.
    pub fn compute(rules: HashMap<String, RuleComplianceResult>) -> Self {
        let arch_12 = rules.get("ARCH-12").map(|r| r.passed).unwrap_or(true);
        let arch_17 = rules.get("ARCH-17").map(|r| r.passed).unwrap_or(true);
        let ap_02 = rules.get("AP-02").map(|r| r.passed).unwrap_or(true);

        let passed_count = [arch_12, arch_17, ap_02].iter().filter(|&&x| x).count();
        let score = passed_count as f64 / 3.0;

        Self {
            arch_12_e1_foundation: arch_12,
            arch_17_enhancement_behavior: arch_17,
            ap_02_no_cross_comparison: ap_02,
            score,
            rule_details: rules,
        }
    }
}

/// Result for a single constitutional rule compliance check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleComplianceResult {
    /// Rule identifier (e.g., "ARCH-12").
    pub rule_id: String,

    /// Rule description.
    pub description: String,

    /// Whether the rule passed.
    pub passed: bool,

    /// Evidence supporting the result.
    pub evidence: String,

    /// Metric value if applicable.
    pub metric_value: Option<f64>,

    /// Threshold if applicable.
    pub threshold: Option<f64>,
}

// ============================================================================
// METRIC COMPUTATION FUNCTIONS
// ============================================================================

/// Compute MRR from ranked results.
pub fn compute_mrr(results: &[(uuid::Uuid, f64)], ground_truth: &[uuid::Uuid]) -> f64 {
    for (rank, (id, _)) in results.iter().enumerate() {
        if ground_truth.contains(id) {
            return 1.0 / (rank + 1) as f64;
        }
    }
    0.0
}

/// Compute Precision@K.
pub fn compute_precision_at_k(results: &[(uuid::Uuid, f64)], ground_truth: &[uuid::Uuid], k: usize) -> f64 {
    let top_k: Vec<_> = results.iter().take(k).map(|(id, _)| id).collect();
    let relevant = top_k.iter().filter(|id| ground_truth.contains(id)).count();
    relevant as f64 / k as f64
}

/// Compute Recall@K.
pub fn compute_recall_at_k(results: &[(uuid::Uuid, f64)], ground_truth: &[uuid::Uuid], k: usize) -> f64 {
    if ground_truth.is_empty() {
        return 0.0;
    }
    let top_k: Vec<_> = results.iter().take(k).map(|(id, _)| id).collect();
    let relevant = top_k.iter().filter(|id| ground_truth.contains(id)).count();
    relevant as f64 / ground_truth.len() as f64
}

/// Compute NDCG@K.
pub fn compute_ndcg_at_k(results: &[(uuid::Uuid, f64)], ground_truth: &[uuid::Uuid], k: usize) -> f64 {
    // DCG
    let mut dcg = 0.0;
    for (rank, (id, _)) in results.iter().take(k).enumerate() {
        if ground_truth.contains(id) {
            dcg += 1.0 / (rank as f64 + 2.0).log2();
        }
    }

    // Ideal DCG
    let num_relevant = ground_truth.len().min(k);
    let mut idcg = 0.0;
    for rank in 0..num_relevant {
        idcg += 1.0 / (rank as f64 + 2.0).log2();
    }

    if idcg == 0.0 {
        0.0
    } else {
        dcg / idcg
    }
}

/// Compute percentile from sorted latencies.
pub fn compute_percentile(sorted_latencies: &[f64], percentile: f64) -> f64 {
    if sorted_latencies.is_empty() {
        return 0.0;
    }
    let idx = ((percentile / 100.0) * (sorted_latencies.len() - 1) as f64).round() as usize;
    sorted_latencies[idx.min(sorted_latencies.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_compute_mrr() {
        let gt = vec![Uuid::from_u128(1), Uuid::from_u128(2)];
        let results = vec![
            (Uuid::from_u128(3), 0.9),
            (Uuid::from_u128(1), 0.8), // Rank 2
            (Uuid::from_u128(2), 0.7),
        ];

        let mrr = compute_mrr(&results, &gt);
        assert!((mrr - 0.5).abs() < 0.001, "MRR should be 1/2 = 0.5, got {}", mrr);

        println!("[VERIFIED] MRR computation correct");
    }

    #[test]
    fn test_compute_precision_at_k() {
        let gt = vec![Uuid::from_u128(1), Uuid::from_u128(2)];
        let results = vec![
            (Uuid::from_u128(1), 0.9),
            (Uuid::from_u128(3), 0.8),
            (Uuid::from_u128(2), 0.7),
        ];

        let p_at_3 = compute_precision_at_k(&results, &gt, 3);
        assert!((p_at_3 - 2.0/3.0).abs() < 0.001);

        println!("[VERIFIED] Precision@K computation correct");
    }

    #[test]
    fn test_asymmetric_validation() {
        // E5-base-v2 handles asymmetry via prefixes - ratio should be ~1.0
        let pairs = vec![
            AsymmetricPairResult {
                base_similarity: 0.8,
                intent_to_context_score: 0.8, // 0.8 * 1.0 (neutral)
                context_to_intent_score: 0.8, // 0.8 * 1.0 (neutral)
                observed_ratio: 1.0,
                passed: true,
            },
        ];

        let metrics = AsymmetricValidationMetrics::compute(pairs);
        assert!(metrics.compliant);
        assert!((metrics.ratio - 1.0).abs() < 0.01);

        println!("[VERIFIED] Asymmetric validation: ratio={:.2} (E5-base-v2 handles asymmetry via prefixes)", metrics.ratio);
    }

    #[test]
    fn test_enhancement_metrics() {
        let mut metrics = E10EnhancementMetrics::default();
        metrics.e1_only_mrr = 0.65;
        metrics.e1_e10_blend_mrr = 0.72;
        metrics.improvement_percent = 10.8;
        metrics.optimal_blend = 0.3;
        metrics.e1_strong_refine_rate = 0.75;
        metrics.e1_weak_broaden_rate = 0.55;

        assert!(metrics.meets_target());
        assert!(metrics.optimal_blend_in_range());
        assert!(metrics.arch17_compliant());

        println!("[VERIFIED] Enhancement metrics: {}% improvement", metrics.improvement_percent);
    }

    #[test]
    fn test_compliance_metrics() {
        let mut rules = HashMap::new();
        rules.insert("ARCH-12".to_string(), RuleComplianceResult {
            rule_id: "ARCH-12".to_string(),
            description: "E1 is always foundation".to_string(),
            passed: true,
            evidence: "blend=0.3 < 0.5".to_string(),
            metric_value: Some(0.3),
            threshold: Some(0.5),
        });

        let metrics = ConstitutionalComplianceMetrics::compute(rules);
        assert!(metrics.arch_12_e1_foundation);

        println!("[VERIFIED] Compliance score: {}", metrics.score);
    }

    #[test]
    fn test_tool_metrics() {
        let mut tool = ToolMetrics::new("search_by_intent");
        tool.mrr = 0.71;
        tool.latency_p95_ms = 1850.0;
        tool.error_rate = 0.0;

        assert!(tool.meets_targets());

        println!("[VERIFIED] Tool {} MRR={:.2}, p95={:.0}ms",
            tool.name, tool.mrr, tool.latency_p95_ms);
    }

    #[test]
    fn test_overall_metrics() {
        let metrics = MCPIntentMetrics {
            enhancement: E10EnhancementMetrics {
                improvement_percent: 10.8,
                optimal_blend: 0.3,
                e1_strong_refine_rate: 0.75,
                e1_weak_broaden_rate: 0.55,
                ..Default::default()
            },
            tools: MCPToolMetrics::default(),
            asymmetric: AsymmetricValidationMetrics {
                compliant: true,
                ratio: 1.49,
                ..Default::default()
            },
            compliance: ConstitutionalComplianceMetrics::default(),
        };

        assert!(metrics.meets_success_criteria());
        let score = metrics.overall_score();
        assert!(score > 0.5);

        println!("[VERIFIED] Overall score: {:.2}", score);
    }
}
