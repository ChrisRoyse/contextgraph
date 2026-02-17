//! Core types for graph relationship discovery.
//!
//! # Graph Analysis Types
//!
//! This module defines the core types used in the graph relationship
//! discovery pipeline:
//!
//! - [`GraphAnalysisResult`]: LLM output for relationship analysis
//! - [`GraphLinkDirection`]: Direction of the graph connection
//! - [`RelationshipType`]: The 20 supported relationship types (19 + None)
//! - [`RelationshipCategory`]: The 6 high-level relationship categories
//! - [`ContentDomain`]: The 4 content domains (Code, Legal, Academic, General)
//! - [`GraphCandidate`]: Candidate pair for LLM analysis
//! - [`MemoryForGraphAnalysis`]: Memory with graph-relevant metadata
//! - [`DomainMarkers`]: Heuristic markers for domain detection

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// CONTENT DOMAIN
// ============================================================================

/// Content domain for relationship analysis.
///
/// Determines which relationship types are most relevant and
/// informs the LLM prompt context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ContentDomain {
    /// Programming code, APIs, software documentation.
    Code,

    /// Legal documents: cases, statutes, contracts, regulations.
    Legal,

    /// Academic content: research papers, studies, citations.
    Academic,

    /// General content that doesn't fit other categories.
    #[default]
    General,
}

impl ContentDomain {
    /// Parse domain from string (case-insensitive).
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().trim() {
            "code" => Self::Code,
            "legal" => Self::Legal,
            "academic" => Self::Academic,
            _ => Self::General,
        }
    }

    /// Get the domain as a string for LLM output.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Code => "code",
            Self::Legal => "legal",
            Self::Academic => "academic",
            Self::General => "general",
        }
    }

    /// All domain values.
    pub fn all() -> &'static [Self] {
        &[Self::Code, Self::Legal, Self::Academic, Self::General]
    }
}

// ============================================================================
// RELATIONSHIP CATEGORY
// ============================================================================

/// High-level relationship categories (6 categories).
///
/// Groups specific relationship types into broader conceptual categories
/// for easier reasoning and filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipCategory {
    /// A contains B (hierarchy, structural nesting).
    /// Types: Contains, ScopedBy
    Containment,

    /// A depends on B (requires, needs).
    /// Types: DependsOn, Imports, Requires
    Dependency,

    /// A references B (mentions, cites).
    /// Types: References, Cites, Interprets, Distinguishes
    Reference,

    /// A implements B (realizes, fulfills).
    /// Types: Implements, CompliesWith, Fulfills
    Implementation,

    /// A extends B (builds upon, modifies).
    /// Types: Extends, Modifies, Supersedes, Overrules
    Extension,

    /// A applies/uses B (active usage).
    /// Types: Calls, Applies, UsedBy
    #[default]
    Invocation,
}

impl RelationshipCategory {
    /// Parse category from string (case-insensitive).
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().trim() {
            "containment" => Self::Containment,
            "dependency" => Self::Dependency,
            "reference" => Self::Reference,
            "implementation" => Self::Implementation,
            "extension" => Self::Extension,
            _ => Self::Invocation,
        }
    }

    /// Get the category as a string for LLM output.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Containment => "containment",
            Self::Dependency => "dependency",
            Self::Reference => "reference",
            Self::Implementation => "implementation",
            Self::Extension => "extension",
            Self::Invocation => "invocation",
        }
    }

    /// All category values.
    pub fn all() -> &'static [Self] {
        &[
            Self::Containment,
            Self::Dependency,
            Self::Reference,
            Self::Implementation,
            Self::Extension,
            Self::Invocation,
        ]
    }
}

// ============================================================================
// GRAPH ANALYSIS RESULT
// ============================================================================

/// Result of LLM-based graph relationship analysis.
///
/// Contains the analysis of whether two memories have a structural
/// relationship and what type of relationship it is.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphAnalysisResult {
    /// Whether a graph connection exists between the two memories.
    pub has_connection: bool,

    /// Direction of the graph connection.
    pub direction: GraphLinkDirection,

    /// Type of relationship detected.
    pub relationship_type: RelationshipType,

    /// High-level category of the relationship.
    pub category: RelationshipCategory,

    /// Detected content domain.
    pub domain: ContentDomain,

    /// Confidence score [0.0, 1.0].
    pub confidence: f32,

    /// Human-readable description of the relationship.
    pub description: String,

    /// Raw LLM response (for debugging).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_response: Option<String>,

    /// LLM provenance metadata (Phase 1.3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_provenance: Option<context_graph_core::types::LLMProvenance>,
}

impl Default for GraphAnalysisResult {
    fn default() -> Self {
        Self {
            has_connection: false,
            direction: GraphLinkDirection::NoConnection,
            relationship_type: RelationshipType::None,
            category: RelationshipCategory::Invocation,
            domain: ContentDomain::General,
            confidence: 0.0,
            description: String::new(),
            raw_response: None,
            llm_provenance: None,
        }
    }
}

