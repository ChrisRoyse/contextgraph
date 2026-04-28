# 5. Teleological Constellation Training

This section specifies the Teleological Constellation Training (TCT) method at the level of detail an independent team could re-implement without reading the codebase. Section 5.1 defines the constellation object $C_P$. Sections 5.2–5.4 specify Phases 1, 2, and 3. Section 5.5 describes the three-layer enforcement stack used for text generation (Case 1). Section 5.6 records the case-specific protocol instantiations that Section 6 reports results against, flagging measured-versus-projected numbers at every step.

Three hedges carry throughout. (i) "Frozen" (V18) means an embedder's parameters receive no gradient update for the duration of the pipeline; LoRA adapters attach only to the generator, never to the embedder panel. (ii) The Phase-3 guard provides **per-output verifiability relative to the chosen centroid panel** — not verifiability against any external identity criterion unavailable to the panel. (iii) Every measured number carries its measurement date, pipeline version hash, and tier flag; every projected number is explicitly labelled and paired with a pending T1 measurement.

## 5.1 The constellation $C_P$

Let $P$ be a subject — a speaker, a visual identity, an authorial style. Let $\mathcal{M} = \{m_1, \ldots, m_M\}$ be a finite set of **modalities** along which identity is asserted. For each $m$, let $R_m = \{r_{m,1}, \ldots, r_{m,n_m}\}$ be a **reference set** of raw samples of $P$ in modality $m$, and let $\phi_m : \mathcal{X}_m \to \mathbb{R}^{d_m}$ be a **frozen embedder** for that modality. The **constellation** for $P$ is

$$
C_P \;=\; \bigl\{(m, c_m) : m \in \mathcal{M}\bigr\}, \qquad |C_P| \;=\; \sum_{m \in \mathcal{M}} n_m, \qquad (3)
$$

where each $c_m \in \mathbb{S}^{d_m - 1}$ is a frozen unit-norm centroid in the output space of $\phi_m$, constructed by the aggregation in Section 5.2. The $|C_P| = \sum_m n_m$ notation emphasises that the constellation's informational weight scales with the total reference-set size aggregated across modalities.

The constellation is the **single paper-level object** the rest of the method is defined against. DDA (Section 3.1) produces the per-embedder projections from which each $c_m$ is constructed; Phase 2 targets $C_P$ as a geometric loss anchor; Phase 3 verifies $C_P$ as a per-output runtime predicate.

## 5.2 Phase 1 — Constellation construction

Phase 1 inputs the subject $P$, the modality set $\mathcal{M}$, the reference set $R_m$ and the frozen embedder $\phi_m$ for each $m$. It outputs the frozen constellation $C_P$ together with a provenance record.

**Procedure (per modality $m$).** (1) **Preprocess** each $r_{m,i}$ using the canonical pipeline of $\phi_m$ (WavLM: 16 kHz mono, silence trim, loudness normalise; CLIP/SigLIP: 224×224 centre crop, standardised normalisation). (2) **Compute projections** $v_{m,i} := \phi_m(r_{m,i}) \in \mathbb{R}^{d_m}$. (3) **Per-sample L2-normalise** $\tilde v_{m,i} := v_{m,i} / \lVert v_{m,i} \rVert_2$; exclude any sample with $\lVert v_{m,i} \rVert_2 < \epsilon = 10^{-6}$. (4) **Average** $\bar v_m := (1/n_m^{\mathrm{kept}}) \sum_i \tilde v_{m,i}$. (5) **L2-normalise the mean** $c_m := \bar v_m / \lVert \bar v_m \rVert_2$. (6) **Stability check (recommended).** Compute the leave-one-out cosine $\mathrm{LOO}_m$; flag if $< 0.85$. (7) **Persist** $c_m$ with the embedder weight SHA-256, reference-set manifest, $n_m$, $n_m^{\mathrm{kept}}$, LOO stability, and timestamp.

The double normalisation (per-sample then mean) is not tunable; it prevents the centroid drifting toward high-norm outliers.

**Aggregation choices and failure modes.** The mean-of-normalised default is consistent with single-reference, weighted-mean, and trimmed-mean variants so long as step 5 produces a unit vector. The degenerate failure mode is a reference set whose samples lie in nearly orthogonal cone sectors — audio clips across radically different acoustic environments, or visual clips spanning multiple subjects — so the mean direction carries little signal; the LOO check is a cheap diagnostic. Section 6.3 reports $n_m = 50$ for measured Case 3; Section 6.2 projects $n_m$ at the clip-subset level for multi-modal Case 2.

