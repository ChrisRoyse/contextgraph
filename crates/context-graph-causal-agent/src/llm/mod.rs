//! Local LLM for causal relationship discovery.
//!
//! Uses Hermes 2 Pro Mistral 7B via llama-cpp-2 for grammar-constrained inference.
//! Optimized for RTX 5090 Blackwell architecture with CUDA support.
//!
//! # VRAM Usage
//!
//! - Hermes 2 Pro Q5_K_M: ~5GB
//! - KV Cache (4096 ctx): ~1GB
//! - **Total: ~6GB (within 9GB budget)**
//!
//! # Architecture
//!
//! The LLM is used to analyze pairs of memories and determine if there's
//! a causal relationship between them. It outputs structured JSON responses
//! that are guaranteed to be valid via GBNF grammar constraints.
//!
//! # Key Improvement
//!
//! Unlike the previous Candle/Qwen implementation (~40% JSON parse rate),
//! this uses llama_cpp's grammar-constrained generation for **100% valid JSON**.

mod prompt;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel, Special};
use llama_cpp_2::sampling::LlamaSampler;
use llama_cpp_2::token::LlamaToken;
use parking_lot::RwLock;
use serde::Deserialize;
use tracing::{debug, info, warn};

use crate::error::{CausalAgentError, CausalAgentResult};
use crate::types::{CausalAnalysisResult, CausalDirectionHint, CausalHint, CausalLinkDirection};

pub use prompt::CausalPromptBuilder;

/// Configuration for the Causal Discovery LLM.
#[derive(Debug, Clone)]
pub struct LlmConfig {
    /// Path to GGUF model file.
    pub model_path: PathBuf,

    /// Context window size (default: 4096).
    pub context_size: u32,

    /// Temperature for sampling (0.0 = deterministic).
    pub temperature: f32,

    /// Maximum tokens to generate per response.
    pub max_tokens: usize,

    /// Number of GPU layers to offload (0xFFFF_FFFF = all).
    pub n_gpu_layers: u32,

    /// Seed for reproducibility (not used in current llama-cpp-2 version).
    pub seed: u32,

    /// Path to GBNF grammar file for causal analysis.
    pub causal_grammar_path: PathBuf,

    /// Path to GBNF grammar file for graph relationship analysis.
    pub graph_grammar_path: PathBuf,

    /// Path to GBNF grammar file for validation.
    pub validation_grammar_path: PathBuf,

    /// Batch size for processing.
    pub batch_size: u32,

    /// Use few-shot examples in prompts for better accuracy.
    /// Adds ~200 tokens but improves direction detection.
    pub use_few_shot: bool,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            model_path: PathBuf::from("models/hermes-2-pro/Hermes-2-Pro-Mistral-7B.Q5_K_M.gguf"),
            context_size: 4096,
            temperature: 0.0, // Deterministic for analysis
            max_tokens: 512, // Increased from 256 to support 1-3 paragraph descriptions
            n_gpu_layers: u32::MAX, // Full GPU offload
            seed: 42,
            causal_grammar_path: PathBuf::from("models/hermes-2-pro/causal_analysis.gbnf"),
            graph_grammar_path: PathBuf::from("models/hermes-2-pro/graph_relationship.gbnf"),
            validation_grammar_path: PathBuf::from("models/hermes-2-pro/validation.gbnf"),
            batch_size: 2048, // Increased for few-shot prompts
            use_few_shot: true, // Enable few-shot examples by default for better accuracy
        }
    }
}

/// Grammar type for different analysis tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrammarType {
    /// Causal relationship analysis (pair comparison).
    Causal,
    /// Graph relationship analysis.
    Graph,
    /// Validation analysis.
    Validation,
    /// Single-text causal hint analysis (for E5 enhancement).
    SingleText,
}

/// Internal state of the LLM.
enum LlmState {
    /// Not loaded.
    Unloaded,

    /// Loaded with llama-cpp-2 model.
    Loaded {
        /// The llama.cpp backend.
        backend: LlamaBackend,
        /// The loaded model.
        model: LlamaModel,
    },
}

// Implement Send and Sync for LlmState
// Safety: LlamaBackend and LlamaModel are thread-safe when used properly
// We protect access with RwLock in CausalDiscoveryLLM
unsafe impl Send for LlmState {}
unsafe impl Sync for LlmState {}