/// Direction of graph relationship between two entities.
///
/// Following the E8 asymmetric pattern (ARCH-15):
/// - Source: Entity that points to another (outgoing relationship)
/// - Target: Entity that is pointed to (incoming relationship)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum GraphLinkDirection {
    /// A connects to B (A is source, B is target).
    /// Example: "Module A imports Module B"
    AConnectsB,

    /// B connects to A (B is source, A is target).
    /// Example: "Module B imports Module A"
    BConnectsA,

    /// Bidirectional connection.
    /// Example: "Module A and Module B reference each other"
    Bidirectional,

    /// No connection detected.
    #[default]
    NoConnection,
}

impl GraphLinkDirection {
    /// Parse direction from string (case-insensitive).
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        let lower = s.to_lowercase();
        let trimmed = lower.trim();

        match trimmed {
            "a_connects_b" | "a connects b" | "a->b" | "a_to_b" | "forward" | "source" => {
                Self::AConnectsB
            }
            "b_connects_a" | "b connects a" | "b->a" | "b_to_a" | "reverse" | "target" => {
                Self::BConnectsA
            }
            "bidirectional" | "both" | "mutual" | "symmetric" | "a<->b" => Self::Bidirectional,
            _ => Self::NoConnection,
        }
    }

    /// Check if this direction indicates a connection exists.
    pub fn is_connected(&self) -> bool {
        !matches!(self, Self::NoConnection)
    }

    /// Get the direction as a string for LLM output.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AConnectsB => "a_connects_b",
            Self::BConnectsA => "b_connects_a",
            Self::Bidirectional => "bidirectional",
            Self::NoConnection => "none",
        }
    }
}

// ============================================================================
// RELATIONSHIP TYPE (20 types: 19 + None)
// ============================================================================

/// The 20 supported relationship types for graph connections.
///
/// Expanded from the original 8 code-specific types to support:
/// - Code: imports, calls, implements, extends, contains, depends_on
/// - Legal: cites, interprets, overrules, supersedes, distinguishes, complies_with
/// - Academic: cites, applies, extends, references
/// - General: references, contains, depends_on, extends
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipType {
    // ========== CONTAINMENT CATEGORY ==========
    /// A contains B structurally.
    /// Example: "module contains function", "document contains section"
    Contains,

    /// B is within A's scope.
    /// Example: "variable scoped by function"
    ScopedBy,

    // ========== DEPENDENCY CATEGORY ==========
    /// A requires B to function.
    /// Example: "requires PostgreSQL"
    DependsOn,

    /// A explicitly imports B (code).
    /// Example: "use crate::module;"
    Imports,

    /// B is a prerequisite for A.
    /// Example: "requires completion of course X"
    Requires,

    // ========== REFERENCE CATEGORY ==========
    /// A mentions B.
    /// Example: "See also: [link]"
    References,

    /// A formally cites B (legal/academic).
    /// Example: "Brown v. Board of Education, 347 U.S. 483"
    Cites,

    /// A explains/construes B (legal).
    /// Example: "Court interprets § 1983 to require..."
    Interprets,

    /// A differentiates from B (legal).
    /// Example: "This case is distinguishable from Smith v. Jones"
    Distinguishes,

    // ========== IMPLEMENTATION CATEGORY ==========
    /// A implements B's interface.
    /// Example: "impl Trait for Struct"
    Implements,

    /// A meets B's requirements.
    /// Example: "complies with HIPAA regulations"
    CompliesWith,

    /// A satisfies B's conditions.
    /// Example: "fulfills contractual obligations"
    Fulfills,

    // ========== EXTENSION CATEGORY ==========
    /// A builds upon B.
    /// Example: "extends BaseClass"
    Extends,

    /// A changes B.
    /// Example: "amendment modifies section 3"
    Modifies,

    /// A replaces B (newer version).
    /// Example: "supersedes prior regulation"
    Supersedes,

    /// A invalidates B (legal).
    /// Example: "overruled by subsequent decision"
    Overrules,

    // ========== INVOCATION CATEGORY ==========
    /// A invokes B directly (code).
    /// Example: "function A calls function B"
    Calls,

    /// A uses B as method/principle.
    /// Example: "applies the doctrine of res judicata"
    Applies,

    /// B consumes A (inverse).
    /// Example: "used by client code"
    UsedBy,

    /// No relationship detected.
    #[default]
    None,
}

impl RelationshipType {
    /// Parse relationship type from string (case-insensitive).
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        let lower = s.to_lowercase();
        let trimmed = lower.trim();

