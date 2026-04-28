---
namespace: decisions
agent_id: agent-25-method-designer
timestamp: 2026-04-17T08:15:00Z
status: active
locked: true
supersedes: none
depends_on:
  - memory/context/mission-brief.md
  - memory/decisions/agent-04-construct-definer--tct-definition.md
  - memory/decisions/agent-04-construct-definer--identity-locked-definition.md
  - memory/decisions/agent-21-theory-builder--theoretical-framework.md
  - memory/decisions/agent-22-model-architect--master-architecture.md
  - memory/decisions/agent-23-hypothesis-generator--hypotheses.md
  - memory/discoveries/agent-24-causal-simulator--weakness-register.md
  - memory/decisions/agent-25-method-designer--tct-protocol.md
downstream_consumers:
  - agent-26-sampling-strategist
  - agent-27-instrument-developer
  - agent-37-methodology-writer
  - agent-42-adversarial-reviewer
tags: methodology, case-studies, shakespeare, santa, voice-clone, evaluation-protocols, locked
answers_questions: [Q06, Q07, Q08, Q15]
---

# Case-Study Evaluation Protocols (Cases 1–3)

## 0. Purpose and scope

This file specifies **how each case study in §6 is evaluated**: the
training inputs, the exact procedure, the enforcement stack, the
measurement protocol, and the pending-number flags required by
agent-24's weakness register.

Every number reported in the manuscript's §6 is **either** T1-MEASURED
(verifiable, reproducible), **or** T3-PROJECTED (labelled PENDING and
disclosed as prediction). No number is asserted without a tier flag.

**Upstream anchors.** TCT protocol (`tct-protocol.md`); hypothesis
ledger (H2a–H3b); weakness register (W1–W10); mission brief
§"three empirical case studies".

---

## 1. Case 1 — Shakespeare stylistic LoRA (3-layer enforcement stack)

### 1.1 Status

- Pipeline: **SFT + DPO training on cleaned data in progress** at
  time of writing.
- FSV (forensic smoke verification): **15 / 15 prior FSV tests passed**.
- Forensic diagnostic: **training-data echo-bug documented** (fixed).
- **Numbers**: PENDING T1; text-style $\tau$ predicted at
  $[0.80, 0.92]$ (H2c T3).

### 1.2 Training inputs

| Input | Specification |
|---|---|
| Base model | Qwen3-8B (instruction-tuned checkpoint) |
| Source corpus | Cleaned Shakespeare works (plays + sonnets) with punctuation / typography normalised; forensically de-duplicated of training-data echoes |
| Target style | "Shakespearean" — early-modern English verse and prose markers (rhythmic meter, archaic second-person, vocabulary, rhetorical devices) |
| SFT dataset | ≈ $N_{\text{SFT}}$ paired (prompt, Shakespearean-response) exemplars; constructed by style-preserving paraphrase + human review |
| DPO dataset | ≈ $N_{\text{DPO}}$ triples (prompt, chosen, rejected); chosen = Shakespearean completion, rejected = modern English completion of equivalent semantic content |
| Reference set $R_{\text{text-style}}$ | 50–100 authentic Shakespeare passages (plays + sonnets) embedded with E1 (e5-large-v2) to form $c_{\text{text-style}}$ |
| Frozen embedder $\phi_{\text{text-style}}$ | E1 (e5-large-v2) — Context Graph's semantic embedder; **alternative** cross-encoder (ColBERT / BGE reranker) recommended for W1 remediation |

**Sample-count note**: $N_{\text{SFT}}$ and $N_{\text{DPO}}$ exact
values are engineering details for the training team; the manuscript
reports the order of magnitude once training completes. PENDING.

### 1.3 SFT + DPO procedure

**Stage 1 — SFT (Supervised Fine-Tuning)**:

1. Attach LoRA adapters to Qwen3-8B attention + MLP modules (rank
   to-be-chosen by ablation; starting rank $r = 16$, $\alpha = 32$).
2. Train on $N_{\text{SFT}}$ (prompt, target) pairs with token-level
   cross-entropy.