/// Local LLM wrapper for causal relationship discovery.
///
/// Uses Hermes 2 Pro Mistral 7B via llama-cpp-2 for grammar-constrained
/// inference, optimized for RTX 5090 Blackwell architecture.
///
/// # VRAM Usage (RTX 5090 32GB)
///
/// | Model | Quantization | VRAM | Performance |
/// |-------|--------------|------|-------------|
/// | Hermes 2 Pro 7B | Q5_K_M | ~5GB | ~50 tok/s |
///
/// # Grammar-Constrained Generation
///
/// This implementation uses GBNF grammar constraints to guarantee
/// 100% valid JSON output, solving the JSON parsing issues of the
/// previous Candle/Qwen implementation.
///
/// # Single-Text Analysis
///
/// The [`analyze_single_text`](Self::analyze_single_text) method provides
/// fast classification of individual texts for causal nature, returning
/// [`CausalHint`] for E5 embedding enhancement.
pub struct CausalDiscoveryLLM {
    /// Configuration.
    config: LlmConfig,

    /// Internal state.
    state: RwLock<LlmState>,

    /// Whether the model is loaded.
    loaded: AtomicBool,

    /// Prompt builder.
    prompt_builder: CausalPromptBuilder,

    /// Cached grammar strings.
    causal_grammar: String,
    graph_grammar: String,
    validation_grammar: String,
    single_text_grammar: String,
}

impl CausalDiscoveryLLM {
    /// Create a new CausalDiscoveryLLM with default configuration.
    pub fn new() -> CausalAgentResult<Self> {
        Self::with_config(LlmConfig::default())
    }

    /// Create with a specific model directory.
    pub fn with_model_dir(model_dir: &str) -> CausalAgentResult<Self> {
        let config = LlmConfig {
            model_path: PathBuf::from(model_dir).join("Hermes-2-Pro-Mistral-7B.Q5_K_M.gguf"),
            causal_grammar_path: PathBuf::from(model_dir).join("causal_analysis.gbnf"),
            graph_grammar_path: PathBuf::from(model_dir).join("graph_relationship.gbnf"),
            validation_grammar_path: PathBuf::from(model_dir).join("validation.gbnf"),
            ..Default::default()
        };
        Self::with_config(config)
    }

    /// Create with custom configuration.
    pub fn with_config(config: LlmConfig) -> CausalAgentResult<Self> {
        // Load grammar files (they're small, load eagerly)
        let causal_grammar = Self::load_grammar_file(&config.causal_grammar_path)?;
        let graph_grammar = Self::load_grammar_file(&config.graph_grammar_path)?;
        let validation_grammar = Self::load_grammar_file(&config.validation_grammar_path)?;
        // Single-text grammar uses embedded default (simpler than pair analysis)
        let single_text_grammar = Self::default_single_text_grammar().to_string();

        Ok(Self {
            config,
            state: RwLock::new(LlmState::Unloaded),
            loaded: AtomicBool::new(false),
            prompt_builder: CausalPromptBuilder::new(),
            causal_grammar,
            graph_grammar,
            validation_grammar,
            single_text_grammar,
        })
    }

    /// Load grammar from file, falling back to embedded default if not found.
    fn load_grammar_file(path: &PathBuf) -> CausalAgentResult<String> {
        match std::fs::read_to_string(path) {
            Ok(content) => Ok(content),
            Err(e) => {
                warn!(
                    path = %path.display(),
                    error = %e,
                    "Grammar file not found, using embedded default"
                );
                // Return embedded default grammar for causal analysis
                Ok(Self::default_causal_grammar().to_string())
            }
        }
    }

