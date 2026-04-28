---
namespace: patterns
agent_id: agent-18-gap-hunter
timestamp: 2026-04-17T02:55:00Z
status: active
locked: false
supersedes: none
tags: gap-to-section, manuscript-placement, section-map, agents-36-41
depends_on:
  - memory/discoveries/agent-18-gap-hunter--consolidated-gaps.md
  - memory/patterns/agent-17-thematic-synthesizer--thematic-frameworks.md
  - memory/patterns/agent-17-thematic-synthesizer--narrative-arc.md
  - memory/patterns/agent-05-research-planner--rewoo-plan.md
downstream_consumers:
  - agent-19-opportunity-identifier
  - agent-36-related-work-writer
  - agent-37-methodology-writer
  - agent-39-discussion-writer
  - agent-40-limitations-writer
  - agent-41-future-work-writer
  - agent-42-adversarial-reviewer
---

# Gap → Manuscript Section Placement Map

## Purpose

Route every gap in `agent-18-gap-hunter--consolidated-gaps.md` to one or
more specific manuscript sections, naming the writer-agent responsible.
This file is the lookup table agents 36–41 consult when deciding "where
does this gap get mentioned".

Rules:
- Every gap gets **exactly one primary section** (the slot where the gap
  is fully stated + routed).
- Any gap may have **secondary sections** (where it is referenced
  briefly — e.g., §1 preview, §2 related-work context).
- "Future work" gaps **must** appear in §10 (Future Work); they may also
  appear in §8 (Limitations) for disclosure.
- "Partial" gaps **must** appear in §8 (Limitations) + §10 (Future
  Work).
- "We fill" gaps **must** appear in whichever §3 / §5 / §6 subsection
  closes them + §7 (Discussion) for framing.

---

## Section ownership reminder (from agent-05)

| Section | Owner | Writer-agent |
|---|---|---|
| Abstract | agent-35 | agent-35-abstract-writer |
| §1 Introduction | agent-36a | agent-36-related-work-writer |
| §2 Related Work | agent-36 | agent-36-related-work-writer |
| §3 Framework (DDA + meaning compression) | agent-37 | agent-37-methodology-writer |
| §4 Systems (Context Graph + ClipCannon) | agent-38 | agent-38-systems-writer |
| §5 TCT method | agent-37 | agent-37-methodology-writer |
| §6 Case studies | agent-38 | agent-38-systems-writer |
| §7 Discussion | agent-39 | agent-39-discussion-writer |
| §8 Limitations | agent-40 | agent-40-limitations-writer |
| §9 Conclusion | agent-39 | agent-39-discussion-writer |
| §10 Future Work | agent-41 | agent-41-future-work-writer |

---

## Master placement table

