# 4. Context Graph: A 13-Embedder Realisation of DDA on Text

## 4.1 Overview

Context Graph is a ~370,000-line Rust workspace spanning ten crates that implements Derived Data Abundance (DDA) for text. Every stored memory is simultaneously embedded through thirteen independent embedding models (E1–E13), producing a ~46 KB *SemanticFingerprint* that captures the memory across dense, sparse, asymmetric, temporal, token-level, and hyper-dimensional spaces. All thirteen vectors plus thirteen-element topic profiles, seventy-eight synergy-weighted cross-correlations, and six hierarchical group alignments are persisted to a RocksDB database with fifty-nine column families. Retrieval is exposed as 67–75 tools (depending on optional feature flags) over the Model Context Protocol (Anthropic, 2024), making the derived representation directly consumable by AI assistants.

The design philosophy follows three constitutional rules:

- **ARCH-01 (atomic fingerprints):** a stored memory is all thirteen embeddings or nothing. There is no partial state. No fusion at storage time — all thirteen vectors are retained independently so that query-time weight profiles and fusion strategies can rearrange their contributions without losing information.
- **ARCH-02 (apples-to-apples comparison):** embeddings are only ever compared against other embeddings from the same embedder. Cross-space comparison happens only through score-normalised fusion strategies that preserve apples-to-apples similarity at the per-embedder level.
- **AP-007 (no fake data):** stub embedders are permitted only in test builds, gated behind the `test-utils` feature flag; production deployments fail hard rather than silently falling back.

## 4.2 The thirteen embedders

Each embedder is an independently-trained frozen model with its own dimension, distance metric, and index type. Together they span 11,008 dense dimensions (after counting all asymmetric dual vectors as separate), two 30,522-term sparse vocabularies, and a variable-length 128-per-token late-interaction space.

| ID | Name | Backbone | Dim | Type | Distance | Index | Captures |
|---|---|---|---|---|---|---|---|
| E1 | Semantic | e5-large-v2 | 1024 | Dense | Cosine | HNSW | General meaning |
| E2 | Temporal Recent | Sinusoidal PE of Unix timestamp | 512 | Dense | Cosine | HNSW | Recency |
| E3 | Temporal Periodic | Fourier basis (5 periods × 51 harmonics) | 512 | Dense | Cosine | HNSW | Cyclical patterns (hour/day/week/month/year) |
| E4 | Temporal Positional | Sinusoidal PE of session turn index | 512 | Dense | Cosine | HNSW | Sequence position within a session |
| E5 | Causal | nomic-embed-text-v1.5 fine-tuned | 768 × 2 | Dense asymmetric | AsymmetricCosine | HNSW ×2 | Cause/effect directionality |
| E6 | Sparse Lexical | BERT-vocab keyword frequency | 30,522 vocab, ≤1,500 active | Sparse | Jaccard + dot | Inverted | Exact keyword matching |
| E7 | Code | Qodo-Embed-1-1.5B (Qwen2 family) | 1536 | Dense | Cosine | HNSW | Code patterns, AST-aware |
| E8 | Graph | e5-large-v2 (structure-view) | 1024 × 2 | Dense asymmetric | Cosine | HNSW ×2 | Connectivity / source-target |
| E9 | HDC | 10K-bit hypervector → 1024D dense | 1024 | Dense | Cosine | HNSW | Typo-robust character-n-gram |
| E10 | Contextual | e5-base-v2 (paraphrase/context) | 768 × 2 | Dense asymmetric | Cosine | HNSW ×2 | Paraphrase equivalence |
| E11 | Entity (KEPLER) | KEPLER (RoBERTa + TransE on Wikidata5M) | 768 | Dense | Cosine | HNSW | Knowledge-graph entity embeddings (Wang et al., 2021) |
| E12 | Late Interaction | ColBERTv2 (Santhanam et al., 2022) | 128/token | Token-level | MaxSim | No HNSW | Token-level precision matching |
| E13 | SPLADE v3 | SPLADE (Formal et al., 2021) | 30,522 vocab, ≤1,500 active | Sparse | Jaccard + BM25 | Inverted | Learned keyword expansion |

Total effective storage per fingerprint is approximately 63 KB (~46 KB uncompressed dense data plus sparse indices and token-level vectors). The system enforces a minimum of 5 KB and a maximum of 150 KB at serialisation time — values outside this band panic, indicating missing embeddings or corrupt input.

**Asymmetric embedders.** E5, E8, and E10 store *dual* vectors. For E5 the two sides are "as cause" and "as effect"; a text embedded as a cause and then compared against another text embedded as an effect yields a directional similarity, with a 1.2× boost for cause→effect and a 0.8× dampening for effect→cause. For E8 the sides are source/target (structural connectivity); for E10 they are paraphrase/context. The asymmetric embedders let Context Graph distinguish "A causes B" from "B causes A" and "A imports B" from "B imports A" — distinctions that symmetric embeddings cannot make.