        match trimmed {
            // Containment
            "contains" | "contain" | "has" | "includes" => Self::Contains,
            "scoped_by" | "scoped by" | "within" => Self::ScopedBy,

            // Dependency
            "depends_on" | "depends on" | "dependency" => Self::DependsOn,
            "imports" | "import" | "uses" | "use" => Self::Imports,
            "requires" | "require" | "prerequisite" => Self::Requires,

            // Reference
            "references" | "reference" | "refs" | "see" | "links" | "links_to" => Self::References,
            "cites" | "cite" | "citation" => Self::Cites,
            "interprets" | "interpret" | "construes" => Self::Interprets,
            "distinguishes" | "distinguish" | "differentiates" => Self::Distinguishes,

            // Implementation
            "implements" | "implement" | "impl" => Self::Implements,
            "complies_with" | "complies with" | "compliant" => Self::CompliesWith,
            "fulfills" | "fulfill" | "satisfies" => Self::Fulfills,

            // Extension
            "extends" | "extend" | "inherits" | "inherit" => Self::Extends,
            "modifies" | "modify" | "amends" | "amend" => Self::Modifies,
            "supersedes" | "supersede" | "replaces" => Self::Supersedes,
            "overrules" | "overrule" | "overruled" | "invalidates" => Self::Overrules,

            // Invocation
            "calls" | "call" | "invokes" | "invoke" => Self::Calls,
            "applies" | "apply" | "uses_as" => Self::Applies,
            "used_by" | "used by" | "consumed_by" | "consumed by" => Self::UsedBy,

            _ => Self::None,
        }
    }

    /// Get the relationship type as a string for LLM output.
    pub fn as_str(&self) -> &'static str {
        match self {
            // Containment
            Self::Contains => "contains",
            Self::ScopedBy => "scoped_by",

            // Dependency
            Self::DependsOn => "depends_on",
            Self::Imports => "imports",
            Self::Requires => "requires",

            // Reference
            Self::References => "references",
            Self::Cites => "cites",
            Self::Interprets => "interprets",
            Self::Distinguishes => "distinguishes",

            // Implementation
            Self::Implements => "implements",
            Self::CompliesWith => "complies_with",
            Self::Fulfills => "fulfills",

            // Extension
            Self::Extends => "extends",
            Self::Modifies => "modifies",
            Self::Supersedes => "supersedes",
            Self::Overrules => "overrules",

            // Invocation
            Self::Calls => "calls",
            Self::Applies => "applies",
            Self::UsedBy => "used_by",

            Self::None => "none",
        }
    }

    /// All valid relationship types (excluding None).
    pub fn all() -> &'static [Self] {
        &[
            // Containment
            Self::Contains,
            Self::ScopedBy,
            // Dependency
            Self::DependsOn,
            Self::Imports,
            Self::Requires,
            // Reference
            Self::References,
            Self::Cites,
            Self::Interprets,
            Self::Distinguishes,
            // Implementation
            Self::Implements,
            Self::CompliesWith,
            Self::Fulfills,
            // Extension
            Self::Extends,
            Self::Modifies,
            Self::Supersedes,
            Self::Overrules,
            // Invocation
            Self::Calls,
            Self::Applies,
            Self::UsedBy,
        ]
    }

    /// Get the category this relationship type belongs to.
    pub fn category(&self) -> RelationshipCategory {
        match self {
            Self::Contains | Self::ScopedBy => RelationshipCategory::Containment,
            Self::DependsOn | Self::Imports | Self::Requires => RelationshipCategory::Dependency,
            Self::References | Self::Cites | Self::Interprets | Self::Distinguishes => {
                RelationshipCategory::Reference
            }
            Self::Implements | Self::CompliesWith | Self::Fulfills => {
                RelationshipCategory::Implementation
            }
            Self::Extends | Self::Modifies | Self::Supersedes | Self::Overrules => {
                RelationshipCategory::Extension
            }
            Self::Calls | Self::Applies | Self::UsedBy | Self::None => {
                RelationshipCategory::Invocation
            }
        }
    }

    /// Check if this relationship type is relevant for a domain.
    pub fn is_relevant_for_domain(&self, domain: ContentDomain) -> bool {
        match domain {
            ContentDomain::Code => matches!(
                self,
                Self::Imports
                    | Self::Calls
                    | Self::Implements
                    | Self::Extends
                    | Self::Contains
                    | Self::DependsOn
                    | Self::UsedBy
                    | Self::Requires
                    | Self::ScopedBy
                    | Self::References
            ),
            ContentDomain::Legal => matches!(
                self,
                Self::Cites
                    | Self::Interprets
                    | Self::Overrules
                    | Self::Supersedes
                    | Self::Distinguishes
                    | Self::CompliesWith
                    | Self::Applies
                    | Self::References
                    | Self::Contains
                    | Self::Modifies
            ),
            ContentDomain::Academic => matches!(
                self,
                Self::Cites
                    | Self::Applies
                    | Self::Extends
                    | Self::References
                    | Self::Contains
                    | Self::DependsOn
                    | Self::Supersedes
            ),
            ContentDomain::General => true, // All types can apply to general content
        }
    }

    /// Convert from legacy 8-type system.
    ///
    /// Maintains backward compatibility with existing code that uses
    /// the original 8 relationship types.
    pub fn from_legacy(legacy: &str) -> Self {
        match legacy.to_lowercase().as_str() {
            "imports" => Self::Imports,
            "depends_on" => Self::DependsOn,
            "references" => Self::References,
            "calls" => Self::Calls,
            "implements" => Self::Implements,
            "extends" => Self::Extends,
            "contains" => Self::Contains,
            "used_by" => Self::UsedBy,
            "none" => Self::None,
            _ => Self::None,
        }
    }

    /// Check if this is a legacy (original 8) type.
    pub fn is_legacy_type(&self) -> bool {
        matches!(
            self,
            Self::Imports
                | Self::DependsOn
                | Self::References
                | Self::Calls
                | Self::Implements
                | Self::Extends
                | Self::Contains
                | Self::UsedBy
                | Self::None
        )
    }

    /// Types specific to legal domain.
    pub fn legal_types() -> &'static [Self] {
        &[
            Self::Cites,
            Self::Interprets,
            Self::Overrules,
            Self::Supersedes,
            Self::Distinguishes,
            Self::CompliesWith,
        ]
    }

    /// Types specific to code domain.
    pub fn code_types() -> &'static [Self] {
        &[Self::Imports, Self::Calls, Self::Implements, Self::ScopedBy]
    }
}

