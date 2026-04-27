//! Insight annotations and perspective coverage for retrieval results.
//!
//! Per constitution.yaml CLAUDE.md:
//! - 14 embedders = 14 unique perspectives on every memory
//! - Each finds what OTHERS MISS. Combined = superior answers.
//!
//! This module provides:
//! - `PerspectiveCoverage`: Tracks which embedders contribute to results
//! - `generate_insight_annotation()`: Creates human-readable annotations
//! - `compute_perspective_coverage()`: Computes coverage metrics for result sets
//!
//! # Philosophy
//!
//! When an embedder finds something unique, it's a "blind spot discovery":
//! - E1 finds: semantic similarity
//! - E11 finds: "Diesel" (knows Diesel IS a database ORM - E1 missed this)
//! - E7 finds: code using sqlx, diesel crates
//! - E5 finds: causal chains (why X caused Y)
//!
//! Annotations explain WHY each result was found, helping users understand
//! the multi-perspective retrieval.

use std::collections::HashSet;

use super::CombinedResult;

/// Embedder category for perspective analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbedderCategory {
    /// Semantic embedders (E1, E5, E6, E7, E10, E12, E13, E14) - topic_weight: 1.0
    Semantic,
    /// Relational embedders (E8, E11) - topic_weight: 0.5
    Relational,
    /// Structural embedders (E9) - topic_weight: 0.5
    Structural,
    /// Temporal embedders (E2, E3, E4) - topic_weight: 0.0, POST-RETRIEVAL ONLY
    Temporal,
}

impl EmbedderCategory {
    /// Get category for an embedder index.
    pub fn from_embedder_idx(idx: usize) -> Self {
        match idx {
            // SEMANTIC: E1, E5, E6, E7, E10, E12, E13, E14
            0 | 4 | 5 | 6 | 9 | 11 | 12 | 13 => Self::Semantic,
            // RELATIONAL: E8, E11
            7 | 10 => Self::Relational,
            // STRUCTURAL: E9
            8 => Self::Structural,
            // TEMPORAL: E2, E3, E4
            1..=3 => Self::Temporal,
            _ => Self::Semantic, // Default to semantic for unknown
        }
    }

    /// Get topic weight for this category (per constitution.yaml).
    pub fn topic_weight(&self) -> f32 {
        match self {
            Self::Semantic => 1.0,
            Self::Relational => 0.5,
            Self::Structural => 0.5,
            Self::Temporal => 0.0, // NEVER counts toward topics per ARCH-04
        }
    }
}

/// Perspective coverage for a set of retrieval results.
///
/// Tracks which embedders contributed to the result set and computes
/// coverage metrics for understanding retrieval quality.
#[derive(Debug, Clone)]
pub struct PerspectiveCoverage {
    /// Set of embedder indices that contributed to any result.
    pub embedders_contributing: HashSet<usize>,

    /// Coverage score [0.0, 1.0] - weighted by category.
    /// Formula: Sum(category_weight * is_present) / max_possible_weight
    pub coverage_score: f32,

    /// Embedders that did NOT contribute to any result.
    /// These represent potential "blind spots" in the retrieval.
    pub missing_perspectives: Vec<MissingPerspective>,

    /// Number of unique contributions (results found by only one embedder).
    pub unique_contribution_count: usize,

    /// Total results analyzed.
    pub total_results: usize,

    /// Breakdown by category.
    pub category_breakdown: CategoryBreakdown,
}

/// Information about a missing perspective (embedder that didn't contribute).
#[derive(Debug, Clone)]
pub struct MissingPerspective {
    /// Embedder index (0-13).
    pub embedder_idx: usize,

    /// Embedder name (e.g., "E7_Code").
    pub embedder_name: &'static str,

    /// What this embedder finds that others miss.
    pub finds_description: &'static str,

    /// Category of this embedder.
    pub category: EmbedderCategory,
}

/// Breakdown of coverage by embedder category.
#[derive(Debug, Clone, Default)]
pub struct CategoryBreakdown {
    /// Semantic embedders contributing (E1, E5, E6, E7, E10, E12, E13, E14).
    pub semantic_count: usize,
    pub semantic_total: usize,

    /// Relational embedders contributing (E8, E11).
    pub relational_count: usize,
    pub relational_total: usize,

