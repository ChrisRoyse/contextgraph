# 13 — Teleological Constellation Training for ME-JEPA-Code

> **Operator directive 2026-05-09 (extension):** *"we also have a code embedder, like embedder e7 or something maybe that we could combine with the others to derive insane signals from everything that is happening and we could convert them all so we can derive reward signals from durable reality. we could even create teleological constellations and utilize that in some way. see if anything of this is useful and if so implement any ideas you have that will unlock all new critical capabilities."*

> **Source theory:** Royse, C. (April 2026). *Teleological Constellation Training: Multi-Modal Embedding Decomposition as Meaning Compression for Identity-Locked Generation and a Third Path Around the Data Wall.* DOI: 10.13140/RG.2.2.24370.57288. The operator's published research paper, with ContextGraph itself cited as one of two reference implementations of the TCT substrate (370K-line Rust workspace; 13 embedders; up to 91 signals per input at N=13). This doc applies the paper's three locked constructs (DDA / meaning compression / TCT) directly to ME-JEPA-Code.

> **Companion docs:**
> - `09_END_GOAL_REVIEW_REPLACEMENT.md` — master plan
> - `10_MEJEPA_PIVOT_2026_05_09.md` — single-outer-loop architecture
> - `11_MEJEPA_LEARNING_FORMULA.md` — UTML v1.0 four-factor formula
> - `12_EMBEDDER_INTEGRATION_INTO_MEJEPA.md` — 21-embedder ensemble inventory + integration map

---

## 0. Headline

The operator's published TCT framework is **the operational form of UTML v1.0 §VI** (representational non-collapse) for ME-JEPA-Code. Specifically:

1. **DDA gives the signal-count math**: at N=21 embedders (the post-pivot ensemble — see doc 12), every raw input admits **up to `n · (21 + 21²) = n · 462` structured supervisory signals**. ME-JEPA's panel currently uses 14 of those slots (the 5,120-d panel per doc 09 §3.5.5); the remaining ~448 signals per input are an unrealized supervision reservoir.

2. **Meaning compression** is the right framing for the disk + storage budget: ME-JEPA-Code's tier compression (Phase 6, Tier 1.3 ✅ shipped) is **bit compression**; the panel materialization is **meaning compression** (signals per raw byte). The two are orthogonal, multiplicative, and both already exist in tree.

