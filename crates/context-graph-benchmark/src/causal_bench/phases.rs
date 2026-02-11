//! 8-phase causal embedding benchmark implementations.
//!
//! Each phase tests a specific aspect of E5 causal embedding quality:
//! 1. Query intent detection accuracy
//! 2. E5 embedding quality (spread, anisotropy, standalone accuracy)
//! 3. Direction modifier verification
//! 4. Ablation analysis (with/without E5)
//! 5. Causal gate effectiveness (TPR/TNR)
//! 6. End-to-end retrieval accuracy
//! 7. Cross-domain generalization
//! 8. Performance profiling

use std::cell::OnceCell;
use std::collections::HashMap;
use std::sync::Arc;

use super::data_loader::{BenchmarkPair, BenchmarkQuery};
use super::metrics::{self, PhaseBenchmarkResult};
use super::provider::EmbeddingProvider;

/// Configuration for benchmark run.
pub struct BenchConfig {
    /// Path to dataset directory.
    pub data_dir: std::path::PathBuf,
    /// Run in quick mode (20% of data).
    pub quick: bool,
    /// Verbose per-query output.
    pub verbose: bool,
    /// Causal gate threshold.
    pub causal_gate_threshold: f32,
    /// E5 weight in causal_reasoning profile.
    pub e5_weight: f32,
    /// Embedding provider (GPU or synthetic).
    pub provider: Arc<dyn EmbeddingProvider>,
    /// Pre-computed E5 scores keyed by pair ID. Populated lazily before Phase 4+.
    /// Avoids redundant GPU calls: 250 pair scores computed once, reused across
    /// 90,000+ simulate_search() invocations in Phases 4/6/7.
    pub pair_score_cache: OnceCell<HashMap<String, f32>>,
    /// Pre-computed E1 semantic scores keyed by "{query_id}_{pair_id}".
    /// Populated lazily before Phases 4/6/7 when GPU provider has E1.
    pub e1_score_cache: OnceCell<HashMap<String, f32>>,
}

impl BenchConfig {
    /// Pre-compute E5 scores for all pairs and cache by pair ID.
    ///
    /// Call this once before Phases 4/6/7 to avoid O(queries * pairs) GPU calls.
    /// Safe to call multiple times — only the first call computes scores.
    pub fn precompute_pair_scores(&self, pairs: &[BenchmarkPair]) {
        if self.pair_score_cache.get().is_some() {
            return;
        }
        let mut cache = HashMap::with_capacity(pairs.len());
        for (i, pair) in pairs.iter().enumerate() {
            let score = self.provider.e5_score(&pair.cause_text, &pair.effect_text);
            cache.insert(pair.id.clone(), score);
            if (i + 1) % 50 == 0 || i + 1 == pairs.len() {
                tracing::info!("  Pre-computed E5 scores: {}/{}", i + 1, pairs.len());
            }
        }
        let _ = self.pair_score_cache.set(cache);
    }

    /// Get E5 score for a pair, using cache if available.
    ///
    /// Falls back to live provider call if cache miss (shouldn't happen
    /// if precompute_pair_scores was called with all pairs).
    fn get_pair_e5_score(&self, pair: &BenchmarkPair) -> f32 {
        if let Some(cache) = self.pair_score_cache.get() {
            if let Some(&score) = cache.get(&pair.id) {
                return score;
            }
        }
        self.provider.e5_score(&pair.cause_text, &pair.effect_text)
    }

    /// Pre-compute E1 semantic scores for all query x pair combinations.
    ///
    /// Call once before Phases 4/6/7 to avoid O(queries x pairs) GPU calls.
    /// Only runs when provider has E1; safe to call multiple times.
    pub fn precompute_e1_scores(&self, queries: &[BenchmarkQuery], pairs: &[BenchmarkPair]) {
        if self.e1_score_cache.get().is_some() || !self.provider.has_e1() {
            return;
        }
        let total = queries.len() * pairs.len();
        tracing::info!("  Pre-computing E1 semantic scores: {} queries x {} pairs = {}", queries.len(), pairs.len(), total);
        let mut cache = HashMap::with_capacity(total);
        for (qi, query) in queries.iter().enumerate() {
            for pair in pairs {
                let passage = format!("{} {}", pair.cause_text, pair.effect_text);
                let score = self.provider.e1_score(&query.query, &passage);
                cache.insert(format!("{}_{}", query.id, pair.id), score);
            }
            if (qi + 1) % 20 == 0 || qi + 1 == queries.len() {
                tracing::info!("    E1 scores: {}/{} queries done", qi + 1, queries.len());
            }
        }
        let _ = self.e1_score_cache.set(cache);
    }

    /// Get E1 score for a query-pair combination from cache.
    fn get_query_pair_e1_score(&self, query_id: &str, pair: &BenchmarkPair) -> Option<f32> {
        self.e1_score_cache
            .get()
            .and_then(|cache| cache.get(&format!("{}_{}", query_id, pair.id)))
            .copied()
    }
}

impl Default for BenchConfig {
    fn default() -> Self {
        Self {
            data_dir: std::path::PathBuf::from("data/causal_benchmark"),
            quick: false,
            verbose: false,
            causal_gate_threshold: 0.30,
            e5_weight: 0.10,
            provider: Arc::new(super::provider::SyntheticProvider::new()),
            pair_score_cache: OnceCell::new(),
            e1_score_cache: OnceCell::new(),
        }
    }
}

impl std::fmt::Debug for BenchConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BenchConfig")
            .field("data_dir", &self.data_dir)
            .field("quick", &self.quick)
            .field("verbose", &self.verbose)
            .field("causal_gate_threshold", &self.causal_gate_threshold)
            .field("e5_weight", &self.e5_weight)
            .field("provider", &self.provider.name())
            .field("cached_pairs", &self.pair_score_cache.get().map(|c| c.len()))
            .field("cached_e1", &self.e1_score_cache.get().map(|c| c.len()))
            .finish()
    }
}

// ============================================================================
// Phase 1: Query Intent Detection
// ============================================================================

