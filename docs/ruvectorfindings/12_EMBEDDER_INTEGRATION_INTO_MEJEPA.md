# 12 — Embedder Integration into ME-JEPA

> **Operator directive 2026-05-09:** *"since we are no longer using the local model that frees up all of our gpu vram. this system has a number of multi embedder systems and like 30 embedders hooked up. we could load any of them or improve the learning embedders with the new utl theory and somehow integrate all of that into this system. review all of that and if anything could be optimally used put it all in the docs."*

> **Companion docs:**
> - `09_END_GOAL_REVIEW_REPLACEMENT.md` — master plan (instruments, panel, predictor, frozen target readback, VICReg, oracle head)
> - `10_MEJEPA_PIVOT_2026_05_09.md` — post-pivot single-outer-loop architecture
> - `11_MEJEPA_LEARNING_FORMULA.md` — UTML v1.0 four-factor formula

---

## 0. Headline

**The embedder ensemble is ME-JEPA's Phase 1 instrument set, already built.** `crates/context-graph-embeddings` ships **21 embedders** (E1–E21) with **production-grade training infrastructure** (LoRA + distillation + multitask + AdamW + InfoNCE + evaluation). With Qwen retired (-22 GB VRAM), there is **~29 GB free on the RTX 5090**, enough to host every weight in the ensemble simultaneously.

**Implication:** doc 09's planned 14 ME-JEPA instruments do not need to be built from scratch. The existing 21 embedders cover the same conceptual space (semantic, code, graph, sparse retrieval, causal, multilingual, multimodal, learner-state) and are **ready to compose into the panel**. The work shifts from "build 14 instruments" to "wire the 21 existing embedders into the `Instrument` trait + add 3 net-new code-domain heads (`E_AST`, `E_CFG`, `E_DataFlow`) the ensemble doesn't already cover."

The shipped LoRA + distillation infrastructure is also *exactly* what UTML v1.0 §V.1 (effective_plasticity / continual backprop / EWC++-grade self-healing) requires. The embedder fine-tuning loop is the ME-JEPA fine-tuning loop, with the same training-cert + witness-chain provenance.

---

## 1. Current ensemble inventory (21 embedders, ~19 GB on disk)

| Slot | Name | Source model | Native dim | Disk | Trainable today | UTML role |
|---|---|---|---|---|---|---|
| **E1** | Semantic | `intfloat/e5-large-v2` | 1024 | 1.3 GB | inference | content / `E_Test`, `E_Problem`, `E_CommitMsg` source |
| **E2** | TemporalRecent | custom (exp decay) | 512 | <1 MB | yes (custom) | content / temporal axis of panel |
| **E3** | TemporalPeriodic | custom (Fourier) | 512 | <1 MB | yes (custom) | content / temporal axis |
| **E4** | TemporalPositional | custom (sinusoidal) | 512 | <1 MB | yes (custom) | content / temporal axis |
| **E5** | Causal | `nomic-ai/nomic-embed-text-v1.5` | 768 | 574 MB | yes (LoRA + distillation infra shipped — see `training/`) | content / cause-effect supervision |
| **E6** | Sparse | `naver/splade-cocondenser` | ~30K → 1536 | 687 MB | inference | content / sparse retrieval, anti-collapse signal |
| **E7** | Code | `Qodo/Qodo-Embed-1-1.5B` | 1536 | **5.8 GB** | inference | content / **direct E_AST/E_Diff candidate** |
| **E8** | Graph | `intfloat/e5-large-v2` (upgraded) | 1024 | 88 MB | inference | content / graph / sentence |
| **E9** | HDC | custom (10K-bit holographic) | 10K → 1024 | 4 KB | yes (custom) | content / panel-dim compression candidate |
| **E10** | Contextual | `intfloat/e5-base-v2` | 768 | 419 MB | inference | content / paraphrase |
| **E11** | Kepler (Entity legacy alias) | RoBERTa-base + TransE on Wikidata5M | 768 | 481 MB | inference | content / **E_TypeGraph candidate** (knowledge-graph embedder) |
| **E12** | LateInteraction | `colbert-ir/colbertv2.0` | 128/token | 419 MB | inference | content / fine-grained re-rank |
| **E13** | Splade v3 | sparse v3 head | ~30K | 688 MB | inference | content / Stage-1 recall in 5-stage pipeline |
| **E14** | BgeM3Dense | `BAAI/bge-m3` (XLM-RoBERTa-Large) | 1024 | **4.3 GB** | inference | content / **multilingual + 8192-token context** — direct candidate for cross-language SWE-bench Lite (Section C mandate) |
| **E15** | AffectSpeech | learner-state, 1024 | 1024 | 631 MB | yes | learner-state / **UTL ΔΩ probe** |
| **E16** | AffectFace | learner-state, 512 | 512 | 655 MB | yes | learner-state |
| **E17** | AffectText | learner-state, 384 | 384 | 932 MB | yes | learner-state / **agent-CoT affect probe** (no sensors needed — runs on conversation transcripts) |
| **E18** | PPG | learner-state, 512 | 512 | 23 MB | yes | learner-state / sensor (operator biofeedback IF available) |
| **E19** | EDA | learner-state, 64 | 64 | 92 KB | yes | learner-state / sensor |
| **E20** | EEG | learner-state, 768 | 768 | 23 MB | yes | learner-state / sensor |
| **E21** | EegArtifactRobust | learner-state, 768 | 768 | 97 MB | yes | learner-state / sensor |

