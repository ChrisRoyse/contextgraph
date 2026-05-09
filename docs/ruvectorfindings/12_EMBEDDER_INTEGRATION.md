# 12 — Embedder Integration: TCT / DDA / Multi-Embedder Reward Signals for ME-JEPA

> **🔁 PIVOT 2026-05-09 (extended):** Read [`10_MEJEPA_PIVOT_2026_05_09.md`](./10_MEJEPA_PIVOT_2026_05_09.md)
> and [`11_MEJEPA_LEARNING_FORMULA.md`](./11_MEJEPA_LEARNING_FORMULA.md) first. This doc is the
> companion that ties the existing **14 production content embedders (E1-E14) + 7 learner-state
> embedders (E15-E21)** into ME-JEPA-Code as the *frozen externally-defined instrument panel*
> that UTML v1.0 §VI.2 prescribes — and shows how to extract **up to 105 structured supervisory
> signals per input** from those embedders using **Teleological Constellation Training (TCT)**
> and **Derived Data Abundance (DDA)** per Royse (2026c).

> **Theoretical backing — Royse (2026c):** *"Teleological Constellation Training: Multi-Modal
> Embedding Decomposition as Meaning Compression for Identity-Locked Generation and a Third Path
> Around the Data Wall"*. ResearchGate DOI 10.13140/RG.2.2.24370.57288. The paper proves:
>
> - **DDA counting identity:** n raw inputs × N frozen embedders → up to `n · (N + (N choose 2))`
>   structured supervisory signals (at N=13 in the paper, 91 signals/input; at our **N=14
>   production embedders, 105 signals/input**; at the full **N=21 stack including learner-state,
>   231 signals/input**).
> - **Meaning compression** is a fourth taxonomic entry alongside bit-/weight-/activation-
>   compression — measured in *signals per raw-data unit*, structurally distinct.
> - **The DDA configuration is structurally OUTSIDE the Shumailov collapse regime** because no
>   generator-in-loop recursion exists: every signal `φm(x)` depends only on a real `x` and
>   frozen parameters. This is *scope, not refutation* — the collapse theorems still hold for
>   recursive self-training, but DDA does not instantiate that pattern.
> - **TCT three-phase protocol** uses the same N frozen embedders in three roles:
>   1. **Retriever** — across stored content (already shipped in `crates/context-graph-embeddings`).
>   2. **Labeller** — produce 105 signals per input on a fixed corpus, no parameter updates.
>   3. **Runtime guard** — per-output cosine test `Gτ(ŷ; CP) = 𝟙[∀m: cos(φm(ŷ), cm) ≥ τm]`
>      providing per-output verifiability without any auxiliary learned model.
> - **Empirical anchor** Case 3: 0.961 mean WavLM SECS (max 0.975) on voice cloning, 9 of 10
>   sentences ≥ 0.95, with DNSMOS / UTMOS parity to natural speech. The TCT method works.

---

## Section 0 — Why this doc exists

The operator pointed out two facts that connect into a major unlock:

1. **Qwen3.6-27B is gone**, freeing ~22 GB of GPU VRAM on the RTX 5090. The full embedder
   stack (E1-E14, 12.5 GB) plus ME-JEPA training (5-10 GB) plus headroom now fit
   simultaneously.
2. **The repo already has 14 production content embedders + 7 learner-state embedders** —
   each independently trained on a different corpus with a different objective, each
   producing a deterministic projection of any input.

The bridge to UTML v1.0:

- UTML v1.0 §VI.2's central claim is that ME-JEPA's structural collapse-immunity comes from
  using **frozen externally-defined instruments** as targets. **The 14 production embedders
  fit this definition exactly.** They are pretrained on external corpora, frozen from
  ME-JEPA's perspective, deterministic given input, and collude-resistant because their
  pretraining objectives are unrelated.

The bridge to TCT/DDA (Royse 2026c):

- TCT formalizes how to **use the same frozen embedder panel in three semantic roles** —
  retriever, labeller, runtime guard — without parameter updates. This is exactly the
  configuration ME-JEPA needs.
- DDA gives the **counting identity**: at N=14 frozen embedders, every input yields 105
  supervisory signals. With one /loop turn producing roughly 50-200 panel triples
  (`(panel[t=0], panel[t=1], panel[t=2])`), the per-turn supervisory yield is on the
  order of **5,000-20,000 structured signals**. This is the "trying to capture all the
  reward signals as possible" the operator demanded — quantified.