**Temporal embedders.** E2, E3, E4 are the cheapest and most unusual of the set. They carry no pretrained weights; each is a pure mathematical encoding (sinusoidal positional encoding of absolute Unix timestamp for E2; Fourier harmonics over five periods for E3; session-position sinusoidal PE for E4). This is intentional. Time-of-day, session sequence, and recency are three dimensions of meaning that no pretrained text embedder captures — they are present in metadata but invisible to content-based models. The temporal embedders contribute three independent information axes to the thirteen-embedder stack.

**E9 HDC (hyperdimensional computing).** E9 encodes text as a 10,000-bit binary hypervector built by XOR-binding character trigram hypervectors with relative position shifts and bundling them by majority vote, then projects the hypervector to 1024D dense. The encoding is deliberately position-invariant at the absolute level and order-preserving at the local level. A one-character typo changes the hypervector at the n-gram scale (roughly 5% bit difference for one-character insertion/deletion in a typical short text), producing high cosine similarity between a correctly-spelled and a typo'd query. This is the embedder that catches content E1 cannot catch because of spelling errors or morphological variation.

## 4.3 Storage: fifty-nine column families

Context Graph's storage layer (`context-graph-storage`, ~38K lines) persists everything derived from the embedding decomposition to a RocksDB instance with exactly fifty-nine column families, organised into five groups:

| Group | Count | Examples |
|---|---|---|
| Base | 11 | `nodes`, `edges`, `embeddings`, `metadata`, `temporal`, `tags`, `sources`, `system`, `embedder_edges`, `typed_edges`, `typed_edges_by_type` |
| Teleological | 28 | `fingerprints`, `topic_profiles`, `e13_splade_inverted`, `e6_sparse_inverted`, `e1_matryoshka_128`, `content`, `source_metadata`, `audit_log`, `merge_history`, `importance_history`, `embedding_registry`, `hnsw_graphs`, `training_records`, `constellations`, `constellation_by_selector`, `contrastive_pairs`, `contrastive_by_kind`, `contrastive_by_anchor`, `typed_edge_records`, `typed_edge_validations`, … |
| Quantised embedder | 13 | `emb_0` (E1 PQ-8) through `emb_12` (E13 SparseNative) |
| Code | 5 | `code_entities`, `code_e7_embeddings`, `code_file_index`, `code_name_index`, `code_signature_index` |
| Causal | 2 | `causal_relationships`, `causal_by_source` |

Keys are predominantly raw UUIDs (16 bytes) for primary-record CFs and composite keys (17, 24, 32, or 33 bytes) for secondary indexes. Values are bincode with a leading version byte for dense-vector CFs and JSON for provenance/audit CFs; the system enforces a serialisation invariant that `skip_serializing_if` may never be used with bincode (the positional format silently corrupts field-count changes) — optional fields must use JSON column families.

The storage layer provides atomic multi-CF writes via RocksDB `WriteBatch`, shared LRU block cache (256 MB), paranoid checksum verification, soft-delete with seven-day retention and tamper-resistant markers in a dedicated system column family, and per-CF write-buffer limits that cap total write-buffer memory at a manageable fraction of physical RAM across all fifty-nine CFs (without which the RocksDB default would consume approximately 7.6 GB).

## 4.4 Indexes

Fifteen in-memory HNSW indexes (Malkov & Yashunin, 2018) cover the thirteen embedders plus the two asymmetric dual variants (E5 cause/effect, E10 paraphrase/context): E1Semantic, E1Matryoshka128, E2, E3, E4, E5, E5CausalCause, E5CausalEffect, E7, E8, E9, E10, E10MultimodalParaphrase, E10MultimodalContext, E11. Matryoshka 128D is a truncation of the E1 1024D vector used as a fast second-stage filter in the pipeline retrieval strategy. Default HNSW parameters are M=16, ef_construction=200, ef_search=100 for most embedders; Matryoshka 128 uses M=32, ef_c=256, ef_s=128 to compensate for the tighter dimension.

Two inverted indexes (E6 sparse, E13 SPLADE) handle the sparse embedders. The late-interaction embedder E12 uses MaxSim token-level scoring — no HNSW applies because each memory has a variable number of 128-dimensional token vectors, not a single vector.

A separate `CausalE11Index` wraps a 768D HNSW specifically over LLM-generated causal-relationship descriptions (Section 4.8), with initial capacity 1,024 growing by doubling.

## 4.5 Search strategies and weighted RRF fusion

Context Graph supports four search strategies:

1. **E1-only.** Single-embedder HNSW search over E1 semantic. Fastest; backward-compatible baseline.
2. **MultiSpace.** Weighted Reciprocal Rank Fusion (RRF; Cormack et al., 2009) across the thirteen embedders, with a chosen weight profile driving each embedder's contribution. This is the default strategy.
3. **Pipeline.** A five-stage progressive filtering pipeline: E13 SPLADE + E6 sparse recall → E1Matryoshka128 fast ANN → multi-space RRF rerank → optional K-NN graph expansion → E12 MaxSim rerank. Target total latency <60 ms at 1M memories.
4. **Embedder-first.** Exposed via a separate MCP tool (`search_by_embedder`) that takes an explicit embedder name and returns results ranked by that embedder alone — the blind-spot discovery mode.

RRF aggregation combines per-embedder rankings without requiring score calibration across embedders:
$$
\mathrm{RRF}_{\mathrm{weighted}}(d) = \sum_{i=1}^{N} \frac{w_i}{K + \mathrm{rank}_i(d) + 1}
$$
with $K = 60$ (Constitution constant, consistent with the Cormack et al. default) and $w_i$ drawn from a weight profile. The system ships fourteen built-in profiles: `semantic_search`, `causal_reasoning`, `code_search`, `fact_checking`, `graph_reasoning`, `temporal_navigation`, `sequence_navigation`, `conversation_history`, `category_weighted`, `typo_tolerant`, `pipeline_stage1_recall`, `pipeline_stage2_scoring`, `pipeline_full`, and `balanced`. Each profile weights the thirteen embedders for a specific class of query; users can construct custom 13-element weight vectors (validated to sum to approximately 1.0) for session-scoped reuse.

Fusion explicitly supports a score-weighted variant (*score-weighted RRF*) for E5 Causal, whose absolute similarity magnitude carries semantically-meaningful information (a strong causal signal is more than just a high rank). Standard RRF degenerates rank-equivalent embedders; score-weighted RRF preserves E5 magnitude.

## 4.6 Causal gate and direction-aware scoring

E5's asymmetric similarity computation includes a *causal gate* applied only to causal-detected queries:

- CAUSAL_THRESHOLD = 0.04: above this, the gate boosts scores by 1.10×.
- NON_CAUSAL_THRESHOLD = 0.008: below this, the gate demotes scores by 0.85×.
- Between these thresholds the score passes through unchanged.

These thresholds were calibrated empirically against the post-training E5 distribution (causal-content mean similarity ≈ 0.138, non-causal mean ≈ 0.016). The gate is deliberately binary rather than continuous to avoid reward-hacking-like behaviour in the causal-inference direction, and it is applied only when the query is classified as causal (to avoid penalising non-causal queries on documents that happen to have low E5 scores for benign reasons).

E8 Graph and E10 Contextual also use asymmetric scoring (source→target 1.2×, target→source 0.8× for E8; paraphrase→context for E10), but without a gate — their direction modifier is the sole mechanism.

## 4.7 Cross-embedder anomaly detection and blind-spot discovery

One of the most empirically-valuable capabilities unlocked by DDA is cross-embedder anomaly detection. Context Graph exposes this through the `search_cross_embedder_anomalies` MCP tool: given a query and two embedders, return memories that score highly in one and poorly in the other. These anomaly pairs directly characterise the *disagreements* between embedders — a memory that is semantically similar (high E1) but causally unrelated (low E5) is a different kind of relationship than one that is both semantically and causally similar.

Three anomaly patterns have proven especially useful in practice:

- **Semantic-but-not-causal** (high E1, low E5): content about the same topic that lacks any cause-effect link — useful for filtering "similar topic, but doesn't answer why" during causal retrieval.
- **Keyword-but-not-paraphrase** (high E6 or E13, low E10): content that shares exact tokens but differs in intent — useful for distinguishing literal matches from semantic matches.
- **Code-shape-but-different-intent** (high E7, low E1): structurally similar code that does different things — a class of bug that pure-text retrieval cannot catch.

The `mine_contrastive_pairs` MCP tool persists discovered anomaly pairs into the `contrastive_pairs` column family with a seven-variant AnomalyKind classification (SemanticButNotCausal, KeywordButNotParaphrase, CodeShapeButDifferentIntent, EntitySharedButDifferentStructure, HdcRobustButSemanticDifferent, Other), producing a directly-usable contrastive-learning dataset from the embedder disagreements alone.

## 4.8 LLM-assisted causal and graph discovery

For causal and graph-relationship content that is not reliably detectable from embedder disagreements alone, Context Graph integrates a local LLM (Hermes-2-Pro-Mistral-7B Q5_K_M via llama-cpp-2 with GBNF grammar-constrained JSON output, using roughly 5 GB of model + 1 GB of KV cache, approximately 6 GB VRAM within a 9 GB budget). The LLM analyses memory pairs, produces JSON-valid causal-relationship descriptions with confidence and mechanism-type labels, and the E5EmbedderActivator generates asymmetric E5 embeddings (cause/effect dual vectors) for confirmed relationships.

