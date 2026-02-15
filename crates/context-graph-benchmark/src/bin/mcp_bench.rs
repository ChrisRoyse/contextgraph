#![allow(dead_code, unused_variables, unreachable_code, unused_assignments)]
//! TRUE MCP Benchmark: Uses actual inject_context and search_graph MCP tools
//!
//! This benchmark ACTUALLY uses the MCP layer:
//! - inject_context: Stores documents with real 13-embedder fingerprints
//! - search_graph: Searches using real multi-space similarity from the MCP server
//!
//! Usage:
//!     cargo run -p context-graph-benchmark --bin mcp-bench --features real-embeddings
//!
//! The benchmark:
//! 1. Creates MCP handlers with real GPU embedding provider
//! 2. Injects documents via inject_context MCP tool
//! 3. Searches via search_graph MCP tool
//! 4. Compares MCP multi-space search vs E1-only cosine similarity baseline

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Challenging corpus with overlapping semantic domains
// This tests whether multi-space can distinguish between semantically similar but different topics
const CORPUS: &[(&str, &str)] = &[
    // ========== MACHINE LEARNING (Topic 0) ==========
    ("ml", "Neural networks learn hierarchical representations through backpropagation. Gradient descent optimizes weights by computing partial derivatives of the loss function with respect to each parameter."),
    ("ml", "Convolutional neural networks apply learned filters across input images. Pooling layers reduce spatial dimensions while preserving important features for classification tasks."),
    ("ml", "Recurrent neural networks maintain hidden state across sequence steps. Long short-term memory cells use gates to control information flow and prevent vanishing gradients."),
    ("ml", "Transformer architectures use self-attention to weigh input relationships. Multi-head attention allows models to focus on different aspects of the sequence simultaneously."),
    ("ml", "Batch normalization stabilizes training by normalizing layer inputs. This technique reduces internal covariate shift and allows higher learning rates."),
    ("ml", "Dropout randomly zeroes activations during training as regularization. This prevents co-adaptation of neurons and reduces overfitting on training data."),
    ("ml", "Learning rate schedulers adjust optimization step size during training. Cosine annealing and warm restarts help escape local minima in the loss landscape."),
    ("ml", "Data augmentation artificially expands training sets through transformations. Random crops, flips, and color jittering improve model generalization."),
    ("ml", "Transfer learning fine-tunes pretrained models on new tasks. Feature extraction from large pretrained networks provides strong initialization for smaller datasets."),
    ("ml", "Ensemble methods combine multiple model predictions for improved accuracy. Bagging and boosting reduce variance and bias through different aggregation strategies."),

    // ========== STATISTICS (Topic 1) - Overlaps semantically with ML ==========
    ("stats", "Maximum likelihood estimation finds parameters that maximize data probability. The log-likelihood is typically optimized for computational convenience with product distributions."),
    ("stats", "Bayesian inference updates prior beliefs with observed data through likelihood. Posterior distributions quantify parameter uncertainty given observed evidence."),
    ("stats", "Hypothesis testing evaluates claims about population parameters using sample data. P-values measure the probability of observing results as extreme under the null hypothesis."),
    ("stats", "Confidence intervals provide ranges likely to contain true population parameters. The interval width reflects estimation uncertainty at a given confidence level."),
    ("stats", "Regression analysis models relationships between dependent and independent variables. Ordinary least squares minimizes the sum of squared residuals."),
    ("stats", "Analysis of variance compares means across multiple groups simultaneously. F-tests assess whether group differences exceed expected random variation."),
    ("stats", "Principal component analysis finds orthogonal directions of maximum variance. Dimensionality reduction projects data onto these principal axes."),
    ("stats", "Cross-validation estimates model performance on unseen data. K-fold partitioning provides reliable estimates of generalization error."),
    ("stats", "Bootstrap resampling generates empirical sampling distributions from data. Percentile methods construct confidence intervals without parametric assumptions."),
    ("stats", "Regularization penalizes model complexity to prevent overfitting. Ridge and lasso add L2 and L1 penalties to the objective function respectively."),

    // ========== DATABASES (Topic 2) ==========
    ("db", "B-tree indexes provide logarithmic lookup time for range queries. Balanced tree structure maintains performance as data grows through node splitting."),
    ("db", "ACID properties ensure transaction reliability in database systems. Atomicity guarantees all-or-nothing execution while isolation prevents interference."),
    ("db", "Query optimization transforms SQL into efficient execution plans. Cost-based optimizers estimate row counts and I/O to choose join orderings."),
    ("db", "Normalization reduces data redundancy through functional dependency analysis. Third normal form eliminates transitive dependencies on non-key attributes."),
    ("db", "Write-ahead logging enables crash recovery by persisting changes before commit. Redo and undo records reconstruct consistent state after failure."),
    ("db", "Concurrency control manages simultaneous transaction access to shared data. Two-phase locking and multiversion schemes provide isolation guarantees."),
    ("db", "Distributed databases partition data across multiple nodes for scalability. Consistent hashing distributes keys evenly while minimizing reorganization."),
    ("db", "Replication provides fault tolerance by maintaining data copies across servers. Synchronous and asynchronous modes trade consistency for availability."),
    ("db", "Columnar storage organizes data by column for analytical query performance. Compression ratios improve significantly for columns with repeated values."),
    ("db", "Graph databases model entities and relationships as nodes and edges. Traversal queries efficiently follow connections without expensive joins."),

    // ========== OPERATING SYSTEMS (Topic 3) ==========
    ("os", "Process scheduling allocates CPU time among competing processes. Priority-based and fair-share algorithms balance responsiveness and throughput."),
    ("os", "Virtual memory extends physical RAM using disk-backed page files. Demand paging loads memory pages only when accessed by the process."),
    ("os", "File systems organize persistent storage into hierarchical directory structures. Inodes store metadata while data blocks hold actual file content."),
    ("os", "Interprocess communication enables data exchange between separate processes. Pipes, shared memory, and message queues offer different tradeoffs."),
    ("os", "Device drivers translate generic I/O requests into hardware-specific commands. Interrupt handlers process asynchronous device notifications efficiently."),
    ("os", "Memory protection prevents processes from accessing unauthorized address spaces. Page tables map virtual addresses with permission bits for each page."),
    ("os", "Deadlock occurs when processes hold resources while waiting for others. Prevention strategies include resource ordering and timeout-based detection."),
    ("os", "Context switching saves and restores process state during scheduling. Register contents and memory mappings must be preserved across switches."),
    ("os", "Kernel modules extend operating system functionality dynamically. Loadable drivers avoid recompilation while maintaining kernel-level access."),
    ("os", "System calls provide controlled entry points into kernel services. The trap mechanism transitions from user mode to kernel mode safely."),

    // ========== NETWORKING (Topic 4) ==========
    ("net", "TCP provides reliable ordered delivery through sequence numbers and acknowledgments. Retransmission timers detect lost packets requiring resend."),
    ("net", "IP routing forwards packets toward destinations using routing tables. Longest prefix matching selects the most specific route for each packet."),
    ("net", "DNS resolves human-readable domain names to numeric IP addresses. Hierarchical nameserver delegation distributes the global namespace."),
    ("net", "TLS encrypts network traffic using symmetric keys exchanged via public key cryptography. Certificate authorities validate server identity."),
    ("net", "Load balancers distribute requests across multiple backend servers. Health checks remove failed servers from the active pool automatically."),
    ("net", "NAT translates private IP addresses to public addresses for internet access. Port mapping enables multiple devices to share a single public IP."),
    ("net", "BGP exchanges routing information between autonomous systems on the internet. Path vector protocol prevents routing loops through AS path tracking."),
    ("net", "DHCP automatically assigns IP addresses to devices joining networks. Lease management recycles addresses when devices disconnect."),
    ("net", "HTTP defines request-response semantics for web resource transfer. Methods like GET and POST specify intended operations on resources."),
    ("net", "WebSocket enables full-duplex communication over single TCP connections. Binary and text frames support real-time bidirectional data exchange."),

    // ========== CRYPTOGRAPHY (Topic 5) ==========
    ("crypto", "Symmetric encryption uses shared secret keys for both encryption and decryption. AES block cipher processes fixed-size chunks with key-dependent permutations."),
    ("crypto", "Public key cryptography enables secure communication without shared secrets. RSA derives security from the difficulty of factoring large prime products."),
    ("crypto", "Hash functions map arbitrary data to fixed-size digests deterministically. Collision resistance prevents finding different inputs with matching outputs."),
    ("crypto", "Digital signatures provide authentication and non-repudiation for messages. Private key signing creates signatures only the key holder can produce."),
    ("crypto", "Key exchange protocols establish shared secrets over insecure channels. Diffie-Hellman leverages discrete logarithm hardness for secure key agreement."),
    ("crypto", "Authenticated encryption combines confidentiality and integrity in single primitives. GCM mode provides both encryption and message authentication efficiently."),
    ("crypto", "Random number generation supplies unpredictable bits for cryptographic keys. Hardware entropy sources and deterministic generators serve different needs."),
    ("crypto", "Key derivation functions stretch passwords into cryptographic keys securely. Salt and iteration count parameters resist precomputation attacks."),
    ("crypto", "Zero-knowledge proofs demonstrate statement truth without revealing witnesses. Interactive protocols enable verification without disclosing secret information."),
    ("crypto", "Homomorphic encryption allows computation on ciphertexts without decryption. Partially and fully homomorphic schemes support different operation sets."),

    // ========== DISTRIBUTED SYSTEMS (Topic 6) ==========
    ("dist", "Consensus protocols enable agreement among distributed nodes despite failures. Paxos and Raft ensure safety while providing progress when majorities are available."),
    ("dist", "CAP theorem states distributed systems cannot simultaneously guarantee consistency, availability, and partition tolerance. Design tradeoffs are unavoidable."),
    ("dist", "Vector clocks track causal ordering in distributed systems without synchronized clocks. Partial ordering detects concurrent events requiring conflict resolution."),
    ("dist", "Two-phase commit coordinates distributed transaction atomicity across participants. Prepared state persists until coordinator signals commit or abort."),
    ("dist", "Eventual consistency allows temporary divergence with guaranteed convergence. CRDTs merge concurrent updates without coordination overhead."),
    ("dist", "Leader election selects a single coordinator among distributed participants. Bully and ring algorithms handle failures with different communication patterns."),
    ("dist", "Gossip protocols disseminate information through random peer exchanges. Epidemic-style spreading achieves scalable eventually-consistent state sharing."),
    ("dist", "Sharding partitions data across nodes to scale beyond single-machine limits. Hash-based and range-based schemes distribute load differently."),
    ("dist", "Circuit breakers prevent cascade failures in distributed service meshes. Open state blocks requests until downstream services recover health."),
    ("dist", "Service discovery enables clients to locate available service instances. Registration and health checking maintain current endpoint inventories."),

    // ========== COMPILERS (Topic 7) ==========
    ("compiler", "Lexical analysis tokenizes source code into meaningful symbols. Regular expressions define token patterns recognized by finite automata."),
    ("compiler", "Parsing constructs syntax trees from token sequences according to grammars. Recursive descent and LR parsing handle different grammar classes."),
    ("compiler", "Type checking verifies program consistency according to language type rules. Inference algorithms deduce types without explicit annotations."),
    ("compiler", "Intermediate representations bridge source languages and target machines. SSA form simplifies dataflow analysis with unique variable definitions."),
    ("compiler", "Register allocation assigns variables to limited machine registers. Graph coloring models interference between simultaneously live values."),
    ("compiler", "Instruction selection maps intermediate operations to target machine instructions. Tree matching and dynamic programming find optimal instruction sequences."),
    ("compiler", "Loop optimizations improve performance of repeated computations. Unrolling, vectorization, and invariant hoisting reduce iteration overhead."),
    ("compiler", "Dead code elimination removes computations whose results are never used. Reachability analysis identifies code unreachable from program entry."),
    ("compiler", "Inline expansion replaces function calls with callee bodies directly. Heuristics balance code size growth against call overhead elimination."),
    ("compiler", "Link-time optimization applies whole-program analysis across compilation units. Cross-module inlining and devirtualization improve final binaries."),

    // ========== SECURITY (Topic 8) - Overlaps with crypto ==========
    ("security", "Buffer overflow exploits write beyond allocated memory boundaries. Stack canaries and address randomization provide defense in depth."),
    ("security", "SQL injection inserts malicious queries through unsanitized user input. Parameterized queries separate code from data to prevent attacks."),
    ("security", "Cross-site scripting embeds malicious scripts in web page content. Content security policies restrict script sources browsers will execute."),
    ("security", "Authentication verifies user identity through credentials or tokens. Multi-factor combines something known, possessed, and inherent."),
    ("security", "Authorization determines permitted actions for authenticated principals. Role-based and attribute-based models express access control policies."),
    ("security", "Penetration testing probes systems for exploitable vulnerabilities. Ethical hackers simulate attacks to identify security weaknesses."),
    ("security", "Intrusion detection monitors systems for signs of malicious activity. Signature and anomaly-based approaches catch different attack types."),
    ("security", "Incident response coordinates actions when security breaches occur. Containment, eradication, and recovery restore secure operational state."),
    ("security", "Secure development integrates security throughout software lifecycles. Threat modeling identifies risks early when mitigation costs less."),
    ("security", "Vulnerability disclosure coordinates flaw announcements between finders and vendors. Responsible disclosure balances transparency with remediation time."),

    // ========== ALGORITHMS (Topic 9) - Overlaps with ML and stats ==========
    ("algo", "Sorting algorithms arrange elements in specified order efficiently. Quicksort and mergesort achieve expected and worst-case O(n log n) respectively."),
    ("algo", "Graph search explores nodes reachable from starting vertices. Breadth-first and depth-first traversals have different exploration patterns."),
    ("algo", "Dynamic programming solves problems by combining subproblem solutions optimally. Memoization stores computed results to avoid redundant work."),
    ("algo", "Greedy algorithms make locally optimal choices hoping for global optima. Activity selection and Huffman coding admit greedy solutions provably."),
    ("algo", "Divide and conquer splits problems into independent subproblems recursively. Merge sort and fast Fourier transform exemplify this paradigm."),
    ("algo", "Amortized analysis averages operation costs over sequences of operations. Aggregate method and potential functions prove amortized bounds."),
    ("algo", "Approximation algorithms provide guaranteed bounds for NP-hard problems. Polynomial-time solutions achieve constant or logarithmic approximation ratios."),
    ("algo", "Randomized algorithms use random choices to achieve expected performance. Las Vegas algorithms always succeed while Monte Carlo may err."),
    ("algo", "Online algorithms process input sequentially without future knowledge. Competitive analysis compares online decisions against optimal offline choices."),
    ("algo", "Streaming algorithms process data in single passes with limited memory. Count-min sketches and HyperLogLog estimate frequencies and cardinalities."),
];

