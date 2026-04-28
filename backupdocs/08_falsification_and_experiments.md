# 08 — Falsification and Experiments

**Purpose.** Seven pre-registered experiments that would either confirm UTL v2's core predictions in measurable terms or falsify them. Each experiment is paired to a specific theorem in UTL v2, has a clean operational definition, names the embedders that produce the measurements, and specifies what would constitute a falsifying result.

UTL v2 already commits (Part XI.6) to three falsifying conditions. This file develops them into runnable experimental designs, adds three new ones for the AI side, and adds one AI-native education experiment from Chris's "answer is no longer a metric" / "100 holes" doctrine.

---

## 1. Why falsification matters here

A theory that cannot be falsified is not a scientific theory. UTL v2's commitment to embedder-grounded measurement is what gives it falsifiability — every term is observable, every prediction is quantitative.

The standard to meet:

- **Pre-registered hypothesis.** Stated in advance with directionality and effect-size band.
- **Power-calculated cohort size.** Reach 80% power on the smallest plausibly meaningful effect.
- **Frozen analysis plan.** Statistics pre-specified; no garden-of-forking-paths.
- **Open data + open code.** Replication possible.
- **Clear falsifier.** A specific result that would force revision of the framework.

---

## Experiment 1 — State regulation produces measurable retention gains

**UTL prediction.** UTL v2 Part X.1 / Part XII summary: a 5-minute pre-class state-regulation protocol (paced 0.10-Hz breathing) raises ΔE from baseline ≈ 0.4 to ≈ 0.7, with no change to content; this should produce measurably better 48-h quiz scores.

**Hypothesis.** H1: Cohort A (state-regulation arm) outperforms Cohort B (no-intervention control) on 48-h retention by d ≥ 0.3, with the intervention's effect mediated by HRV_coherence (E17 PaPaGei reading) and self-reported arousal (Likert).

**Design.**

- N = 100, randomised between arms.
- Same 30-minute lecture content (chemistry vocabulary + concepts) for both arms.
- Cohort A: 5 min paced breathing (audio guide, 0.10 Hz) before lecture.
- Cohort B: 5 min unrelated activity (reading neutral text).
- Measure E17 (PPG → HRV coherence) at three timepoints: pre-intervention, post-intervention, mid-lecture.
- Quiz at 48 h, blinded grading.

**Embedders used:** E14, E15, E16 for affect; E17 for HRV; E1/E5/E12 for ΔC at quiz-answer time.

**Pre-specified analysis.**

- Primary: between-cohort 48-h quiz score difference, t-test, α = 0.05.
- Secondary: causal mediation via PROCESS / lavaan with HRV_coherence as the mediator.
- Effect-size band: meaningful effect d ∈ [0.3, 0.6]; null d < 0.1.

**What would falsify UTL v2.**

- d < 0.1 between cohorts → no measurable retention benefit from state regulation as predicted. UTL v2 would need to reduce its claim about the operationalisability of ΔE through breath-pacing.
- HRV_coherence does not mediate the effect → the 0.10-Hz resonance term in ΔC is not the mechanism. UTL v2's ΔC operationalisation needs revision.

**Power.** N = 100 gives 80% power for d = 0.3 at α = 0.05 (two-sided). Pilot N=20 first to lock measurement.

**Cost.** ~$10K including stipends and equipment.

**Status.** Implementable in 1–2 months given a partner classroom (e.g., KSU undergrad psych) and Phase-1 calibration (file 06).

---

## Experiment 2 — State-transfer term `T(train, deploy)` predicts deployed performance

**UTL prediction.** UTL v2 Part VIIb: `performance_deploy = M(t) · T(train, deploy)` where T = exp(-d(state)/λ). Subjects who train in calm and deploy in stress should underperform vs. subjects who train across a state-distribution that includes deployment.

**Hypothesis.** H2: With M (consolidated mastery on a quiz topic) controlled, state-transfer T predicts deployment quiz score with R² ≥ 0.20.

**Design.**

