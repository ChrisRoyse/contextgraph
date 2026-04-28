---
namespace: reports
agent_id: agent-05-research-planner
timestamp: 2026-04-16T16:15:00Z
status: complete
tags: smap, final-report, research-planner, rewoo
---

# Final Report — Agent-05 research-planner (SMAP)

## S — Situation

Agents 01–04 locked the scoping layer for the arXiv manuscript: high-level guiding
principles (01), canonical glossary (02), 20 essential questions Q01–Q20 (03), and the
four Tier-A construct definitions (04). The mission brief locks the manuscript structure
(Abstract → §1 Intro → §2 Related Work → §3 Framework → §4 Systems → §5 TCT → §6 Case
Studies → §7 Discussion → §8 Limitations → §9 Conclusion → Acknowledgements → References),
the four core claims (DDA, meaning compression, TCT, sample-complexity collapse), and
the three case studies (Shakespeare T2+T3, Santa T2+T3, voice cloning T1). Agent-05's
job is to produce the ReWOO operating plan that downstream agents 06–48 execute without
re-planning.

## M — Mission

Produce (a) a master plan, (b) a compact per-agent tasking table, (c) quality gates /
checkpoints, (d) a locked section-ownership map, (e) a handoff to agent-06, and (f) this
SMAP report. Every artefact must respect the nine final gates (G1–G9), the ten named
failure modes (FM-01 … FM-10), and the locked construct / vocabulary decisions.

## A — Actions

1. Read mission brief, constraints, agent-order, all agent-01 principles and
   evaluation criteria, the canonical glossary, the essential questions, the Tier-A
   construct definition blocks, and the agent-04 handoff. Skimmed the four evidence
   briefs to confirm scope and the prior `docs/researchpaper/` for structure (no
   verbatim reuse).
2. Wrote `memory/decisions/agent-05-research-planner--section-ownership.md` — locks
   which agent writes which section (35–41), the case-study sub-ordering (Case 3
   first, T1 anchor), and the supplier-to-section map for agents 06–34. Case studies
   §6.1–§6.3 carry explicit tier labels per V20 / G8.
3. Wrote `memory/patterns/agent-05-research-planner--quality-gates.md` — eight
   intermediate checkpoints QG1–QG8 (evidence base complete; synthesis coherent; theory
   & hypotheses locked; method reproducibility-ready; validity & ethics cleared;
   adversarial resilience; writing-phase per-section pass; final manuscript ready).
   Each checkpoint names pass / fail criteria and the remediation agent.
4. Wrote `memory/patterns/agent-05-research-planner--agent-tasking-table.md` — compact
   one-row-per-agent table for agents 06–48 with columns: reads, task, writes, FM
   defused, Q answered. All 43 agents covered.
5. Wrote `memory/patterns/agent-05-research-planner--rewoo-plan.md` — master plan
   narrative with §0 scope, §1 section map, §2 evidence sources, §3 agent-by-agent
   tasking, §4 dependency graph, §5 timeline / effort budget (~52 effort units), §6
   14-item risk register (R1–R14 covering every FM plus infrastructure risks), §7
   explicit downstream instructions for agent-06, §8 done-definition checklist.
6. Wrote `memory/handoff/agent-05-to-06--plan-ready.md` — lists files agent-06 reads
   first, the six-cluster field topology it must build, the Q-IDs each cluster serves,
   the citation-tiering threshold, the TCT-delta-per-cluster requirement, and the
   blocker escalation path.
7. Wrote this SMAP report.
8. Filed no blockers. Every locked decision from agents 01–04 is planner-compatible.

## M — Metrics

Six required files produced, each with full frontmatter and cross-references:

| File | Size | Purpose |
|---|---|---|
| `decisions/agent-05-research-planner--section-ownership.md` | locked | Section ownership; writer → section + word budget map |
| `patterns/agent-05-research-planner--rewoo-plan.md` | long | Master plan narrative |
| `patterns/agent-05-research-planner--agent-tasking-table.md` | compact | One row per agent 06–48 |
| `patterns/agent-05-research-planner--quality-gates.md` | compact | 8 checkpoints + 9 final gates |
| `handoff/agent-05-to-06--plan-ready.md` | handoff | Agent-06 entrypoint |
| `reports/agent-05-research-planner--final-report.md` | SMAP | this report |

Plan coverage checklist (from agent-04 handoff "What agent-05 must do"):

- [x] Section-level plan: which construct each §1–§9 invokes, anchored to definition
  files. Covered in `section-ownership.md` + `rewoo-plan.md` §1.
- [x] Case-study plan: constructs per Case 1 / 2 / 3 with tiers. Covered in
  `section-ownership.md` Case-Study Sub-Ownership table.
