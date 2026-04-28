---
namespace: decisions
agent_id: agent-05-research-planner
timestamp: 2026-04-16T16:15:00Z
status: active
locked: true
supersedes: none
depends_on:
  - memory/context/mission-brief.md
  - memory/context/agent-order.md
  - memory/patterns/agent-01-step-back-analyzer--evaluation-criteria.md
  - memory/decisions/agent-02-ambiguity-clarifier--canonical-glossary.md
  - memory/decisions/agent-03-self-ask-decomposer--essential-questions.md
  - memory/patterns/agent-04-construct-definer--definition-blocks-for-writers.md
  - memory/handoff/agent-04-to-05--constructs-locked.md
tags: section-ownership, writers, writing-phase, locked, arxiv-structure
---

# Section Ownership Map — LOCKED

Authoritative mapping of arXiv manuscript sections to the agents that *write* them
(agents 35–41) and the agents that *supply evidence* to them (agents 06–34). Any change
requires a coordinator-signed supersede. Writers MUST NOT trade sections without a
blocker and coordinator resolution.

**Manuscript frame (locked, per mission brief):** Abstract, §1 Introduction, §2 Related
Work, §3 Framework (DDA + meaning compression), §4 Systems (Context Graph + ClipCannon),
§5 TCT method, §6 Case Studies (three), §7 Discussion, §8 Limitations, §9 Conclusion,
Acknowledgements, References. Target length 7.5K–12.5K prose words; 30–60 references; ≤ 30
typeset pages.

---

## Primary ownership (one writer per section — the "pen")

| Section | Writer | Target words | Essential questions answered | Tier-A constructs invoked | Output file |
|---|---|---|---|---|---|
| Abstract | agent-41 abstract-writer | 180–250 | Synthesises Q01, Q04, Q06, Q12, Q15 | All four | `../sections/00_abstract.md` |
| §1 Introduction | agent-35 introduction-writer | 900–1,200 | Q01, Q03, Q04 (teaser), Q06 (teaser) | DDA, meaning compression, TCT, identity-locked generation (all four first-used) | `../sections/01_introduction.md` |
| §2 Related Work | agent-36 literature-review-writer | 1,200–1,800 | Q01, Q02, Q03, Q05, Q06, Q09 | Framing of DDA vs collapse; TCT vs RLHF/DPO/CAI | `../sections/02_related_work.md` |
| §3 Framework | agent-37 methodology-writer (with agent-21 theory spine) | 1,200–1,600 | Q04, Q05 | DDA (§3.1 + §3.4), meaning compression (§3.1 + §3.5), DDA-vs-synthetic (§3.3), orthogonality hedge (§3.4) | `../sections/03_framework.md` |
| §4 Systems | agent-37 methodology-writer | 700–900 | Q10, Q11 | DDA at $N=13$ (Context Graph) and $N=7$ (ClipCannon) | `../sections/04_systems.md` |
| §5 TCT Method | agent-37 methodology-writer | 1,100–1,400 | Q06, Q07, Q08 | TCT three-phase protocol; identity-locked predicate | `../sections/05_tct.md` |
| §6 Case Studies | agent-38 results-writer | 1,500–2,000 | Q12, Q13, Q14, Q15, Q17 | Identity-locked (T1 for Case 3; T2+T3 for Cases 1–2) | `../sections/06_cases.md` |
| §7 Discussion | agent-39 discussion-writer | 800–1,100 | Synthesises Q01–Q17 | All four — narrative integration | `../sections/07_discussion.md` |
| §8 Limitations | agent-29 validity-guardian (drafter) → agent-39 (polish) | 500–700 | Q16, Q18 (proprietary items), Q19 (ethics hook) | Orthogonality hedge; author-is-subject; single-subject; cross-encoder caveat | `../sections/08_limitations.md` |
| §9 Conclusion | agent-40 conclusion-writer | 250–400 | Q20 | Restates claim ceiling; no AGI | `../sections/09_conclusion.md` |
| Acknowledgements | agent-35 (drafter) → agent-40 (polish) | 80–150 | — | Sole location for Teleox.ai / Leapable.ai per V24 / V25 / V8 | `../sections/10_acknowledgements.md` |
| References | agent-44 apa-citation-specialist + agent-45 citation-validator | 30–60 entries | All | — | `../sections/11_references.md` |

**Total prose budget:** 9,440–13,500 words (covers G5 gate with margin on both ends).
Trim first from §2 and §7; preserve §3, §5, §6.3 under length pressure (G5 tie-breaker).

---

## Case-study sub-ownership (inside §6)

Each case-study subsection is written by agent-38 but has distinct evidence tiers and
owner-agents that supply material. Each subsection header is tagged per V20/G8.

| Subsection | Tier | Primary evidence supplier | Forensic supplier | Methodology supplier |
|---|---|---|---|---|
| §6.1 Case 1 — Shakespeare stylistic LoRA | **T2+T3** | agent-10 (framework analyst); prior `docs/researchpaper/07_case_shakespeare.md`; `scripts/shakespeare_full_pipeline.py` | agent-38 cites Shakespeare echo-bug FSV log | agent-25 method designer; agent-26 sampling (corpus split) |
| §6.2 Case 2 — Santa identity-locked talking head | **T2+T3** | agent-10; prior `docs/researchpaper/08_case_santa_lora.md`; Santa `docs/inspect/08_training_audit.md` | agent-38 cites flow-matching sign-bug per glossary #30 | agent-25; agent-27 (instruments: visual cosine, lip-sync) |
| §6.3 Case 3 — Voice cloning ≥ 0.95 WavLM SECS | **T1** | SITREP `tmp/SITREP_2026-04-16_135543.md`; `tasks/newdata/alldata.md` | (none — no open bug) | agent-25; agent-27 (WavLM SECS, DNSMOS P.808, UTMOS); agent-28 (statistical plan) |