3. Add the TCT geometric-proximity term
   $\lambda_{\text{text-style}}(1 - \cos(\phi_{\text{text-style}}(\hat y), c_{\text{text-style}}))$
   with $\lambda_{\text{text-style}} = 0.5$ (starting value; tune by
   ablation).
4. Train until validation loss plateau or constellation-alignment
   plateau (patience 5 epochs on per-modality cosine).

**Stage 2 — DPO (Direct Preference Optimisation)**:

1. Load best SFT checkpoint.
2. Apply DPO loss on (prompt, chosen, rejected) triples with
   $\beta = 0.1$.
3. Keep the TCT geometric-proximity term active throughout DPO (apply
   to the "chosen" completion).
4. Train until DPO-reward gap plateau.

**Stage 3 — TCT Phase 3 (runtime guard)**: see §1.4.

### 1.4 Three-layer enforcement stack

The Shakespeare pipeline uses the three-layer stack specified in the
codebase:

| Layer | Mechanism | Guard role |
|---|---|---|
| **Layer 1 (model prior)** | SFT + DPO with $\mathcal{L}_{\text{TCT}}$ — generator has been pulled toward $c_{\text{text-style}}$ during training | Probabilistic "mean-time" bias toward Shakespearean style |
| **Layer 2 (constrained decoding)** | Token-level constraints at generation time (vocabulary whitelist / blacklist against modern English markers); logit bias toward archaic forms | Hard-filter obvious anachronisms per token |
| **Layer 3 (constellation guard, TCT Phase 3)** | Per-sentence cosine check: $\cos(\phi_{\text{text-style}}(\hat y), c_{\text{text-style}}) \geq \tau_{\text{text-style}}$ | Final accept / reject per generation; rejection triggers regenerate |

**Rejection loop**: on Layer-3 reject, re-run Layer-2 with different
seed / temperature. On $K = 4$ exhausted, fall back to argmax cosine
and flag `below_threshold`.

### 1.5 15 / 15 FSV test battery

**FSV test suite** (already passed at time of writing):

| # | Test | Purpose | Status |
|---|---|---|---|
| F1 | Base-model identity | Qwen3-8B loads with expected hash | PASS |
| F2 | LoRA-adapter attachment | Adapters attach to correct modules | PASS |
| F3 | SFT-data loader integrity | No off-by-one in batch indexing | PASS |
| F4 | DPO-pair integrity | chosen / rejected labelled correctly | PASS |
| F5 | Training-data echo scan | No exact duplicates between SFT and eval | PASS (after forensic fix) |
| F6 | Constellation construction | $c_{\text{text-style}}$ computed with correct normalisation order | PASS |
| F7 | Frozen-encoder audit | $\phi_{\text{text-style}}$ weights unchanged during training | PASS |
| F8 | Centroid audit | $c_{\text{text-style}}$ unchanged between Phases 1 and 3 | PASS |
| F9 | Layer-1 integration | $\mathcal{L}_{\text{TCT}}$ computed correctly | PASS |
| F10 | Layer-2 constrained decoding | Vocabulary constraints applied per token | PASS |
| F11 | Layer-3 guard | Cosine predicate computed per generation | PASS |
| F12 | Rejection policy | Regenerate / fallback behaviour as spec'd | PASS |
| F13 | Provenance record | Cosine trace attached to each output | PASS |
| F14 | End-to-end smoke | 10 prompts → 10 accepted generations on happy path | PASS |
| F15 | Threshold-proximity stability | Decisions stable across 3 re-runs at fixed seed | PASS |

**Note**: FSV tests verify the **pipeline runs as specified**; they
are **not** evidence of identity-lock quality (which requires
held-out evaluation; §1.6).

### 1.6 Measurement protocol (what §6.1 will report)

- **Primary**: mean $\cos(\phi_{\text{text-style}}(\hat y),
  c_{\text{text-style}})$ across $N_{\text{eval}}$ held-out prompts.
- **Distribution**: histogram of per-sentence cosines; report mean,
  median, 10th percentile, max.