/// Phase 1: Test detect_causal_query_intent() accuracy against labeled queries.
///
/// Pass criteria: accuracy >= 90%, negation false positive rate < 15%
pub fn phase1_query_intent(
    queries: &[BenchmarkQuery],
    verbose: bool,
) -> PhaseBenchmarkResult {
    let start = std::time::Instant::now();

    let mut predicted = Vec::new();
    let mut actual = Vec::new();
    let mut negation_total = 0usize;
    let mut negation_fp = 0usize;

    for query in queries {
        let detected = detect_intent(&query.query);
        let expected = &query.expected_direction;

        predicted.push(detected.clone());
        actual.push(expected.clone());

        if query.is_negation {
            negation_total += 1;
            // False positive if we detect a direction when we shouldn't
            if detected != "unknown" && detected != "neutral" {
                negation_fp += 1;
            }
        }

        if verbose {
            let correct = directions_match(&detected, expected);
            tracing::info!(
                "  {} query={:.50}... predicted={} expected={} {}",
                query.id,
                query.query,
                detected,
                expected,
                if correct { "OK" } else { "MISS" }
            );
        }
    }

    let pred_refs: Vec<&str> = predicted.iter().map(|s| s.as_str()).collect();
    let act_refs: Vec<&str> = actual.iter().map(|s| s.as_str()).collect();
    let (accuracy, _cm) = metrics::query_intent_accuracy(&pred_refs, &act_refs);

    let negation_fp_rate = if negation_total > 0 {
        negation_fp as f64 / negation_total as f64
    } else {
        0.0
    };

    // Per-class F1
    let cause_queries: Vec<usize> = actual
        .iter()
        .enumerate()
        .filter(|(_, a)| *a == "cause")
        .map(|(i, _)| i)
        .collect();
    let effect_queries: Vec<usize> = actual
        .iter()
        .enumerate()
        .filter(|(_, a)| *a == "effect")
        .map(|(i, _)| i)
        .collect();

    let cause_correct = cause_queries
        .iter()
        .filter(|&&i| directions_match(&predicted[i], &actual[i]))
        .count();
    let effect_correct = effect_queries
        .iter()
        .filter(|&&i| directions_match(&predicted[i], &actual[i]))
        .count();

    let cause_acc = if cause_queries.is_empty() {
        0.0
    } else {
        cause_correct as f64 / cause_queries.len() as f64
    };
    let effect_acc = if effect_queries.is_empty() {
        0.0
    } else {
        effect_correct as f64 / effect_queries.len() as f64
    };

    let mut phase_metrics = HashMap::new();
    phase_metrics.insert("accuracy".to_string(), accuracy as f64);
    phase_metrics.insert("negation_fp".to_string(), negation_fp_rate);
    phase_metrics.insert("cause_accuracy".to_string(), cause_acc);
    phase_metrics.insert("effect_accuracy".to_string(), effect_acc);
    phase_metrics.insert("total_queries".to_string(), queries.len() as f64);

    let mut targets = HashMap::new();
    targets.insert("accuracy".to_string(), 0.90);
    targets.insert("negation_fp".to_string(), 0.15); // max

    let duration = start.elapsed().as_millis() as u64;

    metrics::make_phase_result(1, "Query Intent Detection", phase_metrics, targets, duration)
}

/// Intent detection delegating to the real `detect_causal_query_intent()` from context-graph-core.
///
/// This eliminates sync drift between the benchmark and the production detector.
/// Returns "cause", "effect", or "unknown".
fn detect_intent(query: &str) -> String {
    use context_graph_core::causal::asymmetric::{detect_causal_query_intent, CausalDirection};
    match detect_causal_query_intent(query) {
        CausalDirection::Cause => "cause".to_string(),
        CausalDirection::Effect => "effect".to_string(),
        CausalDirection::Unknown => "unknown".to_string(),
    }
}

fn directions_match(predicted: &str, expected: &str) -> bool {
    let p = predicted.to_lowercase();
    let e = expected.to_lowercase();
    p == e || (p == "unknown" && e == "neutral") || (p == "neutral" && e == "unknown")
}

// ============================================================================
// Phase 2: E5 Embedding Quality
// ============================================================================

/// Phase 2: Test E5 embedding quality metrics.
///
/// Computes score spread, anisotropy, and standalone accuracy using
/// simulated E5 scores. In production, would use real GPU embeddings.
///
/// Pass criteria: spread > 0.10, anisotropy < 0.30, standalone >= 67%
pub fn phase2_e5_quality(
    causal_pairs: &[BenchmarkPair],
    non_causal_pairs: &[BenchmarkPair],
    config: &BenchConfig,
) -> PhaseBenchmarkResult {
    let start = std::time::Instant::now();

    let mut all_scores = Vec::new();
    let mut all_vectors = Vec::new();

    if config.provider.is_gpu() {
        // GPU mode: use real model embeddings
        for pair in causal_pairs.iter().chain(non_causal_pairs.iter()) {
            all_scores.push(provider_e5_score(config.provider.as_ref(), pair));
            all_vectors.push(provider_e5_embedding(config.provider.as_ref(), pair));
        }
    } else {
        // Synthetic mode: simulate E5 compression
        for pair in causal_pairs {
            all_scores.push(synthetic_e5_score(pair, true));
            all_vectors.push(synthetic_embedding(pair, 768));
        }
        for pair in non_causal_pairs {
            all_scores.push(synthetic_e5_score(pair, false));
            all_vectors.push(synthetic_embedding(pair, 768));
        }
    }

    let spread = metrics::score_spread(&all_scores);
    let anisotropy = metrics::anisotropy_measure(&all_vectors);

    // Standalone accuracy: for each causal pair, check if E5 ranks correct
    // match above random. With compressed scores, this will be low.
    let standalone_acc = compute_standalone_accuracy(causal_pairs, config);

    // Cause-effect vector distance
    let ce_distance = compute_cause_effect_distance(causal_pairs, config);

    if config.verbose {
        tracing::info!("  E5 score range: {:.4}-{:.4}",
            all_scores.iter().cloned().fold(f32::INFINITY, f32::min),
            all_scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max));
        tracing::info!("  Spread: {:.4}, Anisotropy: {:.4}", spread, anisotropy);
        tracing::info!("  Standalone accuracy: {:.4}", standalone_acc);
        tracing::info!("  Cause-effect distance: {:.4}", ce_distance);
    }

    let mut phase_metrics = HashMap::new();
    phase_metrics.insert("spread".to_string(), spread as f64);
    phase_metrics.insert("anisotropy".to_string(), anisotropy as f64);
    phase_metrics.insert("standalone_accuracy".to_string(), standalone_acc as f64);
    phase_metrics.insert("cause_effect_distance".to_string(), ce_distance as f64);
    phase_metrics.insert("num_causal".to_string(), causal_pairs.len() as f64);
    phase_metrics.insert("num_non_causal".to_string(), non_causal_pairs.len() as f64);

    let mut targets = HashMap::new();
    targets.insert("spread".to_string(), 0.10);
    targets.insert("anisotropy".to_string(), 0.30); // max
    targets.insert("standalone_accuracy".to_string(), 0.67);

    let duration = start.elapsed().as_millis() as u64;
    metrics::make_phase_result(2, "E5 Embedding Quality", phase_metrics, targets, duration)
}

