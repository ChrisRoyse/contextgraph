---
namespace: reports
agent_id: agent-21-theory-builder
timestamp: 2026-04-17T05:15:00Z
status: active
tags: final-report, SMAP, theoretical-framework, spine, axioms, propositions, theorems
---

# Agent-21 Final Report — Theoretical Framework (SMAP)

## S — Situation

Agent-17 issued six higher-order themes (A–F) + narrative arc. Agent-19
issued a 10-RO ledger. Agent-20 issued an 8-ISO ledger (5 body-named +
3 metaphor-only). Agent-04 locked four construct definitions (DDA,
meaning compression, TCT, identity-locked generation). Mission brief
required a coherent theoretical spine that links these four constructs
into one testable theory.

Agent-21's task: construct the manuscript's theoretical framework —
the theoretical spine that §3 + §5 of the manuscript will teach.

Deliverables specified:

1. The central theoretical object (the spine).
2. Three axioms the theory rests on.
3. Five propositions derived from the axioms.
4. Two informal theorems.
5. Relation to existing frameworks.
6. Paste-ready §3-scaffold for the methodology-writer.
7. Handoff to agent-22 (model-architect).
8. Final report (SMAP).

Constraints: no over-formalisation (arXiv-submittable, not pure
theory); no new citations (LOCKED 88-pool); no new primitives
(composition-not-invention, MP-12).

## M — Mission

Produce a theoretical spine that:

- Names a single paper-level object from which every contribution is
  defined.
- States three minimally-sufficient axioms with explicit hedges.
- Derives five testable propositions with proof sketches.
- Promotes the two sharpest results to informal theorems (one to lead
  with, one to support the single-most-novel move).
- Positions the spine vs. Chinchilla, compression-as-intelligence,
  scalar-reward alignment, and five cross-field isomorphisms.
- Provides a paste-ready §3-scaffold with word-count targets, Figure 1
  specification, Algorithm 1 reference, and a writer checklist.
- Preserves all upstream hedges verbatim (V11, V18, V22, MP-12, MP-16,
  R1, R3, R4; bias mitigations from agent-14; quality flags from
  agent-13).

## A — Approach

### Step 1 — Identify the spine

Read the four agent-04 construct definitions. Checked each for its
relation to every other. Identified $C_P = \{(m, c_m) : m \in
\mathcal{M}\}$ as the single object that appears in all four:

- DDA produces the per-embedder projections that aggregate into $c_m$.
- Meaning compression measures the ratio of structured signals to raw
  data, which sets the information content available to $C_P$.
- TCT's Phase-1 output is $C_P$; Phase-2 loss targets $C_P$; Phase-3
  guard reads from $C_P$.
- Identity-locked generation is the predicate over $C_P$.

No other object has this four-way co-appearance. $C_P$ is the spine.

### Step 2 — Draft the axioms

Worked from the spine outward. Asked: "What must be true for $C_P$ to
be a coherent theoretical object?"

- For $C_P$ to be grounded: Axiom 1 (frozen embedders measure real
  inputs deterministically).
- For the $\binom{N}{2}$ pairwise term to carry information: Axiom 2
  (approximate orthogonality of panel, V11-hedged).
- For $C_P$ to be constructible at practical sample sizes: Axiom 3
  (reference-set learnability at $n_m \in [10, 10^2]$, Case-3
  T1-anchored).

Chose exactly three. A fourth axiom candidate (generator trainability
against geometric losses) was folded into Phase-2 protocol discipline
rather than promoted to axiom — it is a TCT construction property, not
a foundational assumption.

### Step 3 — Derive the propositions

Generated candidate propositions from the intersection of axioms and
locked constructs:

- P1 (signal count) = Axioms 1 + 2 → DDA counting identity.
- P2 (collapse silence) = Axiom 1 → Shumailov-2024 precondition
  violated.
- P3 (geometric verifiability) = Axioms 1, 2 + Phase 3 predicate →
  structural superiority over scalar reward.
- P4 (role composition) = Axiom 1 → same frozen $\Phi$ in three roles.
- P5 (sample-complexity collapse) = Axioms 1 + 3 → subject complexity
  $O(M \cdot k)$.

