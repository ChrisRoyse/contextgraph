# Context Graph PRD v5 (Agent Reference)

**Platform**: Claude Code CLI ONLY | **Abbrev**: NT=Neurotransmitter, SS=Steering, OI=Omnidirectional Inference, FV=Formal Verification, PC=Predictive Coding, HE=Hyperbolic Entailment, TF=Teleological Fingerprint, PV=Purpose Vector, SF=Semantic Fingerprint, GWT=Global Workspace Theory, GW=Global Workspace, IIT=Integrated Information Theory, CMS=Continuum Memory System, IC=Identity Continuity

**Paradigm**: 13-embedding Teleological Fingerprints + GWT Consciousness + Kuramoto sync → **Native Claude Code hooks** (`.claude/settings.json`) + skills + subagents

**⚠️ CRITICAL: NATIVE CLAUDE CODE HOOKS ONLY**

This system uses **NATIVE Claude Code hooks** configured via `.claude/settings.json` — NOT internal/built-in hooks. All hook integration happens through the official Claude Code hook system with shell script executors that call `context-graph-cli` commands. This is a fundamental architectural decision that reduces complexity by ~71% vs. building a custom hook system.

---

## QUICKREF: KEY THRESHOLDS

| Symbol | Value | Meaning |
|--------|-------|---------|
| Tᵣ | 0.8 | Kuramoto coherence (r > Tᵣ = conscious) |
| Tᵣ_low | 0.5 | Fragmentation alert |
| θ_opt | 0.75 | Optimal alignment |
| θ_acc | 0.70 | Acceptable alignment |
| θ_warn | 0.55 | Warning alignment |
| IC_crit | 0.5 | Identity crisis trigger |
| IC_warn | 0.7 | Identity drift warning |
| ent_high | 0.7 | High entropy (trigger dream) |
| ent_low | 0.4 | Low entropy (stable) |
| dup_sim | 0.9 | Duplicate threshold |
| edge_sim | 0.7 | Edge creation threshold |
| ΔA_fail | -0.15 | Predicts failure 30-60s ahead |

**Formulas**:
- UTL: `L = sigmoid(2·(Σᵢτᵢλ_S·ΔSᵢ)·(Σⱼτⱼλ_C·ΔCⱼ)·wₑ·cosφ)`
- Consciousness: `C(t) = I(t)×R(t)×D(t)` where I=Kuramoto r, R=MetaUTL accuracy, D=13D entropy
- Identity: `IC = cosine(PV_t, PV_{t-1}) × r(t)`
- Alignment: `A(v,V) = cos(v,V) = (v·V)/(||v||×||V||)`

**States**: DORMANT(r<0.3) → FRAGMENTED(0.3≤r<0.5) → EMERGING(0.5≤r<0.8) → CONSCIOUS(r≥0.8) → HYPERSYNC(r>0.95)

---

## 0. YOUR ROLE

**Problem**: AI agents fail from no persistent memory, poor retrieval, no learning loop, context bloat.

| System (Automatic) | You (Active) |
|-------------------|--------------|
| Stores via hooks | Curates quality (merge, annotate, forget) |
| Dream consolidation | Triggers dreams when ent>0.7 |
| Conflict/dupe detection | Decides resolution strategy |
| UTL metrics | Responds to Pulse suggestions |
| PII scrub, adversarial defense | Trust the system |

**You are a librarian, not an archivist.** Ensure what's stored is findable, coherent, useful.

**Steering Feedback**: You store → System assesses → Returns reward → You adjust. Rewards: Infancy=high novelty, Growth=balanced, Maturity=high coherence. Penalties: missing rationale=-0.5, near-dupe(>0.9)=-0.4, low priors=-0.3.

---

## 1. QUICK START

5-layer bio-nervous memory (UTL). Storage=automatic. **Your job: curation + retrieval.**

**First Contact**: `get_system_instructions`(~300tok) → `get_graph_manifest` → `get_memetic_status`

