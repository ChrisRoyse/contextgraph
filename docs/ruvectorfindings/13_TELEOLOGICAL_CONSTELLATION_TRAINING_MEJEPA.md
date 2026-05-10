# 13 — Teleological Constellation Training + Universal Agent Verification for ME-JEPA-Code

> **📊 2026-05-10 STATUS UPDATE — §7 implementation roadmap is mostly shipped at the panel level.** SHIPPED in `crates/context-graph-mejepa-tct/`: item 1 `TctConstellation` struct + `CF_MEJEPA_CONSTELLATION` (`store.rs`), item 2 per-class centroid computation (`builder.rs`), item 3 per-embedder threshold `τ_m` calibration (`calibrator.rs`), item 4 `Gτ` predicate (`gtau.rs`), item 6 `Verdict::GuardRejected` variant (`verdict.rs`), item 8 constellation refresh trigger (`freshness.rs`), item 10 `mcp__cgreality__mejepa_constellation_inspect` MCP tool. SHIPPED in `crates/context-graph-mejepa-train/src/loss/constellation.rs`: item 5 `L_constellation_centroid` loss term. **Pending**: item 7 DDA pairwise signal computation + cross-embedder MI matrix (REQ-UTML-06), item 9 DDA signal-budget reporting in training certs, item 11 per-chunk-type centroids (waiting on Phase 1a multi-language chunker), item 12 per-chunk Gτ + chunk-level `GuardRejected` localization (waiting on Phase 1a). Universal-agent-verifier capability — answering Q1 (exists?), Q2 (works?), Q3 (impact?) on any AI agent's output — is operational at panel granularity today via `mejepa_verify`; per-AST-chunk granularity is gated on Phase 1a chunker.

> **Operator framing 2026-05-09 (verbatim, the synthesis):** *"this would be able to convert anything into insane amounts of data that is fully labeled and should be able to give the me jepa everything it needs to predict reality if the system is working in reality or not and also verify if the agent has done what it says its done to verify and validate the agents outputs and verify they are working in reality or not. with all of this information we will be able to derive an insane amount of reward signals that can all be used to train and optimize the me jepa."*

> **Source paper:** Royse, C. (April 2026). *Teleological Constellation Training: Multi-Modal Embedding Decomposition as Meaning Compression for Identity-Locked Generation and a Third Path Around the Data Wall.* ResearchGate DOI 10.13140/RG.2.2.24370.57288. Operator's published research; ContextGraph cited as one of two reference implementations of the TCT substrate (370K-line Rust workspace; 13 embedders; 91 signals/input at N=13).

> **Companion docs:**
> - `09_END_GOAL_REVIEW_REPLACEMENT.md` — master plan
> - `10_MEJEPA_PIVOT_2026_05_09.md` — single-outer-loop architecture
> - `11_MEJEPA_LEARNING_FORMULA.md` — UTML v1.0 four-factor formula
> - `12_EMBEDDER_INTEGRATION.md` — 21-embedder ensemble inventory + integration map
> - `14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md` — per-chunk routing for chunk-level Gτ

---

## 0. Headline — the capability claim

**Given any AI agent's output `ŷ` (a code patch, a tool call, a plan, a generation), ME-JEPA-Code can answer the three operator questions deterministically:**

```
Verdict = mejepa.verify(ŷ, context)
       = {
           exists:        bool,    // Q1: does what the AI claimed it did exist?
           works:         bool,    // Q2: does it work as intended?
           reality_impact: f32,    // Q3: how is it actually impacting reality?
           confidence:    [lo, hi],
           failed_modalities: Vec<EmbedderId>,
           constellation_violations: Vec<(centroid_id, cos_distance)>,
         }
```

Produced by:

1. **Computing the panel** — run all 14 frozen embedders on `ŷ` and its claimed source-of-truth chain. ~12.5 GB VRAM, all warm-loaded. <100 ms latency target.
2. **Checking the constellation guard `Gτ`** — is the panel within cosine threshold of the (KnownGood, language) centroid in *every* embedder space? Deterministic, no learned model.
3. **Running the predictor** — does ME-JEPA's `Pψ` predict the panel will produce the claimed oracle outcome? Cross-checked against the constellation guard.
4. **Extracting the 105-signal vector** — 14 single-embedder + 91 cross-embedder pair signals at N=14 (231 at N=21) — and inspecting which signals dropped below threshold. Each is a *named geometric reason* for the verdict.

**The agent doesn't need to be Claude Code.** It doesn't need to be a Python coding agent. It doesn't need to run in this repo. It just needs to produce an output the 14 embedders can encode. **ME-JEPA is output-source-agnostic** — a universal agent verifier.

The operator's published TCT framework is **the operational form of UTML v1.0 §VI** (representational non-collapse) for ME-JEPA-Code:

1. **DDA gives the signal-count math** (doc 12 §2): at N=21 frozen embedders, every input yields up to `n · (21 + 21²) = n · 462` structured signals.
2. **Meaning compression** is the right framing for the disk + storage budget: tier compression (Phase 6, ✅ shipped) is **bit compression**; panel materialization is **meaning compression** (signals per raw byte). Orthogonal, multiplicative, both already in tree.
3. **TCT's three-phase protocol** maps **directly** onto ME-JEPA-Code's design — Phase 1 (build CP) ≡ doc 09 Phase 0, Phase 2 (train against CP) ≡ doc 09 Phase 3, Phase 3 (Gτ at inference) ≡ doc 09 Phase 4.
4. **The frozen embedders in 3 simultaneous roles** (labeller + loss target + runtime guard) is **exactly** the architectural advantage doc 11 §5.2 named: ME-JEPA's `ΔΞ.target_collapse = 0 by construction`.

---

## 1. Why this works mathematically

### 1.1 The DDA counting identity (Royse 2026c §3.1)

