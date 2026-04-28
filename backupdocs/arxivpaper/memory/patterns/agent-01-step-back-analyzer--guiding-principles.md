---
namespace: patterns
agent_id: agent-01-step-back-analyzer
timestamp: 2026-04-16T15:15:00Z
status: active
tags: principles, arxiv, first-principles, tct, dda, meaning-compression, cs-lg
---

# Guiding Principles — arXiv Manuscript on TCT / DDA / Meaning Compression

These are the first-principles that govern every downstream agent (02–48). They are derived
from the mission brief and constraints and are NON-NEGOTIABLE unless superseded by a
`./memory/decisions/` file written by the coordinator.

Each principle is stated, justified, and tied to a concrete pass/fail test downstream agents
can apply. Principles 01–03 are epistemic (how we reason). Principles 04–06 are structural
(how we write). Principles 07–09 are defensive (how we survive adversarial review).

---

## P01 — Measured-or-Flagged: every number has a provenance tag

**Statement.** Every numeric claim in the manuscript is either (a) **measured** and cited to a
specific forensic report, SITREP, or log file with file path, or (b) explicitly labeled as
**projected / architecturally-complete / measurement-pending** with the exact condition under
which it will be re-verified for camera-ready.

**Why.** arXiv cs.LG reviewers are trained to smell unsubstantiated numbers. Mixing measured
and projected values without labels is the single fastest rejection path. Case 3 (voice cloning,
SECS 0.961) is the only fully-measured study at submission time; Cases 1 and 2 are architecturally
complete but have pending runs. This asymmetry must be explicit, not hidden.

**Pass test.** Every numeric in the manuscript body has either a footnote pointing to a
specific source document OR a parenthetical "(projected; camera-ready will re-verify)" marker.
No bare numbers.

**Fail example.** "Our SECS is 0.96." Missing provenance; fails.

**Pass example.** "Case 3 reports mean SECS = 0.961 (max 0.975) over 10 unseen sentences
(SITREP 2026-04-16, §Case 3)."

---

## P02 — Skeptic-default audience: write for the reviewer who wants to reject

**Statement.** The target reader is a skeptical cs.LG reviewer whose prior belief is that this
paper is low-quality self-promotion until proven otherwise. Every paragraph must either
strengthen that reviewer's confidence or explicitly acknowledge a limitation.

**Why.** arXiv is cross-listed to cs.CL and cs.CV — reviewers span three subfields. They see
hundreds of papers a year with similar "novel method" claims. Their default action is to skim
for overclaiming, then reject. We write for that reader, not for a sympathetic one.

**Pass test.** Every section answers "what objection does a hostile reviewer raise here, and
is it addressed in-section or deferred to §8 Limitations with a pointer?"

**Fail example.** "Our method achieves state-of-the-art" — with no comparator, no benchmark,
no dataset split.

**Pass example.** "On the specific Case-3 protocol (10 sentences, Qwen3-TTS + Full ICL +
50-clip centroid + WavLM-scored BO12 at T=0.3), mean SECS 0.961 exceeds the 0.95 WavLM-SECS
threshold commonly cited as 'identity-preserving' (cite). We do not claim generality beyond
this protocol; see §8."

---

## P03 — Claim-bearing vocabulary is defined before it is used

**Statement.** The four load-bearing constructs — **Derived Data Abundance (DDA)**, **Meaning
Compression**, **Teleological Constellation Training (TCT)**, **identity-locked generation**
— receive precise, citation-anchored definitions in §3 before any case study, theorem, or
figure invokes them. Every other technical term used in a claim (e.g., "orthogonal signals",
"constellation centroid", "sample-complexity collapse") is either a standard term in the
cited literature or defined in a glossary / §3 sub-section.

**Why.** DDA and meaning compression are *new* framings. If they slide between meanings across
sections, the reviewer has a legitimate grounds to reject for conceptual incoherence. Agent-04
(construct-definer) is the enforcer; every writer (agents 35–41) must pull definitions from
its output.

**Pass test.** Grep the final manuscript for "DDA", "meaning compression", "TCT", "teleological",
"constellation", "identity-locked". The first use of each is tied to a definition paragraph
or a cite to one. No use precedes its definition.

**Fail example.** Using "meaning compression" in the abstract without having distinguished it
from weight-compression and activation-compression.

---

## P04 — Empirical claims map to exactly one of four evidence tiers

