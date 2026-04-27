//! LLM Prompt Variant Experiment Benchmark
//!
//! Tests 5 different system prompt variants for single-text causal analysis
//! against a labeled ground-truth corpus. Measures direction accuracy,
//! confidence calibration, description quality, and latency per variant.
//!
//! ## Usage
//!
//! ```bash
//! cargo run -p context-graph-benchmark --bin causal-prompt-experiment --release \
//!     --features real-embeddings
//! ```

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use context_graph_causal_agent::llm::{CausalDiscoveryLLM, GrammarType, LlmConfig};

// ============================================================================
// PROMPT VARIANTS
// ============================================================================

const VARIANT_NAMES: [&str; 5] = [
    "V1_Baseline",
    "V2_Minimal",
    "V3_Structured_Criteria",
    "V4_FewShot_Rich",
    "V5_Expert_Persona",
];

fn variant_system_prompts() -> [String; 5] {
    [
        // V1: Baseline (current default_single_text_system_prompt)
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

EFFECT_ENTITIES: Array of effect entity spans with character offsets into the original text.
Each entry: {"start": char_offset, "end": char_offset, "label": "entity name"}

If confidence < 0.5, set cause_entities and effect_entities to empty arrays.

DESCRIPTION (CRITICAL - generate when confidence >= 0.5):
Write 1-3 paragraphs explaining the causal relationship.

CONFIDENCE:
- 0.9-1.0: Clear causal language with explicit markers
- 0.7-0.8: Implicit causation, strong language
- 0.5-0.6: Possible causation, weaker indicators
- 0.0-0.4: No clear causal content"#.to_string(),

        // V2: Minimal — stripped to essentials
        r#"Classify text as causal or non-causal. Output JSON.

{"is_causal":true/false,"direction":"cause"/"effect"/"neutral","confidence":0.0-1.0,"key_phrases":[],"description":"","asymmetry_strength":0.0,"cause_entities":[],"effect_entities":[]}

"cause": text describes something causing other things.
"effect": text describes something being caused.
"neutral": no causal content."#.to_string(),

        // V3: Structured decision criteria
        r#"You are a causal text classifier. Follow these EXACT decision steps.

OUTPUT: {"is_causal":true/false,"direction":"cause"/"effect"/"neutral","confidence":0.0-1.0,"key_phrases":[],"description":"...","asymmetry_strength":0.0-1.0,"cause_entities":[],"effect_entities":[]}

STEP 1 — SCAN for causal markers:
  Forward: "causes", "leads to", "results in", "triggers", "produces", "drives"
  Backward: "caused by", "results from", "due to", "driven by", "stems from"
  If NO markers found → is_causal=false, direction="neutral", confidence<0.3

STEP 2 — DETERMINE DIRECTION:
  If text has FORWARD markers (X causes Y) → direction="cause"
  If text has BACKWARD markers (Y caused by X) → direction="effect"
  If BOTH or AMBIGUOUS → direction="neutral"

STEP 3 — CALIBRATE CONFIDENCE:
  Explicit markers ("causes", "leads to"): 0.85-0.95
  Implicit causation ("damages", "triggers"): 0.65-0.80
  Correlational only: 0.3-0.5
  No causal content: 0.0-0.2

STEP 4 — EXTRACT key_phrases (causal markers found in Step 1)
STEP 5 — WRITE description explaining the mechanism (empty if confidence < 0.5)
STEP 6 — SET asymmetry_strength and entity spans"#.to_string(),

        // V4: Few-Shot Rich — many concrete examples
        r#"Classify text for causal content. Output JSON matching this schema:
{"is_causal":true/false,"direction":"cause"/"effect"/"neutral","confidence":0.0-1.0,"key_phrases":[],"description":"...","asymmetry_strength":0.0-1.0,"cause_entities":[],"effect_entities":[]}

EXAMPLES:

Input: "Smoking causes lung cancer through DNA damage in bronchial cells"
→ {"is_causal":true,"direction":"cause","confidence":0.95,"key_phrases":["causes"],"description":"Smoking introduces carcinogens that damage DNA in bronchial epithelial cells, leading to uncontrolled cell growth.","asymmetry_strength":0.9,"cause_entities":[{"start":0,"end":7,"label":"smoking"}],"effect_entities":[{"start":15,"end":26,"label":"lung cancer"}]}

Input: "Heart attacks are caused by coronary artery blockage"
→ {"is_causal":true,"direction":"effect","confidence":0.90,"key_phrases":["caused by"],"description":"Coronary artery blockage restricts blood flow to the heart, causing myocardial infarction.","asymmetry_strength":0.85,"cause_entities":[{"start":31,"end":55,"label":"coronary artery blockage"}],"effect_entities":[{"start":0,"end":13,"label":"heart attacks"}]}

Input: "Python is a programming language created in 1991"
→ {"is_causal":false,"direction":"neutral","confidence":0.05,"key_phrases":[],"description":"","asymmetry_strength":0.0,"cause_entities":[],"effect_entities":[]}

Input: "Deforestation leads to soil erosion and flooding"
→ {"is_causal":true,"direction":"cause","confidence":0.90,"key_phrases":["leads to"],"description":"Removing tree cover exposes soil to rain, increasing runoff and erosion.","asymmetry_strength":0.85,"cause_entities":[{"start":0,"end":13,"label":"deforestation"}],"effect_entities":[{"start":23,"end":35,"label":"soil erosion"}]}

Input: "The recession resulted from aggressive monetary tightening"
→ {"is_causal":true,"direction":"effect","confidence":0.88,"key_phrases":["resulted from"],"description":"Aggressive interest rate hikes reduced economic activity, triggering recession.","asymmetry_strength":0.8,"cause_entities":[{"start":29,"end":58,"label":"aggressive monetary tightening"}],"effect_entities":[{"start":4,"end":13,"label":"recession"}]}

Input: "The conference is held annually in Berlin"
→ {"is_causal":false,"direction":"neutral","confidence":0.02,"key_phrases":[],"description":"","asymmetry_strength":0.0,"cause_entities":[],"effect_entities":[]}"#.to_string(),

        // V5: Expert Persona with methodology
        r#"You are a senior causal inference researcher specializing in natural language processing. You have published extensively on detecting causal relations in text using both linguistic markers and semantic understanding.

TASK: Analyze the given text using your expertise in causal inference methodology.

OUTPUT: {"is_causal":true/false,"direction":"cause"/"effect"/"neutral","confidence":0.0-1.0,"key_phrases":[],"description":"...","asymmetry_strength":0.0-1.0,"cause_entities":[],"effect_entities":[]}

YOUR METHODOLOGY:
1. LINGUISTIC ANALYSIS: Identify explicit causal connectives (because, causes, leads to, results in, due to, triggered by). These are strongest evidence.
2. SEMANTIC ANALYSIS: Look for implicit causation through action verbs (damages, destroys, improves, reduces) that imply one entity acting on another.
3. DIRECTION ASSESSMENT: Determine if text frames the subject as AGENT (cause direction) or PATIENT (effect direction). Active voice with causal verbs = cause. Passive voice with "by" = effect.
4. CONFIDENCE SCORING: Base confidence on linguistic evidence strength. Explicit markers = high confidence. Implicit semantics = moderate. No evidence = low.
5. DESCRIPTION: When causal, explain the MECHANISM — what specific process connects cause to effect. Name concrete pathways, not vague summaries.

CRITICAL: Distinguish correlation from causation. Temporal co-occurrence is NOT causation. Only classify as causal if the text explicitly or strongly implies a generative mechanism."#.to_string(),
    ]
}

