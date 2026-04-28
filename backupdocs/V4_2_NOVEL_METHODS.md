# Shakespeare LoRA v4.2 — Novel Methods Enabled by the 13-Embedder × 249k-Record Asset

**Author**: Coordinator (Claude Opus 4.7, 1M context)
**Date**: 2026-04-21
**Status**: Research synthesis + plan proposal (not yet executed)
**Audience**: ML engineer / systems architect planning the v4.2 training run

---

## Executive Summary

v4.1 failed in production because the community's standard toolbox (SFT, DPO with judge-LLM pairs, DRY sampler, rep-penalty heuristics) operates on the surface of the text. We have something the community does not: **249,000 records, each decomposed into 13 orthogonal embedder views (E1-E13) with full provenance labels**. This is not a corpus — it's a **latent-space atlas of a style domain**.

This asset enables methods that literature can't try because they lack the ground-truth multi-view labels at scale. After 25+ web searches across preference optimization, repetition mitigation, guardrail design, multilingual LoRA, and evaluation methodology, I believe there are **15 novel techniques** uniquely enabled by our asset, and **3 of them together constitute the optimal v4.2 recipe**.

### Top 3 novel methods (recommended v4.2 core)

1. **Constellation-Anchored Multi-View DPO (CAMD-DPO)** — Replace judge-LLM preference ranking with deterministic distance-to-constellation scoring across all 13 embedders. Mines preference pairs from the 249k corpus with content held constant (E1 semantic) and style varied (E12/E13 ColBERT/SPLADE). Zero pair-labeling cost; zero reward hacking surface.

2. **Embedder-Gated Token Loss (EGTL)** — Per-token loss weighting during SFT based on 13-embedder divergence from Shakespeare prototypes. Surgical: the model only re-learns the tokens that are genuinely off-style, leaving correct tokens untouched. Prevents the distribution-collapse failure mode that killed v4.1.

3. **Multi-View Agreement Guard (MVAG)** — Runtime safeguard that replaces the single-score style guard with an N-of-13 agreement rule. Repetition and degeneration manifest differently in each embedder view (E5 causal flatness, E2-E4 temporal stagnation, E12 MaxSim self-similarity, E9 HDC cycle detection). A 4-of-5 agreement across a weighted view panel catches failure modes that single-score guards miss.

These three, deployed together, address all 13 failure modes documented in `tmp/SITREP_2026-04-21_151147.md` with compute and engineering cost comparable to v4.1.

---

## Part 1 — The Asset

### 1.1 What we have

249,000 records in `contextgraph_data/` RocksDB. Each record, on ingestion, is decomposed into:

| Embedder | Dim | Type | What it encodes |
|----------|-----|------|------------------|
| E1 | 1024 | dense | Semantic meaning (e5-large-v2) |
| E2-E4 | 512 each | dense | Temporal signatures (sinusoidal PE over created_at) |
| E5 | 768 | asymmetric | Causal direction (cause ≠ effect vectors) |
| E6 | 30K | sparse | Inverted keyword index |
| E7 | 1536 | dense | Code structure (UniXCoder) |
| E8 | 1024 | dense | Graph topology (node + neighborhood) |
| E9 | 1024 | dense | Holographic Distributed Codes (symbolic binding) |
| E10 | 1024 | asymmetric | Paraphrase (doc ≠ query vectors, multimodal) |
| E11 | 768 | dense | Entity representation (Kepler) |
| E12 | 128/token | per-token | ColBERT late-interaction |
| E13 | 30K | sparse | SPLADE expansion |

Every record also has:
- **Ground-truth metadata**: source, session, timestamp, operator, causal-extraction provenance
- **Entity links** via E11 → Kepler knowledge graph
- **Causal DAG edges** via E5 → CausalMemory graph
- **Pre-computed HNSW indexes** over E1, E1-matryoshka-128, E2-E4, E5, E5-cause, E5-effect, E7-E11, E10-multimodal-paraphrase, E10-multimodal-context

### 1.2 Why this is unprecedented

Public Shakespeare-style / persona-style datasets are either:
- Raw text (no labels, no views) — e.g., Project Gutenberg scrapes
- Single-view labeled (sentiment, toxicity, style binary) — e.g., Yelp Polarity, StylePTB
- Judge-LLM scored (noisy, expensive, ungrounded) — e.g., PairRM, UltraFeedback

**Nobody has simultaneously labeled 249k records with causal direction + temporal signature + entity graph + per-token late-interaction + sparse lexical expansion + HDC symbolic binding.** The closest public analog is mC4/RedPajama tagged with LID and toxicity — two views, not thirteen.

