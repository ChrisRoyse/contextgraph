---
namespace: patterns
agent_id: agent-05-research-planner
timestamp: 2026-04-16T16:15:00Z
status: active
tags: tasking, rewoo, compact-table, agents-06-48, plan
depends_on:
  - memory/decisions/agent-05-research-planner--section-ownership.md
  - memory/patterns/agent-05-research-planner--quality-gates.md
  - memory/patterns/agent-05-research-planner--rewoo-plan.md
---

# Compact Agent Tasking Table — agents 06–48

One row per agent. Columns: primary input memory files to read, primary task (one
sentence), primary output files to write, the failure mode (from FM-01 … FM-10) this
agent MUST defuse, and the essential question (from Q01–Q20) this agent answers.

Every agent additionally writes `./memory/reports/agent-{N}-{role}--final-report.md`
and `./memory/handoff/agent-{N}-{role}--{descriptor}.md`; these are omitted from the
table for brevity.

---

## Phase 2 — Literature & evidence (agents 06–13)

| # | Role | Reads | Task | Writes | Defuses | Answers |
|---|---|---|---|---|---|---|
| 06 | literature-mapper | `_evidence_01`..`_evidence_04`; `agent-03--essential-questions`; `agent-04--definition-blocks`; `section-ownership`; prior `docs/researchpaper/02_related_work.md` | Build the field topology: clusters for scaling-law / data-wall; synthetic-data and collapse; compression (bit/weight/activation/meaning); multi-modal embedders (CLIP/SigLIP/WavLM); voice cloning (VALL-E 2); alignment (RLHF/DPO/CAI). Map each cluster to the Q-IDs it serves. | `patterns/agent-06--field-topology.md` | FM-10 citation padding | Q01, Q03 |
| 07 | source-tier-classifier | agent-06 output; citation pools | Tier every candidate citation T1 (peer-reviewed with working DOI) / T2 (preprint with artefact) / T3 (blog, technical report) / T4 (authoritative web). Target ≥ 80% T1 or T2. | `decisions/agent-07--tier-classification.md` | FM-10 | Q01, Q02, Q03, Q09 |
| 08 | citation-extractor | agent-06 and agent-07 outputs; `_evidence_0*`; prior `13_references.md` | Extract canonical APA records for every cited source, with working URL/DOI, venue, year, authors. Deduplicate. Emit master reference database. | `patterns/agent-08--citation-database.md` | FM-10 | all Q referencing prior work |
| 09 | methodology-scanner | agent-06 output; prior `06_method_tct.md`, `09_case_voice_clone.md` | Inventory the methodologies used in related work (contrastive learning, preference optimisation, zero-shot TTS, flow-matching video, diffusion-guided generation). Tabulate loss form, training data, evaluation metric. Surface what none of them do: per-output runtime geometric guard. | `patterns/agent-09--methodology-patterns.md` | FM-07 re-explaining prior draft | Q06, Q09, Q10, Q11 |
| 10 | theoretical-framework-analyst | agent-06, agent-09 outputs; `agent-04--tct-definition`, `--dda-definition`, `--meaning-compression-definition` | Map theoretical frameworks (information theory, geometric ML, prototypical networks, alignment taxonomy) onto DDA / TCT and surface the nearest precedent for each Tier-A construct. | `patterns/agent-10--framework-map.md` | FM-02 DDA-as-synthetic-data confusion | Q02, Q04, Q05, Q06, Q09 |
| 11 | systematic-reviewer | agents 06–10 outputs | PRISMA-style review of scaling-law + data-wall + synthetic-data + multi-embedder literature. Inclusion/exclusion criteria explicit. | `reports/agent-11--prisma-synthesis.md` | FM-10 | Q01, Q02, Q03 |
| 12 | context-tier-manager | agents 06–08 outputs | Bucket sources into hot / warm / cold tiers for each section; provide a writer-facing lookup table. | `patterns/agent-12--hot-warm-cold.md` | FM-07 | — (infrastructure) |
| 13 | quality-assessor | agent-07 output; agent-08 citation database | Score every citation with a CASP-lite rubric. Flag any Tier-1 that fails rigor spot-check. Gate QG1 evaluator. | `decisions/agent-13--quality-scores.md` | FM-10 | — (gatekeeping) |

## Phase 3 — Analysis & synthesis (agents 14–19)