| Gap ID | Dimension | Routing | Primary § | Secondary §§ | Writer-agent(s) |
|---|---|---|---|---|---|
| GAP-01 | Structural | Fill | §3.1 + §3.4 | §1, §2.1, §7 | 37, 36, 39 |
| GAP-02 | Structural | Fill | §1, §3.3 | §2.1, §7, §9 | 36, 37, 39 |
| GAP-03 | Structural | Fill | §5.3 | §1, §2.5, §2.6, §6.3, §7 | 37, 36, 38, 39 |
| GAP-04 | Structural | Fill | §5.1, §6.3 | §2.5, §3.6, §6.2 | 37, 38, 36 |
| GAP-05 | Structural | Fill | §6.1, §6.2 | §8.4, §9 | 38, 40, 39 |
| GAP-06 | Methodological | Fill | §3.4 | §8 (hedge), §10 (GAP-13 extension) | 37, 40, 41 |
| GAP-07 | Methodological | Fill | §6.3 | §2.5, §8.1 | 38, 36, 40 |
| GAP-08 | Methodological + empirical | Partial | §6.3 | §8.1, §10 | 38, 40, 41 |
| GAP-09 | Theoretical | Partial | §3.4 (upper-bound hedge) | §1.2.5 dda-def (hedge), §8.1, §10 | 37, 40, 41 |
| GAP-10 | Theoretical | Partial | §3.3 (reframe) | §8, §10 | 37, 40, 41 |
| GAP-11 | Theoretical | Partial | §3.3 | §8, §10 | 37, 40, 41 |
| GAP-12 | Theoretical | Partial | §5.1, §5.2 | §2.6, §8, §10 | 37, 36, 40, 41 |
| GAP-13 | Theoretical + empirical | Future | §10 | §3.4 (mention), §8 | 41, 37, 40 |
| GAP-14 | Theoretical + empirical | Future | §10 | §8.6 (ethics), §7 | 41, 40, 39 |
| GAP-15 | Theoretical | Future | §10 | §6.3, §8.1 | 41, 38, 40 |
| GAP-16 | Theoretical + empirical | Future | §10 | §5.3, §8 | 41, 37, 40 |
| GAP-17 | Empirical | Future | §10 | §3.4, §8 | 41, 37, 40 |
| GAP-18 | Empirical | Future | §10 | §3.3, §8 | 41, 37, 40 |
| GAP-19 | Empirical | Future | §10 | §8.6 (ethics) | 41, 40 |
| GAP-20 | Empirical | Future | §10 | §6.3, §3.6, §8 | 41, 38, 37, 40 |
| GAP-21 | Evaluation | Future | §10 | §3.4, §7 | 41, 37, 39 |
| GAP-22 | Evaluation | Partial | §6.3 (Case-3 operating point) | §10 (benchmark), §7 | 38, 41, 39 |
| GAP-23 | Evaluation | Future | §10 | §5.3, §7 | 41, 37, 39 |
| GAP-24 | Evaluation | Partial | §6.3 (in-paper ablation if feasible) | §10 (benchmark), §7 | 38, 41, 39 |
| GAP-25 | Reproducibility | Future | §8.1 (disclosure) | §2.5, §2.6, §6.3 opener | 40, 36, 38 |
| GAP-26 | Reproducibility | Partial | §8 (projected-vs-measured) | §6.1, §6.2, §10 | 40, 38, 41 |
| GAP-27 | Reproducibility | Future | §10 (invitation) | §6.3, §8 | 41, 38, 40 |
| GAP-28 | Reproducibility | Future | §10 | §4.2, §6.2 | 41, 38 |
| GAP-29 | Population | Future | §10 | §6.2, §6.3, §8.1 | 41, 38, 40 |
| GAP-30 | Population | Future | §10 | §8.1 | 41, 40 |
| GAP-31 | Population | Future | §10 | §8.1 | 41, 40 |

---

## Section-aggregated view (what each writer-agent must cover)

### §1 Introduction (agent-36)

**Gaps to reference as framing moves.**
- GAP-01 (signals-per-input unformalised) — preview only.
- GAP-02 (third path) — **primary framing** (three-paths).
- GAP-03 (runtime guard) — preview only (agent-17 R2: §1 footnote
  framing + §5 technical body).

**Narrative beat.** Open with the data-wall problem (Theme A). Name
paths 1 and 2. Name path 3 (DDA). Preview the runtime guard (Theme E)
in a single sentence.

### §2.1 Data-wall framing (agent-36)

**Gaps to reference.**
- GAP-02 (third path) — recapitulate.
- GAP-25 (closed-artefact frontier) — scope note.

### §2.5 TTS / video comparators (agent-36)

**Gaps to reference.**
- GAP-03 (runtime guard absent from TTS / video cluster).
- GAP-04 (multi-clip centroid absent).
- GAP-07 (cross-encoder SECS absent).
- GAP-25 (closed-artefact frontier: VALL-E 2, NaturalSpeech 3,
  EchoMimicV3).

### §2.6 Alignment comparators (agent-36)

**Gaps to reference.**
- GAP-03 (runtime guard absent from alignment cluster).
- GAP-12 (scalar-reward BRK; TCT non-equivalence structural, not
  proved).
- GAP-25 (closed-artefact frontier: Constitutional AI).

### §3.1 Meaning compression as fourth category (agent-37)

**Gaps filled.**
- GAP-01 (signals-per-input ratio formalised).