    /// Structural embedders contributing (E9).
    pub structural_count: usize,
    pub structural_total: usize,

    /// Temporal embedders contributing (E2, E3, E4).
    pub temporal_count: usize,
    pub temporal_total: usize,
}

/// Embedder metadata for annotation generation.
struct EmbedderInfo {
    /// Index (0-13).
    idx: usize,
    /// Short name (e.g., "E7").
    short_name: &'static str,
    /// Display name (e.g., "E7_Code").
    display_name: &'static str,
    /// What this embedder finds.
    finds: &'static str,
    /// What E1 misses that this embedder catches.
    e1_blind_spot: &'static str,
    /// Category.
    category: EmbedderCategory,
}

/// Static embedder metadata per constitution.yaml.
const EMBEDDERS: [EmbedderInfo; 14] = [
    EmbedderInfo {
        idx: 0,
        short_name: "E1",
        display_name: "E1_Semantic",
        finds: "semantic similarity",
        e1_blind_spot: "", // E1 is the foundation
        category: EmbedderCategory::Semantic,
    },
    EmbedderInfo {
        idx: 1,
        short_name: "E2",
        display_name: "E2_Temporal_Recent",
        finds: "recency",
        e1_blind_spot: "temporal context",
        category: EmbedderCategory::Temporal,
    },
    EmbedderInfo {
        idx: 2,
        short_name: "E3",
        display_name: "E3_Temporal_Periodic",
        finds: "time-of-day patterns",
        e1_blind_spot: "temporal patterns",
        category: EmbedderCategory::Temporal,
    },
    EmbedderInfo {
        idx: 3,
        short_name: "E4",
        display_name: "E4_Temporal_Positional",
        finds: "sequence (before/after)",
        e1_blind_spot: "conversation order",
        category: EmbedderCategory::Temporal,
    },
    EmbedderInfo {
        idx: 4,
        short_name: "E5",
        display_name: "E5_Causal",
        finds: "causal chains (why X caused Y)",
        e1_blind_spot: "direction lost in semantic averaging",
        category: EmbedderCategory::Semantic,
    },
    EmbedderInfo {
        idx: 5,
        short_name: "E6",
        display_name: "E6_Sparse",
        finds: "exact keyword matches",
        e1_blind_spot: "diluted by semantic averaging",
        category: EmbedderCategory::Semantic,
    },
    EmbedderInfo {
        idx: 6,
        short_name: "E7",
        display_name: "E7_Code",
        finds: "code patterns, function signatures",
        e1_blind_spot: "treats code as natural language",
        category: EmbedderCategory::Semantic,
    },
    EmbedderInfo {
        idx: 7,
        short_name: "E8",
        display_name: "E8_Graph",
        finds: "graph structure (X imports Y)",
        e1_blind_spot: "structural relationships",
        category: EmbedderCategory::Relational,
    },
    EmbedderInfo {
        idx: 8,
        short_name: "E9",
        display_name: "E9_HDC",
        finds: "noise-robust structure",
        e1_blind_spot: "fragile to perturbations",
        category: EmbedderCategory::Structural,
    },
    EmbedderInfo {
        idx: 9,
        short_name: "E10",
        display_name: "E10_Multimodal",
        finds: "same-goal work (different words)",
        e1_blind_spot: "vocabulary mismatch",
        category: EmbedderCategory::Semantic,
    },
    EmbedderInfo {
        idx: 10,
        short_name: "E11",
        display_name: "E11_Entity",
        finds: "entity knowledge (Diesel=database ORM)",
        e1_blind_spot: "entity relationships",
        category: EmbedderCategory::Relational,
    },
    EmbedderInfo {
        idx: 11,
        short_name: "E12",
        display_name: "E12_Late_Interaction",
        finds: "exact phrase matches",
        e1_blind_spot: "phrase-level precision",
        category: EmbedderCategory::Semantic,
    },
    EmbedderInfo {
        idx: 12,
        short_name: "E13",
        display_name: "E13_SPLADE",
        finds: "term expansions (fast→quick)",
        e1_blind_spot: "term variations",
        category: EmbedderCategory::Semantic,
    },
    EmbedderInfo {
        idx: 13,
        short_name: "E14",
        display_name: "E14_BgeM3Dense",
        finds: "multilingual semantic/style similarity via BGE-M3 dense head",
        e1_blind_spot: "non-English / style-discriminative content",
        category: EmbedderCategory::Semantic,
    },
];