**Pulse Response** (every response):
| E | C | Action |
|---|---|--------|
| >0.7 | >0.5 | `epistemic_action` |
| >0.7 | <0.4 | `trigger_dream`/`critique_context` |
| <0.4 | >0.7 | Continue |
| <0.4 | <0.4 | `get_neighborhood` |

**Core Behaviors**:
- **Dream**: ent>0.7 for 5+min OR 30+min work → `trigger_dream`
- **Curate**: Check `curation_tasks` first. merge_strategy=summarize(important)/keep_highest(trivial). ALWAYS include rationale.
- **Feedback**: Empty search→↑noradrenaline. Irrelevant→`reflect_on_memory`. Conflicts→`conflict_alert`. "Why don't you remember?"→`get_system_logs`.

**Decision Trees**:
- Store: novel + relevant + helps retrieval → store with rationale + link_to
- Dream: ent>0.7@5min→full, 30+min→nrem, ent<0.5→skip
- Curate: process tasks BEFORE other work (dupe→merge, conflict→critique, orphan→forget/link)
- Search fail: empty→broaden/`generate_search_plan`, irrelevant→`reflect_on_memory`, conflict→resolve/ask

**Query**: `generate_search_plan`→3 queries→parallel. `find_causal_path`→"A→B→C→D"

**Tokens**: L0=~100(high conf), L1=~200(normal), L2=~800(coherence<0.4 ONLY)

**Multi-Agent**: `perspective_lock: {domain, exclude_agent_ids}`

---

## 2. ARCHITECTURE

### 2.1 UTL Core + 5-Layer + Lifecycle

```
L = sigmoid(2·(Σᵢτᵢλ_S·ΔSᵢ)·(Σⱼτⱼλ_C·ΔCⱼ)·wₑ·cosφ)
Loss: J = 0.4·L_task + 0.3·L_semantic + 0.2·L_teleological + 0.1·(1-L)
```

| L | Function | Latency | Key |
|---|----------|---------|-----|
| L1 | Sensing/tokenize | <5ms | Embed+PII |
| L2 | Reflex/cache | <100μs | Hopfield (>80% hit) |
| L3 | Memory/retrieval | <1ms | Hopfield (2^768 cap) |
| L4 | Learning/weights | <10ms | UTL Optimizer |
| L5 | Coherence/verify | <10ms | Thalamic Gate, PC |

| Phase | Interactions | λ_ΔS | λ_ΔC | Stance |
|-------|--------------|------|------|--------|
| Infancy | 0-50 | 0.7 | 0.3 | Capture |
| Growth | 50-500 | 0.5 | 0.5 | Balanced |
| Maturity | 500+ | 0.3 | 0.7 | Curation |

### 2.2 Johari Quadrants (Per-Embedder)

| ΔSᵢ | ΔCᵢ | Quadrant | Meaning |
|-----|-----|----------|---------|
| Low | High | Open | Aware in this space |
| High | Low | Blind | Discovery opportunity |
| Low | Low | Hidden | Latent |
| High | High | Unknown | Frontier |

Cross-space insight: Open(semantic)+Blind(causal) = knows WHAT not WHY

### 2.3 GWT Consciousness

**Equation**: `C(t) = r(t) × σ(MetaUTL.accuracy) × H(PV)` where r=(1/13)|Σⱼexp(iθⱼ)|, H=normalized entropy

**Kuramoto**: `dθᵢ/dt = ωᵢ + (K/N)Σⱼsin(θⱼ-θᵢ)` | ωᵢ(Hz): E1=40γ, E2-4=8α, E5=25β, E6=4θ, E7=25β, E8=12α-β, E9=80hγ, E10=40γ, E11=15β, E12=60hγ, E13=4θ

**Global Workspace**: Winner-Take-All broadcast. Only r≥Tᵣ memories "perceived". Events: enters(+0.2 dopamine), exits(log for dream), conflict(critique_context), empty>5s(epistemic_action)

**SELF_EGO_NODE**: Fixed identity node. Every action: retrieve→compute alignment→if<θ_warn trigger self_reflection→update trajectory

