# 09 — Implementation Roadmap

**Purpose.** Phased plan that takes UTL v2 from theory document to running system, with milestones, deliverables, dependencies, and decision gates. Every phase produces something that *runs* — no purely-theoretical phases.

The structure: 5 phases, escalating in complexity. Phase 0 is a 1–2 week proof-of-concept that runs on Chris's existing dev workstation with no new hardware. Phase 4 is a multi-cohort experimental programme that produces a publishable result.

---

## Phase 0 — Proof of concept (1–2 weeks)

**Objective.** Demonstrate that UTL ΔS, ΔC, and ΔE can be computed from a single 30-minute session of Chris using his existing dev setup, with no new hardware purchases.

**Hardware.** Chris's existing webcam + microphone. RTX 5090. No EEG, no wearable.

**Software additions.**

- Implement the UTL provenance spine first: `ObservationEnvelope`, `ComputationEnvelope`, hash-chain parent IDs, embedder-version stamps, threshold-version stamps, and report-export lineage.
- Add E14 (Wav2Vec2-MSP-DIM), E15 (OpenFace 3.0), E16 (MiniLM-EmoBank) to Context Graph's embedder registry.
- Implement webcam-rPPG (remote PPG from face video) as a software-only HRV source. Several open-source implementations exist (POS, CHROM algorithms — known accuracy ~3 bpm vs. 1 bpm for chest strap).
- Implement Likert-scale UI in the dashboard for self-reported state confirmation.
- Implement `compute_delta_s`, `compute_delta_c`, `compute_delta_e`, `compute_L` MCP tools (file 07).
- Implement `CF_FINGERPRINTS_LEARNER`, `CF_LEARNER_DELTA_LOG` column families.

**Deliverables.**

1. Chris can run a 30-min coding session with:
   - Webcam capturing his face (E15 streaming).
   - Mic capturing his voice (E14 on intermittent narration).
   - rPPG from face → HRV proxy.
   - Self-report Likert at 5-min intervals (E16).
2. Dashboard plot at session end showing:
   - L(t) per minute.
   - ΔS, ΔC, ΔE_subcomponents over time.
   - State classification per minute.
   - Diagnostic summary.
3. A 1-page write-up: "What does my coding session look like through the UTL lens?"
4. A forensic runbook: how each plotted point can be traced back to raw hashes, embedder versions, thresholds, and parent observations.

**Decision gate.** If the system runs end-to-end and produces plausible, varying signals (high ΔE during productive work, low ΔE during context-switching, low ΔS during routine refactor, high ΔS during debugging novel issues), proceed to Phase 1. If signals are uniformly noisy or constant, the embedders need recalibration before scaling.

**Risk.** rPPG quality from a desk webcam is the primary risk. Mitigation: have Polar H10 chest strap on hand for ground-truth comparison if rPPG looks bad.

**Cost.** $90 (Polar H10 backup). Engineering time: 60–80 hours.

---

## Phase 1 — Personal calibration and 1-learner deployment (1 month)

**Objective.** Build Chris's personal regulated-state centroid (Phase-1 calibration per file 02), then run UTL diagnostics over real learning sessions for 4 weeks.

**Hardware.** Tier 1 from file 06 — Polar H10 + Muse 2 added.

**Software additions.**

- Add E17 (PaPaGei), E18 (WESAD-CNN), E19 (LaBraM) to Context Graph.
- Implement `CF_LEARNER_PROFILE`, `CF_LEARNER_CONSTELLATIONS`, `CF_LEARNER_STATE_HISTORY`.
- Implement Phase-1 calibration protocol: scripted 3-hour over-1-week baseline session battery (file 06 §2.3).
- Implement `register_learner`, `record_session_observation` MCP tools.
- Implement constellation-compile-for-learner-baseline reuse of existing `compile_constellation` tool.
- Reuse ClipCannon's Santa / voice / Phoenix time-aligned data as a non-claiming calibration sandbox: useful for cross-modal ΔE debugging, not a published human-learning result.

**Deliverables.**

1. Chris's personal regulated-state centroid `C_chris` persisted to RocksDB.
2. 4 weeks of naturalistic sessions logged: coding, learning new domains, meetings, deep work.
3. A trajectory plot: L(t), M(t), state classification, weekly trend.
4. Identification of Chris's personal high-ΔE windows (likely first 90 min after wake, per UTL v2 Section 11.6).
5. A self-report comparison: subjective sense of "good day vs. bad day" against the L/M/T objective signals.

**Decision gate.** If the objective signals correlate with Chris's subjective reports (Spearman r > 0.4 between "good session" self-report and L), the framework is producing meaningful per-learner readings. If not, the calibration is off and needs more work.

