---
namespace: handoff
agent_id: agent-25-method-designer
timestamp: 2026-04-17T08:40:00Z
status: active
tags: handoff, agent-26, sampling-strategist, methodology-ready, tct-protocol, case-study-protocols
---

# Handoff: Agent-25 (method-designer) → Agent-26 (sampling-strategist)

## Read these files FIRST, in this order

1. `./memory/decisions/agent-25-method-designer--tct-protocol.md` —
   Phases 1–3 specification (inputs, procedure, outputs,
   hyperparameters, audits).
2. `./memory/decisions/agent-25-method-designer--case-study-protocols.md` —
   Case-1 (Shakespeare), Case-2 (Santa), Case-3 (voice) evaluation
   protocols with PENDING flags.
3. `./memory/patterns/agent-25-method-designer--weakness-remediation.md` —
   W1–W10 remediation-or-disclose pairs.
4. `./memory/decisions/agent-23-hypothesis-generator--hypotheses.md` —
   locked hypotheses.
5. `./memory/discoveries/agent-24-causal-simulator--weakness-register.md` —
   10 weaknesses with severity and scope.

## What agent-25 did

1. Read mission brief, agent-04 TCT definition, agent-21 theoretical
   framework, agent-22 master architecture, agent-23 hypothesis
   ledger, and agent-24 weakness register.
2. Designed the generic TCT three-phase protocol at re-implementable
   detail: Phase 1 constellation construction (double normalisation,
   LOO stability check, provenance record), Phase 2 constellation-
   conditioned training (loss definition, frozen-encoder audit hook,
   training-mode discipline, $\lambda_m$ guidance, inference-only
   mode), Phase 3 runtime guard (strict conjunction, relaxed best-of-
   $K$, threshold calibration, three rejection policies, per-output
   provenance).
3. Instantiated the protocol into three case-study evaluation
   protocols:
   - **Case 1 (Shakespeare)**: SFT + DPO on cleaned data, 3-layer
     enforcement stack (model prior + constrained decoding +
     constellation guard), 15/15 FSV inventory, E1 text-style
     centroid, measurement protocol with explicit PENDING T1 flags.
   - **Case 2 (Santa)**: EchoMimicV3 + LoRA (rank 256 / alpha 512 /
     8 target modules, `q_audio` excluded per forensic audit),
     mouth-weighted flow-matching, 2,362-clip training set from 16
     min source, operating thresholds (visual 0.70 / emotion 0.60 /
     speaker 0.80 / voice 0.85), measurement protocol with PENDING
     T1 flags.
   - **Case 3 (voice)**: Qwen3-TTS Full ICL + 50-clip centroid +
     WavLM best-of-$K=12$ at $T=0.3$; **T1 MEASURED** headline
     numbers (mean 0.961 / max 0.975; DNSMOS 3.93; UTMOS 3.012)
     with explicit W1/W2/W3/W4 remediation-or-disclose plan.
4. Produced the weakness remediation plan addressing all ten
   weaknesses, with priority order (W1 > W2 > W7 > W3 > W4 > W5 >
   W6 > W8 > W9 > W10) and paste-ready disclosure language per
   section.
5. Carried all hedges forward: V11 (approximate independence), V18
   (frozen / frozen-with-LoRA), V22/R4 (measured vs. projected), R3
   (pool-relative), Bias-05 / Bias-06 (single-subject, English
   only), Theme D (single-encoder-pair — upgraded to W1 remediation
   requirement).

## What agent-26 (sampling-strategist) must design

The TCT protocol and case-study protocols **mention** sampling
requirements at a high level (reference sets $R_m$, calibration
sets $S_m$, held-out evaluation sets, session-level disjointness).
Agent-26 must design the **sampling plans** at population /
procedure / power-analysis detail:

### 1. Target populations per case

| Case | Population(s) | Eligibility criteria | Stratification |
|---|---|---|---|
| 1 Shakespeare | (a) Authentic Shakespeare passages for $R_{\text{text-style}}$; (b) held-out evaluation prompts; (c) DPO preference-pair sources | (a) Verified authorship; typography normalised; (b) prompts spanning genre (dialogue, soliloquy, sonnet, letter); (c) plausible modern-English rejected completions | By genre (play / sonnet / letter), by character type, by prompt style |
| 2 Santa | (a) 2,362 ClipCannon clips from 16 min source for modality-specific reference subsets; (b) held-out visual scenes; (c) held-out speech prompts | (a) Clip duration, audio-video sync quality, facial-region occlusion; (b) novel poses / backgrounds; (c) unseen sentence content | By clip length, by facial region, by lip-sync quality |
| 3 Voice | (a) 50 reference clips for centroid; (b) 10 held-out sentences; (c) calibration set $S_{\text{speaker}}$ | (a) Length $\geq 3$ s, SNR $\geq 20$ dB, no background speech; (b) unseen content, standard English prosody; (c) disjoint from both (a) and (b) | By utterance type (declarative / interrogative / exclamatory) |