- N = 60, three arms.
- All three arms train on the same content over 4 sessions to equivalent post-training mastery (calibrated by adaptive testing).
- Arm A (calm-only): all 4 sessions in low-stress baseline.
- Arm B (variable): sessions vary in stress (timed sessions, cognitive load tasks during study).
- Arm C (deployment-similar): all 4 sessions in conditions designed to mirror the deployment exam (timed, peer-observed, video-recorded).
- Final exam at 7 d in a high-stress condition (timed, 10% pass-rate framing, video-recorded for stakes).

**Embedders used:** all learner-state embedders E15-E21 to compute the state vector; cosine distance to deployment centroid for T.

**Pre-specified analysis.**

- Primary: linear model `final_exam_score ~ M · T + cohort_effect`. T's coefficient should be positive with p < 0.05.
- Secondary: distance d(train_state, deploy_state) should negatively predict score after controlling for M.

**What would falsify UTL v2.**

- T coefficient ≈ 0 → state transfer doesn't predict deployment performance. UTL v2's training-design implication is wrong.
- Arm A (calm-only) outperforms Arm C (deployment-similar) → the variability-of-practice prescription is wrong.

**Power.** N = 60 gives 80% power for an R² increment of 0.10 in a regression with 3 predictors.

**Cost.** ~$15K.

**Status.** Implementable. The MAHNOB-HCI dataset (file 06) provides a baseline for the multimodal state-vector calibration.

---

## Experiment 3 — Mental rehearsal contributes γ ≈ 0.7 to ΔS

**UTL prediction.** UTL v2 Part III: simulated experience contributes γ × |predicted - simulated| to ΔS, with γ ∈ [0.6, 0.8].

**Hypothesis.** H3: When subjects mentally rehearse a procedural skill at the same self-reported intensity as physical practice, the cosine distance from goal centroid (in motor-imagery embedding space + behavioural performance) reduces at a rate consistent with γ ∈ [0.6, 0.8] of the physical-rate.

**Design.**

- Skill: a typing-rhythm task (rhythm-game-style, easily measurable).
- N = 80, four arms, 14 days.
- Arm A (physical only): 10 min/day physical practice.
- Arm B (imagery only): 10 min/day vivid first-person mental rehearsal, eyes closed.
- Arm C (mixed): 5 min physical + 5 min imagery.
- Arm D (control): 10 min unrelated task.
- VMIQ-2 scoring of imagery vividness (γ-fidelity proxy).
- Behavioural score (goal centroid distance) at days 1, 7, 14.

**Embedders used:** task-specific motor embedding (rhythmic patterns can be embedded with a 1D-CNN audio encoder of the keypress rhythm); E14 for self-reported affect.

**Pre-specified analysis.**

- Primary: rate of behavioural improvement in Arm B / rate in Arm A. Predict ratio ∈ [0.6, 0.8].
- Secondary: mixed-effects model with VMIQ-2 score as a moderator.

**What would falsify UTL v2.**

- Imagery-only arm shows zero behavioural improvement → motor imagery doesn't transfer to procedural skill. UTL v2's γ-term is wrong, at least for this task class.
- Ratio < 0.3 or > 1.0 → γ is not in the predicted band.

**Power.** N = 80 (20/arm) gives 80% power for d ≈ 0.4 differences between arms. The exact effect-size prediction (γ ≈ 0.7) corresponds to a specific quantitative band.

**Cost.** ~$8K.

**Status.** Implementable. Rich existing literature on motor imagery (Pascual-Leone, Ruffino, Schuster) supports the broader claim; this experiment quantifies γ.

---

## Experiment 4 — UTL-state-aware FSRS outperforms vanilla FSRS

**UTL prediction.** UTL v2's `M(t)` integral includes ΔE-modulation; sessions in low-ΔE state should produce shorter `τ_decay` than sessions in high-ΔE state. Therefore an FSRS variant that scales review intervals by session-time ΔE should outperform vanilla FSRS on long-term retention.

**Hypothesis.** H4: Cohort A (UTL-FSRS) achieves higher 90-day retention than Cohort B (vanilla FSRS) on the same item set, by d ≥ 0.2.

**Design.**

- N = 60, randomised across arms.
- Both cohorts study the same 200-item Spanish vocabulary deck via Anki for 90 days.
- Cohort A: FSRS scheduling enriched with session-time HRV (E17) and self-reported state. Items studied in low-ΔE sessions get review intervals scaled by 0.7; items in high-ΔE sessions get intervals scaled by 1.3.
- Cohort B: vanilla FSRS.
- 90-day retention test (50-item random sample from the deck), blinded grading.

