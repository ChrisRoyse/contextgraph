//! NORTH-014: Gap Detection Service
//!
//! Identifies gaps in goal coverage including uncovered domains,
//! weak coverage areas, missing links between goals, and temporal gaps.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::autonomous::bootstrap::GoalId;
use crate::autonomous::evolution::{GoalActivityMetrics, GoalLevel};

/// Types of coverage gaps detected in the goal hierarchy
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum GapType {
    /// A domain/topic area with no goal coverage
    UncoveredDomain { domain: String },
    /// A goal with insufficient coverage (below threshold)
    WeakCoverage { goal_id: GoalId, coverage: f32 },
    /// Missing hierarchical or semantic link between goals
    MissingLink { from: GoalId, to: GoalId },
    /// Period without goal activity
    TemporalGap { period: String },
}

impl GapType {
    /// Get the severity of this gap (0.0-1.0, higher is more severe)
    pub fn severity(&self) -> f32 {
        match self {
            GapType::UncoveredDomain { .. } => 0.9,
            GapType::WeakCoverage { coverage, .. } => 1.0 - coverage,
            GapType::MissingLink { .. } => 0.6,
            GapType::TemporalGap { .. } => 0.4,
        }
    }

    /// Get a human-readable description of the gap
    pub fn description(&self) -> String {
        match self {
            GapType::UncoveredDomain { domain } => {
                format!("Domain '{}' has no goal coverage", domain)
            }
            GapType::WeakCoverage { goal_id, coverage } => {
                format!(
                    "Goal {} has weak coverage at {:.1}%",
                    goal_id.0,
                    coverage * 100.0
                )
            }
            GapType::MissingLink { from, to } => {
                format!("Missing link between goals {} and {}", from.0, to.0)
            }
            GapType::TemporalGap { period } => {
                format!("No activity during period: {}", period)
            }
        }
    }
}

/// Goal paired with its activity metrics for analysis
#[derive(Clone, Debug)]
pub struct GoalWithMetrics {
    pub goal_id: GoalId,
    pub level: GoalLevel,
    pub description: String,
    pub parent_id: Option<GoalId>,
    pub child_ids: Vec<GoalId>,
    pub domains: Vec<String>,
    pub metrics: GoalActivityMetrics,
}

impl GoalWithMetrics {
    /// Calculate coverage score for this goal based on activity metrics
    pub fn coverage_score(&self) -> f32 {
        let activity = self.metrics.activity_score();
        let alignment = self.metrics.avg_child_alignment;
        0.6 * activity + 0.4 * alignment
    }

    /// Check if this goal has weak coverage
    pub fn has_weak_coverage(&self, threshold: f32) -> bool {
        self.coverage_score() < threshold
    }
}

/// Configuration for gap detection
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GapDetectionConfig {
    /// Minimum coverage score threshold (0.0-1.0)
    pub coverage_threshold: f32,
    /// Minimum activity score for a goal to be considered active
    pub activity_threshold: f32,
    /// Minimum number of goals per domain
    pub min_goals_per_domain: usize,
    /// Days without activity to flag as temporal gap
    pub inactivity_days: u32,
    /// Whether to detect missing links between related goals
    pub detect_missing_links: bool,
    /// Similarity threshold for detecting potentially linked goals
    pub link_similarity_threshold: f32,
}

impl Default for GapDetectionConfig {
    fn default() -> Self {
        Self {
            coverage_threshold: 0.4,
            activity_threshold: 0.2,
            min_goals_per_domain: 1,
            inactivity_days: 14,
            detect_missing_links: true,
            link_similarity_threshold: 0.7,
        }
    }
}

/// Report of detected gaps with analysis
#[derive(Clone, Debug)]
pub struct GapReport {
    /// All detected gaps
    pub gaps: Vec<GapType>,
    /// Overall coverage score (0.0-1.0)
    pub coverage_score: f32,
    /// Generated recommendations for addressing gaps
    pub recommendations: Vec<String>,
    /// Count of goals analyzed
    pub goals_analyzed: usize,
    /// Count of domains detected
    pub domains_detected: usize,
}

impl GapReport {
    /// Check if there are any gaps
    pub fn has_gaps(&self) -> bool {
        !self.gaps.is_empty()
    }