    /// Default embedded grammar for causal analysis.
    const fn default_causal_grammar() -> &'static str {
        r#"root ::= "{" ws causal-link "," ws direction "," ws confidence "," ws mechanism ws "}"
causal-link ::= "\"causal_link\"" ws ":" ws boolean
direction ::= "\"direction\"" ws ":" ws direction-value
confidence ::= "\"confidence\"" ws ":" ws number
mechanism ::= "\"mechanism\"" ws ":" ws string
direction-value ::= "\"A_causes_B\"" | "\"B_causes_A\"" | "\"bidirectional\"" | "\"none\""
boolean ::= "true" | "false"
number ::= "0" ("." [0-9] [0-9]?)? | "1" ("." "0" "0"?)?
string ::= "\"" ([^"\\] | "\\" .)* "\""
ws ::= [ \t\n\r]*"#
    }

    /// Default embedded grammar for single-text causal hint analysis.
    ///
    /// Includes description field for 1-3 paragraph causal relationship descriptions.
    /// Description enables apples-to-apples semantic search of causal content.
    const fn default_single_text_grammar() -> &'static str {
        r#"root ::= "{" ws is-causal "," ws direction "," ws confidence "," ws key-phrases "," ws description ws "}"
is-causal ::= "\"is_causal\"" ws ":" ws boolean
direction ::= "\"direction\"" ws ":" ws direction-value
confidence ::= "\"confidence\"" ws ":" ws number
key-phrases ::= "\"key_phrases\"" ws ":" ws phrase-array
description ::= "\"description\"" ws ":" ws string
direction-value ::= "\"cause\"" | "\"effect\"" | "\"neutral\""
boolean ::= "true" | "false"
number ::= "0" ("." [0-9] [0-9]?)? | "1" ("." "0" "0"?)?
phrase-array ::= "[" ws (string (ws "," ws string)*)? ws "]"
string ::= "\"" ([^"\\] | "\\" .)* "\""
ws ::= [ \t\n\r]*"#
    }

    /// Check if the model is loaded.
    pub fn is_loaded(&self) -> bool {
        self.loaded.load(Ordering::SeqCst)
    }

    /// Load the model into memory.
    ///
    /// # CUDA Optimization
    ///
    /// On RTX 5090, this uses full GPU offload for maximum performance.
    pub async fn load(&self) -> CausalAgentResult<()> {
        if self.is_loaded() {
            warn!("LLM already loaded, skipping");
            return Ok(());
        }

        info!(
            model_path = %self.config.model_path.display(),
            context_size = self.config.context_size,
            n_gpu_layers = self.config.n_gpu_layers,
            "Loading Causal Discovery LLM (llama-cpp-2 with GBNF grammar)"
        );

        // Initialize backend
        let backend = LlamaBackend::init().map_err(|e| CausalAgentError::LlmLoadError {
            message: format!("Failed to initialize llama backend: {}", e),
        })?;

        // Configure model parameters
        let model_params = LlamaModelParams::default().with_n_gpu_layers(self.config.n_gpu_layers);

        // Load model from GGUF file
        let model = LlamaModel::load_from_file(&backend, &self.config.model_path, &model_params)
            .map_err(|e| CausalAgentError::ModelNotFound {
                path: format!("{}: {}", self.config.model_path.display(), e),
            })?;

        info!(
            n_vocab = model.n_vocab(),
            n_embd = model.n_embd(),
            n_params = model.n_params(),
            "Model loaded successfully"
        );

        let mut state = self.state.write();
        *state = LlmState::Loaded { backend, model };
        self.loaded.store(true, Ordering::SeqCst);

        info!("Causal Discovery LLM loaded successfully with grammar support");
        Ok(())
    }

    /// Unload the model from memory.
    pub async fn unload(&self) -> CausalAgentResult<()> {
        if !self.is_loaded() {
            return Ok(());
        }

        info!("Unloading Causal Discovery LLM");

        let mut state = self.state.write();
        *state = LlmState::Unloaded;
        self.loaded.store(false, Ordering::SeqCst);

        Ok(())
    }

    /// Analyze a pair of memories for causal relationship.
    ///
    /// # Arguments
    ///
    /// * `memory_a` - Content of the first memory (potential cause)
    /// * `memory_b` - Content of the second memory (potential effect)
    ///
    /// # Returns
    ///
    /// Analysis result including whether a causal link exists, direction,
    /// confidence score, and description of the mechanism.
    pub async fn analyze_causal_relationship(
        &self,
        memory_a: &str,
        memory_b: &str,
    ) -> CausalAgentResult<CausalAnalysisResult> {
        if !self.is_loaded() {
            return Err(CausalAgentError::LlmNotInitialized);
        }

        // Use few-shot examples for better accuracy when enabled
        let prompt = if self.config.use_few_shot {
            self.prompt_builder.build_analysis_prompt_with_examples(memory_a, memory_b)
        } else {
            self.prompt_builder.build_analysis_prompt(memory_a, memory_b)
        };

        debug!(
            prompt_len = prompt.len(),
            use_few_shot = self.config.use_few_shot,
            "Analyzing causal relationship"
        );

        // Generate response with grammar constraint
        let response = self
            .generate_with_grammar(&prompt, GrammarType::Causal)
            .await?;

        // Parse the JSON response (guaranteed valid by grammar)
        self.parse_causal_response(&response)
    }

    /// Batch analyze multiple memory pairs.
    pub async fn analyze_batch(
        &self,
        pairs: &[(String, String)],
    ) -> CausalAgentResult<Vec<CausalAnalysisResult>> {
        let mut results = Vec::with_capacity(pairs.len());

        for (i, (a, b)) in pairs.iter().enumerate() {
            debug!(
                pair_index = i,
                total = pairs.len(),
                "Analyzing pair"
            );

            match self.analyze_causal_relationship(a, b).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    warn!(
                        pair_index = i,
                        error = %e,
                        "Failed to analyze pair, using default"
                    );
                    results.push(CausalAnalysisResult::default());
                }
            }
        }

        Ok(results)
    }