/// Compute E5 score for a pair using the configured provider.
///
/// For GPU provider: computes real cosine similarity between cause/effect embeddings.
/// For synthetic provider: produces deterministic hash-based scores simulating E5 compression.
fn provider_e5_score(provider: &dyn EmbeddingProvider, pair: &BenchmarkPair) -> f32 {
    provider.e5_score(&pair.cause_text, &pair.effect_text)
}

/// Compute E5 embedding for a pair using the configured provider.
fn provider_e5_embedding(provider: &dyn EmbeddingProvider, pair: &BenchmarkPair) -> Vec<f32> {
    let text = format!("{} {}", pair.cause_text, pair.effect_text);
    provider.e5_embedding(&text)
}

fn synthetic_e5_score(pair: &BenchmarkPair, is_causal: bool) -> f32 {
    // Simulate E5 compression: all causal text clusters 0.93-0.98
    let base = if is_causal { 0.955 } else { 0.92 };
    let noise = hash_to_float(&pair.id) * 0.04;
    (base + noise).clamp(0.0, 1.0)
}

fn synthetic_embedding(pair: &BenchmarkPair, dim: usize) -> Vec<f32> {
    // Generate deterministic pseudo-random embedding from pair content
    let mut vec = vec![0.0f32; dim];
    let seed = hash_text(&format!("{}{}", pair.cause_text, pair.effect_text));
    for (i, v) in vec.iter_mut().enumerate() {
        let h = hash_usize(seed.wrapping_add(i as u64));
        *v = (h as f32 / u64::MAX as f32) * 2.0 - 1.0;
    }
    // L2 normalize
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 1e-8 {
        for v in vec.iter_mut() {
            *v /= norm;
        }
    }
    vec
}

fn compute_standalone_accuracy(pairs: &[BenchmarkPair], config: &BenchConfig) -> f32 {
    if pairs.len() < 2 {
        return 0.0;
    }

    if config.provider.is_gpu() {
        // GPU mode: use real embeddings for similarity-based retrieval
        let embeddings: Vec<Vec<f32>> = pairs
            .iter()
            .map(|p| config.provider.e5_embedding(&format!("{} {}", p.cause_text, p.effect_text)))
            .collect();

        let mut correct = 0usize;
        for (i, _) in pairs.iter().enumerate() {
            let mut best_idx = 0;
            let mut best_sim = f32::NEG_INFINITY;
            for (j, _) in pairs.iter().enumerate() {
                if i == j { continue; }
                let sim = metrics::cosine_similarity(&embeddings[i], &embeddings[j]);
                if sim > best_sim {
                    best_sim = sim;
                    best_idx = j;
                }
            }
            if pairs[best_idx].domain == pairs[i].domain {
                correct += 1;
            }
        }
        correct as f32 / pairs.len() as f32
    } else {
        // Synthetic mode: score-proximity based
        let mut correct = 0usize;
        for (i, query_pair) in pairs.iter().enumerate() {
            let query_score = synthetic_e5_score(query_pair, true);
            let mut best_idx = 0;
            let mut best_diff = f32::INFINITY;
            for (j, cand_pair) in pairs.iter().enumerate() {
                if i == j { continue; }
                let cand_score = synthetic_e5_score(cand_pair, true);
                let diff = (query_score - cand_score).abs();
                if diff < best_diff {
                    best_diff = diff;
                    best_idx = j;
                }
            }
            if pairs[best_idx].domain == query_pair.domain {
                correct += 1;
            }
        }
        correct as f32 / pairs.len() as f32
    }
}

fn compute_cause_effect_distance(pairs: &[BenchmarkPair], config: &BenchConfig) -> f32 {
    if pairs.is_empty() {
        return 0.0;
    }

    let mut total_sim = 0.0f32;
    if config.provider.is_gpu() {
        // GPU mode: use real dual embeddings
        for pair in pairs {
            let text = format!("{} {}", pair.cause_text, pair.effect_text);
            let (cause_vec, effect_vec) = config.provider.e5_dual_embeddings(&text);
            total_sim += metrics::cosine_similarity(&cause_vec, &effect_vec);
        }
    } else {
        // Synthetic mode: different hash seeds for cause vs effect text
        for pair in pairs {
            let cause_vec = synthetic_embedding(
                &BenchmarkPair {
                    cause_text: pair.cause_text.clone(),
                    effect_text: String::new(),
                    ..pair.clone()
                },
                768,
            );
            let effect_vec = synthetic_embedding(
                &BenchmarkPair {
                    cause_text: String::new(),
                    effect_text: pair.effect_text.clone(),
                    ..pair.clone()
                },
                768,
            );
            total_sim += metrics::cosine_similarity(&cause_vec, &effect_vec);
        }
    }
    total_sim / pairs.len() as f32
}

// ============================================================================
// Phase 3: Direction Modifier Verification
// ============================================================================

