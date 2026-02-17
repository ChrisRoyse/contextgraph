//! Graph relationship analysis prompts for Hermes 2 Pro.
//!
//! This module provides prompt templates for LLM-based graph relationship
//! detection, using the ChatML format for Hermes 2 Pro Mistral 7B.
//!
//! ## Key Features:
//!
//! 1. **Grammar-constrained output**: GBNF ensures 100% valid JSON
//! 2. **Hermes 2 Pro optimization**: Trained for function calling and structured output
//! 3. **Multi-domain support**: Code, Legal, Academic, General content
//! 4. **20 relationship types**: Expanded from 8 code-specific to domain-agnostic

use crate::types::{ContentDomain, DomainMarkers, RelationshipType};

/// Prompt builder for graph relationship analysis.
///
/// Generates prompts in ChatML format compatible with Hermes 2 Pro Mistral 7B.
pub struct GraphPromptBuilder {
    /// Maximum content length per memory (chars).
    max_content_length: usize,
}

impl Default for GraphPromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphPromptBuilder {
    /// Create a new prompt builder with default settings.
    pub fn new() -> Self {
        Self {
            max_content_length: 1500,
        }
    }

    /// Create with custom max content length.
    pub fn with_max_content_length(mut self, max_length: usize) -> Self {
        self.max_content_length = max_length;
        self
    }

    /// Build a prompt for analyzing graph relationship between two memories.
    ///
    /// # Arguments
    /// * `memory_a` - First memory content
    /// * `memory_b` - Second memory content
    ///
    /// # Returns
    /// ChatML-formatted prompt string
    pub fn build_analysis_prompt(&self, memory_a: &str, memory_b: &str) -> String {
        let truncated_a = self.truncate_content(memory_a);
        let truncated_b = self.truncate_content(memory_b);

        // Auto-detect domain from content
        let domain = DomainMarkers::detect_domain_pair(memory_a, memory_b);
        let domain_context = self.domain_hint(domain);

        format!(
            r#"<|im_start|>system
You are an expert at detecting structural relationships between content.

TASK: Analyze if Content A has a STRUCTURAL relationship with Content B.

CRITICAL RULES:
1. If contents are UNRELATED or only share topic similarity -> has_connection: false, relationship_type: "none"
2. Look for EXPLICIT structural links (imports, citations, inheritance), not semantic similarity
3. Confidence should reflect certainty: 0.5=uncertain, 0.7=likely, 0.9=very certain
4. When in doubt, output "none" - don't force a relationship

RELATIONSHIP TYPES (choose the MOST SPECIFIC that applies):

CODE DOMAIN:
- imports: A explicitly imports/uses B (e.g., "use crate::B", "from B import")
- implements: A implements B's interface/trait (e.g., "impl Trait for Struct")
- extends: A inherits from B (e.g., "class A(B)", "extends B")
- calls: A invokes B's function (e.g., "B.method()", "call_b()")
- contains: A is a parent module containing B (e.g., "mod B" inside A)
- depends_on: A requires B as dependency (e.g., Cargo.toml dependency)

LEGAL DOMAIN:
- cites: A formally cites B with case citation (e.g., "Brown v. Board, 347 U.S. 483")
- interprets: A explains/construes B's meaning (e.g., "Court interprets statute X...")
- overrules: A explicitly invalidates/reverses B (e.g., "overruled by...")
- supersedes: A replaces B as newer version (e.g., "ADA supersedes Rehabilitation Act")
- distinguishes: A differentiates its facts from B (e.g., "distinguishable from Miranda...")
- complies_with: A states compliance with B (e.g., "complies with GDPR")
- applies: A applies B's legal test/doctrine (e.g., "applying Chevron framework")

ACADEMIC DOMAIN:
- cites: A cites B with author/year (e.g., "Smith et al. (2023)", "doi:...")
- extends: A builds upon B's findings (e.g., "extends Chen's work by...")
- applies: A uses B's methodology (e.g., "using grounded theory approach")
- references: A mentions B for background (e.g., "see Bishop (2006)")

GENERAL:
- references: A mentions/links to B
- contains: A includes B as subsection
- depends_on: A requires B as prerequisite
- modifies: A overrides/customizes B

OUTPUT JSON FORMAT:
{{"has_connection": bool, "direction": "a_to_b"|"b_to_a"|"bidirectional"|"none", "relationship_type": "...", "category": "...", "domain": "code"|"legal"|"academic"|"general", "confidence": 0.0-1.0, "description": "why"}}
<|im_end|>
<|im_start|>user
{}
Content A:
{}

Content B:
{}

Analyze: Does A have a structural relationship with B? If unrelated, output none.
<|im_end|>
<|im_start|>assistant
"#,
            domain_context, truncated_a, truncated_b
        )
    }

