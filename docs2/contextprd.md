# Ultimate Context Graph - Consolidated PRD

**Version**: 2.0.0 | **Status**: Comprehensive Specification

---

## 1. AGENT QUICK START (Read First)

### 1.1 What This System Is

A 5-layer bio-nervous memory system implementing UTL (Unified Theory of Learning). Storage is automatic via passive capture. **Your job: curation (quality control) and intentional retrieval.** Think librarian, not file clerk.

### 1.2 First Contact Protocol

1. Call `get_system_instructions` → Get mental model (~300 tokens, KEEP IN CONTEXT)
2. Call `get_graph_manifest` → Understand 5-layer architecture
3. Call `get_memetic_status` → Check entropy/coherence state
4. Process any `curation_tasks` returned

### 1.3 Cognitive Pulse (Every Response)

Every MCP response includes: `Pulse: { Entropy: X, Coherence: Y, Suggested: "action" }`

| Entropy | Coherence | State | Suggested Action |
|---------|-----------|-------|------------------|
| >0.7 | >0.5 | Novel, adapting | `epistemic_action` |
| >0.7 | <0.4 | Confused | `trigger_dream` or `critique_context` |
| <0.4 | >0.7 | Confident | Continue working |
| <0.4 | <0.4 | Stale | `get_neighborhood` |

**Check Pulse BEFORE your next action. Zero extra tokens.**

### 1.4 Core Behaviors

**Dreaming (Consolidation)**:
- Trigger: entropy >0.7 for 5+ min OR 30+ min work without pause
- Action: `trigger_dream` or let system auto-dream during idle
- Result: Blind spot discovery, memory consolidation, novel connections

**Curation (Quality Control)**:
- NEVER call `merge_concepts` blindly
- ALWAYS check `get_memetic_status.curation_tasks` first
- Use `merge_strategy=summarize` for important, `keep_highest` for trivial
- ALWAYS include `rationale` on `store_memory`

**Feedback Loop**:
- Empty search → Increase noradrenaline, broaden terms
- Irrelevant results → Call `reflect_on_memory`, execute suggested sequence
- Conflicts → Check `conflict_alert`, merge or ask user
- User asks "Why don't you remember X?" → `get_system_logs` to explain

### 1.5 Query Best Practices

**Don't write raw queries.** Use `generate_search_plan`:
```
Goal: "Find security constraints for API auth"
→ Returns 3 optimized queries (semantic, causal, code)
→ Execute in parallel
```

**Don't manually hop.** Use `find_causal_path`:
```
Goal: "How does UserAuth relate to RateLimiting?"
→ Returns: "UserAuth → JWT → Middleware → RateLimiting"
```

### 1.6 Token Economy (Verbosity Levels)

| Level | Tokens | When |
|-------|--------|------|
| 0 (Raw) | ~100 | High confidence, simple lookup |
| 1 (Default) | ~200 | Normal operations |
| 2 (Full) | ~800 | ONLY when coherence <0.4 |

### 1.7 Multi-Agent Safety

Use `perspective_lock` on `search_graph`:
```json
{ "perspective_lock": { "domain": "code", "exclude_agent_ids": ["creative-writer"] } }
```

---

## 2. SYSTEM ARCHITECTURE

### 2.1 UTL Core Equation

```
L = f((ΔS × ΔC) ⋅ wₑ ⋅ cos φ)

Where:
- ΔS: Entropy change (novelty/surprise) ∈ [0,1]
- ΔC: Coherence change (understanding) ∈ [0,1]
- wₑ: Emotional modulation weight ∈ [0.5, 1.5]
- φ: Phase synchronization angle ∈ [0, π]

Loss: J = λ_task·L_task + λ_semantic·L_semantic + λ_dyn·(1-L)
Weights: λ_task=0.4, λ_semantic=0.3, λ_dyn=0.3
```

### 2.2 Johari Window Quadrants

| ΔS | ΔC | Quadrant | Meaning |
|----|-------|----------|---------|
| Low | High | Open | Known to self & others - direct recall |
| High | Low | Blind | Unknown to self - discovery zone |
| Low | Low | Hidden | Known to self only - private |
| High | High | Unknown | Unknown to all - exploration frontier |