/// Candidate pair for graph relationship analysis.
///
/// Represents two memories that may have a structural relationship,
/// identified by the scanner and awaiting LLM analysis.
#[derive(Debug, Clone)]
pub struct GraphCandidate {
    /// First memory ID.
    pub memory_a_id: Uuid,

    /// First memory content.
    pub memory_a_content: String,

    /// Second memory ID.
    pub memory_b_id: Uuid,

    /// Second memory content.
    pub memory_b_content: String,

    /// Initial heuristic score from scanner [0.0, 1.0].
    pub initial_score: f32,

    /// Timestamp of memory A.
    pub memory_a_timestamp: DateTime<Utc>,

    /// Timestamp of memory B.
    pub memory_b_timestamp: DateTime<Utc>,

    /// Suspected relationship types based on heuristics.
    pub suspected_types: Vec<RelationshipType>,
}

impl GraphCandidate {
    /// Create a new graph candidate.
    pub fn new(
        memory_a_id: Uuid,
        memory_a_content: String,
        memory_b_id: Uuid,
        memory_b_content: String,
    ) -> Self {
        Self {
            memory_a_id,
            memory_a_content,
            memory_b_id,
            memory_b_content,
            initial_score: 0.0,
            memory_a_timestamp: Utc::now(),
            memory_b_timestamp: Utc::now(),
            suspected_types: Vec::new(),
        }
    }
}

/// Memory with metadata for graph analysis.
#[derive(Debug, Clone)]
pub struct MemoryForGraphAnalysis {
    /// Memory UUID.
    pub id: Uuid,

    /// Memory content text.
    pub content: String,

    /// Creation timestamp.
    pub created_at: DateTime<Utc>,

    /// Session ID if available.
    pub session_id: Option<String>,

    /// E1 semantic embedding for similarity clustering.
    pub e1_embedding: Vec<f32>,

    /// Source type (e.g., "HookDescription", "CodeEntity").
    pub source_type: Option<String>,

    /// File path if this memory is from code.
    pub file_path: Option<String>,
}

// ============================================================================
// DOMAIN MARKERS (Multi-domain detection)
// ============================================================================

/// Heuristic markers for detecting content domains.
///
/// Supports four domains: Code, Legal, Academic, General.
pub struct DomainMarkers;