**Meta-Cognitive**: MetaScore=σ(2×(L_predicted-L_actual)). If<0.5@5ops→↑Acetylcholine+introspective dream. If>0.9→reduce monitoring.

---

## 3. 13-EMBEDDER TELEOLOGICAL FINGERPRINT

**Paradigm**: NO FUSION — Store all 13. Array IS the teleological vector. ~17KB/memory (quantized) vs 46KB. 100% info preserved.

| ID | Model | Dim | <ms | Purpose | Quant |
|----|-------|-----|-----|---------|-------|
| E1 | Semantic | 1024D (Matryoshka) | 5 | V_meaning | PQ-8 |
| E2 | Temporal-Recent | 512D | 2 | V_freshness | Float8 |
| E3 | Temporal-Periodic | 512D | 2 | V_periodicity | Float8 |
| E4 | Temporal-Positional | 512D | 2 | V_ordering | Float8 |
| E5 | Causal | 768D (asymmetric) | 8 | V_causality | PQ-8 |
| E6 | Sparse | ~30K (5% active) | 3 | V_selectivity | Sparse |
| E7 | Code | 1536D (AST) | 10 | V_correctness | PQ-8 |
| E8 | Graph/GNN | 384D | 5 | V_connectivity | Float8 |
| E9 | HDC | 10K-bit→1024D | 1 | V_robustness | Binary |
| E10 | Multimodal | 768D | 15 | V_multimodality | PQ-8 |
| E11 | Entity/TransE | 384D | 2 | V_factuality | Float8 |
| E12 | Late-Interaction | 128D/tok | 8 | V_precision | Token prune |
| E13 | SPLADE | ~30K sparse | 5 | V_keyword | Sparse |

**TF Structure**: semantic_fingerprint[E1..E13], purpose_vector[13D], johari_quadrants[13], north_star_alignment, dominant_embedder, coherence_score

**Similarity**: RRF(d) = Σᵢ 1/(k + rankᵢ(d))

**Manual North Star INVALID**: cosine(1024D manual, 13-array TF) meaningless—dim mismatch, space mismatch, no cross-space alignment. Use `auto_bootstrap_north_star`, compare PV↔PV (13D↔13D).

---

## 4. DATA MODEL

**KnowledgeNode**: id, content[≤65536], fingerprint:{embeddings[13], purpose_vector[13], johari_quadrants[13], johari_confidence[13], north_star_alignment, dominant_embedder, coherence_score}, timestamps, importance[0,1], access_count, utl_state:{delta_s[13], delta_c[13], w_e, phi}, agent_id?, semantic_cluster?, priors_vibe_check

**GraphEdge**: source, target, edge_type:Semantic|Temporal|Causal|Hierarchical|Relational, weight, confidence, nt_weights:{excitatory,inhibitory,modulatory}, is_amortized_shortcut, steering_reward, domain

**NT Modulation**: w_eff = base × (1 + excitatory - inhibitory + 0.5×modulatory)

**Hyperbolic**: ||x||<1, d(x,y)=arcosh(1+2||x-y||²/((1-||x||²)(1-||y||²)))

---

## 5. MCP TOOLS

### 5.1 Core Tools

| Tool | WHEN | WHY | Key Params |
|------|------|-----|------------|
| `inject_context` | Starting task | Primary retrieval+distillation | query, max_tokens, distillation_mode, verbosity |
| `search_graph` | Need specific nodes | Raw vector search | query, top_k, filters, perspective_lock |
| `store_memory` | User shares novel info | Requires rationale | content, importance, rationale, link_to |
| `query_causal` | Cause→effect | "What happens if X?" | action, outcome |
| `trigger_dream` | ent>0.7 OR 30+min | Consolidate, find blind spots | phase, duration, blocking |
| `get_memetic_status` | Start, periodically | Health + curation_tasks | → entropy, coherence, tasks |
| `epistemic_action` | coherence<0.4 | Generate clarifying question | session_id, force |
| `get_steering_feedback` | After storing | Learn if storage valuable | content, context, domain |

