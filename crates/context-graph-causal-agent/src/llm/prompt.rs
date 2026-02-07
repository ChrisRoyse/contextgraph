//! Prompt templates for the Causal Discovery LLM.
//!
//! Uses Hermes 2 Pro Mistral's ChatML format for optimal function-calling performance.
//! The model is specifically trained for structured output and JSON generation.
//!
//! # Single-Text Analysis
//!
//! The [`build_single_text_prompt`](CausalPromptBuilder::build_single_text_prompt) method
//! generates prompts for analyzing a SINGLE text for causal nature, used during memory
//! storage to provide hints to the E5 embedder.

/// Builder for causal analysis prompts.
#[derive(Debug, Clone)]
pub struct CausalPromptBuilder {
    /// System prompt for the LLM.
    system_prompt: String,

    /// System prompt for single-text analysis.
    single_text_system_prompt: String,

    /// System prompt for multi-relationship extraction.
    multi_relationship_system_prompt: String,

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
            single_text_system_prompt: Self::default_single_text_system_prompt().to_string(),
            multi_relationship_system_prompt: Self::default_multi_relationship_system_prompt()
                .to_string(),
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

    /// Default system prompt for single-text causal analysis.
    ///
    /// Generates structured causal analysis with 1-3 paragraph descriptions.
    /// Descriptions enable semantic search of causal relationships.
    const fn default_single_text_system_prompt() -> &'static str {
        r#"You analyze text for causal content and generate rich causal descriptions.

TASK: Determine if the text describes causes, effects, or is causal in nature.

OUTPUT FORMAT (JSON):
{"is_causal":true/false,"direction":"cause"/"effect"/"neutral","confidence":0.0-1.0,"key_phrases":[],"description":"...","asymmetry_strength":0.0-1.0,"cause_entities":[],"effect_entities":[]}

DIRECTION CLASSIFICATION:
- "cause": Text describes something that CAUSES other things
  Example: "High cortisol levels cause memory impairment"

- "effect": Text describes something that IS CAUSED by other things
  Example: "Memory impairment results from chronic stress"

- "neutral": Either non-causal OR equally describes both cause and effect

KEY_PHRASES: Extract 1-3 causal markers (e.g., "causes", "leads to", "results from")

ASYMMETRY_STRENGTH: How directional is the relationship? [0.0-1.0]
- 1.0: Strongly directional (A clearly causes B, not vice versa)
- 0.5: Moderate asymmetry
- 0.0: Bidirectional or symmetric

CAUSE_ENTITIES: Array of cause entity spans with character offsets into the original text.
Each entry: {"start": char_offset, "end": char_offset, "label": "entity name"}
Identify the key cause entities/phrases. Offsets MUST point into the input text.

EFFECT_ENTITIES: Array of effect entity spans with character offsets into the original text.
Each entry: {"start": char_offset, "end": char_offset, "label": "entity name"}
Identify the key effect entities/phrases. Offsets MUST point into the input text.

If confidence < 0.5, set cause_entities and effect_entities to empty arrays.

DESCRIPTION (CRITICAL - generate when confidence >= 0.5):
Write 1-3 paragraphs explaining the causal relationship.

Paragraph 1 - RELATIONSHIP: State the causal relationship clearly.
"X causes Y" or "Y is an effect of X"

Paragraph 2 - MECHANISM: Explain HOW or WHY this causal link exists.
Evidence, process, or mechanism details.

Paragraph 3 - CONTEXT: Implications, scope, or conditions.
When does this apply? What are the consequences?

Use \n to separate paragraphs within the description string.

If confidence < 0.5, set description to empty string "".

EXAMPLE OUTPUT (causal):
{"is_causal":true,"direction":"cause","confidence":0.9,"key_phrases":["causes","leads to"],"description":"High cortisol levels cause memory impairment by damaging hippocampal neurons.\n\nThe mechanism involves prolonged glucocorticoid exposure triggering oxidative stress and reducing synaptic plasticity in the hippocampus.\n\nThis relationship is particularly relevant in chronic stress conditions and aging.","asymmetry_strength":0.9,"cause_entities":[{"start":0,"end":20,"label":"high cortisol levels"}],"effect_entities":[{"start":27,"end":44,"label":"memory impairment"}]}

EXAMPLE OUTPUT (non-causal):
{"is_causal":false,"direction":"neutral","confidence":0.1,"key_phrases":[],"description":"","asymmetry_strength":0.0,"cause_entities":[],"effect_entities":[]}

CONFIDENCE:
- 0.9-1.0: Clear causal language with explicit markers
- 0.7-0.8: Implicit causation, strong language
- 0.5-0.6: Possible causation, weaker indicators
- 0.0-0.4: No clear causal content"#
    }

    /// System prompt for extracting ALL causal relationships from text.
    ///
    /// Unlike single-text analysis which returns ONE hint, this extracts
    /// every distinct cause-effect relationship from the content.
    const fn default_multi_relationship_system_prompt() -> &'static str {
        r#"You analyze text for ALL cause-effect relationships and generate explanatory paragraphs with source provenance.

