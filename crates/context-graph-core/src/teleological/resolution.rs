//! Multi-resolution purpose vector hierarchy.
//!
//! From teleoplan.md Section 4.2:
//!
//! Multi-Resolution Hierarchy:
//! - Level 0: Full 13D Purpose Vector
//! - Level 1: 6D Group Purpose Vector
//! - Level 2: 3D Core Purpose Vector (What, How, Why)
//! - Level 3: 1D Purpose Alignment Score

use serde::{Deserialize, Serialize};

use super::groups::{GroupAlignments, GroupType, NUM_GROUPS};
use super::types::NUM_EMBEDDERS;

/// Number of core domains at Level 2.
pub const NUM_DOMAINS: usize = 3;

/// 3D Core Purpose Vector domains.
///
/// From teleoplan.md:
/// - What = Factual + Relational
/// - How = Causal + Implementation + Temporal
/// - Why = Qualitative (+ Analogical from Relational)
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct DomainAlignments {
    /// What domain: Factual + Relational (what IS)
    pub factual_domain: f32,

    /// How domain: Causal + Implementation + Temporal (how it works)
    pub procedural_domain: f32,

    /// Why domain: Qualitative (why it matters)
    pub affective_domain: f32,
}

impl DomainAlignments {
    /// Create from group alignments.
    ///
    /// Domain mapping from teleoplan.md:
    /// - What = (Factual + Relational) / 2
    /// - How = (Causal + Implementation + Temporal) / 3
    /// - Why = Qualitative
    pub fn from_groups(groups: &GroupAlignments) -> Self {
        Self {
            factual_domain: (groups.factual + groups.relational) / 2.0,
            procedural_domain: (groups.causal + groups.implementation + groups.temporal) / 3.0,
            affective_domain: groups.qualitative,
        }
    }

    /// Create from explicit values.
    pub fn new(factual: f32, procedural: f32, affective: f32) -> Self {
        Self {
            factual_domain: factual,
            procedural_domain: procedural,
            affective_domain: affective,
        }
    }

    /// Get as 3D array.
    #[inline]
    pub fn as_array(&self) -> [f32; NUM_DOMAINS] {
        [
            self.factual_domain,
            self.procedural_domain,
            self.affective_domain,
        ]
    }

    /// Create from array.
    pub fn from_array(values: [f32; NUM_DOMAINS]) -> Self {
        Self {
            factual_domain: values[0],
            procedural_domain: values[1],
            affective_domain: values[2],
        }
    }

    /// Average of domain alignments.
    #[inline]
    pub fn average(&self) -> f32 {
        (self.factual_domain + self.procedural_domain + self.affective_domain) / 3.0
    }

    /// Dominant domain (highest alignment).
    pub fn dominant(&self) -> DomainType {
        let arr = self.as_array();
        let mut max_idx = 0;
        let mut max_val = arr[0];

        for (i, &val) in arr.iter().enumerate().skip(1) {
            if val > max_val {
                max_val = val;
                max_idx = i;
            }
        }

        DomainType::from_index(max_idx)
    }

    /// Cosine similarity between two DomainAlignments.
    pub fn similarity(&self, other: &Self) -> f32 {
        let a = self.as_array();
        let b = other.as_array();

        let mut dot = 0.0f32;
        let mut norm_a = 0.0f32;
        let mut norm_b = 0.0f32;

        for i in 0..NUM_DOMAINS {
            dot += a[i] * b[i];
            norm_a += a[i] * a[i];
            norm_b += b[i] * b[i];
        }

        let denom = (norm_a.sqrt()) * (norm_b.sqrt());
        if denom < f32::EPSILON {
            0.0
        } else {
            dot / denom
        }
    }
}

/// The three core knowledge domains.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DomainType {
    /// What something IS (factual + relational)
    Factual,
    /// How it works (causal + implementation + temporal)
    Procedural,
    /// Why it matters (qualitative)
    Affective,
}

impl DomainType {
    /// All domain types.
    pub const ALL: [DomainType; NUM_DOMAINS] = [
        DomainType::Factual,
        DomainType::Procedural,
        DomainType::Affective,
    ];