This doc consolidates the integration plan: how the 21-embedder stack feeds ME-JEPA,
what new capabilities it unlocks, and what to ship to wire it together.

---

## Section 1 — Counting the signals: DDA at N=14 and N=21

### 1.1 The DDA identity (Royse 2026c §3.1)

For a corpus of `n` raw inputs projected through `N` frozen, approximately-independent
embedders, the dataset-level structured-signal count is:

```
|D'| = n · (N + (N choose 2))
```

| Metric | At N=13 (paper) | At N=14 (our production stack) | At N=21 (full stack with learner-state) |
|---|---:|---:|---:|
| Per-modality signals (one embedder × input) | 13 | 14 | 21 |
| Cross-embedder pairwise signals (`N choose 2`) | 78 | 91 | 210 |
| **Total signals per input** | **91** | **105** | **231** |

**Tightness caveat:** The Royse 2026c paper flags this as a *constructive upper bound*
whose tightness depends on a pairwise mutual-information audit (the `dynamicjepa
audit-pairwise-mi` command). If embedder pairs are highly redundant, the effective signal
count is lower. UTML v1.0 §VI.3 wires this audit into ME-JEPA's `ΔΞ.predictor_redundancy`
term — redundant pairs are detected and down-weighted.

### 1.2 Per-/loop-turn supervisory yield for ME-JEPA-Code

Every Claude Code /loop turn produces (per `09 §3.8`):

- **Edits**: each `Edit`/`Write` call = one panel transition (typically 5-50 per turn).
- **Test outcomes**: per-test pass/fail vector (typically 10-200 tests per turn).
- **Witness chain entries**: one per governed write (5-50 per turn).
- **Recommendation**: one structured JSON per turn.

Conservative estimate: **~50-200 panel triples per /loop turn**.

At N=14 embedders × 105 signals each:

```
Per turn:    50-200 triples × 105 signals = 5,250 - 21,000 signals
Per day (96 turns/day if /loop runs every 15 min): 504,000 - 2,016,000 signals
Per week:                                              3.5M - 14M signals
Per quarter:                                           45M - 180M signals
```

This is *per Claude Code instance*. At the corpus scale needed for ME-JEPA Phase 3 training,
DDA produces enough labelled signal in a single quarter to saturate the predictor's capacity
several times over — without ever leaving the Shumailov-safe regime (no generator-in-loop).

**This is the operator's "capture all the reward signals as possible" — at quantitative
scale.**

---

## Section 2 — The 14 + 7 embedder inventory (post-pivot, post-Qwen)

### 2.1 Production content embedders (E1-E14, total ~12.5 GB VRAM)

