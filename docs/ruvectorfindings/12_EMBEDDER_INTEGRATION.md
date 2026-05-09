# 12 — Embedder Integration into ME-JEPA (TCT/DDA + UTML substrate)

> **Operator directive 2026-05-09:** *"since we are no longer using the local model that frees up all of our gpu vram. this system has a number of multi embedder systems and like 30 embedders hooked up. we could load any of them or improve the learning embedders with the new utl theory and somehow integrate all of that into this system."*

> **Companion docs:**
> - `09_END_GOAL_REVIEW_REPLACEMENT.md` — master plan (instruments, panel, predictor, frozen target readback, VICReg, oracle head)
> - `10_MEJEPA_PIVOT_2026_05_09.md` — post-pivot single-outer-loop architecture
> - `11_MEJEPA_LEARNING_FORMULA.md` — UTML v1.0 four-factor formula
> - `13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md` — TCT three-phase protocol + universal-agent verification (Royse 2026c)
> - `14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md` — AST-perfect atomic chunk routing for each embedder

---

## 0. Headline

**The 21-embedder ensemble + the LoRA / distillation / multitask training infrastructure in `crates/context-graph-embeddings/` is ~70% of what doc 09 specifies as ME-JEPA-Code's Phase 1-5 implementation.** With Qwen3.6-27B retired (-22 GB VRAM, -1.5 GB Hermes), there is **~28-29 GB free on the RTX 5090** — enough to host every weight in the ensemble simultaneously, plus the predictor + activations + LoRA adapters.

Two consequences:

1. **Doc 09's planned 14 ME-JEPA instruments do not need to be built from scratch.** The existing 21 embedders cover the same conceptual space (semantic, code, graph, sparse retrieval, causal, multilingual, multimodal, learner-state). Phase 1 work shifts from "build 14 instruments" to "wire the 21 existing embedders into the `Instrument` trait + add 3 net-new code-domain heads (`E_AST`, `E_CFG`, `E_DataFlow`)."

2. **Royse 2026c's DDA counting identity gives the supervisory signal budget.** At N=21 frozen embedders, every input yields `N + (N choose 2) = 21 + 210 = 231` structured supervisory signals (panel-level), and ~280 chunk-level signals when AST-routing is wired (doc 14) — for a combined **~510 labeled signals per Phase 0 mutation entry, scaling to ~1.22 M signals over the full 2,400-entry corpus**. This is the operator's "capture all the reward signals as possible" — quantified.

The shipped LoRA + distillation infrastructure is *exactly* what UTML v1.0 §V.1 (effective_plasticity / continual backprop / EWC++) requires. The embedder fine-tuning loop is the ME-JEPA fine-tuning loop, with the same training-cert + witness-chain provenance.

---

## 1. The 21-embedder ensemble inventory

`crates/context-graph-embeddings/` ships **21 embedders** — 14 production content embedders (E1-E14) + 7 learner-state embedders (E15-E21). Total disk ~19 GB. Typical VRAM footprint when all loaded as bf16: **~14-18 GB** (much smaller than disk because most disk weight is fp32).

### 1.1 Production content embedders (E1-E14, ~12.5 GB VRAM)