Verified each derives from axioms without circularity. Each carries
its own hedge (pool-relative scope on P3 / P4; T1-single-anchor hedge
on P5; structural-upper-bound hedge on P1; categorical-scope hedge on
P2).

### Step 4 — Promote to theorems

From the five propositions, identified the two sharpest, most
cash-out-able statements:

- **Theorem 1 (Sample-Complexity Collapse)** — constructive from the
  three-phase protocol under Axiom 3; leads §1 and §5 because it is
  the structural statement of Mission-Brief Claim #4.
- **Theorem 2 (Runtime Verifiability of Identity Lock)** — computable
  per-output in $O(M \cdot d_{\max})$ with no auxiliary learned model;
  MP-09's crisp form.

Stated both informally (prose + equation + proof sketch) to comply
with "not a pure theory paper" discipline.

### Step 5 — Position vs. existing frameworks

Three external programmes + cross-field isomorphisms:

- **EXT** Chinchilla scaling (axis extension: supervisory-signal
  density as a new axis).
- **EXT (CAT-EXT)** compression-as-intelligence (fourth taxonomic
  category).
- **BRK** scalar-reward alignment (four-axis structural break).
- **Imports** from ISO-01 (RV), ISO-02 (sensor fusion), ISO-04 (MDL),
  ISO-06 (astronomy), ISO-07 (SPC).

Inherited agent-10 novelty codes verbatim. Resolved agent-17 Tension 2
(Huang r≈0.95 within-model vs. V11 across-model) via MDL-ratio
carrying cleanly across scales.

### Step 6 — Paste-ready §3-scaffold

Produced 13 paragraph blocks for agent-37 (methodology-writer), with:

- §3.0 framing; §3.0.1 ISO cueing
- §3.1 four-category taxonomy
- §3.2 three axioms (one paragraph each)
- §3.3 scope argument (P2)
- §3.4 counting (P1)
- §3.6 Theorem 1
- §5.1 TCT framing (ISO-06 astronomical)
- §5.3 guard (ISO-01 + ISO-07 dual attribution) + Theorem 2
- §5.4 identity-locked definition
- §6.3 Case 3 framing
- §7 Discussion (composition-simplicity + relation-to-frameworks)
- §8 Limitations (axiom hedges)

Added: Figure 1 specification (ASCII diagram + paste-ready caption);
Algorithm 1 reference; 10-item writer checklist; word-count targets
totalling ~2,500–3,000 words.

### Step 7 — Handoff to agent-22

Identified 7 open items for agent-22 (Figure 1 construction;
architecture diagram consistency; Algorithm 1 formatting; notation
consistency check; Axiom 3 visual callout; per-output provenance
trace; paradigm-import dual attribution).

## M — Metrics

| Metric | Target | Agent-21 delivered |
|---|---|---|
| Axiom count | 3 | 3 (Grounded measurement, Approximate orthogonality, Reference-set learnability) |
| Proposition count | 5 | 5 (P1–P5) |
| Theorem count | 2 | 2 (Sample-Complexity Collapse; Runtime Verifiability) |
| Spine single-object | yes | yes — $C_P$ |
| Relation-to-existing-frameworks | 3+ programmes | 3 (Chinchilla EXT; compression-as-intelligence CAT-EXT; scalar-reward BRK) + 5 ISOs |
| Informal-not-pretentious | prose + equation | each theorem: statement + proof sketch + confidence + hedges |
| Composition-not-invention | MP-12 preserved | preserved in every first-use clause |
| Pool-relative scope | R3 preserved | all "no cited method" clauses bounded by LOCKED 88-pool |
| Hedges carried verbatim | V11 / V18 / V22 / MP-12 / MP-16 / R1–R4 / agent-14 biases | all carried |
| No new citations | 0 additions | 0 |
| No new primitives | 0 additions | 0 |
| §3-scaffold paragraph count | ≥ 10 blocks | 13 blocks |
| Figure 1 specified | yes | yes (ASCII + caption) |
| Writer checklist | ≥ 8 items | 11 items |
| Handoff open items | ≥ 5 | 7 items |
| Files written | 4 | 4 |