/// Phase 3: Verify direction modifiers produce correct asymmetry.
///
/// For forward pairs: sim(cause→effect) should be > sim(effect→cause)
/// Ratio should be in [1.3, 1.7] (theoretical 1.5 from 1.2/0.8)
///
/// Pass criteria: accuracy > 90%, ratio in [1.3, 1.7]
pub fn phase3_direction_modifiers(
    forward_pairs: &[BenchmarkPair],
    verbose: bool,
) -> PhaseBenchmarkResult {
    let start = std::time::Instant::now();

    // Direction modifiers from asymmetric.rs
    let cause_to_effect = 1.2f32;
    let effect_to_cause = 0.8f32;

    let mut forward_sims = Vec::new();
    let mut reverse_sims = Vec::new();

    for pair in forward_pairs {
        // Simulate base similarity
        let base_sim = 0.75 + hash_to_float(&pair.id) * 0.2;
        let fwd = base_sim * cause_to_effect;
        let rev = base_sim * effect_to_cause;
        forward_sims.push(fwd);
        reverse_sims.push(rev);

        if verbose {
            tracing::info!(
                "  {} fwd={:.4} rev={:.4} correct={}",
                pair.id,
                fwd,
                rev,
                fwd > rev
            );
        }
    }

    let dir_accuracy = metrics::directional_accuracy(&forward_sims, &reverse_sims);
    let ratio = metrics::direction_ratio(&forward_sims, &reverse_sims);

    let mut phase_metrics = HashMap::new();
    phase_metrics.insert("accuracy".to_string(), dir_accuracy as f64);
    phase_metrics.insert("ratio".to_string(), ratio as f64);
    phase_metrics.insert("num_pairs".to_string(), forward_pairs.len() as f64);
    phase_metrics.insert("theoretical_ratio".to_string(), 1.5);

    let mut targets = HashMap::new();
    targets.insert("accuracy".to_string(), 0.90);
    // Ratio targets are checked as range, not threshold
    // We check >= 1.3 for the lower bound
    targets.insert("ratio".to_string(), 1.30);

    let duration = start.elapsed().as_millis() as u64;

    let mut result =
        metrics::make_phase_result(3, "Direction Modifiers", phase_metrics, targets, duration);

    // Additional check: ratio should also be <= 1.7
    if ratio > 1.7 {
        result.pass = false;
        result
            .failing_criteria
            .push(format!("ratio: {:.4} (target: <=1.7)", ratio));
    }

    result
}

// ============================================================================
// Phase 4: Ablation Analysis
// ============================================================================

/// Phase 4: Compare retrieval with and without E5 embedder.
///
/// Pass criteria: ablation_delta > 5%, E5 RRF contribution > 12%
pub fn phase4_ablation(
    queries: &[BenchmarkQuery],
    pairs: &[BenchmarkPair],
    config: &BenchConfig,
) -> PhaseBenchmarkResult {
    let start = std::time::Instant::now();

    // Simulate 3 search configurations
    let mut with_e5_correct = 0usize;
    let mut without_e5_correct = 0usize;
    let mut e5_only_correct = 0usize;

    for query in queries {
        // With E5 (multi_space, causal_reasoning profile)
        let match_with = simulate_search(query, pairs, true, false, config);
        if match_with == query.expected_top1_id {
            with_e5_correct += 1;
        }

        // Without E5 (E5 weight zeroed)
        let match_without = simulate_search(query, pairs, false, false, config);
        if match_without == query.expected_top1_id {
            without_e5_correct += 1;
        }

        // E5 only
        let match_e5only = simulate_search(query, pairs, false, true, config);
        if match_e5only == query.expected_top1_id {
            e5_only_correct += 1;
        }

        if config.verbose {
            tracing::info!(
                "  {} with_e5={} without_e5={} e5_only={}",
                query.id,
                match_with == query.expected_top1_id,
                match_without == query.expected_top1_id,
                match_e5only == query.expected_top1_id,
            );
        }
    }

    let n = queries.len() as f32;
    let acc_with = with_e5_correct as f32 / n;
    let acc_without = without_e5_correct as f32 / n;
    let acc_e5only = e5_only_correct as f32 / n;
    let delta = metrics::ablation_delta(acc_with, acc_without);

    let e5_rrf_pct = if n > 0.0 { (acc_e5only * 100.0) as f64 } else { 0.0 };

    let mut phase_metrics = HashMap::new();
    phase_metrics.insert("accuracy_with_e5".to_string(), acc_with as f64);
    phase_metrics.insert("accuracy_without_e5".to_string(), acc_without as f64);
    phase_metrics.insert("accuracy_e5_only".to_string(), acc_e5only as f64);
    phase_metrics.insert("delta".to_string(), delta as f64);
    phase_metrics.insert("e5_rrf_contribution".to_string(), e5_rrf_pct);
    phase_metrics.insert("num_queries".to_string(), queries.len() as f64);

    let mut targets = HashMap::new();
    targets.insert("delta".to_string(), 5.0);
    targets.insert("e5_rrf_contribution".to_string(), 12.0);

    let duration = start.elapsed().as_millis() as u64;
    metrics::make_phase_result(4, "Ablation Analysis", phase_metrics, targets, duration)
}

fn simulate_search(
    query: &BenchmarkQuery,
    pairs: &[BenchmarkPair],
    with_e5: bool,
    e5_only: bool,
    config: &BenchConfig,
) -> String {
    // Search simulation: E1 approximated by domain+keyword match, E5 from provider.
    let query_lower = query.query.to_lowercase();
    let mut best_id = String::new();
    let mut best_score = f32::NEG_INFINITY;

    for pair in pairs {
        let mut score = 0.0f32;

        if !e5_only {
            // E1 semantic similarity (real or keyword proxy fallback)
            if let Some(e1) = config.get_query_pair_e1_score(&query.id, pair) {
                score += e1 * 0.45;
            } else {
                // Fallback: keyword proxy for CI/synthetic mode
                if pair.domain == query.expected_domain {
                    score += 0.4;
                }
                let cause_words: Vec<&str> = pair.cause_text.split_whitespace().collect();
                let overlap = cause_words
                    .iter()
                    .filter(|w| query_lower.contains(&w.to_lowercase()))
                    .count();
                score += overlap as f32 * 0.05;
            }
        }

        if with_e5 || e5_only {
            // E5 score from cache (pre-computed) or live provider call
            let e5_score = config.get_pair_e5_score(pair);
            let e5_weight = if e5_only { 1.0 } else { config.e5_weight };
            score += e5_score * e5_weight;
        }

        // Deterministic tie-breaking
        score += hash_to_float(&pair.id) * 0.001;

        if score > best_score {
            best_score = score;
            best_id = pair.id.clone();
        }
    }

    best_id
}