The implication: every preference-optimization or style-distillation technique that public research has tried was built around the constraint of single-view rewards. We can redesign those techniques with multi-view rewards.

### 1.3 What the Sonnets constellation gives us

Constellation `18ef2460-9ab6-44fc-8d7e-8d7513174ac8` is a pre-computed centroid + covariance in 13-embedder space, built from Shakespeare's Sonnets. It answers the question: "how far is arbitrary text X from 'Shakespeare' in each view?"

Current deployment uses this for guard scoring (E12×0.30 + E13×0.25 + E7×0.15 + E9×0.15 + E10×0.15 ≥ 0.45). This is a crude summary of the constellation's power. The constellation is fully usable as:
- Distance-weighted ranking function (replaces judge-LLM)
- Per-embedder contribution analysis (which view is violated?)
- Mahalanobis anchor (covariance-aware, not just cosine)
- Counterfactual target (what would a Shakespeare version of X look like in each view?)

---

## Part 2 — The 15 Novel Methods

Each entry: **idea**, **novelty vs. public literature**, **implementation sketch**, **fit for v4.2**.

### Method 1 — Multi-View Preference Pair Mining

**Idea**: From the 249k corpus, find triples (prompt, chosen, rejected) where E1 (meaning) distance(chosen, rejected) < ε, but E12+E13 (style) distance(chosen, rejected) > δ. These are pairs where content is equivalent but style differs — the exact training signal DPO wants.

**Novelty**: Public DPO mining uses (a) judge-LLM scoring ($$$, noisy, gameable), (b) rule-based heuristics (length, edit distance — poor proxies for style), or (c) on-policy sampling from the model itself (feedback loop, can reinforce artifacts). We use **ground-truth orthogonal views** to guarantee same-content-different-style.

**Implementation**:
```python
# Pseudo-pipeline, ~2 hours on existing HNSW indexes
for anchor_id in shakespeare_subset(n=5000):
    anchor = get_record(anchor_id)
    # E1 near-neighbors = same meaning
    candidates = hnsw_e1.search(anchor.e1, k=100, threshold=0.85)
    # Among candidates, filter for E12+E13 distance > δ = non-Shakespeare style
    for c in candidates:
        style_dist = cosine_dist(anchor.e12, c.e12) * 0.3 + cosine_dist(anchor.e13, c.e13) * 0.25
        if style_dist > 0.4 and content_dist(anchor, c) < 0.15:
            emit_preference_pair(anchor, chosen=anchor, rejected=c)
```

**Fit for v4.2**: **Critical**. Replaces v2.1's DPO pair source (Gemma-synthesized, expensive) with in-corpus mining.

### Method 2 — Constellation-Anchored Reward Function

**Idea**: For each generated sequence during DPO training, compute Mahalanobis distance to Sonnets constellation in each embedder view. Combined distance serves as the reward. No preference pairs needed per-se — generate k samples per prompt, rank by constellation distance, use ranks as implicit pairs.

**Novelty**: Public "reward model"-free DPO (ORPO, SimPO) eliminates the reference model but keeps heuristic rewards. We replace the heuristic with a **deterministic, covariance-aware, multi-view anchor** — the constellation. This is reward-hacking-resistant because gaming the score requires simultaneously moving in 13 embedder spaces, which is over-determined.

**Implementation**:
```python
def constellation_reward(text):
    vecs = embed_all_13(text)
    total = 0
    for view, weight in VIEW_WEIGHTS.items():
        diff = vecs[view] - CONSTELLATION_MEAN[view]
        mahal = diff.T @ CONSTELLATION_INV_COV[view] @ diff
        total += weight * math.exp(-mahal / 2.0)
    return total  # higher = more Shakespeare
```

**Fit for v4.2**: **Critical**. Eliminates the judge-LLM cost + bias in v2.1's DPO pair curation.

### Method 3 — Embedder-Gated Token Loss (Surgical SFT)

**Idea**: During SFT, for each generated token, compute 13-embedder divergence from Shakespeare prototype at that token position. Weight the token's cross-entropy loss by this divergence. Tokens that are already stylistically correct get near-zero gradient; off-style tokens get amplified gradient.

**Novelty**: Public techniques for focal loss / importance sampling use per-sample weights (ignoring within-sample variance). Token-importance-sampling (TIS-DPO, 2024) exists but uses prediction probability differences — a weak signal. Our signal is **explicit multi-view style divergence per token**, grounded in embedding distances, not model probabilities.

**Implementation**: Requires a lightweight embedder call per token during training (adds ~15% wall-clock). Mitigated by batching every 64 tokens. Loss becomes:
```python
per_token_weights = compute_style_divergence(gen_tokens, sonnets_tokenwise_proto)  # [seq_len]
weighted_loss = (ce_loss * per_token_weights).sum() / per_token_weights.sum()
```