**All targets met.**

## A — Action items delivered

1. `./memory/decisions/agent-21-theory-builder--theoretical-framework.md`
   — the spine, three axioms, five propositions, two theorems,
   relation-to-existing-frameworks, hedges, cross-references.
2. `./memory/patterns/agent-21-theory-builder--theory-for-writers.md`
   — paste-ready §3-scaffold (13 blocks) + Figure 1 spec + Algorithm
   1 reference + 10-item writer checklist + word-count targets.
3. `./memory/handoff/agent-21-to-22--theory-ready.md` — handoff to
   model-architect with 7 open items.
4. `./memory/reports/agent-21-theory-builder--final-report.md` —
   this file (SMAP).

## P — Progress

**Gate status:**

- **QG-01 axiom count** 3/3. **PASS.**
- **QG-02 proposition count** 5/5. **PASS.**
- **QG-03 theorem count** 2/2. **PASS.**
- **QG-04 informal-not-pretentious.** Each theorem: prose + equation
  + proof sketch. **PASS.**
- **QG-05 spine single-object.** $C_P$ is the single paper-level
  object. **PASS.**
- **QG-06 relation-to-existing-frameworks.** 3 programmes + 5 ISOs.
  **PASS.**
- **QG-07 composition-not-invention.** No new primitives introduced.
  **PASS.**
- **QG-08 hedges carried verbatim.** V11 / V18 / V22 / MP-12 / MP-16
  / R1 / R3 / R4 all carried. **PASS.**
- **QG-09 no new citations.** LOCKED 88-pool preserved. **PASS.**
- **QG-10 novelty codes inherited.** Agent-10 EXT / CAT-EXT / BRK /
  ORTH inherited from RO-1 / RO-2 / RO-3 / RO-4. No upgrades. **PASS.**
- **QG-11 §3-scaffold paste-readiness.** 13 blocks, word-count
  targets, writer checklist. **PASS.**
- **QG-12 handoff completeness.** 7 open items + context file list +
  one-paragraph summary. **PASS.**

All 12 quality gates **PASS**.

## Risks / Open items

1. **Axiom 3's single T1 anchor.** Theorem 1 uses $k = 50$ from Case
   3 as the T1-anchored operating point for one modality. Cases 1
   and 2 are T3-projected. The sample-complexity curve (RO-10) is
   future work. Adversarial reviewers may push on "one modality, one
   subject, one operating point" — the §8 Limitations paragraph in
   the scaffold addresses this.
2. **V11 embedder-independence hedge.** Axiom 2's "approximate
   orthogonality" is empirically unmeasured; RO-8 is the future-work
   item that would replace the count upper-bound with an
   MI-adjusted ratio. Theorem 1's constructive proof does not depend
   on strict independence, but P1's count becomes an overestimate if
   the hedge is violated in practice.
3. **Pool-relative scope (MP-16).** All "no cited method" clauses in
   P3, P4, Theorem 2 are bounded by the LOCKED 88-pool. A
   pool-external counter-example would weaken composition-level
   novelty claims but would not invalidate Theorems 1 or 2
   constructively.
4. **Figure 1 rendering.** Agent-21 produced an ASCII diagram +
   paste-ready caption. Agent-22 must convert to LaTeX TikZ or
   equivalent for arXiv-publication quality. This is the single
   visual that teaches the spine; getting it right is load-bearing.

## Confidence

**HIGH** on all four deliverables.

- Axioms, propositions, theorems are derivations from locked upstream
  constructs; no new atoms introduced.
- Theorems are constructive from the three-phase protocol under the
  stated axioms; proof sketches are closed.
- Relation-to-existing-frameworks preserves agent-10 novelty codes
  verbatim.
- §3-scaffold is paste-ready for agent-37 with word-count targets and
  an 11-item writer checklist.
- Handoff to agent-22 is closed-form with 7 actionable open items.
- No new citations; no new primitives; no upstream hedge dropped.

## Changelog

- 2026-04-17 — Initial issue by agent-21-theory-builder.
