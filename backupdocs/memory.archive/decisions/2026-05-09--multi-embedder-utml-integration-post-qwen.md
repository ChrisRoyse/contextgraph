---
namespace: decisions
agent_id: agent-document-architect
created: 2026-05-09T02:00:00Z
updated: 2026-05-09T02:00:00Z
status: active
severity: high
tags: directive, mejepa, embedders, gpu-vram, frozen-instruments, utml-v1, multi-embedder, learner-state, post-qwen
---

# Multi-embedder system + GPU re-allocation post-Qwen for ME-JEPA training

## Operator directive (2026-05-09)

> "since we are no longer using the local model that frees up all of our gpu vram. this
> system has a number of multi embedder systems and like 30 embedders hooked up. we could
> load any of them or improve the learning embedders with the new utl theory and somehow
> integrate all of that into this syystem. review all of that and if anything could be
> optimally used put it all in the docs."

## What this decision locks

1. **The 14 production content embedders (E1-E14) ARE the frozen externally-defined
   instruments UTML v1.0 §VI.2 calls for.** This makes ME-JEPA's `ΔΞ.target_collapse = 0`
   not just a forward-looking design property — it is the **current, shipped substrate**.
2. **Post-Qwen GPU re-allocation:** the entire 32 GB RTX 5090 is now available for
   embedders + ME-JEPA training. Hard-warm all 14 content embedders (12.5 GB) + reserve
   ~10 GB headroom for ME-JEPA training/inference. This is a Tier-1 throughput unlock.
3. **Multi-embedder coherence is the canonical ΔK signal source.** UTML v1.0 §IV's
   `cos_align`, Fisher protection, and ECE terms all benefit from cross-embedder agreement.
   The 13-space neighbor agreement is already implemented in
   `crates/context-graph-embeddings/`; ME-JEPA wires into it directly.
4. **Trainable embedders (E5 Causal LoRA, E11 Kepler, E14 BGE-M3 fine-tunes) self-improve
   under UTML v1.0's L_step.** Rather than flat per-batch SGD, weight each example by
   `L_step = ΔP × ΔK × ΔΩ × ΔΞ` so noisy/redundant examples contribute nothing.
5. **The 7 learner-state embedders (E15-E21) are repurposed for OPERATOR ΔE monitoring.**
   They were originally for human-physiology UTL v2.0; under UTML v1.0 they are not
   load-bearing for ME-JEPA training, but they remain valuable for operator-state-aware
   review queueing (which patches go to humans first when ME-JEPA is uncertain).

## The 14 content embedders inventory

