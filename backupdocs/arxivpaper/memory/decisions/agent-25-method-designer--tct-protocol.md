---
namespace: decisions
agent_id: agent-25-method-designer
timestamp: 2026-04-17T08:00:00Z
status: active
locked: true
supersedes: none
depends_on:
  - memory/context/mission-brief.md
  - memory/decisions/agent-04-construct-definer--tct-definition.md
  - memory/decisions/agent-21-theory-builder--theoretical-framework.md
  - memory/decisions/agent-22-model-architect--master-architecture.md
  - memory/decisions/agent-23-hypothesis-generator--hypotheses.md
  - memory/discoveries/agent-24-causal-simulator--weakness-register.md
  - memory/handoff/agent-24-to-25--counterfactuals-ready.md
downstream_consumers:
  - agent-26-sampling-strategist
  - agent-37-methodology-writer
  - agent-42-adversarial-reviewer
tags: methodology, tct-protocol, phase-1, phase-2, phase-3, reproducibility, locked
answers_questions: [Q06, Q07, Q08]
---

# TCT Protocol — Re-implementable Specification (Phases 1–3)

## 0. Purpose and scope

This file issues the **protocol-level methodology** for Teleological
Constellation Training (TCT) as the manuscript will describe it in
§5. It is written so that an independent team can re-implement the
three phases **without** reading the codebase — inputs, procedure,
outputs, and hyperparameters are each specified.

This is the generic protocol. Case-specific instantiations (training
data, encoders, losses, thresholds, batch sizes) live in
`agent-25-method-designer--case-study-protocols.md`.

**Anchors.** `tct-definition.md` §3.2 (Phase 1/2/3 formal);
`master-architecture.md` Figure 1 Levels 5–8; `theoretical-framework`
Axioms 1–3, P3, P5, Theorems 1–2.

**Hedge register.** V11, V18, V22, R3, R4 carried through every
specification; Bias-05 / Bias-06 carried on subject-specific numbers;
Theme D carried on identity-validation scorer choice.

---

## 1. Phase 1 — Constellation construction

### 1.1 Inputs (required)

| Symbol | Meaning | Type / constraint |
|---|---|---|
| $P$ | Subject (speaker, identity, style) | Fixed for the lifetime of the pipeline |
| $\mathcal{M} = \{m_1, \ldots, m_M\}$ | Modalities that will carry identity constraints | $M \geq 1$; Case 3 has $M=1$; Case 2 targets $M \in \{2, \ldots, 7\}$ |
| $R_m = \{r_{m,1}, \ldots, r_{m,n_m}\}$ | Reference set for $P$ in modality $m$ | Raw samples; $n_m \in [10, 10^2]$ recommended per Axiom 3 (Case 3 uses $n_m = 50$; Case 2 uses $n_m = 2{,}362$ across visual modalities) |
| $\phi_m : \mathcal{X}_m \to \mathbb{R}^{d_m}$ | Frozen embedder for modality $m$ | **Frozen** per V18; must remain untouched through Phases 2 and 3 |
| `embedder_version_hash` | Commit / weight hash of each $\phi_m$ | Recorded to enable reproducibility audit |

### 1.2 Procedure (re-implementable)

For each modality $m \in \mathcal{M}$:

1. **Preprocess** each $r_{m,i} \in R_m$ using the canonical
   preprocessing pipeline of $\phi_m$ (e.g., for WavLM: 16 kHz mono,
   silence trim, loudness normalise; for CLIP: 224×224 centre crop,
   normalise).
2. **Compute projections**:
   $v_{m,i} \;:=\; \phi_m(r_{m,i}) \in \mathbb{R}^{d_m}$ for each
   $i \in \{1, \ldots, n_m\}$.
3. **L2-normalise per-sample** (first normalisation):
   $\tilde v_{m,i} \;:=\; v_{m,i} / \lVert v_{m,i} \rVert_2$.
   (Degenerate guard: if $\lVert v_{m,i} \rVert_2 < \epsilon = 10^{-6}$,
   log and exclude the sample from the mean.)