- [x] Ablation / robustness plan: embedder sensitivity (Cluster $N$), $\lambda_m$ /
  $\tau_m$ sensitivity, $K$ / calibration sensitivity. Covered in `rewoo-plan.md` §3
  Phase 4 paragraph on agent-24 causal-simulator and in Risk Register R4 / R5 / R6.
- [x] No silent-invention gap: no new essential question required.
- [x] Handoff to agent-06 with file-level pointers.

## A — Analysis

**What agents 35–41 (the writers) own:** agent-35 writes §1 Introduction (900–1,200 w);
agent-36 writes §2 Related Work (1,200–1,800 w); agent-37 writes §3 + §4 + §5 (3,000–
3,900 w total); agent-38 writes §6 Case Studies in Case 3 → Case 2 → Case 1 order
(1,500–2,000 w); agent-39 writes §7 Discussion (800–1,100 w); agent-29 drafts §8
Limitations (500–700 w) which agent-39 polishes; agent-40 writes §9 Conclusion (250–400
w) and polishes Acknowledgements (80–150 w); agent-41 writes the Abstract last
(180–250 w). Combined target: 9,440–13,500 prose words, which brackets the G5 gate of
7,500–12,500 with rewrite headroom on both sides.

**Critical quality gates:** G1 provenance integrity (agent-34's per-claim confidence
table is the load-bearing artefact); G2 definition-before-use (depends on every writer
pulling agent-04's paste-ready blocks verbatim; enforced by agent-42 regex); G4
no-marketing regex (agent-42 scan; desk-reject risk if any match in body); G6
reproducibility completeness (agent-43's 5×6 matrix; HIGH remediation cost, so QG4 must
catch gaps early). Among the intermediate checkpoints, QG6 (adversarial resilience; G1
provenance ready) is the hinge — it guards the writing phase from inheriting unlabelled
numbers and unresponded falsifiers. QG7 per-section is where the writing phase's
marketing / AGI / code-in-body / citation-padding failure modes are caught before
agent-42 sees them.

**Biggest foreseeable risk:** R3 (measured-vs-projected conflation). Three case studies
at three tiers is easy to muddle in aggregate statements. Mitigation is architectural:
§6 ordering (Case 3 first), tier labels in headers, agent-34's per-claim table as a
prerequisite for §6 finalisation.

**Largest un-reduced uncertainty:** the V11 "approximately independent; strict formal
orthogonality is an open question" hedge for DDA's $\binom{N}{2}$ count. The plan
routes this to agent-18 gap-hunter and agent-20 future-work as a Q20 item. Formal
embedder-independence measurement is explicitly out of scope for this submission.

## P — Plans / next steps

Agent-06 literature-mapper is the next agent. Handoff in `handoff/agent-05-to-06--plan-
ready.md`. Six-cluster topology is pre-sketched; seeded citations are named; defuses
FM-10 (citation padding).

## Self-reflection (agent-01 P01–P10 compliance)

- **P01 Measured-or-flagged.** The plan itself makes no numeric empirical claims about
  the manuscript; it only plans for them. ✓.
- **P02 Narrow novelty.** Plan enumerates the four core claims + the novelty-delta
  table §2 will carry (G3). ✓.
- **P03 Construct-before-invocation.** Section map cross-references definition files
  before any writer invokes a construct. ✓.
- **P04 Tier labelling.** Tier labels enforced per §6 subsection in section-ownership
  map. ✓.
- **P05 No-marketing.** Plan uses no forbidden terms. Regex-clean. ✓.
- **P06 No-AGI.** Plan explicitly names AGI as a FM-03 risk with R2 mitigation. ✓.
- **P07 Vocabulary lock.** Plan uses canonical glossary terms throughout. ✓.
- **P08 Hedge mechanics.** V11, V12 hedges cross-referenced. ✓.
- **P09 Disclosure confinement.** Plan locks Teleox.ai / Leapable.ai to
  Acknowledgements. ✓.
- **P10 Failure disclosure.** Forensic paragraphs required for Cases 1 & 2. ✓.

## Confidence

HIGH. Reason: (a) all four scoping layers (principles, glossary, essential questions,
construct definitions) are locked upstream and available as structured inputs;
(b) the mission brief specifies a constrained, well-bounded plan (one structure, three
cases, four claims); (c) every downstream agent has a row in the tasking table with
explicit reads / writes / gates; (d) every FM-01 … FM-10 failure mode is mapped to a
risk-register item with an owner and a fallback; (e) every G1–G9 final gate has an
intermediate checkpoint (QG1–QG8) that catches it earlier; (f) the plan makes no novel
empirical claims — it is structure-only — so no empirical risk flows through it.

---

## Change log

- 2026-04-16 — Initial SMAP report by agent-05-research-planner.
