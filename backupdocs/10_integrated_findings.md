# 10 — Integrated Findings From Chris's Existing Stack

**Purpose.** Capture what changed after reading the full local corpus: learnembed docs, PAPER.md, alldata, LeapableAI transcripts, ClipCannon specs, Context Graph specs, and current external references for FSRS, EU AI Act, C2PA, and AI-era assessment.

---

## 1. The main update

The learnplan should not treat UTL as a generic biosignal dashboard. Chris's existing work already defines the product and research posture:

1. **Local-first, provenance-first, forensic by default.** OCR Provenance and ClipCannon show the right shape: local processing, tamper-evident chains, versioned artifacts, and billing/license metadata separated from content.
2. **Context Graph is the substrate.** The current implementation already has 13 embedder spaces, 59 RocksDB column families, 75 MCP tools, constellations, training exports, contrastive anomaly mining, typed edges, and CAMD-DPO pair mining. UTL is a learner layer beside that substrate, not a replacement.
3. **ClipCannon is the cross-modal calibration sandbox.** Its 23-stage video DAG, seven-modality constellation system, Phoenix behavior hierarchy, and VoiceAgent pipeline provide practical examples of time-aligned multimodal state measurement. These are engineering inputs, not human-learning claims by themselves.
4. **AI-native education is the cleanest first application.** The transcript doctrine that "the answer is no longer a metric" maps directly to UTL: measure the trace, not just the final artifact. Drafts, critiques, source checks, revisions, and oral defense become learning traces.
5. **Regulatory constraints must shape architecture early.** EU education/outcome-evaluation systems can be high-risk, and emotion recognition in education/workplace contexts is prohibited except narrow medical/safety cases. UTL should default to learner-controlled self-measurement and human-reviewed support.

---

## 2. Required roadmap changes

### Phase 0

Build the provenance spine before adding the full dashboard:

- `ObservationEnvelope`: learner ID, consent state, modality, raw hash, preprocessing version, embedder version.
- `ComputationEnvelope`: parent observation IDs, threshold version, ΔS/ΔC/ΔE result, output hash.
- `ReportEnvelope`: parent computations, generated text hash, human-review state.

This lets every plotted point in the proof of concept be audited.

### Phase 1

Use ClipCannon's existing Santa / voice / Phoenix data as a calibration sandbox for cross-modal timing and state-consistency checks. Keep the claim narrow: it proves the pipeline can align modalities and compute constellations; it does not prove UTL's human-learning predictions.

### Phase 2

Add the AI-native course pilot early, not after publication. A small "100 holes" cohort generates the right artifacts for UTL: work-product lineage, critique loops, transfer tests, and oral defense.

### Phase 3

Experiment 7 should join the falsification battery. It tests whether process traces predict delayed transfer better than answer-only grading when AI use is allowed.

### Phase 4

Productization should copy the OCR Provenance pattern: local GPU processing, explicit cloud opt-in, signed billing events, exportable evidence bundles, and no vendor access to learner content by default.

---

## 3. Engineering guardrails

- **Do not mutate Context Graph's existing `TeleologicalFingerprint` v1 wire format.** Keep E1-E14 as the content fingerprint. Add learner data as a separate versioned `LearnerFingerprint` / `LearnerStateVector`.
- **Every threshold is default-pending-calibration.** No ΔE cutoff, β update, λ transfer bandwidth, or state classifier threshold should be written as theory-derived truth until calibrated.
- **No judge LLM as ground truth.** Use deterministic embedder panels, preregistered human rubrics, blinded graders, and contrastive anomaly mining.
- **Separate measurement consent from clone consent.** Learner audio/video collected for UTL cannot be reused for voice, avatar, or meeting-agent generation without a separate opt-in.
- **Generated media needs cryptographic provenance.** If UTL or adjacent tools emit synthetic voice/video/avatar artifacts, attach C2PA / Content Credentials or an equivalent manifest.
- **Compliance exports are product features.** Risk registers, data-governance logs, technical-documentation bundles, human-oversight notes, and audit trails should be generated as normal artifacts.

---

## 4. Open questions before implementation

1. Should E15-E21 run inside Context Graph's Rust learner embedder registry, or as a sidecar learner-state daemon that writes versioned `LearnerFingerprint` records into RocksDB?
2. What is the minimum viable sensor stack for Phase 0: webcam + mic + self-report only, or webcam + mic + Polar H10 backup from day one?
3. Is the first public pilot a personal Chris session, Steve's Anki/bar-exam workflow, or the AI-native "100 holes" classroom experiment?
4. Should the first product bundle target personal learners only, avoiding institutional education/workplace use until compliance tooling is complete?
5. What is the deletion model for derived biometric embeddings and centroids after a learner revokes consent?

---

## 5. Sources

- [PAPER.md](../PAPER.md)
- [alldata.md](../alldata.md)
- [LeapableAI YouTube transcript archive](../leapableai_youtube_transcripts.md)
- [Context Graph System Overview](../../../contextgraph/docs2/contextgraphspecs/01_system_overview.md)
- [Context Graph Storage Layer](../../../contextgraph/docs2/contextgraphspecs/09_storage_layer.md)
- [Context Graph Teleological Subsystem](../../../contextgraph/docs2/contextgraphspecs/13_teleological_subsystem.md)
- [Context Graph Training Subsystem](../../../contextgraph/docs2/contextgraphspecs/16_training_subsystem.md)
- [Context Graph Constellation Subsystem](../../../contextgraph/docs2/contextgraphspecs/17_constellation_subsystem.md)
- [Context Graph Contrastive and Typed Edges](../../../contextgraph/docs2/contextgraphspecs/18_contrastive_and_typed_edges.md)
- [Context Graph CAMD-DPO Subsystem](../../../contextgraph/docs2/contextgraphspecs/19_camd_dpo_subsystem.md)
- [ClipCannon System Overview](../../../clipcannon/docs2/clipcannondocs/01_system_overview.md)
- [ClipCannon Provenance System](../../../clipcannon/docs2/clipcannondocs/05_provenance_system.md)
- [ClipCannon Pipeline Stages](../../../clipcannon/docs2/clipcannondocs/06_pipeline_stages.md)
- [ClipCannon Phoenix Engine](../../../clipcannon/docs2/clipcannondocs/16_phoenix_engine.md)
- [ClipCannon VoiceAgent](../../../clipcannon/docs2/clipcannondocs/17_voiceagent.md)
- [Anki Manual: FSRS](https://docs.ankiweb.net/deck-options)
- [Anki FAQ: v3 Scheduler](https://faqs.ankiweb.net/the-2021-scheduler.html)
- [EU AI Act implementation timeline](https://ai-act-service-desk.ec.europa.eu/en/ai-act/timeline/timeline-implementation-eu-ai-act)
- [EU AI Act Annex III](https://ai-act-service-desk.ec.europa.eu/en/ai-act/annex-3)
- [EU AI Act FAQ](https://ai-act-service-desk.ec.europa.eu/en/faq)
- [C2PA Technical Specification 2.4](https://spec.c2pa.org/specifications/specifications/2.4/specs/C2PA_Specification.html)
- [C2PA Content Credentials Explainer](https://spec.c2pa.org/specifications/specifications/2.2/explainer/Explainer.html)
- [UNESCO: What's worth measuring? The future of assessment in the AI age](https://www.unesco.org/en/articles/whats-worth-measuring-future-assessment-ai-age)
