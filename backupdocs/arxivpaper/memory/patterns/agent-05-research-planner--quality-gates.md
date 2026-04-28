---
namespace: patterns
agent_id: agent-05-research-planner
timestamp: 2026-04-16T16:15:00Z
status: active
tags: quality-gates, checkpoints, pass-fail, stop-proceed, rewoo
depends_on:
  - memory/patterns/agent-01-step-back-analyzer--evaluation-criteria.md
  - memory/decisions/agent-01-step-back-analyzer--failure-modes-to-avoid.md
  - memory/decisions/agent-03-self-ask-decomposer--essential-questions.md
  - memory/decisions/agent-05-research-planner--section-ownership.md
---

# Quality Gates — Pipeline Checkpoints

Nine **arXiv-readiness gates** (from agent-01) are the final exam. Before we reach them,
the pipeline passes eight **intermediate checkpoints** (QG1–QG8). Each checkpoint is a
STOP / PROCEED decision for the coordinator. A checkpoint that fails triggers a rework
loop back to a named agent; it does not trigger a restart.

All checkpoints are evaluated against canonical vocabulary (glossary), named failure modes
(FM-01 … FM-10), and the nine final gates (G1–G9).

---

## QG1 — Evidence base complete (after agent-13)

**Triggered by:** agent-13 quality-assessor final report.

**Criteria.**
- [ ] ≥ 30 references in the pool (mission-brief minimum).
- [ ] ≥ 80% of pool tagged Tier 1 or Tier 2 per agent-07 (T1 = peer-reviewed venue;
  T2 = preprint with reproducible artefact).
- [ ] Every citation in the pool has full APA metadata and a verified URL (agent-08).
- [ ] Each of the 20 essential questions (Q01–Q20) has ≥ 1 Tier-1 citation supporting
  its answer shape, unless marked ANSWERABLE-by-construction (e.g., Q04 DDA counts).
- [ ] `_evidence_01` through `_evidence_04` fully re-ingested; no URL drift detected.

**Pass → proceed to agent-14.** **Fail → rework to agent-06 (expand search) or
agent-07 (re-tier).**

**Enforcer:** coordinator. **Downstream dependency:** §2 Related Work correctness.

---

## QG2 — Synthesis coherent (after agent-17)

**Triggered by:** agent-17 thematic-synthesizer output.

**Criteria.**
- [ ] Cross-study patterns (agent-16) are consistent with the theoretical framework
  (agent-21 ready to build on top; no contradictions flagged by agent-32).
- [ ] Thematic clusters align with §2 structure (scaling / data-wall; synthetic-data and
  collapse; compression; alignment; multi-modal embedders; identity-locked generation).
- [ ] No theme is orphaned (no cluster without a question it serves).

**Pass → proceed to agent-18 gap-hunter.** **Fail → rework to agent-15 + agent-17.**

---

## QG3 — Theory & hypotheses locked (after agent-24)

**Triggered by:** agent-24 causal-simulator.

**Criteria.**
- [ ] DDA's "approximately independent; strict orthogonality is an open question (§10)"
  hedge appears in agent-21's theory artefact verbatim (V11 lock).
- [ ] TCT's three-phase spec in agent-21 matches agent-04's locked definition
  (`tct-definition.md` §3.2).
- [ ] ≥ 5 hypotheses (agent-23) each pointing to a measurable outcome in §6.
- [ ] Causal counterfactuals (agent-24) include at least: (a) embedder-panel shrinkage
  attack on DDA; (b) scalar-reward substitution on TCT; (c) threshold-sensitivity on the
  constellation guard.
- [ ] Zero contradictions against locked definitions (G2).

**Pass → proceed to agent-25 method-designer.** **Fail → rework to agent-21 or
agent-23.**

---

## QG4 — Methodology reproducibility-ready (after agent-28)

**Triggered by:** agent-28 analysis-planner output.

**Criteria.**
- [ ] §5 TCT protocol (agent-25) reproduces `tct-definition.md` three-phase structure.
- [ ] Sampling (agent-26) names the reference-set size and held-out set size for each
  case; Case 3 reports exact 50-clip + 10-sentence split.
- [ ] Instruments (agent-27) enumerated: WavLM SECS (encoder version pinned), DNSMOS
  P.808 (protocol pinned), UTMOS (checkpoint pinned), visual cosine (CLIP/SigLIP
  version pinned), lip-sync metric named. Every instrument qualifies SECS per V4.
- [ ] Analysis plan (agent-28) specifies point estimate + variability summary + scope-
  limit disclosure (FM-05). For Case 3, documents that n=10 held-out, n=1 subject, so
  a formal CI is either reported or explicitly flagged as non-reportable.
- [ ] No Python / Rust code in the plan body (FM-06).

**Pass → proceed to validity phase (agents 29–31).** **Fail → rework to the offending
agent.**

---

## QG5 — Validity & ethics cleared (after agent-31)

**Triggered by:** agent-31 ethics-reviewer output.

**Criteria.**
- [ ] Agent-29 limitations list names every item required by G9 (sample size; pending
  runs; orthogonality hedge; non-generalisation; metric caveats; author-bias).
- [ ] Agent-30 FMEA has ≥ 6 risks each with mitigation; covers (a) embedder
  non-independence, (b) threshold calibration drift, (c) cross-encoder mismatch,
  (d) model-collapse confusion, (e) marketing creep, (f) length creep.
- [ ] Agent-31 addresses author-is-subject consent, impersonation-misuse mitigation
  (constellation guard as detector), and cross-language / cross-demographic scope limits.