/// Benchmark result metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkMetrics {
    pub mrr: f64,
    pub precision_at_5: f64,
    pub precision_at_10: f64,
    pub recall_at_10: f64,
}

/// Clustering metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusteringMetrics {
    pub purity: f64,
}

/// Complete benchmark results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResults {
    pub timestamp: String,
    pub corpus_size: usize,
    pub num_topics: usize,
    pub num_queries: usize,

    // Core metrics
    pub mcp_multispace: BenchmarkMetrics,
    pub e1_baseline: BenchmarkMetrics,

    // Improvements
    pub improvements: Improvements,

    // Timing
    pub total_injection_time_secs: f64,
    pub total_search_time_secs: f64,
    pub avg_embedding_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Improvements {
    pub mrr_pct: f64,
    pub precision_5_pct: f64,
    pub precision_10_pct: f64,
    pub recall_10_pct: f64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!("=======================================================================");
    println!("  TRUE MCP BENCHMARK: inject_context + search_graph");
    println!("=======================================================================");
    println!();

    #[cfg(not(feature = "real-embeddings"))]
    {
        eprintln!("ERROR: This benchmark requires real GPU embeddings.");
        eprintln!("Run with: cargo run -p context-graph-benchmark --bin mcp-bench --features real-embeddings");
        std::process::exit(1);
    }

    #[cfg(feature = "real-embeddings")]
    {
        run_mcp_benchmark().await
    }
}