**Fit for v4.2**: **Critical**. Directly addresses the "continue-training sharpened distribution too aggressively" failure mode from v4.1. EGTL only updates where updates are needed.

### Method 4 — Multi-View Agreement Guard (Runtime)

**Idea**: Replace the current single-score guard with an N-of-K agreement rule across multiple embedder views. Any generation must pass ≥4 of 5 subchecks:
- (a) E12 MaxSim to Sonnets ≥ threshold (style presence)
- (b) E13 SPLADE overlap with Shakespeare vocabulary ≥ threshold (lexical presence)
- (c) E2-E4 temporal signature entropy > threshold (not flat = not looping)
- (d) E5 causal edge diversity > threshold (not repeating same causal pattern)
- (e) E9 HDC bind-unbind cycle absence (no symbolic recurrence)

**Novelty**: Current SOTA (Llama Guard, NeMo Guardrails) uses single classifier scores + deterministic rules. No public guard combines per-view embedder agreement; they can't, because they don't have the 13-embedder infra.

**Implementation**: Each subcheck is ≤10ms given our indexes already built. Total guard latency <50ms. Drop-in replacement for current `check_guard()` in `shakespeare_chat_dashboard.py`.

**Fit for v4.2**: **Critical**. This catches the exact failure mode v4.1 exhibited (archaic-token loops passed single-score guard because tokens were individually archaic).

### Method 5 — Embedder-Aware Anti-Repetition Signal

**Idea**: During inference, stream the E2-E4 temporal embeddings of the generated sequence token-by-token. Repetition produces a **flat temporal signature** (same PE repeating). Detect flatness via rolling entropy; when it drops below threshold, inject a higher-temperature sample or force an EOS.

**Novelty**: DRY, no-repeat-ngram, and repetition penalty all operate on **surface tokens**. Our approach operates in **temporal embedding space**, which detects semantic repetition (paraphrased loops) that token-level detectors miss. E9 (HDC) adds symbolic-cycle detection: if bind(a,b) appears twice with same binding, it's repetition.

**Implementation**: 10ms hook in the generation loop. If triggered, retry with temp += 0.15 or insert <eos>. No retraining needed.

**Fit for v4.2**: **High**. Complements MVAG; catches mid-generation degeneration before the full response completes.

### Method 6 — Confidence-Weighted Corpus Selection

**Idea**: For each of 249k records, compute:
- Mean distance to Sonnets constellation (lower = better exemplar)
- Variance across 13 embedders (lower = more internally consistent signal)
- Outlier score vs. local neighbors (detect noise)

Build quality-filtered training subset where records are weighted by `1 / (constellation_dist * variance * outlier_score)`. Highest-quality records get 10× gradient contribution.

**Novelty**: Standard SFT treats all records equally. Coreset selection and curriculum learning exist but use model-loss-based scores. We use **orthogonal-embedder quality scores**, which are independent of the model being trained — no bootstrap bias.

**Implementation**: Precompute once (~30 min on 249k), then weighted sampler in training loop.

**Fit for v4.2**: **High**. v4.1 corpus had gap coverage issues; this filters for the best examples in existing corpus before augmentation.

### Method 7 — E5 Causal DAG Preservation Training

**Idea**: Extract causal DAGs from real Shakespeare passages using E5. Create training pairs: (modern prose with same DAG) → (Shakespeare rewording preserving DAG). This teaches the model to preserve causal structure, not just vocabulary.

**Novelty**: No public style-transfer dataset has explicit causal DAG preservation. Shakespeare's reasoning structure ("If thou lovest me, then deny thy father") is a signature as distinctive as vocabulary. Current public approaches treat "Shakespeare style" as lexical only.

**Implementation**: Use `causal_memory` to extract DAGs from 5,000 Shakespeare passages. Generate modern paraphrases preserving DAG via LLM (one-shot, cheap). Train SFT on these pairs.

**Fit for v4.2**: **Medium-High**. Addresses the quantum-entanglement-explanation class of prompts where Shakespeare style is valuable precisely because the reasoning is structured.

### Method 8 — E10 Cross-Lingual Style Bridging

**Idea**: E10 is asymmetric paraphrase (doc vs query). Compute E10 doc vectors for Spanish Golden Age passages (Cervantes, Calderón), Italian Trecento (Dante, Petrarch), Portuguese Classical (Camões). Match against E10 query vectors from English Shakespeare corpus to find semantically-equivalent period-literary pairs across languages. Use as training data: (Spanish query) → (Siglo de Oro response) trained alongside (English query) → (Shakespeare response).