| Slot | Name | Source model | Native dim | Trainable today | TCT role / panel placement |
|---|---|---|---:|---|---|
| **E1** | Semantic | `intfloat/e5-large-v2` | 1024 | inference | Labeller for `E_Test` / `E_Problem` / `E_CommitMsg`; runtime guard on natural-language alignment |
| **E2** | TemporalRecent | exp-decay basis | 512 | yes (custom) | Labeller for `E_Trace` recency features |
| **E3** | TemporalPeriodic | Fourier basis | 512 | yes (custom) | Labeller for build/test cycle phase |
| **E4** | TemporalPositional | sinusoidal basis | 512 | yes (custom) | Labeller for within-attempt position |
| **E5** | Causal | `nomic-ai/nomic-embed-text-v1.5` | 768 | **yes (LoRA stages 1-3)** | Labeller for `E_Diff` cause→effect; runtime guard on hunk-attribution |
| **E6** | Sparse | `naver/splade-cocondenser` | 30K→1536 | inference | Sparse retrieval, anti-collapse signal, keyword salience |
| **E7** | Code | **`Qodo/Qodo-Embed-1-1.5B`** | **1536** | inference | **THE primary code instrument** — `E_AST` / `E_CFG` / `E_DataFlow` / `E_Diff` source |
| **E8** | Graph | `intfloat/e5-large-v2` (graph use) | 1024 | inference | `E_TypeGraph` text + symbol relationships |
| **E9** | HDC | hyperdimensional 10K-bit | 10K→1024 | yes (custom) | Typo-resistant signature; ΔΞ adversarial-perturbation signal |
| **E10** | Contextual | `intfloat/e5-base-v2` | 768 | inference | `E_StaticAnalysis` warning clusters; paraphrase |
| **E11** | Kepler | RoBERTa-base + TransE on Wikidata5M | 768 | **yes** | **`E_TypeGraph` candidate** (knowledge-graph embedder) — entity / dependency-graph signal |
| **E12** | LateInteraction | `colbert-ir/colbertv2.0` | 128/token | inference | Token-level fine-grained re-rank; per-line edits |
| **E13** | Splade v3 | sparse v3 head | 30K | inference | Stage-1 recall in 5-stage pipeline |
| **E14** | BgeM3Dense | **`BAAI/bge-m3` (XLM-RoBERTa-Large)** | 1024 | **yes** | **Multilingual + 8192-token context** — direct candidate for cross-language SWE-bench Lite |

**Trainable subset:** E5 + E11 + E14 plus the four custom (E2/E3/E4/E9) embedders all have shipped fine-tune pipelines under `crates/context-graph-embeddings/src/training/`.

### 1.2 Learner-state embedders (E15-E21, ~3-5 GB VRAM, on-demand)

These were originally for human-physiology UTL v2.0. Under UTML v1.0 they do NOT feed ME-JEPA training (machines have no autonomic nervous system, UTML v1.0 §0). Repurposed for **operator-state-aware review queueing** + **agent-CoT affect probing**:

| Slot | Name | Backbone | Dim | UTML v1.0 repurposing |
|---|---|---|---:|---|
| E15 | AffectSpeech | Wav2Vec2-large MSP-DIM | 1024 | Operator review-call quality (when audio available) |
| E16 | AffectFace | OpenFace 3.0 + MTL | 512 | Operator review-fatigue (when video available) |
| E17 | **AffectText** | MiniLM + EmoBank | 384 | **Agent-CoT affect probe** (no sensors needed — runs on Claude Code conversation transcripts) → ΔΩ_outer term |
| E18 | PPG | PaPaGei foundation | 512 | Operator HRV → review-queue scheduling |
| E19 | EDA | WESAD chest physiology | 64 | Operator stress signal |
| E20 | EEG | LaBraM foundation | 768 | Operator focus → schedule review for high-ΔE windows |
| E21 | EegArtifactRobust | EEGPT | 768 | Cross-check E20 |

**Source code:** `crates/context-graph-embeddings/src/types/model_id/core.rs` (E1-E14) + `src/learner/mod.rs` (E15-E21).

---

## 2. DDA signal counting (Royse 2026c §3.1)

For a corpus of `n` raw inputs projected through `N` frozen, approximately-independent embedders, the structured-signal count is:

```
|D'| = n · (N + (N choose 2)) = n · (N + N(N-1)/2)
```

| Stack | N | per-input signals (tight) | Per /loop turn (50-200 inputs) | Per day (96 turns) |
|---|---:|---:|---:|---:|
| Production E1-E14 | 14 | **105** (14 + 91) | 5,250-21,000 | 504K-2M |
| Full E1-E21 | 21 | **231** (21 + 210) | 11,550-46,200 | 1.1M-4.4M |
| Loose upper bound at N=21 | 21 | 462 (21 + 441) | 23K-92K | 2.2M-8.8M |