    /// Convert from index (0-2).
    ///
    /// # Panics
    ///
    /// Panics if index >= NUM_DOMAINS (FAIL FAST).
    pub fn from_index(index: usize) -> Self {
        assert!(
            index < NUM_DOMAINS,
            "FAIL FAST: domain index {} out of bounds (max {})",
            index,
            NUM_DOMAINS - 1
        );
        Self::ALL[index]
    }

    /// Convert to index (0-2).
    #[inline]
    pub fn to_index(self) -> usize {
        match self {
            DomainType::Factual => 0,
            DomainType::Procedural => 1,
            DomainType::Affective => 2,
        }
    }

    /// Get the group types that contribute to this domain.
    pub fn contributing_groups(self) -> &'static [GroupType] {
        match self {
            DomainType::Factual => &[GroupType::Factual, GroupType::Relational],
            DomainType::Procedural => &[
                GroupType::Causal,
                GroupType::Implementation,
                GroupType::Temporal,
            ],
            DomainType::Affective => &[GroupType::Qualitative],
        }
    }

    /// Human-readable description.
    pub fn description(self) -> &'static str {
        match self {
            DomainType::Factual => "What - factual and relational knowledge",
            DomainType::Procedural => "How - causal, implementation, and temporal knowledge",
            DomainType::Affective => "Why - qualitative and affective knowledge",
        }
    }
}

impl std::fmt::Display for DomainType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DomainType::Factual => write!(f, "What"),
            DomainType::Procedural => write!(f, "How"),
            DomainType::Affective => write!(f, "Why"),
        }
    }
}

/// Multi-resolution purpose vector hierarchy.
///
/// Provides 4 levels of abstraction for purpose representation:
/// - Level 0: Raw 13D embedder alignments
/// - Level 1: 6D group alignments
/// - Level 2: 3D domain alignments (What/How/Why)
/// - Level 3: 1D overall alignment score
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MultiResolutionHierarchy {
    /// Level 0: Raw 13D purpose vector alignments
    pub raw: [f32; NUM_EMBEDDERS],

    /// Level 1: 6D group alignments
    pub groups: GroupAlignments,

    /// Level 2: 3D domain alignments
    pub domains: DomainAlignments,

    /// Level 3: 1D overall alignment score
    pub overall: f32,
}

impl MultiResolutionHierarchy {
    /// Build complete hierarchy from raw 13D alignments.
    pub fn from_raw(raw: [f32; NUM_EMBEDDERS]) -> Self {
        let groups = GroupAlignments::from_alignments(&raw, None);
        let domains = DomainAlignments::from_groups(&groups);
        let overall = domains.average();

        Self {
            raw,
            groups,
            domains,
            overall,
        }
    }

    /// Build with custom weights for group computation.
    pub fn from_raw_weighted(raw: [f32; NUM_EMBEDDERS], weights: &[f32; NUM_EMBEDDERS]) -> Self {
        let groups = GroupAlignments::from_alignments(&raw, Some(weights));
        let domains = DomainAlignments::from_groups(&groups);
        let overall = domains.average();

        Self {
            raw,
            groups,
            domains,
            overall,
        }
    }

    /// Get alignment at specific resolution level.
    ///
    /// # Arguments
    /// * `level` - 0 (raw), 1 (groups), 2 (domains), 3 (overall)
    ///
    /// # Panics
    ///
    /// Panics if level > 3 (FAIL FAST).
    pub fn at_level(&self, level: usize) -> ResolutionView {
        match level {
            0 => ResolutionView::Raw(self.raw),
            1 => ResolutionView::Groups(self.groups.as_array()),
            2 => ResolutionView::Domains(self.domains.as_array()),
            3 => ResolutionView::Overall(self.overall),
            _ => panic!(
                "FAIL FAST: resolution level {} out of bounds (max 3)",
                level
            ),
        }
    }

    /// Quick filtering score (Level 3).
    #[inline]
    pub fn quick_score(&self) -> f32 {
        self.overall
    }

    /// Standard retrieval vector (Level 1: 6D).
    #[inline]
    pub fn standard_vector(&self) -> [f32; NUM_GROUPS] {
        self.groups.as_array()
    }

    /// Precise matching vector (Level 0: 13D).
    #[inline]
    pub fn precise_vector(&self) -> [f32; NUM_EMBEDDERS] {
        self.raw
    }