/// Generate a human-readable insight annotation for a result.
///
/// Creates annotations like:
/// - "Found by E11 (entity): knows Diesel IS a database ORM"
/// - "Found by E7 (code): detected function signature pattern"
/// - "Found by E1+E5+E7: semantic match with causal and code patterns"
///
/// # Arguments
/// - `result`: The combined result with contribution tracking
/// - `content_preview`: Optional content preview for context
///
/// # Returns
/// Human-readable annotation string
pub fn generate_insight_annotation(
    result: &CombinedResult,
    content_preview: Option<&str>,
) -> String {
    let embedder_count = result.found_by.len();

    if embedder_count == 0 {
        return "No embedder contributions tracked".to_string();
    }

    // Single embedder - unique contribution, detailed annotation
    if embedder_count == 1 {
        let embedder_idx = *result.found_by.iter().next().unwrap();
        let info = &EMBEDDERS[embedder_idx.min(13)];

        let annotation = if result.unique_contribution {
            // For blind spot discoveries, explain what E1 missed
            if !info.e1_blind_spot.is_empty() {
                format!(
                    "BLIND SPOT DISCOVERY by {} ({}): {} - E1 missed: {}",
                    info.short_name,
                    category_label(info.category),
                    info.finds,
                    info.e1_blind_spot
                )
            } else {
                format!(
                    "BLIND SPOT DISCOVERY by {} ({}): {}",
                    info.short_name,
                    category_label(info.category),
                    info.finds
                )
            }
        } else {
            format!(
                "Found by {} ({}): {}",
                info.short_name,
                category_label(info.category),
                info.finds
            )
        };

        // Add content context if available and this is a blind spot
        if result.unique_contribution {
            if let Some(preview) = content_preview {
                let preview_short = if preview.len() > 50 {
                    format!("{}...", &preview[..47])
                } else {
                    preview.to_string()
                };
                return format!("{} | Content: \"{}\"", annotation, preview_short);
            }
        }

        return annotation;
    }

    // Multiple embedders - build combined annotation
    let primary_info = &EMBEDDERS[result.primary_embedder.min(13)];

    // Collect embedder short names
    let mut embedder_names: Vec<&str> = result
        .found_by
        .iter()
        .map(|idx| EMBEDDERS[(*idx).min(13)].short_name)
        .collect();
    embedder_names.sort();

    // Build category summary
    let mut categories: Vec<&str> = Vec::new();
    let has_semantic = result
        .found_by
        .iter()
        .any(|idx| EmbedderCategory::from_embedder_idx(*idx) == EmbedderCategory::Semantic);
    let has_relational = result
        .found_by
        .iter()
        .any(|idx| EmbedderCategory::from_embedder_idx(*idx) == EmbedderCategory::Relational);
    let has_structural = result
        .found_by
        .iter()
        .any(|idx| EmbedderCategory::from_embedder_idx(*idx) == EmbedderCategory::Structural);
    let has_temporal = result
        .found_by
        .iter()
        .any(|idx| EmbedderCategory::from_embedder_idx(*idx) == EmbedderCategory::Temporal);

    if has_semantic {
        categories.push("semantic");
    }
    if has_relational {
        categories.push("relational");
    }
    if has_structural {
        categories.push("structural");
    }
    if has_temporal {
        categories.push("temporal");
    }

    format!(
        "Found by {} ({}): primary {} at rank {} | {} perspective{}",
        embedder_names.join("+"),
        categories.join("+"),
        primary_info.short_name,
        result.best_rank,
        embedder_count,
        if embedder_count == 1 { "" } else { "s" }
    )
}