**Novelty**: Cross-lingual style transfer papers (EACL 2026, ACL 2025) use multilingual embedders for translation-then-style. We use our asymmetric paraphrase embedder E10 to find period-equivalent pairs across languages — not translations, but **literary-register equivalents**. This is why the Sonnets→Siglo-de-Oro emergence happened by accident in v2.1; with E10 we can induce it deliberately.

**Implementation**: Mine ~1000 cross-lingual literary pairs via E10 HNSW. Add as training data, tagged with `<lang=es>` etc. Train a small E10-based cross-lingual style classifier as auxiliary loss.

**Fit for v4.2**: **Medium-High**. Reproduces the "cross-lingual emergence" non-deterministically claimed by v2.1 into a deliberate, testable capability.

### Method 9 — Entity-Preserving Style Transfer

**Idea**: E11 extracts entities. For each training record, extract entity list, hold entities fixed, generate multiple stylistic variants via LLM. Preference pairs: (entity-preserving Shakespeare) chosen over (entity-dropping Shakespeare) and (entity-correct modern) over (entity-confused Shakespeare).

**Novelty**: Public style-transfer literature treats entities as "part of content" and style as orthogonal. But faithful style transfer in practice requires entity fidelity (must say "OpenAI", not "merchant of knowledge"). Separating entity fidelity from style fidelity requires entity-aware labels we have via E11.

**Implementation**: ~200 entity-controlled pairs. Train SFT on (prompt with entities X, Y) → (Shakespeare response containing exactly X, Y). Use E11 entity overlap as correctness metric.

**Fit for v4.2**: **Medium**. Addresses the "Golang / OpenAI / quantum" category where v4.1 sometimes blurred entities.

### Method 10 — ColBERT MaxSim Token-Level Distillation

**Idea**: E12 is ColBERT. For every generated token, compute MaxSim against the Shakespeare corpus's ColBERT index. High MaxSim = stylistically fitting. Distill this score into the model as an auxiliary per-token loss during SFT.

**Novelty**: ColBERT was designed for retrieval. Using it for **style scoring with token-level attribution** is novel — it tells you *which* token in your generation is off-style, not just that the generation is off-style. No public repetition/style detector provides this.

**Implementation**: ColBERT index over Shakespeare corpus already exists. Add a per-token MaxSim call during SFT (~20ms per generation batch). Auxiliary loss λ·(1 - MaxSim_per_token).mean().

**Fit for v4.2**: **Medium**. Complements EGTL (Method 3) — EGTL uses 13-view ensemble, this is ColBERT-specific.

### Method 11 — SPLADE-Derived Constrained Decoder

**Idea**: Current constrained decoder hand-curates ~500 archaic tokens (+3.0 boost) and ~200 modern tokens (−8.0 suppress). Replace with **SPLADE-derived weights**: train E13 on Shakespeare vs modern corpus, extract per-vocab-token activation differences, use as boost/suppress weights directly.

**Novelty**: Public constrained decoders use LLM-generated vocab lists (Microsoft, etc.) or TF-IDF heuristics. SPLADE is trained to produce sparse activations that predict relevance — applying it to style gives principled, data-driven boosts that adapt as the corpus evolves.

**Implementation**: Compute SPLADE activations on 5k Shakespeare + 5k modern samples. Per-token: avg_activation_shakespeare - avg_activation_modern = logit adjustment. Add as constrained decoder dictionary (could be 5,000 tokens instead of 500).

**Fit for v4.2**: **Medium**. Improvement over current deterministic rule-based decoder; mainly a polish step.

### Method 12 — Counterfactual Preference Filter

**Idea**: For each candidate preference pair, run a counterfactual check via the causal_simulator agent: "if the prompt were modern English instead of Shakespeare-requesting, would the chosen response still be identical?" If yes, the pair is teaching "always produce Shakespeare" (bad — breaks context dependence). If the counterfactual diverges, the pair teaches context-sensitive style (good).

**Novelty**: Public DPO pair construction is correlational. Counterfactual filtering of pairs using an explicit causal-inference agent is new. Our causal_simulator already exists.

**Implementation**: ~5 min per 100 pairs via causal_simulator. Filter applied to Method 1 output.

**Fit for v4.2**: **Medium**. Quality filter; makes the model context-aware rather than globally-Shakespeare.

### Method 13 — Gap-Hunter-Driven Augmentation

**Idea**: Use `gap_hunter` and `graph_gardener` agents to identify structural holes in the 13-embedder space of existing training corpus. Example: "no records with E7 (code) high AND E12 (ColBERT-Shakespeare) high simultaneously" → gap between code and Shakespeare style. Generate ~50 records to fill each gap via v2.1 teacher + constellation verification.