## 5.3 Phase 2 — Constellation-conditioned training

Phase 2 inputs the generator $g_\theta$, the frozen constellation $C_P$, the same frozen embedder panel $\{\phi_m\}$ used in Phase 1, per-modality loss weights $\lambda_m \geq 0$, and the generator's base loss $\mathcal{L}_{\mathrm{gen}}$. It outputs trained parameters $\theta^{*}$ (or, in inference-only mode, the unchanged generator plus a best-of-$K$ selection rule).

**Loss.** For each step on a batch $(z, y)$ producing generation $\hat y = g_\theta(z)$:

$$
\mathcal{L}_{\mathrm{TCT}}(\theta) \;=\; \mathcal{L}_{\mathrm{gen}}(\theta; z, y, \hat y) \;+\; \sum_{m \in \mathcal{M}} \lambda_m \bigl(1 - \cos(\phi_m(\hat y), c_m)\bigr). \qquad (4)
$$

$\mathcal{L}_{\mathrm{gen}}$ is the generator's native loss — token-level cross-entropy for autoregressive language models, mouth-weighted flow-matching for EchoMimicV3 in Case 2, DPO preference loss (Rafailov et al., 2023) for the Case-1 DPO stage. Because $\phi_m$ is frozen, $1 - \cos(\phi_m(\hat y), c_m) \in [0, 2]$; gradients flow through $\hat y$ into $\theta$ but not into $\phi_m$ or $c_m$. Cosine is computed in fp32 against a bf16 generator forward to prevent threshold-proximity nondeterminism; $\phi_m(\hat y)$ is L2-normalised before the inner product with the unit-norm centroid.

**Freeze policy.** Three invariants are checked every run: (i) **embedders frozen** — `param.requires_grad == False` on every $\phi_m$; (ii) **centroids frozen** — $\mathrm{hash}(c_m)$ pre- and post-training match, and a post-training re-run of Phase 1 on identical inputs satisfies $\lVert c_m^{\mathrm{post}} - c_m^{\mathrm{pre}} \rVert_2 < 10^{-5}$; (iii) **LoRA restricted to the generator** — adapters attach to $g_\theta$ (rank 16, $\alpha = 32$ on Qwen3-8B in Case 1; rank 256, $\alpha = 512$ on eight EchoMimicV3 modules excluding `q_audio` in Case 2), never to $\phi_m$. Any failure invalidates the run.

**Hyperparameters.** Mixed precision bf16 generator, fp32 embedders and cosine; AdamW ($\beta_1 = 0.9$, $\beta_2 = 0.95$, weight decay 0.01, gradient clip 1.0); learning rate $10^{-4}$ for LoRA or $10^{-5}$ for full fine-tune, cosine decay with 5 % warmup; halt on validation-loss or per-modality-cosine plateau (patience 5 epochs); separate "best validation" and "best alignment" checkpoints; constant $\lambda_m$ schedule.

**Anti-collapse.** If $\lambda_m$ is too high, the generator collapses to the constellation independent of conditioning: every generation reaches cosine 1.0 while $\mathcal{L}_{\mathrm{gen}}$ stops decreasing. Mitigation: halve $\lambda_m$ and restart. Starting values are given in Section 5.6 per case.

**Inference-only mode.** When the backbone is not fine-tunable — Case 3's Qwen3-TTS is used inference-only — Phase 2 reduces to **best-of-$K$ selection**. For each conditioning input $z$, draw $K$ candidates by varying seed at fixed temperature, score each by $s_m^{(k)} = \cos(\phi_m(\hat y^{(k)}), c_m)$, and select $k^{*} = \arg\max_k \sum_m w_m s_m^{(k)}$. The selected $\hat y^{(k^{*})}$ is forwarded to the Phase 3 guard.

## 5.4 Phase 3 — The runtime constellation guard $G_\tau$

Phase 3 inputs the trained (or inference-only) generator, the frozen $C_P$ and $\{\phi_m\}$, per-modality thresholds $\{\tau_m\}$, a conditioning input $z$, a sampling budget $K$, and a rejection policy. It outputs an accepted, fallback-flagged, or hard-rejected generation.

**Acceptance predicate (strict conjunction).**