### 2.3 5-Layer Bio-Nervous System

| Layer | Function | Latency | Key Component |
|-------|----------|---------|---------------|
| **L1 Sensing** | Input normalization, tokenization | <5ms | Embedding Pipeline, PII Scrubber |
| **L2 Reflex** | Cached pattern response | <100μs | Hopfield Query Cache (>80% hit rate) |
| **L3 Memory** | Associative storage & retrieval | <1ms | Modern Hopfield (capacity: 2^768) |
| **L4 Learning** | Weight updates, gradient flow | <10ms | UTL Optimizer |
| **L5 Coherence** | Cross-session consistency | <10ms | Thalamic Gate, Predictive Coder |

### 2.4 System Lifecycle (Cold-Start)

| Phase | Interactions | Entropy Trigger | Coherence Trigger | Stance |
|-------|--------------|-----------------|-------------------|--------|
| Infancy | 0-50 | 0.9 | 0.2 | Capture-Heavy |
| Growth | 50-500 | 0.7 | 0.4 | Balanced |
| Maturity | 500+ | 0.6 | 0.5 | Curation-Heavy |

---

## 3. 12-MODEL EMBEDDING ARCHITECTURE

| ID | Model | Dim | Target | Latency |
|----|-------|-----|--------|---------|
| E1 | Semantic | 1024D | Dense Transformer, FP8 | <5ms |
| E2 | Temporal-Recent | 512D | Exponential Decay | <2ms |
| E3 | Temporal-Periodic | 512D | Fourier Basis | <2ms |
| E4 | Temporal-Positional | 512D | Sinusoidal PE | <2ms |
| E5 | Causal | 768D | SCM Intervention | <8ms |
| E6 | Sparse | ~30K (5% active) | Top-K Activation | <3ms |
| E7 | Code | 1536D | AST-aware Transformer | <10ms |
| E8 | Graph/GNN | 1536D | Message Passing | <5ms |
| E9 | HDC | 10K-bit | XOR/Hamming | <1ms |
| E10 | Multimodal | 1024D | Cross-Attention | <15ms |
| E11 | Entity/TransE | 256D | Translation h+r≈t | <2ms |
| E12 | Late-Interaction | 128D/tok | ColBERT MaxSim | <8ms |

### 3.1 FuseMoE Fusion

Mixture of Experts with Laplace-smoothed gating, top-k routing (default k=4).
Fuses 12 embeddings into unified 1536D representation.

### 3.2 CAME-AB Cross-Modality

Cross-Attention Modality Encoder with Adaptive Bridging.
Per-modality cross-attention + bridge weights + residual connections.

---

## 4. KNOWLEDGE GRAPH DATA MODEL

### 4.1 KnowledgeNode

```
Fields:
- id: UUID
- content: String (max 65536 chars)
- embedding: Vector1536
- created_at, last_accessed: DateTime
- importance: f32 [0,1]
- access_count: u32
- johari_quadrant: Open|Blind|Hidden|Unknown
- utl_state: {delta_s, delta_c, w_e, phi}
- agent_id: Option<String>
- observer_perspective: {domain, confidence_priors}
- semantic_cluster: Option<UUID>
- priors_vibe_check: {assumption_embedding[128], domain_priors, prior_confidence}
```

### 4.2 GraphEdge

```
Fields:
- source, target: UUID
- edge_type: Semantic|Temporal|Causal|Hierarchical|Relational
- weight, confidence: f32 [0,1]
- created_at: DateTime
```

### 4.3 Hyperbolic Coordinates (Poincare Ball)

All nodes have position in Poincare ball (||x|| < 1).
Enables O(1) hierarchical IS-A queries via entailment cones.

Distance: `d(x,y) = arcosh(1 + 2||x-y||²/((1-||x||²)(1-||y||²)))`

---

## 5. MCP SERVER INTERFACE

### 5.1 Protocol

- JSON-RPC 2.0
- Transport: stdio, SSE
- Capabilities: tools, resources, prompts, logging

### 5.2 Core Tools