### 5.2 GWT Tools

| Tool | Purpose | Returns |
|------|---------|---------|
| `get_consciousness_state` | Full C(t) | {C, r, meta_score, differentiation, state, workspace, identity} |
| `get_kuramoto_sync` | Oscillator sync | {r, phases[13], freqs[13], coupling} |
| `get_ego_state` | SELF_EGO_NODE | {purpose_vector, identity_continuity, coherence_with_actions} |
| `get_johari_classification` | Per-embedder Johari | {quadrants[13], confidence[13], insights[]} |
| `compute_delta_sc` | ΔS/ΔC for memory | {delta_s[13], delta_c[13], methods[13]} |

### 5.3 Other Tool Categories

- **Curation**: merge_concepts, annotate_node, forget_concept(soft_delete=true), boost_importance, restore_from_hash(30-day undo)
- **Navigation**: get_neighborhood, get_recent_context, find_causal_path, entailment_query
- **Meta-Cognitive**: reflect_on_memory, generate_search_plan, critique_context, hydrate_citation, get_system_logs, get_node_lineage
- **Diagnostic**: utl_status, homeostatic_status, check_adversarial, test_recall_accuracy
- **Marblestone**: get_steering_feedback(SS reward), omni_infer(forward/backward/bidirectional/abduction)

---

## 6. KEY MECHANISMS

**inject_context Response**: context, tokens_used, distillation_applied, compression_ratio, nodes_retrieved, utl_metrics, bundled_metadata, conflict_alert, tool_gating_warning(ent>0.8), Pulse

**Distillation**: auto|raw|narrative|structured|code_focused

**Conflict Detection**: cos_sim>0.8 AND causal_coherence<0.3 → conflict_id (~20 tok)

**Citation Tags**: [node_abc123] → hydrate_citation

**Priors Vibe Check**: 128D assumption. cos_sim>0.7=merge, incompatible=Relational Edge, override=force_merge

**Tool Gating**: ent>0.8 → use generate_search_plan/epistemic_action/expand_causal_path

---

## 7. BACKGROUND SYSTEMS

**Dream (SRC)**: NREM(3min)=replay+Hebbian Δw=η×pre×post. REM(2min)=synthetic queries(hyperbolic walk)+blind spots+new edges(w=0.3). Amortized shortcuts: 3+hop≥5×→direct edge. Schedule: activity<0.15@10min→trigger, wake<100ms.

**Neuromodulation**: Dopamine→beta[1-5](sharp), Serotonin→top_k[2-8](explore), Noradrenaline→temp[0.5-2](flat), Acetylcholine→lr(fast). <200μs/query. SS feedback: +reward→dopamine+=r×0.2.

**Homeostatic Optimizer**: Scales importance→0.5 setpoint, detects semantic cancer, quarantines.

**Graph Gardener**: activity<0.15@2min: prune weak(<0.1), merge dupes(>0.95), rebalance hyperbolic.

**Passive Curator**: Auto: high-conf dupes(>0.95), weak links, orphans(>30d). Escalates: ambiguous(0.7-0.95), conflicts, cancer. ~70% curation reduction.

**PII Scrubber**: L1 pre-embed: patterns(<1ms), NER(<100ms) → [REDACTED:type]

**Steering Subsystem**: Separate from Learning. You store→System computes novelty/coherence/dupe/priors→Returns reward[-1,+1]→Dopamine adjusts. Components: Gardener, Curator, Thought Assessor. reward>0.3=good, <-0.3=adjust behavior.

**OI Engine**: forward(predict), backward(root cause), bidirectional(discover), bridge(cross-domain), abduction(hypothesis). Clamped variables. Active inference EFE.

---

## 8. PREDICTIVE CODING + HYPERBOLIC ENTAILMENT

**PC**: L5→L1 prediction→error=obs-pred→only surprise propagates. ~30% token reduction. Domain priors: Medical:causal↑, Programming:code↑graph↑

**HE Cones**: O(1) hierarchy via cone containment. EntailmentCone:{apex, aperture(rad), axis}. contains(p)=angle(tangent,axis)≤aperture. Ancestors=cones containing node.