**Source code:** `crates/context-graph-embeddings/src/types/model_id/core.rs` (E1–E14 enum) + `crates/context-graph-embeddings/src/learner/mod.rs` (E15–E21 namespace).

**Total disk:** ~19 GB. **Typical VRAM footprint when all loaded:** ~14–18 GB (embedders are dramatically smaller in VRAM than disk because most of the disk weight is fp32; loaded as bf16 the ensemble fits comfortably in the ~29 GB free post-Qwen).

---

## 2. Shipped training infrastructure (already in tree)

`crates/context-graph-embeddings/src/training/` (~3,000 LOC):

| Module | Purpose | UTML / ME-JEPA role |
|---|---|---|
| `data.rs` | Training pair structures + DataLoader | corpus ingestion |
| `loss.rs` | InfoNCE + directional + separation + soft-label losses | predictor-side ΔP loss |
| `optimizer.rs` | AdamW with warmup + cosine decay | Phase 3 optimizer (already specified in doc 09) |
| `trainer.rs` | Training loop with **momentum/teacher utilities** | reusable optimizer scaffolding only; ME-JEPA targets still come from frozen instrument readback |
| `evaluation.rs` | Directional accuracy, MRR, AUC | Phase 4 evaluation harness |
| `distillation.rs` | Online embedder teaching loop with EMA-style quality gates | optional consolidation utility; Docker oracle/readback remains the teacher signal |
| `lora.rs` | LoRA adapters (rank, alpha, dropout) — applies to query/key/value | UTML §V.1 effective_plasticity intervention: continual backprop via low-rank updates |
| `multitask.rs` | DirectionLabel + MechanismLabel auxiliary heads | doc 09 §3.7 multi-objective auxiliary heads |
| `pipeline.rs` | End-to-end training pipeline | Phase 3 training loop |

`crates/context-graph-embeddings/src/learner/` (~600 LOC):

- E15–E21 learner-state slots with explicit modality, output_dimension, scalar_heads
- `LearnerEmbedderInput` enum (Text / Samples / Features) for cross-modality input
- `LearnerEmbeddingOutput` with vector + scalars + embedder_version + latency
- `CalibrationDatasetSpec` for E15–E21 calibration

`crates/context-graph-embeddings/src/gpu/`:

- `device` + `memory` + `model_loader` + `ops` + `tensor` — full candle-CUDA stack already wired to the RTX 5090
- This is the same stack ME-JEPA's predictor will sit on (Phase 2)

---

## 3. The integration map: 21 embedders → 14 ME-JEPA instruments

Doc 09 §2.2 specifies 14 instruments for the 5,120-d panel (per §3.5.5). Mapping the shipped ensemble onto those slots:

> **AST-aware routing precondition (per doc 14).** Every code-domain instrument below operates on **AST-perfect atomic chunks** produced by the multi-language `crates/context-graph-core/src/memory/ast_chunker.rs`, NOT raw text. Each chunk carries `(language, entity_type, parent_chain, line_start, line_end, sha256)` metadata; a per-chunk-type embedder routing table (doc 14 §4) decides which embedder slots receive which chunks. This means the table below is the *steady-state* mapping after Phase 1 ships the multi-language chunker extension (doc 14 §6 items 1-5, ~10 days of Phase 1 effort). For a typical SWE-bench Lite mutation entry, ~30-50 chunks pass through the ensemble, yielding ~280 chunk-level signals on top of the ~231 panel-level signals (per doc 14 §4.1) — every signal is **fully labeled** with the known oracle outcome, mutation category, entity type, and language.

| Doc 09 instrument | Dim | Existing embedder(s) | Effort |
|---|---|---|---|
| **E_AST** | 384 | **E7 Code (Qodo, 1536D)** with linear projection 1536→384, OR a small AST-token transformer fine-tuned on top of E7 | ~2 days (head only) |
| **E_CFG** | 256 | **E8 Graph (e5-large-v2)** + tree-sitter CFG extraction with graph-attention head | ~3 days |
| **E_DataFlow** | 256 | **E11 Kepler (RoBERTa+TransE)** + def-use edge extraction with graph-attention head | ~3 days |
| **E_TypeGraph** | 256 | **E11 Kepler** (knowledge-graph embedder, perfect fit) + Pyright/mypy lattice | ~3 days |
| **E_Test** | 384 | **E1 Semantic (e5-large-v2)** on test-fn natural-language descriptions | ~1 day (E1 reuse) |
| **E_Trace** | 512 | **E2/E3/E4 temporal embedders** + sequence-transformer over `(fn_id, line_no, val_hash)` events | ~3 days |
| **E_Diff** | 256 | **E7 Code (Qodo)** with diff-edge encoder | ~2 days |
| **E_Witness** | 256 | **E10 Contextual (e5-base-v2)** on the 73-byte SHAKE-256 chain entries (sequence transformer) — deterministic encoder shipped; neural replacement tracked in specs | ~1.5 days (per existing plan) |
| **E_Oracle** | 128 | ✅ shipped (`crates/context-graph-mejepa-instruments/src/e_oracle.rs`) | done |
| **E_Problem** | 1024 | **E1 Semantic** on SWE-bench problem text | ~1 day |
| **E_CommitMsg** | 384 | **E1 Semantic** on commit text | ~1 day |
| **E_StaticAnalysis** | 256 | per `plan_e_static_analysis.md` — pure-Rust scalar aggregates (mypy/Pyright/ruff/clippy) | ~2 days |
| **E_Runtime** | 256 | aggregates: walltime + RSS + coverage Δ + network signals | ~2 days |
| **E_Reasoning** | 384 | **E17 AffectText** on outer-loop agent CoT (Claude Code conversation transcripts) — affect-aware embedding | ~2 days |

**Net effect:** 8 of 14 instruments can be built as **thin heads on existing embedders** (1–3 days each). The 6 not-quite-direct mappings (`E_Trace`, `E_StaticAnalysis`, `E_Runtime`, `E_Witness`, `E_Oracle`, `Scalars`) are Phase 0/1 work that's already shipped or planned. Total Phase 1 effort with embedder reuse: **~25 days** vs the original ~30+ days from scratch.