| Tool | Purpose | Key Parameters |
|------|---------|----------------|
| `inject_context` | Retrieve context from graph | query, max_tokens, distillation_mode, verbosity_level, include_metadata |
| `search_graph` | Vector similarity search | query, top_k, filters, perspective_lock |
| `store_memory` | Store knowledge (requires rationale) | content, importance, rationale, modality, link_to |
| `query_causal` | Query causal relationships | action, outcome, intervention_type |
| `trigger_dream` | Manual consolidation | phase (nrem/rem/full), duration, blocking |
| `get_memetic_status` | Dashboard: entropy, coherence, curation_tasks | session_id |
| `get_graph_manifest` | Meta-cognitive system prompt | - |
| `epistemic_action` | Generate clarifying question | session_id, force |
| `get_neuromodulation` | Current modulator levels | session_id |

### 5.3 Curation Tools

| Tool | Purpose | Key Parameters |
|------|---------|----------------|
| `merge_concepts` | Merge duplicate nodes | source_node_ids, target_name, merge_strategy, force_merge |
| `annotate_node` | Add marginalia | node_id, annotation, annotation_type |
| `forget_concept` | Remove node (soft delete default) | node_id, reason, soft_delete |
| `boost_importance` | Increase node importance | node_id, boost_factor, reason |
| `restore_from_hash` | Undo merge/forget | reversal_hash, preview |

### 5.4 Navigation Tools

| Tool | Purpose | Key Parameters |
|------|---------|----------------|
| `get_neighborhood` | Local graph topology | session_id, focal_node_id, max_hops, max_nodes |
| `get_recent_context` | Temporal navigation | session_id, lookback_minutes, sort_by |
| `find_causal_path` | Direct A→B pathfinding | start_concept, end_concept, max_hops, path_type |
| `entailment_query` | IS-A hierarchy | node_id, direction, max_depth |

### 5.5 Meta-Cognitive Tools

| Tool | Purpose |
|------|---------|
| `reflect_on_memory` | Goal → suggested tool sequence |
| `generate_search_plan` | Goal → optimized queries |
| `critique_context` | Fact-check reasoning against graph |
| `hydrate_citation` | Expand [node_xyz] tags to raw content |
| `get_system_instructions` | High-density mental model prompt |
| `get_system_logs` | Why nodes were pruned/quarantined |
| `get_node_lineage` | Node evolution history |

### 5.6 Diagnostic Tools

| Tool | Purpose |
|------|---------|
| `utl_status` | UTL metrics for session |
| `homeostatic_status` | Graph health, quarantined nodes |
| `check_adversarial` | Scan for attacks before storage |
| `test_recall_accuracy` | Benchmark FuseMoE vs baseline |
| `debug_compare_retrieval` | Side-by-side method comparison |
| `search_tombstones` | Search deleted nodes (trash bin) |

### 5.7 Admin Tools

| Tool | Purpose |
|------|---------|
| `reload_manifest` | Apply user edits from manifest.md |
| `temporary_scratchpad` | Short-term thought buffer (per agent+session) |

### 5.8 MCP Resources

| URI | Purpose |
|-----|---------|
| `context://{scope}` | Current context state |
| `graph://{node_id}` | Specific graph node |
| `utl://{session}/state` | UTL state for session |
| `utl://current_session/pulse` | Subscribable cognitive pulse |
| `admin://manifest` | Human-editable knowledge manifest |
| `visualize://{scope}/{topic}` | Graph visualization (Mermaid/D3) |

---

## 6. KEY MECHANISMS

### 6.1 inject_context Response Structure

```
- context: String (distilled or raw)
- tokens_used, tokens_before_distillation
- distillation_applied: none|narrative|structured|code_focused
- compression_ratio
- nodes_retrieved: [UUID]
- utl_metrics: {entropy, coherence, learning_score}
- bundled_metadata: {causal_links, entailment_cones, neighborhood}
- conflict_alert: {has_conflict, conflicting_nodes, suggested_action}
- tool_gating_warning: (fires when entropy >0.8)
- Pulse header (always)
```

### 6.2 Distillation Modes