$$
G_\tau(\hat y; C_P) \;=\; \mathbb{1}\!\left[\,\forall m \in \mathcal{M} : \cos(\phi_m(\hat y), c_m) \geq \tau_m\,\right]. \qquad (5)
$$

The predicate is **per output** and is verifiable **relative to the chosen embedder panel $\Phi$ and centroid set $C_P$**; cross-encoder transportability is a separate question this predicate does not resolve. Its cost is $O(M \cdot d_{\max})$ for the cosine comparisons (plus embedder forward passes, one per modality per candidate). It requires **no auxiliary learned model**: $\phi_m$ is frozen by Axiom 1, $c_m$ by Phase 1, $\tau_m$ is a scalar calibrated pre-deployment against a disjoint calibration set (see threshold calibration below). The predicate is deterministic given the embedder, centroid, and candidate tensors; in mixed-precision (bf16 generator, fp32 embedder, fp32 cosine) practice, the threshold margin is subject to $O(10^{-4})$-level floating-point variance, and the audit record logs the numerically-resolved score to support post-hoc replay.

We call the resulting property **per-output verifiability relative to the chosen centroid panel**. What is verified is that $\hat y$ satisfies the specified cosine predicate against the frozen $C_P$ under $\Phi$; what is not verified is that $\hat y$ is correct under any external identity criterion unavailable to the panel. An encoder-matched measurement — using $\phi_m$ as both Phase-3 guard and Section-6 evaluator — demonstrates that the guard behaves as specified; it does not constitute cross-encoder identity verification (Sections 6.3 and 8).

**Relaxed best-of-$K$ variant.** For inference-only backbones or latency-sensitive deployments the guard can be operated as a ranking: $k^{*} = \arg\max_k \sum_m w_m \cos(\phi_m(\hat y^{(k)}), c_m)$, returning $\hat y^{*} := \hat y^{(k^{*})}$. The strict predicate (5) can then be applied post-hoc for audit. Case 3 uses both: select by argmax of the single-modality cosine across $K = 12$ at $T = 0.3$, and record the cosine trace regardless of whether it clears $\tau = 0.95$.

**Threshold calibration.** Thresholds are chosen **before** deployment, not tuned on the evaluation set. The canonical procedure is the **same-session-band floor**: collect a calibration set $S_m$ of authentic samples of $P$ disjoint from $R_m$; compute $s_{\mathrm{cal}, i} = \cos(\phi_m(s_{m,i}), c_m)$; set $\tau_m$ to a lower percentile (default: tenth, nominal 10 % false-reject). The chosen $\tau_m$, the percentile, and $|S_m|$ are logged. Any subsequent adjustment of $\tau_m$ to hit an evaluation number invalidates the guard.

**Rejection-handling.** Under `regenerate`, the generator is resampled with varied seed or temperature; on reaching budget $K$ the policy falls through to `fallback`. Under `fallback`, the relaxed argmax is returned with a `warning_flag` attaching the cosine trace. Under `escalate`, a hard reject routes to a human reviewer. Every returned output carries the cosine trace, embedder weight hash, centroid hash, threshold set, decision, attempt count, and policy — the audit record that makes the guard post-hoc verifiable.

## 5.5 Three-layer enforcement stack for text generation

Text generation (Case 1) composes three layers of increasing granularity and strictness.

**Layer 1 — Trained prior (SFT + DPO with $\mathcal{L}_{\mathrm{TCT}}$).** Supervised fine-tuning on paired (prompt, target-style response) exemplars followed by DPO on (prompt, chosen, rejected) triples, with the geometric-proximity term (4) active throughout. Layer 1 is a **probabilistic** mechanism: it biases the generator's output distribution toward the target style in the mean but does not enforce any per-output property.

**Layer 2 — Decoding-time constraints.** Token-level vocabulary whitelist/blacklist against modality-appropriate markers (for Shakespearean style: archaic-form preferences, modern-English-marker suppression), plus logit biases. Layer 2 is a **per-token hard filter**: it makes some generations impossible token-by-token, stronger than Layer 1 but narrower — it does not see the full generation as a semantic object.

**Layer 3 — Runtime guard $G_\tau$ (Section 5.4).** The full generation $\hat y$ is evaluated against the frozen text-style centroid by predicate (5). Layer 3 is **per-output** and **semantic**: the only one of the three whose decisions depend on the frozen centroid and are verifiable post-hoc against the audit record. Rejection triggers a bounded regeneration loop ($K = 4$ per sentence) and, on budget exhaustion, a fallback with `below_threshold` warning.