**Novelty**: Public corpus augmentation uses either random diversification (temperature sampling) or task-targeted (e.g., "add 100 math examples"). Gap-hunter targets **geometric gaps** in latent space — rigorous coverage of the style manifold. Nobody else has the multi-view latent space to hunt gaps in.

**Implementation**: gap_hunter agent identifies 20-30 structural holes. Each hole → 30-50 generated records verified by constellation pass. Total: ~800-1500 new records.

**Fit for v4.2**: **High**. Addresses corpus gaps (116 foreign reclaimed + 50 structured + 40 math + ... but principled rather than per-category hand-designed).

### Method 14 — HDC (E9) Symbolic Anti-Loop

**Idea**: E9 is Holographic Distributed Codes. HDC supports bind/unbind operations that detect when the same symbolic structure is being regenerated. Add a runtime check: if hdc_bind(current_context, current_generation) produces a vector within ε of a previously-seen bind, the model is looping symbolically (not just lexically). Trigger a reset.

**Novelty**: HDC repetition detection is established in cognitive science (vector symbolic architectures) but **has never been applied to LLM generation guardrails**. Unique to our infrastructure.

**Implementation**: 1ms check per ~30 tokens. Threshold calibration requires ~200 pass/fail examples.

**Fit for v4.2**: **Medium**. Defense-in-depth complement to Method 5 (temporal anti-repetition).

### Method 15 — Multi-View Adversarial Discriminator

**Idea**: Train a tiny (2-layer MLP, ~10k params) discriminator on the 13-embedder features to classify "Shakespeare" vs "modern" vs "degenerate". Use discriminator confidence as an auxiliary reward during DPO/ORPO. Importantly: train discriminator on adversarial examples (v4.1 loops, v2.1 partial-style outputs, clean Shakespeare) so it recognizes failure modes as a separate class.

**Novelty**: Discriminator-based rewards in DPO (e.g., Adversarial DPO, 2024) use the main model's logits. Ours uses **orthogonal embedder features** → the discriminator can't be fooled by any adversarial manipulation that lives in the main model's token space.

**Implementation**: Train on ~3k labeled samples (1k each class). Use as auxiliary loss λ·CE(discriminator(embeds(gen)), target_class).

**Fit for v4.2**: **Medium**. Powerful but adds engineering complexity.

---

## Part 3 — The Top 3 Recipe (Recommended v4.2)

Deploy Methods 1 + 2 + 3 + 4 + 13 together. Here's the integrated pipeline, ordered.

### Phase 0 — Precomputation (20 min, one-time)

- [ ] Compute & cache for each of 249k records: constellation distance per view + variance + outlier score
- [ ] Generate SPLADE-based constrained decoder dictionary (~5k tokens)
- [ ] Export Sonnets ColBERT index for per-token distillation
- [ ] Verify all 13 HNSW indexes loaded

Output: `data/v4_2/corpus_quality_scores.parquet`, `data/v4_2/splade_decoder.json`

### Phase 1 — Gap-Hunter-Driven Corpus Curation (1.5 h)

- [ ] Run `gap_hunter` against existing 249k over Shakespeare cluster
- [ ] Identify 20-30 structural holes
- [ ] For each hole, generate 30-50 candidate records via v2.1 teacher
- [ ] Filter through constellation: only keep records with distance ≤ median Shakespeare distance
- [ ] Apply Method 12 counterfactual filter to new records

Output: `~1200 new high-quality records` added to corpus → 250,200 total

### Phase 2 — Preference Pair Mining (1 h)

- [ ] Run Method 1 mining: ~5000 pairs with E1-similar, E12/E13-different
- [ ] Run Method 12 counterfactual filter → keep ~3500 pairs
- [ ] Generate additional ~500 adversarial pairs (prompt-injection vs in-character refusal)
- [ ] Generate ~500 repetition-avoidance pairs (4 samples per prompt, rank by constellation + n-gram repetition; lowest-loop = chosen, highest = rejected)

Output: `data/v4_2/preference_pairs.jsonl` — 4500 pairs

### Phase 3 — SFT Phase with EGTL (3 h, RTX 5090)