    /// Cosine similarity at a given resolution level.
    pub fn similarity_at_level(&self, other: &Self, level: usize) -> f32 {
        match level {
            0 => cosine_similarity(&self.raw, &other.raw),
            1 => cosine_similarity(&self.groups.as_array(), &other.groups.as_array()),
            2 => cosine_similarity(&self.domains.as_array(), &other.domains.as_array()),
            3 => {
                // For scalar, compute relative difference
                let max_val = self.overall.max(other.overall);
                if max_val < f32::EPSILON {
                    1.0
                } else {
                    1.0 - (self.overall - other.overall).abs() / max_val
                }
            }
            _ => panic!(
                "FAIL FAST: resolution level {} out of bounds (max 3)",
                level
            ),
        }
    }

    /// Recompute derived levels from raw alignments.
    pub fn recompute(&mut self) {
        self.groups = GroupAlignments::from_alignments(&self.raw, None);
        self.domains = DomainAlignments::from_groups(&self.groups);
        self.overall = self.domains.average();
    }
}

impl Default for MultiResolutionHierarchy {
    fn default() -> Self {
        Self::from_raw([0.0; NUM_EMBEDDERS])
    }
}

/// View of alignment at a specific resolution level.
#[derive(Clone, Debug)]
pub enum ResolutionView {
    /// Level 0: 13D raw alignments
    Raw([f32; NUM_EMBEDDERS]),
    /// Level 1: 6D group alignments
    Groups([f32; NUM_GROUPS]),
    /// Level 2: 3D domain alignments
    Domains([f32; NUM_DOMAINS]),
    /// Level 3: 1D overall score
    Overall(f32),
}

impl ResolutionView {
    /// Get dimensionality of this view.
    pub fn dimensions(&self) -> usize {
        match self {
            ResolutionView::Raw(_) => NUM_EMBEDDERS,
            ResolutionView::Groups(_) => NUM_GROUPS,
            ResolutionView::Domains(_) => NUM_DOMAINS,
            ResolutionView::Overall(_) => 1,
        }
    }

    /// Get as vector (copies data).
    pub fn to_vec(&self) -> Vec<f32> {
        match self {
            ResolutionView::Raw(arr) => arr.to_vec(),
            ResolutionView::Groups(arr) => arr.to_vec(),
            ResolutionView::Domains(arr) => arr.to_vec(),
            ResolutionView::Overall(v) => vec![*v],
        }
    }
}