**Risk.** Muse 2 has only 4 EEG channels; LaBraM was pretrained on higher-density EEG. Mitigation: validate that the band-power features still correctly classify Chris's resting alpha vs. focus beta states. If not, switch to a software-side band-power classifier rather than the LaBraM forward.

**Cost.** ~$400 (Polar H10 + Muse 2). Engineering time: 80–100 hours.

---

## Phase 2 — Multi-learner pilot, full M(t) integral (2 months)

**Objective.** Extend to 5–10 learners (Chris + Steve + 3–8 volunteer beta users), implement the full M(t) integral with sleep-gated `k(τ)`, and produce per-trace next-review-time predictions.

**Hardware.** Each pilot learner: webcam, mic, optional Apple Watch / Polar H10 / Oura. EEG optional (only those who opt in).

**Software additions.**

- Implement `CF_LEARNER_M_PER_TRACE`, `CF_LEARNER_RETRIEVAL_LOG`, `CF_LEARNER_K_SLEEP`.
- Implement sleep-stage ingestion (Apple HealthKit / WHOOP API / wearable-specific exports).
- Implement `get_learner_M`, `next_review_for_trace` MCP tools.
- Implement Anki-compatible export (deck format) for pilot learners' active traces.
- Build dashboard `/learner/{id}` route group with trajectory and trace views.

**Deliverables.**

1. 5–10 learners with active constellations and per-trace M(t) tracking.
2. UTL-derived next-review schedule for each learner's active deck.
3. A 60-day retention comparison: UTL-scheduled deck vs. each learner's previous Anki deck. Falls naturally out of the data Phase 1 + Phase 2 produces.
4. A first iteration of the AI training-monitor (file 05): apply UTL diagnostic to a controlled retraining of the Shakespeare LoRA. Goal: catch the prefix-echo bug at < 200 steps if it's present in the data.
5. A small AI-native course pilot: 5–10 learners using the "100 holes" process-artifact workflow, with provenance over drafts, critiques, revisions, sources, and oral-defense notes.

**Decision gate.** If the learners' subjective reports of "this is helpful" exceed a threshold (e.g., 7/10 on average), and if the AI training monitor catches the deliberately-injected echo bug, proceed to Phase 3.

**Risk.** Privacy and consent at multi-learner scale. Mitigation: write the consent + ethics package early; have a colleague with IRB experience review.

**Cost.** ~$2K (some learners may need wearable lend). Engineering time: 200–300 hours.

---

## Phase 3 — Falsification experiments (3 months)

**Objective.** Run Experiments 1, 3, 4, 5, and 7 from file 08. Generate empirical evidence for or against UTL v2's core predictions.