```
n raw inputs × N frozen embedders → up to n · (N + (N choose 2)) structured supervisory signals
```

| Stack | N | per-input signals | per /loop turn (50-200 inputs) | per day (96 turns) | per quarter |
|---|---:|---:|---:|---:|---:|
| 9 (paper Case 1: Shakespeare LoRA) | 9 | 45 | — | — | — |
| 13 (ContextGraph paper-time baseline) | 13 | 91 | — | — | — |
| Production (E1-E14) | 14 | **105** | 5,250-21,000 | 504K-2M | 45M-180M |
| Full stack (E1-E21) | 21 | **231** | 11,550-46,200 | 1.1M-4.4M | 100M-400M |
| Loose upper bound at N=21 | 21 | 462 | 23K-92K | 2.2M-8.8M | 200M-800M |

**Per Phase 0 corpus (2,400 mutation entries) at chunk level (per doc 14):** ~510 signals per entry → **~1.22 M structured signals** for the full corpus, every one labeled by `(mutation_category, oracle_verdict, entity_type, language)` — no human annotation, no synthetic generator.

### 1.2 Why this is collapse-safe (Shumailov scope claim, Royse 2026c §3.3)

- The Shumailov 2024 / Dohmatob 2024 / Alemohammad 2023 collapse theorems require recursion: `D_{t+1} ∋ x̃ ∼ p_θ_t` — next training set contains samples drawn from prior generator.
- DDA does not instantiate this. Each signal `φ_m(x)` depends only on a real `x` and frozen embedder parameters. **Centroid computation depends only on real inputs and frozen embedders.** Training adjusts only ME-JEPA's predictor `Pψ`, never the embedders or centroids.
- Therefore the collapse precondition is not satisfied. ***This is scope, not refutation*** — the collapse theorems still hold for recursive self-training; DDA is structurally outside their domain.

This is what makes the 105-231 signals/input *durable* training material. It does not collapse under recursion because there is no recursion. **It is the third path around the data wall** (Royse 2026c §1): not acquire (license proprietary corpora), not synthesise (recursive self-training), but **decompose** a fixed corpus through frozen embedders.

### 1.3 Why this is per-output verifiable (the structural advantage over RLHF)

Scalar-reward alignment (InstructGPT, DPO, Constitutional AI) optimizes `E[r_ψ(y|x)]` — at inference time `r_ψ` is trained, not auditable, and a single scalar offers no diagnostic signal when it fails.

The TCT guard `Gτ(ŷ; CP) = 𝟙[∀m: cos(φ_m(ŷ), c_m) ≥ τ_m]` is:

- **Deterministic** — no learned model, no RNG.
- **Geometric** — produces a typed reason for refusal (which modalities failed).
- **Re-runnable** — given the same `ŷ` and `CP`, anyone can recompute and verify.
- **Computationally bounded** — `O(N · d_max)` per output.

Royse 2026c §1 / §7 makes this contrast explicit: this is a structural property scalar-reward methods *cannot* provide in the same technical sense.

---

## 2. TCT three-phase protocol for ME-JEPA-Code

### 2.1 Phase 1 — construct frozen multi-modal centroid set CP

Per Royse 2026c §5.1: collect per-modality reference sets, compute L2-normalized per-modality centroids, freeze them.

**For ME-JEPA-Code:**

- **Per-class centroid construction** — for each of 8 mutation categories (KnownGood, SubtleFlip, OffByOne, SwapVariable, DeleteTestCall, WrongFile, OverEngineer, CompileError) and each of 21 embedders, compute the L2-normalized centroid of that category's panel vectors over the Phase 0 mutation corpus.
- **Cross-language centroids** — for each `(mutation_category × language)` pair (8 × 9 supported languages = 72 cells), compute a separate centroid. Handles doc 10 Section C cross-language mandate.
- **Outcome-class centroids** — for each oracle outcome class (Pass / Fail / OOD / Abstain), compute the centroid of panels with that outcome. **The Pass-centroid IS the predicted-reality target ME-JEPA's predictor optimizes toward.**

```rust
pub struct TctConstellation {
    pub per_category_centroids: BTreeMap<MutationCategory, BTreeMap<EmbedderId, Vec<f32>>>,
    pub per_language_centroids: BTreeMap<(Language, MutationCategory), BTreeMap<EmbedderId, Vec<f32>>>,
    pub outcome_centroids: BTreeMap<OracleOutcome, BTreeMap<EmbedderId, Vec<f32>>>,
    // Per doc 14: per-(mutation × entity_type × language × embedder) cells for chunk-level Gτ
    pub per_chunk_type_centroids: BTreeMap<(MutationCategory, EntityType, Language, EmbedderId), Vec<f32>>,
    pub thresholds: BTreeMap<EmbedderId, f32>,                 // panel-level τ_m
    pub per_chunk_type_thresholds: BTreeMap<(EntityType, EmbedderId), f32>,
    pub corpus_provenance: CorpusProvenance,                   // SHA-256 of source corpus + embedder versions
    pub frozen_at: SystemTime,
}
```

**FSV gate:** the constellation is FROZEN — once constructed, never updated except by explicit operator action with provenance. Stored in RocksDB CF `CF_MEJEPA_CONSTELLATION` keyed by version_id.

**Cell count math:** at panel level, 21 embedders × 8 categories × 9 languages = **1,512 cells** (~500 KB total). With per-chunk-type expansion: 8 × 13 entity types × 11 languages × 21 = **24,024 cells** (~74 MB raw, ~30 MB after Bayesian shrinkage on sparse cells).

**Why per-chunk-type centroids matter for verifying agent outputs.** A patch-level verdict tells the operator only "the patch is bad." A per-chunk-type Gτ verdict tells the operator: "the agent's `Function:plus_one` chunk in `crates/foo/src/lib.rs` lines 42-58 fails the `(SubtleFlip, Function, Rust, E7)` centroid by 0.12 below threshold." That is **per-output verifiability at chunk granularity** — every chunk the agent emitted is independently re-verifiable by anyone with the frozen embedders + the constellation.