**Tightness caveat:** Royse 2026c flags this as a *constructive upper bound* whose tightness depends on a pairwise mutual-information audit (`mejepa_audit_pairwise_mi`). UTML v1.0 §VI.3 wires this audit into ME-JEPA's `ΔΞ.predictor_redundancy` term — redundant pairs are detected and down-weighted.

**Per Phase 0 corpus**: 2,400 mutation entries × 231 panel signals = **554,400 structured signals**. With AST-aware chunk routing (~40 chunks × ~7 signals = 280 chunk-level signals per entry, doc 14 §1.3): combined **~1.22 M signals per corpus**, every one labeled by the entry's known (mutation_category, oracle_verdict, entity_type, language) tuple. **No human annotation, no synthetic generator, no recursion.**

---

## 3. Integration map: 21 embedders → 14 ME-JEPA instruments

Doc 09 §2.2 specifies 14 instruments for the 5,120-d panel (per §3.5.5). Mapping the shipped ensemble onto those slots:

> **AST-aware routing precondition** (per doc 14): every code-domain instrument operates on **AST-perfect atomic chunks** produced by `crates/context-graph-core/src/memory/ast_chunker.rs` (Rust today, extending to all 11 languages as Phase 1 deliverable). Each chunk carries `(language, entity_type, parent_chain, line_start, line_end, sha256)` metadata; per-chunk-type embedder routing (doc 14 §4) decides which embedder slots receive which chunks.

| Doc 09 instrument | Dim | Existing embedder(s) | Effort |
|---|---:|---|---|
| **E_AST** | 384 | **E7 Code (Qodo, 1536D)** + linear projection 1536→384 | ~2 days |
| **E_CFG** | 256 | **E8 Graph** + tree-sitter CFG extraction with graph-attention head | ~3 days |
| **E_DataFlow** | 256 | **E11 Kepler** + def-use edge extraction with graph-attention head | ~3 days |
| **E_TypeGraph** | 256 | **E11 Kepler** (perfect fit) + Pyright/mypy lattice | ~3 days |
| **E_Test** | 384 | **E1 Semantic** on test-fn natural-language descriptions | ~1 day |
| **E_Trace** | 512 | **E2/E3/E4** + sequence-transformer over `(fn_id, line_no, val_hash)` events | ~3 days |
| **E_Diff** | 256 | **E7 Code** with diff-edge encoder | ~2 days |
| **E_Witness** | 256 | **E10 Contextual** on 73-byte SHAKE-256 chain entries (sequence transformer) | ~1.5 days |
| **E_Oracle** | 128 | ✅ shipped (`crates/context-graph-mejepa-instruments/src/e_oracle.rs`) | done |
| **E_Problem** | 1024 | **E1 Semantic** on SWE-bench problem text | ~1 day |
| **E_CommitMsg** | 384 | **E1 Semantic** on commit text | ~1 day |
| **E_StaticAnalysis** | 256 | per `plan_e_static_analysis.md` — pure-Rust scalar aggregates | ~2 days |
| **E_Runtime** | 256 | aggregates: walltime + RSS + coverage Δ + network signals | ~2 days |
| **E_Reasoning** | 384 | **E17 AffectText** on outer-loop agent CoT (Claude Code transcripts) — affect-aware embedding | ~2 days |

**Net effect:** 8 of 14 instruments become **thin heads on existing embedders** (1-3 days each). 3 are pure-Rust scalar aggregates already planned. 3 are net-new (E_AST/E_CFG/E_DataFlow tree-sitter integrations). **Total Phase 1 effort with embedder reuse: ~25 days vs ~30+ days from scratch.**