#[cfg(feature = "real-embeddings")]
async fn run_mcp_benchmark() -> Result<(), Box<dyn std::error::Error>> {
    use context_graph_core::monitoring::{LayerStatusProvider, StubLayerStatusProvider};
    use context_graph_core::traits::{MultiArrayEmbeddingProvider, TeleologicalMemoryStore};
    use context_graph_embeddings::{get_warm_provider, initialize_global_warm_provider};
    use context_graph_mcp::handlers::Handlers;
    use context_graph_mcp::protocol::{JsonRpcId, JsonRpcRequest};
    use context_graph_storage::teleological::RocksDbTeleologicalStore;
    use tempfile::TempDir;

    // Topic map for ground truth
    let topic_map: HashMap<&str, usize> = [
        ("ml", 0),
        ("stats", 1),
        ("db", 2),
        ("os", 3),
        ("net", 4),
        ("crypto", 5),
        ("dist", 6),
        ("compiler", 7),
        ("security", 8),
        ("algo", 9),
    ].iter().cloned().collect();

    let num_topics = topic_map.len();

    // ========================================================================
    // PHASE 1: Initialize MCP handlers with real GPU embeddings
    // ========================================================================
    println!("PHASE 1: Initializing MCP handlers with GPU embeddings");
    println!("{}", "-".repeat(70));

    let init_start = Instant::now();

    // Initialize global warm provider (loads all 13 models)
    initialize_global_warm_provider().await?;
    let multi_array_provider = get_warm_provider()?;

    // Create temporary RocksDB store
    let tempdir = TempDir::new()?;
    let db_path = tempdir.path().join("benchmark_db");
    let rocksdb_store = RocksDbTeleologicalStore::open(&db_path)?;
    let teleological_store: Arc<dyn TeleologicalMemoryStore> = Arc::new(rocksdb_store);

    let layer_status_provider: Arc<dyn LayerStatusProvider> = Arc::new(StubLayerStatusProvider);

    // Create MCP handlers - this is the REAL MCP layer
    let handlers = Handlers::with_defaults(
        teleological_store.clone(),
        multi_array_provider.clone(),
        layer_status_provider,
    );

    println!("  Handlers initialized in {:.1}s", init_start.elapsed().as_secs_f32());
    println!();

    // ========================================================================
    // PHASE 2: Inject corpus via MCP inject_context tool
    // ========================================================================
    println!("PHASE 2: Injecting corpus via MCP inject_context");
    println!("{}", "-".repeat(70));
    println!("  Corpus: {} documents across {} topics", CORPUS.len(), num_topics);
    println!();

    let inject_start = Instant::now();
    let mut injection_latencies: Vec<u128> = Vec::new();
    let mut doc_uuids: Vec<(Uuid, usize)> = Vec::new(); // (fingerprint_id, topic)

    for (i, (topic, text)) in CORPUS.iter().enumerate() {
        let topic_idx = topic_map[topic];
        let doc_start = Instant::now();

        // Create MCP request for inject_context
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "tools/call".to_string(),
            id: Some(JsonRpcId::Number(i as i64)),
            params: Some(json!({
                "name": "inject_context",
                "arguments": {
                    "content": text,
                    "rationale": format!("Benchmark document {} (topic: {})", i, topic),
                    "importance": 0.5
                }
            })),
        };

        // Dispatch to MCP handlers - this uses REAL embedding
        let response = handlers.dispatch(request).await;

        if let Some(error) = response.error {
            return Err(format!("inject_context failed: {}", error.message).into());
        }

        // Extract fingerprint ID from response
        let result = response.result.ok_or("No result from inject_context")?;
        let data = extract_mcp_tool_data(&result);
        let fingerprint_id_str = data.get("fingerprintId")
            .and_then(|v| v.as_str())
            .ok_or("No fingerprintId in response")?;
        let fingerprint_id = Uuid::parse_str(fingerprint_id_str)?;

        doc_uuids.push((fingerprint_id, topic_idx));
        injection_latencies.push(doc_start.elapsed().as_millis());

        if (i + 1) % 10 == 0 || i == CORPUS.len() - 1 {
            let avg_ms = injection_latencies.iter().sum::<u128>() as f64 / injection_latencies.len() as f64;
            print!("\r  Injected {}/{} documents ({:.0}ms avg)", i + 1, CORPUS.len(), avg_ms);
            std::io::stdout().flush()?;
        }
    }

    let total_injection_time = inject_start.elapsed();
    let avg_embedding_ms = injection_latencies.iter().sum::<u128>() as f64 / injection_latencies.len() as f64;
    println!();
    println!("  Total injection time: {:.1}s ({:.0}ms/doc)",
        total_injection_time.as_secs_f32(), avg_embedding_ms);
    println!();

    // ========================================================================
    // PHASE 3: Run retrieval benchmarks using MCP search_graph
    // ========================================================================
    println!("PHASE 3: Running retrieval benchmarks via MCP search_graph");
    println!("{}", "-".repeat(70));

    // Select one query per topic (first document from each)
    let mut query_indices: Vec<usize> = Vec::new();
    let mut seen_topics: std::collections::HashSet<usize> = std::collections::HashSet::new();
    for (i, (topic, _)) in CORPUS.iter().enumerate() {
        let topic_idx = topic_map[topic];
        if !seen_topics.contains(&topic_idx) {
            query_indices.push(i);
            seen_topics.insert(topic_idx);
        }
    }

    println!("  Running {} queries (one per topic)", query_indices.len());

    let search_start = Instant::now();
    let mut mcp_mrrs: Vec<f64> = Vec::new();
    let mut mcp_p5s: Vec<f64> = Vec::new();
    let mut mcp_p10s: Vec<f64> = Vec::new();
    let mut mcp_r10s: Vec<f64> = Vec::new();

    let mut e1_mrrs: Vec<f64> = Vec::new();
    let mut e1_p5s: Vec<f64> = Vec::new();
    let mut e1_p10s: Vec<f64> = Vec::new();
    let mut e1_r10s: Vec<f64> = Vec::new();

    for &query_idx in &query_indices {
        let (query_topic, query_text) = CORPUS[query_idx];
        let query_topic_idx = topic_map[query_topic];
        let (query_fp_id, _) = doc_uuids[query_idx];

        // Relevant docs are same topic (excluding query itself)
        let relevant: Vec<Uuid> = doc_uuids.iter()
            .filter(|(uuid, topic)| *topic == query_topic_idx && *uuid != query_fp_id)
            .map(|(uuid, _)| *uuid)
            .collect();
        let num_relevant = relevant.len();

        // ========== MCP search_graph (multi-space) ==========
        // TASK-MULTISPACE: Use multi_space strategy to enable weighted multi-embedder ranking
        let mcp_request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "tools/call".to_string(),
            id: Some(JsonRpcId::Number(1000 + query_idx as i64)),
            params: Some(json!({
                "name": "search_graph",
                "arguments": {
                    "query": query_text,
                    "topK": 20,
                    "strategy": "multi_space",
                    "weightProfile": "semantic_search"
                }
            })),
        };

        let mcp_response = handlers.dispatch(mcp_request).await;
        if let Some(error) = mcp_response.error {
            println!("\n  Warning: search_graph failed for query {}: {}", query_idx, error.message);
            continue;
        }

        let mcp_result = mcp_response.result.ok_or("No result from search_graph")?;
        let mcp_data = extract_mcp_tool_data(&mcp_result);
        let mcp_results_array = mcp_data.get("results")
            .and_then(|v| v.as_array())
            .ok_or("No results array")?;

        // Extract fingerprint IDs from search results (excluding query itself)
        let mcp_top_results: Vec<Uuid> = mcp_results_array.iter()
            .filter_map(|r| r.get("fingerprintId").and_then(|v| v.as_str()))
            .filter_map(|s| Uuid::parse_str(s).ok())
            .filter(|uuid| *uuid != query_fp_id)
            .take(10)
            .collect();

        let mcp_top_5: Vec<Uuid> = mcp_top_results.iter().take(5).cloned().collect();

        // Compute MCP metrics
        mcp_mrrs.push(compute_mrr(&mcp_top_results, &relevant));
        mcp_p5s.push(precision_at_k(&mcp_top_5, &relevant));
        mcp_p10s.push(precision_at_k(&mcp_top_results, &relevant));
        mcp_r10s.push(recall_at_k(&mcp_top_results, &relevant, num_relevant));

        // ========== E1 Baseline (single-embedder) ==========
        // Get E1 embedding for query and compute cosine similarity against all stored fingerprints
        let query_embedding = multi_array_provider.embed_all(query_text).await?;
        let query_e1 = &query_embedding.fingerprint.e1_semantic;

        // Get all stored fingerprints and compute E1-only similarity
        let mut e1_results: Vec<(Uuid, f32)> = Vec::new();
        for (fp_id, _) in &doc_uuids {
            if *fp_id == query_fp_id {
                continue;
            }
            // Retrieve fingerprint from store
            if let Ok(Some(fp)) = teleological_store.retrieve(*fp_id).await {
                let e1_sim = cosine_similarity(query_e1, &fp.semantic.e1_semantic);
                e1_results.push((*fp_id, e1_sim));
            }
        }
        e1_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        let e1_top_10: Vec<Uuid> = e1_results.iter().take(10).map(|(u, _)| *u).collect();
        let e1_top_5: Vec<Uuid> = e1_results.iter().take(5).map(|(u, _)| *u).collect();

        // Compute E1 baseline metrics
        e1_mrrs.push(compute_mrr(&e1_top_10, &relevant));
        e1_p5s.push(precision_at_k(&e1_top_5, &relevant));
        e1_p10s.push(precision_at_k(&e1_top_10, &relevant));
        e1_r10s.push(recall_at_k(&e1_top_10, &relevant, num_relevant));
    }

    let total_search_time = search_start.elapsed();
    println!("  Search complete in {:.1}s", total_search_time.as_secs_f32());
    println!();

    // ========================================================================
    // PHASE 4: Compute results
    // ========================================================================
    let mcp_metrics = BenchmarkMetrics {
        mrr: mean(&mcp_mrrs),
        precision_at_5: mean(&mcp_p5s),
        precision_at_10: mean(&mcp_p10s),
        recall_at_10: mean(&mcp_r10s),
    };

    let e1_metrics = BenchmarkMetrics {
        mrr: mean(&e1_mrrs),
        precision_at_5: mean(&e1_p5s),
        precision_at_10: mean(&e1_p10s),
        recall_at_10: mean(&e1_r10s),
    };

    let improvements = Improvements {
        mrr_pct: pct_improvement(e1_metrics.mrr, mcp_metrics.mrr),
        precision_5_pct: pct_improvement(e1_metrics.precision_at_5, mcp_metrics.precision_at_5),
        precision_10_pct: pct_improvement(e1_metrics.precision_at_10, mcp_metrics.precision_at_10),
        recall_10_pct: pct_improvement(e1_metrics.recall_at_10, mcp_metrics.recall_at_10),
    };

    let results = BenchmarkResults {
        timestamp: Utc::now().to_rfc3339(),
        corpus_size: CORPUS.len(),
        num_topics,
        num_queries: query_indices.len(),
        mcp_multispace: mcp_metrics.clone(),
        e1_baseline: e1_metrics.clone(),
        improvements: improvements.clone(),
        total_injection_time_secs: total_injection_time.as_secs_f64(),
        total_search_time_secs: total_search_time.as_secs_f64(),
        avg_embedding_ms,
    };

    // ========================================================================
    // PHASE 5: Print results and save reports
    // ========================================================================
    println!();
    println!("=======================================================================");
    println!("  BENCHMARK RESULTS");
    println!("=======================================================================");
    println!();
    println!("Configuration:");
    println!("  Corpus: {} documents", results.corpus_size);
    println!("  Topics: {} (overlapping semantic domains)", results.num_topics);
    println!("  Queries: {}", results.num_queries);
    println!();

    println!("RETRIEVAL METRICS (MCP search_graph vs E1 cosine similarity)");
    println!("{}", "-".repeat(70));
    println!("{:<20} {:>15} {:>15} {:>15}", "Metric", "E1 Baseline", "MCP Multi-Space", "Improvement");
    println!("{:<20} {:>15.3} {:>15.3} {:>+14.1}%", "MRR",
        e1_metrics.mrr, mcp_metrics.mrr, improvements.mrr_pct);
    println!("{:<20} {:>15.3} {:>15.3} {:>+14.1}%", "Precision@5",
        e1_metrics.precision_at_5, mcp_metrics.precision_at_5, improvements.precision_5_pct);
    println!("{:<20} {:>15.3} {:>15.3} {:>+14.1}%", "Precision@10",
        e1_metrics.precision_at_10, mcp_metrics.precision_at_10, improvements.precision_10_pct);
    println!("{:<20} {:>15.3} {:>15.3} {:>+14.1}%", "Recall@10",
        e1_metrics.recall_at_10, mcp_metrics.recall_at_10, improvements.recall_10_pct);
    println!();

    let avg_improvement = (improvements.mrr_pct + improvements.precision_5_pct +
        improvements.precision_10_pct + improvements.recall_10_pct) / 4.0;
    let winner = if avg_improvement > 0.0 { "MCP MULTI-SPACE" } else { "E1 BASELINE" };

    println!("=======================================================================");
    println!("  WINNER: {} (average improvement: {:+.1}%)", winner, avg_improvement);
    println!("=======================================================================");
    println!();

    // Save reports
    save_reports(&results)?;

    // Keep tempdir alive until end
    drop(tempdir);

    Ok(())
}