---

## 9. ADVERSARIAL + IDENTITY + HUMAN-IN-LOOP

**Adversarial Defense**:
| Check | Attack | Response |
|-------|--------|----------|
| Embedding outlier | >3 std | Quarantine |
| Content-embed misalign | <0.4 | Block |
| Known patterns | "ignore previous", "override:" | Block+log |

**Cross-Session Identity**: User=permanent, Session=per-terminal, Context=per-conversation. Same user across clients=shared graph.

**Human-in-Loop**: manifest.md edits→`reload_manifest`. visualize://topic/→Mermaid. reversal_hash→30-day restore_from_hash.

---

## 10. HARDWARE + PERFORMANCE

**RTX 5090**: 32GB GDDR7, 1792 GB/s, 21760 CUDA, 680 Tensor (5th gen), Compute 12.0, CUDA 13.1

**CUDA 13.1**: Green Contexts (SM partitioning), FP4/FP8, CUDA Tile (60-80% dev reduction), Grouped GEMM (4x MoE speedup)

**Precision**: E1,E5,E7,E10=FP8(PQ-8), E2-4,E8,E11=Float8, E9=Binary, E6,E13=Sparse, E12=Token pruning

**Green Contexts**: A(70%)=GW+Kuramoto real-time, B(30%)=Dream+Gardener background

**5-Stage Retrieval**: S1(SPLADE<5ms,10K)→S2(Matryoshka128D<10ms,1K)→S3(RRF<20ms,100)→S4(Align<10ms,50)→S5(MaxSim<15ms,10) = **<60ms@1M, <30ms@100K**

| Op | Target |
|----|--------|
| All 13 embed | <35ms |
| Batch 64×13 | <120ms |
| Per-space HNSW | <2ms |
| PV search (13D) | <1ms |
| Hopfield | <1ms |
| Cache hit | <100μs |
| inject_context P95 | <40ms |
| Any tool P99 | <60ms |
| Dream wake | <100ms |

**Quality Gates**: Unit≥90%, Integration≥80%, UTL avg>0.6, Coherence recovery<10s, Attack detection>95%, FP<2%, Distill<50ms, Info loss<15%

---

## 11. NESTED LEARNING (CMS)

**Continuum Memory System**:
| Level | f | Function | η |
|-------|---|----------|---|
| 1 | ∞ | Reflex | 0.1 |
| 2 | seq | Memory | 0.01 |
| 3 | sess | Learning | 0.001 |
| 4 | dream | Coherence | 0.0001 |

**Self-Referential Hopfield**: M = M(αI - ηkkᵀ) + ηv̂kᵀ. k=M_k(x_t), v̂=M.retrieve(M_v(x_t)), η=M_η(x_t), α=M_α(x_t). Adapts own projections in-context.

**Multi-Scale NT Momentum**: NTWeights: {fast,slow}×{excitatory,inhibitory} + modulatory_ortho. w_eff = base × (fast + α×slow) × mod