4. **Average** across reference set:
   $\bar v_m \;:=\; \frac{1}{n_m^{\text{kept}}} \sum_{i} \tilde v_{m,i}$
   where $n_m^{\text{kept}}$ counts non-degenerate samples.
5. **L2-normalise the mean** (second normalisation — yields the
   direction on the unit sphere):
   $c_m \;:=\; \bar v_m / \lVert \bar v_m \rVert_2$.
6. **Stability check (recommended, not required).** Compute
   leave-one-out cosine
   $\text{LOO}_m \;:=\; \text{mean}_i \cos(\phi_m(r_{m,i}), c_m^{(-i)})$
   where $c_m^{(-i)}$ omits the $i$-th sample. Flag if
   $\text{LOO}_m < 0.85$ (centroid is not concentration-stable).
7. **Persist** $c_m$ alongside `embedder_version_hash`,
   `reference_set_manifest` (list of $r_{m,i}$ IDs + SHA-256 hashes),
   $n_m^{\text{kept}}$, and construction timestamp.

### 1.3 Output

- **The constellation** $C_P \;=\; \{(m, c_m) : m \in \mathcal{M}\}$.
  Cardinality $|C_P| = M$. Each $c_m \in \mathbb{S}^{d_m - 1}$ (unit
  sphere).
- **Provenance record** per $c_m$: embedder hash; reference-set
  manifest; $n_m$; $n_m^{\text{kept}}$; LOO-stability if computed;
  timestamp.
- **Lock**: $C_P$ is frozen for the remainder of the pipeline. Any
  downstream operation that mutates $c_m$ violates the protocol
  (Axiom 1 + V18).

### 1.4 Hyperparameters (Phase 1)

| Hyperparameter | Default | Comment |
|---|---|---|
| $\epsilon$ (degenerate-norm guard) | $10^{-6}$ | Prevents division by near-zero norms |
| LOO-stability warning threshold | $0.85$ | Soft warning; does not abort construction |
| `normalisation_order` | double (per-sample, then mean) | **Not tunable**; fixed by `tct-definition.md` §3.2 |

### 1.5 Reproducibility requirements

- Record `embedder_version_hash` for every $\phi_m$ (weight SHA-256).
- Record `reference_set_manifest` as a text file listing each
  $r_{m,i}$ by ID and content hash.
- Record the floating-point precision (fp32 recommended for
  centroid construction to minimise accumulation drift).
- Persist $c_m$ in a format that round-trips losslessly (Parquet +
  fp32 or numpy .npy).

---

## 2. Phase 2 — Constellation-conditioned fine-tuning

### 2.1 Inputs (required)

| Symbol | Meaning | Type / constraint |
|---|---|---|
| $g_\theta$ | Generator to fine-tune | Base model (Case 1: Qwen3-8B; Case 2: EchoMimicV3; Case 3: Qwen3-TTS) |
| $C_P$ | Frozen constellation from Phase 1 | Produced per §1 |
| $\{\phi_m\}_{m \in \mathcal{M}}$ | Same frozen embedders used in Phase 1 | **Identical instances**; V18 + Axiom 1 |
| $\lambda_m \geq 0$ | Per-modality geometric-proximity loss weights | Design choice; see §2.5 |
| $\mathcal{L}_{\text{gen}}$ | Generator base loss | Task-specific (cross-entropy, flow-matching, DPO pair loss, etc.) |
| DDA-enriched training data $D'$ | Training corpus | Optional but compatible |
| Training mode | `full-fine-tune` / `frozen-with-LoRA` / `inference-only` | Case 1 & 2: frozen-with-LoRA; Case 3: inference-only (Phase 2 reduces to best-of-$K$ selection) |

### 2.2 Loss definition — $\mathcal{L}_{\text{TCT}}$

For each training step on a batch $(z, y) \sim D'$ producing
generation $\hat y = g_\theta(z)$:

$$
\mathcal{L}_{\text{TCT}}(\theta) \;=\; \mathcal{L}_{\text{gen}}(\theta; z, y, \hat y) \;+\; \sum_{m \in \mathcal{M}} \lambda_m \bigl(1 \;-\; \cos(\phi_m(\hat y), c_m)\bigr)
$$

where:

- $\mathcal{L}_{\text{gen}}$ is computed in the generator's native
  loss space (e.g., token-level cross-entropy for autoregressive LMs;
  mouth-weighted flow-matching loss for Case 2; DPO preference loss
  for the Shakespeare DPO stage).
- $\phi_m(\hat y)$ applies the **same frozen** $\phi_m$ used in
  Phase 1. For modalities where $\hat y$ needs projection (e.g., Case
  3's speaker modality requires decoding the TTS output to waveform),
  the projection $\hat y \to \text{decoded-waveform} \to \phi_m$ is
  deterministic and outside the autograd graph **only for** $\phi_m$
  (gradients flow into the generator through $\hat y$ but **not** into
  $\phi_m$).
- $\cos(\cdot, \cdot)$ is standard cosine similarity; the output of
  $\phi_m(\hat y)$ is L2-normalised before the inner product with
  $c_m$ (which is already unit by Phase 1).
- **Normalisation convention (locked)**: $1 - \cos(\cdot, \cdot) \in
  [0, 2]$; per-modality loss term is always non-negative.

### 2.3 Frozen-encoder discipline (V18 enforced)

The protocol **fails** the audit if:

1. Any $\phi_m$ receives a gradient update during training (checked
   by hooking `.grad` on $\phi_m$ parameters and asserting
   `param.grad is None` or `param.requires_grad == False` for all
   $\phi_m$ modules).
2. Any $c_m$ is re-computed or mutated during training (checked by
   comparing `hash(c_m)` pre- and post-training).
3. LoRA adapters are attached to $\phi_m$ modules. Adapters may be
   attached to $g_\theta$ modules only (Case 1 & 2).

**Audit hook**: at the end of training, re-run Phase 1 on the
identical reference set with the identical $\phi_m$ weights and
assert $\lVert c_m^{\text{post}} - c_m^{\text{pre}} \rVert_2 < 10^{-5}$.
If this fails, the training run is **invalid** and must be discarded.

### 2.4 Training-time hyperparameters (generic; case studies override)

| Hyperparameter | Default | Notes |
|---|---|---|
| Batch size | 8–32 per modality | Case 2 uses batch 8 with per-clip flow-matching; Case 1 uses batch 16 for SFT, 4 for DPO pairs |
| Learning rate | $10^{-4}$ for LoRA; $10^{-5}$ for full fine-tune | Cosine decay with 5 % warmup |
| Optimiser | AdamW ($\beta_1 = 0.9, \beta_2 = 0.95$) | Weight decay $0.01$ |
| Gradient clipping | 1.0 | Global norm |
| $\lambda_m$ schedule | Constant throughout training | Escalation schedules (ramp-up) are future work |
| Mixed precision | bf16 for generator; fp32 for $\phi_m$ and cosine | Prevents underflow near the centroid |
| Steps | Stopped on validation plateau OR on constellation-alignment plateau (per-modality cosine does not improve for $N_{\text{patience}} = 5$ epochs) | — |
| Checkpointing | Per-epoch + best-validation-loss + best-constellation-alignment | Separate "best" checkpoints for joint optimisation |

### 2.5 Choosing $\lambda_m$ weights

**Design rule.** $\lambda_m$ trades generation quality
($\mathcal{L}_{\text{gen}}$) against identity lock ($\cos(\phi_m(\hat
y), c_m) \to 1$). Practical starting points:

- Case 1 (text style, $M = 1$): $\lambda_{\text{text-style}} \in
  [0.1, 1.0]$. Suggested initial $\lambda = 0.5$.
- Case 2 (visual + speaker, $M \in \{2, \ldots, 7\}$): weighted so the
  speaker-modality term dominates (mouth-weighted flow-matching
  heuristic): $\lambda_{\text{speaker}} = 1.0$,
  $\lambda_{\text{global-visual}} = 0.5$,
  $\lambda_{\text{local-visual}} = 0.3$, $\lambda_{\text{motion}} =
  0.2$, $\lambda_{\text{caption}} = 0.3$. Tune by ablation.
- Case 3 (inference-only): $\lambda_m$ undefined (no gradient step);
  Phase 2 reduces to the best-of-$K$ selection below.

**Anti-collapse**: if $\lambda_m$ is too high, $\hat y$ collapses to
the constellation independent of $z$ (mode collapse). Symptom:
generation loses input-conditioning; cosine to $c_m$ reaches 1.0 on
every input. Mitigation: reduce $\lambda_m$ by ×0.5 and re-train.

### 2.6 Inference-only mode (Case 3)

When `training_mode = inference-only`:

- Phase 2 **does not** update $\theta$.
- Phase 2 instead performs **best-of-$K$ selection** on frozen
  $g_{\theta^*}$:
  1. For each input $z$, draw $K$ candidates
     $\{\hat y^{(k)}\}_{k=1}^{K}$ by varying seed / temperature.
  2. Score each: $s_m^{(k)} = \cos(\phi_m(\hat y^{(k)}), c_m)$.
  3. Pick $k^* = \arg\max_k \sum_m w_m \cdot s_m^{(k)}$ (weighted
     sum of per-modality cosines; weights $w_m \geq 0$ sum to 1).
  4. Forward $\hat y^{(k^*)}$ to Phase 3 (strict guard) for accept /
     reject.

**Case 3 settings**: $K = 12$, $T = 0.3$, $w_{\text{speaker}} = 1.0$
(single-modality).

### 2.7 Output (Phase 2)

- `training_mode = full-fine-tune` or `frozen-with-LoRA`:
  trained parameters $\theta^*$ (or LoRA adapters) with persisted
  checkpoint; training log including per-step
  $\mathcal{L}_{\text{gen}}$, per-step $\sum_m \lambda_m (1 -
  \cos(\phi_m(\hat y), c_m))$, per-step total
  $\mathcal{L}_{\text{TCT}}$, per-epoch validation metrics, and
  per-epoch per-modality cosine statistics (min / mean / max over
  held-out batch).
- `training_mode = inference-only`: unchanged $g_{\theta^*}$;
  Phase-2 "output" is the selected $\hat y^{(k^*)}$ forwarded to
  Phase 3.

---

## 3. Phase 3 — Runtime constellation guard

### 3.1 Inputs (required)

| Symbol | Meaning |
|---|---|
| $g_{\theta^*}$ | Trained generator (from Phase 2) or unchanged backbone (inference-only) |
| $C_P$ | Frozen constellation from Phase 1 |
| $\{\phi_m\}$ | Same frozen embedders |
| $\{\tau_m\}_{m \in \mathcal{M}}$ | Per-modality acceptance thresholds |
| $z$ | Conditioning input (prompt, reference clip, latent seed) |
| $K$ | Sampling budget (maximum attempts) |
| `rejection_policy` | One of: `regenerate`, `fallback`, `escalate` (see §3.4) |

### 3.2 Acceptance predicate (strict conjunction)

$$
G_\tau(\hat y; C_P) \;=\; \mathbb{1}\!\left[\,\forall m \in \mathcal{M} : \cos(\phi_m(\hat y), c_m) \geq \tau_m \,\right]
$$

The predicate is **evaluated per output**, is **deterministic modulo
GPU fp noise**, and requires **no auxiliary learned model** (Theorem 2).

**Relaxed best-of-$K$ variant** (for inference-only backbones or for
high-latency-sensitive deployments):

$$
k^* \;=\; \arg\max_{k=1,\ldots,K} \sum_{m \in \mathcal{M}} w_m \cdot \cos(\phi_m(\hat y^{(k)}), c_m), \qquad \hat y^{*} \;:=\; \hat y^{(k^*)}
$$

Optionally apply the strict predicate to $\hat y^{*}$ post-hoc.

### 3.3 Threshold specification ($\tau_m$) — calibration procedure

Thresholds must be chosen **before** deployment, not tuned on the
evaluation set. The canonical calibration procedure is
**same-session-band floor**:

1. Collect a **calibration set** $S_m$ of authentic samples of $P$ in
   modality $m$, disjoint from $R_m$.
2. Compute $s_{\text{cal}, i} = \cos(\phi_m(s_{m,i}), c_m)$ for all
   $i \in S_m$.
3. Set $\tau_m$ to a lower percentile of this distribution (default:
   the 10th percentile, giving nominal false-reject rate ~10 % on
   authentic samples). Log $\tau_m$, the percentile used, and
   $|S_m|$.
4. **Never** adjust $\tau_m$ to hit a particular evaluation number;
   doing so invalidates the guard as an identity predicate.

**Case-study values (from upstream agents)**:

| Case | Modality | Encoder | Projected / measured $\tau_m$ |
|---|---|---|---|
| 1 (Shakespeare) | text-style | E1 (e5-large-v2) or cross-encoder | T3-projected $[0.80, 0.92]$ (H2c) |
| 2 (Santa) | global visual | CLIP ViT-L/14 / SigLIP | T3-projected $[0.82, 0.92]$ → operating value 0.70 per forensic analysis |
| 2 (Santa) | emotion | emotion encoder | operating value 0.60 |
| 2 (Santa) | speaker (audio) | WavLM-base-plus-sv | operating value 0.80 |
| 2 (Santa) | voice (identity) | WavLM-base-plus-sv | operating value 0.85 |
| 3 (voice) | speaker | WavLM-base-plus-sv | operating value 0.95 (headline); calibration set same-session-band floor |

### 3.4 Rejection handling policies

When $G_\tau(\hat y; C_P) = 0$:

- **`regenerate`** (default for generation-time pipelines).
  1. Increment attempt counter $k \leftarrow k + 1$.
  2. Resample with varied seed / temperature / latent.
  3. If $k < K$: retry. If $k = K$: either fall through to
     `fallback` or report hard-reject with the cosine trace.

- **`fallback`** (used when latency budget is tight or for
  production UX).
  1. Return the relaxed best-of-$K$ argmax $\hat y^{(k^*)}$ even
     though $G_\tau = 0$.
  2. **Must attach** a `warning_flag` to the output metadata:
     "returned_below_threshold; cosines = {s_m(\hat y^{(k^*)})}".
  3. Downstream consumers must respect the flag.

- **`escalate`** (used in safety-critical pipelines).
  1. On hard-reject, route to a human reviewer or to a secondary
     model and log the attempt for audit.
  2. Never return un-flagged.

**Case-study policy matrix**:

| Case | Phase-3 policy | $K$ |
|---|---|---|
| 1 Shakespeare | `regenerate` (LoRA-level reject loop); falls back to `fallback` with warning if $K$ exhausts | $K$ = per-sentence budget (typ. 4) |
| 2 Santa | `regenerate` on training-time proxy; `fallback` with warning at inference | $K = 4$–$12$ |
| 3 voice | `regenerate` → `fallback` on best-of-$K$ argmax regardless (inference-only backbone) | $K = 12$ |

### 3.5 Per-output provenance (required)

Every accepted (or fallback-flagged) $\hat y$ must carry:

- The full cosine trace $\{s_m(\hat y)\}_{m \in \mathcal{M}}$
  (deterministic per Axiom 1).
- `embedder_version_hash` for each $\phi_m$.
- `centroid_hash` for each $c_m$.
- `threshold_set` $\{\tau_m\}$.
- `decision` $\in$ {`accept`, `fallback_flag`, `reject_no_output`}.
- `attempts_used` $k$.
- `policy` (`regenerate` / `fallback` / `escalate`).

This is the **audit record** that makes the guard verifiable
post-hoc.

### 3.6 Phase-3 hyperparameters (generic)

| Hyperparameter | Default | Notes |
|---|---|---|
| $K$ (sampling budget) | 12 | Case 3 default; adjust per latency budget |
| $T$ (sampling temperature) | 0.3–1.0 | Case 3 uses 0.3 to concentrate near the centroid |
| Rejection policy | `fallback` with warning | Default; override to `regenerate` or `escalate` per use case |
| $\tau_m$ calibration percentile | 10th | Of the calibration-set cosine distribution |
| fp precision for cosine | fp32 | Prevents threshold-proximity nondeterminism |

---

## 4. Cross-phase audits (mandatory for reproducibility)

**Invariant checks to run at every handoff (Phase 1 → Phase 2 →
Phase 3)**:

1. **Embedder frozen**: `embedder_version_hash` identical across all
   three phases.
2. **Centroid frozen**: `centroid_hash` identical across Phase 1
   output, Phase 2 loss computation, Phase 3 guard.
3. **Threshold locked pre-deployment**: $\tau_m$ hashed and recorded
   at calibration time; any subsequent change invalidates the run.
4. **Reference-set manifest traceable**: $R_m$ SHA-256 list
   reproducible.

---

## 5. Reproducibility appendix (what to publish)

For every TCT run, publish:

- The reference-set manifest ($R_m$ IDs + content hashes).
- Embedder version hashes for all $\phi_m$.
- Centroid hashes and construction timestamps.
- Training-time hyperparameters table.
- Training log (loss curves per epoch, per-modality cosine
  statistics).
- Calibration-set cosine distribution and chosen $\tau_m$.
- Rejection policy and $K$.
- Per-held-out-sample cosine trace.

Publish as a structured JSON manifest; cross-reference the
manuscript's Case-study tables (§6).

---

## 6. What this protocol does NOT specify

- Case-specific encoder choices, loss functions, and dataset
  preparations (see `case-study-protocols.md`).
- Sampling strategies for reference sets, held-out evaluation sets,
  or calibration sets (see agent-26 sampling-strategist handoff).
- Training-infrastructure details (distributed training, gradient
  accumulation, checkpoint sharding) — deferred to Case-specific
  engineering notes.
- Any form of weight update to $\phi_m$ — explicitly forbidden by
  V18 and Axiom 1.

---

## 7. Weakness-remediation anchors (from W1–W10)

This protocol **mandates** the following remediation steps, which
`weakness-remediation.md` details:

- **W1 (encoder-matched evaluation)**: Phase 3 output must include
  at least one **architecturally-independent** cross-encoder score
  for every evaluation modality that uses selection-on-cosine. For
  Case 3 where the guard is WavLM, a second scorer (ECAPA-TDNN or
  Resemblyzer) must be reported in the paper's §6.3; flagged as
  pending otherwise.
- **W2 (LoRA-only ablation)**: Case 3's Phase-2 output must report
  the **pre-selection** cosine distribution over all $K$ candidates,
  not only the argmax. If ablation is not feasible in time, disclose
  the absence explicitly.
- **W3 (single $n_m$ operating point)**: Axiom 3 language restricted
  to the Case-3 point; curve is named future work.
- **W7 (no $M>1$ evidence)**: §5.4 multi-modal strictness framed as
  derivational under Axiom 2; empirical $M > 1$ evidence named as
  pending Case 2 measurement.

---

## 8. Confidence

**HIGH** that the protocol as specified is re-implementable by an
independent team given the upstream definitions.

**HIGH** that the frozen-encoder discipline (V18) and audit hooks
(§2.3) prevent the most common protocol violation (silent embedder
updates).

**MEDIUM** on the threshold calibration procedure (§3.3) — the
"10th percentile of same-session-band" is one of several valid
calibration choices; alternative approaches (ROC-based, FPR-controlled)
are equally valid and named future work.

## Changelog

- 2026-04-17 — Initial issue by agent-25-method-designer. Locked.