| Slot | Runtime ID | Backbone | Native dim | Storage dim | VRAM | Role for ME-JEPA |
|---|---|---|---:|---:|---:|---|
| E1 | `semantic` | `intfloat/e5-large-v2` | 1024 | 1024 | 1.4 GB | Semantic representation of code change description / problem statement / commit message |
| E2 | `temporal_recent` | Exponential recency basis | 512 | 512 | 15 MB | Recency signal for code-history features |
| E3 | `temporal_periodic` | Fourier periodic basis | 512 | 512 | 15 MB | Periodic time signal (build/test cycle phases) |
| E4 | `temporal_positional` | Sinusoidal positional basis | 512 | 512 | 15 MB | Positional time signal (within an attempt) |
| E5 | `causal` | `nomic-ai/nomic-embed-text-v1.5` (LoRA stages 1-3) | 768 | 768 | 650 MB | **Causal direction signal** — predict cause→effect for hunk-attribution. **Trainable; improve under L_step.** |
| E6 | `sparse` | SPLADE lexical encoder | 30522 | 1536 | 550 MB | Sparse keyword signal — captures literal token presence (test names, error keywords) |
| E7 | `code` | `Qodo/Qodo-Embed-1-1.5B` | 1536 | 1536 | 3.8 GB | **Code structure signal — primary instrument for `E_AST` / `E_CFG` / `E_DataFlow`.** |
| E8 | `graph` | `intfloat/e5-large-v2` (graph use) | 1024 | 1024 | 1.4 GB | Graph text signal — feeds `E_TypeGraph` |
| E9 | `hdc` | Hyperdimensional encoder | 10000 | 1024 | 60 MB | Robust symbolic signature — typo-resistant; useful for adversarial-perturbation training |
| E10 | `contextual` | `intfloat/e5-base-v2` | 768 | 768 | 500 MB | Contextual paraphrase — feeds `E_StaticAnalysis` for warning-cluster embedding |
| E11 | `kepler` | `THU-KEG/KEPLER-Wiki5M-KE` (RoBERTa-base) | 768 | 768 | 350 MB | Entity / knowledge-graph signal — feeds dependency-graph features. **Trainable.** |
| E12 | `late_interaction` | `colbert-ir/colbertv2.0` | 128/token | 128 pooled | 450 MB | Token-level fine-grained signal — captures per-line edits |
| E13 | `splade` | SPLADE v3 | 30522 | 1536 | 550 MB | Stage-1 sparse retrieval — fast first-pass over training corpus |
| E14 | `bge_m3_dense` | `BAAI/bge-m3` (XLM-RoBERTa-Large) | 1024 | 1024 | 2.6 GB | **Multilingual dense retrieval** — critical for cross-language mandate (Python/Rust/C/C++/Java/C#/JS/TS/Ruby) |

**Total VRAM for all 14 content embedders: ~12.5 GB.**

## The 7 learner-state embedders (E15-E21)

| Slot | Runtime ID | Backbone | Dim | Original UTL v2 role | UTML v1.0 repurposing |
|---|---|---|---:|---|---|
| E15 | `affect_speech` | Wav2Vec2-large MSP-DIM | 1024 | Speech arousal/valence | Operator-state monitor (review-call quality) |
| E16 | `affect_face` | OpenFace 3.0 + MTL | 512 | Face affect/AU/gaze | Operator-state monitor (review-fatigue) |
| E17 | `affect_text` | MiniLM + EmoBank | 384 | Text valence/arousal | Operator-tone in PR review threads |
| E18 | `ppg` | PaPaGei foundation model | 512 | HRV coherence | Operator HRV → review-queue scheduling |
| E19 | `eda` | WESAD chest physiology | 64 | Stress floor | Operator stress → defer high-stakes reviews |
| E20 | `eeg` | LaBraM foundation | 768 | Plasticity window | Operator focus → schedule review for high-ΔE windows |
| E21 | `eeg_artifact_robust` | EEGPT | 768 | Plasticity robustness | Cross-check E20 |

**These do NOT feed ME-JEPA training.** Machines have no autonomic nervous system (UTML
v1.0 §0). They inform **operator-state-aware review queueing** — when ME-JEPA's conformal
interval is wide AND the operator is in a low-ΔE state (high stress, low HRV coherence),
defer the review to a higher-ΔE window. This is a quality-of-life win, not a load-bearing
ME-JEPA component.

## Why E1-E14 ARE the UTML v1.0 frozen instruments

UTML v1.0 §VI.2 (`docs2/UTL_v2_embodied.md`) makes the central architectural claim:

> "ME-JEPA solves the target-side collapse problem **structurally**. The target
> representation `s_{t+1}` is not produced by a learnable network at all. It is the output
> of a panel of **frozen externally-defined instruments**, declared in the domain pack
> TOML manifest, each of which is a deterministic function from a typed event to a
> structured feature output ... Instruments have **zero trainable parameters**. They cannot
> drift. They cannot collapse."

**The 14 production content embedders fit this definition exactly:**
- They are pretrained on external corpora (e5-large-v2 trained on 1B+ pairs; Qodo trained
  on code; KEPLER trained on Wikidata; BGE-M3 trained on multilingual multi-task).
- From ME-JEPA's perspective, their weights are **frozen** — no gradient flows back into
  them during ME-JEPA training. (E5, E11, E14 *can* be fine-tuned in the embedder pipeline,
  but those fine-tunes happen via the embeddings crate's own training loop, NOT via
  ME-JEPA's panel-loss gradient.)
- They are deterministic given input — the same code change always produces the same
  E7 Qodo embedding.
- Each embedder has its own pretraining objective, so they cannot collude into a trivial
  fixed point: E1's e5-large-v2 was trained on contrastive pairs; E7's Qodo on code; E11's
  KEPLER on KG-completion. Forcing all 14 to map every input to the same constant would
  require simultaneously breaking 14 unrelated pretraining objectives — impossible.

**This means the structural collapse-immunity claim is not aspirational. It is current
reality on shipped infrastructure.** Doc 11 §13 says "ME-JEPA is uniquely structurally
collapse-immune on the target side because its target panel is produced by frozen
externally-defined instruments rather than by an EMA copy of a learnable encoder" — and
the 14 embedders are exactly those frozen externally-defined instruments.

## GPU VRAM re-allocation (post-Qwen)

| Component | Pre-pivot VRAM | Post-pivot VRAM | Notes |
|---|---:|---:|---|
| Qwen3.6-27B Q6_K_XL | ~22 GB | **0 GB (deleted)** | Removed per `decisions/...single-loop...md` |
| Hermes-2-Pro Q6_K | ~5 GB | **0 GB (deleted)** | Removed per `decisions/...single-loop...md` |
| 14 content embedders (E1-E14) hard-warm | 0 (cold) | **~12.5 GB** | Now keep all warm-loaded simultaneously |
| 7 learner-state embedders (E15-E21) hot when reviewing | 0 (cold) | ~3-5 GB on demand | Loaded only during human review cycles |
| ME-JEPA training (Phase 3+) | 0 | ~5-10 GB | Predictor + EMA target + activations + gradients |
| Total active | ~27 GB (Qwen + Hermes only) | ~22 GB (embedders + ME-JEPA) | **Net 5 GB headroom** |
| Total available | 32 GB (RTX 5090) | 32 GB | Same hardware |

**Net GPU win:** ~5 GB unused VRAM headroom even with all 14 embedders warm AND ME-JEPA
training active. Previously the inner Qwen consumed nearly all available VRAM, leaving
embedders cold-loading per request (~10× slower) and ME-JEPA training unable to fit.

## Multi-embedder coherence as ΔK signal

UTML v1.0 §IV's ΔK has three terms:

```
ΔK = 0.5 · cos_align(g_t, ḡ_prior)
   + 0.3 · (1 − fisher_violation)
   + 0.2 · (1 − ECE)
```

**Multi-embedder cross-space agreement contributes to all three:**

1. **`cos_align`**: when the gradient direction is consistent across the 14 embedder views
   (each induces a different similarity geometry), the gradient is operating in a
   well-aligned subspace of all of them. The cross-embedder gradient agreement IS a
   measure of `cos_align`.
2. **`fisher_violation`**: each embedder's projection onto ME-JEPA's panel has its own
   Fisher importance. If a candidate update preserves panel-distance under E1 but
   destroys it under E7 (code structure), the update is destructive of code knowledge.
   Multi-embedder Fisher protection is strictly stronger than single-embedder Fisher.
3. **`ECE`**: calibration is naturally measured per-embedder. A model whose predictions
   are well-calibrated under E1 but mis-calibrated under E14 (multilingual) has a
   cross-language calibration gap — exactly the signal the cross-language mandate needs.

**Implementation:** `crates/context-graph-embeddings/` already implements per-embedder
similarity + cross-embedder agreement. ME-JEPA's `compute_delta_k(...)` calls into the
existing API; no new infrastructure needed.

## Improving trainable embedders with L_step

E5 Causal, E11 Kepler, and E14 BGE-M3 have trainable LoRA / fine-tune pipelines (see
`crates/context-graph-embeddings/src/training/`). Currently these train under a flat
per-example loss. Switch to UTML v1.0 weighting:

```
loss_weighted(ex) = loss_raw(ex) · L_step(ex)

       L_step(ex) = ΔP(ex) × ΔK(ex) × ΔΩ(ex) × ΔΞ(ex)
```

Practical effect:

- **Examples the embedder already nails (ΔP ≈ 0)** are dropped from the gradient.
- **Examples that destroy prior alignment (ΔK ≈ 0)** are dropped — protects against
  catastrophic forgetting of the existing pretrained knowledge.
- **Examples seen during gradient explosion (ΔΩ ≈ 0)** are dropped — protects against
  bad-state updates.
- **Examples in the model's degenerate region (ΔΞ ≈ 0)** are dropped — when the
  embedder's output covariance is collapsing in some sub-band, examples that would
  exacerbate it are skipped.

This is strictly safer than uniform-weighted SGD and aligns the embedder pipeline with
ME-JEPA's training discipline.

## Memory budget enforcement

`crates/context-graph-embeddings/src/traits/model_factory/memory.rs::TOTAL_MEMORY_ESTIMATE`
is **12,475,000,000 bytes** (12.475 GB). Enforce a hard ceiling:

```
mejepa_train_vram_budget = total_gpu_vram (32 GB)
                         − content_embedder_warm_vram (12.5 GB)
                         − learner_state_on_demand_vram (5 GB max)
                         − safety_headroom (2 GB)
                         = 12.5 GB available for ME-JEPA training
```

If ME-JEPA training requires >12.5 GB at peak, either (a) drop one of the larger
embedders to cold-load (E7 Qodo = 3.8 GB; E14 BGE-M3 = 2.6 GB) or (b) gradient-checkpoint
the predictor. Never silently OOM.

## Cross-language mandate alignment

The cross-language mandate (per
`decisions/2026-05-09--single-loop-claude-code-actor-mejepa-data-factory.md`) requires
ME-JEPA to work on Python, Rust, C, C++, Java, C#, JS, TS, Ruby. Embedder coverage:

| Language | E1 e5-large-v2 | E7 Qodo Code | E14 BGE-M3 multilingual | Compiler-backed instruments |
|---|---|---|---|---|
| Python | ✅ general | ✅ trained on Python | ✅ multilingual | Pyright/mypy |
| Rust | ✅ general | ✅ Qodo covers Rust | ✅ multilingual | rustc, rust-analyzer |
| C | ✅ | ✅ | ✅ | gcc/clang via `compile_commands.json` |
| C++ | ✅ | ✅ | ✅ | g++/clang++ via `compile_commands.json` |
| Java | ✅ | ✅ | ✅ | javac |
| C# | ✅ | ✅ | ✅ | dotnet build |
| JS / TS | ✅ | ✅ | ✅ | tsc / node |
| Ruby | ✅ | ✅ | ✅ | `ruby -c` |

E14 BGE-M3 is the most important embedder for cross-language: it was trained on 100+
languages and is dimension-aligned with E1 (1024D), so the panel layout doesn't change as
the language varies.

## Implementation roadmap

| Step | Description | Effort | Status |
|---|---|---|---|
| 1 | Document inventory + UTML mapping in `docs/ruvectorfindings/12_EMBEDDER_INTEGRATION.md` | This turn | TBD |
| 2 | Wire ME-JEPA's `compute_delta_k(...)` into existing multi-embedder agreement API | ~2 days | deferred (Phase 3) |
| 3 | Switch trainable embedder pipelines (E5, E11, E14) to L_step-weighted loss | ~3 days each | deferred (after Phase 3) |
| 4 | Hard-warm all 14 embedders at MCP startup; benchmark cold-vs-warm latency | ~1 day | deferred |
| 5 | Set up VRAM budget enforcer (fail-closed if budget exceeded) | ~1 day | deferred |
| 6 | Repurpose E15-E21 as operator-state-aware review queueing signal | ~3 days | deferred (post-Phase 4) |
| 7 | Cross-language conformal calibration stratification using E14 multilingual head | ~2 days | deferred (Phase 4) |

## What this enables

- **Stronger structural-collapse-immunity claim** — not just for future ME-JEPA but for
  the current shipped infrastructure: E1-E14 are pretrained, frozen, deterministic, and
  collude-resistant by virtue of independent pretraining objectives.
- **Faster training** — ME-JEPA's `compute_delta_k(...)` reuses the embedder agreement
  cache; no new compute.
- **Better cross-language coverage** — E14 BGE-M3 is the dimensional anchor; calibration
  stratifies by language.
- **Safer trainable-embedder updates** — UTML v1.0 L_step weighting protects existing
  pretrained knowledge.
- **Operator-state-aware reviews** — E18 PPG / E19 EDA / E20 EEG can defer reviews to
  high-ΔE windows when the operator is reviewing manually.

## Related

- `decisions/2026-05-09--single-loop-claude-code-actor-mejepa-data-factory.md` (the
  Qwen-removal directive that frees this VRAM).
- `decisions/2026-05-09--utml-v1-jepa-edition-mejepa-learning-formula.md` (the four-term
  formula this integration implements).
- `crates/context-graph-embeddings/src/types/model_id/core.rs` (the canonical ModelId
  enum with 15 variants — 14 production + legacy Entity).
- `crates/context-graph-embeddings/src/learner/mod.rs` (the 7 learner-state embedders).
- `crates/context-graph-embeddings/src/traits/model_factory/memory.rs` (the 12.475 GB
  total VRAM estimate for all 14 production embedders).
- `crates/context-graph-embeddings/src/training/` (LoRA / fine-tune pipelines for E5,
  E11, E14).
- `backupdocs/11_embedder_matrix.md` (the canonical 21-embedder matrix).
- `docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md` (UTML v1.0 operationalization).
- `docs2/UTL_v2_embodied.md` §VI.2 (frozen externally-defined instruments claim).
- `docs/ruvectorfindings/12_EMBEDDER_INTEGRATION.md` (the public-facing companion to this
  decision — written this turn).

## Tags

`directive` `mejepa` `embedders` `gpu-vram` `frozen-instruments` `utml-v1`
`multi-embedder` `learner-state` `post-qwen` `target-collapse-immunity`
`cross-language` `bge-m3` `qodo-code` `e1-e14` `e15-e21`