    /// Get gaps sorted by severity (highest first)
    pub fn gaps_by_severity(&self) -> Vec<&GapType> {
        let mut sorted: Vec<_> = self.gaps.iter().collect();
        sorted.sort_by(|a, b| {
            b.severity()
                .partial_cmp(&a.severity())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted
    }

    /// Count gaps by type
    pub fn gap_counts(&self) -> HashMap<String, usize> {
        let mut counts = HashMap::new();
        for gap in &self.gaps {
            let type_name = match gap {
                GapType::UncoveredDomain { .. } => "uncovered_domain",
                GapType::WeakCoverage { .. } => "weak_coverage",
                GapType::MissingLink { .. } => "missing_link",
                GapType::TemporalGap { .. } => "temporal_gap",
            };
            *counts.entry(type_name.to_string()).or_insert(0) += 1;
        }
        counts
    }

    /// Get the most severe gap, if any
    pub fn most_severe_gap(&self) -> Option<&GapType> {
        self.gaps.iter().max_by(|a, b| {
            a.severity()
                .partial_cmp(&b.severity())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }
}

/// Service for detecting gaps in goal coverage
#[derive(Clone, Debug)]
pub struct GapDetectionService {
    config: GapDetectionConfig,
}

impl GapDetectionService {
    /// Create a new gap detection service with default configuration
    pub fn new() -> Self {
        Self {
            config: GapDetectionConfig::default(),
        }
    }

    /// Create a new gap detection service with custom configuration
    pub fn with_config(config: GapDetectionConfig) -> Self {
        Self { config }
    }

    /// Analyze coverage and detect all gaps
    pub fn analyze_coverage(&self, goals: &[GoalWithMetrics]) -> GapReport {
        if goals.is_empty() {
            return GapReport {
                gaps: vec![],
                coverage_score: 0.0,
                recommendations: vec![
                    "No goals to analyze. Consider bootstrapping initial goals.".into()
                ],
                goals_analyzed: 0,
                domains_detected: 0,
            };
        }

        let mut all_gaps = Vec::new();

        // Detect domain gaps
        let domain_gaps = self.detect_domain_gaps(goals);
        all_gaps.extend(domain_gaps);

        // Detect weak coverage
        let weak_coverage_gaps = self.detect_weak_coverage(goals);
        all_gaps.extend(weak_coverage_gaps);

        // Detect missing links
        if self.config.detect_missing_links {
            let link_gaps = self.detect_missing_links(goals);
            all_gaps.extend(link_gaps);
        }

        // Compute overall coverage
        let coverage_score = self.compute_coverage_score(goals);

        // Collect all domains
        let domains: HashSet<_> = goals.iter().flat_map(|g| g.domains.iter()).collect();

        // Generate recommendations
        let recommendations = self.generate_recommendations(&all_gaps);

        GapReport {
            gaps: all_gaps,
            coverage_score,
            recommendations,
            goals_analyzed: goals.len(),
            domains_detected: domains.len(),
        }
    }

    /// Detect domains with no or insufficient goal coverage
    pub fn detect_domain_gaps(&self, goals: &[GoalWithMetrics]) -> Vec<GapType> {
        let mut gaps = Vec::new();

        // Build domain coverage map
        let mut domain_goals: HashMap<&String, Vec<&GoalWithMetrics>> = HashMap::new();
        for goal in goals {
            for domain in &goal.domains {
                domain_goals.entry(domain).or_default().push(goal);
            }
        }

        // Check each domain for sufficient coverage
        for (domain, domain_goal_list) in &domain_goals {
            let active_goals: Vec<_> = domain_goal_list
                .iter()
                .filter(|g| g.metrics.activity_score() >= self.config.activity_threshold)
                .collect();

            if active_goals.len() < self.config.min_goals_per_domain {
                gaps.push(GapType::UncoveredDomain {
                    domain: (*domain).clone(),
                });
            }
        }

        gaps
    }

    /// Detect goals with weak coverage (low activity/alignment)
    pub fn detect_weak_coverage(&self, goals: &[GoalWithMetrics]) -> Vec<GapType> {
        let mut gaps = Vec::new();

        for goal in goals {
            if goal.has_weak_coverage(self.config.coverage_threshold) {
                gaps.push(GapType::WeakCoverage {
                    goal_id: goal.goal_id.clone(),
                    coverage: goal.coverage_score(),
                });
            }
        }

        gaps
    }

    /// Detect missing links between related goals
    pub fn detect_missing_links(&self, goals: &[GoalWithMetrics]) -> Vec<GapType> {
        let mut gaps = Vec::new();

        // Build parent-child relationship set
        let mut linked_pairs: HashSet<(GoalId, GoalId)> = HashSet::new();
        for goal in goals {
            if let Some(ref parent_id) = goal.parent_id {
                linked_pairs.insert((parent_id.clone(), goal.goal_id.clone()));
                linked_pairs.insert((goal.goal_id.clone(), parent_id.clone()));
            }
            for child_id in &goal.child_ids {
                linked_pairs.insert((goal.goal_id.clone(), child_id.clone()));
                linked_pairs.insert((child_id.clone(), goal.goal_id.clone()));
            }
        }

        // Check for goals that share domains but are not linked
        for i in 0..goals.len() {
            for j in (i + 1)..goals.len() {
                let goal_a = &goals[i];
                let goal_b = &goals[j];

                // Check if they share domains
                let shared_domains: Vec<_> = goal_a
                    .domains
                    .iter()
                    .filter(|d| goal_b.domains.contains(d))
                    .collect();

                if !shared_domains.is_empty() {
                    // They share domains - check if linked
                    let pair = (goal_a.goal_id.clone(), goal_b.goal_id.clone());
                    if !linked_pairs.contains(&pair) {
                        // Not linked but share domains - potential missing link
                        // Only flag if both are active
                        if goal_a.metrics.is_active() && goal_b.metrics.is_active() {
                            gaps.push(GapType::MissingLink {
                                from: goal_a.goal_id.clone(),
                                to: goal_b.goal_id.clone(),
                            });
                        }
                    }
                }
            }
        }

        gaps
    }

    /// Compute overall coverage score across all goals
    pub fn compute_coverage_score(&self, goals: &[GoalWithMetrics]) -> f32 {
        if goals.is_empty() {
            return 0.0;
        }

        let total: f32 = goals.iter().map(|g| g.coverage_score()).sum();
        let average = total / goals.len() as f32;

        // Penalize for inactive goals
        let active_ratio =
            goals.iter().filter(|g| g.metrics.is_active()).count() as f32 / goals.len() as f32;

        // Weight by level (NorthStar goals matter more)
        let level_weighted: f32 = goals
            .iter()
            .map(|g| {
                let level_weight = match g.level {
                    GoalLevel::NorthStar => 2.0,
                    GoalLevel::Strategic => 1.5,
                    GoalLevel::Tactical => 1.0,
                    GoalLevel::Operational => 0.75,
                };
                g.coverage_score() * level_weight
            })
            .sum::<f32>()
            / goals
                .iter()
                .map(|g| match g.level {
                    GoalLevel::NorthStar => 2.0,
                    GoalLevel::Strategic => 1.5,
                    GoalLevel::Tactical => 1.0,
                    GoalLevel::Operational => 0.75,
                })
                .sum::<f32>();

        // Combine metrics
        (0.4 * average + 0.3 * active_ratio + 0.3 * level_weighted).clamp(0.0, 1.0)
    }

    /// Generate recommendations based on detected gaps
    pub fn generate_recommendations(&self, gaps: &[GapType]) -> Vec<String> {
        let mut recommendations = Vec::new();

        // Count gap types
        let mut uncovered_domains = Vec::new();
        let mut weak_coverage_count = 0;
        let mut missing_link_count = 0;
        let mut temporal_gap_count = 0;

        for gap in gaps {
            match gap {
                GapType::UncoveredDomain { domain } => uncovered_domains.push(domain.clone()),
                GapType::WeakCoverage { .. } => weak_coverage_count += 1,
                GapType::MissingLink { .. } => missing_link_count += 1,
                GapType::TemporalGap { .. } => temporal_gap_count += 1,
            }
        }

        // Generate recommendations based on gaps found
        if !uncovered_domains.is_empty() {
            if uncovered_domains.len() == 1 {
                recommendations.push(format!(
                    "Create a goal to cover the '{}' domain",
                    uncovered_domains[0]
                ));
            } else {
                recommendations.push(format!(
                    "Create goals to cover {} uncovered domains: {}",
                    uncovered_domains.len(),
                    uncovered_domains.join(", ")
                ));
            }
        }

        if weak_coverage_count > 0 {
            if weak_coverage_count == 1 {
                recommendations.push("Review and strengthen the goal with weak coverage".into());
            } else {
                recommendations.push(format!(
                    "Review and strengthen {} goals with weak coverage",
                    weak_coverage_count
                ));
            }
        }

        if missing_link_count > 0 {
            if missing_link_count == 1 {
                recommendations.push("Consider establishing a link between related goals".into());
            } else {
                recommendations.push(format!(
                    "Consider establishing {} links between related goals",
                    missing_link_count
                ));
            }
        }

        if temporal_gap_count > 0 {
            recommendations.push(format!(
                "Address {} period(s) of inactivity by scheduling regular goal reviews",
                temporal_gap_count
            ));
        }

        if recommendations.is_empty() && gaps.is_empty() {
            recommendations.push("Goal coverage is healthy. Continue monitoring.".into());
        }

        recommendations
    }
}

impl Default for GapDetectionService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_metrics(
        goal_id: GoalId,
        memories: u32,
        retrievals: u32,
        alignment: f32,
    ) -> GoalActivityMetrics {
        GoalActivityMetrics {
            goal_id,
            new_aligned_memories_30d: memories,
            retrievals_14d: retrievals,
            avg_child_alignment: alignment,
            weight_trend: 0.0,
            last_activity: Utc::now(),
        }
    }

    fn create_test_goal(
        level: GoalLevel,
        domains: Vec<&str>,
        memories: u32,
        retrievals: u32,
        alignment: f32,
    ) -> GoalWithMetrics {
        let goal_id = GoalId::new();
        GoalWithMetrics {
            goal_id: goal_id.clone(),
            level,
            description: "Test goal".into(),
            parent_id: None,
            child_ids: vec![],
            domains: domains.into_iter().map(String::from).collect(),
            metrics: create_test_metrics(goal_id, memories, retrievals, alignment),
        }
    }

    // ============================================================
    // GapType tests
    // ============================================================

    #[test]
    fn test_gap_type_uncovered_domain_severity() {
        let gap = GapType::UncoveredDomain {
            domain: "security".into(),
        };
        assert!((gap.severity() - 0.9).abs() < f32::EPSILON);
        println!("[PASS] test_gap_type_uncovered_domain_severity");
    }

    #[test]
    fn test_gap_type_weak_coverage_severity() {
        let gap = GapType::WeakCoverage {
            goal_id: GoalId::new(),
            coverage: 0.3,
        };
        assert!((gap.severity() - 0.7).abs() < f32::EPSILON);

        let gap_high = GapType::WeakCoverage {
            goal_id: GoalId::new(),
            coverage: 0.8,
        };
        assert!((gap_high.severity() - 0.2).abs() < f32::EPSILON);
        println!("[PASS] test_gap_type_weak_coverage_severity");
    }

    #[test]
    fn test_gap_type_missing_link_severity() {
        let gap = GapType::MissingLink {
            from: GoalId::new(),
            to: GoalId::new(),
        };
        assert!((gap.severity() - 0.6).abs() < f32::EPSILON);
        println!("[PASS] test_gap_type_missing_link_severity");
    }

    #[test]
    fn test_gap_type_temporal_gap_severity() {
        let gap = GapType::TemporalGap {
            period: "2024-01".into(),
        };
        assert!((gap.severity() - 0.4).abs() < f32::EPSILON);
        println!("[PASS] test_gap_type_temporal_gap_severity");
    }

    #[test]
    fn test_gap_type_description() {
        let domain_gap = GapType::UncoveredDomain {
            domain: "security".into(),
        };
        assert!(domain_gap.description().contains("security"));
        assert!(domain_gap.description().contains("no goal coverage"));

        let goal_id = GoalId::new();
        let weak_gap = GapType::WeakCoverage {
            goal_id: goal_id.clone(),
            coverage: 0.25,
        };
        assert!(weak_gap.description().contains("weak coverage"));
        assert!(weak_gap.description().contains("25.0%"));

        let from = GoalId::new();
        let to = GoalId::new();
        let link_gap = GapType::MissingLink {
            from: from.clone(),
            to: to.clone(),
        };
        assert!(link_gap.description().contains("Missing link"));

        let temporal_gap = GapType::TemporalGap {
            period: "Q1 2024".into(),
        };
        assert!(temporal_gap.description().contains("Q1 2024"));
        println!("[PASS] test_gap_type_description");
    }

    #[test]
    fn test_gap_type_equality() {
        let gap1 = GapType::UncoveredDomain {
            domain: "security".into(),
        };
        let gap2 = GapType::UncoveredDomain {
            domain: "security".into(),
        };
        let gap3 = GapType::UncoveredDomain {
            domain: "performance".into(),
        };

        assert_eq!(gap1, gap2);
        assert_ne!(gap1, gap3);
        println!("[PASS] test_gap_type_equality");
    }

    #[test]
    fn test_gap_type_serialization() {
        let gap = GapType::UncoveredDomain {
            domain: "testing".into(),
        };
        let json = serde_json::to_string(&gap).expect("serialize");
        let deserialized: GapType = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(gap, deserialized);
        println!("[PASS] test_gap_type_serialization");
    }

    // ============================================================
    // GoalWithMetrics tests
    // ============================================================

    #[test]
    fn test_goal_with_metrics_coverage_score() {
        // High activity, high alignment
        let goal = create_test_goal(GoalLevel::Strategic, vec!["domain1"], 50, 25, 0.8);
        let score = goal.coverage_score();
        // activity = 0.5, alignment = 0.8
        // coverage = 0.6 * 0.5 + 0.4 * 0.8 = 0.3 + 0.32 = 0.62
        assert!((score - 0.62).abs() < 0.01);
        println!("[PASS] test_goal_with_metrics_coverage_score");
    }

    #[test]
    fn test_goal_with_metrics_coverage_score_zero() {
        let goal = create_test_goal(GoalLevel::Operational, vec!["domain1"], 0, 0, 0.0);
        let score = goal.coverage_score();
        assert!((score - 0.0).abs() < f32::EPSILON);
        println!("[PASS] test_goal_with_metrics_coverage_score_zero");
    }

    #[test]
    fn test_goal_with_metrics_coverage_score_max() {
        let goal = create_test_goal(GoalLevel::NorthStar, vec!["domain1"], 100, 50, 1.0);
        let score = goal.coverage_score();
        // activity = 1.0, alignment = 1.0
        // coverage = 0.6 * 1.0 + 0.4 * 1.0 = 1.0
        assert!((score - 1.0).abs() < f32::EPSILON);
        println!("[PASS] test_goal_with_metrics_coverage_score_max");
    }

    #[test]
    fn test_goal_with_metrics_has_weak_coverage() {
        let weak_goal = create_test_goal(GoalLevel::Tactical, vec!["domain1"], 10, 5, 0.2);
        assert!(weak_goal.has_weak_coverage(0.4));

        let strong_goal = create_test_goal(GoalLevel::Tactical, vec!["domain1"], 80, 40, 0.9);
        assert!(!strong_goal.has_weak_coverage(0.4));
        println!("[PASS] test_goal_with_metrics_has_weak_coverage");
    }

    // ============================================================
    // GapDetectionConfig tests
    // ============================================================

    #[test]
    fn test_gap_detection_config_default() {
        let config = GapDetectionConfig::default();
        assert!((config.coverage_threshold - 0.4).abs() < f32::EPSILON);
        assert!((config.activity_threshold - 0.2).abs() < f32::EPSILON);
        assert_eq!(config.min_goals_per_domain, 1);
        assert_eq!(config.inactivity_days, 14);
        assert!(config.detect_missing_links);
        assert!((config.link_similarity_threshold - 0.7).abs() < f32::EPSILON);
        println!("[PASS] test_gap_detection_config_default");
    }

    #[test]
    fn test_gap_detection_config_custom() {
        let config = GapDetectionConfig {
            coverage_threshold: 0.5,
            activity_threshold: 0.3,
            min_goals_per_domain: 2,
            inactivity_days: 7,
            detect_missing_links: false,
            link_similarity_threshold: 0.8,
        };
        assert!((config.coverage_threshold - 0.5).abs() < f32::EPSILON);
        assert!((config.activity_threshold - 0.3).abs() < f32::EPSILON);
        assert_eq!(config.min_goals_per_domain, 2);
        assert_eq!(config.inactivity_days, 7);
        assert!(!config.detect_missing_links);
        println!("[PASS] test_gap_detection_config_custom");
    }

    #[test]
    fn test_gap_detection_config_serialization() {
        let config = GapDetectionConfig::default();
        let json = serde_json::to_string(&config).expect("serialize");
        let deserialized: GapDetectionConfig = serde_json::from_str(&json).expect("deserialize");
        assert!((config.coverage_threshold - deserialized.coverage_threshold).abs() < f32::EPSILON);
        assert_eq!(
            config.min_goals_per_domain,
            deserialized.min_goals_per_domain
        );
        println!("[PASS] test_gap_detection_config_serialization");
    }

    // ============================================================
    // GapReport tests
    // ============================================================

    #[test]
    fn test_gap_report_has_gaps() {
        let empty_report = GapReport {
            gaps: vec![],
            coverage_score: 0.8,
            recommendations: vec![],
            goals_analyzed: 5,
            domains_detected: 3,
        };
        assert!(!empty_report.has_gaps());

        let report_with_gaps = GapReport {
            gaps: vec![GapType::UncoveredDomain {
                domain: "security".into(),
            }],
            coverage_score: 0.6,
            recommendations: vec![],
            goals_analyzed: 5,
            domains_detected: 3,
        };
        assert!(report_with_gaps.has_gaps());
        println!("[PASS] test_gap_report_has_gaps");
    }

    #[test]
    fn test_gap_report_gaps_by_severity() {
        let report = GapReport {
            gaps: vec![
                GapType::TemporalGap {
                    period: "Q1".into(),
                }, // 0.4
                GapType::UncoveredDomain {
                    domain: "security".into(),
                }, // 0.9
                GapType::MissingLink {
                    from: GoalId::new(),
                    to: GoalId::new(),
                }, // 0.6
            ],
            coverage_score: 0.5,
            recommendations: vec![],
            goals_analyzed: 5,
            domains_detected: 3,
        };

        let sorted = report.gaps_by_severity();
        assert_eq!(sorted.len(), 3);
        assert!((sorted[0].severity() - 0.9).abs() < f32::EPSILON); // UncoveredDomain first
        assert!((sorted[1].severity() - 0.6).abs() < f32::EPSILON); // MissingLink second
        assert!((sorted[2].severity() - 0.4).abs() < f32::EPSILON); // TemporalGap last
        println!("[PASS] test_gap_report_gaps_by_severity");
    }

    #[test]
    fn test_gap_report_gap_counts() {
        let report = GapReport {
            gaps: vec![
                GapType::UncoveredDomain {
                    domain: "security".into(),
                },
                GapType::UncoveredDomain {
                    domain: "performance".into(),
                },
                GapType::WeakCoverage {
                    goal_id: GoalId::new(),
                    coverage: 0.3,
                },
            ],
            coverage_score: 0.5,
            recommendations: vec![],
            goals_analyzed: 5,
            domains_detected: 3,
        };

        let counts = report.gap_counts();
        assert_eq!(counts.get("uncovered_domain"), Some(&2));
        assert_eq!(counts.get("weak_coverage"), Some(&1));
        assert_eq!(counts.get("missing_link"), None);
        println!("[PASS] test_gap_report_gap_counts");
    }

    #[test]
    fn test_gap_report_most_severe_gap() {
        let report = GapReport {
            gaps: vec![
                GapType::TemporalGap {
                    period: "Q1".into(),
                },
                GapType::WeakCoverage {
                    goal_id: GoalId::new(),
                    coverage: 0.2,
                }, // severity 0.8
                GapType::MissingLink {
                    from: GoalId::new(),
                    to: GoalId::new(),
                },
            ],
            coverage_score: 0.5,
            recommendations: vec![],
            goals_analyzed: 5,
            domains_detected: 3,
        };

        let most_severe = report.most_severe_gap();
        assert!(most_severe.is_some());
        assert!((most_severe.unwrap().severity() - 0.8).abs() < f32::EPSILON);

        let empty_report = GapReport {
            gaps: vec![],
            coverage_score: 0.9,
            recommendations: vec![],
            goals_analyzed: 0,
            domains_detected: 0,
        };
        assert!(empty_report.most_severe_gap().is_none());
        println!("[PASS] test_gap_report_most_severe_gap");
    }

    // ============================================================
    // GapDetectionService tests
    // ============================================================

    #[test]
    fn test_service_new() {
        let service = GapDetectionService::new();
        assert!((service.config.coverage_threshold - 0.4).abs() < f32::EPSILON);
        println!("[PASS] test_service_new");
    }

    #[test]
    fn test_service_with_config() {
        let config = GapDetectionConfig {
            coverage_threshold: 0.6,
            ..Default::default()
        };
        let service = GapDetectionService::with_config(config);
        assert!((service.config.coverage_threshold - 0.6).abs() < f32::EPSILON);
        println!("[PASS] test_service_with_config");
    }

    #[test]
    fn test_service_default() {
        let service = GapDetectionService::default();
        assert!((service.config.coverage_threshold - 0.4).abs() < f32::EPSILON);
        println!("[PASS] test_service_default");
    }

    #[test]
    fn test_analyze_coverage_empty_goals() {
        let service = GapDetectionService::new();
        let report = service.analyze_coverage(&[]);

        assert!(!report.has_gaps());
        assert!((report.coverage_score - 0.0).abs() < f32::EPSILON);
        assert_eq!(report.goals_analyzed, 0);
        assert_eq!(report.domains_detected, 0);
        assert!(!report.recommendations.is_empty());
        println!("[PASS] test_analyze_coverage_empty_goals");
    }

    #[test]
    fn test_analyze_coverage_healthy_goals() {
        let service = GapDetectionService::new();
        let goals = vec![
            create_test_goal(GoalLevel::NorthStar, vec!["core"], 80, 40, 0.9),
            create_test_goal(GoalLevel::Strategic, vec!["security"], 60, 30, 0.85),
            create_test_goal(GoalLevel::Tactical, vec!["performance"], 50, 25, 0.8),
        ];

        let report = service.analyze_coverage(&goals);

        assert_eq!(report.goals_analyzed, 3);
        assert_eq!(report.domains_detected, 3);
        assert!(report.coverage_score > 0.5);
        println!("[PASS] test_analyze_coverage_healthy_goals");
    }

    #[test]
    fn test_detect_domain_gaps_no_gaps() {
        let config = GapDetectionConfig {
            min_goals_per_domain: 1,
            activity_threshold: 0.2,
            ..Default::default()
        };
        let service = GapDetectionService::with_config(config);

        let goals = vec![
            create_test_goal(GoalLevel::Strategic, vec!["security"], 50, 25, 0.8),
            create_test_goal(GoalLevel::Tactical, vec!["performance"], 50, 25, 0.8),
        ];

        let gaps = service.detect_domain_gaps(&goals);
        assert!(gaps.is_empty());
        println!("[PASS] test_detect_domain_gaps_no_gaps");
    }

    #[test]
    fn test_detect_domain_gaps_inactive_domain() {
        let config = GapDetectionConfig {
            min_goals_per_domain: 1,
            activity_threshold: 0.5, // High threshold
            ..Default::default()
        };
        let service = GapDetectionService::with_config(config);

        // This goal has low activity (score < 0.5)
        let goals = vec![create_test_goal(
            GoalLevel::Strategic,
            vec!["security"],
            10,
            5,
            0.3,
        )];

        let gaps = service.detect_domain_gaps(&goals);
        assert_eq!(gaps.len(), 1);
        match &gaps[0] {
            GapType::UncoveredDomain { domain } => assert_eq!(domain, "security"),
            _ => panic!("Expected UncoveredDomain gap"),
        }
        println!("[PASS] test_detect_domain_gaps_inactive_domain");
    }

    #[test]
    fn test_detect_weak_coverage() {
        let service = GapDetectionService::new();

        let goals = vec![
            create_test_goal(GoalLevel::Strategic, vec!["security"], 80, 40, 0.9), // Strong
            create_test_goal(GoalLevel::Tactical, vec!["performance"], 5, 2, 0.1), // Weak
        ];

        let gaps = service.detect_weak_coverage(&goals);
        assert_eq!(gaps.len(), 1);
        match &gaps[0] {
            GapType::WeakCoverage { coverage, .. } => {
                assert!(*coverage < 0.4);
            }
            _ => panic!("Expected WeakCoverage gap"),
        }
        println!("[PASS] test_detect_weak_coverage");
    }

    #[test]
    fn test_detect_weak_coverage_all_strong() {
        let service = GapDetectionService::new();

        let goals = vec![
            create_test_goal(GoalLevel::Strategic, vec!["security"], 80, 40, 0.9),
            create_test_goal(GoalLevel::Tactical, vec!["performance"], 70, 35, 0.85),
        ];

        let gaps = service.detect_weak_coverage(&goals);
        assert!(gaps.is_empty());
        println!("[PASS] test_detect_weak_coverage_all_strong");
    }

    #[test]
    fn test_detect_missing_links_none_expected() {
        let service = GapDetectionService::new();

        // Goals in different domains - no link expected
        let goals = vec![
            create_test_goal(GoalLevel::Strategic, vec!["security"], 50, 25, 0.8),
            create_test_goal(GoalLevel::Tactical, vec!["performance"], 50, 25, 0.8),
        ];

        let gaps = service.detect_missing_links(&goals);
        assert!(gaps.is_empty());
        println!("[PASS] test_detect_missing_links_none_expected");
    }

    #[test]
    fn test_detect_missing_links_shared_domain() {
        let service = GapDetectionService::new();

        // Goals share "security" domain but are not linked
        let goals = vec![
            create_test_goal(GoalLevel::Strategic, vec!["security", "auth"], 50, 25, 0.8),
            create_test_goal(GoalLevel::Tactical, vec!["security", "crypto"], 50, 25, 0.8),
        ];

        let gaps = service.detect_missing_links(&goals);
        assert_eq!(gaps.len(), 1);
        match &gaps[0] {
            GapType::MissingLink { .. } => {}
            _ => panic!("Expected MissingLink gap"),
        }
        println!("[PASS] test_detect_missing_links_shared_domain");
    }

    #[test]
    fn test_detect_missing_links_already_linked() {
        let service = GapDetectionService::new();

        let parent_id = GoalId::new();
        let child_id = GoalId::new();

        let parent_metrics = create_test_metrics(parent_id.clone(), 50, 25, 0.8);
        let child_metrics = create_test_metrics(child_id.clone(), 40, 20, 0.75);

        let goals = vec![
            GoalWithMetrics {
                goal_id: parent_id.clone(),
                level: GoalLevel::Strategic,
                description: "Parent".into(),
                parent_id: None,
                child_ids: vec![child_id.clone()],
                domains: vec!["security".into()],
                metrics: parent_metrics,
            },
            GoalWithMetrics {
                goal_id: child_id.clone(),
                level: GoalLevel::Tactical,
                description: "Child".into(),
                parent_id: Some(parent_id.clone()),
                child_ids: vec![],
                domains: vec!["security".into()],
                metrics: child_metrics,
            },
        ];

        let gaps = service.detect_missing_links(&goals);
        assert!(gaps.is_empty()); // No gap since they are already linked
        println!("[PASS] test_detect_missing_links_already_linked");
    }

    #[test]
    fn test_compute_coverage_score_empty() {
        let service = GapDetectionService::new();
        let score = service.compute_coverage_score(&[]);
        assert!((score - 0.0).abs() < f32::EPSILON);
        println!("[PASS] test_compute_coverage_score_empty");
    }

    #[test]
    fn test_compute_coverage_score_single_goal() {
        let service = GapDetectionService::new();

        let goals = vec![create_test_goal(
            GoalLevel::NorthStar,
            vec!["core"],
            100,
            50,
            1.0,
        )];

        let score = service.compute_coverage_score(&goals);
        assert!(score > 0.8); // High score for active NorthStar
        println!("[PASS] test_compute_coverage_score_single_goal");
    }

    #[test]
    fn test_compute_coverage_score_mixed_levels() {
        let service = GapDetectionService::new();

        let goals = vec![
            create_test_goal(GoalLevel::NorthStar, vec!["core"], 100, 50, 1.0),
            create_test_goal(GoalLevel::Strategic, vec!["security"], 80, 40, 0.9),
            create_test_goal(GoalLevel::Tactical, vec!["performance"], 60, 30, 0.8),
            create_test_goal(GoalLevel::Operational, vec!["logging"], 40, 20, 0.7),
        ];

        let score = service.compute_coverage_score(&goals);
        assert!(score > 0.6);
        assert!(score <= 1.0);
        println!("[PASS] test_compute_coverage_score_mixed_levels");
    }

    #[test]
    fn test_compute_coverage_score_all_inactive() {
        let service = GapDetectionService::new();

        let goals = vec![
            create_test_goal(GoalLevel::Strategic, vec!["security"], 0, 0, 0.3),
            create_test_goal(GoalLevel::Tactical, vec!["performance"], 0, 0, 0.2),
        ];

        let score = service.compute_coverage_score(&goals);
        assert!(score < 0.3); // Low score for inactive goals
        println!("[PASS] test_compute_coverage_score_all_inactive");
    }

    #[test]
    fn test_generate_recommendations_no_gaps() {
        let service = GapDetectionService::new();
        let recommendations = service.generate_recommendations(&[]);

        assert_eq!(recommendations.len(), 1);
        assert!(recommendations[0].contains("healthy"));
        println!("[PASS] test_generate_recommendations_no_gaps");
    }

    #[test]
    fn test_generate_recommendations_single_uncovered_domain() {
        let service = GapDetectionService::new();
        let gaps = vec![GapType::UncoveredDomain {
            domain: "security".into(),
        }];

        let recommendations = service.generate_recommendations(&gaps);
        assert!(!recommendations.is_empty());
        assert!(recommendations[0].contains("security"));
        println!("[PASS] test_generate_recommendations_single_uncovered_domain");
    }

    #[test]
    fn test_generate_recommendations_multiple_uncovered_domains() {
        let service = GapDetectionService::new();
        let gaps = vec![
            GapType::UncoveredDomain {
                domain: "security".into(),
            },
            GapType::UncoveredDomain {
                domain: "performance".into(),
            },
        ];

        let recommendations = service.generate_recommendations(&gaps);
        assert!(!recommendations.is_empty());
        assert!(recommendations[0].contains("2 uncovered domains"));
        println!("[PASS] test_generate_recommendations_multiple_uncovered_domains");
    }

    #[test]
    fn test_generate_recommendations_weak_coverage() {
        let service = GapDetectionService::new();
        let gaps = vec![GapType::WeakCoverage {
            goal_id: GoalId::new(),
            coverage: 0.2,
        }];

        let recommendations = service.generate_recommendations(&gaps);
        assert!(!recommendations.is_empty());
        assert!(recommendations[0].contains("weak coverage"));
        println!("[PASS] test_generate_recommendations_weak_coverage");
    }

    #[test]
    fn test_generate_recommendations_missing_links() {
        let service = GapDetectionService::new();
        let gaps = vec![GapType::MissingLink {
            from: GoalId::new(),
            to: GoalId::new(),
        }];

        let recommendations = service.generate_recommendations(&gaps);
        assert!(!recommendations.is_empty());
        assert!(recommendations[0].contains("link"));
        println!("[PASS] test_generate_recommendations_missing_links");
    }

    #[test]
    fn test_generate_recommendations_temporal_gaps() {
        let service = GapDetectionService::new();
        let gaps = vec![
            GapType::TemporalGap {
                period: "Q1 2024".into(),
            },
            GapType::TemporalGap {
                period: "Q2 2024".into(),
            },
        ];

        let recommendations = service.generate_recommendations(&gaps);
        assert!(!recommendations.is_empty());
        assert!(recommendations[0].contains("2 period(s)"));
        println!("[PASS] test_generate_recommendations_temporal_gaps");
    }

    #[test]
    fn test_generate_recommendations_mixed_gaps() {
        let service = GapDetectionService::new();
        let gaps = vec![
            GapType::UncoveredDomain {
                domain: "security".into(),
            },
            GapType::WeakCoverage {
                goal_id: GoalId::new(),
                coverage: 0.2,
            },
            GapType::MissingLink {
                from: GoalId::new(),
                to: GoalId::new(),
            },
        ];

        let recommendations = service.generate_recommendations(&gaps);
        assert!(recommendations.len() >= 3);
        println!("[PASS] test_generate_recommendations_mixed_gaps");
    }

    #[test]
    fn test_full_analysis_integration() {
        let config = GapDetectionConfig {
            coverage_threshold: 0.5,
            activity_threshold: 0.3,
            min_goals_per_domain: 1,
            detect_missing_links: true,
            ..Default::default()
        };
        let service = GapDetectionService::with_config(config);

        // Create a mix of goals with various issues
        let goals = vec![
            create_test_goal(GoalLevel::NorthStar, vec!["core"], 80, 40, 0.9),
            create_test_goal(GoalLevel::Strategic, vec!["security"], 60, 30, 0.85),
            create_test_goal(GoalLevel::Tactical, vec!["security"], 50, 25, 0.8), // Shares domain with above
            create_test_goal(GoalLevel::Operational, vec!["logging"], 5, 2, 0.1), // Weak
        ];

        let report = service.analyze_coverage(&goals);

        assert_eq!(report.goals_analyzed, 4);
        assert!(report.domains_detected >= 3);
        assert!(report.coverage_score > 0.0);
        assert!(report.coverage_score <= 1.0);

        // Should detect weak coverage for the logging goal
        let weak_gaps: Vec<_> = report
            .gaps
            .iter()
            .filter(|g| matches!(g, GapType::WeakCoverage { .. }))
            .collect();
        assert!(!weak_gaps.is_empty());

        // Should have recommendations
        assert!(!report.recommendations.is_empty());

        println!("[PASS] test_full_analysis_integration");
    }

    #[test]
    fn test_disable_missing_links_detection() {
        let config = GapDetectionConfig {
            detect_missing_links: false,
            ..Default::default()
        };
        let service = GapDetectionService::with_config(config);

        // Goals that would trigger missing link detection if enabled
        let goals = vec![
            create_test_goal(GoalLevel::Strategic, vec!["security"], 50, 25, 0.8),
            create_test_goal(GoalLevel::Tactical, vec!["security"], 50, 25, 0.8),
        ];

        let report = service.analyze_coverage(&goals);

        let link_gaps: Vec<_> = report
            .gaps
            .iter()
            .filter(|g| matches!(g, GapType::MissingLink { .. }))
            .collect();
        assert!(link_gaps.is_empty());
        println!("[PASS] test_disable_missing_links_detection");
    }
}