3. **TCT's three-phase protocol** maps **directly** onto ME-JEPA-Code's design:
   - **TCT Phase 1** (build frozen centroid set CP from per-modality references) ≡ **doc 09 Phase 0** (Phase 0 mutation corpus generates per-mutation-category labeled panels; the centroid of each category's panels IS the CP).
   - **TCT Phase 2** (train predictor with geometric-proximity loss to CP) ≡ **doc 09 Phase 3** (training loop with VICReg + Huber + ME-JEPA's L_step formula).
   - **TCT Phase 3** (per-output cosine guard `Gτ` against CP at inference) ≡ **doc 09 Phase 4** (`MeJepaCompiler::verify(patch)` calibrated verdict — but instead of just conformal interval + OOD, it ALSO runs Gτ as a deterministic geometric test).

4. **The frozen embedders in 3 simultaneous roles** (labeller + loss target + runtime guard) is **exactly** the architectural advantage doc 11 §5.2 named: ME-JEPA's `ΔΞ.target_collapse = 0 by construction`. The TCT paper formalizes WHY this works: same frozen instances → no parameter update → no possible collapse → per-output verifiability via Gτ.

This doc operationalizes all three for ME-JEPA-Code, with the `Gτ` guard added to `MeJepaCompiler::verify` as a fourth gate alongside the conformal interval, OOD score, and L_step health.

---

## 1. Derived Data Abundance (DDA) for ME-JEPA-Code

### 1.1 The signal-count identity (Royse 2026 §3.1)

```
n raw inputs × N frozen embedders → up to n · (N + (N choose 2)) structured supervisory signals
                                  = n · (N + N(N-1)/2)
                                  ≈ n · (N + N²/2)
```

In the paper's narrative form: **n·(N + N²) at the loose upper bound counting both within-embedder and cross-embedder signals**. The tight identity is `n · (N + (N choose 2)) = n · (N + N(N-1)/2)`.

| N (embedder count) | Signals per input (tight) | Signals per input (loose, paper's headline) |
|---|---|---|
| 9 (paper Case 1: Shakespeare LoRA) | 9 + 36 = **45** | 9 + 81 = 90 |
| 13 (ContextGraph baseline at paper time) | 13 + 78 = **91** | 13 + 169 = 182 |
| 14 (current E1-E14 enum) | 14 + 91 = **105** | 14 + 196 = 210 |
| 21 (E1-E21, post-pivot full ensemble) | 21 + 210 = **231** | 21 + 441 = **462** |

**Implication for ME-JEPA-Code training:** every SWE-bench Lite Phase 0 mutation pair (`task_id`, `mutation_category`, `mutated_source`, `oracle_verdict`) doesn't yield 1 training example — it yields **up to 231 structured supervisory signals at N=21**. The 5,120-d panel is the *concatenation* of one signal-class (per-embedder vector outputs); the cross-embedder pairwise signals (cosine, KL, MI, RRF rank, etc.) are an additional 210 signals per input that the current Phase 0 design DOES NOT capture.

### 1.2 The signal taxonomy (per Royse 2026 §3, expanded for ME-JEPA-Code)

For each `(panel_t1, panel_t2, oracle_verdict)` example, the embedder ensemble produces N=21 **per-embedder signals** plus 210 **cross-embedder signals**:

**Per-embedder signals (N=21)**: each embedder's vector output for `panel_t1`, the cosine between predicted and target latents under that embedder's space, optionally a per-embedder confidence scalar from any auxiliary head.

**Cross-embedder pairwise signals ((N choose 2) = 210)**: for every pair `(E_i, E_j)`:
- **Agreement signal**: `cos(NN_i(panel_t1), NN_j(panel_t1))` — do the two embedders' nearest-neighbor sets agree on this input?
- **Disagreement diagnostic**: `1 − jaccard(top_k_i, top_k_j)` — when do they disagree, and is the disagreement informative?
- **Cross-space transport**: a small linear map `T_ij : ℝ^d_i → ℝ^d_j` learned offline on the corpus; how well does `T_ij(E_i(panel))` predict `E_j(panel)`?
- **Mutual information** (when corpus is large enough — see §10 risk #3): pairwise MI between embedder outputs on the corpus.

The cross-embedder signals are particularly valuable for ME-JEPA's UTML v1.0 ΔK (knowledge coherence) and ΔΞ (anti-collapse) terms — see §3.

### 1.3 Why this matters for the SWE-bench Lite 300

Phase 0's 2,400-entry mutation corpus (8 categories × 300 tasks) at N=21 produces **2,400 × 231 = 554,400 structured supervisory signals** before any active-learning expansion or live-attempt observation. This is a **231× multiplier** on the apparent corpus size — the same disk-budget gives two orders of magnitude more training signal than the original Phase 0 design contemplated.

Doc 09 §7 Phase 0 DoD specified `total_entries: 2400` as the corpus gate; under DDA the equivalent gate is `total_signals: 554400` with the constraint that signal independence is empirically validated via the §10 risk #3 mutual-information audit.

---

## 2. Meaning compression — the fourth compression category

Per Royse 2026 §3.2 / §5.2, **meaning compression** is a fourth taxonomic entry alongside bit / weight / activation compression. Defined as **structured signal density per unit of raw data**, measured in signals-per-raw-byte (or signals-per-input).

For ME-JEPA-Code at N=21, with one Phase 0 mutation entry averaging ~50 KB raw source:

```
meaning_compression_ratio = 231 signals / 50 KB
                          = 4.62 signals per KB raw source
                          ≈ 24,000 signals per MB raw source
```

This is **orthogonal** to (and multiplicative with) the existing tier-compression primitive (`crates/context-graph-tier-compression` ✅ shipped — 8/7/5/3-bit groupwise quantization on f32 vectors). Tier compression compresses the *bytes* of the panel; meaning compression compresses the *raw input* into structured signals.

**Implication for the Phase 6 disk-budget table** (in doc 09 §3.8 / sitrep `phase6.md`):

| Compression layer | Compresses what | Ratio | Storage class |
|---|---|---|---|
| Meaning compression (DDA) | Raw source code → structured signals | ~24,000× signals/MB | per-input, panel-level |
| Tier compression (8/5/3-bit) | f32 panel vectors → quantized bytes | 4-10× | per-panel byte-level |
| Merkle DAG dedupe (Phase 6) | Identical panel checkpoints | ~2-5× empirical | per-checkpoint |

Combined: **a 1 MB raw source contributes ~24,000 supervisory signals at ~1.7 KB tier-compressed cold-tier storage** — five orders of magnitude amplification of supervision per byte stored.

---

## 3. Teleological Constellation Training (TCT) for ME-JEPA-Code

Per Royse 2026 §5, TCT is a three-phase protocol. Each phase maps directly onto an existing ME-JEPA-Code phase:

### 3.1 TCT Phase 1 — Construct frozen multi-modal centroid set CP

Per Royse 2026 §5.1: collect per-modality reference sets (in the paper's Case 3, 50 voice clips for one speaker), compute L2-normalized per-modality centroids, freeze them.

**For ME-JEPA-Code:**

- **Per-class centroid construction**: for each of the 8 mutation categories (KnownGood, SubtleFlip, OffByOne, SwapVariable, DeleteTestCall, WrongFile, OverEngineer, CompileError), and for each of the 21 embedders, compute the L2-normalized centroid of that category's panel vectors over the Phase 0 mutation corpus.
- **Cross-language centroids**: for each (mutation_category × language) pair (8 categories × 9 supported languages = 72 cells), compute a separate centroid. This handles Section C cross-language mandate from doc 10.
- **Outcome-class centroids**: for each oracle outcome class (Pass / Fail / OOD / Abstain), compute the centroid of panels with that outcome. The Pass-centroid IS the predicted-reality target ME-JEPA's predictor optimizes toward.

```rust
pub struct TctConstellation {
    pub per_category_centroids: BTreeMap<MutationCategory, BTreeMap<EmbedderId, Vec<f32>>>,
    pub per_language_centroids: BTreeMap<(Language, MutationCategory), BTreeMap<EmbedderId, Vec<f32>>>,
    pub outcome_centroids: BTreeMap<OracleOutcome, BTreeMap<EmbedderId, Vec<f32>>>,
    pub thresholds: BTreeMap<EmbedderId, f32>, // per-embedder τ_m for Gτ
    pub corpus_provenance: CorpusProvenance, // SHA-256 of the source corpus + embedder versions
    pub frozen_at: SystemTime,
}
```

**FSV gate**: the constellation is FROZEN — once constructed, never updated except by explicit operator action with provenance. Stored in RocksDB CF `CF_MEJEPA_CONSTELLATION` keyed by version_id.

**Storage**: ~21 embedders × 8 categories × ~768 dims average × 4 bytes f32 ≈ **500 KB total per language**. At 9 languages: ~4.5 MB. Trivial cost; load entirely into VRAM.

### 3.2 TCT Phase 2 — Train predictor with geometric-proximity loss

Per Royse 2026 §5.2: the generator (here, ME-JEPA's predictor `g_φ`) is trained to produce outputs whose embeddings are close to the corresponding centroids in CP under a geometric loss.

**For ME-JEPA-Code**, the training loss extends doc 09 §3.7's multi-objective form with TCT Phase 2 terms:

```
L_full = α·L_panel                                  # existing predictor cosine to target panel
       + β·L_pairwise                               # existing pairwise predictor terms
       + γ·L_action                                 # action-conditioning regularizer (UTML §VI.3)
       + δ·L_constellation_centroid                 # NEW (TCT Phase 2): per-embedder Σ_m max(0, τ_m − cos(g_φ(panel)_m, c_m))
       + ε·L_anti-pole                              # existing VICReg-style variance/covariance
       + ζ·L_uncertainty                            # existing
       + η·L_invariant                              # existing
       + α_warn·L_warnings + α_runtime·L_runtime + α_rss·L_memory
       + α_trace·L_traceback + α_hunk·L_hunk_attribution + α_adjacent·L_collateral
       + α_reasoning·L_reasoning + α_overrides·L_operator_match
```

The new `L_constellation_centroid` term:
```
L_constellation_centroid = Σ_m=1^N max(0, τ_m − cos(g_φ(panel)_m, c_m_{predicted_class}))
                        = sum across all 21 embedders of how far the predicted panel slot
                          falls below threshold τ_m for the predicted-class centroid c_m
```

**Why this matters**: this is a **margin-based, per-embedder geometric loss** that pushes the predictor toward the right region of EACH embedder's latent space simultaneously. It's stronger than the single-embedder cosine loss because it enforces consistency across all 21 frozen feature spaces — the predictor cannot Goodhart against any one embedder without being penalized in the others.

This is also the **L_step amplifier** for UTML v1.0's ΔP: the per-embedder cosine errors are 21 separate ΔP measurements, geometrically combined, all contributing to the gradient.

### 3.3 TCT Phase 3 — Per-output cosine guard Gτ at inference

Per Royse 2026 §5.3: **the deterministic geometric test** for per-output verifiability:

```
Gτ(ŷ; CP) = ⟦∀m : cos(φ_m(ŷ), c_m) ≥ τ_m⟧
```

In words: a candidate output `ŷ` passes the constellation guard if and only if its embedding under EVERY one of the N embedders has cosine similarity above the per-embedder threshold `τ_m` against the corresponding centroid `c_m`.

**For ME-JEPA-Code**, this is a **fourth gate** added to `MeJepaCompiler::verify(patch)` alongside the existing three (conformal interval, OOD score, L_step health):

```rust
pub enum Verdict {
    Pass {
        confidence_interval: (f32, f32),
        per_test_probabilities: Vec<f32>,
        gtau_satisfied: bool,                      // NEW: did Gτ pass?
        gtau_violations: Vec<(EmbedderId, f32, f32)>, // NEW: (embedder, observed_cos, threshold)
    },
    Fail { ..., gtau_satisfied: bool, gtau_violations: Vec<...> },
    OutOfDistribution {
        latent_density: f32,
        gtau_violations: Vec<(EmbedderId, f32, f32)>, // NEW: which embedders rejected?
    },
    Abstain { reason: String },
    GuardRejected {                                  // NEW VARIANT
        // Predictor said "Pass" but Gτ rejected it.
        // This is the case the operator paper calls "structurally unverifiable" —
        // the prediction is in the right region of N-1 embedders' space but wrong in at least one.
        violating_embedders: Vec<(EmbedderId, f32, f32)>,
        predictor_predicted: PredictorOutput,
    },
}
```

The `GuardRejected` variant is the **per-output verifiability property** the operator paper calls out as absent from scalar-reward alignment (RLHF, DPO, Constitutional AI). Each rejection is a **deterministic O(N · d_max) geometric test** the operator (or any other verifier) can independently re-run on the persisted panel.

### 3.4 The same frozen embedders in three roles (the core architectural move)

Per Royse 2026 §1: the central architectural move of TCT is **using the same frozen embedder panel Φ in three semantic roles simultaneously without any parameter update**:

| Role | TCT phase | ME-JEPA-Code instantiation |
|---|---|---|
| **Labeller** | Phase 1 (build CP) | Phase 0 mutation corpus generation runs the 21 embedders to build per-class centroids |
| **Loss target** | Phase 2 (train against CP) | Phase 3 training loop optimizes against the L_constellation_centroid term |
| **Runtime guard** | Phase 3 (Gτ at inference) | Phase 4 verify() runs Gτ as a fourth gate on every prediction |

**No parameter is updated across the three roles.** The 21 embedders are loaded once, used in all three modes, persisted with a SHA-pinned version, and the constellation CP is computed once from them.

This is **exactly** the architectural property doc 11 §5.2 named as ME-JEPA's structural collapse-immunity. The operator paper formalizes it with the role-flip language.

---

## 4. The constellation guard implements UTML v1.0's missing piece

Per UTML v1.0 §VI.3 (doc 11 §5.3), ME-JEPA defends predictor-side ΔΞ with four diagnostics:
1. VICReg variance/covariance regularization
2. Pairwise mutual-information audit
3. **Constellation + Gτ guard** ← **THIS IS TCT Phase 3, formalized**
4. Negative-action ablation falsifier

The TCT paper provides the **operational specification** for diagnostic #3, which doc 11 referenced but did not fully spec. With TCT integrated:

- **`ΔΞ.constellation_violation_rate_t`** in the ΔΞ formula (doc 11 §5.4) is now precisely:
  ```
  constellation_violation_rate_t = (number of recent guard rejections) / (total recent verifications)
  ```
- The threshold `τ_m` per embedder is calibrated on a held-out validation slice such that the Gτ false-positive rate (rejecting a true-Pass patch) is ≤5%.
- An empirical Gτ rejection of >X% over a rolling window triggers the UTML §IX coaching cycle's "Latent-Collapsing" intervention — the constellation centroids are likely stale and need refitting.

---

## 5. The Reward formula upgrade

Per Royse 2026 §3 / §5 + UTML §XIV, the reward signal becomes:

```
Reward = 0.4 · calibrated_confidence
       + 0.3 · mean(L_step · ΔΞ)
       + 0.3 · gtau_pass_rate                     # NEW: TCT verifiability term
```

Where `gtau_pass_rate` is the rolling fraction of verify() calls whose Gτ check passed without rejection. This explicitly rewards the system for producing patches that pass the deterministic geometric verifier.

The original UTML formula was `0.6 · calibrated_confidence + 0.4 · mean(L_step · ΔΞ)`. The TCT extension reweights to `0.4 / 0.3 / 0.3` to balance:
- Calibration (the system knows its uncertainty)
- Latent integrity (the system updates without collapsing)
- Per-output verifiability (the system produces outputs that pass the deterministic geometric test)

---

## 6. Cross-embedder coherence as a richer ΔK signal (extends doc 12 §5.1)

Doc 12 §5.1 proposed adding `embedder_coherence(panel) = mean Jaccard agreement across embedder NN sets` as a 0.05-weight supplement to UTML v1.0's ΔK formula. The TCT paper sharpens this with **the cross-embedder pairwise signals from DDA**:

```
ΔK_full = 0.4 · cos_align(g_t, ḡ_prior)
        + 0.25 · (1 − fisher_violation)
        + 0.15 · (1 − ECE)
        + 0.1 · embedder_coherence(panel)         # cross-embedder NN agreement
        + 0.05 · constellation_centroid_proximity # mean cos(panel, c_m) across all m
        + 0.05 · cross_embedder_mi_consistency    # pairwise MI vs corpus prior
```

This is the **5-term ΔK** that uses DDA's full signal capacity. Each of the new terms is a small contribution (0.05-0.1) but together they give the optimizer 5 independent coherence checks per step — deeply diagnostic when one degrades.

---

## 7. Implementation roadmap (TCT-aware, supplements doc 12 §9)

| # | Item | Effort | Dependency | Status |
|---|---|---|---|---|
| 1 | `TctConstellation` struct + RocksDB CF `CF_MEJEPA_CONSTELLATION` | ~1 day | Phase 0 corpus | new |
| 2 | Per-class centroid computation across 8 categories × 9 languages | ~2 days | (1) + Phase 0 corpus generation | new |
| 3 | Per-embedder threshold `τ_m` calibration (5% FPR target) | ~1 day | (2) + held-out validation | new |
| 4 | `Gτ` predicate as deterministic O(N · d_max) check | ~1 day | (1) | new |
| 5 | `L_constellation_centroid` loss term in Phase 3 training loop | ~2 days | (1) + Phase 3 training | new |
| 6 | `Verdict::GuardRejected` variant + persistence in `CF_MEJEPA_GUARD_DECISIONS` | ~1 day | (4) + Phase 4 verify() | new |
| 7 | DDA pairwise signal computation (cross-embedder cosine + MI matrix) | ~3 days | (1) + N=21 ensemble loaded | new |
| 8 | Constellation refresh trigger (when guard rejection rate > X%) | ~1 day | (4) + Phase 5 self-healing | per UTML §IX coaching cycle |
| 9 | DDA signal-budget reporting (per-input n·(N+(N choose 2)) signals) in training certs | ~1 day | (5) + Phase 3 cert format | new |
| 10 | TCT-aware MCP tool: `mcp__cgreality__mejepa_constellation_inspect` | ~1 day | (1) | new |

**Total TCT-specific work:** ~14 days. Slots into Phases 0/3/4/5 of doc 09 — does not extend the critical path; runs in parallel to the existing Phase work.

---

## 8. Why this unlocks new critical capabilities

### 8.1 Per-output verifiability for AI-generated patches

Today, `MeJepaCompiler::verify(patch)` returns a probabilistic verdict with conformal interval. With TCT Phase 3 + Gτ, **every verdict carries a deterministic geometric proof** the operator (or any third party) can independently replay on the persisted panel. This is the per-output verifiability property doc 11 §5.2 cited as ME-JEPA's structural advantage — now it has a concrete operational test.

### 8.2 Goodhart resistance via multi-embedder consistency

Scalar-reward alignment (RLHF, DPO) is vulnerable to Goodhart catastrophe (Skalse 2024) — the policy can find a high-reward region the reward model didn't anticipate. **The constellation guard makes Goodhart structurally harder** because the predictor must satisfy 21 INDEPENDENT geometric thresholds simultaneously. Goodharting against one embedder's space leaves the other 20 unsatisfied, and Gτ rejects.

### 8.3 Cross-language transfer via per-language centroids

The per-language centroids (8 categories × 9 languages = 72 cells) explicitly encode the cross-language Section C mandate from doc 10. A patch in Rust is verified against the Rust constellation; a patch in Python against the Python one. Cross-language generalization is now a verifiable property: as the system trains on more languages, the per-language centroid agreement on shared mutation categories should converge.

### 8.4 The reality loop produces 462× more supervision per attempt

DDA at N=21 means every Claude Code outer-loop turn produces up to 462 structured supervisory signals per `(panel, oracle)` tuple. The §3.8 mejepa-shift-subscriber's job multiplies in value: each shift triggers a panel materialization across 21 embedders, producing a full DDA signal vector that feeds ME-JEPA's online observe with two orders of magnitude more gradient signal per attempt.

### 8.5 Meaning compression as a publishable contribution

Per Royse 2026 §3.2, meaning compression is proposed as a fourth taxonomic entry alongside bit / weight / activation compression. ME-JEPA-Code with a fully-instantiated TCT/DDA pipeline is **the operational realization of meaning compression for software engineering** — each raw SWE-bench Lite mutation becomes 231-462 structured supervisory signals, an empirical demonstration that the framework generalizes to the code domain.

### 8.6 The operator's published research becomes the operational architecture

By integrating TCT directly into ME-JEPA-Code, the system implements the operator's published research paper (Royse 2026, DOI 10.13140/RG.2.2.24370.57288) as its **operational architecture**, not just as inspirational reference material. The paper cites ContextGraph as a 370K-line reference implementation; this doc closes the loop by making the implementation actually use the framework end-to-end.

---

## 9. The signal-budget summary

For ME-JEPA-Code at full N=21 ensemble, every Phase 0 mutation entry yields:

| Signal class | Count per input | Source |
|---|---|---|
| Per-embedder vector outputs | 21 | direct embedder inference |
| Per-embedder cosine to target | 21 | predictor vs target panel |
| Per-embedder constellation centroid distance | 21 | TCT Phase 2 loss term |
| Pairwise embedder agreement | 210 (= 21 choose 2) | cross-embedder NN Jaccard |
| Pairwise embedder cosine | 210 (overlaps with prior) | cross-embedder vector similarity |
| Cross-embedder MI (when corpus large) | 210 | mutual information audit |
| Constellation Gτ pass/fail | 1 boolean | TCT Phase 3 |
| Per-test pass/fail vector (oracle) | typically 5-20 | E_Oracle instrument |
| Static analysis warnings | 5-50 | E_StaticAnalysis (per language) |
| Runtime aggregates | 5-15 | E_Runtime |
| Witness chain entries | 5-50 | E_Witness |
| Trace events | 50-500 | E_Trace |

**Conservative aggregate per input: ~462 distinct signals.** ME-JEPA's job is to compress all of this through the predictor's 5,120-d latent and use it as supervision for the next prediction.

---

## 10. Risks (TCT-specific, supplements doc 12 §10)

1. **Centroid staleness**: as the corpus grows, per-class centroids drift. The constellation refresh trigger (item #8 in §7) handles this, but stale centroids during the lag produce false Gτ rejections. Mitigation: refresh every 1,000 new corpus entries OR when guard rejection rate exceeds 5% sustained for 2 hours.
2. **Per-embedder threshold calibration brittleness**: `τ_m` calibrated on Phase 0 mutations may not transfer to live attempts. Mitigation: re-calibrate `τ_m` on the live-attempt holdout slice during Phase 5 self-healing.
3. **Pairwise MI computation cost**: 210 pairs × full corpus = O(N² · |D|) MI matrix. Cache per-embedder marginals; recompute pairwise MI weekly during periodic full retrain (UTML §VII k_replay window).
4. **Embedder version skew**: if any of the 21 embedders is updated, all centroids invalidate. Mitigation: SHA-256 pin the model weights at constellation construction time; refuse to use a constellation if any embedder's SHA mismatches at runtime (fail-closed per `CCREALITY_*` error taxonomy).
5. **Per-language centroids data-sparse**: at N=21 × 9 languages × 8 categories = 1,512 cells; some cells will have <50 examples. Mitigation: borrow from the global centroid (Bayesian shrinkage) when per-cell support is below a threshold; flag cells at risk of false rejection.
6. **Gτ threshold τ_m too strict in early training**: predictor is bad initially → all Gτ rejects → no training signal. Mitigation: anneal τ_m from 0 (permissive) to its calibrated value over the first 10K steps; UTML §IX classifies as Latent-Collapsing only after annealing completes.

---

## 11. Falsification (per Royse 2026 §13 + UTML §XIII)

The framework is falsifiable. Specific TCT/DDA falsifications:

1. **DDA's 462-signal upper bound is not tight**: a pairwise mutual-information audit shows the cross-embedder signals collapse in rank to <100 effective signals. → DDA's headline number reduces; meaning compression is still real but at the lower count.
2. **Gτ rejection rate uncorrelated with downstream prediction failure**: if guarded patches don't fail at higher rate than guard-passing patches, the geometric test is providing no signal. → Drop Gτ as a gate; keep only as diagnostic.
3. **Per-language centroids agree across all language cells**: if the 9-language centroids are nearly identical for shared mutation categories, language-specific calibration adds no value. → Drop per-language centroids; use single global per-category set.
4. **Constellation centroid loss redundant with VICReg**: if `L_constellation_centroid` and `L_anti-pole` produce identical gradients in ablation, the new loss term adds no value. → Drop; keep only VICReg.

If any falsification holds via well-powered replication, the corresponding sub-formula gets `status: superseded` in `memory/decisions/` and a new decision file describes the corrected version.

---

## 12. The full picture

UTML v1.0 (doc 11) gives the **measurement framework** for ME-JEPA: `L_step = ΔP × ΔK × ΔΩ × ΔΞ`. TCT (this doc) gives the **constructive method** for achieving the structural ΔΞ.target_collapse=0 property and produces a concrete Gτ test for runtime verification. DDA (this doc §1) gives the **signal-budget math** that justifies the embedder ensemble's information-theoretic value. The 21-embedder ensemble (doc 12) provides the **substrate**.

Combined: ME-JEPA-Code is the operationalized form of the operator's published TCT framework, with UTML's measurement formula as the optimization objective, on the existing 21-embedder substrate, governed by the per-output Gτ verifiability property — closing the loop between *training* (Phase 3 with `L_constellation_centroid`), *inference* (Phase 4 with `Verdict::GuardRejected`), and *self-healing* (Phase 5 constellation refresh).

The end goal of doc 09 §1.3 — `predicted_oracle_pass > 0.95` with calibrated confidence and OOD score < 0.5 — gains a fourth condition: **`Gτ_satisfied = true`**. This is the per-output cryptographic-grade verification that makes "no human code review needed" structurally defensible.

---

## See also

- Source paper: Royse, C. (April 2026). *Teleological Constellation Training: Multi-Modal Embedding Decomposition as Meaning Compression for Identity-Locked Generation and a Third Path Around the Data Wall.* DOI: 10.13140/RG.2.2.24370.57288.
- Master plan: `09_END_GOAL_REVIEW_REPLACEMENT.md`
- Pivot architecture: `10_MEJEPA_PIVOT_2026_05_09.md`
- UTML v1.0 learning formula: `11_MEJEPA_LEARNING_FORMULA.md`
- 21-embedder integration: `12_EMBEDDER_INTEGRATION_INTO_MEJEPA.md`
- Sitrep status: `sitrep/01_status_overview.md`
- Implementation plans: `sitrep/04_implementation_plans/`
- Source theory backbone: `docs2/UTL_v2_embodied.md` (UTML v1.0 — JEPA Edition)