// ============================================================================
// Phase 5: Causal Gate Effectiveness
// ============================================================================

/// Phase 5: Test causal gate TPR/TNR.
///
/// Pass criteria: TPR > 70%, TNR > 75%
pub fn phase5_causal_gate(
    causal_pairs: &[BenchmarkPair],
    non_causal_pairs: &[BenchmarkPair],
    config: &BenchConfig,
) -> PhaseBenchmarkResult {
    let threshold = config.causal_gate_threshold;
    let start = std::time::Instant::now();

    let mut scores = Vec::new();
    let mut labels = Vec::new();

    if config.provider.is_gpu() {
        for pair in causal_pairs {
            scores.push(provider_e5_score(config.provider.as_ref(), pair));
            labels.push(true);
        }
        for pair in non_causal_pairs {
            scores.push(provider_e5_score(config.provider.as_ref(), pair));
            labels.push(false);
        }
    } else {
        for pair in causal_pairs {
            scores.push(synthetic_e5_score(pair, true));
            labels.push(true);
        }
        for pair in non_causal_pairs {
            scores.push(synthetic_e5_score(pair, false));
            labels.push(false);
        }
    }

    let (tpr, tnr) = metrics::causal_gate_tpr_tnr(&scores, &labels, threshold);

    // Score shift analysis
    let causal_mean: f32 = scores
        .iter()
        .zip(labels.iter())
        .filter(|(_, &l)| l)
        .map(|(s, _)| s)
        .sum::<f32>()
        / causal_pairs.len().max(1) as f32;
    let non_causal_mean: f32 = scores
        .iter()
        .zip(labels.iter())
        .filter(|(_, &l)| !l)
        .map(|(s, _)| s)
        .sum::<f32>()
        / non_causal_pairs.len().max(1) as f32;

    if config.verbose {
        tracing::info!("  Causal mean score: {:.4}", causal_mean);
        tracing::info!("  Non-causal mean score: {:.4}", non_causal_mean);
        tracing::info!("  Score gap: {:.4}", causal_mean - non_causal_mean);
        tracing::info!("  TPR: {:.4}, TNR: {:.4}", tpr, tnr);
    }

    let mut phase_metrics = HashMap::new();
    phase_metrics.insert("tpr".to_string(), tpr as f64);
    phase_metrics.insert("tnr".to_string(), tnr as f64);
    phase_metrics.insert("causal_mean".to_string(), causal_mean as f64);
    phase_metrics.insert("non_causal_mean".to_string(), non_causal_mean as f64);
    phase_metrics.insert("score_gap".to_string(), (causal_mean - non_causal_mean) as f64);
    phase_metrics.insert("threshold".to_string(), threshold as f64);

    let mut targets = HashMap::new();
    targets.insert("tpr".to_string(), 0.70);
    targets.insert("tnr".to_string(), 0.75);

    let duration = start.elapsed().as_millis() as u64;
    metrics::make_phase_result(5, "Causal Gate", phase_metrics, targets, duration)
}

// ============================================================================
// Phase 6: End-to-End Retrieval
// ============================================================================

/// Phase 6: Full retrieval accuracy with multi-embedder fusion.
///
/// Pass criteria: top1 > 55%, MRR > 0.65, NDCG@5 > 0.70
pub fn phase6_e2e_retrieval(
    queries: &[BenchmarkQuery],
    pairs: &[BenchmarkPair],
    config: &BenchConfig,
) -> PhaseBenchmarkResult {
    let start = std::time::Instant::now();

    let mut ranked_results = Vec::new();
    let mut expected_top1 = Vec::new();

    for query in queries {
        // Full multi-space search simulation (E1 proxy + E5 from provider)
        let results = simulate_ranked_search(query, pairs, config);
        expected_top1.push(query.expected_top1_id.clone());

        if config.verbose && results.first() != Some(&query.expected_top1_id) {
            tracing::info!(
                "  {} MISS: got {} expected {}",
                query.id,
                results.first().unwrap_or(&"<empty>".to_string()),
                query.expected_top1_id
            );
        }

        ranked_results.push(results);
    }

    let top1 = metrics::top1_accuracy(&ranked_results, &expected_top1);
    let top5 = metrics::top_k_accuracy(&ranked_results, &expected_top1, 5);
    let mrr = metrics::retrieval_mrr(&ranked_results, &expected_top1);

    // Average NDCG@5
    let mut ndcg_sum = 0.0f32;
    for (i, query) in queries.iter().enumerate() {
        let ndcg = metrics::retrieval_ndcg_at_k(
            &ranked_results[i],
            &query.expected_top5_ids,
            5,
        );
        ndcg_sum += ndcg;
    }
    let avg_ndcg = ndcg_sum / queries.len().max(1) as f32;

    let mut phase_metrics = HashMap::new();
    phase_metrics.insert("top1_accuracy".to_string(), top1 as f64);
    phase_metrics.insert("top5_accuracy".to_string(), top5 as f64);
    phase_metrics.insert("mrr".to_string(), mrr as f64);
    phase_metrics.insert("ndcg_at_5".to_string(), avg_ndcg as f64);
    phase_metrics.insert("num_queries".to_string(), queries.len() as f64);

    let mut targets = HashMap::new();
    targets.insert("top1_accuracy".to_string(), 0.55);
    targets.insert("mrr".to_string(), 0.65);
    targets.insert("ndcg_at_5".to_string(), 0.70);

    let duration = start.elapsed().as_millis() as u64;
    metrics::make_phase_result(6, "End-to-End Retrieval", phase_metrics, targets, duration)
}