The layers interact additively. Layer 1 raises the proportion of candidates Layer 3 accepts; Layer 2 cuts obvious anachronisms before Layer 3 sees them. Dropping Layer 3 removes per-output verifiability; dropping Layer 1 or 2 raises Layer 3's regeneration load but does not remove the per-output guarantee against the chosen panel. The Case-1 Shakespeare LoRA on Gemma-4-E4B-it was trained end-to-end with this stack on the forensically-cleaned corpus and passed 5/5 Full State Verification on 2026-04-17 (§6.1.1); the trained LoRA is published at `https://huggingface.co/cabdru/shakespeare-lora-gemma4`.

## 5.6 Case-specific protocols

We record here only the protocol-level parameters Section 6 reports results against; we flag measured-versus-projected status at every step.

**Case 1 — Shakespeare stylistic LoRA (MEASURED end-to-end 2026-04-17).** Base: Gemma-4-E4B-it, NF4-quantised at inference. LoRA rank 256, $\alpha = 512$ across 258 target modules (`q_proj`, `k_proj`, `v_proj`, `o_proj`, `gate_proj`, `up_proj`, `down_proj` across all transformer layers); frozen-with-LoRA mode. Modality: $\mathcal{M} = \{\mathrm{text\text{-}style}\}$ scored against a 9-embedder STYLE_CENTROID drawn from Context Graph's thirteen-embedder panel (E2, E3, E5, E11 omitted as orthogonal to style). Reference set: 2,698 Shakespeare memories; 7,925 cleaned SFT examples and 11,724 cleaned DPO pairs derived from them. Phase 2 uses the SFT + DPO stack: SFT for 3 epochs at learning rate $2 \times 10^{-4}$, cosine schedule, per-device batch 1 with gradient accumulation 32 (effective batch 32), bf16, `adamw_8bit`, max sequence length 512; DPO for 1 epoch at learning rate $5 \times 10^{-5}$, $\beta = 0.1$, `ref_model=None` (implicit reference) to fit in VRAM. Final SFT loss 1.12 with 98.9% token accuracy; final DPO loss 0.47 with 98.0% `rewards/accuracies` and `rewards/margins` 43.62; total training wall-clock approximately 7.5 h on a single NVIDIA RTX 5090. Phase 3 is the three-layer enforcement stack of §5.5 (trained prior + constrained decoder + 13-embedder audit). Calibrated operating threshold $\tau_{\mathrm{text\text{-}style}}$ is pending the controlled held-out cosine evaluation named in §6.1.5; the three-layer stack passed 5/5 Full State Verification via Playwright against the live dashboard and inference server. The trained LoRA is published at `https://huggingface.co/cabdru/shakespeare-lora-gemma4`.

**Case 2 — Identity-locked talking-head clone (T3 projected end-to-end; forensic integrity complete).** Base: EchoMimicV3. LoRA rank 256, $\alpha = 512$ on eight target modules, excluding `q_audio`. Source: 16 minutes of a single-subject interview, from which ClipCannon's 23-stage DAG curates 2,362 training clips. Modalities (up to seven): global visual (CLIP ViT-L/14 or SigLIP), local visual (patch/crop), motion (optical flow), speaker audio (WavLM-base-plus-sv), emotion (speech-emotion encoder), caption (E1-class text), voice identity (architecturally distinct from WavLM — Resemblyzer or pyannote/embedding — to avoid collapsing conjunction strictness). Phase 2 uses mouth-weighted flow-matching (Lipman et al., 2023) with $\lambda_{\mathrm{speaker}} = 1.0$, $\lambda_{\mathrm{global\text{-}visual}} = 0.5$, $\lambda_{\mathrm{local\text{-}visual}} = 0.3$, $\lambda_{\mathrm{motion}} = 0.2$, $\lambda_{\mathrm{caption}} = 0.3$. Phase 3 operating thresholds (T3 projected, forensic-recommended): visual 0.70, emotion 0.60, speaker 0.80, voice 0.85; rejection `regenerate` with $K \in \{4, 12\}$, fallback with warning on budget exhaustion. Two forensic fixes are embedded in the pipeline: a flow-matching sign-bug correction and exclusion of `q_audio` from the LoRA target set. Both are reported as engineering narrative, not randomised ablations; controlled re-runs isolating them are future work.

