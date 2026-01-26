//! Prompt templates for the Causal Discovery LLM.
//!
//! Uses Hermes 2 Pro Mistral's ChatML format for optimal function-calling performance.
//! The model is specifically trained for structured output and JSON generation.

/// Builder for causal analysis prompts.
#[derive(Debug, Clone)]
pub struct CausalPromptBuilder {
    /// System prompt for the LLM.
    system_prompt: String,

    /// Maximum content length per memory (characters).
    max_content_length: usize,
}

impl Default for CausalPromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl CausalPromptBuilder {
    /// Create a new prompt builder with default settings.
    pub fn new() -> Self {
        Self {
            system_prompt: Self::default_system_prompt().to_string(),
            max_content_length: 1500,
        }
    }

    /// Set a custom system prompt.
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = prompt.into();
        self
    }

    /// Set maximum content length per memory.
    pub fn with_max_content_length(mut self, length: usize) -> Self {
        self.max_content_length = length;
        self
    }

    /// Build the full analysis prompt for two memories.
    ///
    /// Uses Hermes 2 Pro ChatML format with structured output instructions.
    /// The model will generate valid JSON constrained by GBNF grammar.
    pub fn build_analysis_prompt(&self, memory_a: &str, memory_b: &str) -> String {
        let truncated_a = self.truncate_content(memory_a);
        let truncated_b = self.truncate_content(memory_b);

        format!(
            r#"<|im_start|>system
{}
<|im_end|>
<|im_start|>user
Analyze the causal relationship between these two statements:

Statement A: "{}"

Statement B: "{}"

Determine if there is a causal relationship and respond with JSON.
<|im_end|>
<|im_start|>assistant
"#,
            self.system_prompt, truncated_a, truncated_b
        )
    }

    /// Build a batch analysis prompt for multiple pairs.
    pub fn build_batch_prompt(&self, pairs: &[(String, String)]) -> String {
        let mut pairs_text = String::new();
        for (i, (a, b)) in pairs.iter().enumerate() {
            let truncated_a = self.truncate_content(a);
            let truncated_b = self.truncate_content(b);
            pairs_text.push_str(&format!(
                "Pair {}:\n  A: \"{}\"\n  B: \"{}\"\n\n",
                i + 1,
                truncated_a,
                truncated_b
            ));
        }

        format!(
            r#"<|im_start|>system
{}
<|im_end|>
<|im_start|>user
Analyze these statement pairs for causal relationships:

{}

For each pair, determine if there's a causal relationship.
<|im_end|>
<|im_start|>assistant
"#,
            self.system_prompt, pairs_text
        )
    }

    /// Truncate content to maximum length.
    fn truncate_content(&self, content: &str) -> String {
        if content.len() <= self.max_content_length {
            content.to_string()
        } else {
            let truncated = &content[..self.max_content_length];
            // Find last complete word
            if let Some(last_space) = truncated.rfind(' ') {
                format!("{}...", &truncated[..last_space])
            } else {
                format!("{}...", truncated)
            }
        }
    }

    /// Default system prompt optimized for Hermes 2 Pro.
    ///
    /// Hermes 2 Pro is trained for function calling and structured output,
    /// so we keep the prompt focused and direct.
    const fn default_system_prompt() -> &'static str {
        r#"You are an expert in causal inference for knowledge graphs.

TASK: Analyze if Statement A and Statement B have a causal relationship.

OUTPUT FORMAT (JSON):
{
  "causal_link": true/false,
  "direction": "A_causes_B" | "B_causes_A" | "bidirectional" | "none",
  "confidence": 0.0-1.0,
  "mechanism": "Specific causal mechanism (1-2 sentences)",
  "mechanism_type": "direct" | "mediated" | "feedback" | "temporal"
}

MECHANISM EXTRACTION (CRITICAL):
Bad:  "A causes B" (tautological - useless for retrieval)
Good: "Increased cortisol impairs hippocampal function" (actionable, searchable)

MECHANISM TYPES:
- "direct": A directly causes B without intermediaries
- "mediated": A causes X which causes B (indirect pathway)
- "feedback": A and B mutually reinforce each other (loops)
- "temporal": A precedes B in a necessary sequence