**Hardware.** Cohort-scale wearable provisioning. Plan for: 100 webcams (use participants' own), 100 Polar H10s (own or provision), 30 Muse 2s for the EEG-substudy.

**Software additions.**

- Cohort-management dashboard.
- Anonymisation pipeline.
- Statistical analysis pipeline (R or Python with pre-registered notebooks).
- Pre-registered analysis plans on OSF.

**Deliverables.**

1. Pre-registered protocols on OSF for Experiments 1, 3, 4, 5.
2. IRB approval (KSU or partner institution).
3. Ran experiments + analyses + write-ups.
4. Two pre-prints on arXiv:
   - Experiment 1 (state regulation → retention).
   - Experiment 5 (UTL diagnostic for AI training).
   - Experiment 7 can become either a methods paper or the applied-education case study inside the human-side paper.

**Decision gate.** If the experiments confirm UTL's predictions (effect sizes in the predicted band), proceed to Phase 4 (multi-cohort generalisation, journal submission, productisation). If the experiments reveal predictable failure modes, revise the framework and rerun.

**Risk.** Cohort recruitment timing; cohort attrition over 90 days. Mitigation: stipends; engagement incentives; partner with existing learning platforms.

**Cost.** ~$60K–90K (stipends, hardware, statistical consulting, and Experiment 7 unless it is embedded in an existing class).

**Time.** 3 months for execution + 1 month for analysis and write-up.

---

## Phase 4 — Productisation (4–6 months)

**Objective.** Turn the validated UTL measurement system into a deployable component of Leapable.ai's MCP marketplace and Chris's existing Context Graph offering.

**Hardware.** N/A (consumers bring their own).

**Software additions.**

- Local-first deployment with strict learner-data isolation. Raw physiological and work-product data stays on the learner's machine by default; cloud processing is explicit opt-in.
- Per-learner dashboards.
- Educator / coach views with explicit per-learner consent and no emotion-recognition dashboard for workplace/education deployments.
- API surface for third-party developers (e.g., Anki plugin developers, language-learning apps).
- Compliance export: risk register, data-governance log, technical-documentation bundle, human-oversight notes, audit logs, and model/threshold version manifests for any high-risk deployment.
- C2PA / Content Credentials attachment for any generated voice/video/avatar artifact derived from learner media.

**Deliverables.**

1. Production-grade deployment.
2. Documentation, SDK, examples.
3. Pricing model that can follow the OCR Provenance pattern: local processing plus signed usage/billing events, with no content visibility to the vendor.
4. Two market segments piloted:
   - Educational (a partner classroom or online course).
   - Corporate L&D (a partner with $370B-industry-pain).
5. The published paper (Phase 3 results + the framework writeup) as Chris's second author paper after PAPER.md.

**Decision gate.** Open-ended. The system runs.

**Cost.** Variable. Engineering time: 600–1000 hours. Marketing / sales: external to engineering plan.

---

## Phase milestones at a glance

```
  Now              Phase 0      Phase 1        Phase 2          Phase 3              Phase 4
   |                  |            |              |                |                    |
   v                  v            v              v                v                    v
  ─────────────────────────────────────────────────────────────────────────────────────────►
  Theory      Proof-of-concept   Personal     Multi-learner    Falsification      Productisation
  Document     (1-2 weeks)      calibration   pilot (2 mo)    experiments        (4-6 months)
                                  (1 month)                    (3+1 months)
```

Total time from now to publishable: 7-8 months.
Total time from now to product: 12-14 months.

---

## Resource summary

| Phase | Hardware | Engineering hours | Other cost |
|---|---|---|---|
| 0 | $90 | 60-80 | — |
| 1 | $400 | 80-100 | — |
| 2 | ~$2K | 200-300 | — |
| 3 | (provisioned per cohort) | 100-220 (analysis only; main effort is the experiments) | $60K-90K |
| 4 | N/A | 600-1000 | external |
| **Total to Phase 3 (validated framework)** | **$2.5K** | **440-620 hours** | **$60K-90K** |

This is a Chris-scale project. The whole stack from Phase 0 through Phase 3 is achievable on a single RTX 5090 with one developer, one experimental partner, and a partner classroom or pilot cohort.

---

## What's NOT in scope

- **Clinical applications** (PTSD, depression, addiction). Section 11.2 of UTL v2 mentions these; they require clinical-grade IRB and cannot be addressed within this roadmap. Defer to a clinical-partner programme.
- **Real-time avatar coaching** (file 02 §5 names the centroid pattern). Implementable but requires a separate UI / interaction-design effort that is more product than research.
- **Cross-cultural validation.** Phase 3 cohort is US/single-language; cross-language transfer of the framework is future work.
- **Children under 16.** Specifically excluded for ethics reasons; Algorithmic-Foreclosure-thesis-aligned principle.
- **Continuous deployment outside research consent context.** The framework is not deployed into the wild as an "always-on monitor" without explicit per-session opt-in.
- **Automated education/workplace emotion recognition.** In EU-like regulatory contexts this is prohibited except narrow medical/safety cases; UTL productization must use learner-controlled self-measurement and human-reviewed support.
- **Automated admissions, grading, proctoring, or employment decisions.** These cross into high-risk or prohibited territory and are not part of this roadmap.

---

## Decision dependencies

| Decision | Depends on |
|---|---|
| Proceed to Phase 1 | Phase 0 produces non-trivial varying signals |
| Proceed to Phase 2 | Phase 1 produces objective-subjective correlation r > 0.4 |
| Proceed to Phase 3 | Phase 2 cohort feedback ≥ 7/10; AI training monitor catches injected bug |
| Proceed to Phase 4 | Phase 3 experiments produce predicted effect sizes |
| Revise framework | Any decision gate fails. Specific revision depends on which signal failed. |

The framework explicitly commits to revision-on-failure. This is the falsifiability commitment from file 01 / file 08.

---

## Tooling and repository structure

Recommended additions to Chris's existing repos:

```
/home/cabdru/learnembed/
  docs/
    UTL_v2_embodied.md          (Steve's draft)
    PAPER.md                     (TCT paper)
    learnplan/                   (this plan, files 00–09)
  src/
    utl/                          (Python implementation)
      embedders/                  (E15-E21 wrappers)
      compute/                    (delta_s, delta_c, delta_e, L, M, T)
      calibration/                (Phase-1 protocol)
      provenance/                 (observation envelopes, hash chains, lineage exports)
      schedulers/                 (FSRS-UTL hybrid)
    utl_ai/                       (AI-side training monitor)
    utl_course/                   (AI-native process-assessment pilot)
    utl_cohort/                   (cohort management for experiments)
    utl_compliance/               (risk register, consent state, export bundles)
  scripts/
    phase0_proof_of_concept.py
    phase1_personal_calibration.py
    phase2_multi_learner.py
    experiments/
      exp1_state_regulation.py
      exp3_motor_imagery.py
      exp4_utl_fsrs.py
      exp5_ai_diagnostic.py

/home/cabdru/contextgraph/
  crates/context-graph-embeddings/
    src/lib.rs                    (E15-E21 learner-state slots added beside production E1-E14)
    src/embedders/affect_speech.rs (NEW)
    src/embedders/affect_face.rs   (NEW)
    src/embedders/affect_text.rs   (NEW)
    src/embedders/ppg.rs           (NEW)
    src/embedders/eda.rs           (NEW)
    src/embedders/eeg.rs           (NEW)
  crates/context-graph-storage/
    src/column_families.rs        (CF_LEARNER_* added)
  crates/context-graph-mcp/
    src/tools/definitions/utl.rs   (NEW)
    src/handlers/tools/utl_tools.rs (NEW)
    src/server/learner_state_updater.rs (NEW)

/home/cabdru/clipcannon/
  src/clipcannon/provenance/       (reuse hash-chain design)
  src/phoenix/                     (optional avatar-state integration, not required for UTL measurement)
```

This keeps UTL as a clean extension layer rather than a fork.

---

## Steve's role

Steve is the theoretical co-author. He's already produced the v2 draft; the path forward is:

- Phase 0 / Phase 1: Steve doesn't need to do anything (Chris implements).
- Phase 2: Steve becomes the second pilot learner. His Anki-card protocol for the bar exam (audio transcript) becomes the test case for Experiment 4.
- Phase 3: Steve co-authors the experimental write-ups.
- Phase 4: Steve co-authors the methodology paper, brand placement on Leapable.ai, etc.

The deal Chris and Steve discussed in the audio is straightforward: **Chris implements; Steve co-theorises; both names go on the publications.** This is a fair division given Steve's contribution is the v2 framework and Chris's contribution is the operationalisation.

---

## What this plan actually delivers

By the end of Phase 3 (~7 months from now):

1. UTL v2 is no longer a theory document. It's a running system with measured behaviour.
2. The framework either holds (in which case Phase 4 productises) or doesn't (in which case the failures are diagnostic and v2.1 / v3 follows).
3. Two papers are submittable:
   - Methodology + framework + Experiment 1 + Experiment 4 → behavioural sciences journal.
   - AI training diagnostic (Experiment 5 + 6) → ML conference.
4. The Context Graph stack has gained ~20 % code with a new product capability (learner measurement).
5. Steve has a quantitative, published version of his framework with Chris's name attached.

This is enough deliverables to justify the engineering and is in scope for one engineer + one theorist + a pilot cohort.

---

## The connection to Steve's golf metaphor

Steve in the audio: "*I started off, you gave it to me on the 6th hole, and I've taken it over. I'm over now on about the 10th or 11th hole. ... I wouldn't tell you that I was at the 18th hole yet, but I would tell you that I definitely got on the back nine.*"

In this plan's terms:

- Steve's v1 was holes 1–6. Chris's review and the embedded-cognition / UTL-v2 framework expansion is hole 7–11.
- Phase 0–1 is hole 12–14 (concrete operationalisation).
- Phase 2–3 is hole 15–17 (validation).
- Phase 4 is hole 18 (deployment).

We're a bit past the back-nine turn. The hole that matters next is Phase 0 — the proof-of-concept that turns the document into a running thing.

---

## Sources

- All preceding files in this learnplan (00–08) provide the technical inputs.
- [PAPER.md](../PAPER.md) — TCT methodology / forensic disciplines.
- [UTL_v2_embodied.md](../UTL_v2_embodied.md) — Steve's framework.
- [Context Graph specs](../../../contextgraph/docs2/contextgraphspecs/) — integration target.
- [ClipCannon Phoenix engine](../../../clipcannon/docs2/clipcannondocs/16_phoenix_engine.md) — for cross-system constellation reuse.
- [ClipCannon Provenance System](../../../clipcannon/docs2/clipcannondocs/05_provenance_system.md) — hash-chain design pattern.
- [LeapableAI YouTube transcript archive](../leapableai_youtube_transcripts.md) — forensic engineering and AI-native education source.
- [EU AI Act Service Desk: implementation timeline](https://ai-act-service-desk.ec.europa.eu/en/ai-act/timeline/timeline-implementation-eu-ai-act)
- [EU AI Act Service Desk: Annex III](https://ai-act-service-desk.ec.europa.eu/en/ai-act/annex-3)
- [C2PA Technical Specification 2.4](https://spec.c2pa.org/specifications/specifications/2.4/specs/C2PA_Specification.html)
