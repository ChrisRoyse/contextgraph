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
        let domain_hint = self.domain_hint(domain);

        format!(
            r#"<|im_start|>system
You are an expert relationship analyzer for knowledge graphs.

TASK: Determine if Content A and Content B have a structural relationship.

CONTENT DOMAINS:
- code: Programming code, APIs, software documentation
- legal: Cases, statutes, contracts, regulations
- academic: Research papers, studies, citations
- general: Other content

RELATIONSHIP CATEGORIES:
1. CONTAINMENT: A contains/scopes B (hierarchy)
   Types: contains, scoped_by
2. DEPENDENCY: A requires/depends on B
   Types: depends_on, imports, requires
3. REFERENCE: A references/cites B (mention)
   Types: references, cites, interprets, distinguishes
4. IMPLEMENTATION: A implements/realizes B
   Types: implements, complies_with, fulfills
5. EXTENSION: A extends/modifies B
   Types: extends, modifies, supersedes, overrules
6. INVOCATION: A applies/uses B
   Types: calls, applies, used_by

OUTPUT FORMAT (JSON):
{{
  "has_connection": true/false,
  "direction": "a_to_b" | "b_to_a" | "bidirectional" | "none",
  "relationship_type": "<type>",
  "category": "<category>",
  "domain": "code" | "legal" | "academic" | "general",
  "confidence": 0.0-1.0,
  "description": "Brief explanation"
}}

IMPORTANT:
- Semantic similarity alone is NOT a structural relationship
- Only identify clear structural/dependency relationships
<|im_end|>
<|im_start|>user
{}Analyze if there is a structural relationship between these contents:

Content A:
{}

Content B:
{}
<|im_end|>
<|im_start|>assistant
"#,
            domain_hint, truncated_a, truncated_b
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
                "Domain hint: This appears to be programming code.\nRelevant types: imports, calls, implements, extends, contains, depends_on\n\n".to_string()
            }
            ContentDomain::Legal => {
                "Domain hint: This appears to be legal content.\nRelevant types: cites, interprets, overrules, supersedes, distinguishes, complies_with\n\n".to_string()
            }
            ContentDomain::Academic => {
                "Domain hint: This appears to be academic/research content.\nRelevant types: cites, applies, extends, references\n\n".to_string()
            }
            ContentDomain::General => String::new(),
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

        // Find last space before max length
        let truncated = &content[..self.max_content_length];
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

        // Should include all domain options
        assert!(prompt.contains("code:"));
        assert!(prompt.contains("legal:"));
        assert!(prompt.contains("academic:"));
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
        assert!(prompt.contains("Domain hint: This appears to be programming code"));
        assert!(prompt.contains("imports, calls, implements"));
    }

    #[test]
    fn test_analysis_prompt_legal_domain_hint() {
        let builder = GraphPromptBuilder::new();
        let prompt = builder.build_analysis_prompt(
            "The court held pursuant to 42 U.S.C. § 1983 that the plaintiff...",
            "In Brown v. Board of Education, 347 U.S. 483 (1954), the court established...",
        );

        // Should include legal domain hint
        assert!(prompt.contains("Domain hint: This appears to be legal content"));
        assert!(prompt.contains("cites, interprets, overrules"));
    }

    #[test]
    fn test_analysis_prompt_academic_domain_hint() {
        let builder = GraphPromptBuilder::new();
        let prompt = builder.build_analysis_prompt(
            "Smith et al. (2023) found statistical significance (p < 0.05) with n = 150 participants.",
            "This methodology follows the approach described in prior research...",
        );

        // Should include academic domain hint
        assert!(prompt.contains("Domain hint: This appears to be academic"));
        assert!(prompt.contains("cites, applies, extends"));
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