**Statement.** Every claim in the manuscript maps to one of the following tiers, and the tier
is visible (either stylistic or via a footnote) to the reader:

  - **T1 — Measured:** backed by a numeric result in a specific report (e.g., Case 3 SECS
    0.961). Tier-1 claims can be stated without hedging.
  - **T2 — Constructive:** the claim is "the system exists and runs" — backed by file paths,
    line counts, test counts (e.g., Context Graph's 5,184 tests, ClipCannon's 23-stage DAG).
    These support existence claims, not performance claims.
  - **T3 — Projected:** the run is in progress or architecturally complete but not yet
    measured (e.g., Case 1 Shakespeare SFT+DPO). These MUST be flagged.
  - **T4 — Theoretical:** derivations, counts, and frameworks (e.g., "N embedders yield ~N +
    C(N,2) signals"). These are proved or cited, not measured.

**Why.** Mixing tiers without marking is the second-fastest rejection vector. Separating them
makes the paper's evidential structure legible.

**Pass test.** Downstream agents can annotate every claim sentence with a T1–T4 label without
ambiguity.

---

## P05 — Structure is arXiv-canonical, not bespoke

**Statement.** Section layout follows the cs.LG default: Abstract, 1. Introduction,
2. Related Work, 3. Framework, 4. Systems, 5. Method (TCT), 6. Case Studies (Results),
7. Discussion, 8. Limitations, 9. Conclusion, Acknowledgements, References (optional:
Appendix for pseudocode). Deviating risks marking the paper as "not a standard research paper"
— a known rejection heuristic.

**Why.** Novel structures are acceptable at venues the author is known at; on an arXiv cold
submission they burn goodwill before §1 ends.

**Pass test.** The table of contents matches the section list in the mission brief exactly.

---

## P06 — Length discipline: ~8K–12K words of prose, 30–60 references, ≤30 typeset pages

**Statement.** The manuscript is **short on purpose**. Prose padding, repeated explanations,
and long background discourse are *removed* during writing, not after. Every agent that writes
a section is told its target length and must respect it.

**Why.** The prior manuscript in `docs/researchpaper/` is ~2,306 lines across 16 files. This
arXiv version is derived from it, not a copy. Reviewer fatigue is real; shorter, denser
papers with verified numbers beat long manuscripts with diluted claims.

**Pass test.** §46 (file-length-manager) confirms the assembled `PAPER.md` is within budget
before §48 (adaptive-orchestrator) signs off.

---

## P07 — Related-work honesty: acknowledge precedent, distinguish novelty narrowly

**Statement.** §2 Related Work names the closest adjacent lines (scaling-law / data-wall;
synthetic-data and model collapse; multi-modal retrieval & CLIP-style representation
learning; TurboQuant and weight/activation compression; LoRA and identity-preserving
generation; voice-cloning SOTA). For each line, we state what it does, then state in one
sentence what DDA / TCT / meaning-compression add that the line does not already address.
Novelty claims are narrow and defensible, not sweeping.

**Why.** The single fastest way to lose reviewer trust is to claim novelty that overlaps
prior work. The safer posture is: "We add X to a landscape that contains Y and Z; Y and Z
do not do X because of the following structural reason."

**Pass test.** For each load-bearing novelty claim, agent-36 can point to the prior-work
paragraph in §2 that names the closest precedent and the specific delta.

---

## P08 — Reproducibility artifacts are first-class, not afterthoughts

**Statement.** The manuscript contains (either in-body or in an appendix) the following for
each case study: (a) hyperparameters, (b) data splits / source provenance, (c) evaluation
metrics and their exact computation, (d) the specific command / script that produced the
reported numbers or would produce them for a reader with the same source materials. Where
source data is proprietary or identity-sensitive, we say so explicitly and describe the
substitutable public approximation.

**Why.** Reproducibility is the single strongest positive signal for a cold arXiv submission.
It converts "trust me" into "verify yourself".

**Pass test.** Agent-43 (reproducibility-checker) can produce, for each case study, a
numbered checklist of what a replicator would need, and the manuscript supplies each item
(or flags it as proprietary with rationale).

---

## P09 — No marketing, no AGI, no superlatives

**Statement.** The body contains zero instances of: "revolutionary", "breakthrough",
"game-changing", "best-in-class", "unprecedented", "the first to", "solves AGI",
"world-class", "industry-leading". Teleox.ai and Leapable.ai appear ONLY in Acknowledgements
and a commercial-disclosure footnote. The data-wall is "reframed", not "solved". Identity-locked
generation is "demonstrated on three modalities", not "made easy".

**Why.** arXiv reviewers are trained to reject commercial pitches in research guise. A single
superlative in the abstract is sometimes sufficient grounds for a desk reject. This is the
rule the author must trust their agents to enforce, because the temptation from years of
marketing copy is high.

**Pass test.** Agent-42 (adversarial-reviewer) runs a regex over the assembled `PAPER.md` for
the forbidden terms. Zero matches is the pass condition.

---

## P10 — Failure documentation is evidence, not embarrassment

**Statement.** The Shakespeare training-data echo-bug and the Santa flow-matching sign-bug
are **assets**, not liabilities. They demonstrate forensic depth and intellectual honesty.
The manuscript includes (in §6 or §8) a compact description of each, what was wrong, how it
was diagnosed, and what the correction yielded. Hiding failures would be both dishonest and
strategically dumb — reviewers reward papers that show their work.

**Why.** The author's forensic reports (`docs/inspect/08_training_audit.md` in Santa; the
Shakespeare FSV logs) are rare and compelling. They differentiate this submission from
"method X works, here are numbers" papers.

**Pass test.** Both bugs are discussed with enough detail that a replicator understands the
failure mode and the fix. Both are cited to their forensic documents.

---

## Relationship to downstream agents

- **P01, P04** → bind every writer (35–41) and the confidence-quantifier (34).
- **P02, P07, P09** → bind the adversarial-reviewer (42) and the abstract-writer (41).
- **P03** → binds agent-04 (construct-definer) most of all; writers pull from its output.
- **P05, P06** → bind agents 35–41 on section structure; 46 on length.
- **P08** → binds agent-43 (reproducibility-checker); method-designer (25); instrument-developer (27).
- **P10** → binds results-writer (38) and discussion-writer (39).

Downstream agents that violate these principles should file a `./memory/decisions/` override
with rationale, not silently ignore.
