//! Real LLM Causal Provenance Benchmark
//!
//! Tests the actual CausalDiscoveryLLM with GBNF grammar for source span extraction.
//! Requires real-embeddings feature and CUDA GPU.
//!
//! ## Usage
//!
//! ```bash
//! cargo run -p context-graph-benchmark --bin causal-provenance-llm-bench --release \
//!     --features real-embeddings -- --num-documents 10
//! ```

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use context_graph_causal_agent::{CausalDiscoveryLLM, LlmConfig};

// ============================================================================
// CLI Arguments
// ============================================================================

#[derive(Debug, Clone)]
struct Args {
    output_path: PathBuf,
    num_documents: usize,
    min_confidence: f32,
    verbose: bool,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            output_path: PathBuf::from("benchmark_results/causal_provenance_llm_bench.json"),
            num_documents: 10,
            min_confidence: 0.6,
            verbose: false,
        }
    }
}

fn parse_args() -> Args {
    let mut args = Args::default();
    let mut argv = std::env::args().skip(1);

    while let Some(arg) = argv.next() {
        match arg.as_str() {
            "--output" | "-o" => {
                args.output_path = PathBuf::from(argv.next().expect("--output requires a value"));
            }
            "--num-documents" | "-n" => {
                args.num_documents = argv
                    .next()
                    .expect("--num-documents requires a value")
                    .parse()
                    .expect("--num-documents must be a number");
            }
            "--min-confidence" => {
                args.min_confidence = argv
                    .next()
                    .expect("--min-confidence requires a value")
                    .parse()
                    .expect("--min-confidence must be a number");
            }
            "--verbose" | "-v" => {
                args.verbose = true;
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            _ => {
                eprintln!("Unknown argument: {}", arg);
                print_usage();
                std::process::exit(1);
            }
        }
    }

    args
}

fn print_usage() {
    eprintln!(
        r#"
Real LLM Causal Provenance Benchmark

USAGE:
    causal-provenance-llm-bench [OPTIONS]

OPTIONS:
    --output, -o <PATH>         Output path for results JSON
    --num-documents, -n <NUM>   Number of documents to test (default: 10)
    --min-confidence <NUM>      Minimum confidence threshold (default: 0.6)
    --verbose, -v               Verbose output
    --help, -h                  Show this help message

NOTE:
    This benchmark requires --features real-embeddings and a CUDA GPU.

EXAMPLE:
    cargo run -p context-graph-benchmark --bin causal-provenance-llm-bench --release \
        --features real-embeddings -- --num-documents 10
"#
    );
}

// ============================================================================
// Result Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmProvenanceBenchResults {
    pub timestamp: String,
    pub config: BenchmarkConfig,
    pub llm_info: LlmInfo,
    pub extraction_metrics: ExtractionMetrics,
    pub span_accuracy: SpanAccuracyMetrics,
    pub latency: LatencyMetrics,
    pub targets: PerformanceTargets,
    pub per_document_results: Vec<DocumentResult>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    pub num_documents: usize,
    pub min_confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmInfo {
    pub model_name: String,
    pub model_load_time_sec: f64,
    pub grammar_type: String,
    pub vram_usage_mb: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionMetrics {
    pub total_documents: usize,
    pub documents_with_relationships: usize,
    pub total_relationships: usize,
    pub avg_relationships_per_doc: f64,
    pub avg_confidence: f64,
    pub mechanism_distribution: HashMap<String, usize>,
    pub json_parse_success_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanAccuracyMetrics {
    pub total_spans: usize,
    pub spans_populated_rate: f64,
    pub offset_valid_rate: f64,
    pub text_match_rate: f64,
    pub avg_span_length: f64,
    pub span_type_distribution: HashMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyMetrics {
    pub avg_extraction_ms: f64,
    pub min_extraction_ms: f64,
    pub max_extraction_ms: f64,
    pub p95_extraction_ms: f64,
    pub total_time_sec: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceTargets {
    pub span_populated_met: bool,
    pub offset_valid_met: bool,
    pub text_match_met: bool,
    pub json_parse_met: bool,
    pub all_critical_passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentResult {
    pub doc_id: usize,
    pub file_path: String,
    pub content_length: usize,
    pub relationships_found: usize,
    pub spans_populated: usize,
    pub spans_valid: usize,
    pub spans_text_match: usize,
    pub extraction_time_ms: f64,
    pub json_parse_success: bool,
    pub error: Option<String>,
}

// ============================================================================
// Test Corpus
// ============================================================================

const TEST_CORPUS: &[(&str, &str)] = &[
    ("neuroscience/stress.md",
     "Chronic psychological stress leads to persistently elevated cortisol levels in the bloodstream. \
      The hippocampus, critical for memory formation, contains high density of cortisol receptors. \
      When cortisol levels remain elevated, it causes progressive damage to hippocampal neurons. \
      This neuronal damage results in impaired memory formation and difficulty consolidating new memories."),

    ("pharmacology/ssri.md",
     "Selective serotonin reuptake inhibitors work by blocking serotonin reabsorption in neurons. \
      This blockade increases serotonin availability in synaptic clefts. \
      Elevated serotonin enhances neurotransmission in mood-regulating circuits. \
      Over weeks, improved neurotransmission leads to reduced depression symptoms."),

    ("immunology/vaccines.md",
     "mRNA vaccines deliver genetic instructions encoding viral spike proteins to cells. \
      Ribosomes translate the mRNA and produce spike proteins on cell surfaces. \
      The immune system recognizes these proteins as foreign antigens. \
      Recognition triggers antibody production and provides protection against infection."),

    ("cardiology/hypertension.md",
     "Chronic hypertension subjects arterial walls to sustained mechanical stress. \
      This stress causes endothelial dysfunction and vessel damage. \
      Damaged endothelium promotes atherosclerotic plaque formation. \
      Plaque rupture leads to heart attacks or strokes."),

    ("metabolism/diabetes.md",
     "Insulin resistance causes cells to respond poorly to insulin signaling. \
      The pancreas compensates by producing more insulin, causing hyperinsulinemia. \
      Prolonged hyperinsulinemia exhausts pancreatic beta cells. \
      Beta cell exhaustion results in type 2 diabetes."),

    ("oncology/cancer.md",
     "UV radiation causes DNA damage in skin cells. \
      Damage to tumor suppressor genes like p53 removes growth controls. \
      Without functional suppressors, cells proliferate uncontrollably. \
      Uncontrolled division leads to tumor formation."),

    ("neurology/parkinsons.md",
     "In Parkinson's disease, dopaminergic neurons progressively degenerate. \
      Neuronal loss reduces dopamine production in the basal ganglia. \
      Decreased dopamine disrupts motor control circuits. \
      Disrupted circuits cause tremors and rigidity."),

    ("infectious/antibiotics.md",
     "Antibiotic overuse creates selective pressure on bacteria. \
      Resistant bacteria survive while susceptible ones die. \
      Survivors reproduce and pass resistance genes to offspring. \
      Selection produces multi-drug resistant strains."),

    ("technical/api.md",  // Control - no causal content
     "The API accepts JSON payloads with id and name fields. \
      Authentication requires a Bearer token. \
      Rate limiting is 100 requests per minute. \
      Responses follow JSON:API specification."),

    ("history/timeline.md",  // Control - no causal content
     "The library was founded in 1923. \
      Collections include 2 million books. \
      Hours are Monday to Friday 9am-9pm. \
      Membership is free for residents."),
];

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() {
    let args = parse_args();

    println!("╔═══════════════════════════════════════════════════════════════════════════════╗");
    println!("║       REAL LLM CAUSAL PROVENANCE BENCHMARK                                     ║");
    println!("╠═══════════════════════════════════════════════════════════════════════════════╣");
    println!("║ Tests actual Hermes 2 Pro LLM with GBNF grammar for source span extraction    ║");
    println!("╚═══════════════════════════════════════════════════════════════════════════════╝");
    println!();

    let benchmark_start = Instant::now();

    // Initialize LLM
    println!("═══ Phase 1: Loading LLM ══════════════════════════════════════════════════════");
    let load_start = Instant::now();

    let llm_config = LlmConfig::default();
    let llm = match CausalDiscoveryLLM::with_config(llm_config) {
        Ok(llm) => {
            println!("  LLM config created");
            llm
        }
        Err(e) => {
            eprintln!("  Failed to create LLM config: {}", e);
            eprintln!("  Ensure Hermes 2 Pro model is available and CUDA is working.");
            std::process::exit(1);
        }
    };

    // Load the model into GPU memory
    if let Err(e) = llm.load().await {
        eprintln!("  Failed to load LLM into GPU: {}", e);
        eprintln!("  Ensure CUDA is available and model file exists.");
        std::process::exit(1);
    }
    println!("  LLM loaded into GPU successfully");

    let load_time = load_start.elapsed().as_secs_f64();
    println!("  Load time: {:.2}s", load_time);
    println!();

    // Run extraction on test corpus
    println!("═══ Phase 2: Running LLM Extraction ═══════════════════════════════════════════");

    let num_docs = args.num_documents.min(TEST_CORPUS.len());
    let test_docs: Vec<_> = TEST_CORPUS.iter().take(num_docs).collect();

    let mut per_doc_results: Vec<DocumentResult> = Vec::new();
    let mut extraction_times: Vec<f64> = Vec::new();
    let mut total_relationships = 0;
    let mut docs_with_relationships = 0;
    let mut total_confidence = 0.0;
    let mut total_spans = 0;
    let mut valid_offset_spans = 0;
    let mut text_match_spans = 0;
    let mut total_span_length = 0usize;
    let mut mechanism_counts: HashMap<String, usize> = HashMap::new();
    let mut span_type_counts: HashMap<String, usize> = HashMap::new();
    let mut json_parse_successes = 0;

    for (doc_idx, (file_path, content)) in test_docs.iter().enumerate() {
        let extract_start = Instant::now();

        if args.verbose {
            println!("\n  Processing: {} ({} chars)", file_path, content.len());
        }

        // Call the real LLM extraction
        let result = llm.extract_causal_relationships(content).await;

        let extraction_time = extract_start.elapsed().as_secs_f64() * 1000.0;
        extraction_times.push(extraction_time);

        let mut doc_result = DocumentResult {
            doc_id: doc_idx,
            file_path: file_path.to_string(),
            content_length: content.len(),
            relationships_found: 0,
            spans_populated: 0,
            spans_valid: 0,
            spans_text_match: 0,
            extraction_time_ms: extraction_time,
            json_parse_success: false,
            error: None,
        };

        match result {
            Ok(multi_result) => {
                doc_result.json_parse_success = true;
                json_parse_successes += 1;

                if !multi_result.relationships.is_empty() {
                    docs_with_relationships += 1;
                }

                doc_result.relationships_found = multi_result.relationships.len();

                for rel in &multi_result.relationships {
                    total_relationships += 1;
                    total_confidence += rel.confidence as f64;

                    // Track mechanism types
                    let mech_str = rel.mechanism_type.as_str().to_string();
                    *mechanism_counts.entry(mech_str).or_insert(0) += 1;

                    // Analyze source spans
                    if !rel.source_spans.is_empty() {
                        doc_result.spans_populated += 1;

                        for span in &rel.source_spans {
                            total_spans += 1;
                            total_span_length += span.end_char.saturating_sub(span.start_char);

                            // Track span types
                            let span_type_str = span.span_type.as_str().to_string();
                            *span_type_counts.entry(span_type_str).or_insert(0) += 1;

                            // Validate offset bounds
                            if span.is_valid_for(content.len()) {
                                valid_offset_spans += 1;
                                doc_result.spans_valid += 1;

                                // Validate text match
                                if span.matches_source(content) {
                                    text_match_spans += 1;
                                    doc_result.spans_text_match += 1;
                                }
                            }
                        }
                    }
                }

                if args.verbose {
                    println!(
                        "    → {} relationships, {} spans",
                        doc_result.relationships_found, doc_result.spans_populated
                    );
                }
            }
            Err(e) => {
                doc_result.error = Some(format!("{}", e));
                if args.verbose {
                    println!("    → Error: {}", e);
                }
            }
        }

        per_doc_results.push(doc_result);

        if !args.verbose {
            print!(".");
            std::io::stdout().flush().ok();
        }
    }
    println!();
    println!();

    // Calculate metrics
    let extraction_metrics = ExtractionMetrics {
        total_documents: num_docs,
        documents_with_relationships: docs_with_relationships,
        total_relationships,
        avg_relationships_per_doc: if docs_with_relationships > 0 {
            total_relationships as f64 / docs_with_relationships as f64
        } else {
            0.0
        },
        avg_confidence: if total_relationships > 0 {
            total_confidence / total_relationships as f64
        } else {
            0.0
        },
        mechanism_distribution: mechanism_counts,
        json_parse_success_rate: (json_parse_successes as f64 / num_docs as f64) * 100.0,
    };

    let rels_with_spans: usize = per_doc_results.iter().map(|r| r.spans_populated).sum();
    let span_accuracy = SpanAccuracyMetrics {
        total_spans,
        spans_populated_rate: if total_relationships > 0 {
            (rels_with_spans as f64 / total_relationships as f64) * 100.0
        } else {
            0.0
        },
        offset_valid_rate: if total_spans > 0 {
            (valid_offset_spans as f64 / total_spans as f64) * 100.0
        } else {
            0.0
        },
        text_match_rate: if total_spans > 0 {
            (text_match_spans as f64 / total_spans as f64) * 100.0
        } else {
            0.0
        },
        avg_span_length: if total_spans > 0 {
            total_span_length as f64 / total_spans as f64
        } else {
            0.0
        },
        span_type_distribution: span_type_counts,
    };

    let mut sorted_times = extraction_times.clone();
    sorted_times.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let latency = LatencyMetrics {
        avg_extraction_ms: if extraction_times.is_empty() {
            0.0
        } else {
            extraction_times.iter().sum::<f64>() / extraction_times.len() as f64
        },
        min_extraction_ms: sorted_times.first().copied().unwrap_or(0.0),
        max_extraction_ms: sorted_times.last().copied().unwrap_or(0.0),
        p95_extraction_ms: if sorted_times.is_empty() {
            0.0
        } else {
            sorted_times[(sorted_times.len() as f64 * 0.95) as usize]
        },
        total_time_sec: benchmark_start.elapsed().as_secs_f64(),
    };

    // Print results
    println!("═══ Extraction Results ══════════════════════════════════════════════════════════");
    println!("  Documents processed: {}", num_docs);
    println!(
        "  Documents with relationships: {}",
        docs_with_relationships
    );
    println!("  Total relationships: {}", total_relationships);
    println!(
        "  JSON parse success: {:.1}%",
        extraction_metrics.json_parse_success_rate
    );
    println!("  Avg confidence: {:.2}", extraction_metrics.avg_confidence);
    println!();

    println!("═══ Source Span Accuracy ═══════════════════════════════════════════════════════");
    println!("  Total spans: {}", total_spans);
    println!(
        "  Span populated rate: {:.1}%",
        span_accuracy.spans_populated_rate
    );
    println!(
        "  Offset valid rate: {:.1}%",
        span_accuracy.offset_valid_rate
    );
    println!("  Text match rate: {:.1}%", span_accuracy.text_match_rate);
    println!(
        "  Avg span length: {:.0} chars",
        span_accuracy.avg_span_length
    );
    println!();

    println!("═══ Latency ═══════════════════════════════════════════════════════════════════");
    println!("  Avg extraction: {:.0}ms", latency.avg_extraction_ms);
    println!("  P95 extraction: {:.0}ms", latency.p95_extraction_ms);
    println!("  Total time: {:.2}s", latency.total_time_sec);
    println!();

    // Evaluate targets
    let targets = PerformanceTargets {
        span_populated_met: span_accuracy.spans_populated_rate >= 80.0,
        offset_valid_met: span_accuracy.offset_valid_rate >= 90.0,
        text_match_met: span_accuracy.text_match_rate >= 70.0,
        json_parse_met: extraction_metrics.json_parse_success_rate >= 95.0,
        all_critical_passed: false,
    };

    let all_passed = targets.span_populated_met
        && targets.offset_valid_met
        && targets.text_match_met
        && targets.json_parse_met;

    let targets = PerformanceTargets {
        all_critical_passed: all_passed,
        ..targets
    };

    println!("═══ Performance Targets ═════════════════════════════════════════════════════════");
    println!(
        "  Span populated (>80%):  {} ({:.1}%)",
        if targets.span_populated_met {
            "✓ PASS"
        } else {
            "✗ FAIL"
        },
        span_accuracy.spans_populated_rate
    );
    println!(
        "  Offset valid (>90%):    {} ({:.1}%)",
        if targets.offset_valid_met {
            "✓ PASS"
        } else {
            "✗ FAIL"
        },
        span_accuracy.offset_valid_rate
    );
    println!(
        "  Text match (>70%):      {} ({:.1}%)",
        if targets.text_match_met {
            "✓ PASS"
        } else {
            "✗ FAIL"
        },
        span_accuracy.text_match_rate
    );
    println!(
        "  JSON parse (>95%):      {} ({:.1}%)",
        if targets.json_parse_met {
            "✓ PASS"
        } else {
            "✗ FAIL"
        },
        extraction_metrics.json_parse_success_rate
    );
    println!();

    if targets.all_critical_passed {
        println!(
            "╔═══════════════════════════════════════════════════════════════════════════════╗"
        );
        println!(
            "║                    ✓ ALL TARGETS PASSED                                       ║"
        );
        println!(
            "╚═══════════════════════════════════════════════════════════════════════════════╝"
        );
    } else {
        println!(
            "╔═══════════════════════════════════════════════════════════════════════════════╗"
        );
        println!(
            "║                    ✗ SOME TARGETS FAILED                                      ║"
        );
        println!(
            "╚═══════════════════════════════════════════════════════════════════════════════╝"
        );
    }

    // Generate recommendations
    let mut recommendations = Vec::new();
    if !targets.span_populated_met {
        recommendations.push("Improve GBNF grammar to require source_spans field".to_string());
    }
    if !targets.text_match_met {
        recommendations
            .push("Refine prompt to ensure text_excerpt matches source exactly".to_string());
    }
    if !targets.json_parse_met {
        recommendations
            .push("Review GBNF grammar for edge cases causing parse failures".to_string());
    }
    if all_passed {
        recommendations
            .push("All targets met - provenance extraction working correctly".to_string());
    }

    // Build results
    let results = LlmProvenanceBenchResults {
        timestamp: Utc::now().to_rfc3339(),
        config: BenchmarkConfig {
            num_documents: num_docs,
            min_confidence: args.min_confidence,
        },
        llm_info: LlmInfo {
            model_name: "Hermes-2-Pro-Mistral-7B".to_string(),
            model_load_time_sec: load_time,
            grammar_type: "MultiRelationship".to_string(),
            vram_usage_mb: 6000.0, // Approximate
        },
        extraction_metrics,
        span_accuracy,
        latency,
        targets,
        per_document_results: per_doc_results,
        recommendations,
    };

    // Write results
    if let Some(parent) = args.output_path.parent() {
        fs::create_dir_all(parent).ok();
    }

    let json = serde_json::to_string_pretty(&results).expect("Failed to serialize");
    let mut file = File::create(&args.output_path).expect("Failed to create file");
    file.write_all(json.as_bytes()).expect("Failed to write");
    println!("\nResults written to: {}", args.output_path.display());
}