### 2. Sample size with power analysis

- **Reference set**: $n_m$ targets per Axiom 3. Case 3 locked at 50.
  Cases 1 & 2 initial targets 50–100 and 2,362 respectively; agent-26
  may recommend adjusted sizes from power-analysis.
- **Held-out evaluation**: Case 3 locked at 10 sentences. Cases 1
  & 2 need evaluation-set sizes calculated for:
  - Detecting a cross-encoder mean SECS difference $\geq 0.05$ with
    $\alpha = 0.05, \beta = 0.80$.
  - Detecting a per-modality conjunction accept-rate change of
    $\geq 10\%$ with same $\alpha, \beta$.
- **Calibration set $S_m$**: 50–200 authentic samples per modality
  for $\tau_m$ percentile calibration.
- **$n_m$ curve (W3 remediation)**: $\{10, 25, 50, 100\}$ minimum;
  agent-26 decides if $\{5, 10, 25, 50, 100, 200\}$ is feasible.

### 3. Eligibility screening instruments

Concrete instruments / checks needed:

- **Case 1**: passage authenticity verification (cross-reference
  Folger digital text or canonical edition); typography normalisation
  protocol; duplicate-detection against SFT training data (FSV F5
  check already in place).
- **Case 2**: clip-quality check (audio SNR, video frame-rate
  consistency, face-detection confidence); lip-sync quality
  (SyncNet LSE-D baseline); identity-verification (same subject
  across all clips).
- **Case 3**: audio-quality check (SNR $\geq 20$ dB,
  single-speaker); content disjointness (held-out sentences NOT
  in any reference clip's transcript); session audit (W4 — **must
  resolve** whether held-out comes from same recording session as
  reference).

### 4. Disjointness and session-level audit (W4)

Agent-26 must produce a **disjointness matrix** per case documenting:

- Reference set ∩ evaluation set = ∅ (sample-level).
- Reference set ∩ calibration set = ∅.
- Evaluation set ∩ calibration set = ∅.
- Session-level relationship documented (same-session / cross-session
  for Case 3).
- Temporal separation (time-of-recording) noted per clip-based cases.

### 5. Recruitment / access procedures

- **Case 1**: textual sources — open-access (Folger, Project
  Gutenberg). Recruitment of 3 human raters for blind Shakespearean-
  ness judgement (optional). Eligibility: native English speakers;
  literary background preferred; pilot-test calibration.
- **Case 2**: only one subject (the author). Recruitment N/A for
  subject; access to 16-min source confirmed.
- **Case 3**: same subject; same source. No external recruitment.

Agent-26 may decline (4) and (5) for Cases 2 & 3 given the
single-subject constraint, but **must** explicitly document the
Bias-05 / Bias-06 scope (single subject, single language, single
dialect) in the sampling plan.

### 6. Sampling constraints carried forward

| Constraint | Source | Must respect |
|---|---|---|
| $n_m \in [10, 10^2]$ | Axiom 3 | Yes (Cases 1 & 3); Case 2 deviates at 2,362 with disclosure |
| Reference ∩ eval = ∅ | Methodological integrity | Mandatory |
| Session audit | W4 | Mandatory |
| Calibration set disjoint from eval and reference | §3.3 of `tct-protocol.md` | Mandatory |
| Single-subject scope disclosed | Bias-05 / Bias-06 | Mandatory |
| Cross-encoder evaluation hook | W1 | Sampling plan should reserve a second scorer-friendly subset where possible |

### 7. Deliverables expected from agent-26

- `decisions/agent-26-sampling-strategist--sampling-plans.md`:
  per-case sampling plan with populations, eligibility, sample
  sizes, power analysis, recruitment, screening, disjointness.
- `decisions/agent-26-sampling-strategist--disjointness-matrix.md`:
  explicit matrix per case with session-audit resolution (W4 MUST
  be closed here).
- Handoff file to agent-27 (instrument-developer) specifying which
  instruments / questionnaires / scorers are needed.

## Open items for agent-26 (must resolve)

1. **W4 session audit on Case 3**: definitive resolution of whether
   the 10 held-out sentences come from the same recording session as
   the 50 reference clips. This **cannot be deferred**; agent-26 and
   agent-37 must coordinate and document.
