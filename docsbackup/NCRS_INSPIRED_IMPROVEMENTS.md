# NCRS-Inspired Improvements for Context Graph

**Subtitle:** What a Neuromorphic Commonsense Reasoning System Teaches Us, and What We Should Actually Port

**Version:** 1.0 — April 14, 2026
**Status:** Internal design note (local only — `docs/` is gitignored)
**Companion docs:**
- [`MARKETING_AND_POSITIONING_2026.md`](./MARKETING_AND_POSITIONING_2026.md) — commercial positioning
- [`contextgraphsugestionimprove.pdf`](./contextgraphsugestionimprove.pdf) — 13-embedder architectural thesis
- [`TRAINING_DATA_EXPORT_PLAN.md`](./TRAINING_DATA_EXPORT_PLAN.md) — training-data export pipeline

---

## 0. TL;DR (The One-Paragraph Answer)

NCRS is a fascinating system, but the valuable ideas for Context Graph are **all algorithmic and architectural, not substrate-level**. Spiking neural networks, Brian2 simulation, k-WTA as fusion, Time-to-First-Spike encoding, and neuromorphic hardware give **zero wall-clock advantage** for our workload (1M-node K-NN graph traversal on RTX 5090-class hardware). GPU HNSW already dominates anything neuromorphic silicon can deliver on absolute latency, and k-WTA is strictly worse than Reciprocal Rank Fusion for heterogeneous score distributions. **But** NCRS contains at least **eight transferable insights** that are real wins: typed-edge-specific weight profiles, target-centroid-guided K-NN walks with plateau termination, CLS-style two-speed memory, multi-path K=3 parallel walks, ensemble-agreement early termination, regression-detected causal chains, formal episodic/semantic lifecycle states, and K-specialized-column routing over our 13 embedders. This document identifies those eight, specifies how to implement each, estimates effort, and provides a 30-day build plan for the top three.

---

## 1. NCRS Architecture, In Our Vocabulary

NCRS is a hybrid neuromorphic-LLM system for commonsense reasoning over ConceptNet. The team behind it has built something genuinely novel. Stripped of substrate:

| NCRS component | What it actually does |
|---|---|
| 6-step pipeline | Classify query → extract seeds → walk graph → select winners → detect regression → (deprecated: synthesize) |
| 9 cortical columns | Nine separately-indexed knowledge partitions, one per ConceptNet relation type (IsA, HasA, CausedBy, UsedFor, etc.) |
| Brian2 SNN | Spiking simulation that implements greedy best-first search on each column's graph |
| k-WTA (k=10) no decoder | At each "hop," stop when 10 neurons have fired; the first 10 firings are the top-10 neighbors, by construction |
| Goal distance | Cosine(current candidate embedding, goal centroid) — used as the heuristic in best-first search |
| Multi-path K=3 | Three independent walks from different seeds, in parallel processes (not beam search — no shared candidate set) |
| Regression detection | Terminate the walk when no neighbor reduces distance to goal (hill-climbing plateau) |
| TTFS (Time-to-First-Spike) | Encode cosine similarity as firing latency; sort by first-spike time |
| Outlines constrained gen | LLM outputs are grammar-constrained for >95% valid JSON |
| Qwen2.5-7B RPC | LLM does classification, seed extraction, synthesis; SNN does the fast walk |

**Rephrased in textbook algorithms:** NCRS is **per-relation-type greedy best-first graph search toward a goal centroid, with plateau-detection termination, run K=3 times in parallel on LLM-extracted seeds, and wrapped in an LLM-classified query router**. The spiking simulation is a biologically-inspired **scheduler** for the same thing GPU HNSW does in a straight `for` loop.

That reframe is the unlock. Once you see NCRS as *"best-first search + target centroid + plateau termination + multi-path + relation-typed specialization,"* every idea in it maps onto a concrete modification to `traverse_graph`, `get_causal_chain`, `get_unified_neighbors`, or the weight-profile system.

---

## 2. The Mapping Table (NCRS ↔ Context Graph)

| NCRS concept | Context Graph analog | Gap today | Improvement lever? |
|---|---|---|---|
| 9 cortical columns by relation | 8 typed edge types (`SemanticSimilar`, `CodeRelated`, `EntityShared`, `CausalChain`, `GraphConnected`, `ParaphraseAligned`, `KeywordOverlap`, `MultiAgreement`) | No per-edge-type weight profile; RRF fuses uniformly | **HIGH** — "MoE-by-relation" weight profiles |
| k-WTA winner selection | Top-K from each embedder, then RRF | We use weighted RRF; k-WTA gives less calibrated fusion | **LOW** — do not port; RRF dominates |
| Goal-distance pathfinding | K-NN graph walk (`traverse_graph`) with no target direction | Walk is undirected; no goal-vector conditioning | **HIGH** — target-centroid-guided walks |
| Regression detection | None explicit | Walks continue until `max_hops` or `min_weight` budget exhausted | **HIGH** — plateau termination |
| Multi-path K=3 parallel | Single walk per query | Solo search misses embedder-disagreement cases | **MEDIUM** — K parallel walks with different profiles |
| Episodic column | Session memory (`SessionCache`, `E4_Temporal_Positional`) but not typed as episodic | No first-class fast/slow distinction; E4 participates in all RRF uniformly | **MEDIUM** — formal CLS-style lifecycle |
| Seed extraction (LLM → entities → fuzzy search) | `extract_entities` (E11 KEPLER) + `search_by_entities` | Already well-matched | **NONE** — we have this |
| Qwen constrained generation (Outlines) | Hermes-2-Pro GBNF grammar for causal discovery | Already well-matched | **NONE** — we have this |
| Brian2 spiking substrate | HNSW + RocksDB | Different substrate; SNN gives no speedup for our scale | **NONE** — do not port |
| TTFS encoding | Sorted top-K by cosine | Mathematically equivalent on von Neumann HW | **NONE** — do not port |
| Cortical column *type* routing | Query-type → weight profile mapping | We have 14 built-in profiles but no classifier-driven routing | **MEDIUM** — LLM query classifier like NCRS Step 1 |
| Synthesis (deprecated) | LLM answer generation not in scope for retrieval | N/A | N/A |