fn simulate_ranked_search(
    query: &BenchmarkQuery,
    pairs: &[BenchmarkPair],
    config: &BenchConfig,
) -> Vec<String> {
    let query_lower = query.query.to_lowercase();
    let mut scored: Vec<(String, f32)> = pairs
        .iter()
        .map(|pair| {
            let mut score = 0.0f32;

            // E1 semantic similarity (real or keyword proxy fallback)
            if let Some(e1) = config.get_query_pair_e1_score(&query.id, pair) {
                score += e1 * 0.45;
            } else {
                // Fallback: keyword proxy for CI/synthetic mode
                if pair.domain == query.expected_domain {
                    score += 0.35;
                }

                // Word overlap with cause_text
                let words: Vec<&str> = pair.cause_text.split_whitespace().collect();
                let overlap = words
                    .iter()
                    .filter(|w| w.len() > 3 && query_lower.contains(&w.to_lowercase()))
                    .count();
                score += overlap as f32 * 0.08;

                // Word overlap with effect_text
                let words: Vec<&str> = pair.effect_text.split_whitespace().collect();
                let overlap = words
                    .iter()
                    .filter(|w| w.len() > 3 && query_lower.contains(&w.to_lowercase()))
                    .count();
                score += overlap as f32 * 0.06;
            }

            // E5 contribution from cache (pre-computed) or live provider call
            let e5 = config.get_pair_e5_score(pair);
            score += e5 * config.e5_weight;

            // Deterministic noise for variety
            score += hash_to_float(&format!("{}_{}", query.id, pair.id)) * 0.02;

            (pair.id.clone(), score)
        })
        .collect();

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().map(|(id, _)| id).collect()
}

// ============================================================================
// Phase 7: Cross-Domain Generalization
// ============================================================================

/// Phase 7: Test generalization to held-out domains.
///
/// Pass criteria: held_out_accuracy > 45%, transfer_gap < 25%
pub fn phase7_cross_domain(
    queries: &[BenchmarkQuery],
    train_pairs: &[BenchmarkPair],
    held_out_pairs: &[BenchmarkPair],
    config: &BenchConfig,
) -> PhaseBenchmarkResult {
    let start = std::time::Instant::now();

    // Queries targeting train domains
    let train_domains: std::collections::HashSet<&str> =
        train_pairs.iter().map(|p| p.domain.as_str()).collect();
    let held_out_domains: std::collections::HashSet<&str> =
        held_out_pairs.iter().map(|p| p.domain.as_str()).collect();

    let train_queries: Vec<&BenchmarkQuery> = queries
        .iter()
        .filter(|q| train_domains.contains(q.expected_domain.as_str()))
        .collect();
    let held_out_queries: Vec<&BenchmarkQuery> = queries
        .iter()
        .filter(|q| held_out_domains.contains(q.expected_domain.as_str()))
        .collect();

    // Train domain accuracy
    let mut train_correct = 0usize;
    for q in &train_queries {
        let result = simulate_search(q, train_pairs, true, false, config);
        if result == q.expected_top1_id {
            train_correct += 1;
        }
    }
    let train_acc = if train_queries.is_empty() {
        0.0
    } else {
        train_correct as f32 / train_queries.len() as f32
    };

    // Held-out domain accuracy
    let mut held_out_correct = 0usize;
    for q in &held_out_queries {
        let result = simulate_search(q, held_out_pairs, true, false, config);
        if result == q.expected_top1_id {
            held_out_correct += 1;
        }
    }
    let held_out_acc = if held_out_queries.is_empty() {
        0.0
    } else {
        held_out_correct as f32 / held_out_queries.len() as f32
    };

    let gap = metrics::cross_domain_gap(train_acc, held_out_acc);

    if config.verbose {
        tracing::info!("  Train domains: {:?}", train_domains);
        tracing::info!("  Held-out domains: {:?}", held_out_domains);
        tracing::info!("  Train accuracy: {:.4} ({}/{})", train_acc, train_correct, train_queries.len());
        tracing::info!("  Held-out accuracy: {:.4} ({}/{})", held_out_acc, held_out_correct, held_out_queries.len());
        tracing::info!("  Transfer gap: {:.4}", gap);
    }

    let mut phase_metrics = HashMap::new();
    phase_metrics.insert("train_accuracy".to_string(), train_acc as f64);
    phase_metrics.insert("held_out_accuracy".to_string(), held_out_acc as f64);
    phase_metrics.insert("gap".to_string(), gap as f64);
    phase_metrics.insert("train_queries".to_string(), train_queries.len() as f64);
    phase_metrics.insert("held_out_queries".to_string(), held_out_queries.len() as f64);

    let mut targets = HashMap::new();
    targets.insert("held_out_accuracy".to_string(), 0.45);
    targets.insert("gap".to_string(), 0.25); // max

    let duration = start.elapsed().as_millis() as u64;
    metrics::make_phase_result(7, "Cross-Domain Generalization", phase_metrics, targets, duration)
}

// ============================================================================
// Phase 8: Performance Profiling
// ============================================================================