**Cross-language win (doc 10 Section C):** swapping E1 → **E14 BgeM3Dense** wherever practical buys multilingual coverage (Python, Rust, C, C++, Java, C#, JS, TS, Ruby) at no additional training cost. BGE-M3's 8192-token context also accommodates much larger code chunks than E5's 512-token limit.

---

## 4. Post-Qwen GPU VRAM budget (RTX 5090, 32 GB)

Pre-pivot: Qwen3.6-27B Q6_K_XL ~22 GB + Hermes ~1.5 GB + KV-cache ~3 GB + headroom ~5 GB.

Post-pivot (proposed steady state):

| Component | VRAM | Justification |
|---|---:|---|
| Embedder ensemble (E1-E14, bf16, hard-warm) | ~12.5 GB | core panel-materialization pipeline |
| Learner-state embedders (E15-E21, on-demand) | ~3-5 GB | loaded only during human review cycles |
| ME-JEPA predictor (6-layer transformer, hidden 1024, 8 heads, bf16) | ~3 GB | Phase 2 |
| Frozen target-readback buffers | <1 GB | persisted instrument outputs, not a second trainable model |
| Activation memory (batch 64) | ~5 GB | Phase 3 forward+backward |
| Optimizer state (AdamW) | ~2 GB | per-param momentum + variance |
| LoRA adapters (rank 32, all 21 embedders) | ~0.5 GB | Phase 5 self-healing |
| KV-cache + scratch | ~1 GB | inference path |
| **Total** | **~28-29 GB** | fits with ~3-4 GB driver/system overhead |

**Headroom for batch-128 training, gradient checkpointing, or larger predictor (12-layer):** available by lazy-loading infrequently-used learner-state embedders. **Net: the entire RTX 5090 is now usable** for embedder + ME-JEPA + headroom — previously Qwen consumed nearly all VRAM, leaving embedders cold-loading per request (~10× slower) and ME-JEPA training unable to fit.

---

## 5. Shipped training infrastructure (~3,600 LOC, already in tree)

`crates/context-graph-embeddings/src/training/`:

| Module | Purpose | UTML / ME-JEPA role |
|---|---|---|
| `data.rs` | Training pair structures + DataLoader | corpus ingestion |
| `loss.rs` | InfoNCE + directional + separation + soft-label losses | predictor-side ΔP loss |
| `optimizer.rs` | AdamW with warmup + cosine decay | Phase 3 optimizer (already specified in doc 09) |
| `trainer.rs` | Training loop with momentum/teacher utilities | reusable optimizer scaffolding |
| `evaluation.rs` | Directional accuracy, MRR, AUC | Phase 4 evaluation harness |
| `distillation.rs` | Online embedder teaching with EMA quality gates | optional consolidation utility |
| `lora.rs` | LoRA adapters (rank, alpha, dropout) on Q/K/V | UTML §V.1 effective_plasticity intervention via low-rank updates |
| `multitask.rs` | DirectionLabel + MechanismLabel auxiliary heads | doc 09 §3.7 multi-objective auxiliary heads |
| `pipeline.rs` | End-to-end training pipeline | Phase 3 training loop |

`crates/context-graph-embeddings/src/learner/`: E15-E21 slots, `LearnerEmbedderInput` enum, `LearnerEmbeddingOutput`, `CalibrationDatasetSpec`.

`crates/context-graph-embeddings/src/gpu/`: `device` + `memory` + `model_loader` + `ops` + `tensor` — full candle-CUDA stack already wired to the RTX 5090. **This is the same stack ME-JEPA's predictor will sit on (Phase 2).**

---

## 6. UTML v1.0 integration — embedder ensemble as ΔK / ΔΞ / ΔΩ instrumentation

UTML v1.0's `L_step = ΔP × ΔK × ΔΩ × ΔΞ` is meant to be **measurable in real time per training step**. The embedder ensemble provides the measurement substrate.

### 6.1 ΔK — knowledge coherence via cross-embedder agreement

UTML §IV.1's `cos_align(g_t, ḡ_prior)` measures gradient agreement with prior knowledge. The ensemble adds a **complementary** signal: **cross-embedder neighbor agreement**.

For any candidate panel, run all 14 content embedders; check if their nearest-neighbor sets in the corpus agree. Strong agreement = high coherence. Disagreement = embedder-specific edge case (could be OOD, could be unusual code change).

```
embedder_coherence(panel) = mean(jaccard(NN_i(panel), NN_j(panel))
                                 for all (i, j) in content embedder pairs)

ΔK_t = clamp_[0,1](
  0.5 · cos_align(g_t, ḡ_prior)
  + 0.3 · (1 − fisher_violation)
  + 0.15 · (1 − ECE)
  + 0.05 · embedder_coherence(panel_t)
)
```

Doc 13 §6 sharpens this further with the full 5-term DDA-aware ΔK that adds constellation-centroid proximity + cross-embedder MI consistency.

### 6.2 ΔΞ — predictor anti-collapse via embedder diversity

UTML §VI.4's `predictor_redundancy_t` measures pairwise mutual-information across panel slots. The ensemble provides a **structural** anti-collapse defense beyond MI auditing:

If the predictor's output `g_φ(panel_t01)` collapses to a constant or low-rank, **all 14 content embedders' similarity scores between predicted and target panels degrade in lockstep**. Cross-embedder *spread* on predicted-vs-target similarity is a powerful collapse-detector:

```
predicted_similarity_per_embedder = [cos(predicted, target) under E_i for i in 1..14]
predictor_collapse_score = 1 - std(predicted_similarity_per_embedder)
```

Healthy predictor → diverse spreads. Collapsed predictor → uniform agreement → high collapse score. This extends doc 11's "negative-action ablation falsifier" across the ensemble.

### 6.3 ΔΩ — optimizer state via E15-E21 learner-state embedders

UTML §V's `effective_plasticity / landscape_health / stability_floor` measures the *model's* state. E15-E21 measure the *agent's* state (Claude Code producing the training data).

Even without sensors, **E17 AffectText running on Claude Code's conversation transcripts** provides a measurable outer-loop-state signal:

```
agent_state_score = E17(conversation_transcript_window).valence
                  · E17(conversation_transcript_window).arousal_inverted_U

ΔΩ_outer = (model_plasticity)^0.4
         · (model_landscape_health)^0.4
         · (model_stability_floor)^0.1
         · (agent_state_score)^0.1
```

This **closes the loop** between UTL v2.0's biological framework (operator's original citation) and UTML v1.0's ML-native rewrite: the biology was always real; in this system, it's measured via the learner-state embedders watching the outer-loop agent.