**Embedders used:** E17 (HRV); E16 (text affect at session start); E14 (speech affect at session start, optional).

**Pre-specified analysis.**

- Primary: between-cohort 90-d retention difference. t-test, α = 0.05.
- Secondary: per-item analysis of state-at-encoding vs. retention probability.

**What would falsify UTL v2.**

- d ≤ 0 → state-aware FSRS doesn't help; the ΔE modulation of M(t) is not load-bearing for spaced-repetition. UTL v2 reduces to FSRS for this domain.
- Mixed effect (helps for some items, hurts for others) → ΔE interacts with item difficulty in unpredicted ways; UTL v2 needs refinement.

**Power.** N = 60 gives 80% power for d = 0.3.

**Cost.** ~$12K including 90-day stipends.

**Status.** Implementable. FSRS is open-source. The 24-h calibration protocol (file 06) pre-establishes the state thresholds.

---

## Experiment 5 — UTL diagnostic catches the AI training bugs that loss curves miss

**UTL prediction.** File 05 §7: a UTL-aware training monitor would have flagged the Shakespeare echo bug and the EchoMimicV3 sign bug in the first ~200 steps via low ΔC_panel, before the 7.5-h training completed.

**Hypothesis.** H5: On a controlled set of "obviously broken" training runs (corrupted target, label noise, sign bug, prefix-echo bug), the UTL diagnostic flags the run as Dissipating or Dysregulated within the first 5% of training, while standard loss-curve monitoring does not.

**Design.**

- 6 controlled training scenarios, each with two configurations (broken / healthy).
  - Scenario 1: 67% prefix-echo on training data (Shakespeare bug recreation).
  - Scenario 2: target sign flipped (EchoMimicV3 bug recreation).
  - Scenario 3: 30% label noise.
  - Scenario 4: catastrophic-forgetting from continual learning.
  - Scenario 5: out-of-distribution evaluation set leakage.
  - Scenario 6: healthy training on clean data (control for false positives).
- For each scenario, run UTL diagnostic (`compute_L_AI` with embedder-panel coherence ΔC_panel) every 100 steps. Flag whenever L_AI < 0.3 with ΔE_AI > 0.4 (Dissipating) or ΔE_AI < 0.4 (Dysregulated).
- Compare to baseline monitor (loss curve only).

**Embedders used:** Chris's production E1-E14 content stack against the training data and against the evolving model's outputs.

**Pre-specified analysis.**

- Primary: % of broken scenarios caught within first 5% of training by UTL vs. baseline. UTL should catch ≥ 80%; baseline ≤ 30%.
- Secondary: false-positive rate on the healthy control should be < 5%.

**What would falsify UTL v2.**

- UTL catches < 50% of broken scenarios → ΔC_panel is not a sensitive enough indicator for these bugs. The multi-embedder agreement signal is too noisy.
- High false positive rate on healthy control → the diagnostic is unusable.

**Power.** This is a software experiment; can run hundreds of replicates of each scenario. Statistical significance is easy; the meaningful question is the % caught.

**Cost.** ~$5K compute for the training runs.

**Status.** Implementable now. Chris already has the bugged Shakespeare data and the EchoMimicV3 codebase. Reconstructing the bad training scenarios is a matter of reverting commits.

---

## Experiment 6 — Sparse-autoencoder feature counts predict M(t) for a model

**UTL prediction.** File 05 §5.2: M(t) for an AI is operationalisable as the count of monosemantic features extracted by a sparse autoencoder on the model's hidden states. Feature count should grow as a function of `∫ ΔS · ΔC · ΔE_AI dt` over training.

**Hypothesis.** H6: Across training runs of the same architecture with controlled variation in ΔE_AI (induced by varying learning rate to manipulate sharpness), final SAE feature count correlates with `∫ ΔS · ΔC · ΔE_AI dt` at r ≥ 0.6.

**Design.**