| # | Role | Reads | Task | Writes | Defuses | Answers |
|---|---|---|---|---|---|---|
| 14 | bias-detector | agent-13 output; agent-11 PRISMA | Flag citation-biases (venue bias, geographic bias, n=1 evaluation, encoder-as-training-signal); surface the author-is-subject bias in Case 3 explicitly. | `patterns/agent-14--bias-flags.md` | FM-05 hidden single-subject bias | Q16, Q19 |
| 15 | evidence-synthesizer | agents 11, 13, 14 outputs | Meta-synthesis across the four clusters: what the literature agrees on; what it disputes; where it is silent (silence := gap for agent-18). | `patterns/agent-15--meta-synthesis.md` | FM-02, FM-03 AGI overclaim | Q02, Q03, Q05, Q09 |
| 16 | pattern-analyst | agent-15 output; case-study drafts in `docs/researchpaper/07..09_case_*` | Cross-study patterns across Cases 1, 2, 3: what recurs (reference-set → centroid → guard); what varies (modalities; loss weights). | `patterns/agent-16--cross-study-patterns.md` | FM-04 measured/projected conflation | Q07, Q12, Q13, Q15 |
| 17 | thematic-synthesizer | agent-15, agent-16 outputs | Thematic synthesis. Produce the five themes that §2 and §7 rest on: (i) data-wall reframing; (ii) DDA not synthetic-data; (iii) meaning compression as a fourth category; (iv) TCT as geometric-not-scalar alignment; (v) sample-complexity collapse. | `patterns/agent-17--themes.md` | FM-07 re-explaining | Q01..Q06, Q15 |
| 18 | gap-hunter | agent-04 constructs; agent-17 themes | Enumerate ≥ 15 gaps: formal embedder-independence measurement, cross-encoder SECS generalisation, non-author subjects, multi-language, hierarchical constellations, safety-profile modality, etc. | `patterns/agent-18--gaps.md` | FM-03 | Q16, Q20 |
| 19 | opportunity-identifier | agent-18 gaps | Classify gaps into (a) §9 future-work items, (b) §8 limitations items, (c) out-of-scope. | `patterns/agent-19--opportunities.md` | FM-03 | Q20 |

## Phase 4 — Theory & model building (agents 20–24)