Summary: **five HIGH-or-MEDIUM wins**, zero substrate ports, two things we already match.

---

## 3. The Eight Transferable Improvements (Ranked by Expected Value)

Each improvement below includes: *why* it matters, *what to build*, *estimated effort*, *measurable success criterion*, and the existing Context Graph components it touches.

### 3.1 Improvement #1 — Target-Centroid-Guided K-NN Walks with Plateau Termination ★★★★★

**Why it matters:**
Today, `traverse_graph` and `get_causal_chain` are **undirected** — they follow typed edges up to `max_hops`, filtered by `min_weight`. There is no target state. NCRS's key insight is that graph walks are ten times more useful when you give them a **goal embedding** and let them hill-climb toward it, terminating when no neighbor closes the distance. This is what HNSW's greedy routing does internally at index time; we just don't expose it at query time for multi-hop walks.

Combining this with our **constellation compiler** (Phase 2) closes a loop nobody else has closed: constellations already produce per-embedder goal centroids for a compiled memory group, and we already have `score_against_constellation`. A target-guided walk uses those centroids *as the walk heuristic*, not just as a post-hoc scorer.

**What to build:**

1. New walker mode on `traverse_graph`: parameter `target_memory_id` (start from any node and walk toward this one) or `target_constellation_id` (walk toward the E1 centroid of a compiled constellation). If neither is given, current undirected behavior persists.

2. Walk heuristic: `score(candidate) = w_semantic · cos(candidate.e1, target.e1) + w_causal · asymmetric_cos(candidate.e5_cause, target.e5_effect) + ...`. Re-use our existing `compute_similarity_for_space` machinery — this is the same code path `search_graph` uses.