DIRECTION DECISION:
1. Does A describe an ACTION/STATE that could produce B's OUTCOME?
2. Does B describe an ACTION/STATE that could produce A's OUTCOME?
3. If A→B only: "A_causes_B"
4. If B→A only: "B_causes_A"
5. If both (feedback): "bidirectional"
6. If neither: "none"

CONFIDENCE CALIBRATION:
- 0.9-1.0: Established mechanism, interventional evidence
- 0.7-0.8: Strong evidence, clear pathway
- 0.5-0.6: Plausible, some confounding possible
- 0.3-0.4: Weak/indirect, correlation may explain
- 0.0-0.2: No causal link

IMPORTANT: Correlation, semantic similarity, or topical overlap are NOT causation. Be conservative."#
    }
}

impl CausalPromptBuilder {
    /// Build analysis prompt with few-shot examples for better accuracy.
    pub fn build_analysis_prompt_with_examples(&self, memory_a: &str, memory_b: &str) -> String {
        let truncated_a = self.truncate_content(memory_a);
        let truncated_b = self.truncate_content(memory_b);

        format!(
            r#"<|im_start|>system
{}
<|im_end|>
<|im_start|>user
Example 1:
A: "Aspirin inhibits cyclooxygenase enzymes"
B: "Reduced prostaglandin synthesis decreases inflammation"
Answer: {{"causal_link":true,"direction":"A_causes_B","confidence":0.85,"mechanism":"Aspirin's COX inhibition reduces prostaglandin production, which mediates inflammation.","mechanism_type":"mediated"}}

Example 2:
A: "Patients showed improved cognitive function"
B: "The drug crosses the blood-brain barrier"
Answer: {{"causal_link":true,"direction":"B_causes_A","confidence":0.75,"mechanism":"BBB crossing enables drug delivery to neurons, improving function.","mechanism_type":"direct"}}

Example 3:
A: "Chronic stress elevates cortisol levels"
B: "High cortisol impairs hippocampal neurogenesis"
Answer: {{"causal_link":true,"direction":"A_causes_B","confidence":0.80,"mechanism":"Stress-induced cortisol elevation damages hippocampal neurons.","mechanism_type":"mediated"}}

Example 4:
A: "Inflammation increases pain sensitivity"
B: "Pain triggers stress response which worsens inflammation"
Answer: {{"causal_link":true,"direction":"bidirectional","confidence":0.75,"mechanism":"Pain and inflammation form a positive feedback loop.","mechanism_type":"feedback"}}

Example 5:
A: "The patient has blue eyes"
B: "The tumor was malignant"
Answer: {{"causal_link":false,"direction":"none","confidence":0.00,"mechanism":"Eye color and tumor malignancy are biologically unrelated.","mechanism_type":"direct"}}

Now analyze:
A: "{}"
B: "{}"
<|im_end|>
<|im_start|>assistant
"#,
            self.system_prompt, truncated_a, truncated_b
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_analysis_prompt() {
        let builder = CausalPromptBuilder::new();
        let prompt = builder.build_analysis_prompt("Event A occurred", "Event B followed");

        assert!(prompt.contains("<|im_start|>system"));
        assert!(prompt.contains("Event A occurred"));
        assert!(prompt.contains("Event B followed"));
        assert!(prompt.contains("causal_link"));
    }

    #[test]
    fn test_truncate_content() {
        let builder = CausalPromptBuilder::new().with_max_content_length(50);

        let short = "This is a short string";
        assert_eq!(builder.truncate_content(short), short);

        let long = "This is a much longer string that exceeds the maximum content length limit";
        let truncated = builder.truncate_content(long);
        assert!(truncated.len() <= 55); // 50 + "..."
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn test_build_batch_prompt() {
        let builder = CausalPromptBuilder::new();
        let pairs = vec![
            ("A1".to_string(), "B1".to_string()),
            ("A2".to_string(), "B2".to_string()),
        ];

        let prompt = builder.build_batch_prompt(&pairs);

        assert!(prompt.contains("Pair 1:"));
        assert!(prompt.contains("Pair 2:"));
        assert!(prompt.contains("A1"));
        assert!(prompt.contains("B2"));
    }
}