impl DomainMarkers {
    // ========== CODE DOMAIN MARKERS ==========
    /// Markers that indicate programming code.
    pub const CODE: &'static [&'static str] = &[
        "fn ", "pub ", "let ", "const ", "impl ", "struct ", "enum ", "trait ",
        "use ", "mod ", "crate::", "import ", "export ", "class ", "def ",
        "function ", "return ", "()", "{}", "->", "=>", "::", "//", "/*",
        "async ", "await ", "match ", "if let ", "#[", "self.", "super::",
    ];

    // ========== LEGAL DOMAIN MARKERS ==========
    /// Markers that indicate legal documents.
    pub const LEGAL: &'static [&'static str] = &[
        "§", "U.S.C.", "F.3d", "S.Ct.", "U.S. ", " v. ", "plaintiff", "defendant",
        "court", "statute", "regulation", "precedent", "holding", "overruled",
        "affirmed", "reversed", "remanded", "jurisdiction", "pursuant to",
        "hereby", "thereof", "whereas", "id.", "supra", "infra", "stare decisis",
        "certiorari", "appellant", "appellee", "judgment", "order", "ruling",
        "brief", "motion", "petition", "enjoin", "injunction", "habeas corpus",
    ];

    // ========== ACADEMIC DOMAIN MARKERS ==========
    /// Markers that indicate academic/research content.
    pub const ACADEMIC: &'static [&'static str] = &[
        "et al.", "p < ", "n = ", "methodology", "hypothesis", "findings",
        "abstract", "introduction", "conclusion", "references", "doi:",
        "journal", "published", "peer-reviewed", "study", "research",
        "participants", "sample size", "statistical", "significance",
        "correlation", "regression", "experiment", "control group",
        "systematic review", "meta-analysis", "literature review",
    ];

    /// Detect content domain from text using heuristic markers.
    ///
    /// Returns the most likely domain based on marker counts.
    /// Requires at least 2 markers to classify as a specific domain,
    /// otherwise returns General.
    pub fn detect_domain(content: &str) -> ContentDomain {
        let lower = content.to_lowercase();

        // Count markers for each domain
        let code_score = Self::CODE
            .iter()
            .filter(|m| lower.contains(&m.to_lowercase()))
            .count();
        let legal_score = Self::LEGAL
            .iter()
            .filter(|m| lower.contains(&m.to_lowercase()))
            .count();
        let academic_score = Self::ACADEMIC
            .iter()
            .filter(|m| lower.contains(&m.to_lowercase()))
            .count();

        // Require minimum threshold and pick highest score.
        //
        // LOW-5 Note: Code requires strictly beating BOTH Legal and Academic scores,
        // while Legal/Academic only need to beat Code. This is intentional: Code is
        // the default fallback domain, so it requires stronger evidence to classify
        // content as Code (preventing over-classification of general technical text).
        // Legal and Academic domains are more specialized and less likely to appear
        // incidentally, so a lower bar is appropriate.
        let threshold = 2;

        if code_score >= threshold && code_score > legal_score && code_score > academic_score {
            ContentDomain::Code
        } else if legal_score >= threshold && legal_score > code_score {
            ContentDomain::Legal
        } else if academic_score >= threshold && academic_score > code_score {
            ContentDomain::Academic
        } else {
            ContentDomain::General
        }
    }

    /// Detect domain from two content pieces (for relationship analysis).
    ///
    /// Returns the dominant domain between the two pieces,
    /// or General if they differ significantly.
    pub fn detect_domain_pair(content_a: &str, content_b: &str) -> ContentDomain {
        let domain_a = Self::detect_domain(content_a);
        let domain_b = Self::detect_domain(content_b);

        // If both are the same non-General domain, use that
        if domain_a == domain_b {
            return domain_a;
        }

        // If one is General and the other is specific, use the specific one
        if domain_a == ContentDomain::General {
            return domain_b;
        }
        if domain_b == ContentDomain::General {
            return domain_a;
        }

        // If they differ (e.g., Code vs Legal), default to General
        ContentDomain::General
    }

    /// Check if content looks like code.
    ///
    /// Backward-compatible helper that returns true if content
    /// contains at least one code marker. Uses a lower threshold
    /// than `detect_domain()` for broader code detection.
    pub fn looks_like_code(content: &str) -> bool {
        let lower = content.to_lowercase();
        Self::CODE
            .iter()
            .any(|m| lower.contains(&m.to_lowercase()))
    }
}

// ============================================================================
// GRAPH MARKERS (Relationship heuristics)
// ============================================================================

/// Markers for detecting graph relationships heuristically.
///
/// Updated to support all 20 relationship types across domains.
pub struct GraphMarkers;