The GBNF grammar constraint is load-bearing: it eliminates the ~60% invalid-JSON rate that afflicted earlier Candle/Qwen experiments, replacing unreliable parse-failure-handling with 100% valid JSON by construction. This is one of the more important engineering lessons from the system: LLM outputs destined for structured storage should be grammar-constrained at the generation level rather than validated at the parse level.

The graph-discovery agent reuses the same LLM (shared via `Arc<CausalDiscoveryLLM>`) and analyses memory pairs for 20 relationship types across four content domains (code, legal, academic, general). Both agents require the `llm` feature flag at build time.

## 4.9 Training-data export

The `export_training_corpus` MCP tool materialises every memory's full derived representation into a single bincode `TrainingRecord` persisted to the `training_records` column family. Each record carries:

- identity and provenance (UUID, content, importance, session, source, SHA-256 content hash);
- all thirteen embeddings (dense and sparse; token-level for E12);
- the stored 13D topic profile, 78 synergy-weighted cross-correlations, and 6D group alignments;
- outgoing and incoming typed edges and per-embedder K-NN neighbours;
- LLM-discovered causal labels;
- cluster memberships;
- Phase-5 temporal labels (hour-of-day, day-of-week, month, age, session position, periodic bucket, E2/E3/E4 norms);
- an optional Phase-4 Tucker-1 core decomposition of the 13×13×1024 interaction tensor at ranks (4, 4, 128), adding ~18 KB per record.

This is the concrete artifact of DDA on text: a bincode blob, typically several hundred kilobytes per memory, containing roughly 91 independent labeled signals per input. A corpus of 100,000 memories becomes a training-data asset of approximately 9.1 million labeled signals — a 91× multiplication of the effective corpus without generating any synthetic content.

A separate `export_typed_edges_corpus` tool materialises the graph-structural relationships between memories into `typed_edge_records`, producing a contrastive-style training set keyed by (source, target, edge_type) with per-embedder similarity profiles on each edge. The `derive_anomalies_from_edges` tool then uses the exported edges to produce anomaly pairs directly from typed-edge embedder scores without re-scanning the full corpus.

A companion CLI command (`context-graph-cli export training-corpus`) writes the same payloads to Parquet with Zstandard compression for offline ML workflows. The Parquet schema is two columns: `memory_id: Utf8` and `record_bytes: Binary` (the version-byte + bincode payload, byte-for-byte identical to the RocksDB value).

## 4.10 Scale, verification, and operational properties

The system builds on stable Rust (2021 edition), targets x86_64-unknown-linux-gnu with CUDA 13.x through HuggingFace Candle (v0.9.2-alpha), runs on NVIDIA RTX 5090 (Blackwell) for all GPU inference, and ships with approximately 5,184 `#[test]` / `#[tokio::test]` attributes across 422 files plus 192 dedicated integration-test files (verified April 2026). The test suite includes ~1,500 `[VERIFIED]`/`[PASS]`/`[FSV]` markers for forensic state-verification.

Operational limits that matter for the DDA argument:

- Maximum memories per HNSW index: 2,000,000.
- Maximum causal graph: 500,000 nodes, 2,000,000 edges.
- Maximum topics tracked: 10,000.
- Soft-delete GC retention: 7 days.
- Fingerprint size bounds: 5–150 KB (panics outside this band).

These are ordinary production limits, not theoretical ones. The system handles corpora at the "large organisational knowledge base" scale (millions of memories, tens of thousands of topics) on a single workstation with an RTX 5090 and 64+ GB RAM.

## 4.11 What Context Graph demonstrates for DDA

Context Graph is the most complete realisation of DDA applied to text that we are aware of. It establishes three concrete claims:

1. **Thirteen independent embedders of text are implementable on one workstation and produce non-redundant information.** The `search_cross_embedder_anomalies` tool finds memories on demand that demonstrate the embedders are not redundant — a redundancy-free 13-dimensional meaning space on text is a real object, not a theoretical construct.
2. **The derived data can be persisted as structured training artifacts.** `export_training_corpus` materialises the DDA output into a concrete file format suitable for downstream ML workflows, providing a reference spec for what "decomposed" data looks like on disk.
3. **The MCP tool surface makes the decomposition consumable by AI assistants.** With 67–75 tools registered over JSON-RPC 2.0, an AI assistant can exploit the thirteen-dimensional representation without being retrained to know about the system — MCP's machine-readable tool descriptions eliminate the training-data problem for the platform itself (a meta-instance of the DDA argument).

Section 5 shows the same principle realised on video through ClipCannon's 7-modality pipeline.