// ============================================================================
// GROUND TRUTH CORPUS
// ============================================================================

#[derive(Debug, Clone)]
struct TestCase {
    text: &'static str,
    expected_direction: &'static str, // "cause", "effect", "neutral"
    expected_causal: bool,
    #[allow(dead_code)]
    domain: &'static str,
}

fn test_corpus() -> Vec<TestCase> {
    vec![
        // === CAUSE direction (text describes something CAUSING other things) ===
        TestCase {
            text: "Chronic stress elevates cortisol levels in the bloodstream",
            expected_direction: "cause",
            expected_causal: true,
            domain: "biomedical",
        },
        TestCase {
            text: "Deforestation causes soil erosion and downstream flooding",
            expected_direction: "cause",
            expected_causal: true,
            domain: "environmental",
        },
        TestCase {
            text: "High interest rates drive housing prices downward",
            expected_direction: "cause",
            expected_causal: true,
            domain: "economics",
        },
        TestCase {
            text: "Smoking causes lung cancer through DNA damage mechanisms",
            expected_direction: "cause",
            expected_causal: true,
            domain: "health",
        },
        TestCase {
            text: "Poverty leads to poor educational outcomes for children",
            expected_direction: "cause",
            expected_causal: true,
            domain: "sociology",
        },
        TestCase {
            text: "Heat causes thermal expansion in metal structures",
            expected_direction: "cause",
            expected_causal: true,
            domain: "physics",
        },
        TestCase {
            text: "SQL injection vulnerabilities lead to data breaches",
            expected_direction: "cause",
            expected_causal: true,
            domain: "cybersecurity",
        },
        TestCase {
            text: "Memory leaks cause application crashes over time",
            expected_direction: "cause",
            expected_causal: true,
            domain: "software",
        },
        TestCase {
            text: "UV radiation triggers DNA mutations in skin cells",
            expected_direction: "cause",
            expected_causal: true,
            domain: "biology",
        },
        TestCase {
            text: "Inflation erodes purchasing power of consumers",
            expected_direction: "cause",
            expected_causal: true,
            domain: "economics",
        },
        // === EFFECT direction (text describes something BEING CAUSED) ===
        TestCase {
            text: "Memory impairment results from chronic stress exposure",
            expected_direction: "effect",
            expected_causal: true,
            domain: "biomedical",
        },
        TestCase {
            text: "Downstream flooding is caused by deforestation upstream",
            expected_direction: "effect",
            expected_causal: true,
            domain: "environmental",
        },
        TestCase {
            text: "The housing crash was driven by speculative lending",
            expected_direction: "effect",
            expected_causal: true,
            domain: "economics",
        },
        TestCase {
            text: "Lung cancer is primarily caused by tobacco smoking",
            expected_direction: "effect",
            expected_causal: true,
            domain: "health",
        },
        TestCase {
            text: "Poor educational outcomes are driven by poverty",
            expected_direction: "effect",
            expected_causal: true,
            domain: "sociology",
        },
        TestCase {
            text: "Bridge expansion resulted from extreme heat waves",
            expected_direction: "effect",
            expected_causal: true,
            domain: "physics",
        },
        TestCase {
            text: "The data breach resulted from SQL injection attacks",
            expected_direction: "effect",
            expected_causal: true,
            domain: "cybersecurity",
        },
        TestCase {
            text: "Application crashes are due to memory management bugs",
            expected_direction: "effect",
            expected_causal: true,
            domain: "software",
        },
        TestCase {
            text: "Skin cancer is triggered by prolonged UV exposure",
            expected_direction: "effect",
            expected_causal: true,
            domain: "biology",
        },
        TestCase {
            text: "Purchasing power decline stems from persistent inflation",
            expected_direction: "effect",
            expected_causal: true,
            domain: "economics",
        },
        // === NEUTRAL direction (no causal content) ===
        TestCase {
            text: "The library has over two million books in its collection",
            expected_direction: "neutral",
            expected_causal: false,
            domain: "factual",
        },
        TestCase {
            text: "Python is an interpreted programming language",
            expected_direction: "neutral",
            expected_causal: false,
            domain: "technical",
        },
        TestCase {
            text: "The annual conference takes place in March",
            expected_direction: "neutral",
            expected_causal: false,
            domain: "event",
        },
        TestCase {
            text: "The REST API returns JSON formatted responses",
            expected_direction: "neutral",
            expected_causal: false,
            domain: "technical",
        },
        TestCase {
            text: "Water boils at 100 degrees Celsius at sea level",
            expected_direction: "neutral",
            expected_causal: false,
            domain: "factual",
        },
        TestCase {
            text: "The development team consists of fifteen engineers",
            expected_direction: "neutral",
            expected_causal: false,
            domain: "descriptive",
        },
        TestCase {
            text: "SSH uses port 22 by default for connections",
            expected_direction: "neutral",
            expected_causal: false,
            domain: "technical",
        },
        TestCase {
            text: "The Earth completes one orbit around the Sun per year",
            expected_direction: "neutral",
            expected_causal: false,
            domain: "factual",
        },
        TestCase {
            text: "JavaScript is widely used for web development",
            expected_direction: "neutral",
            expected_causal: false,
            domain: "technical",
        },
        TestCase {
            text: "The quarterly meeting is scheduled for three o'clock",
            expected_direction: "neutral",
            expected_causal: false,
            domain: "event",
        },
    ]
}