impl GraphMarkers {
    // ========== CONTAINMENT ==========
    pub const CONTAIN_MARKERS: &'static [&'static str] =
        &["contains", "includes", "has", "module", "struct", "enum", "pub mod", "section", "chapter"];
    pub const SCOPED_MARKERS: &'static [&'static str] =
        &["scoped by", "within", "inside", "under", "in scope"];

    // ========== DEPENDENCY ==========
    pub const IMPORT_MARKERS: &'static [&'static str] = &[
        "import", "use", "require", "include", "from", "extern crate", "mod",
    ];
    pub const DEPENDENCY_MARKERS: &'static [&'static str] =
        &["depends on", "requires", "needs", "dependency", "prerequisite"];
    pub const REQUIRES_MARKERS: &'static [&'static str] =
        &["requires", "prerequisite", "must have", "needed"];

    // ========== REFERENCE ==========
    pub const REFERENCE_MARKERS: &'static [&'static str] = &[
        "see:", "ref:", "see also", "links to", "references", "related to",
        "http://", "https://",
    ];
    pub const CITE_MARKERS: &'static [&'static str] = &[
        "cites", "citing", "citation", "cited in", " v. ", "U.S.", "F.3d", "S.Ct.",
        "et al.", "supra", "infra", "id.",
    ];
    pub const INTERPRET_MARKERS: &'static [&'static str] =
        &["interprets", "construes", "meaning of", "interpretation"];
    pub const DISTINGUISH_MARKERS: &'static [&'static str] =
        &["distinguishes", "distinguishable", "differs from", "unlike"];

    // ========== IMPLEMENTATION ==========
    pub const IMPL_MARKERS: &'static [&'static str] =
        &["impl", "implements", "implementation", "trait"];
    pub const COMPLY_MARKERS: &'static [&'static str] =
        &["complies with", "compliant", "in compliance", "meets requirements"];
    pub const FULFILL_MARKERS: &'static [&'static str] =
        &["fulfills", "satisfies", "meets", "achieves"];

    // ========== EXTENSION ==========
    pub const EXTEND_MARKERS: &'static [&'static str] =
        &["extends", "inherits", "derives", "subclass", "child of", "builds upon"];
    pub const MODIFY_MARKERS: &'static [&'static str] =
        &["modifies", "amends", "changes", "alters", "updates"];
    pub const SUPERSEDE_MARKERS: &'static [&'static str] =
        &["supersedes", "replaces", "succeeds", "newer version"];
    pub const OVERRULE_MARKERS: &'static [&'static str] =
        &["overrules", "overruled", "invalidates", "vacates", "reverses"];

    // ========== INVOCATION ==========
    pub const CALL_MARKERS: &'static [&'static str] =
        &["calls", "invokes", "executes", "runs", "triggers", "fn ", "()"];
    pub const APPLY_MARKERS: &'static [&'static str] =
        &["applies", "applying", "application of", "uses method"];
    pub const USED_BY_MARKERS: &'static [&'static str] =
        &["used by", "consumed by", "client of", "caller"];

    /// Count markers in content for a specific relationship type.
    pub fn count_markers_for_type(content: &str, rel_type: RelationshipType) -> usize {
        let markers: &[&str] = match rel_type {
            // Containment
            RelationshipType::Contains => Self::CONTAIN_MARKERS,
            RelationshipType::ScopedBy => Self::SCOPED_MARKERS,

            // Dependency
            RelationshipType::Imports => Self::IMPORT_MARKERS,
            RelationshipType::DependsOn => Self::DEPENDENCY_MARKERS,
            RelationshipType::Requires => Self::REQUIRES_MARKERS,

            // Reference
            RelationshipType::References => Self::REFERENCE_MARKERS,
            RelationshipType::Cites => Self::CITE_MARKERS,
            RelationshipType::Interprets => Self::INTERPRET_MARKERS,
            RelationshipType::Distinguishes => Self::DISTINGUISH_MARKERS,

            // Implementation
            RelationshipType::Implements => Self::IMPL_MARKERS,
            RelationshipType::CompliesWith => Self::COMPLY_MARKERS,
            RelationshipType::Fulfills => Self::FULFILL_MARKERS,

            // Extension
            RelationshipType::Extends => Self::EXTEND_MARKERS,
            RelationshipType::Modifies => Self::MODIFY_MARKERS,
            RelationshipType::Supersedes => Self::SUPERSEDE_MARKERS,
            RelationshipType::Overrules => Self::OVERRULE_MARKERS,

            // Invocation
            RelationshipType::Calls => Self::CALL_MARKERS,
            RelationshipType::Applies => Self::APPLY_MARKERS,
            RelationshipType::UsedBy => Self::USED_BY_MARKERS,

            RelationshipType::None => return 0,
        };

        let lower = content.to_lowercase();
        markers.iter().filter(|m| lower.contains(*m)).count()
    }

    /// Get suspected relationship types based on content markers.
    pub fn detect_suspected_types(content: &str) -> Vec<RelationshipType> {
        let mut types = Vec::new();

        for rel_type in RelationshipType::all() {
            if Self::count_markers_for_type(content, *rel_type) > 0 {
                types.push(*rel_type);
            }
        }

        types
    }

    /// Count all graph-related markers in content.
    pub fn count_all_markers(content: &str) -> usize {
        RelationshipType::all()
            .iter()
            .map(|t| Self::count_markers_for_type(content, *t))
            .sum()
    }

    /// Get suspected types filtered by domain relevance.
    pub fn detect_suspected_types_for_domain(
        content: &str,
        domain: ContentDomain,
    ) -> Vec<RelationshipType> {
        Self::detect_suspected_types(content)
            .into_iter()
            .filter(|t| t.is_relevant_for_domain(domain))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== DIRECTION TESTS ==========

    #[test]
    fn test_direction_from_str() {
        assert_eq!(
            GraphLinkDirection::from_str("a_connects_b"),
            GraphLinkDirection::AConnectsB
        );
        assert_eq!(
            GraphLinkDirection::from_str("B->A"),
            GraphLinkDirection::BConnectsA
        );
        assert_eq!(
            GraphLinkDirection::from_str("bidirectional"),
            GraphLinkDirection::Bidirectional
        );
        assert_eq!(
            GraphLinkDirection::from_str("invalid"),
            GraphLinkDirection::NoConnection
        );
    }

    // ========== CONTENT DOMAIN TESTS ==========

    #[test]
    fn test_content_domain_from_str() {
        assert_eq!(ContentDomain::from_str("code"), ContentDomain::Code);
        assert_eq!(ContentDomain::from_str("LEGAL"), ContentDomain::Legal);
        assert_eq!(ContentDomain::from_str("academic"), ContentDomain::Academic);
        assert_eq!(ContentDomain::from_str("general"), ContentDomain::General);
        assert_eq!(ContentDomain::from_str("unknown"), ContentDomain::General);
    }

    #[test]
    fn test_content_domain_as_str() {
        assert_eq!(ContentDomain::Code.as_str(), "code");
        assert_eq!(ContentDomain::Legal.as_str(), "legal");
        assert_eq!(ContentDomain::Academic.as_str(), "academic");
        assert_eq!(ContentDomain::General.as_str(), "general");
    }

    // ========== RELATIONSHIP CATEGORY TESTS ==========

    #[test]
    fn test_relationship_category_from_str() {
        assert_eq!(
            RelationshipCategory::from_str("containment"),
            RelationshipCategory::Containment
        );
        assert_eq!(
            RelationshipCategory::from_str("DEPENDENCY"),
            RelationshipCategory::Dependency
        );
        assert_eq!(
            RelationshipCategory::from_str("reference"),
            RelationshipCategory::Reference
        );
        assert_eq!(
            RelationshipCategory::from_str("implementation"),
            RelationshipCategory::Implementation
        );
        assert_eq!(
            RelationshipCategory::from_str("extension"),
            RelationshipCategory::Extension
        );
        assert_eq!(
            RelationshipCategory::from_str("invocation"),
            RelationshipCategory::Invocation
        );
    }

    // ========== RELATIONSHIP TYPE TESTS ==========

    #[test]
    fn test_relationship_type_from_str() {
        // Original types
        assert_eq!(RelationshipType::from_str("imports"), RelationshipType::Imports);
        assert_eq!(RelationshipType::from_str("DEPENDS_ON"), RelationshipType::DependsOn);
        assert_eq!(RelationshipType::from_str("calls"), RelationshipType::Calls);
        assert_eq!(RelationshipType::from_str("invalid"), RelationshipType::None);

        // New legal types
        assert_eq!(RelationshipType::from_str("cites"), RelationshipType::Cites);
        assert_eq!(RelationshipType::from_str("interprets"), RelationshipType::Interprets);
        assert_eq!(RelationshipType::from_str("overrules"), RelationshipType::Overrules);
        assert_eq!(RelationshipType::from_str("supersedes"), RelationshipType::Supersedes);
        assert_eq!(RelationshipType::from_str("distinguishes"), RelationshipType::Distinguishes);

        // New implementation types
        assert_eq!(RelationshipType::from_str("complies_with"), RelationshipType::CompliesWith);
        assert_eq!(RelationshipType::from_str("fulfills"), RelationshipType::Fulfills);

        // New types
        assert_eq!(RelationshipType::from_str("scoped_by"), RelationshipType::ScopedBy);
        assert_eq!(RelationshipType::from_str("modifies"), RelationshipType::Modifies);
        assert_eq!(RelationshipType::from_str("applies"), RelationshipType::Applies);
    }

    #[test]
    fn test_relationship_type_all_has_19_types() {
        // 19 valid types (excluding None)
        assert_eq!(RelationshipType::all().len(), 19);
    }

    #[test]
    fn test_relationship_type_category() {
        assert_eq!(
            RelationshipType::Contains.category(),
            RelationshipCategory::Containment
        );
        assert_eq!(
            RelationshipType::Imports.category(),
            RelationshipCategory::Dependency
        );
        assert_eq!(
            RelationshipType::Cites.category(),
            RelationshipCategory::Reference
        );
        assert_eq!(
            RelationshipType::Implements.category(),
            RelationshipCategory::Implementation
        );
        assert_eq!(
            RelationshipType::Extends.category(),
            RelationshipCategory::Extension
        );
        assert_eq!(
            RelationshipType::Calls.category(),
            RelationshipCategory::Invocation
        );
    }

    #[test]
    fn test_relationship_type_domain_relevance() {
        // Code-specific types
        assert!(RelationshipType::Imports.is_relevant_for_domain(ContentDomain::Code));
        assert!(!RelationshipType::Imports.is_relevant_for_domain(ContentDomain::Legal));

        // Legal-specific types
        assert!(RelationshipType::Overrules.is_relevant_for_domain(ContentDomain::Legal));
        assert!(!RelationshipType::Overrules.is_relevant_for_domain(ContentDomain::Code));

        // Cross-domain types
        assert!(RelationshipType::References.is_relevant_for_domain(ContentDomain::Code));
        assert!(RelationshipType::References.is_relevant_for_domain(ContentDomain::Legal));
        assert!(RelationshipType::References.is_relevant_for_domain(ContentDomain::Academic));

        // General accepts all
        assert!(RelationshipType::Imports.is_relevant_for_domain(ContentDomain::General));
        assert!(RelationshipType::Overrules.is_relevant_for_domain(ContentDomain::General));
    }

    #[test]
    fn test_relationship_type_legacy_compatibility() {
        // Legacy types should all parse correctly
        assert_eq!(RelationshipType::from_legacy("imports"), RelationshipType::Imports);
        assert_eq!(RelationshipType::from_legacy("depends_on"), RelationshipType::DependsOn);
        assert_eq!(RelationshipType::from_legacy("references"), RelationshipType::References);
        assert_eq!(RelationshipType::from_legacy("calls"), RelationshipType::Calls);
        assert_eq!(RelationshipType::from_legacy("implements"), RelationshipType::Implements);
        assert_eq!(RelationshipType::from_legacy("extends"), RelationshipType::Extends);
        assert_eq!(RelationshipType::from_legacy("contains"), RelationshipType::Contains);
        assert_eq!(RelationshipType::from_legacy("used_by"), RelationshipType::UsedBy);
        assert_eq!(RelationshipType::from_legacy("none"), RelationshipType::None);

        // All legacy types should be identified as legacy
        assert!(RelationshipType::Imports.is_legacy_type());
        assert!(RelationshipType::DependsOn.is_legacy_type());

        // New types should NOT be legacy
        assert!(!RelationshipType::Cites.is_legacy_type());
        assert!(!RelationshipType::Overrules.is_legacy_type());
    }

    #[test]
    fn test_legal_and_code_types() {
        assert_eq!(RelationshipType::legal_types().len(), 6);
        assert!(RelationshipType::legal_types().contains(&RelationshipType::Cites));
        assert!(RelationshipType::legal_types().contains(&RelationshipType::Overrules));

        assert_eq!(RelationshipType::code_types().len(), 4);
        assert!(RelationshipType::code_types().contains(&RelationshipType::Imports));
        assert!(RelationshipType::code_types().contains(&RelationshipType::Calls));
    }

    // ========== DOMAIN MARKERS TESTS ==========

    #[test]
    fn test_domain_detection_code() {
        let rust_code = "fn main() { let x = 5; impl Foo for Bar {} }";
        assert_eq!(DomainMarkers::detect_domain(rust_code), ContentDomain::Code);
    }

    #[test]
    fn test_domain_detection_legal() {
        let legal_text = "The court held that pursuant to 42 U.S.C. § 1983, the plaintiff...";
        assert_eq!(DomainMarkers::detect_domain(legal_text), ContentDomain::Legal);
    }

    #[test]
    fn test_domain_detection_academic() {
        let academic_text = "The study (et al., 2023) found statistical significance (p < 0.05) with n = 150 participants.";
        assert_eq!(DomainMarkers::detect_domain(academic_text), ContentDomain::Academic);
    }

    #[test]
    fn test_domain_detection_general() {
        let general_text = "This is a general document about various topics.";
        assert_eq!(DomainMarkers::detect_domain(general_text), ContentDomain::General);
    }

    #[test]
    fn test_domain_detection_pair() {
        let code = "fn main() { use crate::foo; }";
        let legal = "The court held pursuant to statute...";
        let general = "Some general text here.";

        // Same domain
        assert_eq!(
            DomainMarkers::detect_domain_pair(code, code),
            ContentDomain::Code
        );

        // One general, one specific
        assert_eq!(
            DomainMarkers::detect_domain_pair(code, general),
            ContentDomain::Code
        );

        // Different specific domains -> General
        assert_eq!(
            DomainMarkers::detect_domain_pair(code, legal),
            ContentDomain::General
        );
    }

    #[test]
    fn test_looks_like_code_backward_compat() {
        assert!(DomainMarkers::looks_like_code("fn foo() { use bar; }"));
        assert!(!DomainMarkers::looks_like_code("The court held that..."));
    }

    // ========== GRAPH MARKERS TESTS ==========

    #[test]
    fn test_marker_detection() {
        let content = "use crate::module; impl Trait for Struct";
        let types = GraphMarkers::detect_suspected_types(content);
        assert!(types.contains(&RelationshipType::Imports));
        assert!(types.contains(&RelationshipType::Implements));
    }

    #[test]
    fn test_marker_detection_legal() {
        let content = "This case cites Brown v. Board of Education and distinguishes Smith v. Jones";
        let types = GraphMarkers::detect_suspected_types(content);
        assert!(types.contains(&RelationshipType::Cites));
        assert!(types.contains(&RelationshipType::Distinguishes));
    }

    #[test]
    fn test_marker_detection_with_domain_filter() {
        let content = "use crate::foo; the court cites prior precedent";

        // Without filter - gets all
        let all_types = GraphMarkers::detect_suspected_types(content);
        assert!(all_types.contains(&RelationshipType::Imports));
        assert!(all_types.contains(&RelationshipType::Cites));

        // With Code filter - only code-relevant
        let code_types = GraphMarkers::detect_suspected_types_for_domain(content, ContentDomain::Code);
        assert!(code_types.contains(&RelationshipType::Imports));
        assert!(!code_types.contains(&RelationshipType::Cites)); // Cites not relevant for Code

        // With Legal filter - only legal-relevant
        let legal_types = GraphMarkers::detect_suspected_types_for_domain(content, ContentDomain::Legal);
        assert!(!legal_types.contains(&RelationshipType::Imports)); // Imports not relevant for Legal
        assert!(legal_types.contains(&RelationshipType::Cites));
    }
}