/// Compute perspective coverage for a set of results.
///
/// Analyzes which embedders contributed to the result set and identifies
/// missing perspectives (potential blind spots).
///
/// # Arguments
/// - `results`: Slice of combined results with contribution tracking
///
/// # Returns
/// `PerspectiveCoverage` with metrics and missing perspective details
pub fn compute_perspective_coverage(results: &[CombinedResult]) -> PerspectiveCoverage {
    let mut embedders_contributing: HashSet<usize> = HashSet::new();
    let mut unique_contribution_count = 0;

    // Collect all contributing embedders
    for result in results {
        embedders_contributing.extend(&result.found_by);
        if result.unique_contribution {
            unique_contribution_count += 1;
        }
    }

    // Build category breakdown
    let mut breakdown = CategoryBreakdown {
        semantic_total: 8,   // E1, E5, E6, E7, E10, E12, E13, E14
        relational_total: 2, // E8, E11
        structural_total: 1, // E9
        temporal_total: 3,   // E2, E3, E4
        ..Default::default()
    };

    for idx in &embedders_contributing {
        match EmbedderCategory::from_embedder_idx(*idx) {
            EmbedderCategory::Semantic => breakdown.semantic_count += 1,
            EmbedderCategory::Relational => breakdown.relational_count += 1,
            EmbedderCategory::Structural => breakdown.structural_count += 1,
            EmbedderCategory::Temporal => breakdown.temporal_count += 1,
        }
    }

    // Compute coverage score (weighted by category)
    // Max possible: 8×1.0 (semantic incl. E14) + 2×0.5 (relational) + 1×0.5 (structural) + 3×0.0 (temporal) = 9.5
    // Note: Temporal doesn't contribute to topic coverage per ARCH-04
    let max_weight: f32 = 8.0 * 1.0 + 2.0 * 0.5 + 1.0 * 0.5; // 9.5
    let actual_weight: f32 = embedders_contributing
        .iter()
        .map(|idx| EmbedderCategory::from_embedder_idx(*idx).topic_weight())
        .sum();
    let coverage_score = if max_weight > 0.0 {
        actual_weight / max_weight
    } else {
        0.0
    };

    // Identify missing perspectives
    let mut missing_perspectives: Vec<MissingPerspective> = Vec::new();
    for info in &EMBEDDERS {
        if !embedders_contributing.contains(&info.idx) {
            missing_perspectives.push(MissingPerspective {
                embedder_idx: info.idx,
                embedder_name: info.display_name,
                finds_description: info.finds,
                category: info.category,
            });
        }
    }

    PerspectiveCoverage {
        embedders_contributing,
        coverage_score,
        missing_perspectives,
        unique_contribution_count,
        total_results: results.len(),
        category_breakdown: breakdown,
    }
}

/// Annotate results with insight annotations.
///
/// Modifies results in-place to add human-readable annotations.
///
/// # Arguments
/// - `results`: Mutable slice of combined results
/// - `contents`: Optional content strings for enhanced annotations
pub fn annotate_results(results: &mut [CombinedResult], contents: Option<&[Option<String>]>) {
    for (i, result) in results.iter_mut().enumerate() {
        let content_preview = contents
            .and_then(|c| c.get(i))
            .and_then(|opt| opt.as_ref())
            .map(|s| s.as_str());

        result.insight_annotation = Some(generate_insight_annotation(result, content_preview));
    }
}

/// Get category label for display.
fn category_label(category: EmbedderCategory) -> &'static str {
    match category {
        EmbedderCategory::Semantic => "semantic",
        EmbedderCategory::Relational => "relational",
        EmbedderCategory::Structural => "structural",
        EmbedderCategory::Temporal => "temporal",
    }
}

/// Get embedder info by index.
pub fn get_embedder_info(idx: usize) -> (&'static str, &'static str, &'static str) {
    let info = &EMBEDDERS[idx.min(13)];
    (info.short_name, info.display_name, info.finds)
}