**Case 3 — Voice cloning with a 50-clip audio centroid (T1 measured; Section 6.3 headline).** Base: Qwen3-TTS, **inference-only**; Phase 2 reduces to best-of-$K$ selection. Conditioning: **Full In-Context Learning (ICL)** — all 50 reference clips provided as in-context examples. Reference set: $n_m = 50$ audio clips from a 16-minute single-subject source (a subset disjoint at the clip level from Case 2's training partition). Frozen embedder: $\phi_{\mathrm{speaker}}$ = **WavLM-base-plus-sv** (Chen et al., 2022). Modality: $\mathcal{M} = \{\mathrm{speaker}\}$, single-modality so the weighted sum reduces to argmax. Phase 2 selection: $K = 12$ candidates per sentence at $T = 0.3$ (measured); each scored by $s^{(k)} = \cos(\phi_{\mathrm{speaker}}(\hat y^{(k)}), c_{\mathrm{speaker}})$; $k^{*} = \arg\max_k s^{(k)}$. Phase 3 threshold $\tau_{\mathrm{speaker}} = 0.95$ calibrated to the same-session-band floor on a disjoint calibration set; the operational output is the argmax regardless of whether the strict predicate clears $\tau$, and the cosine trace is attached as provenance.

The measured Case-3 protocol is therefore Full ICL + 50-clip centroid + WavLM-scored best-of-12 at $T = 0.3$, with $\tau = 0.95$ recorded for audit. Section 6.3 reports the resulting headline under the encoder-matched scope (guard and evaluator both WavLM-base-plus-sv): **mean WavLM SECS 0.961, max 0.975 on 10 held-out English sentences for one speaker**, T1 measured at date 2026-04-16, pipeline version `qwen3-tts-foricl-v1`. Parity observations against real recordings (descriptive equality within evaluator noise, $N=14$): DNSMOS P.808 clone 3.93 vs real 3.93; UTMOS clone 3.012 vs real 2.997. A within-WavLM-family retrospective 95 % bootstrap CI on the mean ($N = 10$, 10,000 resamples, fixed seed 20260416, percentile method) is $[0.951, 0.971]$ — retrospective, not power-based, descriptive summary of the ten observed values.

Three companion measurements are instrument-ready and named as pending: cross-encoder triangulation with ECAPA-TDNN (Desplanques et al., 2020) or Resemblyzer (addresses the encoder-matched scope); the pre-selection cosine distribution over all 120 candidates (isolates TCT-delta from base-model sampling); and the reference-set-size curve at $n_m \in \{10, 25, 50, 100\}$ (anchors sample complexity at more than one operating point). Section 8 carries the paste-ready disclosures.

Table 2 summarises the case-level protocol. Bold values are T1 measured under the encoder-matched protocol; italicised values are T3 projected.

| Case | TCT mode | $M$ | Modalities | $n_m$ | Headline operating point | Tier |
|---|---|---|---|---|---|---|
| 1 Shakespeare | frozen-with-LoRA (Gemma-4-E4B-it; SFT + DPO; rank 256, $\alpha = 512$) | 1 | text-style (9-embedder STYLE_CENTROID) | 2,698 memories $\to$ 7,925 SFT + 11,724 DPO | **SFT loss 1.12, token-acc 98.9%; DPO rewards-acc 98.0%, margins 43.62; 5/5 FSV PASS** | **measured 2026-04-17** |
| 2 Santa | frozen-with-LoRA (mouth-weighted flow-matching) | up to 7 | visual/local/motion/speaker/emotion/caption/voice | 2,362 source clips | *$\tau = (0.70, 0.60, 0.80, 0.85, \ldots)$* | *projected end-to-end*; forensic integrity complete |
| 3 Voice | inference-only (best-of-$K$) | 1 | speaker (WavLM-base-plus-sv) | **50** | **$K = 12$, $T = 0.3$, $\tau = 0.95$**; mean SECS **0.961**, max **0.975** on **10** sentences (encoder-matched) | **measured 2026-04-16** |

*Table 2. Case-level TCT protocol instantiations. Cross-encoder triangulation, pre-selection distribution, and $n_m$ curve for Case 3 are instrument-ready and pending.*

Section 6 reports the measured Case-3 numbers with full within-WavLM-family scope and the projected Case-1 and Case-2 status; Section 7 discusses the three-role composition, the geometric-over-scalar per-output verifiability, and the sample-complexity reading of the one measured operating point; Section 8 catalogues the limitations.