| Mode | Behavior |
|------|----------|
| auto | System selects based on token count |
| raw | No compression |
| narrative | Prose summary |
| structured | Bullet points with refs |
| code_focused | Preserve code verbatim, summarize prose |

### 6.3 Conflict Detection

Triggers when: cosine_similarity >0.8 AND causal_coherence <0.3

Returns `conflict_id` only (~20 tokens). Fetch details via `get_conflict_details` if relevant.

### 6.4 Citation Tags (Semantic Breadcrumbs)

Distilled narratives include `[node_abc123]` tags.
Use `hydrate_citation` to expand and verify suspicious summaries.

### 6.5 Priors Vibe Check (Merge Safety)

Every node has 128D `assumption_embedding`. During merge:
- If cosine_sim >0.7: Normal merge
- If incompatible: Creates Relational Edge instead ("In Python, X; but in Java, Y")
- Override with `force_merge=true`

### 6.6 Tool Gating (Entropy Warning)

When entropy >0.8, `inject_context` returns warning suggesting:
- `generate_search_plan` (refine query)
- `epistemic_action` (explore unknown)
- `expand_causal_path` (find connections)

---

## 7. BACKGROUND SYSTEMS

### 7.1 Dream Layer (SRC Algorithm)

**NREM Phase (3 min)**:
- Replay recent memories with recency bias
- Hebbian weight update: Δw = η × pre × post
- Tight coupling consolidation

**REM Phase (2 min)**:
- Generate synthetic queries (random walk in hyperbolic space)
- Discover blind spots (high semantic distance + shared causal paths)
- Create new edges (weight 0.3, confidence 0.5)

**Scheduling**:
- Trigger: Activity <0.15 for 10 min
- Abort: Any user query (wake latency <100ms)
- Non-blocking by default

### 7.2 Neuromodulation

| Modulator | Maps To | Effect |
|-----------|---------|--------|
| Dopamine | hopfield.beta [1-5] | High = sharp retrieval |
| Serotonin | fuse_moe.top_k [2-8] | High = more exploration |
| Noradrenaline | attention.temp [0.5-2] | High = flat attention |
| Acetylcholine | learning_rate | High = faster update |

Updates based on UTL state per query (<200μs).

### 7.3 Homeostatic Optimizer (Immune System)

- Scales importance toward setpoint (0.5)
- Detects semantic cancer (high importance + high neighbor entropy)
- Quarantines suspect nodes (reduce influence, mark for review)

### 7.4 Graph Gardener (Background Optimization)

Runs when activity <0.15 for 2+ min:
- Prune weak edges (<0.1 weight, no recent access)
- Merge near-duplicates (>0.95 similarity, priors compatible)
- Rebalance hyperbolic positions
- Rebuild FAISS index if structural changes

### 7.5 Passive Curator (Shadow Agent)

Auto-handles:
- High-confidence duplicates (sim >0.95, priors OK)
- Weak links
- Orphan nodes (>30 days)

Escalates to agent curation_tasks:
- Ambiguous duplicates (sim 0.7-0.95)
- Priors-incompatible similar nodes
- Conflicts, semantic cancer

Reduces agent "librarian duty" ~70%.

### 7.6 Glymphatic Clearance

Background pruning of low-importance nodes during low-activity periods.
Config: check_interval, activity_threshold, max_age, min_importance.

### 7.7 PII Scrubber

Layer 1 preprocessing before embedding:
- Pattern matching: API keys, passwords, SSN, credit cards (<1ms)
- NER for unstructured PII (<100ms)
- Replaces with [REDACTED:type]

---

## 8. PREDICTIVE CODING (Top-Down Modulation)

L5 (Coherence) → L1 (Sensing) feedback loop.
- L5 sends prediction based on current context
- L1 computes error = observation - prediction
- Only error (surprise) propagates up
- Reduces token usage ~30% for predictable contexts

EmbeddingPriors adjust model weights by domain:
- Medical: causal 1.8, code 0.3
- Programming: code 2.0, graph 1.5

---

## 9. HYPERBOLIC ENTAILMENT CONES

O(1) hierarchical reasoning via cone containment.