    /// Build a prompt for batch analysis of multiple memory pairs.
    ///
    /// # Arguments
    /// * `pairs` - Vector of (memory_a, memory_b) content pairs
    ///
    /// # Returns
    /// ChatML-formatted prompt string expecting JSON array response
    pub fn build_batch_prompt(&self, pairs: &[(String, String)]) -> String {
        let mut pairs_text = String::new();

        for (i, (a, b)) in pairs.iter().enumerate() {
            let truncated_a = self.truncate_content(a);
            let truncated_b = self.truncate_content(b);

            pairs_text.push_str(&format!(
                "Pair {}:\n  A: {}\n  B: {}\n\n",
                i + 1,
                truncated_a,
                truncated_b
            ));
        }

        format!(
            r#"<|im_start|>system
You are an expert relationship analyzer for knowledge graphs.

For each pair, output JSON with:
- has_connection: true if structural relationship exists
- direction: "a_to_b", "b_to_a", "bidirectional", or "none"
- relationship_type: contains, scoped_by, depends_on, imports, requires, references, cites, interprets, distinguishes, implements, complies_with, fulfills, extends, modifies, supersedes, overrules, calls, applies, used_by, or none
- category: containment, dependency, reference, implementation, extension, or invocation
- domain: code, legal, academic, or general
- confidence: 0.0 to 1.0
- description: brief explanation

Semantic similarity alone is NOT a structural relationship.
<|im_end|>
<|im_start|>user
Analyze these content pairs for structural relationships:

{}
<|im_end|>
<|im_start|>assistant
"#,
            pairs_text
        )
    }

    /// Build a prompt for validating a specific relationship type.
    ///
    /// # Arguments
    /// * `memory_a` - First memory content
    /// * `memory_b` - Second memory content
    /// * `expected_type` - The relationship type to validate
    ///
    /// # Returns
    /// ChatML-formatted prompt string
    pub fn build_validation_prompt(
        &self,
        memory_a: &str,
        memory_b: &str,
        expected_type: RelationshipType,
    ) -> String {
        let truncated_a = self.truncate_content(memory_a);
        let truncated_b = self.truncate_content(memory_b);

        // Auto-detect domain from content
        let domain = DomainMarkers::detect_domain_pair(memory_a, memory_b);
        let domain_hint = self.domain_hint(domain);

        format!(
            r#"<|im_start|>system
You validate structural relationships. Determine if the proposed relationship is accurate.

Output JSON with these fields:
- valid: true if the relationship exists, false otherwise
- confidence: 0.0 to 1.0 indicating your confidence
- explanation: brief explanation of your assessment
<|im_end|>
<|im_start|>user
{}Does Content A have a "{}" relationship with Content B?

Content A:
{}

Content B:
{}
<|im_end|>
<|im_start|>assistant
"#,
            domain_hint,
            expected_type.as_str(),
            truncated_a,
            truncated_b
        )
    }

    /// Generate domain-specific hint for the prompt.
    fn domain_hint(&self, domain: ContentDomain) -> String {
        match domain {
            ContentDomain::Code => {
                "[CODE DOMAIN DETECTED]\nFocus on: imports (use/from statements), implements (trait impl), extends (class inheritance), calls (function invocation), contains (module hierarchy), depends_on (Cargo/package deps).\nDO NOT confuse: extends (inheritance) vs implements (interface) - they are different!\n\n".to_string()
            }
            ContentDomain::Legal => {
                "[LEGAL DOMAIN DETECTED]\nFocus on: cites (case citations with U.S./F.3d/etc), interprets (explaining statute meaning), overrules (explicitly invalidating), supersedes (replacing law), distinguishes (differentiating facts), complies_with (regulatory compliance), applies (using legal doctrine).\nDO NOT confuse: cites (formal citation) vs references (general mention)!\n\n".to_string()
            }
            ContentDomain::Academic => {
                "[ACADEMIC DOMAIN DETECTED]\nFocus on: cites (author/year citations), extends (building on prior work), applies (using methodology), references (background mention).\nDO NOT confuse: cites (et al., doi:) vs references (see also)!\n\n".to_string()
            }
            ContentDomain::General => {
                "[GENERAL DOMAIN]\nCarefully check if contents have an actual structural link. Semantic similarity alone is NOT a relationship.\n\n".to_string()
            }
        }
    }