3. Termination conditions, in priority order:
   - `score(best_neighbor) ≤ score(current) + plateau_epsilon` → plateau termination (regression detected)
   - `hop_count ≥ max_hops`
   - `visited_set` contains all candidates within `k` of current node
   - A fixed budget of `max_walks_per_second` is exceeded (mirror NCRS's safety net)

4. Output: an ordered path `[start, hop1, hop2, …, terminated_at]` plus a per-step score trace so downstream tools can visualize convergence. Emit a `termination_reason` enum so we distinguish `PlateauReached` from `MaxHopsExceeded` from `NoNeighborsFound`.

**Existing code to extend:**
- `crates/context-graph-mcp/src/handlers/tools/graph_link_tools.rs` — `traverse_graph` handler
- `crates/context-graph-core/src/retrieval/distance.rs` — `compute_similarity_for_space_with_direction` already does per-space scoring
- `crates/context-graph-core/src/causal/chain.rs` — `score_causal_chain`, `score_causal_chain_attenuated` need an optional `target_embedding` parameter

**Effort:** 3–5 days of work. No new crate dependencies. No new ML model. New MCP tool signature fields only.

**Success criterion:**
- On the Shakespeare FSV corpus, walks toward a target constellation (e.g., "monologue-style" constellation from a curated set) produce paths whose final memory's score against the target is ≥ 0.70 in ≥ 80% of trials; undirected walks from the same start points achieve < 0.50 average.
- Plateau termination fires before `max_hops=5` in ≥ 60% of trials (evidence that the path is finding local optima, not burning budget).

**Why this beats NCRS's version:**
NCRS uses cosine on a single embedder as the heuristic. We can use a **weighted 13-embedder blend** as the heuristic — richer goal signal, so the walk converges faster and to a more meaningful local optimum.

---

### 3.2 Improvement #2 — Typed-Edge-Specific Weight Profiles ("MoE-by-Relation") ★★★★★

**Why it matters:**
NCRS's 9 cortical columns are not really "9 SNNs" — they are **9 separately-indexed graphs, one per relation type, searched with relation-appropriate heuristics**. That insight translates directly onto our 8 typed edge types. Today, when a traversal hits a `CausalChain` edge, we use the same 13-embedder weight profile as when it hits a `CodeRelated` edge. That's obviously suboptimal: you want E5-heavy weighting on causal walks, E7-heavy on code walks, E11-heavy on entity walks.

**What to build:**

1. Add a `relation_weight_profiles: HashMap<GraphLinkEdgeType, [f32; 13]>` struct to the weights module. Default values:

   | Edge type | Dominant embedder weights |
   |---|---|
   | `SemanticSimilar` | E1=0.45, E10=0.20, E9=0.10, E6=0.10, E12=0.05, E13=0.05, rest 0.01 |
   | `CodeRelated` | E7=0.50, E1=0.25, E9=0.05, E6=0.10, E13=0.05, rest 0.01 |
   | `EntityShared` | E11=0.45, E1=0.25, E8=0.10, E10=0.10, rest 0.01 |
   | `CausalChain` | E5=0.40 (cause side), E1=0.30, E11=0.10, E8=0.05, rest 0.02 |
   | `GraphConnected` | E8=0.45 (source→target), E1=0.20, E11=0.10, E5=0.10, rest 0.02 |
   | `ParaphraseAligned` | E10=0.40 (paraphrase side), E1=0.30, E12=0.10, E6=0.05, rest 0.02 |
   | `KeywordOverlap` | E6=0.30, E13=0.30, E1=0.20, E12=0.10, rest 0.02 |
   | `MultiAgreement` | All active embedders at 1/n (existing behavior) |

2. At each hop in `traverse_graph` / `get_causal_chain`, look up the edge type and apply the corresponding profile for computing next-hop scores. Target-guided walks (Improvement #1) layer on top: the heuristic is computed with the edge-specific profile.

3. Expose via a new MCP parameter: `relation_profile_overrides: Option<HashMap<String, [f32; 13]>>` so callers can tune on a per-query basis. Default is the built-in table above.

**Existing code to extend:**
- `crates/context-graph-core/src/weights/mod.rs` — add `RELATION_WEIGHT_PROFILES: Lazy<HashMap<GraphLinkEdgeType, [f32; 13]>>`
- `crates/context-graph-core/src/graph_linking/edge_type.rs` — `GraphLinkEdgeType` already exists; add `default_weight_profile()` method
- `crates/context-graph-mcp/src/handlers/tools/graph_link_tools.rs` — thread the profile lookup through traversal

**Effort:** 2 days.

**Success criterion:**
- On a mixed-relation corpus (Rust code + causal claims + entity references), per-relation profiles produce paths whose average target-score at hop 3 is ≥ 15% higher than uniform weighting.
- No regression on single-relation synthetic FSV corpora (profiles collapse to current behavior when one relation dominates).

**Why this is necessary:**
This is the single most-obvious missing piece in Context Graph. We built typed edges in the graph-linking subsystem but we don't specialize search behavior by edge type. MoE-by-relation is what every KG-embedding paper (RotatE, QuatE, TransE) has been doing since 2019; porting the idea to *retrieval* rather than *embedding* is the next step.

---

### 3.3 Improvement #3 — Multi-Path K=3 Parallel Walks ★★★★

**Why it matters:**
NCRS runs three independent walks from three different seeds in parallel. This is **explicitly not beam search** — the walks do not share a candidate set. The purpose is diversity: one walk might find the "semantically closest" path, another the "causally most relevant" path, a third the "entity-matched" path. Beam search collapses these into one frontier and loses the diversity. NCRS keeps them separate and returns all three.

This maps directly onto our existing "weight profiles" system, and it is the **killer feature for cross-embedder anomaly discovery at traversal time**. Right now, cross-embedder anomalies are mined offline via `mine_contrastive_pairs`. Online, during a graph walk, we never detect when the E1-weighted walk and the E5-weighted walk disagree. Multi-path K=3 would surface that disagreement as a first-class output.

**What to build:**

1. New MCP tool: `multi_path_traverse` with parameters `start_memory_id`, `target_memory_id` (optional), `profiles: Vec<String>` (length 1–5, default `["semantic_search", "causal_reasoning", "graph_reasoning"]`), `max_hops_per_path`.

2. Spawn `profiles.len()` parallel walks via `tokio::join!`. Each walk uses Improvement #1 (target-guided + plateau termination) and Improvement #2 (typed-edge profiles).

3. Output:
   - Per-path: ordered memory IDs, per-step scores, termination reason
   - Aggregate: intersection set (memories visited by all paths) — these are "consensus" nodes
   - Aggregate: symmetric difference per pair — these are "disagreement" nodes, valuable for hard-negative mining
   - Aggregate: RRF fusion across all paths' terminal scores for a unified ranking

4. Store the disagreement set as a side effect into `CF_CONTRASTIVE_PAIRS` with a new `AnomalyKind::MultiPathDisagreement`. This turns every user traversal into training data.

**Existing code to extend:**
- `crates/context-graph-mcp/src/handlers/tools/graph_link_tools.rs` — add a new handler
- `crates/context-graph-core/src/contrastive/types.rs` — extend `AnomalyKind` enum with a new variant
- `crates/context-graph-core/src/fusion/mod.rs` — RRF is already there; reuse for cross-path fusion

**Effort:** 3 days.

**Success criterion:**
- On FSV, multi-path traversal surfaces at least 1 novel disagreement pair per 10 queries that was not found by the offline miner.
- Per-path latency (Tokio-parallelized) is within 1.5× of single-path latency (not 3×).

**Why NCRS's framing is right:**
Beam search is dominated by whichever embedder's scores are on top of the beam early. Multi-path preserves all embedders' independent hypotheses, and **disagreement becomes signal**. This is the exact cross-embedder anomaly logic we already believe in — NCRS shows how to run it online as part of every walk.

---

### 3.4 Improvement #4 — LLM Query Classifier → Dynamic Weight Profile ★★★

**Why it matters:**
NCRS Step 1 classifies query intent (~50ms) and generates a dynamic weight profile. We have 14 built-in profiles but no classifier — callers either pass a profile name manually or we default to `semantic_search`. NCRS's insight is that a small LLM call up front saves a big LLM call on the back end, because the wrong profile throws away relevant results and forces the caller to re-query.

We already have the LLM infrastructure: Hermes-2-Pro-Mistral-7B with GBNF grammar via llama-cpp-2, used by the causal discovery agent. We can add a grammar for query classification with near-zero incremental cost.

**What to build:**

1. New service: `QueryClassifier` in `crates/context-graph-mcp/src/adapters/` (or a new `classifier/` subdir). GBNF grammar constrains output to a JSON with fields: `query_type` (enum over `semantic | causal | code | graph | entity | temporal | paraphrase | keyword | mixed`), `suggested_profile` (string name), `custom_weights` (optional [f32; 13]), `rationale` (1-2 sentences).

2. Prompt template: "Given this user query, classify the intent and suggest the most appropriate retrieval weight profile from: {list of 14 profiles}. Return JSON matching the grammar."

3. Integrate into `search_graph`: if neither `weightProfile` nor `customWeights` is provided, call the classifier and use its suggestion. Cost: ~100-200ms added latency on first-call; cache by query hash so repeat queries are free.

4. Emit the classifier's decision into `CF_AUDIT_LOG` as an operator_id="query_classifier" record, so downstream fine-tuning can measure how often the classifier's choice beat the default `semantic_search`.

**Existing code to extend:**
- `crates/context-graph-causal-agent/src/llm/mod.rs` — LLM client already there, reuse
- `crates/context-graph-mcp/src/handlers/tools/memory_tools.rs` — `search_graph` entry point
- `crates/context-graph-core/src/weights/mod.rs` — accept runtime custom weights (already supported)

**Effort:** 3–4 days (most of it grammar tuning and eval harness).

**Success criterion:**
- On a held-out query set labeled with ground-truth profiles, the classifier agrees with the label in ≥ 85% of cases.
- When the classifier disagrees with the user's manual choice, its choice wins on P@10 in ≥ 40% of cases (a healthy "disagreement rate" without being adversarial).

---

### 3.5 Improvement #5 — CLS-Style Two-Speed Memory Lifecycle ★★★

**Why it matters:**
NCRS's "episodic column" is the fast-updated, query-specific, short-horizon memory. The other eight columns are the slow-updated, general, long-horizon memory. This is **Complementary Learning Systems** theory (Kumaran, Hassabis, McClelland, *Trends in Cognitive Science* 2016), which formalizes two-speed memory as: hippocampal fast-write for recent episodes, neocortical slow-consolidate for lifetime patterns.

Context Graph has the substrate — session cache, `E4_Temporal_Positional`, merge/consolidate pipeline — but we have **no first-class lifecycle states** for "this is an episodic memory, don't consolidate yet" vs. "this is consolidated into a topic, stable." Every memory is equal-footing across all 13 embedders. That's wrong: episodic memories should be retrievable by session/sequence but not weighted into topic detection until they've stabilized.

**What to build:**

1. Add a `memory_lifecycle_state` field to `SemanticFingerprint`, enum over `{Episodic, Consolidating, Consolidated, Archived}`.

2. New state transitions, managed by a background task:
   - `Episodic`: just stored, < 1 hour old OR fewer than 3 references OR in active session
   - `Consolidating`: 1h–24h old, has been referenced or surfaced in ≥ 2 searches, OR currently being merged
   - `Consolidated`: > 24h old, part of a stable topic (`TopicStability::Stable`), included in weighted agreement
   - `Archived`: > 7 days, no new references, deprioritized in search

3. Weight-profile modifier: `{profile}_episodic` and `{profile}_consolidated` automatic variants. Episodic variants up-weight E4 (positional) and E2 (recency); consolidated variants up-weight E1 (semantic) and E8 (graph).

4. Topic detection runs ONLY on `Consolidated` and `Archived` memories. Episodic memories are visible to direct queries but not to divergence detection or stability metrics — mirroring hippocampal/cortical separation.

**Existing code to extend:**
- `crates/context-graph-core/src/types/fingerprint/teleological/types.rs` — add `lifecycle_state` field (bincode-safe append at end)
- `crates/context-graph-core/src/clustering/manager.rs` — filter by lifecycle state before clustering
- `crates/context-graph-storage/src/teleological/rocksdb_store/` — background task for state transitions

**Effort:** 5–7 days. Touches the core fingerprint struct, so extra care with serialization compatibility.

**Success criterion:**
- Topic stability metrics stop fluctuating on high-ingest workloads (episodic memories no longer destabilize topics).
- Session-scoped queries (sequence-navigation profile) return > 90% episodic memories when `sessionScope="current"`; cross-session queries return > 80% consolidated memories.

**The connection to ClipCannon:**
ClipCannon's MouthMemory atlas (Phase 4, `mouth_frames` table) is already a two-speed system — it stores per-frame face crops with landmarks, viseme labels, quality scores, and retrieves from them at generation time. The episodic/consolidated split for text mirrors this: fast-write fingerprints during a session, slow-consolidate into constellations after.

---

### 3.6 Improvement #6 — Regression-Detected Causal Chains ★★★

**Why it matters:**
`get_causal_chain` today walks `MAX_CHAIN_LENGTH=10` hops with hop attenuation 0.9^hop, stopping only when the minimum-similarity threshold is missed. NCRS's regression detection is simpler and more principled: stop when no unvisited neighbor reduces distance to the goal. This gives **shorter, more meaningful chains** because we stop at a natural plateau rather than burning hop budget.

**What to build:**

1. Reuse Improvement #1's plateau-termination machinery in `build_causal_chain()` (file: `core/causal/chain.rs`).

2. Goal: the target cause (for `search_effects`) or target effect (for `search_causes`). We already have these — they're the input to abductive/predictive reasoning.

3. New termination: `if score(best_next_hop) ≤ score(current) + plateau_epsilon * (0.9^hop) AND hop > 2: terminate`. The `0.9^hop` scaling respects the existing attenuation.

4. Return the chain plus a `terminated_at_plateau: bool` signal so downstream can tell apart "found a local optimum" (high-quality chain) from "ran out of hops" (may want to retry with different seed).

**Existing code to extend:**
- `crates/context-graph-core/src/causal/chain.rs` — `score_causal_chain_attenuated`, `build_causal_chain`
- MCP tool `get_causal_chain` — expose `terminated_at_plateau` in response

**Effort:** 1–2 days.

**Success criterion:**
- Average chain length drops from current ~5 hops to ~3 hops, but average per-hop score increases (proof we're stopping at better places).
- No regression on ground-truth causal chains from FSV (chains that ARE 10 hops long still terminate at the correct tail).

---

### 3.7 Improvement #7 — Query Classifier → Routing Ensemble (the "9-Column" Analog) ★★★

**Why it matters:**
This is the meta-insight from NCRS's columnar architecture. Rather than running all 13 embedders on every query (wasteful for "what color is the sky?"), route the query to a **subset of columns** based on detected query type. On a simple factoid query, run E1 + E11 + E6 only; on a code question, run E7 + E1 + E6; on a causal question, run E1 + E5 + E7. This can drop per-query embedder compute by 50–70% without meaningful recall loss.

**What to build (extends Improvement #4):**

1. Query classifier (from #4) outputs not just a weight profile but an `active_embedder_mask` — a `u16` bitmask selecting which of 13 embedders to actually evaluate.

2. `search_graph` and friends honor the mask: skip embedder indices where the mask is 0, zero their RRF contribution.

3. Safety net: if fewer than 3 embedders are active (classifier too aggressive), fall back to full evaluation. Production knob `min_active_embedders` defaults to 5.

4. Instrumentation: log effective embedder cost per query to `CF_AUDIT_LOG`; periodic job computes "embedder pruning savings" as a scalar.

**Existing code to extend:**
- `crates/context-graph-core/src/retrieval/query.rs` — `EmbeddingSpaceMask` already exists, plumb it through from classifier
- `crates/context-graph-mcp/src/handlers/tools/memory_tools.rs` — `search_graph` dispatch

**Effort:** 2 days on top of Improvement #4.

**Success criterion:**
- Average per-query embedder evaluations drop from 10 (current MultiSpace default) to 5–7.
- NDCG@10 on a gold-labeled test set does not drop by more than 1 point vs. full-13 evaluation.

---

### 3.8 Improvement #8 — Ensemble-Agreement Early Termination for Search ★★

**Why it matters:**
NCRS's regression detection says "stop when no neighbor improves goal distance." A related idea: for multi-embedder search, **stop the retrieval stages early when the top-K candidates stabilize across embedders** — i.e., if the top 10 from E1 and the top 10 from E5 are the same 10 UUIDs in the same order, there's nothing more to compute. This is "ensemble agreement" as a termination condition for the Stage 3 multi-space fusion.

**What to build:**

1. In the pipeline Stage 3 (multi-space RRF rerank), after each new embedder's top-K is incorporated, compute Kendall tau between the running top-K and the previous running top-K.

2. If tau > 0.95 for two consecutive embedders, terminate — the ranking has stabilized.

3. This is purely an optimization; it changes nothing about correctness, just saves compute when the query is easy.

**Existing code to extend:**
- `crates/context-graph-storage/src/teleological/search/pipeline/stages.rs` — Stage 3 loop
- No new MCP fields; expose via observability / logging

**Effort:** 1–2 days.

**Success criterion:**
- On "easy" queries (high E1-cosine) pipeline terminates at 3–4 embedders instead of 10–11; latency drops by 30–50%.
- On "hard" queries, all embedders run and no regression.

---

## 4. What Explicitly NOT to Port

### 4.1 The SNN Substrate (Brian2, SpikingJelly, Norse, Loihi, Akida)

**Evidence:**
- Brian2 is CPU-only at production scale. Brian2CUDA exists but is in maintenance; SpikingJelly is better-maintained but optimized for dense vision SNNs, not sparse graph BFS.
- Loihi 2 is research-only (Hala Point, Kapoho Point are for INRC members). Mike Davies has publicly said no near-term commercial product.
- SpiNNcloud's SpiNNaker 2 is the one commercially-available option, but pricing is enterprise-research-scale and latency on 1M-node graphs is not competitively published against cuGraph BFS.
- BrainChip Akida, Innatera, SynSense Speck are edge/sensor parts (mW class) — not server components.

**The key number:** cuGraph BFS on a 1M-node graph is **single-digit milliseconds on a modern GPU**. No published neuromorphic benchmark beats this in absolute wall-clock terms on published hardware. Neuromorphic wins are in **energy per inference** (often 10×–1000×), not **latency per query**. We are not an edge-inference company.

**Decision:** Do not add a Brian2 dependency. Do not target Loihi/SpiNNaker. If in 2027+ SpiNNcloud publishes a head-to-head BFS benchmark that beats cuGraph on absolute ms at 1M nodes, revisit.

### 4.2 Time-to-First-Spike (TTFS) Encoding

**Evidence:** TTFS encodes cosine similarity as firing latency and returns the first k spikes. On von Neumann hardware (CPU/GPU), this is arithmetically equivalent to sorting by negated cosine and taking top-K. The sort is faster than the event simulation.

**Decision:** Do not port. The conceptual elegance of "the strongest match fires first" is just sorting with extra steps on silicon that isn't neuromorphic.

### 4.3 k-WTA as Fusion Primitive

**Evidence:**
- Cormack, Clarke, Büttcher RRF paper (SIGIR 2009) and every fusion-comparison paper since (Bruch et al. SIGIR 2023, *"An Analysis of Fusion Functions for Hybrid Retrieval"*) confirm RRF dominates convex-combination and score-normalized fusion when component scores have heterogeneous distributions — exactly our situation with E1 cosines in [-1,1], E6 Jaccard in [0,1], E12 MaxSim in [0,1] but with different scaling.
- k-WTA is a **selection** operator (pick top-k winners), not a **fusion** operator (combine N source rankings). It doesn't answer the same question RRF answers.

**Decision:** Do not replace our weighted RRF with k-WTA. Keep RRF. k-WTA-style sparse selection inside individual embedder forward passes (Ahmad & Scheinkman 2019, *"How Can We Be So Dense?"*) is a different technique and not relevant at the retrieval ensemble layer.

### 4.4 Spiking Transformers as Drop-in Embedders

**Evidence:**
- SpikeGPT (260M, 2023), SpikingBERT (BERT-base, AAAI 2024), SpikeLLM (7B, ICLR 2025) are all research prototypes.
- None trained at modern scale (trillions of tokens, curated data).
- None competitive on MTEB / BEIR against e5-large-v2, BGE-M3, nomic-embed-text-v1.5, Voyage-3.
- No drop-in replacement for any of our 13 embedders exists.

**Decision:** Do not replace E1/E5/E7/E8/E10/E11 with spiking variants. Watch SpikeLLM scale-ups in 2026–2027; reconsider if parity is reached.

### 4.5 "Neuromorphic" as a Marketing Frame

**Evidence:** The positioning document (`MARKETING_AND_POSITIONING_2026.md`) sells Context Graph as *"the measurement instrument for meaning."* Adding "neuromorphic-inspired" marketing to that is confusion — buyers who care about neuromorphic want energy-per-inference wins; buyers who care about measurement want label density and provenance. Different markets.

**Decision:** Do not rename, re-brand, or add "neuromorphic" to public marketing. Internally, cite the CLS theory and MoE-by-relation literature; externally, keep the Illumina-for-text framing.

---

## 5. The Thirty-Day Build Plan

Based on expected value × implementation cost:

### Week 1 — Target-Guided Walks + Regression Termination (Improvement #1 + #6)
- Day 1: Extend `traverse_graph` signature with `target_memory_id` and `target_constellation_id`.
- Day 2: Implement plateau detection in the walker; add `termination_reason` enum.
- Day 3: Update `get_causal_chain` with the same plateau logic (Improvement #6 is mostly free given #1).
- Day 4: Test on Shakespeare FSV corpus; add integration tests.
- Day 5: Document new MCP parameters; update `08_mcp_tools_reference.md`.

**Deliverable:** `target_memory_id` and `target_constellation_id` parameters on `traverse_graph` and `get_causal_chain`; termination-reason telemetry in responses; integration tests passing.

### Week 2 — Typed-Edge Weight Profiles (Improvement #2)
- Day 1: Add `RELATION_WEIGHT_PROFILES` static map in `weights/mod.rs`.
- Day 2: Add `GraphLinkEdgeType::default_weight_profile()` method.
- Day 3: Thread the profile lookup through `get_typed_edges`, `traverse_graph`, `get_causal_chain`.
- Day 4: Calibrate default profiles on FSV corpus; add per-edge-type FSV tests.
- Day 5: Document the 8 default profiles; add `relation_profile_overrides` MCP param.

**Deliverable:** Every typed-edge traversal uses an edge-type-appropriate weight profile by default; override path for power users.

### Week 3 — Multi-Path K=3 (Improvement #3)
- Day 1: New MCP tool `multi_path_traverse` skeleton with `tokio::join!` execution.
- Day 2: Integrate Improvements #1 and #2 into each parallel path.
- Day 3: Implement intersection/symmetric-difference aggregation; RRF across paths.
- Day 4: Persist disagreement pairs to `CF_CONTRASTIVE_PAIRS` with new `AnomalyKind::MultiPathDisagreement`.
- Day 5: Integration tests; measure disagreement-mining yield vs. offline `mine_contrastive_pairs`.

**Deliverable:** New MCP tool returning K paths + consensus/disagreement analysis; online contrastive pair ingestion.

### Week 4 — Query Classifier + Active-Embedder Mask (Improvements #4 + #7)
- Day 1: Build GBNF grammar for query-classifier output.
- Day 2: Craft prompt template, hook into causal-agent LLM infrastructure.
- Day 3: Integrate into `search_graph`'s fallback path (no explicit profile → classifier).
- Day 4: Add `active_embedder_mask` output; honor mask in retrieval pipeline.
- Day 5: Eval harness on held-out query set; publish P@10 / NDCG comparison vs. default.

**Deliverable:** Automatic query classification with active-embedder pruning; audit-log instrumentation for A/B evaluation.

**Month-5 stretch (not in 30-day plan):**
- Improvement #5 (CLS-style lifecycle) — touches fingerprint serialization, needs careful migration plan
- Improvement #8 (ensemble-agreement early termination) — optimization, not blocker

---

## 6. Open Research Questions (Things I Could Not Resolve Without More Data)

### 6.1 Is there a point at which SNN substrate wins?

If Context Graph scales to **10B+ memories** and we need to traverse graphs that no longer fit in GPU VRAM, the neuromorphic-energy-advantage could matter. Specifically: a Hala-Point-class system (1.15B neurons, ~100W) could host a 10M-node graph at ~1000× lower energy than an RTX 5090 cluster doing the same workload. The question is whether any customer cares about energy at that scale before they care about latency.

**Status:** Monitor SpiNNcloud's SpiNNaker 2 commercial trajectory and Intel's Loihi roadmap. Revisit if a customer emerges with an "energy per query" requirement.

### 6.2 Could SpikeLLM become the 14th embedder?

SpikeLLM (Xing et al., ICLR 2025) scaled a spiking LLM to 7B params with ternary-spike encoding. It's not competitive on MTEB today. If the 2026–2027 follow-ups hit parity with e5-mistral on retrieval, and if the efficiency advantage translates to lower per-document embedding cost at scale, it could slot into our embedder lineup as an energy-efficient alternative to E1. That's a 2027+ question.

### 6.3 Does the Numenta Thousand Brains Project (Monty) have anything transferable?

Monty (Numenta, late 2024 open-source) is Hawkins' attempt to productize cortical-column reasoning. Early stage. The core idea — **reference frames per object**, with each column modeling the same object via a different sensorimotor trajectory — is interesting but distant from our text-retrieval use case. Worth reading when their first-pass benchmarks land.

### 6.4 Is there value in a spiking encoding of *causal asymmetry* specifically?

E5 (causal, asymmetric cause/effect) is our most structurally unusual embedder. A spiking encoding of cause→effect as **directed temporal firing** (cause fires first, effect fires after) is biologically natural and might yield an implicit temporal-ordering signal we don't currently capture. Worth prototyping as a one-weekend experiment once the 30-day plan is done.

---

## 7. The Synthesis — What NCRS Teaches Us About Our Own Architecture

Re-reading NCRS with fresh eyes, the insight is **not** that spiking neural networks are the future of retrieval. It's that **NCRS treated the graph walk as a first-class algorithmic primitive** — goal-directed, plateau-terminated, multi-path, relation-typed — while we have treated it as an afterthought bolted onto HNSW search.

Our 13-embedder decomposition is a **measurement system**. NCRS's nine columns are a **reasoning system**. The missing third leg is **traversal as reasoning**: walks on our K-NN graph that are goal-directed, self-terminating, and specialized by edge type. The eight improvements above are that missing leg.

The best compliment we can pay NCRS is to port its algorithmic ideas while discarding its substrate ideas — because the algorithms transfer, and the substrate doesn't. Brian2 SNNs are how NCRS implemented those ideas on a shoestring, and they made it work, but we have better primitives (RocksDB + HNSW + typed edges + constellation compiler) for the same algorithms.

**The 30-day plan above is the concrete output.** After it lands, `traverse_graph` is a **target-guided, relation-specialized, plateau-terminated, multi-path graph reasoner** that produces contrastive pairs as a side effect. That is the real value extract from the NCRS reading.

---

## 8. Appendix A — NCRS References Worth Reading

Based on the system description provided, plus the research agent's assessment of state-of-the-art neighbors:

### Neuromorphic background (for context, not for porting)
- Eshraghian, J. K., et al. *"Training Spiking Neural Networks Using Lessons From Deep Learning."* Proc. IEEE, 2023. — Canonical snntorch paper.
- Aimone, J. B., Parekh, O. D., et al. *"Provable advantages for graph algorithms in spiking neural networks."* SPAA 2021. — Sandia's shortest-path-on-Loihi line; where the "graphs on neuromorphic" claim comes from.
- Modha, D. S., et al. *"IBM NorthPole."* Science 2023. — The best-in-class neuromorphic prototype; research only.
- Davies, M. "Loihi 2: A New Generation of Neuromorphic Computing." Intel Labs, 2021.

### Algorithmic ideas we are porting
- Cormack, G. V., Clarke, C. L. A., Büttcher, S. *"Reciprocal rank fusion outperforms Condorcet and individual rank learning methods."* SIGIR 2009. — Still the canonical RRF reference.
- Bruch, S., Gai, S., Ingber, A. *"An Analysis of Fusion Functions for Hybrid Retrieval."* SIGIR 2023. — Confirms RRF dominates for heterogeneous score distributions.
- Kumaran, D., Hassabis, D., McClelland, J. L. *"What Learning Systems Do Intelligent Agents Need? Complementary Learning Systems Theory Updated."* Trends in Cognitive Sciences, 2016. — CLS theory, the basis for Improvement #5.
- Bordes, A., et al. *"Translating Embeddings for Modeling Multi-relational Data"* (TransE). NeurIPS 2013. — Our E11 KEPLER uses TransE; relation-specific scoring is the ancestor of Improvement #2.
- Whittington, J. C. R., et al. *"The Tolman-Eichenbaum Machine."* Cell 2020. — Elegant hippocampal-entorhinal model; conceptual cousin to CLS lifecycle states.

### To monitor (do not port yet)
- SpikeLLM (Xing et al., ICLR 2025) — if it scales beyond 7B.
- Numenta Thousand Brains Project / Monty (late 2024 OSS) — if benchmarks land.
- SpiNNcloud SpiNNaker 2 commercial deployments (2025+).
- Ahmad, S., Scheinkman, L. *"How Can We Be So Dense? The Benefits of Using Highly Sparse Representations."* 2019. — Sparse representations inside networks (not for fusion).

---

## 9. Appendix B — Mapping to Existing Context Graph Files

Every improvement above has a specific set of files to modify. This appendix spells them out for quick reference during implementation.

### Improvement #1 — Target-Guided Walks + Plateau Termination
- `crates/context-graph-mcp/src/handlers/tools/graph_link_tools.rs` — `traverse_graph` handler: add `target_memory_id`, `target_constellation_id` params
- `crates/context-graph-mcp/src/tools/definitions/graph_link.rs` — MCP schema update
- `crates/context-graph-core/src/causal/chain.rs` — `build_causal_chain`, `score_causal_chain_attenuated` extensions
- `crates/context-graph-core/src/retrieval/distance.rs` — reuse `compute_similarity_for_space_with_direction`
- New type: `crates/context-graph-core/src/graph_linking/walk_result.rs` — `WalkTerminationReason` enum

### Improvement #2 — Typed-Edge Weight Profiles
- `crates/context-graph-core/src/weights/mod.rs` — add `RELATION_WEIGHT_PROFILES`
- `crates/context-graph-core/src/graph_linking/edge_type.rs` — `GraphLinkEdgeType::default_weight_profile()`
- `crates/context-graph-storage/src/graph_edges/repository.rs` — attach profile hint to edge queries

### Improvement #3 — Multi-Path K=3
- New MCP tool: `crates/context-graph-mcp/src/handlers/tools/multi_path_tools.rs`
- `crates/context-graph-mcp/src/tools/names.rs` — add `multi_path_traverse` const
- `crates/context-graph-mcp/src/handlers/tools/dispatch.rs` — new dispatch arm
- `crates/context-graph-core/src/contrastive/types.rs` — extend `AnomalyKind` with `MultiPathDisagreement`
- `crates/context-graph-storage/src/teleological/rocksdb_store/contrastive.rs` — no schema change; new kind is a new byte

### Improvement #4 — Query Classifier
- New module: `crates/context-graph-mcp/src/adapters/query_classifier.rs`
- New grammar file: `models/hermes-2-pro/grammars/query_classifier.gbnf`
- `crates/context-graph-mcp/src/handlers/tools/memory_tools.rs` — `search_graph` fallback path
- `crates/context-graph-causal-agent/src/llm/mod.rs` — reuse LLM client
- Audit: `crates/context-graph-storage/src/teleological/rocksdb_store/audit_log.rs` — log classifier decisions

### Improvement #5 — CLS Lifecycle
- `crates/context-graph-core/src/types/fingerprint/teleological/types.rs` — `lifecycle_state` field
- `crates/context-graph-core/src/clustering/manager.rs` — lifecycle-filtered clustering
- New background task: `crates/context-graph-mcp/src/server/lifecycle_manager.rs`
- Migration: `crates/context-graph-storage/src/teleological/schema.rs` — default `Consolidated` for legacy records

### Improvement #6 — Regression-Detected Chains
- `crates/context-graph-core/src/causal/chain.rs` — plateau termination (shared with #1)
- MCP tool `get_causal_chain` response: add `terminated_at_plateau: bool`

### Improvement #7 — Active Embedder Mask
- `crates/context-graph-core/src/retrieval/query.rs` — `EmbeddingSpaceMask` already exists; plumb through
- `crates/context-graph-mcp/src/handlers/tools/memory_tools.rs` — honor mask in dispatch
- Observability: emit effective embedder count per query

### Improvement #8 — Ensemble-Agreement Termination
- `crates/context-graph-storage/src/teleological/search/pipeline/stages.rs` — Stage 3 Kendall-tau check
- No public API change; expose via logs

---

## 10. One Last Note

Spiking neural networks are genuinely beautiful computer science. They carry forty years of neuroscience, they are the substrate that the only general intelligence we've ever observed (the human brain) runs on, and they will eventually matter for AI systems that have to run on Mars rovers or in cochlear implants.

They are not, as of 2026, the right substrate for server-side text retrieval over 13-embedder decompositions on RTX 5090-class hardware. The **right** substrate is Rust + RocksDB + HNSW + Tokio, which is what we have.

But NCRS's team saw something real, and it was this: **graph walks deserve to be first-class algorithms**. They built goal-directed, plateau-terminated, relation-typed, multi-path walks on top of their spiking substrate because that was what they had. We should build the same walks on top of our HNSW substrate because that's what *we* have. The algorithms transfer; the substrate doesn't.

Thirty days. Eight improvements. Credit to the NCRS team for the ideas.

---

*End of document. Version 1.0, April 14, 2026.*