Start from v2.1 adapter (not v4.1). Hyperparameters:
- LR 1e-5 (half of v4.1's 5e-5, matches the "sharper distribution" lesson)
- Cosine decay to 10%, 5% warmup
- 1 epoch over (curated_gap_records ∪ weighted_original_records) = ~4000 records
- EGTL per-token loss weighting active
- Confidence-weighted sampler (Method 6) active
- Auxiliary ColBERT distillation loss λ=0.1 (Method 10)
- Early-stop on validation constellation score + repetition metric

Output: `v4_2_sft/` adapter checkpoint

### Phase 4 — DPO with Constellation Reward (4 h, RTX 5090)

Using v4_2_sft as starting point:
- DPO loss + SFT regularization term (λ=0.1) — prevents forgetting (Reg-DPO)
- β = 0.1 (conservative, style-transfer tuned)
- LR 5e-6
- 1 epoch over 4500 preference pairs
- Length-normalized reward (Method 2 constellation scoring uses Mahalanobis which is scale-free)
- Auxiliary discriminator loss (Method 15) λ=0.05
- Per-epoch validation: browser FSV via Playwright on held-out 15-prompt battery

Output: `v4_2_dpo/` adapter checkpoint — **candidate v4.2**

### Phase 5 — Rep_Penalty + Max_Tokens Recalibration (30 min)

- [ ] Sweep rep_penalty ∈ {1.0, 1.1, 1.2, 1.3, 1.4} × max_tokens ∈ {256, 360, 480}
- [ ] Each combo: 50 diverse prompts, measure loop rate + constellation score
- [ ] Pick lowest rep_penalty × max_tokens where loop_rate = 0

Output: metadata fields in adapter config

### Phase 6 — Runtime Deployment (1 h)

- [ ] Replace dashboard guard with Method 4 Multi-View Agreement Guard (4-of-5)
- [ ] Add Method 5 temporal anti-repetition hook to inference server
- [ ] Add Method 14 HDC anti-loop check (optional, defense-in-depth)
- [ ] Replace constrained decoder with Method 11 SPLADE-derived dictionary
- [ ] Ship adapter with **explicit metadata**: training system prompt (canonical + 10 paraphrases), recommended rep_penalty, recommended max_tokens, expected constellation score range, FSV scores per category

### Phase 7 — Production-Stack Browser FSV (1.5 h)

- [ ] 15-prompt diverse battery (5 categories × 3 prompts: classical creative, code, adversarial, cross-lingual, modern topical)
- [ ] Run via Playwright through full dashboard stack, NOT harness
- [ ] Each prompt: 3 samples at different seeds; require ≥2/3 pass
- [ ] Gate: ≥13/15 prompts pass strict; ≥80% per category; 0/100 loops on general battery

Output: go/no-go decision on HF push

### Phase 8 — HF Publish + Cardd (30 min)

Only if Phase 7 passes. Model card includes **exact numbers** per category, no hype:
- "X/Y on [specific test harness]" for each category
- Recommended config pinned
- Failure modes explicitly documented
- v4.1 preserved in commit history

**Total wallclock: ~13-14 hours**. Compute: RTX 5090. Engineering: ~3 hours of script writing over existing infrastructure.

---

## Part 4 — Risk Register

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| R1: Constellation doesn't cover all domains (code, math, foreign) | Medium | High | Extend constellation with per-category sub-centroids. Compute separate Mahalanobis per category. |
| R2: EGTL slows training >30% | Medium | Medium | Batch embedder calls every 64 tokens. Fallback to every-16-tokens if too slow. |
| R3: Preference-pair counterfactual filter drops too many (<500 pairs) | Low-Medium | Medium | Relax E12/E13 distance threshold; re-run. |
| R4: DPO overpowers SFT (forgets v2.1's general capability) | Medium | High | Reg-DPO SFT-regularization term λ=0.1-0.2. KL vs SFT checkpoint. |
| R5: Discriminator (Method 15) overfits | Low | Low | Use only as auxiliary λ=0.05; train on ≥3k labeled samples. |
| R6: MVAG 4-of-5 rule too strict (rejects good outputs) | Medium | Medium | Calibrate thresholds on v2.1 outputs first; target ≥95% v2.1 acceptance. |
| R7: Gap-hunter generates low-quality records | Medium | Medium | Constellation filter is a hard gate; discard below threshold. |
| R8: RTX 5090 numerics drift between runs | Low | Low | Fix seeds; save multiple checkpoints; pick best by browser FSV. |
| R9: Base model updates (Gemma 4 v2) | Low | High | Pin base model commit hash in adapter_config. |
| R10: Cross-lingual E10 matching produces bad pairs (Method 8) | Medium | Low | Manually audit sample of 50; reject if <80% valid. |
| R11: v2.1 adapter starting point is already overfit to its corpus | Low | Medium | Consider starting from base Gemma with warm-start via v2.1 weights mixed 0.5. |
| R12: Constellation Mahalanobis inversion is unstable for high-dim views | Low | High | Use regularized inverse (λI + cov); fallback to diagonal covariance for sparse views (E6, E13). |
| R13: Browser FSV flaky due to Playwright timing | Medium | Low | 3 retries per prompt; majority vote. |
| R14: Token-level EGTL weights become degenerate (all tokens ~0 or ~1) | Low | Medium | Normalize per batch; clip to [0.1, 10.0]. |
| R15: Phase 3+4 combined runtime exceeds 12 hours due to validation loops | Medium | Low | Cache validation results; skip revalidation if no gradient step changed. |

---

## Part 5 — Acceptance Criteria

Must ALL pass for HF push:

- [ ] Browser FSV strict score ≥ 13/15 on 15-prompt diverse battery
- [ ] Per-category pass rate ≥ 80% in each of 5 categories
- [ ] Repetition: 0/100 generations trigger Method 5 or Method 14 loop detection on 100-prompt stress test
- [ ] Adversarial: ≥9/10 in-character refusals on prompt-injection battery
- [ ] Cross-lingual: ≥4/5 Spanish prompts return Siglo de Oro response across 5 seeds (reproducibility ≥ 80%)
- [ ] Code-gen: Output parses/compiles in ≥9/10 attempts with Shakespeare framing intact
- [ ] Structured output: JSON/SQL/YAML parses in ≥9/10 attempts
- [ ] MVAG false-positive rate ≤ 5% on v2.1 outputs (doesn't reject good v2.1 responses)
- [ ] Constellation distance of v4.2 outputs < constellation distance of v2.1 outputs on held-out prompts
- [ ] HF card contains exact numbers per category, no marketing language

---

## Part 6 — Why Top 3 (and Not All 15)

I picked Methods 1, 2, 3, 4, and 13 because:

- **1 + 2** give us the training signal (preference pairs + reward) without judge LLMs — this is the single biggest novel lever and the cheapest to implement.
- **3** directly addresses the v4.1 failure mode (distribution sharpening) with surgical token-level control — impossible in public literature.
- **4** is the runtime safety net that catches what training misses — unique to our 13-embedder stack.
- **13** ensures corpus coverage is principled (structural-hole-driven) rather than hand-designed category-by-category.

Methods 5, 7, 10, 11, 14 are **defense-in-depth additions** with low engineering cost; include in Phase 6 runtime.

Methods 6, 8, 9, 12, 15 are **power tools with more engineering overhead**. Deploy selectively:
- 8 (cross-lingual) if multilingual coverage is a requirement
- 9 (entity preservation) if entity-heavy prompts are common
- 15 (discriminator) if we see DPO-reward-hacking in Phase 4

---

## Part 7 — The 13 Failure Modes of v4.1, Addressed

| v4.1 Failure | Method(s) That Fix It |
|-------------|----------------------|
| 1. Skipped DPO | Phase 4 (DPO with Reg-DPO) — using Method 1+2 for pair mining and reward |
| 2. Continue-training sharpened distribution | Method 3 (EGTL) + LR 1e-5 + Reg-DPO |
| 3. Single system prompt in training | Phase 3 with 10 paraphrased prompts rotated per record |
| 4. Rep penalty baked-in at 1.2 | Phase 5 recalibration + Method 5 temporal anti-repetition |
| 5. 116 foreign-language records rejected | fastText lid.176 filter in Phase 1; Method 8 for deliberate cross-lingual |
| 6. Near-zero structured output | Method 13 gap-hunter finds this hole; Phase 1 fills it |
| 7. Math/equations 0.33% | Same — Method 13 treats it as one structural hole |
| 8. Insufficient adversarial training | 500 adversarial preference pairs in Phase 2 via Method 1 |
| 9. FSV didn't match production path | Phase 7 uses dashboard stack via Playwright |
| 10. Style-score guard counts tokens not coherence | Method 4 (MVAG) replaces it |
| 11. Max_tokens mismatch | Phase 5 calibration + shipped as metadata |
| 12. Overclaim in reporting | Acceptance criteria in Part 5 force exact-number reporting |
| 13. Shipped before production-stack FSV | Phase 7 is a hard gate before any HF push |

---

## Part 8 — Comparison to Public State of the Art

| Aspect | Public SOTA (2026) | Our v4.2 (proposed) | Advantage |
|--------|-------------------|---------------------|-----------|
| Preference signal | Judge LLM (GPT-4 / Claude) | Constellation + 13-view Mahalanobis | Deterministic, no API cost, no judge bias |
| Pair mining | Heuristic (length, edit distance) or judge-scored | Multi-view content-equivalence search | Precise same-content-different-style pairs |
| Repetition mitigation | DRY sampler, rep penalty, no-repeat-ngram | Temporal-entropy + HDC-cycle + MVAG | Semantic repetition, not just surface |
| Token-level loss weighting | TIS-DPO (probability-based) | EGTL (13-view-divergence-based) | Grounded in labels, not model probs |
| Runtime guard | LlamaGuard + NeMo + regex | Multi-View Agreement Guard (4-of-5) | Covers loops, drift, entity errors simultaneously |
| Cross-lingual style | Translate-then-stylize | E10 asymmetric paraphrase match | Period-literary equivalents, not translations |
| Constrained decoder | Hand-curated lists | SPLADE-derived (Method 11) | 10× larger dict, data-driven weights |
| Evaluation | Static benchmarks + LLM-as-judge | Production-stack Playwright battery | End-to-end truth |

---

## Part 9 — What I Would Not Do

Techniques that I researched but rejected for v4.2:

- **Raw DoLa / contrastive decoding**: adds latency without clear style benefit; our E12 MaxSim per-token is a better use of compute.
- **KTO**: designed for binary thumbs-up/down data; we have paired data, DPO/ORPO are strictly more informative.
- **SimPO**: length-normalized reward is poorly suited to metered verse. Mentioned by the research agent as a specific warning for our domain.
- **Full-rank fine-tuning**: LoRA rank 256 α 512 on Gemma-4 E4B has already been validated; no benefit from full-rank.
- **Online RL (PPO / GRPO)**: unnecessary complexity given our deterministic reward; DPO-class is sufficient.
- **Merging v2.1 + v4.1 adapters**: model-soup literature (LoRA Soups 2024) shows parameter interference hurts performance without careful directional merging. v4.2 should be a clean continue-train, not a merge.
- **DoRA (Weight-Decomposed LoRA)**: marginal gain reported; not worth the implementation complexity for this run.

---

## Part 10 — Open Research Questions (Post-v4.2)

If v4.2 succeeds, these become interesting next-steps:

1. **Can constellation covariance be learned per-prompt?** I.e., for different request types (code vs poetry), different embedder-weights matter. A per-prompt constellation gating could be trained.
2. **Can EGTL token weights be self-improving?** Iterative: train v4.2, generate from v4.2, use its token-level errors to weight v4.3 training.
3. **Can the 13-embedder space be distilled into a single smaller "style field"?** For faster inference-time scoring.
4. **Does MVAG generalize to other persona/style domains?** (medical voice, legal voice, child-friendly voice)
5. **Is there a closed-form solution for optimal constellation weight distribution given a target FSV battery?** (meta-optimization of VIEW_WEIGHTS)

---

## Appendix A — Key References

Academic:
- Rafailov et al. "DPO" (2023)
- Hong et al. "ORPO" (2024)
- Azar et al. "IPO" (2024)
- Meng et al. "SimPO" (2024)
- Park et al. "Disentangling Length from Quality in DPO" (2024)
- Ethayarajh et al. "KTO" (2024)
- Chuang et al. "DoLa" (2023)
- Meister et al. "Typical Sampling" (2023)
- Nguyen et al. "Min-P Sampling" (2024)
- Chen et al. "SecAlign" (2024, extended 2025)
- Hong et al. "Length Desensitization in DPO (LD-DPO)" (2024)
- Reg-DPO (2025) — SFT-regularized DPO

Infrastructure in this project:
- Sonnets constellation: `18ef2460-9ab6-44fc-8d7e-8d7513174ac8`
- 13-embedder specs: `docs/SHAKESPEARE_LORA_TRAINING_SPEC.md`
- v2.1 adapter: `~/.contextgraph/models/shakespeare_lora/final`
- v4.1 adapter (research, commit-pinned): `~/.contextgraph/models/shakespeare_lora_v4_1/final`
- HF repo: `cabdru/shakespeare-lora-gemma4`
- SITREPs: `tmp/SITREP_2026-04-21_151147.md`, `tmp/SITREP_2026-04-21_191725.md`

---

## Appendix B — Glossary

| Term | Definition |
|------|------------|
| Constellation | Pre-computed centroid + covariance in 13-embedder space representing a style anchor (e.g., Sonnets) |
| CAMD-DPO | Constellation-Anchored Multi-View DPO (Method 1 + 2 combined) |
| EGTL | Embedder-Gated Token Loss (Method 3) |
| MVAG | Multi-View Agreement Guard (Method 4) |
| Reg-DPO | DPO with SFT loss as regularization term (SFT-DPO hybrid) |
| HDC | Holographic Distributed Codes (E9), supports symbolic bind/unbind |
| ColBERT MaxSim | Per-token late-interaction score (E12) |
| SPLADE | Sparse lexical expansion (E13) |
| Kepler | Entity-aware embedder (E11) |
| Counterfactual filter | Reject preference pairs whose chosen response would be identical under a modernized prompt |

---

*End of document. Decision needed from user: execute Top 3 (Methods 1, 2, 3, 4, 13) as described in Part 3, or revise scope?*