### §3.3 Third-path argument + collapse-theorem scope (agent-37)

**Gaps filled.**
- GAP-02 (third path categorical argument).
- GAP-10 (partial: reframe stated, empirical re-fit future).
- GAP-11 (partial: collapse-silent argument, formal theorem future).

### §3.4 DDA count + cross-embedder interaction features (agent-37)

**Gaps filled.**
- GAP-01 (signals-per-input count).
- GAP-06 (cross-embedder interaction feature protocol).
- GAP-09 (partial: upper-bound hedge stated; MI measurement future).

**Gaps referenced.**
- GAP-13 (MI-adjusted MC ratio — future).
- GAP-17 (empirical MI measurement — future).

### §3.6 Sample-complexity framing (agent-37)

**Gaps referenced.**
- GAP-04 (centroid aggregation enables the collapse claim).
- GAP-20 (sample-complexity curve — future).

### §4.1 Context Graph (agent-38)

**Gaps referenced.**
- Role-flip implementation (embedder-as-retriever role); no new gap
  IDs — Theme B framing only.

### §4.2 ClipCannon (agent-38)

**Gaps referenced.**
- GAP-28 (ClipCannon end-to-end DAG reproducibility — future).

### §5.1 TCT theoretical framing (agent-37)

**Gaps filled.**
- GAP-04 (centroid-construction phase).
- GAP-12 (partial: structural BRK argument).

### §5.2 Phase-2 loss (agent-37)

**Gaps referenced.**
- GAP-12 (partial).

### §5.3 Constellation guard (agent-37)

**Gaps filled.**
- GAP-03 (runtime geometric guard).

**Gaps referenced.**
- GAP-16 (guard scaling — future).
- GAP-23 (multi-encoder identity-cohesion score — future).

### §6.1 Case 1 (Shakespeare) (agent-38)

**Gaps filled.**
- GAP-05 (partial: echo-bug forensic disclosure).
- GAP-26 (partial: Case 1 projected; disclosure + hyperparameter sheet).

### §6.2 Case 2 (Santa) (agent-38)

**Gaps filled.**
- GAP-05 (partial: flow-matching sign-bug forensic disclosure by
  reference).
- GAP-26 (partial: Case 2 projected).

**Gaps referenced.**
- GAP-04 (projected 7-modality centroid).
- GAP-29 (single-subject — future).

### §6.3 Case 3 (voice cloning) (agent-38)

**Gaps filled.**
- GAP-03 (constellation guard measured).
- GAP-04 (50-clip centroid measured).
- GAP-07 (cross-encoder SECS: Qwen3-TTS → WavLM).
- GAP-08 (partial: same-session band reported for one speaker).

**Gaps partially addressed.**
- GAP-22 (runtime rejection-rate — Case 3 operating-point reported).
- GAP-24 (guard-on / guard-off ablation — in-paper if feasible).

**Gaps referenced.**
- GAP-15 (cross-encoder theoretical characterisation — future).
- GAP-20 (sample-complexity curve — future).
- GAP-25 (open-scorer posture contrasts closed-artefact frontier).
- GAP-27 (independent-lab replication — future).
- GAP-29 (single-subject — future).

### §7 Discussion (agent-39)

**Gaps to synthesise.**
- GAP-01, GAP-02, GAP-03 (composition-not-invention framing, Theme
  MP-12).
- GAP-14 (TCT generalisation — philosophical framing).
- GAP-21, GAP-22, GAP-23, GAP-24 (evaluation-metric call-out for the
  field).
- Agent-17 R3 (pool-relative scope) reiterated.

### §8 Limitations (agent-40)

**Gaps to disclose (§8 is the honest-reporting landing zone).**
- GAP-06 hedge (count is upper bound).
- GAP-09 (V11 hedge; independence assumed).
- GAP-10, GAP-11, GAP-12 (partial hedges).
- GAP-25 (closed-artefact frontier disclosure).
- GAP-26 (projected-vs-measured ratio, 1:2).
- GAP-29 (single-subject).
- GAP-30 (single-language / English-only).
- GAP-31 (single cultural context).
- MP-14 (distributed SINGLE-SOURCE risk across 5 clusters).
- MP-15 (temporal-narrative-moment hedge on Q4-2024 signals).
- MP-16 (pool-relative scope).