**Pass → proceed to agent-32.** **Fail → rework to the offending agent; any high-severity
ethics item blocks submission.**

---

## QG6 — Adversarial resilience checked (after agent-34)

**Triggered by:** agent-34 confidence-quantifier output.

**Criteria.**
- [ ] Agent-32 surfaces every contradiction between our claims and cited prior work;
  each is either addressed in §7 or §8 with a citation, or flagged as open.
- [ ] Agent-33 produces ≥ 5 falsifiers; at least one per Tier-A construct. A falsifier
  example for TCT: "if per-modality thresholds $\tau_m$ can be arbitrarily lowered
  without quality loss, the constellation guard is vacuous". Each has a response.
- [ ] Agent-34 per-claim confidence table covers 100% of numeric claims with columns
  {claim, value, tier (T1–T4), source, page/line, confidence}. This directly satisfies
  G1 provenance.
- [ ] Zero FM-04 conflations in any prior agent's output when spot-checked.

**Pass → proceed to writing phase.** **Fail → rework to agent-32 / 33 / 34.**

---

## QG7 — Writing phase per-section pass (after each of agents 35–40)

**Triggered by:** each writer agent's output.

**Criteria (applied to each section).**
- [ ] Word count inside the allotted range (section-ownership table).
- [ ] Every construct first-use matches the locked definition block.
- [ ] Every SECS is qualified by encoder (V4).
- [ ] Every numeric value carries T1–T4 label or the "(projected; camera-ready will
  re-verify)" flag (V22, FM-04).
- [ ] No forbidden-term match (G4 regex).
- [ ] Every in-section citation is in the agent-08 / agent-44 reference database.
- [ ] Pseudocode only; no Python / Rust blocks (FM-06).
- [ ] DDA first use followed within 2 sentences by "not synthetic data" (V6, FM-02).
- [ ] First "alignment" carries V10 geometric-vs-reward disambiguation.

**Per-section-specific additions.**
- §1 Introduction: all four Tier-A constructs first-used; all three paths (acquire,
  synthesise, DDA) named; AGI ban (FM-03) clean.
- §2 Related Work: agent-36 produces the 4-row novelty-delta table (G3).
- §3 Framework: 4-category taxonomy table present; $N + \binom{N}{2}$ equation present;
  V11 hedge present.
- §5 TCT: three-phase algorithm block; acceptance predicate; RLHF/DPO/CAI contrast
  paragraph.
- §6 Case 3 header tagged "(T1)"; Cases 1 & 2 headers tagged "(T2+T3)"; forensic
  paragraphs for the two disclosed bugs.
- §7 Discussion: no AGI sentence; data-wall reframed (not solved).
- §8 Limitations: all six G9 items present.
- Acknowledgements: Teleox.ai / Leapable.ai mentioned here only; body clean (V24 / V25).

**Pass per section → next writer.** **Fail → writer rework; ≤ 2 iterations before
escalation to agent-42.**

---

## QG8 — Final manuscript ready (after agent-41 abstract)

**Triggered by:** agent-41 abstract-writer output.

**Criteria.**
- [ ] Abstract cites Case 3's measured numbers (WavLM SECS 0.961 mean; DNSMOS P.808
  parity 3.93; UTMOS parity 3.012) and flags Cases 1 & 2 as projected.
- [ ] Abstract states DDA claim ceiling ("reframes the data wall"; not "solves").
- [ ] Abstract states TCT differentiator vs scalar-reward alignment in one sentence.
- [ ] Abstract length 180–250 words.
- [ ] Assembled PAPER.md satisfies G5 word count 7,500–12,500.
- [ ] All nine locked definitions consistent from abstract through conclusion.

**Pass → proceed to review phase (agents 42–48).** **Fail → rework to agent-41 and
affected writers.**

---

## Final gates (G1–G9) — re-stated here for traceability

| Gate | Owner | Confidence | Blocking |
|---|---|---|---|
| G1 Provenance integrity | agents 34, 43 | H | YES — desk-reject risk |
| G2 Definition-before-use | agents 04, 42 | H | YES — semantic drift kills trust |
| G3 Novelty delta specificity | agents 36, 42 | H | YES — overclaim risk |
| G4 No-marketing regex | agent 42 | H | YES — desk-reject risk |
| G5 Length budget | agent 46 | H | YES |
| G6 Reproducibility completeness | agent 43 | H | YES |
| G7 Citation validity | agents 44, 45 | M | NO (≤ 5% transient tolerated) |
| G8 Empirical tier labeling | agents 38, 42 | M | NO (fixable in annotation pass) |
| G9 Limitations completeness | agents 29, 42 | M | NO (fixable in one rewrite) |

Precedence for conflict resolution (copied from agent-01): G1 > G4 > G2 > G6 > G3 > G8 >
G9 > G7 > G5.

---

## Critical-path gate priority

The three gates most likely to fail late and cost the most to fix (and thus the ones
every earlier checkpoint must defend):

1. **G6 Reproducibility completeness.** If §5 / §6 / §8.5 do not supply the 5×6 matrix
   (three cases × two systems; six reproducibility items each), agent-43 blocks
   submission. Remediation cost: HIGH. Defended by QG4.
2. **G1 Provenance integrity.** If agent-34's per-claim table leaves any numeric value
   unlabelled, agent-43 blocks. Remediation: LOW per claim but COUNT-proportional.
   Defended by QG6.
3. **G2 Definition-before-use.** If any writer paraphrases a locked definition, agent-42
   blocks. Remediation: LOW. Defended by QG7.

---

## Change log

- 2026-04-16 — Initial issue by agent-05-research-planner.