---

## 7. Trainable embedder fine-tuning under L_step weighting

Three of the 14 content embedders have shipped trainable LoRA pipelines:

| Embedder | Crate / module | Current loss | Improvement under L_step |
|---|---|---|---|
| E5 Causal | `crates/context-graph-embeddings/src/training/` (3-stage LoRA) | per-batch CE on cause-effect pairs | weight each example by `L_step = ΔP × ΔK × ΔΩ × ΔΞ` |
| E11 Kepler | `src/models/pretrained/kepler/` | KEPLER KG-completion | same |
| E14 BGE-M3 | training pipeline (extends to per-language fine-tunes) | BGE-M3 multilingual contrastive | same; especially valuable for cross-language ΔK |

**Practical effect of L_step weighting:**
- Examples the embedder already nails (ΔP ≈ 0) → dropped.
- Examples that destroy prior alignment (ΔK ≈ 0) → dropped (protects pretrained knowledge).
- Examples seen during gradient explosion (ΔΩ ≈ 0) → dropped.
- Examples in the embedder's degenerate region (ΔΞ ≈ 0) → dropped.

Strictly safer than uniform-weight SGD; same discipline ME-JEPA's predictor uses.

---

## 8. Self-healing via shipped LoRA + distillation

UTML §V.1 prescribes **continual backprop** (Sutton 2024) when `effective_plasticity` collapses. The shipped `lora.rs` is the more elegant alternative: instead of re-initializing dormant units, fine-tune via LoRA adapters on a fresh slice of the reality stream:

```
if ΔΩ.effective_plasticity < 0.4:
    # Frozen-base embedders never lose plasticity — they're frozen.
    # Only the LoRA adapters need refreshing.
    refresh_lora_adapter(embedder=E_i, fresh_corpus_slice, rank=32, epochs=10)
```

UTML §VII (consolidation): the shipped `distillation.rs` is the operational form of M(t):

```
distillation_loop {
    pull_recent_observations()                    # from §3.8 shift-log subscriber
    for embedder in active_embedders:
        teacher_score = oracle.score(panel)        # the new ground truth
        student_score = embedder.score(panel)
        loss = info_nce_loss(student, teacher)
        student.backward(loss)
        ema_update(student.weights, τ=0.996)       # the "future-self prior" of UTML §VIII
}
```

Distillation already supports `quality_gate: true` — examples below confidence threshold are skipped. **This is UTML's `signal_clarity` term baked in.**

---

## 9. The `mejepa-shift-subscriber` (Phase 7 §3.8) consumes the embedder pipeline

Doc 09 §3.8 specifies a Tokio task that tails the shift log + harness artifacts and feeds `mejepa.observe()`. With the ensemble loaded:

```
for shift in cgreality_shift_log_tail() {
    panel_t1 = build_panel_via_embedder_ensemble(shift.after_sha)
                    # parallel inference: E1, E5, E7, E8, E10, E11, E14 on post-edit code
                    # parallel inference: E2-E4 on temporal context
                    # E_Witness on the witness-chain entry
                    # E_Diff on the (before, after) pair
    if shift.has_oracle_outcome() {
        panel_t2 = build_panel_t2(shift.after_sha, shift.oracle_outcome)
        L_step = compute_utml_l_step(panel_t1, panel_t2, ...)
        if L_step > 0.05:                             # passes the §11.2 sampling-weight gate
            mejepa.observe(panel_t1, panel_t2)        # online update
        log_per_step_cert(L_step, ΔP, ΔK, ΔΩ, ΔΞ)
    }
    mejepa.predict_latest(panel_t1)                   # always emit a live prediction
}
```

**Latency budget per shift** (RTX 5090, batch 1):
- Parallel embedder inference (14 embedders): ~50ms (each <5ms)
- Panel materialization: <10ms
- ME-JEPA forward: <10ms
- VICReg + L_step computation: <5ms
- RocksDB persistence: <5ms
- **Total: ~80ms** — well under the §3.8.6 budget of 5s p50 / 30s p99.

---

## 10. New MCP tools to ship

Per Royse 2026c §4 (Context Graph instantiates DDA at 75 MCP tools, 91 signals/input at N=13), our N=14/21 implementation should expose:

| Tool | Phase | Purpose |
|---|---|---|
| `mejepa_compute_panel(input)` | Phase 1 | Run all 14 embedders; return panel of 14 latent vectors. SHA-cached. |
| `mejepa_compute_signal_set(input)` | Phase 1 | Compute 105 (or 231) DDA signals for one input. |
| `mejepa_constellation_get(state, action, language)` | Phase 4 | Read frozen centroid for `(mutation_category, language)`. |
| `mejepa_constellation_distance(panel, ...)` | Phase 4 | Per-embedder cosine to centroid; returns `Vec<f32>`. |
| `mejepa_guard_check(panel, ..., taus)` | Phase 4 | Apply `Gτ`: `{passed: bool, failed_modalities: Vec<ModelId>}`. Deterministic. |
| `mejepa_verify(patch, context)` | Phase 4-7 | End-user inference: panel → predictor → constellation distance → guard check → conformal calibration → `Verdict`. |
| `mejepa_observe(panel, oracle_outcome)` | Phase 5 | Online update with EWC++ (UTML §V.3). |
| `mejepa_audit_pairwise_mi()` | Phase 1 | Pairwise MI matrix over N=21 embedders on the corpus. **Crucial for tightening the DDA upper bound.** |

---