// ============================================================================
// RESULT STRUCTURES
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExperimentResults {
    timestamp: String,
    model: String,
    corpus_size: usize,
    variants: Vec<VariantResult>,
    comparative_analysis: ComparativeAnalysis,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VariantResult {
    name: String,
    prompt_length_chars: usize,
    // Accuracy metrics
    direction_accuracy: f64,
    cause_accuracy: f64,
    effect_accuracy: f64,
    neutral_accuracy: f64,
    is_causal_accuracy: f64,
    // Confidence metrics
    avg_confidence_causal: f64,
    avg_confidence_neutral: f64,
    confidence_separation: f64, // gap between causal and neutral confidence
    // Quality metrics
    avg_description_length: f64,
    descriptions_nonempty_rate: f64,
    avg_key_phrases: f64,
    // Latency
    avg_latency_ms: f64,
    p95_latency_ms: f64,
    total_tokens: u32,
    // Per-case details
    per_case: Vec<CaseResult>,
    // Error tracking
    parse_failures: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CaseResult {
    text_preview: String,
    expected_direction: String,
    predicted_direction: String,
    correct: bool,
    confidence: f32,
    is_causal: bool,
    description_len: usize,
    key_phrases: Vec<String>,
    latency_ms: f64,
    tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ComparativeAnalysis {
    best_overall: String,
    best_cause: String,
    best_effect: String,
    best_neutral: String,
    best_confidence_separation: String,
    fastest: String,
    most_descriptive: String,
    ranking: Vec<RankEntry>,
    insights: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RankEntry {
    rank: usize,
    name: String,
    direction_accuracy: f64,
    confidence_separation: f64,
    avg_latency_ms: f64,
    composite_score: f64,
}

// ============================================================================
// LLM RESPONSE PARSING
// ============================================================================

#[derive(Deserialize)]
struct SingleTextResponse {
    is_causal: bool,
    direction: String,
    confidence: f32,
    #[serde(default)]
    key_phrases: Vec<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    asymmetry_strength: Option<f32>,
}

// ============================================================================
// MAIN
// ============================================================================

#[tokio::main]
async fn main() {
    let output_path = PathBuf::from("benchmark_results/causal_prompt_experiment");
    fs::create_dir_all(&output_path).expect("Failed to create output dir");

    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║     LLM PROMPT VARIANT EXPERIMENT — 5 Variants × 30 Cases   ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");

    // Phase 1: Load LLM
    println!("Phase 1: Loading Hermes 2 Pro Mistral 7B...");
    let load_start = Instant::now();
    let llm =
        CausalDiscoveryLLM::with_config(LlmConfig::default()).expect("Failed to create LLM config");
    llm.load().await.expect("Failed to load LLM into GPU");
    let load_time = load_start.elapsed().as_secs_f64();
    println!("  Loaded in {:.2}s\n", load_time);

    // Phase 2: Prepare corpus and prompts
    let corpus = test_corpus();
    let prompts = variant_system_prompts();
    println!(
        "Phase 2: {} test cases × {} variants = {} LLM calls\n",
        corpus.len(),
        VARIANT_NAMES.len(),
        corpus.len() * VARIANT_NAMES.len()
    );

    // Phase 3: Run each variant
    let mut variant_results: Vec<VariantResult> = Vec::new();

    for (vi, (variant_name, system_prompt)) in VARIANT_NAMES.iter().zip(prompts.iter()).enumerate()
    {
        println!(
            "═══ Variant {}: {} ({} chars) ═══",
            vi + 1,
            variant_name,
            system_prompt.len()
        );

        let mut case_results: Vec<CaseResult> = Vec::new();
        let mut parse_failures = 0u32;

        for (ci, case) in corpus.iter().enumerate() {
            // Build ChatML prompt (same format as build_single_text_prompt)
            let truncated = if case.text.len() > 1500 {
                &case.text[..1500]
            } else {
                case.text
            };
            let prompt = format!(
                "<|im_start|>system\n{}\n<|im_end|>\n<|im_start|>user\nText: \"{}\"\n<|im_end|>\n<|im_start|>assistant\n{{\"is_causal\":",
                system_prompt, truncated
            );

            let start = Instant::now();
            let gen_result = llm
                .generate_with_grammar_and_metadata(&prompt, GrammarType::SingleText)
                .await;
            let latency_ms = start.elapsed().as_secs_f64() * 1000.0;

            match gen_result {
                Ok((response, tokens, _time_ms)) => {
                    match serde_json::from_str::<SingleTextResponse>(&response) {
                        Ok(parsed) => {
                            let correct = parsed.direction == case.expected_direction;
                            let desc_len =
                                parsed.description.as_ref().map(|d| d.len()).unwrap_or(0);

                            case_results.push(CaseResult {
                                text_preview: case.text.chars().take(60).collect(),
                                expected_direction: case.expected_direction.to_string(),
                                predicted_direction: parsed.direction.clone(),
                                correct,
                                confidence: parsed.confidence,
                                is_causal: parsed.is_causal,
                                description_len: desc_len,
                                key_phrases: parsed.key_phrases,
                                latency_ms,
                                tokens,
                            });
                        }
                        Err(_e) => {
                            parse_failures += 1;
                            case_results.push(CaseResult {
                                text_preview: case.text.chars().take(60).collect(),
                                expected_direction: case.expected_direction.to_string(),
                                predicted_direction: "PARSE_FAIL".to_string(),
                                correct: false,
                                confidence: 0.0,
                                is_causal: false,
                                description_len: 0,
                                key_phrases: vec![],
                                latency_ms,
                                tokens,
                            });
                        }
                    }
                }
                Err(e) => {
                    parse_failures += 1;
                    case_results.push(CaseResult {
                        text_preview: case.text.chars().take(60).collect(),
                        expected_direction: case.expected_direction.to_string(),
                        predicted_direction: format!("ERROR: {}", e),
                        correct: false,
                        confidence: 0.0,
                        is_causal: false,
                        description_len: 0,
                        key_phrases: vec![],
                        latency_ms: 0.0,
                        tokens: 0,
                    });
                }
            }

            if (ci + 1) % 10 == 0 || ci == corpus.len() - 1 {
                print!("\r  Processed {}/{}", ci + 1, corpus.len());
                std::io::stdout().flush().ok();
            }
        }
        println!();

        // Compute variant-level metrics
        let vr = compute_variant_metrics(
            variant_name,
            system_prompt,
            &case_results,
            &corpus,
            parse_failures,
        );
        println!(
            "  Direction accuracy: {:.1}%  |  Cause: {:.1}%  Effect: {:.1}%  Neutral: {:.1}%",
            vr.direction_accuracy * 100.0,
            vr.cause_accuracy * 100.0,
            vr.effect_accuracy * 100.0,
            vr.neutral_accuracy * 100.0
        );
        println!(
            "  Confidence sep: {:.3}  |  Avg latency: {:.0}ms  |  Desc rate: {:.0}%",
            vr.confidence_separation,
            vr.avg_latency_ms,
            vr.descriptions_nonempty_rate * 100.0
        );
        println!();

        variant_results.push(vr);
    }

    // Phase 4: Comparative analysis
    println!("═══ COMPARATIVE ANALYSIS ═══════════════════════════════════════");
    let analysis = build_comparative_analysis(&variant_results);
    print_comparative_table(&variant_results, &analysis);

    // Phase 5: Save results
    let results = ExperimentResults {
        timestamp: Utc::now().to_rfc3339(),
        model: "Hermes-2-Pro-Mistral-7B.Q5_K_M".to_string(),
        corpus_size: corpus.len(),
        variants: variant_results,
        comparative_analysis: analysis,
    };

    let json_path = output_path.join("results.json");
    let json = serde_json::to_string_pretty(&results).unwrap();
    fs::write(&json_path, &json).expect("Failed to write results");
    println!("\nResults saved to: {}", json_path.display());
}

// ============================================================================
// METRICS COMPUTATION
// ============================================================================

fn compute_variant_metrics(
    name: &str,
    system_prompt: &str,
    cases: &[CaseResult],
    corpus: &[TestCase],
    parse_failures: u32,
) -> VariantResult {
    let total = cases.len() as f64;
    let correct_dir: f64 = cases.iter().filter(|c| c.correct).count() as f64;

    // Per-class accuracy
    let (cause_correct, cause_total) = class_accuracy(cases, corpus, "cause");
    let (effect_correct, effect_total) = class_accuracy(cases, corpus, "effect");
    let (neutral_correct, neutral_total) = class_accuracy(cases, corpus, "neutral");

    // is_causal accuracy
    let is_causal_correct: f64 = cases
        .iter()
        .zip(corpus.iter())
        .filter(|(c, tc)| c.is_causal == tc.expected_causal)
        .count() as f64;

    // Confidence metrics
    let causal_confs: Vec<f32> = cases
        .iter()
        .zip(corpus.iter())
        .filter(|(_, tc)| tc.expected_causal)
        .map(|(c, _)| c.confidence)
        .collect();
    let neutral_confs: Vec<f32> = cases
        .iter()
        .zip(corpus.iter())
        .filter(|(_, tc)| !tc.expected_causal)
        .map(|(c, _)| c.confidence)
        .collect();
    let avg_conf_causal = if causal_confs.is_empty() {
        0.0
    } else {
        causal_confs.iter().sum::<f32>() as f64 / causal_confs.len() as f64
    };
    let avg_conf_neutral = if neutral_confs.is_empty() {
        0.0
    } else {
        neutral_confs.iter().sum::<f32>() as f64 / neutral_confs.len() as f64
    };

    // Quality metrics
    let desc_lens: Vec<usize> = cases.iter().map(|c| c.description_len).collect();
    let avg_desc_len = desc_lens.iter().sum::<usize>() as f64 / total;
    let nonempty_rate = cases.iter().filter(|c| c.description_len > 0).count() as f64 / total;
    let avg_kp = cases.iter().map(|c| c.key_phrases.len()).sum::<usize>() as f64 / total;

    // Latency
    let latencies: Vec<f64> = cases.iter().map(|c| c.latency_ms).collect();
    let avg_lat = latencies.iter().sum::<f64>() / total;
    let mut sorted_lat = latencies.clone();
    sorted_lat.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p95_lat = sorted_lat[((total * 0.95) as usize).min(sorted_lat.len() - 1)];

    let total_tokens: u32 = cases.iter().map(|c| c.tokens).sum();

    VariantResult {
        name: name.to_string(),
        prompt_length_chars: system_prompt.len(),
        direction_accuracy: correct_dir / total,
        cause_accuracy: if cause_total > 0 {
            cause_correct as f64 / cause_total as f64
        } else {
            0.0
        },
        effect_accuracy: if effect_total > 0 {
            effect_correct as f64 / effect_total as f64
        } else {
            0.0
        },
        neutral_accuracy: if neutral_total > 0 {
            neutral_correct as f64 / neutral_total as f64
        } else {
            0.0
        },
        is_causal_accuracy: is_causal_correct / total,
        avg_confidence_causal: avg_conf_causal,
        avg_confidence_neutral: avg_conf_neutral,
        confidence_separation: avg_conf_causal - avg_conf_neutral,
        avg_description_length: avg_desc_len,
        descriptions_nonempty_rate: nonempty_rate,
        avg_key_phrases: avg_kp,
        avg_latency_ms: avg_lat,
        p95_latency_ms: p95_lat,
        total_tokens,
        per_case: cases.to_vec(),
        parse_failures: parse_failures as usize,
    }
}

fn class_accuracy(cases: &[CaseResult], corpus: &[TestCase], class: &str) -> (usize, usize) {
    let mut correct = 0;
    let mut total = 0;
    for (c, tc) in cases.iter().zip(corpus.iter()) {
        if tc.expected_direction == class {
            total += 1;
            if c.correct {
                correct += 1;
            }
        }
    }
    (correct, total)
}

// ============================================================================
// COMPARATIVE ANALYSIS
// ============================================================================

fn build_comparative_analysis(variants: &[VariantResult]) -> ComparativeAnalysis {
    let best_overall = variants
        .iter()
        .max_by(|a, b| {
            a.direction_accuracy
                .partial_cmp(&b.direction_accuracy)
                .unwrap()
        })
        .map(|v| v.name.clone())
        .unwrap_or_default();
    let best_cause = variants
        .iter()
        .max_by(|a, b| a.cause_accuracy.partial_cmp(&b.cause_accuracy).unwrap())
        .map(|v| v.name.clone())
        .unwrap_or_default();
    let best_effect = variants
        .iter()
        .max_by(|a, b| a.effect_accuracy.partial_cmp(&b.effect_accuracy).unwrap())
        .map(|v| v.name.clone())
        .unwrap_or_default();
    let best_neutral = variants
        .iter()
        .max_by(|a, b| a.neutral_accuracy.partial_cmp(&b.neutral_accuracy).unwrap())
        .map(|v| v.name.clone())
        .unwrap_or_default();
    let best_conf = variants
        .iter()
        .max_by(|a, b| {
            a.confidence_separation
                .partial_cmp(&b.confidence_separation)
                .unwrap()
        })
        .map(|v| v.name.clone())
        .unwrap_or_default();
    let fastest = variants
        .iter()
        .min_by(|a, b| a.avg_latency_ms.partial_cmp(&b.avg_latency_ms).unwrap())
        .map(|v| v.name.clone())
        .unwrap_or_default();
    let most_desc = variants
        .iter()
        .max_by(|a, b| {
            a.avg_description_length
                .partial_cmp(&b.avg_description_length)
                .unwrap()
        })
        .map(|v| v.name.clone())
        .unwrap_or_default();

    // Composite scoring: 50% direction accuracy + 25% confidence separation + 25% latency efficiency
    let max_lat = variants
        .iter()
        .map(|v| v.avg_latency_ms)
        .fold(0.0f64, f64::max);
    let mut ranking: Vec<RankEntry> = variants
        .iter()
        .map(|v| {
            let lat_score = if max_lat > 0.0 {
                1.0 - (v.avg_latency_ms / max_lat)
            } else {
                1.0
            };
            let conf_score = v.confidence_separation.max(0.0).min(1.0);
            let composite = 0.50 * v.direction_accuracy + 0.25 * conf_score + 0.25 * lat_score;
            RankEntry {
                rank: 0,
                name: v.name.clone(),
                direction_accuracy: v.direction_accuracy,
                confidence_separation: v.confidence_separation,
                avg_latency_ms: v.avg_latency_ms,
                composite_score: composite,
            }
        })
        .collect();
    ranking.sort_by(|a, b| b.composite_score.partial_cmp(&a.composite_score).unwrap());
    for (i, r) in ranking.iter_mut().enumerate() {
        r.rank = i + 1;
    }

    // Generate insights
    let mut insights = Vec::new();
    if let Some(best) = ranking.first() {
        insights.push(format!(
            "Best overall: {} (composite={:.3})",
            best.name, best.composite_score
        ));
    }
    if let Some(worst) = ranking.last() {
        insights.push(format!(
            "Worst overall: {} (composite={:.3})",
            worst.name, worst.composite_score
        ));
    }

    let min_prompt = variants
        .iter()
        .min_by_key(|v| v.prompt_length_chars)
        .unwrap();
    let max_prompt = variants
        .iter()
        .max_by_key(|v| v.prompt_length_chars)
        .unwrap();
    insights.push(format!(
        "Prompt length range: {} ({}) to {} ({}) chars",
        min_prompt.prompt_length_chars,
        min_prompt.name,
        max_prompt.prompt_length_chars,
        max_prompt.name
    ));

    if best_cause != best_effect {
        insights.push(format!(
            "Cause vs Effect best differ: cause={}, effect={}",
            best_cause, best_effect
        ));
    }

    let avg_parse_fail: f64 = variants
        .iter()
        .map(|v| v.parse_failures as f64)
        .sum::<f64>()
        / variants.len() as f64;
    if avg_parse_fail > 0.0 {
        insights.push(format!(
            "Average parse failures per variant: {:.1}",
            avg_parse_fail
        ));
    } else {
        insights
            .push("Zero parse failures across all variants (GBNF grammar reliable)".to_string());
    }

    ComparativeAnalysis {
        best_overall,
        best_cause,
        best_effect,
        best_neutral,
        best_confidence_separation: best_conf,
        fastest,
        most_descriptive: most_desc,
        ranking,
        insights,
    }
}

fn print_comparative_table(variants: &[VariantResult], analysis: &ComparativeAnalysis) {
    println!(
        "\n{:<25} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>9}",
        "Variant", "Dir%", "Cause%", "Effect%", "Neut%", "ConfSep", "Lat(ms)", "Composite"
    );
    println!("{}", "-".repeat(95));

    for r in &analysis.ranking {
        let v = variants.iter().find(|v| v.name == r.name).unwrap();
        println!(
            "{:<25} {:>7.1} {:>7.1} {:>7.1} {:>7.1} {:>8.3} {:>8.0} {:>9.3}",
            v.name,
            v.direction_accuracy * 100.0,
            v.cause_accuracy * 100.0,
            v.effect_accuracy * 100.0,
            v.neutral_accuracy * 100.0,
            v.confidence_separation,
            v.avg_latency_ms,
            r.composite_score
        );
    }
    println!();

    println!("WINNERS:");
    println!("  Overall Direction: {}", analysis.best_overall);
    println!("  Cause Detection:   {}", analysis.best_cause);
    println!("  Effect Detection:  {}", analysis.best_effect);
    println!("  Neutral Rejection: {}", analysis.best_neutral);
    println!(
        "  Confidence Gap:    {}",
        analysis.best_confidence_separation
    );
    println!("  Fastest:           {}", analysis.fastest);
    println!("  Most Descriptive:  {}", analysis.most_descriptive);
    println!();

    println!("INSIGHTS:");
    for insight in &analysis.insights {
        println!("  - {}", insight);
    }
}