2. **Power analysis on Cases 1 & 2 evaluation set sizes**: cap Case
   1's evaluation at $N_{\text{eval,1}} \in [20, 50]$ prompts per
   agent-26's analysis; Case 2 at $N_{\text{eval,2}} \in [20, 100]$
   scenes / prompts.
3. **$n_m$ curve sampling plan** for W3 remediation.
4. **Disjointness documentation** for Case 2 clip-splits (which of
   the 2,362 clips serve as $R_m$ vs. held-out scenes).

## Carried hedges (carry forward unchanged)

- V11 approximate independence on every H1-family mention.
- V18 frozen / frozen-with-LoRA on every $\phi_m$ mention.
- V22 / R4 measured-vs-projected on every number.
- R3 pool-relative on every novelty clause.
- Bias-05 / Bias-06 on single-subject Cases 2 & 3.
- Theme D cross-encoder flag → W1 remediation requirement.
- MP-12 composition-not-invention.

## Dependencies agent-26 pulls forward

- Agent-25 TCT protocol and case-study protocols (this handoff).
- Agent-23 locked hypotheses and evidence map.
- Agent-24 weakness register.
- Agent-22 master architecture (Figure 1 levels and interconnects).
- Agent-21 theoretical framework (Axioms 1–3).
- Agent-14 bias audit (single-subject scope).

## One-paragraph summary for agent-26

> The TCT protocol is specified as re-implementable three-phase
> methodology: Phase 1 constructs constellation $C_P$ from per-
> modality reference sets $R_m$ using double L2-normalisation with a
> stability check and provenance; Phase 2 adds $\sum_m \lambda_m (1 -
> \cos(\phi_m(\hat y), c_m))$ to the generator's base loss under a
> frozen-encoder audit hook (the run is invalid if $\phi_m$ weights
> change); Phase 3 evaluates the strict conjunctive guard $G_\tau$ at
> inference with three rejection policies (regenerate / fallback /
> escalate) and per-output cosine provenance. Case 1 (Shakespeare) is
> SFT+DPO + 3-layer enforcement (model prior + constrained decoding
> + guard) with 15/15 FSV passing and all T1 numbers PENDING. Case 2
> (Santa) is EchoMimicV3 + LoRA (rank 256 / alpha 512 / 8 modules
> excluding q_audio) + mouth-weighted flow-matching + 4-threshold
> operating set (visual 0.70 / emotion 0.60 / speaker 0.80 / voice
> 0.85), trained on 2,362 clips from 16 min source, all T1 PENDING.
> Case 3 (voice) is inference-only Qwen3-TTS Full ICL + 50-clip
> centroid + WavLM best-of-$K=12$ at $T=0.3$ with **T1 MEASURED**
> headline (mean 0.961 / max 0.975 SECS; DNSMOS 3.93; UTMOS 3.012)
> plus explicit W1/W2/W3/W4 remediation-or-disclose plan. Agent-26
> must design sampling plans (populations, eligibility, sample sizes
> with power, recruitment, disjointness matrices) and **resolve W4's
> session audit on Case 3**; the critical constraint is preserving
> reference / calibration / evaluation disjointness across all three
> cases while respecting Bias-05 / Bias-06 single-subject scope on
> Cases 2 and 3.

## Quality-gate status delivered by agent-25

| Gate | Status |
|---|---|
| TCT Phase 1 inputs / procedure / outputs / hyperparams | **PASS** |
| TCT Phase 2 loss / training-mode discipline / $\lambda_m$ guidance | **PASS** |
| TCT Phase 3 guard / thresholds / rejection policies / provenance | **PASS** |
| Frozen-encoder audit hook specified | **PASS** |
| Reproducibility manifest specification | **PASS** |
| Case 1 Shakespeare SFT+DPO+FSV+3-layer stack | **PASS** (numbers T1 PENDING flagged) |
| Case 2 Santa EchoMimicV3 LoRA+flow-matching+thresholds | **PASS** (numbers T1 PENDING flagged) |
| Case 3 voice Qwen3-TTS+ICL+best-of-K=12 with T1 MEASURED | **PASS** |
| W1/W2/W3/W4 remediation-or-disclose plan | **PASS** |
| W5-W10 disclosure language indexed per section | **PASS** |
| All hedges carried (V11, V18, V22, R3, R4, Bias-05/06, Theme D) | **PASS** |
| No new primitives, no new citations | **PASS** |

## Confidence

**HIGH** that the methodology package (TCT protocol + case-study
protocols + weakness remediation) is complete, re-implementable,
and publishable under the mission-brief constraints.

**HIGH** that agent-26 has a tractable sampling design task with
clear constraints and one must-resolve item (W4 session audit).

## Changelog

- 2026-04-17 — Initial handoff by agent-25-method-designer.