    /// Truncate content to max length at word boundary.
    fn truncate_content(&self, content: &str) -> String {
        // Normalize whitespace
        let content: String = content
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        if content.len() <= self.max_content_length {
            return content;
        }

        // HIGH-4 FIX: Use floor_char_boundary to avoid panic on multi-byte UTF-8
        let safe_end = content.floor_char_boundary(self.max_content_length);
        let truncated = &content[..safe_end];
        if let Some(last_space) = truncated.rfind(' ') {
            format!("{}...", &content[..last_space])
        } else {
            format!("{}...", truncated)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_builder_default() {
        let builder = GraphPromptBuilder::new();
        assert_eq!(builder.max_content_length, 1500);
    }

    #[test]
    fn test_analysis_prompt_format() {
        let builder = GraphPromptBuilder::new();
        let prompt = builder.build_analysis_prompt("use crate::foo;", "pub mod foo {}");

        // Verify ChatML tokens present
        assert!(prompt.contains("<|im_start|>system"));
        assert!(prompt.contains("<|im_end|>"));
        // Verify user content is included
        assert!(prompt.contains("use crate::foo;"));
        assert!(prompt.contains("pub mod foo"));
        // Verify prompt ends with assistant tag for model to respond
        assert!(prompt.ends_with("<|im_start|>assistant\n"));
    }

    #[test]
    fn test_analysis_prompt_includes_multi_domain() {
        let builder = GraphPromptBuilder::new();
        let prompt = builder.build_analysis_prompt("use crate::foo;", "pub mod foo {}");

        // Should include all domain sections in system prompt
        assert!(prompt.contains("CODE DOMAIN:"));
        assert!(prompt.contains("LEGAL DOMAIN:"));
        assert!(prompt.contains("ACADEMIC DOMAIN:"));
        // Should include new relationship types
        assert!(prompt.contains("cites"));
        assert!(prompt.contains("overrules"));
        assert!(prompt.contains("complies_with"));
    }

    #[test]
    fn test_analysis_prompt_code_domain_hint() {
        let builder = GraphPromptBuilder::new();
        let prompt = builder.build_analysis_prompt(
            "fn main() { use crate::foo; }",
            "pub mod foo { pub fn bar() {} }",
        );

        // Should include code domain hint
        assert!(prompt.contains("[CODE DOMAIN DETECTED]"));
        assert!(prompt.contains("imports"));
        assert!(prompt.contains("calls"));
        assert!(prompt.contains("implements"));
    }

    #[test]
    fn test_analysis_prompt_legal_domain_hint() {
        let builder = GraphPromptBuilder::new();
        let prompt = builder.build_analysis_prompt(
            "The court held pursuant to 42 U.S.C. § 1983 that the plaintiff...",
            "In Brown v. Board of Education, 347 U.S. 483 (1954), the court established...",
        );

        // Should include legal domain hint
        assert!(prompt.contains("[LEGAL DOMAIN DETECTED]"));
        assert!(prompt.contains("cites"));
        assert!(prompt.contains("interprets"));
        assert!(prompt.contains("overrules"));
    }

    #[test]
    fn test_analysis_prompt_academic_domain_hint() {
        let builder = GraphPromptBuilder::new();
        let prompt = builder.build_analysis_prompt(
            "Smith et al. (2023) found statistical significance (p < 0.05) with n = 150 participants.",
            "This methodology follows the approach described in prior research...",
        );

        // Should include academic domain hint
        assert!(prompt.contains("[ACADEMIC DOMAIN DETECTED]"));
        assert!(prompt.contains("cites"));
        assert!(prompt.contains("applies"));
        assert!(prompt.contains("extends"));
    }

    #[test]
    fn test_content_truncation() {
        let builder = GraphPromptBuilder::new().with_max_content_length(20);
        let long_content = "This is a very long content string that should be truncated";
        let truncated = builder.truncate_content(long_content);

        assert!(truncated.len() <= 25); // 20 + "..."
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn test_batch_prompt_format() {
        let builder = GraphPromptBuilder::new();
        let pairs = vec![
            ("use foo;".to_string(), "mod foo;".to_string()),
            ("calls bar()".to_string(), "fn bar() {}".to_string()),
        ];

        let prompt = builder.build_batch_prompt(&pairs);
        assert!(prompt.contains("Pair 1:"));
        assert!(prompt.contains("Pair 2:"));
        // Should include all types
        assert!(prompt.contains("cites"));
        assert!(prompt.contains("overrules"));
    }

    #[test]
    fn test_validation_prompt_format() {
        let builder = GraphPromptBuilder::new();
        let prompt = builder.build_validation_prompt(
            "use crate::module;",
            "pub mod module {}",
            RelationshipType::Imports,
        );

        assert!(prompt.contains("imports"));
        assert!(prompt.contains("valid"));
    }

    #[test]
    fn test_validation_prompt_legal_type() {
        let builder = GraphPromptBuilder::new();
        let prompt = builder.build_validation_prompt(
            "This case cites Brown v. Board",
            "Brown v. Board of Education, 347 U.S. 483",
            RelationshipType::Cites,
        );

        assert!(prompt.contains("cites"));
        assert!(prompt.contains("valid"));
    }
}