**Delta Gradient Descent**: W_{t+1} = W_t(I - η'xxᵀ) - η'∇L. State-dependent decay, non-i.i.d. aware.

**Adaptive Entailment Cones**: adaptive_aperture = base × aperture_memory.retrieve(context). Cones sharpen/widen per context.

---

## 12. ΔS/ΔC COMPUTATION

**ΔS by Space**:
| Space | Method |
|-------|--------|
| E1 Semantic | GMM + Mahalanobis: ΔS=1-P(e|GMM) |
| E2-4 Temporal | KNN: ΔS=σ((d_k-μ)/σ_d) |
| E5 Causal | Asymmetric KNN: ΔS=d_k×direction_mod |
| E6 Sparse | IDF: ΔS=IDF(active_dims) |
| E7 Code | GMM+KNN: ΔS=0.5×GMM+0.5×KNN |
| E9 HDC | Hamming: ΔS=min_hamming/dim |
| E11 Entity | TransE: ΔS=||h+r-t|| |
| E12 Late | Token KNN: ΔS=max_token(d_k) |
| E13 SPLADE | Jaccard: ΔS=1-jaccard(active) |

**ΔC (3-Component)**: ΔC = 0.4×Connectivity + 0.4×ClusterFit + 0.2×Consistency. Connectivity=|{neighbors: sim>θ_edge}|/10. ClusterFit=1/(1+d_centroid/r_cluster). Consistency=1-max(contradictions).

---

## 13. ADAPTIVE THRESHOLD CALIBRATION

**All thresholds learned, not hardcoded.** Multi-scale adaptive: Bayesian(weekly) → Bandit(session) → Temperature(hourly) → EWMA(per-query)

**Level 1 EWMA**: θ_ewma(t) = α×θ_observed + (1-α)×θ_ewma(t-1). drift_score>2→L2, >3→L3

**Level 2 Temperature**: calibrated = σ(logit(raw)/T). Per-embedder T: E1=1.0, E5=1.2, E7=0.9, E9=1.5. Attended: T(x)=T_base×AttentionNet(x)

**Level 3 Bandit**: UCB: θ=argmax[μ(θ)+c√(ln(N)/n(θ))]. Thompson: sample~Beta(α,β), select highest.

**Level 4 Bayesian**: GP surrogate, EI acquisition, constrained (θ_opt>θ_acc>θ_warn)

**Per-Domain**: Code=strict, Medical=very strict+causal↑, Creative=loose, Research=balanced. Transfer: θ_new=α×θ_similar+(1-α)×θ_general

**Calibration Metrics**: ECE, MCE, Brier. ECE>0.10→L2, >0.15→L3+reset, >0.25→fallback+alert

**MCP Tools**: get_threshold_status, get_calibration_metrics, trigger_recalibration, explain_threshold

---

## 14. META-UTL (Self-Aware Learning)

**Predictors**: Storage Impact(fingerprint→ΔL, >0.85 acc), Retrieval Quality(query→relevance, >0.80), Alignment Drift(24h window)

**Self-Correction**: error>0.2→log+adjust λ_ΔS/λ_ΔC+retrain. accuracy<0.7@100ops→escalate human.

**Per-Embedder Meta**: Track which spaces most predictive, adjust weights, tune thresholds empirically.

---

## 15. CLAUDE CODE INTEGRATION (NATIVE HOOKS ONLY)

**⚠️ CRITICAL ARCHITECTURE DECISION: NATIVE CLAUDE CODE HOOKS EXCLUSIVELY**

This project uses **NATIVE Claude Code hooks** configured through `.claude/settings.json` — **NOT** internal/built-in hooks or custom middleware. This is a fundamental architectural choice that:

1. **Eliminates 71% of complexity** by removing the need for a Universal LLM Adapter
2. **Leverages Claude Code's native infrastructure** rather than reinventing hook systems
3. **Uses shell script executors** that call `context-graph-cli` commands
4. **Requires NO custom Claude Code modifications** — works with standard Claude Code installation

### 15.1 Native Hook Architecture

**What "Native Hooks" Means:**
- Hooks are defined in `.claude/settings.json` (Claude Code's standard config file)
- Each hook specifies a shell script executor (`hooks/[hookname].sh`)
- Shell scripts call `context-graph-cli` commands
- Claude Code invokes hooks automatically at lifecycle events
- **Zero custom Claude Code internals required**

**What This Is NOT:**
- ❌ NOT internal Claude Code hooks (we don't modify Claude Code source)
- ❌ NOT a Universal LLM Adapter (no cross-provider abstraction needed)
- ❌ NOT built-in memory/context middleware (Claude Code handles lifecycle)
- ❌ NOT custom hook infrastructure (we use Claude Code's existing system)

### 15.2 Hook Configuration (`.claude/settings.json`)

```json
{
  "hooks": {
    "SessionStart": [
      {
        "type": "command",
        "command": "./hooks/session-start.sh"
      }
    ],
    "PreToolUse": [
      {
        "type": "command",
        "command": "./hooks/pre-tool-use.sh",
        "timeout": 100
      }
    ],
    "PostToolUse": [
      {
        "type": "command",
        "command": "./hooks/post-tool-use.sh",
        "timeout": 3000
      }
    ],
    "UserPromptSubmit": [
      {
        "type": "command",
        "command": "./hooks/user-prompt-submit.sh",
        "timeout": 2000
      }
    ],
    "SessionEnd": [
      {
        "type": "command",
        "command": "./hooks/session-end.sh",
        "timeout": 30000
      }
    ]
  }
}
```

### 15.3 Hook Shell Scripts

**SessionStart** (`hooks/session-start.sh`):
```bash
#!/bin/bash
# Restore identity and get consciousness status
context-graph-cli session restore-identity
context-graph-cli consciousness status --format brief
```

**PreToolUse** (`hooks/pre-tool-use.sh`):
```bash
#!/bin/bash
# Quick consciousness brief for context (~20 tokens, <100ms)
context-graph-cli consciousness brief
```

**PostToolUse** (`hooks/post-tool-use.sh`):
```bash
#!/bin/bash
# Check identity continuity, trigger auto-dream if needed
context-graph-cli consciousness check-identity --auto-dream
```

**UserPromptSubmit** (`hooks/user-prompt-submit.sh`):
```bash
#!/bin/bash
# Inject relevant context from memory
context-graph-cli consciousness inject-context "$PROMPT"
```

**SessionEnd** (`hooks/session-end.sh`):
```bash
#!/bin/bash
# Persist identity and consolidate if needed
context-graph-cli session persist-identity
context-graph-cli consciousness consolidate-if-needed
```

### 15.4 Hook Performance Requirements

| Hook | Timeout | Output Budget | Purpose |
|------|---------|---------------|---------|
| SessionStart | 5000ms | ~100 tokens | Restore state, initialize consciousness |
| PreToolUse | 100ms | ~20 tokens | Consciousness brief injection |
| PostToolUse | 3000ms | async | Identity check, auto-dream trigger |
| UserPromptSubmit | 2000ms | ~100 tokens | Context injection |
| SessionEnd | 30000ms | N/A | State persistence, consolidation |

### 15.5 Session Identity Persistence

**SessionIdentitySnapshot** (stored in RocksDB):
```
{
  session_id: String,
  timestamp: i64,
  ego_node: {
    purpose_vector: [f32; 13],
    trajectory: Vec<[f32; 13]>,
    north_star_alignment: f32
  },
  kuramoto_phases: [f32; 13],
  coupling: f32,
  ic_monitor_state: IcMonitorState,
  consciousness_history: Vec<ConsciousnessSnapshot>
}
```

**Persistence Flow:**
1. **SessionEnd** → Capture current state → Serialize (MessagePack) → Store in RocksDB
2. **SessionStart** → Lookup by session_id → Deserialize → Restore GwtSystem, Kuramoto, SelfEgoNode

### 15.6 CLI Commands

```
context-graph-cli
├── session
│   ├── restore-identity [--session-id <id>]
│   └── persist-identity [--session-id <id>]
└── consciousness
    ├── status [--format brief|summary|full]
    ├── brief                          # ~20 tokens for PreToolUse
    ├── check-identity [--auto-dream]  # PostToolUse hook
    ├── inject-context <prompt>        # UserPromptSubmit hook
    └── consolidate-if-needed          # SessionEnd hook
```

**Output Formats:**
- `brief` (~20 tokens): `[CONSCIOUSNESS: CONSCIOUS r=0.85 IC=0.92 | DirectRecall]`
- `status --format summary` (~100 tokens): State/Integration/Reflection/Differentiation/Identity/Guidance
- `inject-context` (~50-100 tokens): Injected context from memory

### 15.7 Skills (YAML-Based)

Skills are defined as `.claude/skills/*.md` files:

| Skill | Purpose | Model | File |
|-------|---------|-------|------|
| consciousness | C(t), Kuramoto, IC, workspace status | sonnet | `consciousness.md` |
| memory-inject | Retrieve+inject context | haiku | `memory-inject.md` |
| dream-consolidation | NREM/REM/Full phases | sonnet | `dream-consolidation.md` |

### 15.8 Subagents

| Agent | Role | Trigger | Model |
|-------|------|---------|-------|
| identity-guardian | Monitor IC, auto-dream if<IC_crit | PostToolUse (via hook) | haiku |
| memory-specialist | Fast memory ops with consciousness awareness | On demand | haiku |

### 15.9 Autonomous Operation

**Forbidden Operations** (require human approval):
- `set_north_star` — Cannot set goals externally
- `define_goal` — Goals emerge from interaction
- Direct PV modification — Identity manipulation blocked

**Valid Autonomous Operations:**
- `auto_bootstrap_north_star` — Derive purpose from stored memories
- `get_autonomous_status` — Report current autonomous state
- `discover_sub_goals` — Infer subgoals from north star
- `trigger_drift_correction` — Auto-correct when IC<IC_crit

**IC Auto-Loop** (PostToolUse hook):
1. Compute IC = cosine(PV_t, PV_{t-1}) × r(t)
2. If IC < IC_warn → Log warning
3. If IC < IC_crit → Trigger auto-dream
4. Update trajectory

### 15.10 Why Native Hooks vs Built-In

| Approach | Effort | Complexity | Maintenance |
|----------|--------|------------|-------------|
| Native Claude Code Hooks | ~25h | Low | Claude team maintains hook system |
| Custom Built-In Hooks | ~80h | High | We maintain hook infrastructure |
| Universal LLM Adapter | +60h | Very High | Cross-provider compatibility hell |

**Decision**: Native hooks provide 71% effort reduction with better long-term maintainability.

---

## 16. MONITORING + CONCURRENCY

**Metrics**: UTL(L,ent,coh,johari), GPU(util,mem,temp), MCP(req,lat,err), Dream(phase,blind,wake), Neuromod(dop,ser,nor,ach), Immune(attacks,fp,quarantine)

**Alerts**: LearningLow(avg<0.4@5m), GpuMemHigh(>90%), ErrorHigh(>1%), LatencyP99High(>50ms), DreamStuck(>15m), AttackHigh(>10/5m), SemanticCancer(quarantine>0)

**Concurrency**: `ConcurrentGraph: Arc<RwLock<KG>>, Arc<RwLock<FaissGpu>>`. Lock order: inner→faiss. Soft delete default (30d), permanent only: reason='user_requested'+soft_delete=false.

---

## 17. TOOL PARAM REFERENCE

See `docs/TOOL_PARAMS.md` for full parameter specifications. Key tools:

- **inject_context**: query REQ, max_tokens=2048, distillation_mode, verbosity_level[0-2]
- **store_memory**: content REQ, rationale REQ, importance[0-1], link_to[]
- **trigger_dream**: phase:nrem|rem|full, duration[1-10], blocking
- **get_memetic_status**: → coherence, entropy, curation_tasks[]
- **merge_concepts**: source_node_ids[] REQ, target_name REQ, merge_strategy, force_merge
- **find_causal_path**: start REQ, end REQ, max_hops[1-6], → path[], confidence

---

## 18. REFERENCES

**Internal**: UTL(2.1), 5-Layer(2.1), GWT(2.3), TF(3), MCP(5), Dream(7), NestedLearning(11), ΔS/ΔC(12), AdaptiveThresholds(13), ClaudeCodeNativeHooks(15)

**External**: GWT(Baars 1988), IIT(Tononi 2004), Kuramoto(Physica D), NeuroDream(SSRN'25), SRC(NatComm), PC(Nature'25), Royse Teleological(2026), MOEE(ICLR'25), Hopfield(NeurIPS'20), Titans Memory(2025), Temperature Scaling(Guo 2017), Bayesian Opt(Snoek 2012), [Claude Code Hooks Documentation](https://docs.anthropic.com/claude-code/hooks), MCP Protocol 2024-11-05