| # | Role | Reads | Task | Writes | Defuses | Answers |
|---|---|---|---|---|---|---|
| 20 | interdisciplinary-pollinator | agent-17 themes | Surface isomorphic patterns from adjacent fields (information theory's coding theorems; psycholinguistics' prototype vs exemplar debate; astronomy's constellation reference frames as metaphor only — no over-extension). | `patterns/agent-20--isomorphisms.md` | FM-03 | Q06 |
| 21 | theory-builder | agent-04 constructs; agent-17 themes; agent-20 isomorphisms | Consolidate the theoretical spine: DDA count derivation with explicit independence hedge (V11); meaning-compression taxonomy; TCT three-phase with acceptance predicate. Reproduce each piece of math from the locked definitions; do NOT re-derive. | `decisions/agent-21--dda-tct-theory.md` | FM-02 | Q04, Q05, Q06, Q07 |
| 22 | model-architect | agent-21 theory | Draw the conceptual model block-diagram (pseudocode / Mermaid-equivalent in Markdown): corpus → frozen embedders → signals → centroid → constellation guard. No code. | `patterns/agent-22--model-diagram.md` | FM-06 code in body | Q07, Q10, Q11 |
| 23 | hypothesis-generator | agents 21, 22 outputs | Produce ≥ 5 testable hypotheses tied to measurable outcomes: H1 WavLM SECS ≥ 0.95 on 10 held-out with 50-clip centroid; H2 DNSMOS P.808 parity; H3 constellation-guard accept-rate above chance on Shakespeare held-out; H4 visual-centroid cosine ≥ $\tau_v$ on Santa held-out; H5 sample-complexity collapse under ablation of N. | `decisions/agent-23--hypotheses.md` | FM-04 | Q08, Q12, Q13, Q15 |
| 24 | causal-simulator | agents 21, 23 outputs | Counterfactual / circular-reasoning audit. (a) If embedder $\phi_m$ is replaced by noise, does DDA survive? (b) If $\tau_m$ → 0, does guard become vacuous? (c) Shakespeare echo-bug counterfactual. | `patterns/agent-24--counterfactuals.md` | FM-08 silent failure hiding | Q16, Q17 |

## Phase 5 — Design (agents 25–28)

| # | Role | Reads | Task | Writes | Defuses | Answers |
|---|---|---|---|---|---|---|
| 25 | method-designer | `agent-04--tct-definition`; agents 21, 22, 23 outputs | Specify the TCT three-phase protocol as pseudocode only; cite `tct-definition.md` §3.2 verbatim. Specify per-case instantiation (Case 1/2/3). | `decisions/agent-25--tct-protocol.md` | FM-06 | Q06, Q07 |
| 26 | sampling-strategist | agent-25 output; prior cases | For each case, name: reference-set size, held-out size, split rule, subject cohort (with author-is-subject disclosure for Case 3). | `decisions/agent-26--sampling.md` | FM-05 | Q08, Q12, Q13 |
| 27 | instrument-developer | agent-25 protocol | Nail every evaluation metric: WavLM SECS (checkpoint ID, pooling), DNSMOS P.808 (protocol version), UTMOS (checkpoint ID), visual cosine (CLIP/SigLIP name + preprocessing), lip-sync metric. Always qualify SECS by encoder (V4). | `decisions/agent-27--metrics.md` | FM-04 | Q08, Q12, Q14 |
| 28 | analysis-planner | agents 25, 26, 27 outputs | Statistical / qualitative analysis plan. Case 3: point estimate + max + parity comparison to real recordings; explicit "n=10 held-out, n=1 speaker" scope. Cases 1 & 2: projected metrics with "(projected; camera-ready will re-verify)" flag. | `decisions/agent-28--analysis-plan.md` | FM-04, FM-05 | Q08, Q12, Q13, Q14 |

## Phase 6 — Quality & risk (agents 29–34)

| # | Role | Reads | Task | Writes | Defuses | Answers |
|---|---|---|---|---|---|---|
| 29 | validity-guardian | agents 25–28 outputs; `_evidence_01..04` | Draft §8 Limitations covering six G9 items: small-n Case 3; pending-runs Cases 1/2; orthogonality hedge; non-generalisation; metric caveats (SECS is English-biased via WavLM; DNSMOS/UTMOS are MOS-proxies); author-bias. | `decisions/agent-29--limitations.md` (+ `sections/08_limitations.md` draft) | FM-05, FM-08 | Q16, Q19 |
| 30 | risk-analyst | agents 25–29 outputs | FMEA with ≥ 6 risks, each with severity × likelihood × mitigation: embedder non-independence, threshold drift, cross-encoder mismatch, model-collapse conflation, marketing creep, length creep. | `patterns/agent-30--fmea.md` | FM-01, FM-02, FM-03 | Q16 |
| 31 | ethics-reviewer | agents 25, 26, 29 outputs | Author-is-subject consent (explicit), impersonation-misuse (constellation guard as detector/enforcer), downstream safety (safety-profile constellations as future work), cross-language/demographic scope disclosure. | `decisions/agent-31--ethics.md` | FM-05 | Q19 |
| 32 | contradiction-analyzer | agents 15, 17 outputs; all locked definitions | Surface every contradiction between our claims and cited prior work; each must be addressed in §7 or flagged. | `patterns/agent-32--contradictions.md` | FM-08 | Q02, Q09, Q14 |
| 33 | falsification-agent | agents 21, 23 outputs | Produce ≥ 5 falsifiers; ≥ 1 per Tier-A construct. Each falsifier has a response ("would be falsified if X; we would retract"). | `patterns/agent-33--falsifiers.md` | FM-03, FM-04 | Q16 |
| 34 | confidence-quantifier | agents 15, 29, 32, 33 outputs | Per-claim confidence table for 100% of numeric claims: {claim, value, tier T1–T4, source, page/line, confidence}. This table is the G1 provenance artefact. | `decisions/agent-34--per-claim-confidence.md` | FM-04 | all numeric Q |

## Phase 7 — Writing (agents 35–41)

| # | Role | Reads | Task | Writes | Defuses | Answers |
|---|---|---|---|---|---|---|
| 35 | introduction-writer | `mission-brief`, `agent-01..04` locked files; `agent-15` meta-synthesis; `agent-17` themes; `section-ownership` | Draft §1 Introduction (900–1,200 words): three paths framing (acquire / synthesise / DDA); first-use of all four Tier-A constructs; first-use V10 geometric-alignment disambiguation; AGI-ban clean; paragraph naming Case 3 as the T1 anchor. | `../sections/01_introduction.md` | FM-01, FM-03 | Q01, Q03, Q04 (teaser), Q06 (teaser) |
| 36 | literature-review-writer | agents 10, 11, 15, 17, 18 outputs; agent-08 citations | Draft §2 Related Work (1,200–1,800 words) in six sub-lines (scaling/data-wall; synthetic-data and collapse; compression taxonomy; alignment; multi-modal embedders and voice cloning). Produce 4-row novelty-delta table (G3). | `../sections/02_related_work.md` | FM-07, FM-10, FM-02 | Q01, Q02, Q03, Q05, Q06, Q09 |
| 37 | methodology-writer | `agent-04--definition-blocks`; agents 21, 22, 25, 27, 28 outputs | Draft §3 Framework (1,200–1,600 words) + §4 Systems (700–900 words) + §5 TCT Method (1,100–1,400 words). Reproduces locked definitions verbatim; builds the 4-category taxonomy; three-phase algorithm block. No Python / Rust in body. | `../sections/03_framework.md`, `../sections/04_systems.md`, `../sections/05_tct.md` | FM-02, FM-06, FM-09 | Q04, Q05, Q06, Q07, Q10, Q11 |
| 38 | results-writer | agents 25, 26, 27, 28, 34 outputs; SITREP; prior `07..09_case_*.md`; Santa inspect docs | Draft §6 (1,500–2,000 words) in order Case 3 (T1) → Case 2 (T2+T3) → Case 1 (T2+T3). Every subsection header carries its tier label. Forensic paragraphs for the two disclosed bugs. Every SECS qualified by encoder. | `../sections/06_cases.md` | FM-04, FM-05, FM-08 | Q12, Q13, Q14, Q15, Q17 |
| 39 | discussion-writer | agents 15, 16, 24, 32, 33 outputs; prior `10_discussion.md` | Draft §7 Discussion (800–1,100 words): data-wall *reframed*; TCT differentiator; what Case 3 shows and what it does not; integration of forensic lessons. No AGI. | `../sections/07_discussion.md` | FM-01, FM-03, FM-08 | Q15, Q16, Q17 |
| 40 | conclusion-writer | agent-19 opportunities; agent-39 discussion draft | Draft §9 Conclusion (250–400 words) + polish Acknowledgements (80–150 words; sole Teleox.ai / Leapable.ai location). | `../sections/09_conclusion.md`, `../sections/10_acknowledgements.md` | FM-01, FM-03 | Q20 |
| 41 | abstract-writer | all prior sections | Draft Abstract (180–250 words) last: DDA claim ceiling; TCT differentiator; Case 3 measured numbers with encoder-qualified SECS; Cases 1 & 2 flagged projected; no AGI; no marketing. | `../sections/00_abstract.md` | FM-01, FM-03, FM-04 | synthesises Q01, Q04, Q06, Q12, Q15 |

## Phase 8 — Review & polish (agents 42–48)

| # | Role | Reads | Task | Writes | Defuses | Answers |
|---|---|---|---|---|---|---|
| 42 | adversarial-reviewer | all sections; all locked definitions; G4 regex | Red-team every section as a skeptical cs.LG reviewer. Run G4 regex; audit G2 consistency; audit G3 novelty-delta specificity. Produce a findings list with severity. BLOCKING on any H-gate failure. | `reports/agent-42--adversarial-findings.md` | every FM | — (gatekeeping) |
| 43 | reproducibility-checker | §5, §6, §8.5, appendix; agents 25, 26, 27, 28 | Produce the 5×6 matrix (three cases × two systems) × (input provenance; model + dims; hyperparams; evaluation metric definition; seed/determinism; reproduction pointer). BLOCKING on G6. | `reports/agent-43--reproducibility-matrix.md` | FM-04, FM-06 | Q18 |
| 44 | apa-citation-specialist | agent-08 citation database; all sections | Finalise APA 7th formatting; normalise in-text citations. | `../sections/11_references.md` | — | — |
| 45 | citation-validator | agent-44 references | Verify every URL / DOI; produce URL-health report. G7. | `reports/agent-45--url-health.md` | FM-10 | — |
| 46 | file-length-manager | assembled `PAPER.md` | Word count + reference count; enforce G5; split files > 1500 lines per SMAP rule. | `reports/agent-46--length-report.md` | FM-07 | — |
| 47 | graph-gardener | `./memory/` tree | Prune dead notes; merge near-duplicates; produce final memory-graph summary. | `reports/agent-47--memory-prune.md` | — | — |
| 48 | adaptive-orchestrator | all reports; all 9 gates | Final sign-off scorecard. If any H-gate fails, select the remediation agent by cost column in G-table and re-run. Otherwise approve submission. | `reports/agent-48--final-scorecard.md` | — (final authority) | — |

---

## Compact dependency reminders

- Agent-06 cannot start until agent-05's three planner files (this one, section-ownership,
  quality-gates) are on disk. ✅ satisfied by this agent's outputs.
- Agent-13 gates QG1 → blocks agents 14+.
- Agent-34 gates QG6 → blocks agents 35+.
- Agent-38 cannot finalise §6 until agent-34 confidence table is complete (for §6 tier
  labels and provenance).
- Agent-41 abstract writes last (after 35–40). Mission brief constraint.
- Agent-42 is the first blocking reviewer; if it fails, rework specific sections, then
  re-run agent-42. Budget ≤ 2 adversarial rounds.
- Agent-48 is the final signer.

---

## Change log

- 2026-04-16 — Initial issue by agent-05-research-planner.