**Cross-language win (Section C mandate):** swapping E1 → **E14 BgeM3Dense** wherever practical buys multilingual support (Python, Rust, C, C++, Java, C#, JS, TS, Ruby) at no additional training cost. BGE-M3's 8192-token context also accommodates much larger code chunks than E5's 512-token limit.

---

## 4. The post-Qwen GPU VRAM budget

Pre-pivot (2026-05-08): RTX 5090 32 GB
- Qwen3.6-27B (Q6_K_XL GGUF): ~22 GB resident
- KV-cache: ~3 GB
- Headroom: ~7 GB

Post-pivot (2026-05-09):
- Qwen retired: **+22 GB freed**
- Hermes retired: **+1.5 GB freed** (when active)
- **Total free**: ~32 GB minus baseline allocations (~3-4 GB system + driver) = **~28-29 GB available for ME-JEPA + embedders**

**Proposed allocation (target steady state):**

| Component | VRAM | Justification |
|---|---|---|
| Embedder ensemble (21 weights, bf16) | ~14 GB | core panel-materialization pipeline |
| ME-JEPA predictor (6-layer transformer, hidden 1024, 8 heads, bf16) | ~3 GB | Phase 2 |
| Frozen target-readback buffers | <1 GB | Phase 2; target panels are persisted instrument outputs, not a second trainable model |
| Activation memory (batch 64) | ~5 GB | Phase 3 forward+backward |
| Optimizer state (AdamW) | ~2 GB | per-param momentum + variance |
| LoRA adapters (rank 32, all 21 embedders) | ~0.5 GB | Phase 5 self-healing fine-tuning |
| KV-cache + scratch | ~1 GB | inference path |
| **Total** | **~28-29 GB** | fits |

**Headroom for batch-128 training, gradient checkpointing, or larger predictor (12-layer)**: available if needed, by reducing the embedder load (lazy-load infrequently-used learner-state embedders).

---

## 5. UTML v1.0 integration — the embedder ensemble as ΔK and ΔΞ instrumentation

UTML v1.0's `L_step = ΔP × ΔK × ΔΩ × ΔΞ` is meant to be **measurable in real time per training step**. The embedder ensemble provides the measurement substrate:

### 5.1 ΔK — Knowledge coherence via cross-embedder agreement

UTML §IV.1's `cos_align(g_t, ḡ_prior)` measures gradient agreement with prior knowledge. The embedder ensemble adds a **complementary** signal: **cross-embedder neighbor agreement**.

For any candidate panel `panel_t1`, run all 14 content embedders (E1–E14) and check: do their nearest-neighbor sets in the corpus agree? Strong agreement = the panel sits in a region all embedders recognize = high coherence. Disagreement = the panel is in an embedder-specific edge case = low coherence (could be OOD, could be a structurally unusual code change).

```
embedder_coherence(panel) = mean(jaccard(NN_i(panel), NN_j(panel))
                                 for all (i, j) in content embedder pairs)

ΔK_supplement(panel) = embedder_coherence(panel)
```

Add as a fourth subterm to the existing UTML §IV `ΔK_t` formula:

```
ΔK_t = clamp_[0,1](
  0.5 · cos_align(g_t, ḡ_prior)
  + 0.3 · (1 − fisher_violation)
  + 0.15 · (1 − ECE)
  + 0.05 · embedder_coherence(panel_t)
)
```

Weights renormalized to keep total at 1.0; the new term is small (0.05) because gradient alignment + Fisher protection are the load-bearing signals.

### 5.2 ΔΞ — Predictor-side anti-collapse via embedder diversity

UTML §VI.4's `predictor_redundancy_t` measures pairwise mutual-information across panel slots. The embedder ensemble provides a **structural** anti-collapse defense beyond MI auditing:

If the predictor's output `g_φ(panel_t01)` collapses to a constant (or low-rank), then **all 14 content embedders' similarity scores between predicted and target panels will degrade in lockstep**. Cross-embedder disagreement on the predicted-vs-target similarity is itself a powerful collapse-detector:

```
predicted_similarity_per_embedder = [cos(predicted_panel, target_panel) under E_i for i in 1..14]
predictor_collapse_score = 1 - std(predicted_similarity_per_embedder)
```

When the predictor is healthy, different embedders produce different similarity scores reflecting their different feature spaces — high std. When the predictor has collapsed, all embedders agree (because the predicted panel is a constant) — low std → high collapse score.

This is doc 11's **"negative-action ablation falsifier" extended across the embedder ensemble**: instead of just removing action conditioning and measuring drop, we measure the *spread* of similarity scores across 14 distinct feature spaces. A healthy predictor produces diverse spreads; a collapsed one produces uniform agreement.

### 5.3 ΔΩ — Optimizer state via E15-E21 learner-state embedders

UTML §V's effective_plasticity / landscape_health / stability_floor measures the *model's* state. The E15–E21 learner-state embedders measure the *agent's* state (the outer-loop agent — Claude Code — producing the training data).

| E15-E21 slot | Maps to UTML signal |
|---|---|
| **E17 AffectText** (no sensors needed) | Conversation transcripts + recommendation JSONs → UTML's `valence_arousal` term in §V.2. A frustrated / cycling-on-the-same-error outer-loop session produces low-quality training data. |
| **E15 AffectSpeech** (when audio available) | Operator's verbal interactions during pair-coding sessions. |
| **E16 AffectFace** (when video available) | Operator's facial state. |
| **E18 PPG, E19 EDA, E20/E21 EEG** | Operator's autonomic state (when sensors hooked up). |

Even without sensors, **E17 AffectText running on Claude Code's conversation transcripts provides a measurable outer-loop-state signal**:

```
agent_state_score = E17(conversation_transcript_window).valence
                  · E17(conversation_transcript_window).arousal_inverted_U

ΔΩ_outer = (model_plasticity)^0.4
         · (model_landscape_health)^0.4
         · (model_stability_floor)^0.1
         · (agent_state_score)^0.1
```

Weights renormalized to keep total at 1.0; agent_state contributes the smallest factor because the model's state is more directly causal of L_step than the operator's state.

This **closes the loop** between UTL v2.0's biological framework (which the operator originally cited) and UTML v1.0's ML-native rewrite: the biology was always real; in this system, it's measured via the learner-state embedders watching the outer-loop agent.

---

## 6. Self-healing via shipped LoRA + distillation infrastructure

UTML §V.1 prescribes **continual backprop** (Sutton 2024 — selective re-initialization of dormant units) as the intervention when `effective_plasticity` collapses. The shipped `crates/context-graph-embeddings/src/training/lora.rs` is the more elegant alternative: instead of re-initializing dormant units, fine-tune via LoRA adapters on a fresh slice of the reality stream.

```
if ΔΩ.effective_plasticity < 0.4:
    # The frozen-base embedders never lose plasticity (they're frozen).
    # Only the LoRA adapters need refreshing.
    refresh_lora_adapter(embedder=E_i, fresh_corpus_slice, rank=32, epochs=10)
```

Similarly UTML §VII (consolidation): the shipped `distillation.rs` is the operational form of M(t):

```
# Online observe → distill into LoRA adapters with EMA τ
distillation_loop {
    pull_recent_observations()                       # from §3.8 shift-log subscriber
    for embedder in active_embedders:
        teacher_score = oracle.score(panel)           # the new ground truth
        student_score = embedder.score(panel)         # the current prediction
        loss = info_nce_loss(student, teacher)
        student.backward(loss)
        ema_update(student.weights, τ=0.996)          # the "future-self prior" of UTML §VIII
}
```

The distillation infrastructure already supports `quality_gate: true` — examples below confidence threshold are skipped. This is **UTML's `signal_clarity` term** baked in.

---

## 7. The `mejepa-shift-subscriber` (Phase 7 §3.8) consumes the embedder pipeline

Doc 09 §3.8 specifies a Tokio task that tails the shift log + harness artifacts and feeds `mejepa.observe()`. With the embedder ensemble loaded:

```
for shift in cgreality_shift_log_tail() {
    panel_t1 = build_panel_via_embedder_ensemble(shift.after_sha)
                    # parallel inference: E1, E5, E7, E8, E10, E11, E14 on the post-edit code
                    # parallel inference: E2-E4 on the temporal context
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

**Latency budget** (per shift):
- Parallel embedder inference (14 embedders, batch 1, RTX 5090): ~50ms (each <5ms)
- Panel materialization: <10ms
- ME-JEPA forward: <10ms
- VICReg + L_step computation: <5ms
- RocksDB persistence: <5ms
- **Total: ~80ms per shift** — well under the §3.8.6 budget of 5s p50 / 30s p99

---

## 8. New work this enables (vs. doc 09's original plan)

### 8.1 Phase 1 (instruments) — 25 days vs ~35 days

8 of 14 instruments become thin heads on existing embedders (~1-3 days each). 3 are pure-Rust scalar aggregates tracked in `specs/implementation-plan.md`. 3 are net-new (E_AST/E_CFG/E_DataFlow tree-sitter integrations).

### 8.2 Phase 2 (predictor) — unchanged ~7-10 days

The predictor architecture (6-layer transformer) is independent of the embedder choice; embedders feed the panel, predictor is downstream.

### 8.3 Phase 3 (training) — accelerated by reusing shipped infrastructure

`crates/context-graph-embeddings/src/training/{trainer.rs, optimizer.rs, loss.rs, evaluation.rs, pipeline.rs}` is the Phase 3 training-loop scaffolding ready to extend. AdamW + InfoNCE + AUC eval already work end-to-end on the ensemble — Phase 3 wires the predictor into this pipeline rather than building a new one.

### 8.4 Phase 5 (self-healing) — shipped infrastructure becomes operational

LoRA adapters + distillation + multitask + momentum-style utilities are all in tree. Phase 5 becomes wiring (and the EWC++ Fisher matrix), not green-field implementation.

### 8.5 Cross-language Section-C mandate — directly satisfied by E14 BGE-M3

Multilingual coverage at no additional model-training cost. Pyright/mypy/rustc/javac/dotnet build/etc. already produce typed diagnostics fed into E_StaticAnalysis; the embedder substrate handles the underlying source-text encoding for any of the 9 supported languages.

---

## 9. Implementation roadmap (post-pivot, embedder-aware)

| # | Item | Effort | Dependency | Status |
|---|---|---|---|---|
| 1 | `Instrument` trait wrapper around shipped embedders (E1, E7, E11, E14) | ~1 day | none | new |
| 2 | E_Test / E_Problem / E_CommitMsg as thin E1 heads | ~1 day | (1) | new |
| 3 | E_Diff as Qodo+diff-encoder | ~2 days | (1) | new |
| 4 | E_Witness as E10+sequence-transformer (per `plan_e_witness.md`) | ~1.5 days | (1) | planned |
| 5 | E_AST as Qodo+AST-token-transformer | ~2 days | (1) | new |
| 6 | E_CFG as E8+graph-attention | ~3 days | (1) + tree-sitter CFG extraction | new |
| 7 | E_DataFlow as E11+graph-attention | ~3 days | (1) + def-use edges | new |
| 8 | E_TypeGraph as E11+Pyright | ~3 days | (1) + Pyright integration | new |
| 9 | E_Trace as E2/E3/E4+sequence transformer | ~3 days | (1) | new |
| 10 | E_StaticAnalysis (pure-Rust scalar aggregates) | ~2 days | none | planned per `plan_e_static_analysis.md` |
| 11 | E_Runtime (pure-Rust scalar aggregates) | ~2 days | none | new |
| 12 | E_Reasoning as E17 AffectText on agent CoT | ~2 days | (1) | new |
| 13 | Predictor + frozen target readback + VICReg + Oracle head (Phase 2) | ~7-10 days | all instruments | per `plan_phase2_predictor.md` |
| 14 | UTML v1.0 L_step computation per training cert | ~2 days | (13) | per doc 11 §11 |
| 15 | LoRA self-healing wiring (Phase 5) | ~3 days | shipped LoRA + (13) | accelerated |
| 16 | Distillation-based online observe (Phase 5) | ~2 days | shipped distillation + (13) | accelerated |
| 17 | Cross-embedder coherence as ΔK supplement | ~1 day | (1) | per doc 12 §5.1 |
| 18 | Cross-embedder spread as predictor_collapse_score | ~1 day | (1) | per doc 12 §5.2 |
| 19 | E17 AffectText on agent CoT for ΔΩ_outer | ~2 days | (12) | per doc 12 §5.3 |

**Total:** ~38 days for full Phase 1-5 with embedder reuse, vs ~50+ days from scratch.

---

## 10. Risks (specific to embedder integration)

1. **Embedder model versioning drift**: each embedder pins to a specific HuggingFace revision. If a model is updated upstream, the embedding space shifts. Pin SHA-256 digests in `models_config.toml`. ME-JEPA's predicted-vs-target distance is calibrated per (embedder_version, panel_version) pair.
2. **VRAM allocation contention**: simultaneous loading of all 21 embedders + the predictor + target-readback buffers + LoRA adapters + activation memory must stay under 29 GB. Lazy-load learner-state embedders (E15-E21) on demand if budget tightens.
3. **Cross-embedder coherence brittle on small corpora**: the embedder_coherence term in §5.1 needs ≥1,000 training examples before NN sets become statistically meaningful. Default to `embedder_coherence = 0.5` (neutral) for the first 1k corpus entries.
4. **E17 AffectText calibration on AI conversation transcripts**: the model was trained on natural language, not LLM/agent transcripts. AffectText's outputs may need recalibration on a held-out set of 100-500 outer-loop conversation samples before the agent_state_score becomes reliable.
5. **LoRA adapter explosion**: 21 embedders × N LoRA adapters per embedder (one per task or per language) could grow large. Cap at 1 base LoRA + ≤4 task-specific delta-LoRAs per embedder; merge older ones via SVD compression.

---

## 11. What this doc supersedes (or augments)

- **Doc 09 §2.2 instrument set**: augmented — instead of building 14 instruments from scratch, build 14 instruments as thin heads on the existing 21-embedder ensemble.
- **Doc 09 §3.4 holdout split**: unchanged.
- **Doc 09 §5.1 online observe + EWC++**: augmented — the shipped LoRA + distillation infrastructure provides the per-embedder fine-tuning path; EWC++ Fisher matrix sits on top.
- **Doc 11 §13 implementation roadmap**: replaced with the 19-item table in §9 above.
- **`specs/implementation-plan.md` Phase 2 predictor section**: the Phase 2 predictor scope is unchanged but its inputs come from the embedder ensemble's panel; update plan when picking up.

---

## 12. The big-picture insight

> **The 21-embedder ensemble + the LoRA/distillation/multitask training infrastructure is ~70% of what doc 09 specifies as ME-JEPA-Code's Phase 1-5 implementation.** It was built originally for general semantic search + memory infrastructure, but its architectural shape is exactly what UTML v1.0 prescribes for a self-healing JEPA: frozen base models (collapse-immune by construction at the embedder level — UTML §VI.2 analog), trainable LoRA adapters for selective plasticity, distillation for consolidation, multitask heads for ΔK auxiliary objectives, and an ensemble of complementary feature spaces for ΔΞ cross-validation.

The pivot from inner-actor LLM to ME-JEPA isn't just "delete Qwen + build ME-JEPA from scratch." It's "**redirect the existing embedder ensemble's training infrastructure at the SWE-bench Docker oracle as teacher, with the outer-loop reality stream as data.**" This is a much shorter critical path than doc 09 originally implied.

ME-JEPA-Code's predictor sits on top of 21 already-built embedders, fine-tuned via already-built LoRA + distillation, governed by UTML v1.0's L_step formula computed via the embedder ensemble's already-built evaluation harness. The "1500 days" of work doc 09 originally projected is more like 38 days when the embedder-substrate reuse is exploited.

---

## See also

- `crates/context-graph-embeddings/` — the 21-embedder ensemble + training infra (~80,000 LOC)
- `crates/context-graph-embeddings/src/types/model_id/core.rs` — E1-E14 enum
- `crates/context-graph-embeddings/src/learner/mod.rs` — E15-E21 learner-state slots
- `crates/context-graph-embeddings/src/training/` — LoRA + distillation + multitask + AdamW + InfoNCE infrastructure
- Master plan: `09_END_GOAL_REVIEW_REPLACEMENT.md`
- Pivot architecture: `10_MEJEPA_PIVOT_2026_05_09.md`
- UTML v1.0 learning formula: `11_MEJEPA_LEARNING_FORMULA.md`
- Source theory: `docs2/UTL_v2_embodied.md` (UTML v1.0 — JEPA Edition)