### 2.2 Phase 2 — train predictor with geometric-proximity loss

Per Royse 2026c §5.2: the generator (ME-JEPA's predictor `g_φ`) is trained to produce outputs whose embeddings are close to the corresponding centroids in CP under a geometric loss.

**Training loss extends doc 09 §3.7's multi-objective form with TCT Phase 2 terms:**

```
L_full = α·L_panel                                  # existing predictor cosine to target panel
       + β·L_pairwise                               # existing pairwise predictor terms
       + γ·L_action                                 # action-conditioning regularizer (UTML §VI.3)
       + δ·L_constellation_centroid                 # NEW (TCT Phase 2): per-embedder margin
       + ε·L_anti-pole                              # existing VICReg-style variance/covariance
       + ζ·L_uncertainty + η·L_invariant            # existing
       + α_warn·L_warnings + α_runtime·L_runtime + α_rss·L_memory
       + α_trace·L_traceback + α_hunk·L_hunk_attribution + α_adjacent·L_collateral
       + α_reasoning·L_reasoning + α_overrides·L_operator_match

L_constellation_centroid = Σ_m=1^N max(0, τ_m − cos(g_φ(panel)_m, c_m_{predicted_class}))
```

**Why this matters:** a margin-based, per-embedder geometric loss that pushes the predictor toward the right region of EACH embedder's latent space simultaneously. Stronger than single-embedder cosine loss because it enforces consistency across all 21 frozen feature spaces — the predictor cannot Goodhart against any one embedder without being penalized in the others.

This is also the **L_step amplifier** for UTML v1.0's ΔP: per-embedder cosine errors are 21 separate ΔP measurements, geometrically combined, all contributing to gradient.

### 2.3 Phase 3 — per-output cosine guard Gτ at inference

Per Royse 2026c §5.3: the deterministic geometric test for per-output verifiability.

```
Gτ(ŷ; CP) = ⟦∀m : cos(φ_m(ŷ), c_m) ≥ τ_m⟧
```

Words: candidate output `ŷ` passes the guard iff its embedding under EVERY one of the N embedders has cosine similarity ≥ per-embedder threshold `τ_m` against the corresponding centroid `c_m`.

**For ME-JEPA-Code** this is a **fourth gate** added to `MeJepaCompiler::verify(patch)` alongside conformal interval, OOD score, L_step health:

```rust
pub struct ChunkId {
    pub file_sha: [u8; 32],
    pub language: Language,
    pub entity_type: EntityType,
    pub line_start: u32,
    pub line_end: u32,
    pub parent_chain_hash: [u8; 16],
}

pub struct GtauViolation {
    pub embedder: EmbedderId,
    pub observed_cos: f32,
    pub threshold: f32,
    pub violating_chunk: Option<ChunkId>,    // localizes rejection to a chunk (per doc 14)
}

pub enum Verdict {
    Pass {
        confidence_interval: (f32, f32),
        per_test_probabilities: Vec<f32>,
        gtau_satisfied: bool,
        gtau_violations: Vec<GtauViolation>,
    },
    Fail { ..., gtau_satisfied: bool, gtau_violations: Vec<GtauViolation> },
    OutOfDistribution {
        latent_density: f32,
        gtau_violations: Vec<GtauViolation>,
    },
    Abstain { reason: String },
    GuardRejected {                           // NEW VARIANT
        // Predictor said "Pass" but Gτ rejected: prediction sits in the right region
        // of N-1 embedders' space but wrong in at least one. The operator paper calls
        // this "structurally unverifiable."
        violating_embedders: Vec<GtauViolation>,
        predictor_predicted: PredictorOutput,
        violating_chunks: Vec<ChunkId>,        // per doc 14 — chunk-level localization
    },
}
```

The `GuardRejected` variant is **the per-output verifiability property** scalar-reward alignment cannot provide. Each rejection is a deterministic O(N · d_max) geometric test the operator (or any other verifier) can independently re-run on the persisted panel.

**Per-chunk localization (per doc 14 §8.1).** With chunk-level Gτ wired, a `GuardRejected` no longer says "this 200-line patch is bad" — it says "the `Function:apply_filter` chunk in `tests/test_units.py` lines 124-167 fails the `(SubtleFlip, Function, Python, E11)` centroid by 0.18." The agent (or operator) can:

1. **Know precisely what failed** — which function, class, import block.
2. **Re-verify independently** — re-run the deterministic Gτ test on the persisted chunk panel, no model state needed.
3. **Repair surgically** — the next outer-loop turn targets only violating chunks.

### 2.4 The same frozen embedders in three roles (the core architectural move)

Per Royse 2026c §1: the central architectural move of TCT is **using the same frozen embedder panel Φ in three semantic roles simultaneously without any parameter update**:

| Role | TCT phase | ME-JEPA-Code instantiation |
|---|---|---|
| **Labeller** | Phase 1 (build CP) | Phase 0 mutation corpus generation runs the 21 embedders to build per-class centroids |
| **Loss target** | Phase 2 (train against CP) | Phase 3 training loop optimizes against `L_constellation_centroid` |
| **Runtime guard** | Phase 3 (Gτ at inference) | Phase 4 verify() runs Gτ as a fourth gate on every prediction |

**No parameter is updated across the three roles.** The 21 embedders are loaded once, used in all three modes, persisted with a SHA-pinned version, and CP is computed once from them. This is **exactly** the architectural property doc 11 §5.2 named as ME-JEPA's structural collapse-immunity.

---

## 3. Constellation guard implements UTML v1.0's missing piece (ΔΞ diagnostic #3)

Per UTML v1.0 §VI.3 (doc 11 §5.3), ME-JEPA defends predictor-side ΔΞ with four diagnostics:

1. VICReg variance/covariance regularization
2. Pairwise mutual-information audit
3. **Constellation + Gτ guard** ← **TCT Phase 3, formalized**
4. Negative-action ablation falsifier

The TCT paper provides the **operational specification** for diagnostic #3, which doc 11 referenced but did not fully spec. With TCT integrated:

- **`ΔΞ.constellation_violation_rate_t`** in the ΔΞ formula (doc 11 §5.4):
  ```
  constellation_violation_rate_t = (recent guard rejections) / (total recent verifications)
  ```
- Per-embedder threshold `τ_m` calibrated on a held-out validation slice such that Gτ false-positive rate (rejecting true-Pass patch) is ≤5%.
- Empirical Gτ rejection >X% over a rolling window triggers UTML §IX coaching cycle's "Latent-Collapsing" intervention — constellation centroids likely stale, refit needed.

### 3.1 Reward-formula upgrade

Per Royse 2026c §3 / §5 + UTML §XIV, the reward signal becomes:

```
Reward = 0.4 · calibrated_confidence
       + 0.3 · mean(L_step · ΔΞ)
       + 0.3 · gtau_pass_rate                     # NEW: TCT verifiability term
```

`gtau_pass_rate` = rolling fraction of verify() calls whose Gτ check passed without rejection. This explicitly rewards producing patches that pass the deterministic geometric verifier. Original UTML formula (`0.6 / 0.4`) reweights to balance:

- **Calibration** (system knows its uncertainty)
- **Latent integrity** (system updates without collapsing)
- **Per-output verifiability** (system produces outputs passing the geometric test)

---

## 4. The universal agent verifier

### 4.1 Verifying Claude Code (immediate use case)

When Claude Code claims:

> "I fixed the bug in `astropy/coordinates/earth.py` line 387 by changing `>` to `>=`."

ME-JEPA verifies:

1. **Q1 (exists?)** — Read post-edit SHA. Run E11 (Kepler) + E1 (semantic) on `(claim, post_edit_diff)`. If `cos(claim_embed, diff_embed) < τ_exists`, claim does NOT match actual edit — refuse with `MEJEPA_CLAIM_DOESNT_EXIST`.
2. **Q2 (works?)** — Run all 14 embedders on post-edit code. Check `Gτ(panel; KnownGood_python)`. If panel far from centroid in (E5, E7, E11), refuse with `MEJEPA_INFER_CONSTELLATION_VIOLATION` listing failed modalities.
3. **Q3 (impact?)** — Run predictor's oracle-pass head. Compare against actual Docker oracle (when available). The 91 pair signals give a 91-dim diagnostic vector for *how* reality diverged from the prediction.

### 4.2 Verifying any other AI agent

The exact same machinery applies to:

- **External LLM agents** (GPT-5, Gemini, Cursor, Aider, etc.) — they emit a patch; ME-JEPA verifies the same way.
- **Human PRs** — verifies humans aren't hallucinating about what their patches do.
- **Automated bots** — Dependabot, Renovate, etc. — verify upgrade patches don't break the constellation.
- **ME-JEPA itself** when it eventually proposes patches via gradient ascent on its predicted reality (Royse 2026c §1) — verifies its own outputs by the same guard, no privileged path.

The agent's identity is irrelevant. **What matters is that the output can be encoded by the 14 frozen embedders and compared to constellations computed from real working code.**

### 4.3 Cross-language and cross-domain universality

**Cross-language:** E14 BGE-M3 multilingual head anchors panel layout dimension-wise across Python, Rust, C, C++, Java, C#, JS, TS, Ruby. Per-language constellation centroids capture "working code" per language. For a Rust patch: load `(KnownGood, Rust)` constellation; run `Gτ`. Machinery unchanged.

**Cross-domain (beyond code):** Royse 2026c instantiates the same machinery on:
- Voice cloning (Case 3): WavLM + DNSMOS + UTMOS centroids → 0.961 mean SECS, 9 of 10 sentences ≥ 0.95.
- Stylistic LoRA (Case 1): 9-embedder text-style panel → robust Shakespearean generation with emergent zero-shot Spanish Golden Age period transfer.
- Talking-head clone (Case 2): visual + speaker + AU + text panel.

**The pattern transfers.** ME-JEPA-Code is the code-domain instance of TCT. Future ME-JEPA-Voice, ME-JEPA-Image, ME-JEPA-X domain packs follow the same structure: pick N frozen embedders covering the modality, build constellations from real working examples, train a predictor with constellation-attraction loss, gate inference with `Gτ`.

---

## 5. The agent-output verification flow (operator's primary use case)

### 5.1 Verification flow

```
Agent (Claude Code) emits a claim:
  "I edited file X.py at lines 45-52, added function `parse_widget`,
   running tests should now pass test_widget_parsing."

ME-JEPA receives:
  - Pre-state panel from before claimed edit (file SHA = pre_sha)
  - Post-state panel from after claimed edit (file SHA = post_sha)
  - Claim graph: ((added, function, parse_widget, X.py:45-52),
                  (predicted_passes, test, test_widget_parsing))

Verification:
  Step 1 — DOES IT EXIST:
    - observed_post_panel = embed_all_21(post_state)
    - predicted_post_panel_from_claim = predict_panel_from_claim(claim)
    - Gτ(observed_post_panel; CP_for_claim_class)
    - Any embedder rejects → CLAIM_DOES_NOT_MATCH_REALITY error with
      per-embedder diagnostics. Reward signal: agent's claim was false.

  Step 2 — DOES IT WORK:
    - g_φ(panel[t=0..1]) → predicted_panel_t2
    - Gτ(predicted_panel_t2; CP_Pass) — does the predictor think it'll pass?
    - Conformal interval: is uncertainty narrow enough?
    - L_step health: prediction made under healthy ΔP/ΔK/ΔΩ/ΔΞ?
    - All green → Verdict::Pass with conformal interval; reward: claim verified-true.

  Step 3 — IMPACT ON REALITY:
    - Compare observed_post_panel vs predicted_post_panel across all 21 embedders
    - Each axis where they disagree is a captured "reality surprise" — a labeled
      training example saying "the agent's claim about this slot was off by X
      under embedder E_i". Becomes ME-JEPA training data for the next round.
```

### 5.2 Reward-signal density per single agent-claim verification

| Reward signal | Count | Use |
|---|---:|---|
| Per-embedder claim-vs-reality cosine | 21 | direct ΔP signal per embedder for "did claim match observed?" |
| Per-embedder Gτ violation distance | 21 | how far below threshold each embedder rejected (graded penalty) |
| Cross-embedder agreement on observed panel | 210 | ΔK signal for "is observed reality coherent?" |
| Cross-embedder agreement on predicted panel | 210 | ΔΞ signal for predictor representation health |
| Gτ pass/fail per claim aspect | 1-N per claim | binary supervised reward |
| Conformal interval vs actual oracle (when sampled) | 1 boolean | calibration anchor (sparse but gold-standard) |
| Per-test predicted-vs-actual outcome | 5-50 | direct supervision per test in suite |
| Side-effect prediction-vs-observed | 1-20 | files modified, env vars set, network calls, etc. |

**Aggregate per single agent-claim verification: 470-540 distinct reward signals.** Across SWE-bench Lite 300 × ~10 attempts per task, the steady-state corpus accumulates **hundreds of millions of labeled supervisory signals** — without any LLM in the loop, without human labels beyond ground-truth Docker oracle on a 5% sample.

This is the **operational realization** of "insane amount of reward signals from durable reality": every Edit/Write tool call by Claude Code produces a panel update; every panel update produces 231-462 DDA signals; every claim the agent makes produces another 470-540 verification signals when checked. **The system converts every byte of reality into hundreds of labeled training examples.**

### 5.3 Three load-bearing uses of the signal supply

**a. Training ME-JEPA itself** — every panel triple × 105 signals × `L_step` weighting → gradient on predictor `Pψ`. Standard Phase 3 training loop.

**b. Training other agents (reward signal for RL / RLHF / fine-tuning)** — any agent that wants to improve its code-generation can use ME-JEPA's verdict as reward. Geometrically grounded (not a scalar from a learned reward model that can be Goodharted), so **safer than RLHF rewards** — Goodharting the constellation requires simultaneously fooling 14 independent embedders.

**c. Operator dashboards / debugging** — when an agent's output fails the guard, the failed-modalities vector tells the operator *which embedders disagreed*. "E5 + E7 failed" means "causal direction wrong AND code structure wrong." Categorical fault label, not vague low-confidence flag.

### 5.4 Why this makes "no human code review needed" structurally defensible

When an agent submits a patch with `verify(patch).Pass{ confidence: [0.96, 0.99], gtau_satisfied: true, gtau_violations: [], per_test_probabilities: [...] }`, the human reviewer's role reduces to:

1. **Spot-checking calibration** — is ME-JEPA's claimed 0.97 pass-probability empirically validated on similar past patches? (One-time, answered by §4 evaluation.)
2. **Auditing constellation provenance** — was CP constructed from a corpus the reviewer trusts? (One-time, answered by Royse 2026c §5.1's frozen-centroid provenance.)
3. **Re-running Gτ if suspicious** — deterministic O(N · d_max) test takes <100 ms on persisted panel. Reviewer re-verifies geometric proof without retraining anything.

**What the reviewer no longer does:** read patch line-by-line for correctness (Gτ already verified), re-run tests manually (per-test pass-probability + occasional Docker oracle covers it), reason about side effects (predicted vs observed shift-log delta covers it).

The role shifts from "verifier" to "occasional spot-checker of the verifier-of-the-verifier." Per the source paper, per-output verifiability via Gτ is what makes this structurally different from RLHF/DPO/Constitutional AI: those approaches verify only the *training* of the policy; TCT-ME-JEPA verifies *each output* against a deterministic, replayable geometric test.

---

## 6. Cross-embedder coherence as richer ΔK signal

Doc 12 §6.1 proposed `embedder_coherence(panel)` as a 0.05-weight supplement to UTML v1.0's ΔK. The TCT paper sharpens this with **the cross-embedder pairwise signals from DDA**:

```
ΔK_full = 0.4 · cos_align(g_t, ḡ_prior)
        + 0.25 · (1 − fisher_violation)
        + 0.15 · (1 − ECE)
        + 0.1 · embedder_coherence(panel)         # cross-embedder NN agreement
        + 0.05 · constellation_centroid_proximity # mean cos(panel, c_m) across all m
        + 0.05 · cross_embedder_mi_consistency    # pairwise MI vs corpus prior
```

The 5-term ΔK uses DDA's full signal capacity. Each new term contributes 0.05-0.1 individually, but together they give the optimizer 5 independent coherence checks per step — deeply diagnostic when one degrades.

### 6.1 High-leverage pair signals for code

| Pair | What it measures | Why it's load-bearing |
|---|---|---|
| (E1 semantic, E7 code) | "Does the natural-language description match the code structure?" | Catches AI patches where the commit message lies about what the code does |
| (E5 causal, E7 code) | "Does the predicted cause→effect direction match the AST diff?" | Catches confused causal claims in AI-written diff explanations |
| (E7 code, E11 kepler) | "Does the code use the entities the dependency graph says are reachable?" | Catches ImportError / NameError before runtime |
| (E1 semantic, E14 multilingual) | "Does the description survive translation through the multilingual head?" | Catches language-specific overfitting; cross-language anchor |
| (E6 sparse, E13 splade) | "Do two independent sparse encoders agree on keyword salience?" | Catches AI patches that hallucinate function names |
| (E9 HDC, E12 late_interaction) | "Robust signature × token-precision agreement" | Catches typo-induced regressions |
| (E10 contextual, E8 graph) | "Static-analysis warnings × type-graph structure" | Catches type errors before compiler runs |
| (E5 effect→cause vs cause→effect) | "Causal asymmetry preserved" | Catches reversed-direction reasoning |

Across 91 pair-combinations at N=14 (210 at N=21), ME-JEPA gets a **dense diagnostic signal vector** for every panel triple — far beyond what single-embedder slot signals carry.

---

## 7. Implementation roadmap

| # | Item | Effort | Dependency | Status |
|---|---|---|---|---|
| 1 | `TctConstellation` struct + RocksDB CF `CF_MEJEPA_CONSTELLATION` | ~1 day | Phase 0 corpus | new |
| 2 | Per-class centroid computation (8 categories × 9 languages) | ~2 days | (1) + Phase 0 corpus generation | new |
| 3 | Per-embedder threshold `τ_m` calibration (5% FPR target) | ~1 day | (2) + held-out validation | new |
| 4 | `Gτ` predicate as deterministic O(N · d_max) check | ~1 day | (1) | new |
| 5 | `L_constellation_centroid` loss term in Phase 3 training loop | ~2 days | (1) + Phase 3 training | new |
| 6 | `Verdict::GuardRejected` variant + persistence in `CF_MEJEPA_GUARD_DECISIONS` | ~1 day | (4) + Phase 4 verify() | new |
| 7 | DDA pairwise signal computation (cross-embedder cosine + MI matrix) | ~3 days | (1) + N=21 ensemble loaded | new |
| 8 | Constellation refresh trigger (when guard rejection rate > X%) | ~1 day | (4) + Phase 5 self-healing | per UTML §IX |
| 9 | DDA signal-budget reporting (per-input n·(N+(N choose 2)) signals) in training certs | ~1 day | (5) + Phase 3 cert format | new |
| 10 | TCT-aware MCP tool: `mcp__cgreality__mejepa_constellation_inspect` | ~1 day | (1) | new |
| 11 | Per-chunk-type centroids (per doc 14 §5) | ~2 days | (2) + doc 14 chunker | new |
| 12 | Per-chunk Gτ + chunk-level `GuardRejected` localization | ~2 days | (4) + (11) | new |

**Total TCT-specific work:** ~14 days for panel-level + ~4 days for chunk-level = **~18 days**. Slots into Phases 0/3/4/5 of doc 09 — does not extend the critical path; runs in parallel.

### 7.1 Quarter-by-quarter capability gates

| Quarter | Deliverable | Capability unlocked |
|---|---|---|
| Q1 (current) | Memory decisions + docs 10-14 + sitrep updated | Operating contract clarified; UTML v1.0 + TCT/DDA + 105-signal capability locked in |
| Q2 | Pairwise-MI audit + `mejepa_compute_panel`/`mejepa_compute_signal_set` MCP tools + cross-language Phase 0 corpus | DDA tightness measured; per-input panel + 105-signal vector callable from agents |
| Q3 | Phase 1-3 instruments + predictor + constellation-aware training | ME-JEPA produces calibrated predictions on held-out mutation corpus |
| Q4 | Phase 4 `mejepa_verify` MCP tool + `Gτ` guard | **Universal agent verifier callable; "verify what the AI did" capability live** |
| Q5+ | Phase 5 self-healing + Phase 7 reality-loop integration | ME-JEPA continuously improves from /loop turns; correlation climbs toward 0.95 |
| When correlation ≥ 0.95 | Phase 8 evaluation gate met | "No code review needed" gate achieved on Q1 ship target |

---

## 8. New capabilities unlocked

### 8.1 Per-output verifiability for AI-generated patches

Today: `MeJepaCompiler::verify(patch)` returns probabilistic verdict with conformal interval. With TCT Phase 3 + Gτ, **every verdict carries a deterministic geometric proof** the operator (or any third party) can independently replay on the persisted panel. Per-output verifiability — no auxiliary learned model, no scalar proxy. **Q1 ("did it exist?") answered structurally for the first time at this scale.**

### 8.2 Goodhart resistance via multi-embedder consistency

Scalar-reward alignment (RLHF, DPO) is vulnerable to Goodhart catastrophe (Skalse 2024) — policy can find high-reward region the reward model didn't anticipate. **The constellation guard makes Goodhart structurally harder** because the predictor must satisfy 21 INDEPENDENT geometric thresholds simultaneously. Goodharting against one embedder leaves the other 20 unsatisfied; Gτ rejects.

### 8.3 Cross-language transfer via per-language centroids

Per-language centroids (8 categories × 9 languages = 72 cells) explicitly encode the cross-language Section C mandate. A patch in Rust verified against Rust constellation; Python against Python. Cross-language generalization is now a verifiable property: as the system trains on more languages, per-language centroid agreement on shared mutation categories should converge.

### 8.4 The reality loop produces 462× more supervision per attempt

DDA at N=21 means every Claude Code outer-loop turn produces up to 462 structured signals per `(panel, oracle)` tuple. The §3.8 mejepa-shift-subscriber's job multiplies in value: each shift triggers panel materialization across 21 embedders, producing a full DDA signal vector that feeds online observe with two orders of magnitude more gradient signal per attempt.

### 8.5 Will the patch work in reality? — predicted before deployment

Today: run the test suite. Slow, sometimes flaky, often incomplete coverage.

Post-integration: `mejepa.verify(...)` returns a calibrated confidence interval on oracle outcome before any test runs. When confidence ≥ 0.95 *and* OOD score < 0.5 *and* constellation guard passes, the patch is *already verified* — the test run becomes optional confirmation, not the primary signal.

### 8.6 How is the patch impacting reality? — diagnosed via 91/210-dim vector

Today: operator reads logs, runs profilers, asks "did this break something else?".

Post-integration: the 91 (or 210) cross-embedder pair signals form a **diagnostic vector**. Specific entries name specific failure modes: "(E5, E7) low" → causal direction contradicts code structure; "(E1, E14) low" → change works in English-only and breaks for multilingual users. The operator reads a structured verdict, not raw logs.

### 8.7 Infinite training data without leaving the reality-grounded regime

Per §1.2: every signal is a deterministic function of real inputs and frozen embedders. **The training corpus is structurally outside the Shumailov collapse regime**. Corpus can grow without bound — every Claude Code action, every external agent's verified output, every PR review, every failed Docker oracle run — and contribute durable training signal. The data factory is **infinite in supply and finite in collapse risk**.

### 8.8 Self-improving reward signal for any future agent

Once ME-JEPA's correlation crosses ~0.95 (Phase 8 ship gate), it becomes a **reusable verifier**:
- Future Claude Code versions train RLHF on ME-JEPA's verdicts.
- Future external agents calibrate their generation against ME-JEPA's `Gτ`.
- Future ME-JEPA-X domain packs inherit the architecture.

The reward signal is **shareable infrastructure**, not Claude-Code-specific.

### 8.9 Meaning compression as a publishable contribution

Per Royse 2026c §3.2, meaning compression is proposed as a fourth taxonomic entry alongside bit / weight / activation compression — defined as **structured signal density per unit of raw data**, in signals-per-raw-byte. ME-JEPA-Code with full TCT/DDA pipeline is **the operational realization of meaning compression for software engineering** — each raw SWE-bench Lite mutation becomes 231-462 structured supervisory signals, an empirical demonstration the framework generalizes to the code domain.

For one Phase 0 mutation entry averaging ~50 KB raw source: `meaning_compression_ratio = 231 signals / 50 KB ≈ 24,000 signals per MB raw source`. **Combined with tier compression (Phase 6), a 1 MB raw source contributes ~24,000 supervisory signals at ~1.7 KB tier-compressed cold-tier storage** — five orders of magnitude amplification of supervision per byte stored.

### 8.10 The operator's published research becomes the operational architecture

By integrating TCT directly into ME-JEPA-Code, the system implements the operator's published research paper (Royse 2026c, DOI 10.13140/RG.2.2.24370.57288) as its **operational architecture**, not just inspirational reference. The paper cites ContextGraph as a 370K-line reference implementation; this doc closes the loop by making the implementation actually use the framework end-to-end.

---

## 9. Signal-budget summary

For ME-JEPA-Code at full N=21 ensemble, every Phase 0 mutation entry yields:

| Signal class | Count per input | Source |
|---|---:|---|
| Per-embedder vector outputs | 21 | direct embedder inference |
| Per-embedder cosine to target | 21 | predictor vs target panel |
| Per-embedder constellation centroid distance | 21 | TCT Phase 2 loss term |
| Pairwise embedder agreement (NN Jaccard) | 210 | cross-embedder neighbor sets |
| Pairwise embedder cosine | 210 | cross-embedder vector similarity |
| Cross-embedder MI (when corpus large) | 210 | mutual information audit |
| Constellation Gτ pass/fail | 1 boolean | TCT Phase 3 |
| Per-test pass/fail vector (oracle) | typically 5-20 | E_Oracle instrument |
| Static analysis warnings | 5-50 | E_StaticAnalysis (per language) |
| Runtime aggregates | 5-15 | E_Runtime |
| Witness chain entries | 5-50 | E_Witness |
| Trace events | 50-500 | E_Trace |
| Per-chunk panel signals (per doc 14) | ~280 | ~40 chunks × ~7 signals |

**Conservative aggregate per input: ~462 panel signals + ~280 chunk signals = ~742 distinct supervisory signals.** ME-JEPA's job is to compress all of this through the predictor's 5,120-d latent and use it as supervision for the next prediction.

---

## 10. Risks (TCT-specific)

1. **Centroid staleness** — as corpus grows, per-class centroids drift. Refresh trigger (item #8 in §7) handles, but stale centroids during lag produce false Gτ rejections. Mitigation: refresh every 1,000 new corpus entries OR when guard rejection rate exceeds 5% sustained for 2 hours.
2. **Per-embedder threshold calibration brittleness** — `τ_m` calibrated on Phase 0 mutations may not transfer to live attempts. Mitigation: re-calibrate `τ_m` on live-attempt holdout slice during Phase 5 self-healing.
3. **Pairwise MI computation cost** — 210 pairs × full corpus = O(N² · |D|) MI matrix. Cache per-embedder marginals; recompute pairwise MI weekly during periodic full retrain (UTML §VII k_replay window).
4. **Embedder version skew** — if any of 21 embedders updated, all centroids invalidate. Mitigation: SHA-256 pin model weights at constellation construction; refuse to use a constellation if any embedder's SHA mismatches at runtime (fail-closed per `CCREALITY_*` error taxonomy).
5. **Per-language centroids data-sparse** — at N=21 × 9 languages × 8 categories = 1,512 cells; some <50 examples. Mitigation: borrow from global centroid (Bayesian shrinkage) when per-cell support is below threshold; flag cells at risk of false rejection.
6. **Gτ threshold τ_m too strict in early training** — predictor is bad initially → all Gτ rejects → no training signal. Mitigation: anneal τ_m from 0 (permissive) to calibrated value over first 10K steps; UTML §IX classifies as Latent-Collapsing only after annealing completes.

---

## 11. Falsification (per Royse 2026c §13 + UTML §XIII)

The framework is **falsifiable**. Specific scenarios:

1. **DDA tightness audit returns ~uniform pairwise MI** → most of 91/210 pair signals are redundant; effective signal count drops toward N. Headline 105-462 may need revision down.
2. **Constellation centroids don't separate mutation categories** → if `KnownGood` centroid is closer to `SubtleFlip` than to itself in some embedder space, that embedder is not contributing useful structural signal — drop from panel.
3. **Cross-language T(train, deploy) gap unbounded** → if per-language `KnownGood` centroids are too far apart, cross-language transfer fails and ME-JEPA must train per-language. Detectable via conformal coverage on cross-language holdouts. **Falsifies if Python-trained ME-JEPA evaluating Rust loses > 50 percentage points on (predict-correct ∩ guard-passes) accuracy.**
4. **Guard `Gτ` rejection rate uncorrelated with actual oracle failure** → guard is noise, not signal — disable it, fall back to predictor-only inference. **Falsifies if correlation < 0.5 on a held-out 50-task set.**
5. **The Shumailov collapse regime turns out to apply** (e.g., if we accidentally feed ME-JEPA's own predictions back as constellation centroids). Discipline failure, not theory failure — fix by enforcing "centroids never include model outputs" in CF write gate.
6. **Constellation centroid loss redundant with VICReg** → if `L_constellation_centroid` and `L_anti-pole` produce identical gradients in ablation, drop new term; keep VICReg.
7. **ME-JEPA-vs-oracle prediction agreement plateaus < 0.85** within 6 months of /loop running with integration shipped → revisit Phase 2-3 architecture.
8. **Verdict reproducibility < 99.9%** — running `mejepa.verify(ŷ, context)` twice on same input returns identical verdicts. **Falsifies if non-determinism > 0.1%.**

If any falsification holds via well-powered replication, the corresponding sub-formula gets `status: superseded` in `memory/decisions/` and a new decision file describes the corrected version.

---

## 12. Linking back to the audit substrate (UTML v1.0 §XII)

Every panel, every constellation distance, every guard verdict is persisted to existing 25 RocksDB CFs (`dj_*`) + BlackBoxFix SQLite ledger at `LEDGER_SCHEMA_VERSION = 14`. This is the **physical M(t)** UTML v1.0 §XII names.

Each verdict is byte-equal-readback-verified before influencing the next decision (Royse 2026a §7). The training corpus, the constellation centroids, the 105/231-signal vectors, the guard decisions — all queryable, all replay-able, all auditable.

ME-JEPA is not "a model with predictions" — it is **a model + an audit substrate where every prediction is geometrically grounded and re-verifiable**.

---

## 13. The full picture

UTML v1.0 (doc 11) gives the **measurement framework**: `L_step = ΔP × ΔK × ΔΩ × ΔΞ`. TCT (this doc) gives the **constructive method** for achieving structural ΔΞ.target_collapse=0 and produces the concrete Gτ test for runtime verification. DDA (this doc §1) gives the **signal-budget math** that justifies the ensemble's information-theoretic value. The 21-embedder ensemble (doc 12) provides the **substrate**. AST-aware chunk routing (doc 14) provides the **per-function granularity**.

Combined: ME-JEPA-Code is the operationalized form of the operator's published TCT framework, with UTML's measurement formula as optimization objective, on the existing 21-embedder substrate, governed by the per-output Gτ verifiability property — closing the loop between *training* (Phase 3 with `L_constellation_centroid`), *inference* (Phase 4 with `Verdict::GuardRejected`), and *self-healing* (Phase 5 constellation refresh).

The end goal of doc 09 §1.3 — `predicted_oracle_pass > 0.95` with calibrated confidence and OOD score < 0.5 — gains a fourth condition: **`Gτ_satisfied = true`**. **This is the per-output cryptographic-grade verification that makes "no human code review needed" structurally defensible.** The 14 production embedders + UTML v1.0 four-term L_step + Royse 2026c TCT/DDA framework **together** turn every Claude Code action into up to 105 fully-labelled supervisory signals per input (231 at N=21), structurally outside the Shumailov 2024 collapse regime. Per-output verifiability is the structural property scalar-reward alignment cannot provide. Cross-language and cross-modality coverage are constellation-set-selection problems, not retraining problems. **The data factory is infinite in supply and finite in collapse risk — a third path around the data wall.**

---

## See also

- Source paper: Royse, C. (April 2026). *Teleological Constellation Training: Multi-Modal Embedding Decomposition as Meaning Compression for Identity-Locked Generation and a Third Path Around the Data Wall.* DOI: 10.13140/RG.2.2.24370.57288.
- Master plan: `09_END_GOAL_REVIEW_REPLACEMENT.md`
- Pivot architecture: `10_MEJEPA_PIVOT_2026_05_09.md`
- UTML v1.0 learning formula: `11_MEJEPA_LEARNING_FORMULA.md`
- 21-embedder integration: `12_EMBEDDER_INTEGRATION.md`
- AST-aware chunking: `14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md`
- Spec status/roadmap: `specs/_README.md` + `specs/implementation-plan.md`
- Task graph: `specs/tasks/_index.md` + `specs/tasks/_traceability.md`
- Source theory backbone: `docs2/TheoryOfMachineLearning.md` (UTML v1.0 — JEPA Edition)
- Royse, C. (2026a). *ContextGraph Reality Loop*. ResearchGate DOI 10.13140/RG.2.2.16013.91366.
- Royse, C. (2026b). *Dynamic / ME-JEPA*. ResearchGate DOI 10.13140/RG.2.2.28485.41446.
- Locked decisions:
  - `memory/decisions/2026-05-09--multi-embedder-utml-integration-post-qwen.md`
  - `memory/decisions/2026-05-09--utml-v1-jepa-edition-mejepa-learning-formula.md`
  - `memory/decisions/2026-05-09--single-loop-claude-code-actor-mejepa-data-factory.md`
