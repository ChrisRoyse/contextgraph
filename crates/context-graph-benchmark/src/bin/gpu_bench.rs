#![allow(dead_code, unused_variables, unreachable_code, unused_assignments)]
//! GPU Benchmark: Multi-space (13 embedders) vs Single-embedder (E1 only)
//!
//! This benchmark uses real GPU embeddings to compare:
//! - Multi-space: Full 13-embedder fingerprints via warm provider
//! - Single-embedder: E1 semantic embedding only
//!
//! Usage:
//!     cargo run -p context-graph-benchmark --bin gpu-bench --features real-embeddings
//!
//! Requirements:
//!     - CUDA-enabled GPU
//!     - Models loaded in ./models directory
//!     - ~16GB VRAM for all 13 models

use std::collections::HashMap;
use uuid::Uuid;

use context_graph_core::types::fingerprint::SemanticFingerprint;

// Test corpus - diverse topics for meaningful clustering
// Larger corpus with more overlapping semantics to challenge single-embedder
const TEST_CORPUS: &[(&str, &str)] = &[
    // Science - Physics (topic 0)
    ("science_physics", "Quantum mechanics describes the behavior of particles at the atomic and subatomic level. The uncertainty principle states we cannot simultaneously know position and momentum."),
    ("science_physics", "The theory of general relativity describes gravity as the curvature of spacetime caused by mass and energy. Black holes are regions where this curvature becomes extreme."),
    ("science_physics", "The Standard Model of particle physics describes fundamental particles and forces. Quarks combine to form protons and neutrons held together by the strong force."),
    ("science_physics", "String theory proposes that fundamental particles are one-dimensional vibrating strings. This theory attempts to unify quantum mechanics with general relativity."),
    ("science_physics", "Dark matter makes up approximately 27% of the universe but does not interact with light. Scientists detect it only through gravitational effects on visible matter."),

    // Science - Biology (topic 1)
    ("science_biology", "DNA carries genetic instructions for all living organisms. The double helix structure was discovered by Watson and Crick using X-ray crystallography data."),
    ("science_biology", "Photosynthesis converts sunlight, water, and carbon dioxide into glucose and oxygen. Chlorophyll in plant cells absorbs light energy to drive this process."),
    ("science_biology", "CRISPR gene editing allows precise modification of DNA sequences. This technology has revolutionary applications in medicine, agriculture, and research."),
    ("science_biology", "Evolution through natural selection explains how species adapt to environments over time. Genetic mutations and reproductive success drive these changes."),
    ("science_biology", "The human microbiome contains trillions of bacteria that play crucial roles in digestion, immunity, and even mental health through the gut-brain axis."),

    // Technology - AI/ML (topic 2)
    ("tech_ai", "Machine learning algorithms learn patterns from data without explicit programming. Neural networks mimic biological neurons to process information in layers."),
    ("tech_ai", "Deep learning uses multiple neural network layers to learn hierarchical representations. Convolutional networks excel at image recognition tasks."),
    ("tech_ai", "Transformer architectures revolutionized natural language processing. Attention mechanisms allow models to focus on relevant parts of input sequences."),
    ("tech_ai", "Reinforcement learning trains agents through rewards and penalties. AlphaGo used this approach to master the complex game of Go."),
    ("tech_ai", "Large language models like GPT demonstrate emergent capabilities from scale. Few-shot learning allows these models to perform novel tasks without explicit training."),

    // Technology - Infrastructure (topic 3)
    ("tech_infra", "Cloud computing provides on-demand access to computing resources over the internet. Services like AWS and Azure offer scalable infrastructure and platform services."),
    ("tech_infra", "Kubernetes orchestrates container deployment across clusters of machines. This enables automatic scaling, load balancing, and self-healing of applications."),
    ("tech_infra", "Microservices architecture decomposes applications into small, independent services. Each service can be developed, deployed, and scaled independently."),
    ("tech_infra", "DevOps practices combine software development and IT operations. Continuous integration and deployment pipelines automate the release process."),
    ("tech_infra", "Edge computing processes data closer to where it is generated rather than in centralized data centers. This reduces latency for real-time applications."),

    // History - Ancient (topic 4)
    ("history_ancient", "The Roman Empire at its peak controlled most of Europe, North Africa, and the Middle East. Its legacy includes law, engineering, and language."),
    ("history_ancient", "Ancient Egypt built pyramids as tombs for pharaohs using sophisticated engineering. The Great Pyramid of Giza remains one of the Seven Wonders."),
    ("history_ancient", "Greek philosophy with Socrates, Plato, and Aristotle established foundations of Western thought. Their ideas on ethics, politics, and metaphysics endure today."),
    ("history_ancient", "The Silk Road connected East and West facilitating trade in goods, ideas, and technologies. This network stretched from China to the Mediterranean."),
    ("history_ancient", "Mesopotamian civilizations developed writing, mathematics, and astronomy. Cuneiform tablets preserve records of early human civilization."),

    // History - Modern (topic 5)
    ("history_modern", "The Industrial Revolution transformed economies from agrarian to manufacturing. Steam power and mechanization dramatically increased production capabilities."),
    ("history_modern", "World War II was the deadliest conflict in human history with over 70 million casualties. It reshaped global politics and led to the United Nations."),
    ("history_modern", "The Cold War was a period of geopolitical tension between the United States and Soviet Union. It shaped international relations from 1947 to 1991."),
    ("history_modern", "The civil rights movement in America fought against racial segregation and discrimination. Martin Luther King Jr.'s leadership helped pass landmark legislation."),
    ("history_modern", "The Space Race between superpowers culminated in the Apollo 11 moon landing. This achievement demonstrated the potential of human exploration and technology."),

    // Programming - Systems (topic 6)
    ("programming_systems", "Rust provides memory safety without garbage collection through its ownership system. The borrow checker prevents data races at compile time."),
    ("programming_systems", "Operating systems manage hardware resources and provide abstractions for applications. The kernel handles process scheduling, memory management, and I/O operations."),
    ("programming_systems", "Concurrent programming requires careful handling of shared state. Mutexes, semaphores, and channels help coordinate access between threads."),
    ("programming_systems", "WebAssembly enables near-native performance in web browsers. This portable binary format allows languages like Rust and C++ to run in the browser."),
    ("programming_systems", "Database systems use B-trees and LSM trees for efficient storage and retrieval. ACID properties ensure transaction reliability in critical applications."),

    // Programming - Web (topic 7)
    ("programming_web", "React pioneered the component-based UI paradigm in web development. Virtual DOM diffing enables efficient updates to the browser's DOM."),
    ("programming_web", "GraphQL provides a query language for APIs that lets clients request exactly the data they need. This reduces over-fetching compared to REST APIs."),
    ("programming_web", "WebSockets enable bidirectional real-time communication between browsers and servers. This is essential for chat applications and live updates."),
    ("programming_web", "Server-side rendering improves initial page load performance and SEO. Frameworks like Next.js combine SSR with client-side hydration."),
    ("programming_web", "Progressive web apps combine web and native app capabilities. Service workers enable offline functionality and push notifications."),

    // Finance (topic 8)
    ("finance", "Blockchain is a distributed ledger technology that records transactions across many computers. Bitcoin was the first cryptocurrency to use this technology."),
    ("finance", "Algorithmic trading uses computer programs to execute trades based on predefined strategies. High-frequency trading operates on millisecond timescales."),
    ("finance", "Risk management involves identifying, assessing, and prioritizing potential losses. Derivatives like options and futures help hedge against market volatility."),
    ("finance", "Central banks use monetary policy to influence economic conditions. Interest rate adjustments and quantitative easing affect inflation and employment."),
    ("finance", "Decentralized finance DeFi aims to recreate traditional financial services on blockchain. Smart contracts enable automated lending, borrowing, and trading."),

    // Environment (topic 9)
    ("environment", "Climate change is caused by greenhouse gas emissions from human activities. Rising temperatures lead to sea level rise, extreme weather, and ecosystem disruption."),
    ("environment", "Renewable energy sources like solar and wind are replacing fossil fuels. Battery storage technology enables integration of intermittent generation sources."),
    ("environment", "Coral reefs are underwater ecosystems built by coral polyps that provide habitat for diverse marine life. Climate change and ocean acidification threaten their survival."),
    ("environment", "The Amazon rainforest produces about 20 percent of the world's oxygen. Deforestation threatens this vital ecosystem and its biodiversity."),
    ("environment", "Sustainable agriculture practices reduce environmental impact while maintaining productivity. Crop rotation, cover crops, and reduced tillage improve soil health."),
];

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("=== GPU Benchmark: Multi-space vs Single-embedder ===");
    println!();

    // Check if real-embeddings feature is enabled
    #[cfg(not(feature = "real-embeddings"))]
    {
        eprintln!("ERROR: This benchmark requires real GPU embeddings.");
        eprintln!("Run with: cargo run -p context-graph-benchmark --bin gpu-bench --features real-embeddings");
        std::process::exit(1);
    }

    #[cfg(feature = "real-embeddings")]
    {
        run_benchmark().await
    }
}