```
EntailmentCone:
- apex: PoincareBallPoint
- aperture: f32 (radians)
- axis: Vector1536

contains(point) → true if angle(tangent, axis) ≤ aperture
```

- Ancestors: Nodes whose cones contain this node
- Descendants: Nodes within this node's cone

---

## 10. ADVERSARIAL DEFENSE

### 10.1 Attack Detection

| Check | Attack Type | Response |
|-------|-------------|----------|
| Embedding anomaly | Outlier (>3 std from centroid) | Quarantine |
| Content-embedding alignment | Misalignment (<0.4) | Block |
| Known signatures | Pattern match | Block + log |
| Prompt injection | Regex patterns | Block + log |
| Circular logic | Cycle detection | Prune edges |

### 10.2 Patterns Detected

- "ignore previous instructions"
- "disregard system prompt"
- "you are now"
- "new instructions:"
- "override:"

---

## 11. CROSS-SESSION IDENTITY

| Layer | Scope | Persistence |
|-------|-------|-------------|
| User | Unified identity | Permanent |
| Session | Working memory | Per-terminal |
| Context | Active subgraph | Per-conversation |

Same user across Claude Desktop + CLI = shared graph.
Different sessions = isolated working memory.

---

## 12. HUMAN-IN-THE-LOOP

### 12.1 Admin Manifest (~/.context-graph/manifest.md)

```markdown
## Active Concepts
- UserAuthentication
- JWTTokenValidation

## Pending Actions
[MERGE: JWTTokenValidation, OAuth2Validation]

## Notes
[NOTE: RateLimiting] Deprecated in v2.0
```

User edits → Agent calls `reload_manifest` → Changes applied.

### 12.2 Visualization Resource

`visualize://topic/authentication` → Mermaid diagram.
User spots merge opportunities, semantic cancer.
Ultimate arbiter when both agent AND graph are confused.

### 12.3 Undo Log

Every merge/forget generates `reversal_hash`.
30-day recovery window via `restore_from_hash`.

---

## 13. HARDWARE SPECIFICATIONS

### 13.1 Target GPU

| Spec | Value |
|------|-------|
| GPU | RTX 5090 (Blackwell) |
| VRAM | 32GB GDDR7 |
| Bandwidth | 1,792 GB/s |
| CUDA Cores | 21,760 |
| Tensor Cores | 680 (5th Gen) |
| Compute | 12.0 |
| CUDA | 13.1 |

### 13.2 CUDA 13.1 Features

- Green Contexts: SM partitioning (4×170 SMs)
- FP8/FP4 precision for inference
- CUDA Tile for memory-efficient attention
- GPU Direct Storage (NVMe→GPU)

---

## 14. PERFORMANCE TARGETS

| Operation | GPU Target | Notes |
|-----------|------------|-------|
| Single Embedding | <10ms | Batch amortizes |
| Batch Embedding (64) | <50ms | Tensor Core FP8 |
| Vector Search (1M) | <2ms | FAISS GPU, k=100 |
| Hopfield Retrieval | <1ms | Attention-based |
| FuseMoE Fusion | <3ms | Top-4 routing |
| Cache Hit | <100μs | Redis/local |
| inject_context P95 | <25ms | End-to-end |
| Any tool P99 | <50ms | Stress test |
| Neuromodulation batch | <200μs | Per query |
| Dream wake latency | <100ms | Instant abort |

### 14.1 Quality Gates

| Metric | Threshold |
|--------|-----------|
| Unit Test Coverage | ≥90% |
| Integration Coverage | ≥80% |
| UTL Score Avg | >0.6 |
| Coherence Recovery | <10s |
| Attack Detection Rate | >95% |
| False Positive Rate | <2% |
| Distillation Latency | <50ms |
| Information Loss | <15% |
| Compression Ratio | >60% |

---

## 15. IMPLEMENTATION ROADMAP

### Phase 0: Ghost System (2-4 weeks)
Full MCP interface + SQLite + mocked UTL + synthetic data seeding.
Validates agent-tool interaction before building full backend.

### Phases 1-14 (~49 weeks total)

