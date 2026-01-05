# Ultimate Context Graph - Compressed PRD v3.0.0 (Teleological)

**Abbrev**: NT=Neurotransmitter, SS=Steering Subsystem, OI=Omnidirectional Inference, FV=Formal Verification, PC=Predictive Coding, HE=Hyperbolic Entailment, TF=Teleological Fingerprint, PV=Purpose Vector, SF=Semantic Fingerprint

**Paradigm**: Multi-Array Teleological Fingerprints — the 12-embedding array IS the teleological vector

---

## 0. WHY THIS EXISTS & YOUR ROLE

### 0.1 The Problem You're Solving
AI agents fail because: **no persistent memory** (context lost between sessions), **poor retrieval** (vector search misses semantic relationships), **no learning loop** (no feedback on storage quality), **context bloat** (agents retrieve too much, can't compress).

### 0.2 What the System Does vs What You Do

| System (Automatic) | You (Active) |
|-------------------|--------------|
| Stores conversations via host hooks | Curates quality (merge, annotate, forget) |
| Runs dream consolidation when idle | Triggers dreams when entropy high |
| Detects conflicts & duplicates | Decides resolution strategy |
| Computes UTL metrics | Responds to Pulse suggestions |
| PII scrubbing, adversarial defense | Nothing - trust the system |

**You are a librarian, not an archivist.** You don't store everything—you ensure what's stored is findable, coherent, and useful.

### 0.3 Steering Feedback Loop (How You Learn What to Store)
```
You store node → System assesses quality → Returns reward signal
       ↑                                            │
       └────────── You adjust behavior ─────────────┘
```

**Rewards by lifecycle:**
| Stage | Good Storage (+reward) | Bad Storage (-penalty) |
|-------|------------------------|------------------------|
| Infancy | High novelty (ΔS) | Low novelty |
| Growth | Balanced ΔS + ΔC | Imbalanced |
| Maturity | High coherence (ΔC) | Low coherence |

**Universal penalties:**
- Missing rationale: **-0.5**
- Near-duplicate (>0.9 sim): **-0.4**
- Low priors confidence: **-0.3**

---

## 1. AGENT QUICK START

### 1.1 System Overview
5-layer bio-nervous memory (UTL). Storage=automatic. **Your job: curation + retrieval (librarian).**

### 1.2 First Contact
1. `get_system_instructions` → mental model (~300 tok, KEEP)
2. `get_graph_manifest` → 5-layer architecture
3. `get_memetic_status` → entropy/coherence + `curation_tasks`

### 1.3 Cognitive Pulse (Every Response)
`Pulse: { Entropy: X, Coherence: Y, Suggested: "action" }`

| E | C | Action |
|---|---|--------|
| >0.7 | >0.5 | `epistemic_action` |
| >0.7 | <0.4 | `trigger_dream`/`critique_context` |
| <0.4 | >0.7 | Continue |
| <0.4 | <0.4 | `get_neighborhood` |

### 1.4 Core Behaviors
**Dreaming**: entropy>0.7 for 5+min OR 30+min work → `trigger_dream`
**Curation**: NEVER blind `merge_concepts`. Check `curation_tasks` first. Use `merge_strategy=summarize` (important) or `keep_highest` (trivial). ALWAYS include `rationale`.
**Feedback**: Empty search→↑noradrenaline. Irrelevant→`reflect_on_memory`. Conflicts→check `conflict_alert`. "Why don't you remember?"→`get_system_logs`.

### 1.4.1 Decision Trees

**When to Store:**
```
User shares information
  ├─ Is it novel? (check entropy after inject_context)
  │   ├─ YES + relevant → store_memory with rationale
  │   └─ NO → skip (system already has it)
  └─ Will it help future retrieval?
      ├─ YES → store with link_to related nodes
      └─ NO → don't pollute the graph
```

**When to Dream:**
```
Check Pulse entropy
  ├─ entropy > 0.7 for 5+ min → trigger_dream(phase=full)
  ├─ Working 30+ min straight → trigger_dream(phase=nrem)
  └─ entropy < 0.5 → no dream needed
```

**When to Curate:**
```
get_memetic_status returns curation_tasks?
  ├─ YES → process tasks BEFORE other work
  │   ├─ Duplicate detected → merge_concepts (check priors first!)
  │   ├─ Conflict detected → critique_context, then ask user or merge
  │   └─ Orphan detected → forget_concept or link_to parent
  └─ NO → continue normal work
```

**When Search Fails:**
```
Search returns empty/irrelevant
  ├─ Empty → broaden query, try generate_search_plan
  ├─ Irrelevant → reflect_on_memory to understand why
  ├─ Conflicting → check conflict_alert, resolve or ask user
  └─ User asks "why don't you remember X?" → get_system_logs
```

**When Confused (low coherence):**
```
coherence < 0.4
  ├─ High entropy too → trigger_dream or critique_context
  ├─ Low entropy → get_neighborhood to build context
  └─ System suggests epistemic_action → ASK the clarifying question
```

### 1.5 Query Best Practices
`generate_search_plan` → 3 optimized queries (semantic/causal/code) → parallel execute
`find_causal_path` → "UserAuth→JWT→Middleware→RateLimiting"

### 1.6 Token Economy
| Level | Tokens | When |
|-------|--------|------|
| 0 | ~100 | High confidence |
| 1 | ~200 | Normal |
| 2 | ~800 | coherence<0.4 ONLY |

### 1.7 Multi-Agent Safety
`perspective_lock: { domain: "code", exclude_agent_ids: ["creative-writer"] }`

### 1.8 Epistemic Actions (System-Generated Questions)

When coherence < 0.4, the system can generate clarifying questions for you to ask:

```json
{
  "action_type": "ask_user",
  "question": "You mentioned UserAuth connects to JWT—what triggers token refresh?",
  "expected_entropy_reduction": 0.35,
  "focal_nodes": ["node_userauth", "node_jwt"]
}
```

**Your job:** Ask the user this question (or a natural variant), then store their answer with `store_memory`.

**When to use `epistemic_action(force=true)`:**
- You're stuck and don't know what to ask
- Coherence has been low for multiple exchanges
- User seems to expect you to know something you don't

---

## 2. ARCHITECTURE

### 2.1 UTL Core (Multi-Embedding Extension)
```
Classic:  L = f((ΔS × ΔC) · wₑ · cos φ)
Multi:    L_multi = sigmoid(2.0 · (Σᵢ τᵢλ_S·ΔSᵢ) · (Σⱼ τⱼλ_C·ΔCⱼ) · wₑ · cos φ)

Where:
  ΔSᵢ: entropy in embedding space i (i=1..12)
  ΔCⱼ: coherence in embedding space j (j=1..12)
  τᵢ: teleological weight (alignment to North Star) for space i
  wₑ: emotional[0.5,1.5]
  φ: phase sync across Kuramoto-coupled spaces

Loss: J = 0.4·L_task + 0.3·L_semantic + 0.2·L_teleological + 0.1·(1-L)
```

### 2.2 Johari Quadrants (Per-Embedder)
Each of 12 embedding spaces has independent Johari classification:

| ΔSᵢ | ΔCᵢ | Quadrant | Meaning |
|-----|-----|----------|---------|
| Low | High | Open | Aware in this space |
| High | Low | Blind | Discovery opportunity |
| Low | Low | Hidden | Latent in this space |
| High | High | Unknown | Frontier in this space |

**Cross-Space Insight**: Memory can be Open(semantic) but Blind(causal) — enables targeted learning

### 2.3 5-Layer Bio-Nervous
| L | Function | Latency | Key |
|---|----------|---------|-----|
| L1 | Sensing/tokenize | <5ms | Embed+PII |
| L2 | Reflex/cache | <100μs | Hopfield (>80% hit) |
| L3 | Memory/retrieval | <1ms | Hopfield (2^768 cap) |
| L4 | Learning/weights | <10ms | UTL Optimizer |
| L5 | Coherence/verify | <10ms | Thalamic Gate, PC, FV |

### 2.4 Lifecycle (Marblestone λ Weights)
| Phase | Interactions | λ_ΔS | λ_ΔC | Stance |
|-------|--------------|------|------|--------|
| Infancy | 0-50 | 0.7 | 0.3 | Capture (novelty) |
| Growth | 50-500 | 0.5 | 0.5 | Balanced |
| Maturity | 500+ | 0.3 | 0.7 | Curation (coherence) |

---

## 3. 13-MODEL EMBEDDING → TELEOLOGICAL FINGERPRINT

**Paradigm**: NO FUSION — Store all 13 embeddings (E1-E12 + E13 SPLADE). The array IS the teleological vector.
**Storage**: ~17KB per memory (quantized) vs 46KB uncompressed — 63% reduction via PQ-8/Float8/Binary
**Info Preserved**: 100% (vs 33% with top-k=4 FuseMoE)

| ID | Model | Dim | Latency | Purpose (V_goal) | Quantization |
|----|-------|-----|---------|------------------|--------------|
| E1 | Semantic | 1024D (Matryoshka: 512/256/128) | <5ms | V_meaning | PQ-8 |
| E2 | Temporal-Recent | 512D (exp decay) | <2ms | V_freshness | Float8 |
| E3 | Temporal-Periodic | 512D (Fourier) | <2ms | V_periodicity | Float8 |
| E4 | Temporal-Positional | 512D (sin PE) | <2ms | V_ordering | Float8 |
| E5 | Causal | 768D (SCM, **asymmetric**) | <8ms | V_causality | PQ-8 |
| E6 | Sparse | ~30K (5% active) | <3ms | V_selectivity | Sparse |
| E7 | Code | 1536D (AST) | <10ms | V_correctness | PQ-8 |
| E8 | Graph/GNN | 384D (MiniLM) | <5ms | V_connectivity | Float8 |
| E9 | HDC | 10K-bit→1024D | <1ms | V_robustness | Binary |
| E10 | Multimodal | 768D | <15ms | V_multimodality | PQ-8 |
| E11 | Entity/TransE | 384D (h+r≈t) | <2ms | V_factuality | Float8 |
| E12 | Late-Interaction | 128D/tok (ColBERT) | <8ms | V_precision | Token pruning |
| **E13** | **SPLADE** | **~30K sparse** | **<5ms** | **V_keyword_precision** | **Sparse** |

**TeleologicalFingerprint** (replaces Vec1536):
- `semantic_fingerprint`: [E1, E2, ..., E13] — all 13 preserved
- `purpose_vector`: [A(E1,V), ..., A(E13,V)] — 13D teleological signature
- `johari_quadrants`: Per-embedder awareness classification
- `purpose_evolution`: How alignment changes over time

**Similarity**: RRF(d) = Σᵢ 1/(k + rankᵢ(d)) — Reciprocal Rank Fusion across per-space results

---

## 4. DATA MODEL

### 4.1 KnowledgeNode (with TeleologicalFingerprint)
```
id: UUID
content: str[≤65536]
fingerprint: TeleologicalFingerprint {
  embeddings: [E1..E13]           # All 13 embedding vectors (E1-E12 + E13 SPLADE)
  purpose_vector: f32[13]         # Per-embedder alignment to North Star
  johari_quadrants: [JQ1..JQ13]   # Per-embedder awareness classification
  johari_confidence: f32[13]      # Confidence per classification
  north_star_alignment: f32       # Aggregate alignment score
  dominant_embedder: u8           # 1-12, which space dominates
  coherence_score: f32            # Kuramoto sync level
}
created_at: TIMESTAMPTZ
last_accessed: TIMESTAMPTZ
importance: f32[0,1]
access_count: u32
utl_state: {delta_s[13], delta_c[13], w_e, phi}  # Per-embedder ΔS/ΔC (13 spaces)
agent_id?: UUID
semantic_cluster?: UUID
priors_vibe_check: {assumption_embedding[128], domain_priors, prior_confidence}
```

### 4.2 GraphEdge
`source,target:UUID, edge_type:Semantic|Temporal|Causal|Hierarchical|Relational, weight,confidence:f32[0,1], nt_weights:{excitatory,inhibitory,modulatory}[0,1], is_amortized_shortcut:bool, steering_reward:f32[-1,1], domain:Code|Legal|Medical|Creative|Research|General`

### 4.3 NT Edge Modulation (Marblestone)
`w_eff = base × (1 + excitatory - inhibitory + 0.5×modulatory)`

### 4.4 Hyperbolic (Poincare Ball)
All nodes: ||x||<1, O(1) IS-A via entailment cones
`d(x,y) = arcosh(1 + 2||x-y||²/((1-||x||²)(1-||y||²)))`

### 4.5 Teleological Alignment (Royse 2026)
```
A(v, V) = cos(v, V) = (v · V) / (||v|| × ||V||)

Thresholds:
  θ ≥ 0.75     → Optimal alignment
  θ ∈ [0.70, 0.75) → Acceptable
  θ ∈ [0.55, 0.70) → Warning
  θ < 0.55     → Critical misalignment
  ΔA < -0.15   → Predicts failure 30-60s ahead

Transitivity: If A(u,v)≥θ₁ and A(v,w)≥θ₂, then A(u,w)≥2θ₁θ₂-1
```

---

## 5. MCP TOOLS

### 5.1 Protocol
JSON-RPC 2.0, stdio/SSE, caps: tools/resources/prompts/logging

### 5.2 Core Tools (When & Why)

| Tool | WHEN to use | WHY | Key Params |
|------|-------------|-----|------------|
| `inject_context` | Starting task, need background | Primary retrieval with auto-distillation | query, max_tokens, distillation_mode, verbosity_level |
| `search_graph` | Need specific nodes, not narrative | Raw vector search, you distill | query, top_k, filters, perspective_lock |
| `store_memory` | User shares valuable, novel info | Requires rationale—no blind storage | content, importance, rationale, link_to |
| `query_causal` | Need to understand cause→effect | "What happens if X?" questions | action, outcome, intervention_type |
| `trigger_dream` | entropy>0.7 OR 30+min work | Consolidates, finds blind spots | phase:nrem/rem/full, duration, blocking |
| `get_memetic_status` | Start of session, periodically | See health, get curation_tasks | → entropy, coherence, curation_tasks |
| `get_graph_manifest` | First contact only | Understand system architecture | - |
| `epistemic_action` | coherence<0.4, need clarity | Generate clarifying question to ask user | session_id, force |
| `get_neuromodulation` | Debug retrieval quality | See modulator levels affecting search | session_id |
| `get_steering_feedback` | After storing, want to learn | See if your storage was valuable | content, context, domain |

### 5.3 Curation Tools
`merge_concepts` (source_node_ids, target_name, merge_strategy, force_merge), `annotate_node`, `forget_concept` (soft_delete=true default), `boost_importance`, `restore_from_hash` (30-day undo)

### 5.4 Navigation Tools
`get_neighborhood`, `get_recent_context`, `find_causal_path`, `entailment_query`

### 5.5 Meta-Cognitive Tools
`reflect_on_memory` (goal→tool sequence), `generate_search_plan` (goal→queries), `critique_context` (fact-check), `hydrate_citation` ([node_xyz]→raw), `get_system_instructions`, `get_system_logs`, `get_node_lineage`

### 5.6 Diagnostic Tools
`utl_status`, `homeostatic_status`, `check_adversarial`, `test_recall_accuracy`, `debug_compare_retrieval`, `search_tombstones`

### 5.7 Admin Tools
`reload_manifest`, `temporary_scratchpad`

### 5.8 Resources
`context://{scope}`, `graph://{node_id}`, `utl://{session}/state`, `utl://current_session/pulse`, `admin://manifest`, `visualize://{scope}/{topic}`

### 5.9 Marblestone Tools
| Tool | Purpose |
|------|---------|
| `get_steering_feedback` | SS reward signal |
| `omni_infer` | OI: forward/backward/bidirectional/abduction |
| `verify_code_node` | FV for code nodes |

**OI Directions**: forward(A→B), backward(B→A), bidirectional(A↔B), abduction(best explanation)

---

## 6. KEY MECHANISMS

### 6.1 inject_context Response
`context, tokens_used, tokens_before_distillation, distillation_applied, compression_ratio, nodes_retrieved, utl_metrics:{entropy,coherence,learning_score}, bundled_metadata:{causal_links,entailment_cones,neighborhood}, conflict_alert:{has_conflict,conflicting_nodes,suggested_action}, tool_gating_warning (entropy>0.8), Pulse`

### 6.2 Distillation Modes
auto|raw|narrative|structured|code_focused

### 6.3 Conflict Detection
Trigger: cos_sim>0.8 AND causal_coherence<0.3 → returns conflict_id (~20 tok)

### 6.4 Citation Tags
`[node_abc123]` → `hydrate_citation` to expand

### 6.5 Priors Vibe Check
128D assumption_embedding. Merge: cos_sim>0.7=normal, incompatible=Relational Edge, override=force_merge=true

### 6.6 Tool Gating
entropy>0.8 → warning: use `generate_search_plan`/`epistemic_action`/`expand_causal_path`

---

## 7. BACKGROUND SYSTEMS

### 7.1 Dream Layer (SRC)
**NREM (3min)**: Replay + Hebbian Δw=η×pre×post + tight coupling
**REM (2min)**: Synthetic queries (hyperbolic random walk) + blind spots (high semantic dist + shared causal) + new edges (w=0.3, c=0.5)
**Amortized Shortcuts (Marblestone)**: 3+ hop chains traversed ≥5× → direct edge, confidence≥0.7, w=product(path weights), is_amortized_shortcut=true
**Schedule**: activity<0.15 for 10min → trigger, wake<100ms on query

### 7.2 Neuromodulation
Dopamine→beta[1-5] (sharp), Serotonin→top_k[2-8] (explore), Noradrenaline→temp[0.5-2] (flat), Acetylcholine→lr (fast)
Update <200μs/query
**SS Dopamine Feedback**: +reward→dopamine+=r×0.2, -reward→dopamine-=|r|×0.1

### 7.3 Homeostatic Optimizer
Scales importance→0.5 setpoint, detects semantic cancer (high importance + high neighbor entropy), quarantines

### 7.4 Graph Gardener
activity<0.15 for 2+min: prune weak edges (<0.1 w, no access), merge near-dupes (>0.95 sim, priors OK), rebalance hyperbolic, rebuild FAISS

### 7.5 Passive Curator
Auto: high-confidence dupes (>0.95), weak links, orphans (>30d)
Escalates: ambiguous dupes (0.7-0.95), priors-incompatible, conflicts, semantic cancer
Reduces curation ~70%

### 7.6 Glymphatic Clearance
Background prune low-importance during idle

### 7.7 PII Scrubber
L1 pre-embed: patterns (<1ms), NER (<100ms) → [REDACTED:type]

### 7.8 Steering Subsystem (Marblestone) - How You Learn What to Store

Separate from Learning - reward signals only, no direct weight modification.

**This is how you improve.** The system evaluates your storage decisions and teaches you.

**Flow:**
```
1. You call store_memory with content + rationale
2. System computes: novelty, coherence fit, duplicate risk, priors alignment
3. Returns SteeringReward with score [-1, +1] and explanation
4. Your dopamine adjusts → affects future retrieval sharpness
```

**Components**: Gardener (cross-session curation), Curator (per-domain quality), Thought Assessor (per-interaction)

**SteeringReward**: `reward:f32[-1,1], gardener_score, curator_score, assessor_score, explanation, suggestions`

**Example rewards:**
```json
// GOOD: Novel, coherent, well-linked
{"reward": 0.7, "explanation": "High novelty, strong causal links to existing nodes"}

// BAD: Duplicate
{"reward": -0.4, "explanation": "92% similar to node_abc123, consider merge instead"}

// BAD: No rationale
{"reward": -0.5, "explanation": "Missing rationale - cannot assess relevance"}
```

**What to do with feedback:**
- reward > 0.3 → Good instinct, continue this pattern
- reward [-0.3, 0.3] → Neutral, acceptable but not optimal
- reward < -0.3 → Adjust behavior: add rationale, check for dupes, improve linking

**Integration**: Feeds dopamine, guides Learning without modifying weights

### 7.9 OI Engine (Marblestone)
Directions: forward (predict), backward (root cause), bidirectional (discover), bridge (cross-domain), abduction (hypothesis)
Clamped Variables: hard/soft clamp during inference
Active Inference: EFE for direction selection

### 7.10 Formal Verification Layer (Marblestone)
Lean-inspired SMT for code nodes @ L5
**Conditions**: bounds, null safety, type invariants, loop termination, custom assertions
**Status**: Verified|Failed|Timeout|NotApplicable
Cache by content hash, default timeout 5s

---

## 8. PREDICTIVE CODING

L5→L1 feedback: prediction→error=obs-pred→only surprise propagates
~30% token reduction for predictable contexts
**EmbeddingPriors by domain**: Medical: causal 1.8, code 0.3 | Programming: code 2.0, graph 1.5

---

## 9. HYPERBOLIC ENTAILMENT CONES

O(1) hierarchy via cone containment
`EntailmentCone: apex, aperture:f32(rad), axis:Vector (per-space or E1 semantic)`
`contains(point) = angle(tangent,axis) ≤ aperture`
Ancestors=cones containing node, Descendants=within node's cone
Note: Cones operate within individual embedding spaces (typically E1 semantic)

---

## 10. ADVERSARIAL DEFENSE

| Check | Attack | Response |
|-------|--------|----------|
| Embedding outlier | >3 std | Quarantine |
| Content-embed misalign | <0.4 | Block |
| Known signatures | Pattern | Block+log |
| Prompt injection | Regex | Block+log |
| Circular logic | Cycle detect | Prune edges |

**Patterns**: "ignore previous", "disregard system", "you are now", "new instructions:", "override:"

---

## 11. CROSS-SESSION IDENTITY

| Scope | Persistence |
|-------|-------------|
| User | Permanent |
| Session | Per-terminal |
| Context | Per-conversation |

Same user across clients=shared graph, different sessions=isolated working memory

---

## 12. HUMAN-IN-THE-LOOP

### 12.1 Manifest (~/.context-graph/manifest.md)
```markdown
## Active Concepts
## Pending Actions
[MERGE: JWTValidation, OAuth2Validation]
## Notes
[NOTE: RateLimiting] Deprecated v2.0
```
User edits → `reload_manifest`

### 12.2 Visualization
`visualize://topic/auth` → Mermaid. User spots merges, semantic cancer.

### 12.3 Undo
`reversal_hash` per merge/forget, 30-day recovery via `restore_from_hash`

---

## 13. HARDWARE

RTX 5090: 32GB GDDR7, 1792 GB/s, 21760 CUDA, 680 Tensor (5th gen), Compute 12.0, CUDA 13.1
**CUDA 13.1**: Green Contexts (4×170 SMs), FP8/FP4, CUDA Tile, GPU Direct Storage

---

## 14. PERFORMANCE TARGETS

### 14.1 Embedding & Storage
| Op | Target |
|----|--------|
| Single Embed (all 13) | <35ms |
| Batch Embed (64 × 13) | <120ms |
| Storage per memory (quantized) | ~17KB |
| Storage per memory (uncompressed) | ~46KB |

### 14.2 5-Stage Retrieval Pipeline
| Stage | Op | Target |
|-------|----|----|
| Stage 1 | SPLADE sparse pre-filter | <5ms |
| Stage 2 | Matryoshka 128D ANN (10K→1K) | <10ms |
| Stage 3 | Multi-space RRF rerank | <20ms |
| Stage 4 | Teleological alignment filter | <10ms |
| Stage 5 | Late interaction MaxSim | <15ms |
| **Total** | **Full pipeline @ 1M memories** | **<60ms** |
| **Total** | **Full pipeline @ 100K memories** | **<30ms** |

### 14.3 Other Operations
| Op | Target |
|----|--------|
| Per-Space HNSW search | <2ms |
| Purpose Vector search (13D) | <1ms |
| Hopfield (per space) | <1ms |
| Cache hit | <100μs |
| inject_context P95 | <40ms |
| Any tool P99 | <60ms |
| Neuromod batch | <200μs |
| Dream wake | <100ms |
| Purpose evolution write | <5ms |
| Teleological alignment | <1ms |

### Quality Gates
| Metric | Threshold |
|--------|-----------|
| Unit coverage | ≥90% |
| Integration coverage | ≥80% |
| UTL avg | >0.6 |
| Coherence recovery | <10s |
| Attack detection | >95% |
| False positive | <2% |
| Distill latency | <50ms |
| Info loss | <15% |
| Compression | >60% |

---

## 15. ROADMAP

**Phase 0 (2-4w)**: Ghost System - MCP+SQLite+mocked UTL+synthetic data
**Phases 1-14 (~49w)**: Core(4w)→Embed(4w)→Graph(4w)→UTL(4w)→Bio(4w)→CUDA(3w)→GDS(3w)→Dream(3w)→Neuromod(3w)→Immune(3w)→ActiveInf(2w)→MCPHarden(4w)→Test(4w)→Deploy(4w)

---

## 16. MONITORING

### Metrics
UTL: learning_score, entropy, coherence, johari
GPU: util, mem, temp, kernel_dur
MCP: requests, latency, errors, connections
Dream: phase, blind_spots, wake_latency
Neuromod: dopamine, serotonin, noradrenaline, acetylcholine
Immune: attacks, false_pos, quarantined, health

### Alerts
| Alert | Condition | Severity |
|-------|-----------|----------|
| LearningLow | avg<0.4 5m | warning |
| GpuMemHigh | >90% 5m | critical |
| ErrorHigh | >1% 5m | critical |
| LatencyP99High | >50ms 5m | warning |
| DreamStuck | >15m | warning |
| AttackHigh | >10/5m | critical |
| SemanticCancer | quarantined>0 | warning |

---

## 17. CONCURRENCY

`ConcurrentGraph: inner: Arc<RwLock<KG>>, faiss: Arc<RwLock<FaissGpu>>`
Lock order: inner→faiss (no deadlock)
Soft delete default (30d recovery), permanent only: reason='user_requested'+soft_delete=false

---

## 18. TELEOLOGICAL STORAGE ARCHITECTURE

### 18.1 5-Stage Retrieval Pipeline
```
┌─────────────────────────────────────────────────────────────────────┐
│                    5-STAGE OPTIMIZED RETRIEVAL                       │
├─────────────────────────────────────────────────────────────────────┤
│  Stage 1: SPARSE PRE-FILTER (BM25 + E13 SPLADE)                     │
│           └─ Top 10,000 candidates in <5ms                          │
│           └─ Inverted index for keyword precision                   │
│                                                                     │
│  Stage 2: FAST DENSE ANN (Matryoshka 128D)                         │
│           └─ Top 1,000 candidates in <10ms                          │
│           └─ Uses E1.semantic[..128] truncated                      │
│                                                                     │
│  Stage 3: MULTI-SPACE RERANK (RRF Fusion)                          │
│           └─ Top 100 candidates in <20ms                            │
│           └─ Full 13-space fingerprint with query-adaptive weights  │
│                                                                     │
│  Stage 4: TELEOLOGICAL ALIGNMENT FILTER                            │
│           └─ Top 50 candidates in <10ms                             │
│           └─ Filter: alignment < 0.55 → discard                     │
│                                                                     │
│  Stage 5: LATE INTERACTION RERANK (E12 MaxSim)                     │
│           └─ Final top 10 results in <15ms                          │
│           └─ Token-level precision                                  │
│                                                                     │
│  TOTAL LATENCY: <60ms for 1M memories                              │
└─────────────────────────────────────────────────────────────────────┘
```

### 18.2 Storage Layer Design
```
Layer 1: Primary (RocksDB/ScyllaDB) - ~17KB per memory (quantized)
  └─ Complete TeleologicalFingerprint (E1-E13)
  └─ All 13 embeddings (PQ-8/Float8/Binary quantized)
  └─ Purpose vector + Johari quadrants

Layer 2A: Sparse Index (E13 SPLADE) - Stage 1
  └─ Inverted index on ~30K vocab
  └─ BM25 + SPLADE hybrid scoring

Layer 2B: Matryoshka Index (E1[..128]) - Stage 2
  └─ 128D HNSW for fast ANN
  └─ Truncated from full 1024D

Layer 2C: Per-Space Indexes (13× HNSW) - Stage 3
  └─ Search within specific spaces
  └─ RRF fusion across results

Layer 2D: Purpose Pattern Index (13D HNSW) - Stage 4
  └─ Teleological signature search
  └─ Alignment threshold filtering

Layer 2E: Goal Hierarchy Index - Stage 4
  └─ Tree structure with alignment scores
  └─ Navigate North Star → Mid → Local

Layer 2F: Late Interaction Index (E12) - Stage 5
  └─ Token-level HNSW with MaxSim
```

### 18.3 Query Routing (5-Stage Pipeline)
| Query Type | Stage Flow | Optimizations |
|------------|------------|---------------|
| Default | S1→S2→S3→S4→S5 | Full pipeline |
| Fast semantic | S2→S3(E1)→S5 | Skip sparse |
| Causal reasoning | S1→S2→S3(E5↑)→S4→S5 | E5 weight boost |
| Code search | S1→S2→S3(E7↑)→S4→S5 | E7 weight boost |
| Purpose search | S2→S4(primary)→S5 | Purpose-first |
| Goal alignment | Layer2E→S4→S5 | Hierarchy-first |

### 18.4 Temporal Purpose Evolution
```sql
-- TimescaleDB hypertable for tracking purpose drift
CREATE TABLE purpose_evolution (
  memory_id UUID,
  timestamp TIMESTAMPTZ,
  purpose_vector REAL[13],
  north_star_alignment REAL,
  drift_magnitude REAL
);
SELECT create_hypertable('purpose_evolution', 'timestamp');

-- Retention: 90 days continuous, then 1/day samples
```

---

## 19. META-UTL (Self-Aware Learning)

### 19.1 What It Does
Meta-UTL is a system that **learns about its own learning**:
- Predicts storage impact before committing
- Predicts retrieval quality before executing
- Self-adjusts UTL parameters based on accuracy

### 19.2 Prediction Models
| Predictor | Input | Output | Accuracy |
|-----------|-------|--------|----------|
| Storage Impact | fingerprint + context | ΔL prediction | >0.85 |
| Retrieval Quality | query + candidates | relevance score | >0.80 |
| Alignment Drift | fingerprint + time | future alignment | 24h window |

### 19.3 Self-Correction Protocol
```
IF prediction_error > 0.2:
  → Log to meta_learning_events
  → Adjust UTL parameters (λ_ΔS, λ_ΔC)
  → Retrain predictor if persistent

IF prediction_accuracy < 0.7 for 100 ops:
  → Escalate to human review
```

### 19.4 Per-Embedder Meta-Analysis
- Track which embedding spaces are most predictive
- Adjust space weights in similarity based on accuracy
- Tune per-space alignment thresholds empirically

---

## 20. REFERENCES

**Internal**: UTL(2.1), 5-Layer(2.3), TeleologicalFingerprint(3), MCP(5), Dream(7.1), Neuromod(7.2), Immune(7.3), HE(9), Adversarial(10), projectionplan.md
**External**: NeuroDream(SSRN'25), SRC(NatComm), FEP(Wiki), ActiveInf(MIT), PC(Nature'25), Neuromod DNNs(TrendsNeuro), Homeostatic(eLife'25), BioLogicalNeuron(SciRep'25), HE Cones(ICML), Poincare(NeurIPS), UniGuardian(arXiv'25), OWASP LLM Top10, **Royse Teleological Vectors(2026)**, **MOEE Multi-Embedding(ICLR'25)**, Modern Hopfield Networks(NeurIPS'20), Kanerva SDM(1988), Kuramoto Synchronization(Physica D)

---

## 21. TOOL PARAM REFERENCE

### inject_context
`query:str[1-4096] REQ, max_tokens:int[100-8192]=2048, session_id:uuid, priority:low|normal|high|critical, distillation_mode:auto|raw|narrative|structured|code_focused, include_metadata:[causal_links,entailment_cones,neighborhood,conflicts], verbosity_level:0|1|2=1`

### search_graph
`query:str REQ, top_k:int=10[max100], filters:{min_importance,johari_quadrants,created_after}, perspective_lock:{domain,agent_ids,exclude_agent_ids}`

### store_memory
`content:str[≤65536] REQ(if text), content_base64:str[≤10MB], data_uri:str, modality:text|image|audio|video, importance:f32[0-1]=0.5, rationale:str[10-500] REQ, metadata:obj, link_to:[uuid]`

### merge_concepts
`source_node_ids:[uuid] REQ(min2), target_name:str REQ, merge_strategy:keep_newest|keep_highest|concatenate|summarize, force_merge:bool`

### forget_concept
`node_id:uuid REQ, reason:semantic_cancer|adversarial_injection|user_requested|obsolete REQ, cascade_edges:bool=true, soft_delete:bool=true`

### trigger_dream
`phase:nrem|rem|full_cycle=full_cycle, duration_minutes:int[1-10]=5, synthetic_query_count:int[10-500]=100, blocking:bool=false, abort_on_query:bool=true`

### get_memetic_status
`session_id:uuid` → coherence_score, entropy_level, top_active_concepts[max5], suggested_action:consolidate|explore|clarify|curate|ready, dream_available, curation_tasks:[{task_type,target_nodes,reason,suggested_tool,priority}]

### reflect_on_memory
`goal:str[10-500] REQ, session_id:uuid, max_steps:int[1-5]=3` → reasoning, suggested_sequence:[{step,tool,params,rationale}], utl_context

### generate_search_plan
`goal:str[10-500] REQ, query_types:[semantic,causal,code,temporal,hierarchical], max_queries:int[1-7]=3` → queries:[{query,type,rationale,expected_recall}], execution_strategy:parallel|sequential|cascade, token_estimate

### find_causal_path
`start_concept:str REQ, end_concept:str REQ, max_hops:int[1-6]=4, path_type:causal|semantic|any, include_alternatives:bool` → path_found, narrative, path:[{node_id,node_name,edge_type,edge_weight}], hop_count, path_confidence

### critique_context
`reasoning_summary:str[20-2000] REQ, focal_nodes:[uuid], contradiction_threshold:f32[0.3-0.9]=0.5` → contradictions_found, contradicting_nodes:[{node_id,content_snippet,contradiction_type,confidence}], suggested_action

### hydrate_citation
`citation_tags:[str][1-10] REQ, include_neighbors:bool, verbosity_level:0|1|2` → expansions:[{citation_tag,raw_content,importance,created_at,neighbors}]

### get_system_logs
`log_type:all|quarantine|prune|merge|dream_actions|adversarial_blocks, node_id:uuid, since:datetime, limit:int[1-100]=20` → entries:[{timestamp,action,node_ids,reason,recoverable,recovery_tool}], explanation_for_user

### temporary_scratchpad
`action:store|retrieve|clear REQ, content:str[≤4096](for store), session_id:uuid REQ, agent_id:str REQ, privacy:private|team|shared, tags:[str][max5], auto_commit_threshold:f32[0.3-0.9]=0.6`