TASK: Extract every distinct cause-effect relationship from the text with exact source locations.

OUTPUT FORMAT (JSON):
{
  "relationships": [
    {
      "cause": "Brief statement of the cause",
      "effect": "Brief statement of the effect",
      "explanation": "1-2 paragraph explanation of HOW and WHY this causal link exists",
      "confidence": 0.0-1.0,
      "mechanism_type": "direct" | "mediated" | "feedback" | "temporal",
      "source_spans": [
        {
          "start_char": 0,
          "end_char": 100,
          "text_excerpt": "...exact text from input...",
          "span_type": "full" | "cause" | "effect"
        }
      ]
    }
  ],
  "has_causal_content": true/false
}

SOURCE SPAN EXTRACTION (CRITICAL - READ CAREFULLY):

RULE: text_excerpt MUST be EXACT CHARACTER-FOR-CHARACTER COPY from input.
DO NOT paraphrase. DO NOT summarize. DO NOT change ANY words.

CORRECT EXAMPLE:
Input: "High cortisol from chronic stress damages hippocampal neurons, leading to memory problems."
        ^                                         ^
        position 0                                position 56

Output span:
{
  "start_char": 0,
  "end_char": 56,
  "text_excerpt": "High cortisol from chronic stress damages hippocampal neurons",
  "span_type": "full"
}
✓ CORRECT - text_excerpt equals input[0:56] exactly

WRONG EXAMPLE (DO NOT DO THIS):
{
  "start_char": 0,
  "end_char": 56,
  "text_excerpt": "Elevated cortisol damages neurons",
  "span_type": "full"
}
✗ WRONG - "Elevated cortisol damages neurons" is NOT at positions 0-56

HOW TO EXTRACT:
1. Find exact start position in input
2. Find exact end position in input
3. Copy EXACTLY those characters: text_excerpt = input[start_char:end_char]

ADDITIONAL RULES:
- span_type indicates which part: "full" (entire relationship), "cause", or "effect"
- Each relationship MUST have at least one source_span
- Verify character offsets are accurate before outputting
- If relationship spans multiple sentences, use span_type "full"
- Truncate text_excerpt to 200 characters if needed

EXPLANATION REQUIREMENTS (CRITICAL FOR E5 EMBEDDING):

The explanation is embedded via E5 asymmetric similarity for causal search.
Rich mechanism details = better retrieval. Vague summaries = poor retrieval.

REQUIRED FORMAT (2 PARAGRAPHS, 40-60 WORDS TOTAL):

PARAGRAPH 1: State cause-effect with SPECIFIC mechanism.
- Include scientific/technical terms (e.g., "HPA axis", "glucocorticoid receptors")
- Name specific pathways, molecules, or processes

PARAGRAPH 2: Describe consequences and conditions.
- Include downstream effects
- Mention relevant context or scope

SEPARATE PARAGRAPHS WITH \n\n (double newline).

BAD (vague, no mechanism):
"Stress causes memory problems. This is harmful."
✗ No mechanism details, useless for search