fn extract_mcp_tool_data(result: &serde_json::Value) -> serde_json::Value {
    if let Some(is_error) = result.get("isError").and_then(|v| v.as_bool()) {
        if is_error {
            panic!("MCP tool returned error: {:?}", result);
        }
    }

    if let Some(content) = result.get("content").and_then(|v| v.as_array()) {
        let text = content[0]
            .get("text")
            .and_then(|v| v.as_str())
            .expect("content[0] must have text field");
        serde_json::from_str(text).expect("text field must be valid JSON")
    } else {
        result.clone()
    }
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

fn compute_mrr(results: &[Uuid], relevant: &[Uuid]) -> f64 {
    for (i, uuid) in results.iter().enumerate() {
        if relevant.contains(uuid) {
            return 1.0 / (i + 1) as f64;
        }
    }
    0.0
}

fn precision_at_k(results: &[Uuid], relevant: &[Uuid]) -> f64 {
    if results.is_empty() {
        return 0.0;
    }
    let hits = results.iter().filter(|u| relevant.contains(u)).count();
    hits as f64 / results.len() as f64
}

fn recall_at_k(results: &[Uuid], relevant: &[Uuid], total_relevant: usize) -> f64 {
    if total_relevant == 0 {
        return 0.0;
    }
    let hits = results.iter().filter(|u| relevant.contains(u)).count();
    hits as f64 / total_relevant as f64
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() { 0.0 } else { values.iter().sum::<f64>() / values.len() as f64 }
}

fn pct_improvement(baseline: f64, improved: f64) -> f64 {
    if baseline.abs() < 0.001 { 0.0 } else { (improved - baseline) / baseline * 100.0 }
}

fn save_reports(results: &BenchmarkResults) -> Result<(), Box<dyn std::error::Error>> {
    let docs_dir = Path::new("./docs");
    fs::create_dir_all(docs_dir)?;

    // JSON report
    let json_path = docs_dir.join("benchmark-results.json");
    let json_content = serde_json::to_string_pretty(results)?;
    let mut json_file = File::create(&json_path)?;
    json_file.write_all(json_content.as_bytes())?;
    println!("JSON report saved to: {}", json_path.display());

    // Markdown report
    let md_path = docs_dir.join("BENCHMARK_REPORT.md");
    let md_content = generate_markdown_report(results);
    let mut md_file = File::create(&md_path)?;
    md_file.write_all(md_content.as_bytes())?;
    println!("Markdown report saved to: {}", md_path.display());

    Ok(())
}

fn generate_markdown_report(results: &BenchmarkResults) -> String {
    let avg_improvement = (results.improvements.mrr_pct + results.improvements.precision_5_pct +
        results.improvements.precision_10_pct + results.improvements.recall_10_pct) / 4.0;
    let winner = if avg_improvement > 0.0 { "MCP Multi-Space" } else { "E1 Baseline" };

    format!(r#"# Context Graph MCP Benchmark Report

## TRUE MCP Benchmark: inject_context + search_graph

**Generated:** {}
**Winner:** {} ({:+.1}% average improvement)

---

## Executive Summary

This benchmark uses the **actual MCP tools** to compare retrieval approaches:

1. **E1 Baseline**: Single-embedder cosine similarity (traditional RAG)
2. **MCP Multi-Space**: Full search_graph with weighted 13-embedder similarity

### Key Differences from Previous Benchmark

- **Previous**: Bypassed MCP, computed similarity directly
- **Current**: Actually calls inject_context and search_graph MCP tools
- **Corpus**: Semantically overlapping topics (ML/Stats, Crypto/Security, etc.)

---

## Benchmark Configuration

| Parameter | Value |
|-----------|-------|
| Corpus Size | {} documents |
| Topics | {} (with semantic overlap) |
| Queries | {} |
| Injection Time | {:.1}s |
| Search Time | {:.1}s |
| Avg Embedding | {:.0}ms/doc |

---

## Topic Overlap Design

The corpus deliberately includes semantically similar topics to challenge single-embedder retrieval:

| Topic Pair | Why Challenging |
|------------|-----------------|
| ML ↔ Statistics | Shared vocabulary (optimization, variance, models) |
| Crypto ↔ Security | Overlapping concepts (encryption, authentication) |
| Algorithms ↔ ML | Similar techniques (optimization, gradient descent) |
| OS ↔ Distributed | Shared concerns (scheduling, concurrency) |

---

## Retrieval Results

| Metric | E1 Baseline | MCP Multi-Space | Improvement |
|--------|-------------|-----------------|-------------|
| **MRR** | {:.3} | {:.3} | {:+.1}% |
| **Precision@5** | {:.3} | {:.3} | {:+.1}% |
| **Precision@10** | {:.3} | {:.3} | {:+.1}% |
| **Recall@10** | {:.3} | {:.3} | {:+.1}% |

---

## Analysis

### Why MCP Multi-Space {}

{}

---

## Technical Details

### MCP inject_context
- Embeds content with all 13 embedders
- Stores TeleologicalFingerprint in RocksDB
- Computes UTL metrics (entropy, coherence, surprise)

### MCP search_graph
- Embeds query with all 13 embedders
- Searches using weighted multi-space similarity
- Returns results ranked by combined similarity score

### E1 Baseline
- Uses only e1_semantic (1024D) embedding
- Pure cosine similarity ranking
- No additional embedder signals

---

*Report generated by Context Graph TRUE MCP Benchmark*
"#,
        results.timestamp,
        winner, avg_improvement,
        results.corpus_size,
        results.num_topics,
        results.num_queries,
        results.total_injection_time_secs,
        results.total_search_time_secs,
        results.avg_embedding_ms,
        results.e1_baseline.mrr, results.mcp_multispace.mrr, results.improvements.mrr_pct,
        results.e1_baseline.precision_at_5, results.mcp_multispace.precision_at_5, results.improvements.precision_5_pct,
        results.e1_baseline.precision_at_10, results.mcp_multispace.precision_at_10, results.improvements.precision_10_pct,
        results.e1_baseline.recall_at_10, results.mcp_multispace.recall_at_10, results.improvements.recall_10_pct,
        if avg_improvement > 0.0 { "Wins" } else { "Loses" },
        if avg_improvement > 0.0 {
            "The multi-space similarity combines signals from E1 (semantic), E5 (causal), E7 (code), and E10 (multimodal) to distinguish between semantically similar but different topics.\n\nWhen documents about \"neural network optimization\" and \"statistical optimization\" have similar E1 embeddings, the other embedders help differentiate them."
        } else {
            "In this specific corpus and query set, E1 semantic embedding alone was sufficient to distinguish between topics. This may indicate the topics aren't overlapping enough to challenge single-embedder retrieval."
        }
    )
}