- **Threshold pass-rate**: % of $\hat y$ with cosine $\geq 0.80$ and
  $\geq 0.90$ and $\geq \tau_{\text{text-style}}$ (calibrated).
- **Cross-encoder check (W1 remediation)**: repeat measurement with
  a cross-encoder (ColBERT / BGE-reranker) on the same $\hat y$ set.
  If unavailable in time, **PENDING flag** in §6.1.
- **LoRA-only ablation (W2 remediation)**: report mean cosine from
  the LoRA-only checkpoint **before** Layer-2 / Layer-3 engages.
  If unavailable in time, **PENDING flag** in §6.1.
- **Human evaluation (optional)**: 3 blinded raters judging
  Shakespearean-ness on a Likert scale over 50 prompts. If
  unavailable, name as future work.

### 1.7 PENDING numbers (must be flagged in §6.1)

- Mean text-style cosine on held-out prompts: **T1 PENDING**.
- Threshold pass-rate: **T1 PENDING**.
- Calibrated $\tau_{\text{text-style}}$: **T1 PENDING** (H2c prior is
  $[0.80, 0.92]$).
- Cross-encoder measurement (W1): **T1 PENDING** or disclosed absent.
- LoRA-only ablation (W2): **T1 PENDING** or disclosed absent.

---

## 2. Case 2 — Identity-locked talking-head clone ("Santa")

### 2.1 Status

- Pipeline: **EchoMimicV3 + LoRA**. Training dataset curated from
  **2,362 clips** (ClipCannon-extracted from 16 min single-subject
  interview).
- Forensic diagnostic: **flow-matching sign-bug documented** (fixed;
  see `docs/inspect/08_training_audit.md` in Santa project).
- **Numbers**: PENDING T1 end-to-end; forensic audit complete.

### 2.2 Training inputs