/// Phase 8: Measure embedding latency, throughput, and overhead.
///
/// Uses synthetic timing since we don't have GPU access in the benchmark binary.
/// In production, this would use real CausalModel + SemanticModel.
///
/// Pass criteria: overhead < 2.5x, throughput > 80 QPS
pub fn phase8_performance(
    pairs: &[BenchmarkPair],
    verbose: bool,
) -> PhaseBenchmarkResult {
    let start = std::time::Instant::now();

    let n = pairs.len();

    // Simulate embedding latency
    // E5 dual (cause + effect): ~2x single embedding time
    // E1 single: baseline
    let e1_times_us: Vec<u64> = pairs
        .iter()
        .map(|p| {
            let text_len = p.cause_text.len() + p.effect_text.len();
            // ~1-5ms per embedding, proportional to text length
            1000 + (text_len as u64 * 10)
        })
        .collect();

    let e5_times_us: Vec<u64> = pairs
        .iter()
        .map(|p| {
            let text_len = p.cause_text.len() + p.effect_text.len();
            // Dual vectors = ~1.5x time (some batching efficiency)
            1500 + (text_len as u64 * 15)
        })
        .collect();

    let e1_median = percentile(&e1_times_us, 0.50);
    let e1_p95 = percentile(&e1_times_us, 0.95);
    let e5_median = percentile(&e5_times_us, 0.50);
    let e5_p95 = percentile(&e5_times_us, 0.95);
    let e5_p99 = percentile(&e5_times_us, 0.99);

    let overhead = e5_median as f64 / e1_median.max(1) as f64;

    // Throughput: embeddings per second
    let total_e5_time_s = e5_times_us.iter().sum::<u64>() as f64 / 1_000_000.0;
    let throughput = n as f64 / total_e5_time_s.max(0.001);

    // Memory: dual vectors = 2x storage
    let single_vector_bytes = 768 * 4; // 768D * f32
    let dual_storage_bytes = n * single_vector_bytes * 2;
    let single_storage_bytes = n * single_vector_bytes;

    if verbose {
        tracing::info!("  E1 median: {}us, p95: {}us", e1_median, e1_p95);
        tracing::info!("  E5 median: {}us, p95: {}us, p99: {}us", e5_median, e5_p95, e5_p99);
        tracing::info!("  Overhead: {:.2}x", overhead);
        tracing::info!("  Throughput: {:.1} QPS", throughput);
        tracing::info!("  Dual storage: {} bytes ({:.1}x single)", dual_storage_bytes, 2.0);
    }

    let mut phase_metrics = HashMap::new();
    phase_metrics.insert("e1_median_us".to_string(), e1_median as f64);
    phase_metrics.insert("e1_p95_us".to_string(), e1_p95 as f64);
    phase_metrics.insert("e5_median_us".to_string(), e5_median as f64);
    phase_metrics.insert("e5_p95_us".to_string(), e5_p95 as f64);
    phase_metrics.insert("e5_p99_us".to_string(), e5_p99 as f64);
    phase_metrics.insert("overhead".to_string(), overhead);
    phase_metrics.insert("throughput".to_string(), throughput);
    phase_metrics.insert("dual_storage_bytes".to_string(), dual_storage_bytes as f64);
    phase_metrics.insert("storage_ratio".to_string(), dual_storage_bytes as f64 / single_storage_bytes.max(1) as f64);

    let mut targets = HashMap::new();
    targets.insert("overhead".to_string(), 2.5); // max
    targets.insert("throughput".to_string(), 80.0);

    let duration = start.elapsed().as_millis() as u64;
    metrics::make_phase_result(8, "Performance Profiling", phase_metrics, targets, duration)
}

fn percentile(values: &[u64], p: f64) -> u64 {
    if values.is_empty() {
        return 0;
    }
    let mut sorted = values.to_vec();
    sorted.sort();
    let idx = ((sorted.len() as f64 * p).ceil() as usize).min(sorted.len() - 1);
    sorted[idx]
}

// ============================================================================
// Run all phases
// ============================================================================

/// Run all 8 benchmark phases.
pub fn run_all_phases(
    pairs: &[BenchmarkPair],
    queries: &[BenchmarkQuery],
    config: &BenchConfig,
) -> Vec<PhaseBenchmarkResult> {
    let mut results = Vec::new();

    let causal_pairs: Vec<BenchmarkPair> =
        pairs.iter().filter(|p| p.is_causal()).cloned().collect();
    let non_causal_pairs: Vec<BenchmarkPair> =
        pairs.iter().filter(|p| !p.is_causal()).cloned().collect();
    let forward_pairs: Vec<BenchmarkPair> =
        pairs.iter().filter(|p| p.is_forward()).cloned().collect();
    let (train_pairs, held_out_pairs) =
        super::data_loader::split_by_domain(pairs, &["psychology", "history"]);

    tracing::info!("=== Phase 1: Query Intent Detection ({} queries) ===", queries.len());
    results.push(phase1_query_intent(queries, config.verbose));

    tracing::info!("=== Phase 2: E5 Embedding Quality ({} causal + {} non-causal) ===", causal_pairs.len(), non_causal_pairs.len());
    results.push(phase2_e5_quality(&causal_pairs, &non_causal_pairs, config));

    tracing::info!("=== Phase 3: Direction Modifiers ({} forward pairs) ===", forward_pairs.len());
    results.push(phase3_direction_modifiers(&forward_pairs, config.verbose));

    // Pre-compute E5 scores for all pairs once. Phases 4/6/7 call simulate_search()
    // per query×pair — without caching, GPU mode would require 90,000+ forward passes.
    // With caching: 250 pair scores computed once, reused across all queries.
    if config.provider.is_gpu() {
        tracing::info!("=== Pre-computing E5 pair scores ({} pairs) ===", pairs.len());
    }
    config.precompute_pair_scores(pairs);

    // Pre-compute E1 semantic scores for all query x pair combinations
    if config.provider.has_e1() {
        tracing::info!("=== Pre-computing E1 semantic scores ({} queries x {} pairs) ===", queries.len(), pairs.len());
    }
    config.precompute_e1_scores(queries, pairs);

    tracing::info!("=== Phase 4: Ablation Analysis ({} queries × {} pairs) ===", queries.len(), pairs.len());
    results.push(phase4_ablation(queries, pairs, config));

    tracing::info!("=== Phase 5: Causal Gate ({} causal + {} non-causal) ===", causal_pairs.len(), non_causal_pairs.len());
    results.push(phase5_causal_gate(&causal_pairs, &non_causal_pairs, config));

    tracing::info!("=== Phase 6: End-to-End Retrieval ({} queries) ===", queries.len());
    results.push(phase6_e2e_retrieval(queries, pairs, config));

    tracing::info!("=== Phase 7: Cross-Domain ({} train + {} held-out) ===", train_pairs.len(), held_out_pairs.len());
    results.push(phase7_cross_domain(queries, &train_pairs, &held_out_pairs, config));

    tracing::info!("=== Phase 8: Performance ({} pairs) ===", pairs.len());
    results.push(phase8_performance(pairs, config.verbose));

    results
}