- Train 20 small LLMs (e.g., 50M-parameter GPT-2 variants) on the same OpenWebText subset.
- Vary learning rate across runs to manipulate sharpness (and therefore plasticity_window in UTL v2 mapping).
- Vary curriculum order across runs to manipulate ΔS / ΔC time-series.
- After each run, train an SAE on the hidden states of layer 5; count interpretable features by automated semanticity score (Anthropic's monosemanticity protocol or open replicas).
- Correlate feature count with `∫ L · ΔE_AI dt`.

**Embedders used:** the model's own residual stream (its self-representations) plus an external SAE on those activations.

**Pre-specified analysis.**

- Primary: Pearson r between integrated UTL product and SAE feature count.
- Secondary: separate correlations for each UTL term (ΔS, ΔC, ΔE_AI) to see which is most predictive.

**What would falsify UTL v2.**

- r < 0.3 → integrated UTL doesn't predict feature consolidation. Either the AI-side mapping in file 05 is wrong, or the SAE feature count is the wrong M(t) proxy.
- ΔE_AI alone correlates more than the product → the multiplicative gating form is wrong; the additive form might be better.

**Power.** N = 20 runs gives 80% power for r = 0.6 at α = 0.05.

**Cost.** ~$20K compute for the 20 training runs (small models help). SAE training is comparable.

**Status.** Implementable. Anthropic's monosemanticity work and 2025 open replicas (Goodfire, Neuronpedia) provide the SAE infrastructure.

---

## Experiment 7 — AI-native process assessment predicts transfer better than answer-only grading

**UTL prediction.** If learning is a trajectory through ΔS, ΔC, ΔE, and M(t), then a single final answer is an impoverished measurement. Versioned process artifacts should predict delayed transfer and oral defense better than answer-only grading, especially when AI tools are allowed.

**Hypothesis.** H7: A process-assessed, AI-allowed course section produces higher delayed transfer and oral-defense scores than an answer-only, AI-allowed section on the same material, with the advantage mediated by process-trace quality.

**Design.**

- N = 80, two course sections or bootcamp cohorts, same content and instructor.
- Both cohorts may use AI tools openly.
- Cohort A: "100 holes" process assessment. Students submit problem framing, AI prompts, critiques, source checks, revision history, error logs, and final artifact. They also complete a short oral defense.
- Cohort B: conventional final artifact + quiz grading.
- Delayed transfer task at 30 days with new constraints not seen during the course.
- Human graders blinded to cohort score final transfer and oral defense.

**Embedders used:** E1/E5/E12/E13 for artifact and source coherence; contrastive anomaly mining for "sounds right but wrong" cases; learner-state E15-E21 optional for ΔE mediation; provenance hashes for process lineage.

**Pre-specified analysis.**

- Primary: between-cohort difference on delayed transfer task, α = 0.05.
- Secondary: regression `transfer_score ~ process_trace_quality + final_artifact_score + baseline_skill`.
- Mediation: process-trace quality should mediate the cohort effect.

**What would falsify UTL v2.**

- Process assessment produces no transfer advantage over answer-only grading despite similar workload.
- Process-trace quality fails to predict delayed transfer after controlling for final artifact quality.
- Oral defense exposes no additional signal beyond the final answer.

**Power.** N = 80 gives adequate power for d ≈ 0.45. This can be run as a classroom/bootcamp pilot before larger institutional deployment.

**Status.** Implementable as a Leapable.ai / partner-classroom pilot. It is also the best public demonstration of UTL because it aligns the theory, the product, and the AI-native education thesis.

---

## Summary of falsification commitments

| Experiment | UTL claim tested | Falsifier | Power |
|---|---|---|---|
| 1 | State regulation produces retention gains | d < 0.1 in 48-h quiz | 80% @ d=0.3, N=100 |
| 2 | T(train, deploy) predicts deployed performance | T coefficient ≈ 0 | 80% @ ΔR²=0.10, N=60 |
| 3 | Mental rehearsal γ ≈ 0.7 | ratio outside [0.3, 1.0] | 80% @ d=0.4, N=80 |
| 4 | UTL-aware FSRS > vanilla FSRS | d ≤ 0 in 90-d retention | 80% @ d=0.3, N=60 |
| 5 | UTL catches AI bugs early | catches < 50% in first 5% | controlled software, n=hundreds |
| 6 | SAE feature count ~ ∫ L · ΔE_AI | r < 0.3 | 80% @ r=0.6, N=20 runs |
| 7 | Process traces predict AI-native learning | no transfer/process advantage | 80% @ d≈0.45, N=80 |

Each is a clean, pre-registered, runnable experiment. If all human experiments are run independently, the total cohort is ~380 humans + 20 AI training runs. Total cost: ~$80K–90K depending on whether Experiment 7 is embedded in an existing class. Total time: 6 months for the full battery (Experiments 1, 3, 7 in parallel; 2 follows 1; 4 in parallel; 5–6 software, in parallel with everything).

---

## How this connects to publication

A paper combining Experiments 1–4 and 7 (human-side, plus mediation analyses) is a 2026 *Nature Human Behaviour* or *PNAS* candidate. A paper combining Experiments 5–6 (AI-side) is a NeurIPS / ICML candidate. UTL v2 with these experimental anchors is the kind of paper that survives strong peer review — every claim is operationalisable, every term is measurable, every falsifier is named.

The framework's strength is exactly that: it commits in advance to what would invalidate it. That is the discipline that distinguishes UTL v2 from the predictive-processing / FEP critique developed in file 01.

---

## What we should NOT claim until these run

- That the embedder programme can replace human experts in pedagogy. The claim is that it provides a quantitative diagnostic; the diagnostic is useful to humans, not a replacement for them.
- That UTL is a complete theory of learning. It is a measurement framework that captures the mechanically tractable aspects.
- That UTL applies uniformly across all learners or all skills. Cross-population generalisation is one of the experiments yet to run (Experiment 1 establishes effect on a single cohort; Experiment 2 across deployment conditions; cross-cultural / cross-age remains future work).
- That UTL-aware AI training is universally better than standard ML practice. Experiment 5 establishes the diagnostic value for known failure modes; whether it also improves training of healthy runs is a separate question.

These are exactly the "scope hedges" Chris's PAPER.md uses for the TCT framework, and they apply with the same force here.

---

## Sources

- [UTL v2 Part XI.6 — What Would Falsify v2.0](../UTL_v2_embodied.md)
- [Lehrer et al. (2020) HRV biofeedback meta-analysis](https://link.springer.com/article/10.1007/s10484-020-09466-z) — Experiment 1 effect-size basis
- [Yeager et al. (2019) National Study of Learning Mindsets](https://www.nature.com/articles/s41586-019-1466-y) — population-scale learning RCT precedent
- [Ruffino, Papaxanthis & Lebon (2017) Neuroscience](https://www.sciencedirect.com/science/article/pii/S0306452216307138) — motor imagery γ
- [Schuster et al. (2011) BMC Medicine](https://bmcmedicine.biomedcentral.com/articles/10.1186/1741-7015-9-75) — motor imagery best practice
- [FSRS algorithm — Anki](https://faqs.ankiweb.net/what-spaced-repetition-algorithm) — Experiment 4 baseline
- [Anki Manual — Deck Options / FSRS](https://docs.ankiweb.net/deck-options) — current FSRS baseline details
- [Anki FAQs — v3 Scheduler](https://faqs.ankiweb.net/the-2021-scheduler.html) — current scheduler status
- [DRL-SRS](https://www.mdpi.com/2076-3417/14/13/5591) — DRL spaced-repetition baseline
- [Scaling Monosemanticity (Anthropic 2024)](https://transformer-circuits.pub/2024/scaling-monosemanticity/) — Experiment 6 SAE protocol
- [A Survey on Sparse Autoencoders (2025)](https://arxiv.org/html/2503.05613v3)
- [Information-Theoretic Progress Measures reveal Grokking is an Emergent Phase Transition](https://arxiv.org/html/2408.08944v1) — alternate AI-side measure for Experiment 6 design variants
- [PAPER.md §6.1.3 (Shakespeare bug), §6.2.2 (EchoMimicV3 bug)](../PAPER.md) — Experiment 5 bug-recreation source
- [LeapableAI YouTube transcript archive](../leapableai_youtube_transcripts.md) — "answer is no longer a metric" / "100 holes" source for Experiment 7
- [UNESCO: What's worth measuring? The future of assessment in the AI age](https://www.unesco.org/en/articles/whats-worth-measuring-future-assessment-ai-age) — AI-era process-assessment support