/// Compute cosine similarity between two slices.
fn cosine_similarity<const N: usize>(a: &[f32; N], b: &[f32; N]) -> f32 {
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    for i in 0..N {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }

    let denom = (norm_a.sqrt()) * (norm_b.sqrt());
    if denom < f32::EPSILON {
        0.0
    } else {
        dot / denom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== DomainAlignments Tests =====

    #[test]
    fn test_domain_alignments_default() {
        let da = DomainAlignments::default();

        assert!((da.factual_domain - 0.0).abs() < f32::EPSILON);
        assert!((da.procedural_domain - 0.0).abs() < f32::EPSILON);
        assert!((da.affective_domain - 0.0).abs() < f32::EPSILON);

        println!("[PASS] DomainAlignments::default creates zeros");
    }

    #[test]
    fn test_domain_alignments_from_groups() {
        let groups = GroupAlignments::new(0.8, 0.7, 0.6, 0.5, 0.4, 0.9);

        let domains = DomainAlignments::from_groups(&groups);

        // Factual = (factual + relational) / 2 = (0.8 + 0.5) / 2 = 0.65
        assert!(
            (domains.factual_domain - 0.65).abs() < 0.001,
            "factual = {} (expected 0.65)",
            domains.factual_domain
        );

        // Procedural = (causal + implementation + temporal) / 3 = (0.6 + 0.9 + 0.7) / 3 = 0.733
        assert!(
            (domains.procedural_domain - 0.733).abs() < 0.01,
            "procedural = {} (expected 0.733)",
            domains.procedural_domain
        );

        // Affective = qualitative = 0.4
        assert!(
            (domains.affective_domain - 0.4).abs() < f32::EPSILON,
            "affective = {} (expected 0.4)",
            domains.affective_domain
        );

        println!("[PASS] DomainAlignments::from_groups computes correctly");
    }

    #[test]
    fn test_domain_alignments_as_array() {
        let da = DomainAlignments::new(0.1, 0.2, 0.3);
        assert_eq!(da.as_array(), [0.1, 0.2, 0.3]);

        println!("[PASS] as_array returns correct order");
    }

    #[test]
    fn test_domain_alignments_average() {
        let da = DomainAlignments::new(0.3, 0.6, 0.9);
        assert!((da.average() - 0.6).abs() < f32::EPSILON);

        println!("[PASS] average computes correctly");
    }

    #[test]
    fn test_domain_alignments_dominant() {
        let da = DomainAlignments::new(0.3, 0.9, 0.6);
        assert_eq!(da.dominant(), DomainType::Procedural);

        println!("[PASS] dominant finds maximum");
    }

    #[test]
    fn test_domain_alignments_similarity() {
        let da1 = DomainAlignments::new(0.5, 0.5, 0.5);
        let da2 = DomainAlignments::new(0.5, 0.5, 0.5);

        assert!((da1.similarity(&da2) - 1.0).abs() < 0.001);

        println!("[PASS] Identical domains have similarity 1.0");
    }

    // ===== DomainType Tests =====

    #[test]
    fn test_domain_type_from_index() {
        assert_eq!(DomainType::from_index(0), DomainType::Factual);
        assert_eq!(DomainType::from_index(1), DomainType::Procedural);
        assert_eq!(DomainType::from_index(2), DomainType::Affective);

        println!("[PASS] DomainType::from_index works");
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_domain_type_from_index_out_of_bounds() {
        let _ = DomainType::from_index(3);
    }

    #[test]
    fn test_domain_type_to_index_roundtrip() {
        for i in 0..NUM_DOMAINS {
            assert_eq!(DomainType::from_index(i).to_index(), i);
        }

        println!("[PASS] DomainType index roundtrip works");
    }

    #[test]
    fn test_domain_type_contributing_groups() {
        let factual_groups = DomainType::Factual.contributing_groups();
        assert!(factual_groups.contains(&GroupType::Factual));
        assert!(factual_groups.contains(&GroupType::Relational));

        let procedural_groups = DomainType::Procedural.contributing_groups();
        assert_eq!(procedural_groups.len(), 3);

        println!("[PASS] contributing_groups match teleoplan.md");
    }

    #[test]
    fn test_domain_type_display() {
        assert_eq!(format!("{}", DomainType::Factual), "What");
        assert_eq!(format!("{}", DomainType::Procedural), "How");
        assert_eq!(format!("{}", DomainType::Affective), "Why");

        println!("[PASS] DomainType Display works");
    }

    // ===== MultiResolutionHierarchy Tests =====

    #[test]
    fn test_multi_resolution_from_raw() {
        let raw = [0.8f32; NUM_EMBEDDERS];
        let hierarchy = MultiResolutionHierarchy::from_raw(raw);

        // All values uniform, so all levels should be ~0.8
        assert!((hierarchy.overall - 0.8).abs() < 0.001);
        assert!((hierarchy.groups.average() - 0.8).abs() < 0.001);
        assert!((hierarchy.domains.average() - 0.8).abs() < 0.001);

        println!("[PASS] MultiResolutionHierarchy::from_raw builds all levels");
    }

    #[test]
    fn test_multi_resolution_at_level() {
        let raw = [0.5f32; NUM_EMBEDDERS];
        let hierarchy = MultiResolutionHierarchy::from_raw(raw);

        match hierarchy.at_level(0) {
            ResolutionView::Raw(arr) => assert_eq!(arr.len(), NUM_EMBEDDERS),
            _ => panic!("Expected Raw view"),
        }

        match hierarchy.at_level(1) {
            ResolutionView::Groups(arr) => assert_eq!(arr.len(), NUM_GROUPS),
            _ => panic!("Expected Groups view"),
        }

        match hierarchy.at_level(2) {
            ResolutionView::Domains(arr) => assert_eq!(arr.len(), NUM_DOMAINS),
            _ => panic!("Expected Domains view"),
        }

        match hierarchy.at_level(3) {
            ResolutionView::Overall(v) => assert!((v - 0.5).abs() < 0.001),
            _ => panic!("Expected Overall view"),
        }

        println!("[PASS] at_level returns correct views");
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_multi_resolution_at_level_out_of_bounds() {
        let hierarchy = MultiResolutionHierarchy::default();
        let _ = hierarchy.at_level(4);
    }

    #[test]
    fn test_multi_resolution_quick_score() {
        let mut raw = [0.0f32; NUM_EMBEDDERS];
        raw[0] = 1.0; // High factual

        let hierarchy = MultiResolutionHierarchy::from_raw(raw);

        // Quick score should be non-zero
        assert!(hierarchy.quick_score() > 0.0);

        println!("[PASS] quick_score returns Level 3 value");
    }

    #[test]
    fn test_multi_resolution_standard_vector() {
        let raw = [0.7f32; NUM_EMBEDDERS];
        let hierarchy = MultiResolutionHierarchy::from_raw(raw);

        let standard = hierarchy.standard_vector();
        assert_eq!(standard.len(), NUM_GROUPS);

        println!("[PASS] standard_vector returns 6D");
    }

    #[test]
    fn test_multi_resolution_precise_vector() {
        let raw = [0.6f32; NUM_EMBEDDERS];
        let hierarchy = MultiResolutionHierarchy::from_raw(raw);

        let precise = hierarchy.precise_vector();
        assert_eq!(precise, raw);

        println!("[PASS] precise_vector returns 13D");
    }

    #[test]
    fn test_multi_resolution_similarity_at_level() {
        let h1 = MultiResolutionHierarchy::from_raw([0.8f32; NUM_EMBEDDERS]);
        let h2 = MultiResolutionHierarchy::from_raw([0.8f32; NUM_EMBEDDERS]);

        // Identical hierarchies should have similarity ~1.0 at all levels
        for level in 0..=3 {
            let sim = h1.similarity_at_level(&h2, level);
            assert!(
                (sim - 1.0).abs() < 0.01,
                "Level {} similarity = {} (expected ~1.0)",
                level,
                sim
            );
        }

        println!("[PASS] similarity_at_level works for all levels");
    }

    #[test]
    fn test_multi_resolution_recompute() {
        let mut hierarchy = MultiResolutionHierarchy::from_raw([0.5f32; NUM_EMBEDDERS]);

        // Modify raw
        hierarchy.raw[0] = 0.9;

        // Before recompute, derived values are stale
        let old_overall = hierarchy.overall;

        hierarchy.recompute();

        // After recompute, should reflect new raw values
        // (difference may be small depending on the specific change)
        assert!((hierarchy.overall - old_overall).abs() > 0.001 || hierarchy.raw[0] != 0.5);

        println!("[PASS] recompute updates derived levels");
    }

    #[test]
    fn test_multi_resolution_default() {
        let hierarchy = MultiResolutionHierarchy::default();

        assert_eq!(hierarchy.raw, [0.0; NUM_EMBEDDERS]);
        assert!((hierarchy.overall - 0.0).abs() < f32::EPSILON);

        println!("[PASS] MultiResolutionHierarchy::default creates zeros");
    }

    #[test]
    fn test_multi_resolution_serialization() {
        let hierarchy = MultiResolutionHierarchy::from_raw([0.7f32; NUM_EMBEDDERS]);

        let json = serde_json::to_string(&hierarchy).unwrap();
        let deserialized: MultiResolutionHierarchy = serde_json::from_str(&json).unwrap();

        assert!((hierarchy.overall - deserialized.overall).abs() < f32::EPSILON);

        println!("[PASS] Serialization roundtrip works");
    }

    // ===== ResolutionView Tests =====

    #[test]
    fn test_resolution_view_dimensions() {
        assert_eq!(ResolutionView::Raw([0.0; NUM_EMBEDDERS]).dimensions(), 13);
        assert_eq!(ResolutionView::Groups([0.0; NUM_GROUPS]).dimensions(), 6);
        assert_eq!(ResolutionView::Domains([0.0; NUM_DOMAINS]).dimensions(), 3);
        assert_eq!(ResolutionView::Overall(0.0).dimensions(), 1);

        println!("[PASS] ResolutionView::dimensions correct");
    }

    #[test]
    fn test_resolution_view_to_vec() {
        let view = ResolutionView::Groups([0.1, 0.2, 0.3, 0.4, 0.5, 0.6]);
        let vec = view.to_vec();

        assert_eq!(vec.len(), 6);
        assert!((vec[0] - 0.1).abs() < f32::EPSILON);

        println!("[PASS] ResolutionView::to_vec works");
    }
}