/// Get all embedder names for a set of indices.
pub fn embedder_names_for_indices(indices: &HashSet<usize>) -> Vec<&'static str> {
    let mut names: Vec<&str> = indices
        .iter()
        .map(|idx| EMBEDDERS[(*idx).min(13)].display_name)
        .collect();
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn make_result(
        found_by: Vec<usize>,
        primary: usize,
        best_rank: usize,
        unique: bool,
    ) -> CombinedResult {
        CombinedResult {
            memory_id: Uuid::new_v4(),
            rrf_score: 0.1,
            boosted_score: if unique { 0.11 } else { 0.1 },
            found_by: found_by.into_iter().collect(),
            primary_embedder: primary,
            unique_contribution: unique,
            best_rank,
            insight_annotation: None,
        }
    }

    #[test]
    fn test_embedder_category() {
        // Semantic: E1, E5, E6, E7, E10, E12, E13
        assert_eq!(
            EmbedderCategory::from_embedder_idx(0),
            EmbedderCategory::Semantic
        );
        assert_eq!(
            EmbedderCategory::from_embedder_idx(4),
            EmbedderCategory::Semantic
        );
        assert_eq!(
            EmbedderCategory::from_embedder_idx(5),
            EmbedderCategory::Semantic
        );
        assert_eq!(
            EmbedderCategory::from_embedder_idx(6),
            EmbedderCategory::Semantic
        );
        assert_eq!(
            EmbedderCategory::from_embedder_idx(9),
            EmbedderCategory::Semantic
        );
        assert_eq!(
            EmbedderCategory::from_embedder_idx(11),
            EmbedderCategory::Semantic
        );
        assert_eq!(
            EmbedderCategory::from_embedder_idx(12),
            EmbedderCategory::Semantic
        );

        // Relational: E8, E11
        assert_eq!(
            EmbedderCategory::from_embedder_idx(7),
            EmbedderCategory::Relational
        );
        assert_eq!(
            EmbedderCategory::from_embedder_idx(10),
            EmbedderCategory::Relational
        );

        // Structural: E9
        assert_eq!(
            EmbedderCategory::from_embedder_idx(8),
            EmbedderCategory::Structural
        );

        // Temporal: E2, E3, E4
        assert_eq!(
            EmbedderCategory::from_embedder_idx(1),
            EmbedderCategory::Temporal
        );
        assert_eq!(
            EmbedderCategory::from_embedder_idx(2),
            EmbedderCategory::Temporal
        );
        assert_eq!(
            EmbedderCategory::from_embedder_idx(3),
            EmbedderCategory::Temporal
        );

        println!("[VERIFIED] Embedder categories match constitution.yaml");
    }

    #[test]
    fn test_topic_weights() {
        assert!((EmbedderCategory::Semantic.topic_weight() - 1.0).abs() < 0.001);
        assert!((EmbedderCategory::Relational.topic_weight() - 0.5).abs() < 0.001);
        assert!((EmbedderCategory::Structural.topic_weight() - 0.5).abs() < 0.001);
        assert!((EmbedderCategory::Temporal.topic_weight() - 0.0).abs() < 0.001);

        println!("[VERIFIED] Topic weights per ARCH-04 and constitution.yaml");
    }

    #[test]
    fn test_annotation_single_embedder() {
        let result = make_result(vec![10], 10, 0, true);
        let annotation = generate_insight_annotation(&result, None);

        assert!(annotation.contains("E11"));
        assert!(annotation.contains("BLIND SPOT"));
        assert!(annotation.contains("entity"));
        assert!(annotation.contains("E1 missed")); // Explains what E1 missed
        println!("Annotation: {}", annotation);

        println!("[VERIFIED] Single embedder annotation with blind spot and E1 explanation");
    }

    #[test]
    fn test_annotation_multiple_embedders() {
        let result = make_result(vec![0, 4, 6], 0, 0, false);
        let annotation = generate_insight_annotation(&result, None);

        assert!(annotation.contains("E1"));
        assert!(annotation.contains("E5"));
        assert!(annotation.contains("E7"));
        assert!(annotation.contains("semantic"));
        assert!(annotation.contains("3 perspectives"));
        println!("Annotation: {}", annotation);

        println!("[VERIFIED] Multiple embedder annotation");
    }

    #[test]
    fn test_annotation_with_content_preview() {
        let result = make_result(vec![10], 10, 0, true);
        let annotation =
            generate_insight_annotation(&result, Some("Diesel is a type-safe ORM for Rust"));

        assert!(annotation.contains("Diesel"));
        assert!(annotation.contains("Content:"));
        println!("Annotation: {}", annotation);

        println!("[VERIFIED] Annotation includes content preview for blind spots");
    }

    #[test]
    fn test_perspective_coverage_full() {
        // Results from all 14 embedders
        let results = vec![
            make_result(vec![0, 1, 2, 3, 4, 5, 6], 0, 0, false),
            make_result(vec![7, 8, 9, 10, 11, 12, 13], 7, 0, false),
        ];

        let coverage = compute_perspective_coverage(&results);

        assert_eq!(coverage.embedders_contributing.len(), 14);
        assert!(coverage.missing_perspectives.is_empty());
        assert!((coverage.coverage_score - 1.0).abs() < 0.001);
        assert_eq!(coverage.category_breakdown.semantic_count, 8);
        assert_eq!(coverage.category_breakdown.relational_count, 2);
        assert_eq!(coverage.category_breakdown.structural_count, 1);
        assert_eq!(coverage.category_breakdown.temporal_count, 3);

        println!("[VERIFIED] Full coverage when all embedders contribute");
    }

    #[test]
    fn test_perspective_coverage_partial() {
        // Only E1 and E11
        let results = vec![make_result(vec![0, 10], 0, 0, false)];

        let coverage = compute_perspective_coverage(&results);

        assert_eq!(coverage.embedders_contributing.len(), 2);
        assert_eq!(coverage.missing_perspectives.len(), 12);

        // E1 (1.0) + E11 (0.5) = 1.5 / 9.5 ≈ 0.158 (post-E14)
        let expected_score = 1.5 / 9.5;
        assert!((coverage.coverage_score - expected_score).abs() < 0.01);

        println!(
            "[VERIFIED] Partial coverage: score={:.3}, missing={}",
            coverage.coverage_score,
            coverage.missing_perspectives.len()
        );
    }

    #[test]
    fn test_perspective_coverage_unique_contributions() {
        let results = vec![
            make_result(vec![0, 4], 0, 0, false),
            make_result(vec![10], 10, 0, true), // unique
            make_result(vec![6], 6, 0, true),   // unique
        ];

        let coverage = compute_perspective_coverage(&results);

        assert_eq!(coverage.unique_contribution_count, 2);
        assert_eq!(coverage.total_results, 3);

        println!("[VERIFIED] Unique contribution count tracked");
    }

    #[test]
    fn test_annotate_results() {
        let mut results = vec![
            make_result(vec![10], 10, 0, true),
            make_result(vec![0, 6], 0, 0, false),
        ];

        let contents = vec![
            Some("Diesel ORM for Rust".to_string()),
            Some("fn process_data()".to_string()),
        ];

        annotate_results(&mut results, Some(&contents));

        assert!(results[0].insight_annotation.is_some());
        assert!(results[1].insight_annotation.is_some());

        println!(
            "Result 0: {}",
            results[0].insight_annotation.as_ref().unwrap()
        );
        println!(
            "Result 1: {}",
            results[1].insight_annotation.as_ref().unwrap()
        );

        println!("[VERIFIED] annotate_results populates annotations");
    }

    #[test]
    fn test_missing_perspectives_info() {
        let results = vec![make_result(vec![0], 0, 0, false)]; // Only E1

        let coverage = compute_perspective_coverage(&results);

        // Should have 13 missing perspectives (E2-E14)
        assert_eq!(coverage.missing_perspectives.len(), 13);

        // Find E11 in missing
        let e11_missing = coverage
            .missing_perspectives
            .iter()
            .find(|m| m.embedder_idx == 10);
        assert!(e11_missing.is_some());
        let e11 = e11_missing.unwrap();
        assert_eq!(e11.embedder_name, "E11_Entity");
        assert!(e11.finds_description.contains("entity"));

        println!("[VERIFIED] Missing perspectives have detailed info");
    }

    #[test]
    fn test_embedder_names_for_indices() {
        let indices: HashSet<usize> = [0, 6, 10].into_iter().collect();
        let names = embedder_names_for_indices(&indices);

        assert_eq!(names.len(), 3);
        assert!(names.contains(&"E1_Semantic"));
        assert!(names.contains(&"E7_Code"));
        assert!(names.contains(&"E11_Entity"));

        println!("[VERIFIED] embedder_names_for_indices returns correct names");
    }

    #[test]
    fn test_get_embedder_info() {
        let (short, display, finds) = get_embedder_info(10);
        assert_eq!(short, "E11");
        assert_eq!(display, "E11_Entity");
        assert!(finds.contains("entity"));

        println!("[VERIFIED] get_embedder_info returns correct metadata");
    }
}