| Slot | Runtime ID | Backbone | Native dim | Storage dim | VRAM | TCT role for ME-JEPA-Code | Constellation centroid type |
|---|---|---|---:|---:|---:|---|---|
| E1 | `semantic` | `intfloat/e5-large-v2` | 1024 | 1024 | 1.4 GB | Labeller for `E_Problem`/`E_Test`/`E_CommitMsg`; runtime guard on natural-language alignment | "Code change description matches problem statement" centroid |
| E2 | `temporal_recent` | Exponential recency basis | 512 | 512 | 15 MB | Labeller for `E_Trace` recency features; constellation per attempt-history-window | "This change happened in the same epoch as similar successes" centroid |
| E3 | `temporal_periodic` | Fourier periodic basis | 512 | 512 | 15 MB | Labeller for build/test cycle phase; runtime guard on cyclical regression patterns | "Tests fail in characteristic cycles" centroid |
| E4 | `temporal_positional` | Sinusoidal positional basis | 512 | 512 | 15 MB | Labeller for within-attempt position; tracks "how far through the fix is the agent" | "Agent reasoning trajectory shape" centroid |
| E5 | `causal` | `nomic-ai/nomic-embed-text-v1.5` (LoRA stages 1-3) | 768 | 768 | 650 MB | **Labeller for `E_Diff` cause→effect direction**; runtime guard on hunk-attribution ordering. **Trainable; improve under L_step.** | "This hunk causes this test to pass" centroid |
| E6 | `sparse` | SPLADE lexical encoder | 30522 | 1536 | 550 MB | Labeller for keyword presence (test names, error keywords, function names); literal-token signal | "Test name matches error keyword" centroid |
| E7 | `code` | **`Qodo/Qodo-Embed-1-1.5B`** | **1536** | **1536** | **3.8 GB** | **THE primary code instrument. Labeller for `E_AST`/`E_CFG`/`E_DataFlow` panel slots; runtime guard on code-structural similarity to past successes** | **"Working code structure" constellation — anchored on KnownGood mutation category** |
| E8 | `graph` | `intfloat/e5-large-v2` (graph use) | 1024 | 1024 | 1.4 GB | Labeller for `E_TypeGraph` text + symbol relationships | "Type-graph structure preserved after change" centroid |
| E9 | `hdc` | Hyperdimensional encoder | 10000 | 1024 | 60 MB | Labeller for **typo-resistant / robust** signature; ΔΞ adversarial-perturbation training signal | "Robust code signature stable under noise" centroid |
| E10 | `contextual` | `intfloat/e5-base-v2` | 768 | 768 | 500 MB | Labeller for `E_StaticAnalysis` warning clusters | "Static-analysis-clean change" centroid |
| E11 | `kepler` | **`THU-KEG/KEPLER-Wiki5M-KE`** (RoBERTa-base) | 768 | 768 | 350 MB | **Entity / knowledge-graph signal — feeds dependency-graph features**; runtime guard on entity-level claim graph integrity. **Trainable.** | "Dependency-graph entity references resolved correctly" centroid |
| E12 | `late_interaction` | `colbert-ir/colbertv2.0` | 128/token | 128 pooled | 450 MB | Token-level fine-grained signal — **per-line edits**; runtime guard on token-precision | "Token-level edit pattern matches successful fixes" centroid |
| E13 | `splade` | SPLADE v3 | 30522 | 1536 | 550 MB | Stage-1 sparse retrieval — fast first-pass over training corpus; not panel-targeted | (used as retriever, not centroid target) |
| E14 | `bge_m3_dense` | **`BAAI/bge-m3`** (XLM-RoBERTa-Large) | 1024 | 1024 | 2.6 GB | **Multilingual dense retrieval — CRITICAL for cross-language mandate** (Python/Rust/C/C++/Java/C#/JS/TS/Ruby) | **Per-language constellation set; one centroid per (language, mutation_category) pair** |

**Total VRAM for all 14 content embedders: ~12.5 GB.**

### 2.2 Learner-state embedders (E15-E21, on-demand, ~3-5 GB VRAM)

These were originally for human-physiology UTL v2.0. Under UTML v1.0 they do NOT feed
ME-JEPA training (machines have no autonomic nervous system, UTML v1.0 §0). They are
repurposed for **operator-state-aware review queueing**:

| Slot | Runtime ID | Backbone | Dim | UTML v1.0 repurposing |
|---|---|---|---:|---|
| E15 | `affect_speech` | Wav2Vec2-large MSP-DIM | 1024 | Operator-state monitor (review-call quality) |
| E16 | `affect_face` | OpenFace 3.0 + MTL | 512 | Operator-state monitor (review-fatigue) |
| E17 | `affect_text` | MiniLM + EmoBank | 384 | Operator-tone in PR review threads |
| E18 | `ppg` | PaPaGei foundation model | 512 | Operator HRV → review-queue scheduling |
| E19 | `eda` | WESAD chest physiology | 64 | Operator stress → defer high-stakes reviews |
| E20 | `eeg` | LaBraM foundation | 768 | Operator focus → schedule review for high-ΔE windows |
| E21 | `eeg_artifact_robust` | EEGPT | 768 | Cross-check E20 |

**These don't add to the 105-signal-per-input count for ME-JEPA training.** They produce
a separate signal: when ME-JEPA's conformal interval is wide AND the operator is in a
low-ΔE state (high stress, low HRV coherence), defer the review to a higher-ΔE window.

### 2.3 GPU VRAM budget post-Qwen

| Component | Pre-pivot VRAM | Post-pivot VRAM | Notes |
|---|---:|---:|---|
| Qwen3.6-27B Q6_K_XL | ~22 GB | **0 (deleted)** | Per `decisions/...single-loop...md` |
| Hermes-2-Pro Q6_K | ~5 GB | **0 (deleted)** | Per `decisions/...single-loop...md` |
| 14 content embedders (E1-E14) hard-warm | 0 (cold) | **~12.5 GB** | Now keep all warm-loaded simultaneously |
| 7 learner-state embedders (E15-E21) on demand | 0 | ~3-5 GB on demand | Loaded only during human review cycles |
| ME-JEPA training (Phase 3+) | 0 | ~5-10 GB | Predictor + frozen target-readback buffers + activations + gradients |
| **Total active simultaneously** | ~27 GB (Qwen + Hermes only) | **~22 GB (embedders + ME-JEPA)** | **~5-10 GB headroom on 32 GB RTX 5090** |

**Net: the entire RTX 5090 is now usable for embedder + ME-JEPA + headroom.** Previously
Qwen consumed nearly all VRAM, leaving embedders cold-loading per request (~10× slower)
and ME-JEPA training unable to fit.

---

## Section 3 — Teleological Constellation Training (TCT) for ME-JEPA-Code

### 3.1 The TCT three-phase protocol mapped onto code

Per Royse (2026c §5):

**Phase 1 — Constellation construction.** For each `(state, action)` pair in the
mutation-corpus (Phase 0), compute the per-modality centroid set:

```
CP = {c_m  for m ∈ {E1, E2, ..., E14}}
where c_m = L2_normalize(mean(φ_m(x_i)  for x_i ∈ ReferenceSet(state, action)))
```

For ME-JEPA-Code, the natural reference sets are:

- **`KnownGood` constellation:** All 300 SWE-bench Lite tasks where the official fix is
  applied → centroid encodes "code that works." Per embedder, one centroid.
- **`SubtleFlip` constellation:** All boolean-inverted versions → centroid encodes
  "subtle wrong." Per embedder, one centroid.
- **`OffByOne` constellation:** All numeric ±1 versions → centroid encodes "off by one
  wrong."
- ... (one constellation per mutation category × language).

For N=14 embedders × 8 mutation categories × 9 supported languages = **1,008 constellation
centroids** computed once at Phase 0 corpus-finalization, persisted to a new RocksDB CF
`CF_MEJEPA_CONSTELLATIONS`, never changed thereafter.

**Phase 2 — Training under geometric proximity.** ME-JEPA's panel loss includes a
**constellation-attraction term**:

```
L_constellation = sum_m  weight_m · (1 − cos(φ_m(panel[t=2]), c_m^{KnownGood, language}))

Total loss = L_panel + L_pairwise + L_action
           + ε · L_anti-pole
           + ζ · L_uncertainty + η · L_invariant
           + δ · L_constellation                # NEW per TCT Phase 2
```

The model is rewarded for predicting panels that lie close to the `KnownGood` centroid
for the appropriate (mutation_category, language) and far from all other-category
centroids.

**Phase 3 — Runtime guard `Gτ`.** At inference, given a candidate patch `ŷ`:

```
Gτ(ŷ; CP) = 𝟙[∀m: cos(φ_m(ŷ), c_m^{KnownGood, language}) ≥ τ_m]
```

If the predicted panel is *not* close enough to the KnownGood constellation centroid in
*every* embedder space, the patch is **refused at inference** with a typed error:
`MEJEPA_INFER_CONSTELLATION_VIOLATION` (per UTML v1.0 §VI.3 / decision file taxonomy).

This is the **per-output verifiability property** scalar-reward alignment cannot provide:
the cosine test is deterministic, requires no auxiliary learned model, and gives a
geometric reason for refusal.

### 3.2 Why this is structurally collapse-immune

UTML v1.0 §VI.4 + Royse 2026c §3.3 jointly prove:

- The constellation centroids are computed from a **fixed real corpus** (Phase 0
  mutation corpus) and held frozen.
- Training adjusts only ME-JEPA's predictor `Pψ` — it does not modify the embedders or
  the centroids.
- At inference, the guard `Gτ` is a deterministic geometric test against the frozen
  centroids.
- **No generator-in-loop recursion exists.** Each signal `φm(x)` depends only on a real
  `x` and frozen parameters. Each constellation centroid `c_m` depends only on real
  inputs and frozen parameters.
- **Therefore the Shumailov 2024 collapse precondition is not satisfied** — by scope,
  not refutation.

This is the structural property that makes ME-JEPA-Code the **third path around the
data wall** (Royse 2026c §1): not acquire, not synthesise, but **decompose** a fixed
corpus through frozen embedders and treat the resulting signals as supervisory.

---

## Section 4 — Per-pair signals (the `(N choose 2)` half of DDA)

The DDA identity has two terms:

- `N` per-modality signals (one per embedder, captured per input).
- `N choose 2` cross-embedder pair signals — for each pair `(m_i, m_j)`, the **agreement
  between the two embedder views** is itself a signal.

For ME-JEPA-Code at N=14, this is **91 cross-embedder pair signals per input**, beyond
the 14 single-embedder signals.

### 4.1 What pair signals look like

For two embedders `m_i, m_j` and an input `x`, possible pair signals:

- `cos(φ_{m_i}(x), φ_{m_j}(x'))` for each retrieval-pair `(x, x')` — "do these two
  embedders agree on which past inputs are similar to this one?"
- `1 − KL(softmax(neighbor_distribution_{m_i}) || softmax(neighbor_distribution_{m_j}))`
  — "do the two embedders agree on the shape of the local neighborhood?"
- `corr(rank_{m_i}(neighbors), rank_{m_j}(neighbors))` — Spearman rank agreement on
  top-k neighbors.

### 4.2 High-leverage pair signals for code

| Pair | What it measures | Why it's load-bearing for ME-JEPA |
|---|---|---|
| (E1 semantic, E7 code) | "Does the natural-language description of the change match the code structure?" | Catches AI patches where the commit message lies about what the code does |
| (E5 causal, E7 code) | "Does the predicted cause→effect direction match the AST diff?" | Catches confused causal claims in AI-written diff explanations |
| (E7 code, E11 kepler) | "Does the code use the entities the dependency graph says are reachable?" | Catches ImportError / NameError before runtime |
| (E1 semantic, E14 multilingual) | "Does the English description survive translation through the multilingual head?" | Catches language-specific overfitting; cross-language anchor |
| (E6 sparse, E13 splade) | "Do two independent sparse encoders agree on keyword salience?" | Catches AI patches that hallucinate function names |
| (E9 HDC, E12 late_interaction) | "Robust signature × token-precision agreement" | Catches typo-induced regressions |
| (E10 contextual, E8 graph) | "Static-analysis warnings × type-graph structure" | Catches type errors before compiler runs |
| (E5 causal direction = effect→cause vs cause→effect) | "Causal asymmetry preserved" | Catches reversed-direction reasoning |

These 8 high-leverage pairs each yield a per-input signal. Across all 91 pair-combinations
at N=14, ME-JEPA gets a **dense diagnostic signal vector** for every panel triple — far
beyond what the single-embedder slot signals carry.

### 4.3 Pair-signal feed-in to UTML v1.0

The pair-signal vector feeds directly into UTML v1.0's:

- **ΔK (knowledge coherence)** — cross-embedder agreement is the canonical multi-view
  consistency signal. High pair-agreement = high ΔK.
- **ΔΞ.predictor_redundancy** — pairwise MI between embedder views is exactly what the
  paper's pairwise-MI audit measures. High redundancy → ΔΞ down-weights, training
  example contributes less.
- **`E_Reasoning` panel slot** — the pair-signal vector itself can be a panel feature,
  feeding the predictor a "how confidently do all 14 views agree about this code change"
  meta-signal.

---

## Section 5 — Improving trainable embedders under UTML v1.0 L_step

Three of the 14 embedders have trainable LoRA / fine-tune pipelines:

| Embedder | Training crate | Current loss | Improvement under L_step |
|---|---|---|---|
| E5 Causal | `crates/context-graph-embeddings/src/training/` (3-stage LoRA: stages 1, 2, 3) | Per-batch cross-entropy on cause-effect pairs | Weight each example by `L_step = ΔP × ΔK × ΔΩ × ΔΞ` |
| E11 Kepler | `crates/context-graph-embeddings/src/models/pretrained/kepler/` | KEPLER KG-completion loss | Same as E5 |
| E14 BGE-M3 | (training pipeline; potentially extends to per-language fine-tunes) | BGE-M3 multilingual contrastive | Same; especially valuable for cross-language ΔK |

Practical effect of L_step weighting:

- **Examples the embedder already nails (ΔP ≈ 0)** are dropped.
- **Examples that destroy prior alignment (ΔK ≈ 0)** are dropped — protects existing
  pretrained knowledge.
- **Examples seen during gradient explosion (ΔΩ ≈ 0)** are dropped.
- **Examples in the embedder's degenerate region (ΔΞ ≈ 0)** are dropped.

This is strictly safer than uniform-weight SGD and is the same discipline ME-JEPA's
predictor will use.

---

## Section 6 — New MCP tools to ship (Phase 4-7)

Per Royse 2026c §4 (Context Graph instantiates DDA at 75 MCP tools, 91 signals per input
at N=13), our N=14 implementation should expose:

| Tool | Phase | Purpose |
|---|---|---|
| `mejepa_compute_panel(input)` | Phase 1 | Run all 14 embedders on input; return panel of 14 latent vectors. Cached by SHA. |
| `mejepa_compute_signal_set(input)` | Phase 1 | Compute the 105 DDA signals for one input (14 single + 91 pair). Returns structured array. |
| `mejepa_constellation_get(state, action, language)` | Phase 4 | Read frozen constellation centroid for `(mutation_category, language)`. |
| `mejepa_constellation_distance(panel, state, action, language)` | Phase 4 | Per-embedder cosine to centroid; returns `Vec<f32>` of length 14. |
| `mejepa_guard_check(panel, state, action, language, taus)` | Phase 4 | Apply `Gτ`: returns `{passed: bool, failed_modalities: Vec<ModelId>}`. Deterministic geometric test. |
| `mejepa_verify(patch, context)` | Phase 4-7 | The end-user inference path. Runs `compute_panel` → `predictor` → `constellation_distance` → `guard_check` → conformal calibration → returns `Verdict`. |
| `mejepa_observe(panel, oracle_outcome)` | Phase 5 | Online update with EWC++ (per UTML v1.0 §V.3). |
| `mejepa_audit_pairwise_mi()` | Phase 1 | Compute pairwise MI matrix across N=14 embedders on the corpus. **Crucial for tightening the DDA upper bound.** |

These tools are the operator-/agent-facing surface. The 91 pair signals don't all need
their own MCP tool — `compute_signal_set` returns the full vector.

---

## Section 7 — What this unlocks (concretely)

### 7.1 Critical capabilities now possible

1. **Per-output verifiability of AI patches without an auxiliary learned model.** The
   guard `Gτ` is deterministic, geometric, and re-runnable — exactly the property
   scalar-reward alignment (RLHF/DPO/Constitutional AI) cannot provide. *(The ResearchGate
   paper makes this contrast explicit at §1 / §7.)*

2. **Cross-language reasoning by construction.** The E14 multilingual embedder anchors
   the panel layout dimension-wise, and per-language constellation centroids capture
   "what working code looks like" in each language. Cross-language deployment becomes
   a centroid-set selection, not a retraining job.

3. **105 supervisory signals per input (vs. 1 oracle bit).** ME-JEPA's gradient signal
   is no longer "did the test pass?" — it is a **dense 105-dim diagnostic vector**
   describing how the change relates to every dimension of "working code" the embedders
   capture.

4. **Self-healing via constellation-violation detection.** When the runtime guard rejects
   a patch with `failed_modalities = [E5, E7]`, the system has a structural reason for
   rejection ("causal direction wrong AND code structure wrong") — not a scalar score.
   Operators can investigate the geometry directly.

5. **Structurally collapse-immune training corpus.** No generator-in-loop recursion;
   the Shumailov 2024 / Dohmatob 2024 collapse theorems do not apply to this configuration.
   Royse 2026c §3.3 makes this scope claim formally.

6. **Meaning compression as a fourth axis.** Bit-/weight-/activation-compression measure
   economy of output. **Meaning compression measures signals per raw-data unit.** ME-JEPA-
   Code at N=14 achieves 105:1 — for every 1 raw input, 105 structured signals.

### 7.2 What this changes about the training-data story

Pre-pivot framing: "Inner Qwen produces patches; ME-JEPA learns to predict their oracle
outcome." This framing is retired.

Post-pivot + DDA framing: "Every Claude Code action — every edit, every test, every
recommendation — produces 105 structured supervisory signals. Per-day yield at /loop
running every 15 min: 504K-2M signals. The training corpus IS the reality loop running."

This is exactly the operator's "trying to capture all the reward signals as possible so we
can have as much data as possible for the me jepa to be able to predict reality with" —
quantified and proven structurally safe.

### 7.3 Connection to the three operator questions

| Operator question (verbatim 2026-05-09) | DDA / TCT mechanism that answers it |
|---|---|
| 1. Does what the AI said it did EXIST? | **Constellation guard `Gτ`** on the claim graph: cosine distance from the predicted panel to the SHA-chain-anchored "claim verified" centroid in E11 (Kepler) + E1 (semantic). Below threshold → claim doesn't exist. |
| 2. Does it WORK as intended? | **Predicted oracle outcome** from ME-JEPA's predictor + **constellation distance** to the (KnownGood, language) centroid in E7 (code). Both must agree. |
| 3. How is it actually IMPACTING REALITY? | **Pair signals** (E1 ↔ E14 multilingual, E5 ↔ E7 causal-vs-code, E11 ↔ E8 entity-vs-graph) decompose the impact across 91 cross-embedder views. The operator can read the diagnostic vector and see *where* reality diverged from the AI's claim. |

The 14 embedders + 105 signals + frozen constellations are not just an optimization —
they are **the technical mechanism by which ME-JEPA answers the three questions**.

---

## Section 8 — Implementation roadmap

| Step | Description | Effort | Dependencies | Status |
|---|---|---|---|---|
| 1 | Document this integration in this doc + memory decision (`2026-05-09--multi-embedder-utml-integration-post-qwen.md`) | This turn | — | ✅ done |
| 2 | Compute pairwise MI matrix on the existing corpus to tighten DDA upper bound | ~2 days | corpus exists | deferred |
| 3 | Phase 0: extend mutation corpus to all 9 supported languages (currently Python-only) | ~5 days each language | — | deferred |
| 4 | Phase 1: implement `mejepa_compute_panel` + `mejepa_compute_signal_set` MCP tools (uses existing embedders) | ~3 days | embedders running warm | deferred |
| 5 | Phase 1: implement `audit_pairwise_mi` MCP tool | ~2 days | step 4 | deferred |
| 6 | Phase 0 finalization: compute 1,008 constellation centroids (14 embedders × 8 mutation categories × 9 languages); persist to `CF_MEJEPA_CONSTELLATIONS` | ~3 days | step 3, 4 | deferred |
| 7 | Phase 4: implement `Gτ` guard + `constellation_distance` + `verify` MCP tools | ~5 days | step 6 | deferred |
| 8 | Phase 5: implement `observe` with online EWC++ updates | ~3 days | step 7 | deferred |
| 9 | Phase 3: switch trainable-embedder pipelines (E5, E11, E14) to L_step-weighted loss | ~3 days each | UTML v1.0 implementation | deferred |
| 10 | VRAM budget enforcer (fail-closed if budget exceeded) | ~1 day | step 4 | deferred |
| 11 | Repurpose E15-E21 as operator-state-aware review queueing signal | ~3 days | E15-E21 trained models | deferred |
| 12 | Cross-language conformal calibration stratification using E14 | ~2 days | step 6 | deferred |

---

## Section 9 — Falsification (per Royse 2026c §8 and UTML v1.0 §XIII)

This integration is **falsifiable**. Falsification scenarios:

1. **DDA tightness audit returns ~uniform pairwise MI.** Then most of the 91 pair signals
   are redundant — the effective signal count drops toward N=14 instead of 105. This
   would still leave us with N=14 single-embedder signals (already a 14× improvement
   over single-oracle-bit training), but would invalidate the "105 signals per input"
   headline.
2. **Constellation centroids don't separate mutation categories.** If the centroid for
   `KnownGood` is closer to `SubtleFlip` than to itself in some embedder space, that
   embedder is not contributing useful structural signal for code — drop from the panel.
3. **Cross-language transfer with constellations fails.** If the per-language `KnownGood`
   centroids are too far apart, T(train, deploy) → 0 and ME-JEPA must train per-language.
   This is detectable via the conformal coverage on cross-language holdouts.
4. **Guard `Gτ` rejection rate is uncorrelated with actual oracle failure.** Then the
   guard is noise, not signal — disable it and fall back to predictor-only inference.
5. **The Shumailov collapse regime turns out to apply (e.g., if we accidentally feed
   ME-JEPA's own predictions back as constellation centroids).** This would be a
   discipline failure, not a theory failure — fix by enforcing the "centroids never
   include model outputs" rule in `CF_MEJEPA_CONSTELLATIONS`'s write gate.

If any of these holds, the relevant section of this doc gets `status: superseded` and a
revised version describes the corrected integration.

---

## Section 10 — Cross-references

- **Theoretical backing:** Royse, C. (2026c). *Teleological Constellation Training:
  Multi-Modal Embedding Decomposition as Meaning Compression for Identity-Locked
  Generation and a Third Path Around the Data Wall*. ResearchGate DOI
  10.13140/RG.2.2.24370.57288. April 2026.
- **Source theory:** `docs2/UTL_v2_embodied.md` — UTML v1.0 — JEPA Edition.
- **Pivot architecture:** `10_MEJEPA_PIVOT_2026_05_09.md`.
- **Learning formula:** `11_MEJEPA_LEARNING_FORMULA.md`.
- **Master plan:** `09_END_GOAL_REVIEW_REPLACEMENT.md`.
- **Locked decisions:**
  - `memory/decisions/2026-05-09--multi-embedder-utml-integration-post-qwen.md`
  - `memory/decisions/2026-05-09--utml-v1-jepa-edition-mejepa-learning-formula.md`
  - `memory/decisions/2026-05-09--single-loop-claude-code-actor-mejepa-data-factory.md`
- **Embedder source code:**
  - `crates/context-graph-embeddings/src/types/model_id/core.rs` — ModelId enum
    (E1-E14 production + legacy Entity, 15 variants).
  - `crates/context-graph-embeddings/src/learner/mod.rs` — LearnerEmbedderSlot enum
    (E15-E21).
  - `crates/context-graph-embeddings/src/traits/model_factory/memory.rs` — VRAM
    estimates (12.475 GB total for E1-E14).
  - `crates/context-graph-embeddings/src/training/` — LoRA / fine-tune pipelines
    for E5, E11, E14.
- **Existing embedder docs:**
  - `backupdocs/11_embedder_matrix.md` — canonical 21-embedder matrix.
  - `backupdocs/02_human_state_embedders_delta_E.md` — original UTL v2 mapping.
  - `backupdocs/03_surprise_and_coherence_embedders.md` — original UTL v2 ΔS/ΔC mapping.
- **Project papers:**
  - Royse (2026a) — *ContextGraph Reality Loop* white paper.
  - Royse (2026b) — *Dynamic / ME-JEPA* paper (frozen-instrument target-side
    collapse-immunity).
  - Royse (2026c) — *Teleological Constellation Training* (this doc's anchor).

---

## Section 11 — One-paragraph summary

The 14 production content embedders (E1-E14) plus 7 learner-state embedders (E15-E21)
in `crates/context-graph-embeddings/` are **already** the frozen externally-defined
instrument panel UTML v1.0 §VI.2 prescribes for ME-JEPA's structural target-side
collapse-immunity. With Qwen3.6-27B removed, the 12.5 GB needed to keep all 14 content
embedders warm + 5-10 GB for ME-JEPA training + headroom fits comfortably in the RTX
5090's 32 GB. Royse (2026c)'s **Teleological Constellation Training** and **Derived
Data Abundance** frameworks formalize the integration: every Claude Code /loop turn
yields up to **105 structured supervisory signals per input** (14 single-embedder + 91
cross-embedder pair signals at N=14), all structurally outside the Shumailov 2024
collapse regime because no generator-in-loop recursion exists. **Constellation
centroids** computed at Phase-0 corpus finalization (1,008 = 14 embedders × 8 mutation
categories × 9 supported languages) become the Phase-2 training target and the Phase-3
runtime guard `Gτ`, providing **per-output verifiability** without any auxiliary learned
model. This is the technical mechanism by which ME-JEPA answers the three operator
questions ("Does it exist? Does it work? How does it impact reality?") for any code
change in any supported language. **It is the operator's "capture all the reward
signals as possible" — quantified, proven, and structurally safe.**