| Input | Specification |
|---|---|
| Base model | EchoMimicV3 (diffusion / flow-matching talking-head model) |
| LoRA configuration (per forensic) | **Rank 256, alpha 512, 8 target modules**; `q_audio` module **excluded** per forensic audit (the sign-bug was localised to this path) |
| Training dataset | **2,362 clips** extracted from a 16-min single-subject interview by ClipCannon's 23-stage DAG |
| Source video | 16 minutes, single subject $P$, one recording session |
| Reference set per modality | Depending on modality slot $m$, a sub-population of the 2,362 clips (or clip-segments) acts as $R_m$ |
| Frozen embedders $\{\phi_m\}$ | Seven modality slots (ClipCannon's 7 modalities) — canonical encoders: CLIP ViT-L/14 or SigLIP (global visual); patch/crop encoder (local visual); optical-flow encoder (motion); WavLM-base-plus-sv (speaker audio); emotion-encoder (emotion); E1-class text encoder (caption); voice-identity encoder (voice) |

### 2.3 Procedure — mouth-weighted flow-matching

**Phase 1 — constellation construction**: for each modality $m$,
build $c_m$ from the appropriate $R_m$ subset of the 2,362 clips
using the normalisation-twice procedure (§1.2 of
`tct-protocol.md`).

**Phase 2 — constellation-conditioned flow-matching**:

1. LoRA adapters attached to EchoMimicV3 (rank 256, alpha 512, 8
   modules, excluding `q_audio`).
2. Base generator loss: mouth-weighted flow-matching loss (the
   pixel-space flow-matching loss is weighted higher on mouth /
   lower-face regions to respect lip-sync priority).
3. TCT term:
   $\sum_{m} \lambda_m (1 - \cos(\phi_m(\hat y), c_m))$
   with $\lambda_{\text{speaker}} = 1.0$,
   $\lambda_{\text{global-visual}} = 0.5$,
   $\lambda_{\text{local-visual}} = 0.3$,
   $\lambda_{\text{motion}} = 0.2$,
   $\lambda_{\text{caption}} = 0.3$ (starting values; tune by
   ablation).
4. Mixed precision: bf16 generator, fp32 embedders.
5. Train until validation plateau or $K$-epoch constellation-
   alignment plateau.

**Phase 3 — guard at inference**:

| Modality $m$ | Threshold $\tau_m$ (operating) |
|---|---|
| Global visual | **0.70** |
| Emotion | **0.60** |
| Speaker (audio) | **0.80** |
| Voice (identity) | **0.85** |

Thresholds above are the **forensic-recommended operating values**;
they are not the paper's final T1 measurements. T3 projections in
H2d remain in place.

**Rejection policy**: `regenerate` on strict conjunction; fall back
to relaxed weighted-sum argmax if $K = 12$ exhausted, with warning
flag attached.

### 2.4 Forensic fixes embedded in the pipeline

- **Flow-matching sign-bug**: was flipping the direction of the
  pixel-space flow. Fixed pre-training. Documented in
  `docs/inspect/08_training_audit.md` of the Santa project.
- **q_audio exclusion**: the audio-query projection exhibited
  training instability when included in the LoRA target set; the
  forensic remedy is to **exclude** `q_audio` from the 8-module LoRA
  target list.

### 2.5 Measurement protocol (what §6.2 will report)

- **Primary (pending)**: per-modality cosine at inference against
  each $c_m$, averaged over $N_{\text{eval}}$ held-out prompts /
  scenes.
- **Conjunction accept-rate**: % of $\hat y$ where $G_\tau = 1$
  under the four thresholds (visual 0.70, emotion 0.60, speaker
  0.80, voice 0.85).
- **Relaxed argmax metric**: weighted-sum cosine of the argmax
  candidate.
- **Lip-sync quality (non-cosine)**: SyncNet score or LSE-D (lip-sync
  error — distance).
- **Visual quality (non-cosine)**: FID on a held-out visual pool;
  subjective 3-rater Likert if feasible.
- **Held-out separation**: held-out prompts and scenes MUST be
  disjoint from the 2,362 training clips (at the session-segment
  level; see agent-24 W4 and agent-26 sampling handoff).

### 2.6 PENDING numbers (must be flagged in §6.2)

- All per-modality cosines: **T1 PENDING**.
- Conjunction accept-rate at operating thresholds: **T1 PENDING**.
- SyncNet / LSE-D: **T1 PENDING**.
- FID on held-out visual: **T1 PENDING**.
- Subjective naturalness: **T1 PENDING** (optional).

The paper must explicitly state in §6.2: "Case 2 is the $M > 1$
projected instantiation; end-to-end measurement is named future
work. Forensic fixes and pipeline integrity are complete."

---

## 3. Case 3 — Voice clone beyond 0.95 WavLM SECS

### 3.1 Status

- **Numbers MEASURED** (T1):
  - **Mean WavLM-SECS = 0.961** on 10 held-out English sentences.
  - **Max WavLM-SECS = 0.975**.
  - **DNSMOS P.808 = 3.93** (parity with real recording 3.93).
  - **UTMOS = 3.012** (parity with real 2.997).
- Protocol: **Qwen3-TTS Full ICL + 50-clip centroid + WavLM-scored
  best-of-12 at T = 0.3**.
- Forensic status: **complete** (SITREP 2026-04-16, white paper
  `tasks/newdata/alldata.md`).

### 3.2 Training inputs (inference-only backbone)

| Input | Specification |
|---|---|
| Base generator $g_{\theta^*}$ | Qwen3-TTS (inference-only; no parameter update in Case 3) |
| ICL strategy | **Full ICL** — all 50 reference clips provided as in-context examples |
| Reference set $R_{\text{speaker}}$ | **50 audio clips** from the 16-min single-subject source (disjoint subset from what the Case-2 training set consumes at the clip level; documented in agent-26 sampling handoff) |
| Frozen embedder | **WavLM-base-plus-sv** (speaker-verification WavLM variant) |
| Held-out eval | **10 unseen English sentences** (disjoint from ICL context and reference set) |

### 3.3 Procedure (inference-only TCT)

**Phase 1 — centroid**: from the 50 reference clips, compute
$c_{\text{speaker}}$ with the normalisation-twice procedure.

**Phase 2 — best-of-$K$ selection (TCT Phase 2 in inference-only mode)**:

1. For each held-out sentence $z$:
   a. Generate $K = 12$ candidate utterances
      $\{\hat y^{(k)}\}_{k=1}^{12}$ by varying seed at fixed
      temperature $T = 0.3$.
   b. Compute $s^{(k)} = \cos(\phi_{\text{speaker}}(\hat y^{(k)}),
      c_{\text{speaker}})$ for each.
   c. Select $k^* = \arg\max_k s^{(k)}$ (single-modality so weighted
      sum reduces to argmax of the one cosine).
2. Optionally apply strict guard $\mathbb{1}[s^{(k^*)} \geq \tau =
   0.95]$ for audit purposes; operational output is $\hat y^{(k^*)}$
   regardless.

**Phase 3 — strict guard**: cosine trace $\{s^{(k^*)}\}$ attached to
every output as provenance.

### 3.4 Measurement protocol (what §6.3 reports — MEASURED)

**Headline numbers (T1 MEASURED)**:

| Metric | Value | Method |
|---|---|---|
| Mean WavLM SECS (over 10 sentences) | **0.961** | Mean of $s^{(k^*)}$ across sentences |
| Max WavLM SECS | **0.975** | Max of $s^{(k^*)}$ |
| 9 / 10 $\geq 0.95$ | **Yes** | Count above threshold |
| DNSMOS P.808 (generated) | **3.93** | DNSMOS framework on decoded WAV |
| DNSMOS P.808 (real recording) | **3.93** | Parity reference |
| UTMOS (generated) | **3.012** | UTMOS naturalness |
| UTMOS (real recording) | **2.997** | Parity reference |

**Audit numbers (T1 MEASURED or explicitly flagged)**:

- Sampling temperature $T = 0.3$ — **measured**.
- Sampling budget $K = 12$ — **measured**.
- Reference-set size $n_m = 50$ — **measured**.
- Held-out-set size 10 sentences — **measured**.

### 3.5 Weakness-remediation measurements required for §6.3

Per agent-24's weakness register (W1, W2, W3, W4), the paper MUST
augment §6.3 with the following, **or** disclose absence clearly:

**W1 — Cross-encoder evaluation (remediation)**:

- Re-score the same 10 held-out sentences (and the same argmax
  candidates) with an **architecturally-independent scorer**:
  ECAPA-TDNN **or** Resemblyzer (preferably both).
- Report: mean / max / 9-of-10-above-threshold under the non-WavLM
  scorer.
- **If measured**: mean SECS under cross-encoder and degree of
  agreement with WavLM. Expected: some drop from 0.961; agreement
  within ±0.10 is supportive.
- **If not measured in time**: paper must flag explicitly:
  > *"The 0.961 mean SECS is an encoder-matched measurement: the
  > guard and evaluator are both WavLM-base-plus-sv. This demonstrates
  > the guard behaves as specified; it does not constitute
  > cross-encoder identity verification. Cross-encoder evaluation
  > (ECAPA-TDNN / Resemblyzer) is named in §8 Limitations as L-01
  > and deferred to future work."*
- Report in §6.3 and §8 L-01.

**W2 — LoRA-only vs. LoRA+TCT ablation (remediation)**:

- Case 3 has **no LoRA** (inference-only backbone). The relevant
  ablation is therefore:
  1. **Pre-selection cosine distribution**: report the SECS of
     **all 12** candidates (not just the argmax) per sentence.
  2. **Mean raw SECS**: average of $s^{(k)}$ across $k = 1..12$ and
     all 10 sentences (pre-selection).
  3. **TCT-delta**: `post_selection_mean − pre_selection_mean`.
  4. **Best-of-$K$ with $\tau = 0$**: argmax without any threshold —
     isolates "sampling-diversity" from "threshold rejection".
- **If measured**: report the distribution histogram and the delta.
- **If not measured in time**: disclose as §8 L-02 and soften the
  §5.4 attribution from "TCT causes" to "TCT verifies and
  selectively accepts".

**W3 — $n_m$ curve (remediation)**:

- Minimal ablation: $n_m \in \{10, 25, 50, 100\}$.
- For each, construct centroid from the corresponding subset,
  run best-of-$K = 12$ at $T = 0.3$, report mean SECS.
- **If measured**: report the 4-point curve in §6.3.
- **If not measured in time**: disclose single-operating-point
  limitation in §8 L-03 and carry RO-10 as named future work.

**W4 — Held-out-session audit (remediation)**:

- Confirm whether the 10 held-out sentences come from:
  - **Same recording session** as the 50 reference clips → disclose
    as L-04.
  - **Cross-session** → disclose as strengthener.
- Inclusion of one cross-session clip is preferred if available.

### 3.6 PENDING numbers (must be flagged)

If remediation cannot land in time:
- Cross-encoder (W1): **PENDING**, flagged as L-01.
- LoRA-only / pre-selection distribution (W2): **PENDING**, flagged
  as L-02.
- $n_m$ curve (W3): **PENDING**, flagged as L-03.
- Session audit (W4): **MANDATORY at writing time**; cannot be
  deferred — agent-26 / agent-37 must resolve.

### 3.7 Scope and hedges carried

Per Bias-05 / Bias-06 and Theme D (agent-14 / agent-17):

- Single subject $P$; single language (English); single encoder pair
  (WavLM guard + WavLM evaluator without remediation).
- Manuscript must state the **scope is single-subject voice clone**;
  no generalisation claim over multi-subject populations (W7) is
  made.
- Architectural independence between WavLM (speaker encoder) and
  the Context Graph E1–E13 panel is acknowledged but is **not** the
  relevant independence property; the relevant property is between
  guard encoder and evaluator encoder (W1) — disclosed explicitly
  per §3.5.

---

## 4. Summary — Case-level tier / pending matrix

| Case | TCT mode | $M$ | Reference $n_m$ | Headline metric | Tier |
|---|---|---|---|---|---|
| 1 Shakespeare | frozen-with-LoRA (SFT + DPO) | 1 (text-style) | 50–100 (prior) | text-style cosine vs. E1 centroid | **T1 PENDING** |
| 2 Santa | frozen-with-LoRA (mouth-weighted flow-matching) | 2–7 (visual + emotion + speaker + voice + more) | 2,362 clips (source; distributed per modality) | conjunction accept-rate at (visual 0.70, emotion 0.60, speaker 0.80, voice 0.85) | **T1 PENDING** |
| 3 Voice | inference-only (best-of-$K=12$, $T=0.3$) | 1 (speaker) | 50 | mean WavLM SECS | **T1 MEASURED = 0.961** (mean), **0.975** (max); DNSMOS 3.93; UTMOS 3.012 |

**Pending / remediation priority** (for agent-26 + agent-27 + agent-37):

1. Case 3 cross-encoder run (W1). Highest ROI.
2. Case 3 pre-selection distribution (W2).
3. Case 3 $n_m$ curve (W3).
4. Case 3 session-audit disclosure (W4). **Mandatory**.
5. Case 1 end-to-end T1 measurement (if training completes before
   submission).
6. Case 2 end-to-end T1 measurement (stretch).

---

## 5. Confidence

**HIGH** that the protocol specifications for Case 1 (SFT+DPO, 3-layer
stack, 15/15 FSV inventory) and Case 2 (LoRA rank 256/alpha 512/8
modules excluding q_audio, mouth-weighted flow-matching, 4-threshold
operating set) are sufficient for independent re-implementation.

**HIGH** that the Case 3 measurement protocol faithfully reproduces
the SITREP + white-paper numbers (0.961 mean / 0.975 max; DNSMOS 3.93;
UTMOS 3.012) with explicit W1–W4 disclosures.

**MEDIUM** on whether W1–W3 remediation runs will land before
submission; protocol specifies both the remediation procedure AND
the fallback disclosure path so the paper does not depend on the
remediation landing.

## Changelog

- 2026-04-17 — Initial issue by agent-25-method-designer. Locked.