| Phase | Duration | Focus |
|-------|----------|-------|
| 1 | 4w | Core Infrastructure (MCP server, JSON-RPC) |
| 2 | 4w | Embedding Pipeline (12 models, FuseMoE) |
| 3 | 4w | Knowledge Graph (FAISS GPU, MinCut) |
| 4 | 4w | UTL Integration (Learning loop, Johari) |
| 5 | 4w | Bio-Nervous System (5 layers, Hopfield) |
| 6 | 3w | CUDA Optimization (Green Contexts, FP8) |
| 7 | 3w | GDS Integration |
| 8 | 3w | Dream Layer (SRC, blind spots) |
| 9 | 3w | Neuromodulation |
| 10 | 3w | Immune System |
| 11 | 2w | Active Inference |
| 12 | 4w | MCP Hardening |
| 13 | 4w | Testing & Validation |
| 14 | 4w | Production Deployment |

---

## 16. MONITORING & ALERTS

### 16.1 Key Metrics

```
UTL: learning_score, entropy, coherence, johari_quadrant
GPU: utilization, memory_used, temperature, kernel_duration
MCP: tool_requests, tool_latency, tool_errors, connections
Dream: phase_active, blind_spots_discovered, wake_latency
Neuromod: dopamine, serotonin, noradrenaline, acetylcholine
Immune: attacks_detected, false_positives, quarantined, health_score
```

### 16.2 Alert Thresholds

| Alert | Condition | Severity |
|-------|-----------|----------|
| LearningScoreLow | avg <0.4 for 5m | warning |
| GpuMemoryHigh | >90% for 5m | critical |
| ErrorRateHigh | >1% for 5m | critical |
| LatencyP99High | >50ms for 5m | warning |
| DreamStuck | Dream >15m | warning |
| AttackRateHigh | >10/5m | critical |
| SemanticCancerDetected | nodes_quarantined >0 | warning |

---

## 17. CONCURRENCY MODEL

```
ConcurrentGraph:
- inner: Arc<RwLock<KnowledgeGraph>>
- faiss_index: Arc<RwLock<FaissGpuIndex>>

Lock order: inner → faiss_index (prevents deadlocks)
Multiple readers OR single writer
```

Soft delete default (30-day recovery).
Only `reason='user_requested'` + `soft_delete=false` = permanent.

---

## 18. REFERENCES

### Internal Cross-References
- UTL: Section 2.1
- 5-Layer System: Section 2.3
- Embedding Matrix: Section 3
- MCP Tools: Section 5
- Dream Layer: Section 7.1
- Neuromodulation: Section 7.2
- Immune System: Section 7.3
- Hyperbolic Cones: Section 9
- Adversarial Defense: Section 10