## 11. Implementation roadmap (post-pivot, embedder-aware)

| # | Item | Effort | Dependency | Status |
|---|---|---|---|---|
| 1 | `Instrument` trait wrapper around shipped embedders (E1, E7, E11, E14) | ~1 day | none | new |
| 2 | E_Test / E_Problem / E_CommitMsg as thin E1 heads | ~1 day | (1) | new |
| 3 | E_Diff as Qodo+diff-encoder | ~2 days | (1) | new |
| 4 | E_Witness as E10+sequence-transformer | ~1.5 days | (1) | planned |
| 5 | E_AST as Qodo+AST-token-transformer | ~2 days | (1) + doc 14 chunker | new |
| 6 | E_CFG as E8+graph-attention | ~3 days | (1) + tree-sitter CFG | new |
| 7 | E_DataFlow as E11+graph-attention | ~3 days | (1) + def-use edges | new |
| 8 | E_TypeGraph as E11+Pyright | ~3 days | (1) + Pyright integration | new |
| 9 | E_Trace as E2/E3/E4+sequence transformer | ~3 days | (1) | new |
| 10 | E_StaticAnalysis (pure-Rust scalar aggregates) | ~2 days | none | planned |
| 11 | E_Runtime (pure-Rust scalar aggregates) | ~2 days | none | new |
| 12 | E_Reasoning as E17 AffectText on agent CoT | ~2 days | (1) | new |
| 13 | Predictor + frozen target readback + VICReg + Oracle head (Phase 2) | ~7-10 days | all instruments | per `plan_phase2_predictor.md` |
| 14 | UTML v1.0 L_step computation per training cert | ~2 days | (13) | per doc 11 §11 |
| 15 | LoRA self-healing wiring (Phase 5) | ~3 days | shipped LoRA + (13) | accelerated |
| 16 | Distillation-based online observe (Phase 5) | ~2 days | shipped distillation + (13) | accelerated |
| 17 | Cross-embedder coherence → ΔK supplement | ~1 day | (1) | per §6.1 |
| 18 | Cross-embedder spread → predictor_collapse_score | ~1 day | (1) | per §6.2 |
| 19 | E17 AffectText on agent CoT for ΔΩ_outer | ~2 days | (12) | per §6.3 |
| 20 | `audit_pairwise_mi` MCP tool — tightens DDA upper bound | ~2 days | (1) | new |

**Total:** ~38 days for full Phase 1-5 with embedder reuse, vs ~50+ days from scratch. TCT-specific deliverables (constellation centroids, Gτ guard, `Verdict::GuardRejected`) add ~14 more days; see doc 13 §7.

---

## 12. Risks (embedder-integration specific)

1. **Embedder model versioning drift** — each embedder pins to a specific HuggingFace revision. If updated upstream, the embedding space shifts. Pin SHA-256 digests in `models_config.toml`. ME-JEPA's predicted-vs-target distance is calibrated per `(embedder_version, panel_version)` pair.
2. **VRAM allocation contention** — simultaneous loading of 21 embedders + predictor + target buffers + LoRA + activations must stay under 29 GB. Lazy-load E15-E21 on demand if budget tightens.
3. **Cross-embedder coherence brittle on small corpora** — needs ≥1,000 training examples before NN sets become statistically meaningful. Default to `embedder_coherence = 0.5` (neutral) for first 1k entries.
4. **E17 AffectText calibration on AI conversation transcripts** — model trained on natural language, not LLM/agent transcripts. May need recalibration on 100-500 outer-loop conversation samples before `agent_state_score` is reliable.
5. **LoRA adapter explosion** — 21 embedders × N adapters per embedder (one per task or per language) could grow large. Cap at 1 base LoRA + ≤4 task-specific delta-LoRAs per embedder; merge older ones via SVD.
6. **DDA tightness uncertainty** — pair-signal headline (105 at N=14, 231 at N=21) is an upper bound. If pairwise MI audit shows high redundancy, effective signal count drops. ME-JEPA's predictor capacity needs are still well below the empirical floor in practice (steady-state signals/day ≫ params/predictor).