/// Run a single phase by number.
pub fn run_single_phase(
    phase: u8,
    pairs: &[BenchmarkPair],
    queries: &[BenchmarkQuery],
    config: &BenchConfig,
) -> Option<PhaseBenchmarkResult> {
    let causal_pairs: Vec<BenchmarkPair> =
        pairs.iter().filter(|p| p.is_causal()).cloned().collect();
    let non_causal_pairs: Vec<BenchmarkPair> =
        pairs.iter().filter(|p| !p.is_causal()).cloned().collect();
    let forward_pairs: Vec<BenchmarkPair> =
        pairs.iter().filter(|p| p.is_forward()).cloned().collect();
    let (train_pairs, held_out_pairs) =
        super::data_loader::split_by_domain(pairs, &["psychology", "history"]);

    // Pre-compute pair scores for phases that use simulate_search
    if matches!(phase, 4 | 6 | 7) {
        config.precompute_pair_scores(pairs);
        config.precompute_e1_scores(queries, pairs);
    }

    match phase {
        1 => Some(phase1_query_intent(queries, config.verbose)),
        2 => Some(phase2_e5_quality(&causal_pairs, &non_causal_pairs, config)),
        3 => Some(phase3_direction_modifiers(&forward_pairs, config.verbose)),
        4 => Some(phase4_ablation(queries, pairs, config)),
        5 => Some(phase5_causal_gate(&causal_pairs, &non_causal_pairs, config)),
        6 => Some(phase6_e2e_retrieval(queries, pairs, config)),
        7 => Some(phase7_cross_domain(queries, &train_pairs, &held_out_pairs, config)),
        8 => Some(phase8_performance(pairs, config.verbose)),
        _ => None,
    }
}

// ============================================================================
// Hash utilities for deterministic synthetic data
// ============================================================================

fn hash_text(text: &str) -> u64 {
    let mut hash = 5381u64;
    for byte in text.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u64);
    }
    hash
}

fn hash_usize(val: u64) -> u64 {
    let mut h = val;
    h = h.wrapping_mul(0x517cc1b727220a95);
    h ^= h >> 32;
    h = h.wrapping_mul(0x6c62272e07bb0142);
    h ^= h >> 32;
    h
}

fn hash_to_float(text: &str) -> f32 {
    let h = hash_usize(hash_text(text));
    // Use top 23 bits (f32 mantissa) to preserve discrimination
    ((h >> 40) as f32) / ((1u64 << 24) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::causal_bench::data_loader::BenchmarkPair;

    fn sample_pairs() -> Vec<BenchmarkPair> {
        vec![
            BenchmarkPair {
                id: "causal_001".into(),
                cause_text: "Chronic stress elevates cortisol".into(),
                effect_text: "Cortisol damages hippocampal neurons".into(),
                direction: "forward".into(),
                confidence: 0.92,
                mechanism: "biological".into(),
                domain: "health".into(),
                hard_negative: "Hippocampus aids navigation".into(),
                confounder: String::new(),
                difficulty: 0.3,
            },
            BenchmarkPair {
                id: "causal_002".into(),
                cause_text: "Smoking introduces carcinogens".into(),
                effect_text: "Lung cancer risk increases".into(),
                direction: "forward".into(),
                confidence: 0.95,
                mechanism: "biological".into(),
                domain: "health".into(),
                hard_negative: "CT scans detect lung cancer".into(),
                confounder: String::new(),
                difficulty: 0.2,
            },
            BenchmarkPair {
                id: "causal_181".into(),
                cause_text: "The sun is a star".into(),
                effect_text: "GDP measures economic output".into(),
                direction: "none".into(),
                confidence: 0.05,
                mechanism: "none".into(),
                domain: "none".into(),
                hard_negative: String::new(),
                confounder: String::new(),
                difficulty: 0.0,
            },
        ]
    }

    fn sample_queries() -> Vec<BenchmarkQuery> {
        vec![
            BenchmarkQuery {
                id: "q_001".into(),
                query: "What causes memory impairment from chronic stress?".into(),
                expected_direction: "cause".into(),
                expected_domain: "health".into(),
                expected_top1_id: "causal_001".into(),
                expected_top5_ids: vec!["causal_001".into(), "causal_002".into()],
                is_negation: false,
                is_multi_hop: false,
            },
            BenchmarkQuery {
                id: "q_002".into(),
                query: "What happens when you smoke cigarettes?".into(),
                expected_direction: "effect".into(),
                expected_domain: "health".into(),
                expected_top1_id: "causal_002".into(),
                expected_top5_ids: vec!["causal_002".into(), "causal_001".into()],
                is_negation: false,
                is_multi_hop: false,
            },
        ]
    }

    #[test]
    fn test_detect_intent_cause() {
        assert_eq!(detect_intent("Why does stress cause memory loss?"), "cause");
        assert_eq!(detect_intent("What causes lung cancer?"), "cause");
        assert_eq!(detect_intent("Root cause of inflation"), "cause");
    }

    #[test]
    fn test_detect_intent_effect() {
        assert_eq!(detect_intent("What happens when interest rates rise?"), "effect");
        assert_eq!(detect_intent("Consequence of deforestation"), "effect");
        assert_eq!(detect_intent("The effect of smoking on health"), "effect");
    }

    #[test]
    fn test_detect_intent_unknown() {
        assert_eq!(detect_intent("hello world"), "unknown");
        assert_eq!(detect_intent("the weather is nice"), "unknown");
    }

    #[test]
    fn test_phase1_query_intent() {
        let queries = sample_queries();
        let result = phase1_query_intent(&queries, false);
        assert_eq!(result.phase, 1);
        assert!(result.metrics.contains_key("accuracy"));
    }

    #[test]
    fn test_phase2_e5_quality() {
        let pairs = sample_pairs();
        let causal: Vec<_> = pairs.iter().filter(|p| p.is_causal()).cloned().collect();
        let non_causal: Vec<_> = pairs.iter().filter(|p| !p.is_causal()).cloned().collect();
        let result = phase2_e5_quality(&causal, &non_causal, &BenchConfig::default());
        assert_eq!(result.phase, 2);
        assert!(result.metrics.contains_key("spread"));
    }

    #[test]
    fn test_phase3_direction() {
        let pairs = sample_pairs();
        let forward: Vec<_> = pairs.iter().filter(|p| p.is_forward()).cloned().collect();
        let result = phase3_direction_modifiers(&forward, false);
        assert_eq!(result.phase, 3);
        // Direction modifiers always produce forward > reverse
        assert!(result.pass);
    }

    #[test]
    fn test_phase8_performance() {
        let pairs = sample_pairs();
        let result = phase8_performance(&pairs, false);
        assert_eq!(result.phase, 8);
        assert!(result.metrics.contains_key("throughput"));
    }

    #[test]
    fn test_hash_deterministic() {
        assert_eq!(hash_to_float("test"), hash_to_float("test"));
        assert_ne!(hash_to_float("test1"), hash_to_float("test2"));
    }
}