GOOD (specific mechanism):
"Chronic stress activates the HPA axis, elevating cortisol which binds to hippocampal glucocorticoid receptors, triggering oxidative stress and reduced BDNF expression.\n\nThis impairs synaptic plasticity and long-term potentiation, degrading memory encoding and consolidation over time."
✓ Named: HPA axis, glucocorticoid, BDNF, synaptic plasticity, LTP

EXTRACT ALL RELATIONSHIPS:
- If text mentions "A causes B" and "B causes C", extract BOTH relationships
- If text mentions feedback loops, extract as bidirectional relationship
- If text has no causal content, return {"relationships": [], "has_causal_content": false}

CONFIDENCE:
- 0.9-1.0: Explicit causal language ("causes", "leads to", "results in")
- 0.7-0.8: Strong implicit causation
- 0.5-0.6: Possible causation, weaker indicators
- <0.5: Do not include (skip uncertain relationships)

MECHANISM TYPES:
- "direct": A directly causes B without intermediaries
- "mediated": A causes X which causes B (indirect pathway)
- "feedback": A and B mutually reinforce each other (loops)
- "temporal": A precedes B in a necessary sequence

EXAMPLES:

Input: "High cortisol from chronic stress damages hippocampal neurons, leading to memory problems."
Output:
{
  "relationships": [
    {
      "cause": "Chronic stress elevates cortisol levels",
      "effect": "Elevated cortisol damages hippocampal neurons",
      "explanation": "Chronic psychological stress triggers sustained activation of the HPA axis, resulting in persistently elevated cortisol levels.\n\nThis glucocorticoid excess causes oxidative stress and reduces synaptic plasticity in hippocampal neurons.",
      "confidence": 0.85,
      "mechanism_type": "mediated",
      "source_spans": [
        {
          "start_char": 0,
          "end_char": 56,
          "text_excerpt": "High cortisol from chronic stress damages hippocampal neurons",
          "span_type": "full"
        }
      ]
    },
    {
      "cause": "Hippocampal neuron damage",
      "effect": "Memory impairment",
      "explanation": "The hippocampus is critical for memory formation and consolidation. When neurons in this region are damaged by cortisol exposure, memory encoding becomes impaired.\n\nThis manifests as difficulty forming new memories and retrieving recent ones.",
      "confidence": 0.90,
      "mechanism_type": "direct",
      "source_spans": [
        {
          "start_char": 32,
          "end_char": 89,
          "text_excerpt": "damages hippocampal neurons, leading to memory problems",
          "span_type": "full"
        }
      ]
    }
  ],
  "has_causal_content": true
}

Input: "The sky is blue on clear days."
Output:
{
  "relationships": [],
  "has_causal_content": false
}"#
    }

    /// Build prompt for analyzing a SINGLE text for causal nature.
    ///
    /// Used during memory storage to provide hints to the E5 embedder.
    /// Optimized for fast classification (~50ms latency target).
    ///
    /// # Arguments
    /// * `content` - The text content to analyze
    ///
    /// # Returns
    /// A ChatML-formatted prompt for single-text causal analysis.
    pub fn build_single_text_prompt(&self, content: &str) -> String {
        let truncated = self.truncate_content(content);
        format!(
            r#"<|im_start|>system
{}
<|im_end|>
<|im_start|>user
Text: "{}"
<|im_end|>
<|im_start|>assistant
{{"is_causal":"#,
            self.single_text_system_prompt, truncated
        )
    }

    /// Build prompt for extracting ALL causal relationships from text.
    ///
    /// Unlike [`build_single_text_prompt`](Self::build_single_text_prompt) which
    /// returns a single hint, this extracts every distinct cause-effect
    /// relationship from the content, each with its own explanatory paragraph.
    ///
    /// # Arguments
    /// * `content` - The text content to analyze
    ///
    /// # Returns
    /// A ChatML-formatted prompt for multi-relationship extraction.
    pub fn build_multi_relationship_prompt(&self, content: &str) -> String {
        let truncated = self.truncate_content(content);
        format!(
            r#"<|im_start|>system
{}
<|im_end|>
<|im_start|>user
Analyze for causal relationships:

"{}"
<|im_end|>
<|im_start|>assistant
"#,
            self.multi_relationship_system_prompt,
            truncated.replace('"', "\\\"")
        )
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