---

## 13. What this doc supersedes (or augments)

- **Doc 09 §2.2 instrument set** — augmented: instead of building 14 instruments from scratch, build 14 instruments as thin heads on the existing 21-embedder ensemble.
- **Doc 09 §5.1 online observe + EWC++** — augmented: shipped LoRA + distillation provides per-embedder fine-tuning path; EWC++ Fisher matrix sits on top.
- **Doc 11 §13 implementation roadmap** — replaced with the 20-item table in §11 above.
- **`specs/implementation-plan.md` Phase 2 predictor section** — Phase 2 predictor scope unchanged but inputs come from the embedder ensemble's panel.

---

## 14. The big-picture insight

> **The 21-embedder ensemble + LoRA/distillation/multitask training infrastructure is ~70% of what doc 09 specifies as ME-JEPA-Code's Phase 1-5 implementation.** It was originally built for general semantic search + memory infrastructure, but its architectural shape is exactly what UTML v1.0 prescribes for a self-healing JEPA: frozen base models (collapse-immune by construction at the embedder level — UTML §VI.2 analog), trainable LoRA adapters for selective plasticity, distillation for consolidation, multitask heads for ΔK auxiliary objectives, and an ensemble of complementary feature spaces for ΔΞ cross-validation.

The pivot from inner-actor LLM to ME-JEPA isn't "delete Qwen + build ME-JEPA from scratch." It's **"redirect the existing embedder ensemble's training infrastructure at the SWE-bench Docker oracle as teacher, with the outer-loop reality stream as data."** This is a much shorter critical path than doc 09 originally implied.

ME-JEPA-Code's predictor sits on top of 21 already-built embedders, fine-tuned via already-built LoRA + distillation, governed by UTML v1.0's L_step formula computed via the embedder ensemble's already-built evaluation harness, with the 105-signal-per-input (231 at N=21) DDA budget formalized by Royse 2026c. **The training corpus IS the reality loop running** — every Edit/Write call by Claude Code produces a panel update; every panel update produces 231-462 DDA signals; every claim the agent makes about its own work produces another 470-540 verification signals when checked (doc 13 §7.5.2). The "1500 days" of work doc 09 originally projected is more like 38 days when the embedder-substrate reuse is exploited.

---

## See also

- `crates/context-graph-embeddings/` — the 21-embedder ensemble + training infra (~80,000 LOC)
- `crates/context-graph-embeddings/src/types/model_id/core.rs` — E1-E14 enum
- `crates/context-graph-embeddings/src/learner/mod.rs` — E15-E21 learner-state slots
- `crates/context-graph-embeddings/src/training/` — LoRA + distillation + multitask + AdamW + InfoNCE
- `crates/context-graph-embeddings/src/traits/model_factory/memory.rs` — VRAM estimates (12.475 GB for E1-E14)
- Master plan: `09_END_GOAL_REVIEW_REPLACEMENT.md`
- Pivot architecture: `10_MEJEPA_PIVOT_2026_05_09.md`
- UTML v1.0 learning formula: `11_MEJEPA_LEARNING_FORMULA.md`
- TCT three-phase + universal verifier: `13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md`
- AST-aware chunk routing: `14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md`
- Source theory: `docs2/TheoryOfMachineLearning.md` (UTML v1.0 — JEPA Edition)
- Royse, C. (2026a). *ContextGraph Reality Loop*. ResearchGate DOI 10.13140/RG.2.2.16013.91366.
- Royse, C. (2026b). *Dynamic / ME-JEPA*. ResearchGate DOI 10.13140/RG.2.2.28485.41446.
- Royse, C. (2026c). *Teleological Constellation Training: Multi-Modal Embedding Decomposition as Meaning Compression for Identity-Locked Generation and a Third Path Around the Data Wall*. ResearchGate DOI 10.13140/RG.2.2.24370.57288.