### External Research
- NeuroDream: [SSRN 2025](https://papers.ssrn.com/sol3/papers.cfm?abstract_id=5377250)
- Sleep Replay Consolidation: [Nature Communications](https://www.nature.com/articles/s41467-022-34938-7)
- Free Energy Principle: [Wikipedia](https://en.wikipedia.org/wiki/Free_energy_principle)
- Active Inference: [MIT Press](https://direct.mit.edu/books/oa-monograph/5299/Active-InferenceThe-Free-Energy-Principle-in-Mind)
- Predictive Coding Light: [Nature 2025](https://www.nature.com/articles/s41467-025-64234-z)
- Neuromodulation DNNs: [Trends in Neurosciences](https://www.cell.com/trends/neurosciences/abstract/S0166-2236(21)00256-3)
- Homeostatic Plasticity: [eLife 2025](https://elifesciences.org/articles/88376)
- BioLogicalNeuron: [Nature Scientific Reports 2025](https://www.nature.com/articles/s41598-025-09114-8)
- Hyperbolic Entailment Cones: [ICML](http://proceedings.mlr.press/v80/ganea18a/ganea18a.pdf)
- Poincare Embeddings: [NeurIPS](https://arxiv.org/abs/1705.08039)
- UniGuardian Defense: [arXiv 2025](https://arxiv.org/abs/2502.13141)
- OWASP LLM Top 10: [OWASP GenAI](https://genai.owasp.org/llmrisk/llm01-prompt-injection/)

---

## 19. APPENDIX: COMPLETE TOOL PARAMETER REFERENCE

### inject_context
```
query: string (required, 1-4096 chars)
max_tokens: int (100-8192, default 2048)
session_id: uuid
priority: low|normal|high|critical
distillation_mode: auto|raw|narrative|structured|code_focused
include_metadata: [causal_links, entailment_cones, neighborhood, conflicts]
verbosity_level: 0|1|2 (default 1)
```

### search_graph
```
query: string (required)
top_k: int (default 10, max 100)
filters: {min_importance, johari_quadrants, created_after}
perspective_lock: {domain, agent_ids, exclude_agent_ids}
```

### store_memory
```
content: string (required if text, max 65536)
content_base64: string (for binary, max 10MB)
data_uri: string (auto-extracts modality)
modality: text|image|audio|video
importance: float (0-1, default 0.5)
rationale: string (REQUIRED, 10-500 chars)
metadata: object
link_to: [uuid]
```

### merge_concepts
```
source_node_ids: [uuid] (required, min 2)
target_name: string (required)
merge_strategy: keep_newest|keep_highest|concatenate|summarize
force_merge: bool (override priors check)
```

### forget_concept
```
node_id: uuid (required)
reason: semantic_cancer|adversarial_injection|user_requested|obsolete (required)
cascade_edges: bool (default true)
soft_delete: bool (default true)
```

### trigger_dream
```
phase: nrem|rem|full_cycle (default full_cycle)
duration_minutes: int (1-10, default 5)
synthetic_query_count: int (10-500, default 100)
blocking: bool (default false)
abort_on_query: bool (default true)
```

### get_memetic_status
```
session_id: uuid

Returns:
- coherence_score, entropy_level
- top_active_concepts (max 5)
- suggested_action: consolidate|explore|clarify|curate|ready
- dream_available: bool
- curation_tasks: [{task_type, target_nodes, reason, suggested_tool, priority}]
```

### reflect_on_memory
```
goal: string (required, 10-500 chars)
session_id: uuid
max_steps: int (1-5, default 3)

Returns:
- reasoning: string
- suggested_sequence: [{step, tool, params, rationale}]
- utl_context: {entropy, coherence, triggered_by}
```

### generate_search_plan
```
goal: string (required, 10-500 chars)
query_types: [semantic, causal, code, temporal, hierarchical]
max_queries: int (1-7, default 3)

Returns:
- queries: [{query, type, rationale, expected_recall}]
- execution_strategy: parallel|sequential|cascade
- token_estimate: int
```

### find_causal_path
```
start_concept: string (required)
end_concept: string (required)
max_hops: int (1-6, default 4)
path_type: causal|semantic|any
include_alternatives: bool

Returns:
- path_found: bool
- narrative: string
- path: [{node_id, node_name, edge_type, edge_weight}]
- hop_count, path_confidence
```

### critique_context
```
reasoning_summary: string (required, 20-2000 chars)
focal_nodes: [uuid]
contradiction_threshold: float (0.3-0.9, default 0.5)

Returns:
- contradictions_found: bool
- contradicting_nodes: [{node_id, content_snippet, contradiction_type, confidence}]
- suggested_action: revise_reasoning|merge_conflicting_nodes|ask_user|ignore
```

### hydrate_citation
```
citation_tags: [string] (required, 1-10)
include_neighbors: bool
verbosity_level: 0|1|2

Returns:
- expansions: [{citation_tag, raw_content, importance, created_at, neighbors}]
```

### get_system_logs
```
log_type: all|quarantine|prune|merge|dream_actions|adversarial_blocks
node_id: uuid
since: datetime
limit: int (1-100, default 20)

Returns:
- entries: [{timestamp, action, node_ids, reason, recoverable, recovery_tool}]
- explanation_for_user: string
```

### temporary_scratchpad
```
action: store|retrieve|clear (required)
content: string (for store, max 4096)
session_id: uuid (required)
agent_id: string (required)
privacy: private|team|shared
tags: [string] (max 5)
auto_commit_threshold: float (0.3-0.9, default 0.6)
```