**Required paragraphs (agent-14 §8 discipline).**
1. Open-scorer / disclosed-forensic mitigation applies to **our**
   numbers only (MP-04 hedge).
2. Case 3 is the only measured anchor; Cases 1 + 2 are projected (R4).
3. Single-subject / single-encoder-pair / English-only scope.
4. Pool-relative absence claims only.

### §8.6 Ethics paragraph (agent-40)

**Gaps to discuss.**
- GAP-14 (TCT for non-identity safety targets — negative constellations
  open).
- GAP-19 (safety-profile constellation existence — future).
- Q19 detector / Constellation Defender reference.

### §9 Conclusion (agent-39)

**Gaps to reference (high-level).**
- GAP-01, GAP-02, GAP-03 (three load-bearing composition moves).
- MP-14 (distributed risk).

### §10 Future Work (agent-41)

**Primary gaps (CRITICAL / HIGH):**

**CRITICAL.**
- GAP-09 / GAP-17 (formal embedder-independence — protocol + measurement):
  MINE / InfoNCE across the 13-embedder panel on a fixed corpus.

**HIGH.**
- GAP-10 / GAP-18 (scaling-law re-fit under DDA-enriched supervision).
- GAP-12 (TCT-reward non-equivalence proof; Bradley-Terry-recovery
  analysis).
- GAP-14 / GAP-19 (TCT for safety targets / negative constellations;
  paired ethics framing).
- GAP-16 (guard scaling — accept-rate budget analysis).
- GAP-20 (sample-complexity curve for Case 3).
- GAP-22 / GAP-24 (guard rejection-rate benchmark / guard-on vs.
  guard-off ablation protocol).
- GAP-23 (multi-encoder identity-cohesion score definition).
- GAP-29 (multi-subject same-session band study, K ≥ 50 speakers).
- GAP-30 (cross-language transportability study).

**MEDIUM.**
- GAP-11 (non-collapse corollary; optional formal theorem).
- GAP-13 (MI-adjusted MC ratio).
- GAP-15 (cross-encoder SECS identification theory).
- GAP-21 (signals-per-input-per-bit metric).
- GAP-25 (field-problem disclosure; re-call for open releases).
- GAP-27 (third-party Case 3 replication invitation).
- GAP-28 (ClipCannon DAG replication on release).
- GAP-31 (cross-cultural portability).

**§10 structure recommendation (agent-41).**
- §10.1 **Empirical measurements** (GAP-17, GAP-18, GAP-20, GAP-22,
  GAP-24, GAP-29, GAP-30).
- §10.2 **Theoretical extensions** (GAP-09 + GAP-13, GAP-12, GAP-16).
- §10.3 **Paradigm extensions** (GAP-14 / GAP-19 ethics; GAP-11
  non-collapse corollary).
- §10.4 **Metric + benchmark definitions** (GAP-21, GAP-23).
- §10.5 **Replication invitations** (GAP-26, GAP-27, GAP-28).
- §10.6 **Cultural / linguistic scope** (GAP-31 paired with GAP-30).

---

## Cross-references

- **Upstream ledger:** `agent-18-gap-hunter--consolidated-gaps.md`.
- **Thematic frameworks:** `agent-17-thematic-synthesizer--thematic-frameworks.md`.
- **Narrative arc:** `agent-17-thematic-synthesizer--narrative-arc.md`.
- **Section ownership:** `agent-05-research-planner--rewoo-plan.md`.
- **Bias mitigation §8 discipline:** `agent-14-bias-detector--bias-mitigations.md`.

## Confidence

**HIGH.** Every gap in the ledger has exactly one primary section and
at least one secondary section. Every writer-agent (36, 37, 38, 39, 40,
41) has an explicit list of gaps to cover. §10 structure has six
sub-sections organised by dimension.

## Changelog

- 2026-04-17 — Initial issue by agent-18-gap-hunter.