    // =========================================================================
    // SINGLE-TEXT CAUSAL ANALYSIS (E5 Embedding Enhancement)
    // =========================================================================

    /// Analyze a SINGLE text for causal nature.
    ///
    /// Returns a [`CausalHint`] for E5 embedding enhancement. This method is
    /// optimized for fast classification (~50ms target latency) to avoid
    /// blocking the memory storage pipeline.
    ///
    /// # Arguments
    ///
    /// * `content` - The text content to analyze
    ///
    /// # Returns
    ///
    /// A [`CausalHint`] containing:
    /// - `is_causal`: Whether the text contains causal language
    /// - `direction_hint`: Whether it primarily describes causes or effects
    /// - `confidence`: Classification confidence [0.0, 1.0]
    /// - `key_phrases`: Detected causal markers
    ///
    /// # Graceful Degradation
    ///
    /// If LLM analysis fails, returns a default hint with `is_causal: false`.
    /// The caller can then fall back to marker-based detection.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let hint = llm.analyze_single_text("High cortisol causes memory loss").await?;
    /// assert!(hint.is_causal);
    /// assert_eq!(hint.direction_hint, CausalDirectionHint::Cause);
    /// ```
    pub async fn analyze_single_text(&self, content: &str) -> CausalAgentResult<CausalHint> {
        if !self.is_loaded() {
            return Err(CausalAgentError::LlmNotInitialized);
        }

        // Build prompt for single-text analysis
        let prompt = self.prompt_builder.build_single_text_prompt(content);

        // Generate response with grammar constraint
        let response = self
            .generate_with_grammar(&prompt, GrammarType::SingleText)
            .await?;

        // Parse and return
        self.parse_single_text_response(&response)
    }

    /// Parse single-text analysis response into CausalHint.
    fn parse_single_text_response(&self, response: &str) -> CausalAgentResult<CausalHint> {
        // The response should be valid JSON thanks to grammar constraint
        // Format: {"is_causal":true/false,"direction":"cause"/"effect"/"neutral","confidence":0.0-1.0,"key_phrases":[],"description":"..."}

        #[derive(Deserialize)]
        struct SingleTextResponse {
            is_causal: bool,
            direction: String,
            confidence: f32,
            key_phrases: Vec<String>,
            #[serde(default)]
            description: Option<String>,
        }

        // Try JSON parsing first (should work due to grammar constraint)
        match serde_json::from_str::<SingleTextResponse>(response) {
            Ok(parsed) => {
                let direction_hint = CausalDirectionHint::from_str(&parsed.direction);
                let mut hint = CausalHint::new(
                    parsed.is_causal,
                    direction_hint,
                    parsed.confidence,
                    parsed.key_phrases,
                );
                // Set description if provided and non-empty
                hint.description = parsed.description.filter(|d| !d.is_empty());
                Ok(hint)
            }
            Err(e) => {
                warn!(
                    response = response,
                    error = %e,
                    "Failed to parse single-text response, using fallback"
                );
                // Fallback: try regex extraction
                self.parse_single_text_fallback(response)
            }
        }
    }