**Ordering rule.** §6 is presented Case 3 → Case 2 → Case 1 (strongest evidence first) so
the adversarial reviewer encounters T1 evidence before T2+T3 projections. This ordering
is locked; writers do not re-order without a coordinator blocker.

---

## Evidence-supplier agents (06–34) — section destinations

For each evidence agent, the sections its output populates. Writers (35–41) READ the
supplier's memory files; suppliers do not write manuscript prose.

| Supplier agent | Primary destination sections | Key memory outputs writers pull from |
|---|---|---|
| agent-06 literature-mapper | §2 | `./memory/patterns/agent-06--field-topology.md` |
| agent-07 source-tier-classifier | §2, §6, References | `./memory/decisions/agent-07--tier-classification.md` |
| agent-08 citation-extractor | all sections, References | `./memory/patterns/agent-08--citation-database.md` |
| agent-09 methodology-scanner | §2, §5 | `./memory/patterns/agent-09--methodology-patterns.md` |
| agent-10 theoretical-framework-analyst | §2, §3, §5 | `./memory/patterns/agent-10--framework-map.md` |
| agent-11 systematic-reviewer | §2 | `./memory/reports/agent-11--prisma-synthesis.md` |
| agent-12 context-tier-manager | all sections | `./memory/patterns/agent-12--hot-warm-cold.md` |
| agent-13 quality-assessor | §2, References | `./memory/decisions/agent-13--quality-scores.md` |
| agent-14 bias-detector | §2, §7, §8 | `./memory/patterns/agent-14--bias-flags.md` |
| agent-15 evidence-synthesizer | §2, §3, §7 | `./memory/patterns/agent-15--meta-synthesis.md` |
| agent-16 pattern-analyst | §7 | `./memory/patterns/agent-16--cross-study-patterns.md` |
| agent-17 thematic-synthesizer | §2, §7 | `./memory/patterns/agent-17--themes.md` |
| agent-18 gap-hunter | §2, §7, §8, §9 | `./memory/patterns/agent-18--gaps.md` |
| agent-19 opportunity-identifier | §9 | `./memory/patterns/agent-19--opportunities.md` |
| agent-20 interdisciplinary-pollinator | §2, §7 | `./memory/patterns/agent-20--isomorphisms.md` |
| agent-21 theory-builder | §3 (with §3.4 hedge), §5 | `./memory/decisions/agent-21--dda-tct-theory.md` |
| agent-22 model-architect | §3, §5 | `./memory/patterns/agent-22--model-diagram.md` |
| agent-23 hypothesis-generator | §3, §6, §8 | `./memory/decisions/agent-23--hypotheses.md` |
| agent-24 causal-simulator | §7, §8 | `./memory/patterns/agent-24--counterfactuals.md` |
| agent-25 method-designer | §5, §6 | `./memory/decisions/agent-25--tct-protocol.md` |
| agent-26 sampling-strategist | §6 | `./memory/decisions/agent-26--sampling.md` |
| agent-27 instrument-developer | §5, §6 | `./memory/decisions/agent-27--metrics.md` |
| agent-28 analysis-planner | §6, §8 | `./memory/decisions/agent-28--analysis-plan.md` |
| agent-29 validity-guardian | §8 (drafter) | `./memory/decisions/agent-29--limitations.md` |
| agent-30 risk-analyst | §7, §8 | `./memory/patterns/agent-30--fmea.md` |
| agent-31 ethics-reviewer | §8.6 | `./memory/decisions/agent-31--ethics.md` |
| agent-32 contradiction-analyzer | §7 | `./memory/patterns/agent-32--contradictions.md` |
| agent-33 falsification-agent | §7, §8 | `./memory/patterns/agent-33--falsifiers.md` |
| agent-34 confidence-quantifier | §6, §7, §8 | `./memory/decisions/agent-34--per-claim-confidence.md` |

---

## Review-phase ownership (42–48) — who audits what

| Reviewer agent | Audits | Blocking authority |
|---|---|---|
| agent-42 adversarial-reviewer | Whole manuscript; regex G4; novelty deltas G3; definition consistency G2 | Can block submission on any H-gate fail |
| agent-43 reproducibility-checker | §5, §6, §8.5, appendix; G6 reproducibility matrix | Can block submission on G6 fail |
| agent-44 apa-citation-specialist | References section; in-text citation format | MEDIUM — coordinates with agent-45 |
| agent-45 citation-validator | URL resolution, DOI health, APA conformance; G7 | MEDIUM — tolerates ≤ 5% transient failures |
| agent-46 file-length-manager | G5 length; splits long files | HIGH — can force cuts |
| agent-47 graph-gardener | `./memory/` tidy-up; dead-note prune | LOW — non-blocking |
| agent-48 adaptive-orchestrator | Final scorecard across all 9 gates | HIGH — final sign-off authority |

---

## Ownership change-control

Writer agents (35–41) MAY NOT expand their section beyond the word budget without
coordinator approval (agent-46 file-length-manager enforces). If a writer needs material
from a non-owner supplier, they file a read request in the supplier's memory; they do
not rewrite the supplier's output.

Suppliers (06–34) MAY NOT produce manuscript prose; they produce analysis notes.
Violating this rule triggers FM-07 (re-explaining prior manuscript).

---

## Change log

- 2026-04-16 — Initial lock by agent-05-research-planner.