#[cfg(feature = "real-embeddings")]
async fn run_benchmark() -> Result<(), Box<dyn std::error::Error>> {
    use context_graph_embeddings::{initialize_global_warm_provider, get_warm_provider};

    println!("Initializing warm embedding provider (loading 13 models to GPU)...");
    println!("This may take 20-30 seconds on first run.");
    println!();

    let init_start = Instant::now();
    initialize_global_warm_provider().await?;
    println!("Initialization complete in {:.1}s", init_start.elapsed().as_secs_f32());
    println!();

    // Get the warm provider
    let provider = get_warm_provider()?;

    // Build corpus with ground truth
    println!("Building test corpus ({} documents, 10 topics)...", TEST_CORPUS.len());
    let topic_map: HashMap<&str, usize> = [
        ("science_physics", 0),
        ("science_biology", 1),
        ("tech_ai", 2),
        ("tech_infra", 3),
        ("history_ancient", 4),
        ("history_modern", 5),
        ("programming_systems", 6),
        ("programming_web", 7),
        ("finance", 8),
        ("environment", 9),
    ].iter().cloned().collect();

    let mut documents: Vec<(Uuid, String, usize)> = Vec::new();
    for (i, (topic, text)) in TEST_CORPUS.iter().enumerate() {
        let uuid = Uuid::from_u128(i as u128);
        let topic_idx = topic_map[topic];
        documents.push((uuid, text.to_string(), topic_idx));
    }

    // ========================================================================
    // Generate embeddings with full 13-embedder pipeline
    // ========================================================================
    println!();
    println!("Generating multi-space embeddings (all 13 embedders)...");

    let multi_start = Instant::now();
    let mut multi_fingerprints: HashMap<Uuid, SemanticFingerprint> = HashMap::new();

    for (i, (uuid, text, _topic)) in documents.iter().enumerate() {
        print!("\r  Embedding {}/{}", i + 1, documents.len());
        std::io::stdout().flush()?;

        let output = provider.embed_all(text).await?;
        multi_fingerprints.insert(*uuid, output.fingerprint);
    }
    let multi_elapsed = multi_start.elapsed();
    println!("\r  Complete: {:.2}s total, {:.0}ms per doc",
        multi_elapsed.as_secs_f32(),
        multi_elapsed.as_millis() as f64 / documents.len() as f64);

    // ========================================================================
    // Extract E1 (semantic) only for single-embedder baseline
    // ========================================================================
    println!();
    println!("Extracting single-embedder baseline (E1 semantic only)...");

    let mut single_embeddings: HashMap<Uuid, Vec<f32>> = HashMap::new();
    for (uuid, fp) in &multi_fingerprints {
        single_embeddings.insert(*uuid, fp.e1_semantic.clone());
    }
    println!("  Complete: {} E1 embeddings extracted", single_embeddings.len());

    // ========================================================================
    // Run retrieval benchmarks
    // ========================================================================
    println!();
    println!("Running retrieval benchmarks...");

    // Use 10 queries (one from each topic)
    let query_indices = [0, 5, 10, 15, 20, 25, 30, 35, 40, 45]; // First doc from each topic
    let k = 10;

    let mut single_mrr = 0.0;
    let mut single_precision = 0.0;
    let mut multi_mrr = 0.0;
    let mut multi_precision = 0.0;

    for &query_idx in &query_indices {
        let (query_uuid, _, query_topic) = &documents[query_idx];
        let query_fp = &multi_fingerprints[query_uuid];
        let query_e1 = &single_embeddings[query_uuid];

        // Relevant docs are those with same topic (excluding query)
        let relevant: Vec<Uuid> = documents.iter()
            .filter(|(uuid, _, topic)| *topic == *query_topic && uuid != query_uuid)
            .map(|(uuid, _, _)| *uuid)
            .collect();

        // Single-embedder retrieval (E1 cosine similarity)
        let mut single_results: Vec<(Uuid, f32)> = single_embeddings.iter()
            .filter(|(uuid, _)| **uuid != *query_uuid)
            .map(|(uuid, emb)| (*uuid, cosine_similarity(query_e1, emb)))
            .collect();
        single_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // Multi-space retrieval (weighted combination of key embedders)
        let mut multi_results: Vec<(Uuid, f32)> = multi_fingerprints.iter()
            .filter(|(uuid, _)| **uuid != *query_uuid)
            .map(|(uuid, fp)| (*uuid, multi_space_similarity(query_fp, fp)))
            .collect();
        multi_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // Compute metrics
        let single_top_k: Vec<Uuid> = single_results.iter().take(k).map(|(u, _)| *u).collect();
        let multi_top_k: Vec<Uuid> = multi_results.iter().take(k).map(|(u, _)| *u).collect();

        // MRR
        for (i, uuid) in single_top_k.iter().enumerate() {
            if relevant.contains(uuid) {
                single_mrr += 1.0 / (i + 1) as f64;
                break;
            }
        }
        for (i, uuid) in multi_top_k.iter().enumerate() {
            if relevant.contains(uuid) {
                multi_mrr += 1.0 / (i + 1) as f64;
                break;
            }
        }

        // Precision@K
        let single_hits = single_top_k.iter().filter(|u| relevant.contains(u)).count();
        let multi_hits = multi_top_k.iter().filter(|u| relevant.contains(u)).count();
        single_precision += single_hits as f64 / k as f64;
        multi_precision += multi_hits as f64 / k as f64;
    }

    let n_queries = query_indices.len() as f64;
    single_mrr /= n_queries;
    single_precision /= n_queries;
    multi_mrr /= n_queries;
    multi_precision /= n_queries;

    // ========================================================================
    // Run clustering benchmarks (simple k-means style)
    // ========================================================================
    println!();
    println!("Running clustering benchmarks...");

    // Single-embedder clustering
    let single_clusters = cluster_embeddings(&single_embeddings, 10);
    let single_purity = compute_purity(&documents, &single_clusters);

    // Multi-space clustering (concatenated key embeddings)
    let multi_combined: HashMap<Uuid, Vec<f32>> = multi_fingerprints.iter()
        .map(|(uuid, fp)| {
            let mut combined = Vec::with_capacity(384 * 4);
            // Take first 384 dims from E1, E5, E7, E10 (semantic embedders)
            combined.extend(fp.e1_semantic.iter().take(384).cloned());
            combined.extend(fp.e5_causal.iter().take(384).cloned());
            combined.extend(fp.e7_code.iter().take(384).cloned());
            combined.extend(fp.e10_multimodal_paraphrase.iter().take(384).cloned());
            (*uuid, combined)
        })
        .collect();
    let multi_clusters = cluster_embeddings(&multi_combined, 10);
    let multi_purity = compute_purity(&documents, &multi_clusters);

    // ========================================================================
    // Print results
    // ========================================================================
    println!();
    println!("{}", "=".repeat(60));
    println!("BENCHMARK RESULTS");
    println!("{}", "=".repeat(60));
    println!();
    println!("Corpus: {} documents, 10 topics", documents.len());
    println!("Queries: {}", query_indices.len());
    println!("K: {}", k);
    println!();

    println!("RETRIEVAL METRICS");
    println!("{}", "-".repeat(40));
    println!("{:<25} {:>10} {:>10}", "", "Single", "Multi");
    println!("{:<25} {:>10.3} {:>10.3}", "MRR:", single_mrr, multi_mrr);
    println!("{:<25} {:>10.3} {:>10.3}", "Precision@10:", single_precision, multi_precision);
    println!();

    println!("CLUSTERING METRICS");
    println!("{}", "-".repeat(40));
    println!("{:<25} {:>10.3} {:>10.3}", "Purity:", single_purity, multi_purity);
    println!();

    println!("IMPROVEMENTS (Multi-space vs Single-embedder)");
    println!("{}", "-".repeat(40));
    let mrr_improvement = (multi_mrr - single_mrr) / single_mrr.max(0.001) * 100.0;
    let precision_improvement = (multi_precision - single_precision) / single_precision.max(0.001) * 100.0;
    let purity_improvement = (multi_purity - single_purity) / single_purity.max(0.001) * 100.0;

    println!("{:<25} {:>+10.1}%", "MRR:", mrr_improvement);
    println!("{:<25} {:>+10.1}%", "Precision@10:", precision_improvement);
    println!("{:<25} {:>+10.1}%", "Purity:", purity_improvement);
    println!();

    let avg_improvement = (mrr_improvement + precision_improvement + purity_improvement) / 3.0;
    let winner = if avg_improvement > 0.0 { "MULTI-SPACE" } else { "SINGLE-EMBEDDER" };

    println!("WINNER: {} (avg improvement: {:+.1}%)", winner, avg_improvement);
    println!();

    println!("TIMING");
    println!("{}", "-".repeat(40));
    println!("Multi-space embedding: {:.2}s total, {:.0}ms/doc",
        multi_elapsed.as_secs_f32(),
        multi_elapsed.as_millis() as f64 / documents.len() as f64);

    Ok(())
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a < f32::EPSILON || norm_b < f32::EPSILON {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

fn multi_space_similarity(a: &SemanticFingerprint, b: &SemanticFingerprint) -> f32 {
    // Weighted combination of semantic embedders (per CLAUDE.md categories)
    // SEMANTIC: E1, E5, E6, E7, E10, E12, E13 (weight 1.0)
    // Using E1, E5, E7, E10 for dense comparison
    let e1_sim = cosine_similarity(&a.e1_semantic, &b.e1_semantic);
    let e5_sim = cosine_similarity(&a.e5_causal, &b.e5_causal);
    let e7_sim = cosine_similarity(&a.e7_code, &b.e7_code);
    let e10_sim = cosine_similarity(&a.e10_multimodal_paraphrase, &b.e10_multimodal_paraphrase);

    // Weighted average (semantic embedders)
    0.35 * e1_sim + 0.25 * e5_sim + 0.25 * e7_sim + 0.15 * e10_sim
}

fn cluster_embeddings(embeddings: &HashMap<Uuid, Vec<f32>>, k: usize) -> HashMap<Uuid, usize> {
    use rand::prelude::*;
    use rand_chacha::ChaCha8Rng;

    if embeddings.is_empty() || k == 0 {
        return HashMap::new();
    }

    let mut rng = ChaCha8Rng::seed_from_u64(42);
    let items: Vec<(Uuid, &Vec<f32>)> = embeddings.iter().map(|(u, e)| (*u, e)).collect();
    let dim = items[0].1.len();

    // Initialize centroids
    let mut indices: Vec<usize> = (0..items.len()).collect();
    indices.shuffle(&mut rng);
    let centroid_indices: Vec<usize> = indices.into_iter().take(k).collect();
    let mut centroids: Vec<Vec<f32>> = centroid_indices.iter()
        .map(|&i| items[i].1.clone())
        .collect();

    // K-means iterations
    let mut assignments: HashMap<Uuid, usize> = HashMap::new();

    for _ in 0..10 {
        assignments.clear();

        // Assign to nearest centroid
        for (uuid, emb) in &items {
            let mut best_cluster = 0;
            let mut best_sim = f32::NEG_INFINITY;
            for (i, centroid) in centroids.iter().enumerate() {
                let sim = cosine_similarity(emb, centroid);
                if sim > best_sim {
                    best_sim = sim;
                    best_cluster = i;
                }
            }
            assignments.insert(*uuid, best_cluster);
        }

        // Update centroids
        let mut new_centroids: Vec<Vec<f32>> = vec![vec![0.0; dim]; k];
        let mut counts: Vec<usize> = vec![0; k];

        for (uuid, emb) in &items {
            if let Some(&cluster) = assignments.get(uuid) {
                for (i, val) in emb.iter().enumerate() {
                    new_centroids[cluster][i] += val;
                }
                counts[cluster] += 1;
            }
        }

        for (cluster, centroid) in new_centroids.iter_mut().enumerate() {
            if counts[cluster] > 0 {
                for val in centroid.iter_mut() {
                    *val /= counts[cluster] as f32;
                }
            }
        }

        centroids = new_centroids;
    }

    assignments
}

fn compute_purity(documents: &[(Uuid, String, usize)], clusters: &HashMap<Uuid, usize>) -> f64 {
    // Map cluster -> topic -> count
    let mut cluster_topics: HashMap<usize, HashMap<usize, usize>> = HashMap::new();

    for (uuid, _, topic) in documents {
        if let Some(&cluster) = clusters.get(uuid) {
            *cluster_topics.entry(cluster).or_default().entry(*topic).or_default() += 1;
        }
    }

    let mut correct = 0;
    let mut total = 0;

    for topic_counts in cluster_topics.values() {
        if let Some(&max_count) = topic_counts.values().max() {
            correct += max_count;
        }
        total += topic_counts.values().sum::<usize>();
    }

    if total > 0 { correct as f64 / total as f64 } else { 0.0 }
}