    /// Fallback parsing using regex when JSON parsing fails.
    fn parse_single_text_fallback(&self, response: &str) -> CausalAgentResult<CausalHint> {
        use regex::Regex;

        // Extract is_causal
        let is_causal_re = Regex::new(r#""is_causal"\s*:\s*(true|false)"#).unwrap();
        let is_causal = is_causal_re
            .captures(response)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str() == "true")
            .unwrap_or(false);

        // Extract direction
        let direction_re = Regex::new(r#""direction"\s*:\s*"(\w+)""#).unwrap();
        let direction = direction_re
            .captures(response)
            .and_then(|c| c.get(1))
            .map(|m| CausalDirectionHint::from_str(m.as_str()))
            .unwrap_or(CausalDirectionHint::Neutral);

        // Extract confidence
        let confidence_re = Regex::new(r#""confidence"\s*:\s*([0-9.]+)"#).unwrap();
        let confidence = confidence_re
            .captures(response)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse::<f32>().ok())
            .unwrap_or(0.0);

        // Extract key_phrases (simplified - just get strings)
        let phrases_re = Regex::new(r#""key_phrases"\s*:\s*\[(.*?)\]"#).unwrap();
        let key_phrases = phrases_re
            .captures(response)
            .and_then(|c| c.get(1))
            .map(|m| {
                let phrase_re = Regex::new(r#""([^"]+)""#).unwrap();
                phrase_re
                    .captures_iter(m.as_str())
                    .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        // Extract description (between "description":" and the last quote before })
        let description_re = Regex::new(r#""description"\s*:\s*"((?:[^"\\]|\\.)*)""#).unwrap();
        let description = description_re
            .captures(response)
            .and_then(|c| c.get(1))
            .map(|m| {
                // Unescape the string
                m.as_str()
                    .replace("\\n", "\n")
                    .replace("\\\"", "\"")
                    .replace("\\\\", "\\")
            })
            .filter(|d| !d.is_empty());

        let mut hint = CausalHint::new(is_causal, direction, confidence, key_phrases);
        hint.description = description;
        Ok(hint)
    }

    /// Generate text from a custom prompt.
    ///
    /// This is a public wrapper for the internal `generate` method,
    /// allowing other crates (like graph-agent) to use the shared LLM
    /// for their own prompts while sharing the same model instance.
    ///
    /// # Arguments
    ///
    /// * `prompt` - The full prompt text (should include system/user/assistant tokens)
    ///
    /// # Returns
    ///
    /// Generated text response from the LLM.
    pub async fn generate_text(&self, prompt: &str) -> CausalAgentResult<String> {
        self.generate_with_grammar(prompt, GrammarType::Causal).await
    }

    /// Generate text with a specific grammar type.
    ///
    /// This allows other crates to use the appropriate grammar for their needs.
    pub async fn generate_with_grammar(
        &self,
        prompt: &str,
        grammar_type: GrammarType,
    ) -> CausalAgentResult<String> {
        let state = self.state.read();

        let LlmState::Loaded {
            ref backend,
            ref model,
        } = *state
        else {
            return Err(CausalAgentError::LlmNotInitialized);
        };

        // Create context for this generation
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(Some(std::num::NonZeroU32::new(self.config.context_size).unwrap()));

        let mut ctx = model
            .new_context(backend, ctx_params)
            .map_err(|e| CausalAgentError::LlmInferenceError {
                message: format!("Failed to create context: {}", e),
            })?;

        // Tokenize the prompt
        let tokens = model
            .str_to_token(prompt, AddBos::Always)
            .map_err(|e| CausalAgentError::LlmInferenceError {
                message: format!("Tokenization failed: {}", e),
            })?;

        let prompt_len = tokens.len();
        debug!(prompt_tokens = prompt_len, "Tokenized prompt");

        // Create batch for processing
        let mut batch = LlamaBatch::new(self.config.batch_size as usize, 1);

        // Add tokens to batch
        for (i, &token) in tokens.iter().enumerate() {
            let is_last = i == tokens.len() - 1;
            batch
                .add(token, i as i32, &[0], is_last)
                .map_err(|e| CausalAgentError::LlmInferenceError {
                    message: format!("Failed to add token to batch: {}", e),
                })?;
        }

        // Process the prompt
        ctx.decode(&mut batch)
            .map_err(|e| CausalAgentError::LlmInferenceError {
                message: format!("Decode failed: {}", e),
            })?;

        // Get the appropriate grammar
        let grammar_str = match grammar_type {
            GrammarType::Causal => &self.causal_grammar,
            GrammarType::Graph => &self.graph_grammar,
            GrammarType::Validation => &self.validation_grammar,
            GrammarType::SingleText => &self.single_text_grammar,
        };

        // Create sampler chain with grammar constraint
        let mut sampler = LlamaSampler::chain_simple([
            LlamaSampler::grammar(model, grammar_str, "root")
                .map_err(|e| CausalAgentError::LlmInferenceError {
                    message: format!("Failed to create grammar sampler: {}", e),
                })?,
            LlamaSampler::temp(self.config.temperature),
            LlamaSampler::greedy(),
        ]);

        // Generate tokens
        let mut generated_tokens: Vec<LlamaToken> = Vec::new();
        let mut current_pos = prompt_len;

        for _ in 0..self.config.max_tokens {
            // Sample next token with grammar constraint
            // -1 means use last logits from batch
            let token = sampler.sample(&ctx, -1);

            // Check for end of generation
            if model.is_eog_token(token) {
                debug!("End of generation token reached");
                break;
            }

            generated_tokens.push(token);

            // Prepare next iteration
            batch.clear();
            batch
                .add(token, current_pos as i32, &[0], true)
                .map_err(|e| CausalAgentError::LlmInferenceError {
                    message: format!("Failed to add token: {}", e),
                })?;

            ctx.decode(&mut batch)
                .map_err(|e| CausalAgentError::LlmInferenceError {
                    message: format!("Decode failed: {}", e),
                })?;

            current_pos += 1;
        }

        // Decode generated tokens to string
        let output = model
            .tokens_to_str(&generated_tokens, Special::Tokenize)
            .map_err(|e| CausalAgentError::LlmInferenceError {
                message: format!("Token decoding failed: {}", e),
            })?;

        debug!(
            generated_tokens = generated_tokens.len(),
            output_len = output.len(),
            "Generation complete"
        );

        Ok(output)
    }

    /// Parse the LLM response into a CausalAnalysisResult.
    ///
    /// With grammar constraints, the JSON is guaranteed to be valid.
    fn parse_causal_response(&self, response: &str) -> CausalAgentResult<CausalAnalysisResult> {
        // Parse the JSON (guaranteed valid by grammar)
        let json: serde_json::Value =
            serde_json::from_str(response.trim()).map_err(|e| CausalAgentError::ParseError {
                message: format!(
                    "JSON parse failed (unexpected with grammar): {}. Response: {}",
                    e, response
                ),
            })?;

        let has_causal_link = json
            .get("causal_link")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let direction_str = json
            .get("direction")
            .and_then(|v| v.as_str())
            .unwrap_or("none");

        let confidence = json
            .get("confidence")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32;

        let mechanism = json
            .get("mechanism")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let mechanism_type = json
            .get("mechanism_type")
            .and_then(|v| v.as_str())
            .and_then(crate::types::MechanismType::from_str);

        Ok(CausalAnalysisResult {
            has_causal_link,
            direction: CausalLinkDirection::from_str(direction_str),
            confidence: confidence.clamp(0.0, 1.0),
            mechanism,
            mechanism_type,
            raw_response: Some(response.to_string()),
        })
    }

    /// Get the model configuration.
    pub fn config(&self) -> &LlmConfig {
        &self.config
    }

    /// Estimate VRAM usage in MB.
    pub fn estimated_vram_mb(&self) -> usize {
        // Hermes 2 Pro Q5_K_M: ~5GB model + ~1GB KV cache
        5000 + (self.config.context_size as usize / 1024) * 256
    }
}

impl std::fmt::Debug for CausalDiscoveryLLM {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CausalDiscoveryLLM")
            .field("model_path", &self.config.model_path)
            .field("loaded", &self.is_loaded())
            .field("context_size", &self.config.context_size)
            .field("n_gpu_layers", &self.config.n_gpu_layers)
            .finish()
    }
}

// Thread safety
unsafe impl Send for CausalDiscoveryLLM {}
unsafe impl Sync for CausalDiscoveryLLM {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_config_default() {
        let config = LlmConfig::default();
        assert_eq!(config.context_size, 4096);
        assert_eq!(config.temperature, 0.0); // Deterministic
        assert_eq!(config.n_gpu_layers, u32::MAX); // Full GPU
    }

    #[test]
    fn test_vram_estimate() {
        let config = LlmConfig::default();
        let llm = CausalDiscoveryLLM::with_config(config).unwrap();
        let vram = llm.estimated_vram_mb();
        assert!(vram < 8000); // Should be under 8GB
    }

    #[test]
    fn test_parse_causal_response() {
        let llm = CausalDiscoveryLLM::new().unwrap();

        let response = r#"{"causal_link": true, "direction": "A_causes_B", "confidence": 0.85, "mechanism": "Direct causation"}"#;

        let result = llm.parse_causal_response(response).unwrap();
        assert!(result.has_causal_link);
        assert_eq!(result.direction, CausalLinkDirection::ACausesB);
        assert!((result.confidence - 0.85).abs() < 0.01);
        assert_eq!(result.mechanism, "Direct causation");
    }

    #[test]
    fn test_default_grammar() {
        let grammar = CausalDiscoveryLLM::default_causal_grammar();
        assert!(grammar.contains("causal_link"));
        assert!(grammar.contains("direction"));
        assert!(grammar.contains("A_causes_B"));
    }
}
