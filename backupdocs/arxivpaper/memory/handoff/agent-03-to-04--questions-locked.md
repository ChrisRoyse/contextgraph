---
namespace: handoff
agent_id: agent-03-self-ask-decomposer
timestamp: 2026-04-16T15:45:00Z
status: active
tags: handoff, agent-04, construct-definer, essential-questions, locked
---

# Handoff: Agent-03 → Agent-04 (construct-definer)

## Read these files FIRST, in this order

1. `./memory/decisions/agent-03-self-ask-decomposer--essential-questions.md` —
   **LOCKED** 20 essential questions with per-question metadata (section, owner,
   evidence source, tier, answerability, dependencies).
2. `./memory/patterns/agent-03-self-ask-decomposer--question-to-section-map.md` —
   routing of each question to sections / agents.
3. `./memory/discoveries/agent-03-self-ask-decomposer--knowledge-gaps.md` — nine
   named gaps (G01–G09), four HIGH severity. Your construct definitions must be
   stateable without closing these gaps.
4. Prior binding docs (already loaded upstream): the agent-02 canonical glossary and
   vocabulary rules, the agent-01 principles and failure-modes files.

## What agent-03 did

Decomposed the manuscript into 20 essential questions across 8 categories:

- A (Q01–Q03): problem framing and motivation
- B (Q04–Q06): core construct definitions (DDA, meaning compression, TCT)
- C (Q07–Q09): TCT method mechanics
- D (Q10–Q11): systems (Context Graph, ClipCannon)
- E (Q12–Q14): case-study measurements and projections
- F (Q15): sample-complexity collapse
- G (Q16–Q17): threats to validity, forensic failures
- H (Q18–Q20): reproducibility, ethics, future work

Each question names its primary section, evidence source, owner agent, evidence tier
(T1–T4), and current answerability (ANSWERABLE / PARTIAL / GAP). No new vocabulary
introduced — canonical glossary used verbatim.

## What agent-04 (construct-definer) must do

Produce formal, pseudocode-and-equation-grade definitions for the four load-bearing
constructs so that agents 21, 25, 35–41 pull one canonical definition per construct.

The **binding questions** for agent-04 are:

- **Q04** — Derived Data Abundance (DDA). Formal derivation of $n(N + \binom{N}{2})$.
  Must include V11 hedge on the first use of "orthogonal".
- **Q05** — Meaning compression. Four-category taxonomy table with dimensionally-distinct
  unit columns; orthogonality to bit/weight/activation compression. Rule V5 forbids
  "semantic compression".
- **Q06** — Teleological Constellation Training (TCT). Training objective +
  per-output constellation-guard at inference. Must contrast explicitly with RLHF/DPO
  scalar-reward alignment (rule V10 disambiguation).
- **Q15** — Identity-locked generation. Formal predicate: output accepted iff
  $\min_m \cos(\phi_m(\hat{y}), c_m) \geq \tau_m$ for every modality in the subject's
  constellation (glossary #4).

**Constraints agent-04 must respect:**

- Use canonical glossary terms verbatim. No "prototype" (V1), no "anchor" (V2), no
  "identity-preserving" (V3), no "semantic compression" (V5), no bare "SECS" (V4).
- Every definition cites glossary entry number (e.g., "DDA (glossary #1)").
- Evidence-tier tags: DDA count is T4; meaning-compression taxonomy is T4; TCT
  specification is T4 for the protocol + T2 for the code artifact; identity-locked
  predicate is T4.
- Pseudocode only — no Python or Rust in the body (FM-06).

## Specific questions your output must answer in-place

Agent-04's deliverable should directly resolve Q04, Q05, Q06, Q15. Other agents (21,
25, 37, 38) handle the remaining Qs and will cite your file.

## Files agent-04 should write (SMAP)

- `./memory/decisions/agent-04-construct-definer--formal-definitions.md` (LOCKED)
- `./memory/patterns/agent-04-construct-definer--notation-and-equations.md`
- `./memory/handoff/agent-04-to-05--definitions-locked.md`
- `./memory/reports/agent-04-construct-definer--final-report.md`

Optional: `./memory/blockers/agent-04-{blocker}.md` if a canonical-glossary entry is
inadequate for formal definition; file a blocker, do not silently extend vocabulary.

## Pointers / cross-references

- When defining DDA, quote glossary #1 + R01 rationale verbatim; then extend with
  derivation (per-embedder count $N$ + pairwise count $\binom{N}{2}$).
- When defining meaning compression, build the 4-row taxonomy table; cite TurboQuant /
  GPTQ / SparseGPT via `_evidence_01_data_wall.md` for the adjacent categories.
- When defining TCT, structure as: (a) frozen embedders $\{\phi_m\}$; (b) constellation
  $C_P = \{(m, c_m)\}$ computed from reference sets; (c) training loss with geometric
  term $\sum_m \lambda_m (1 - \cos(\phi_m(\hat{y}), c_m))$; (d) runtime guard.
- When defining identity-locked generation, use the strict predicate, then describe
  the relaxed / soft variant (weighted-min) used by Case 3's best-of-$K$ selector.

## Success criterion for agent-04's handoff to agent-05

Agent-05 (research-planner) can read your definitions file and produce a research plan
whose every claim traces to a construct definition you provided. If agent-05 has to
re-invent a definition, your handoff failed.

## Escalation path

If a construct cannot be defined without violating a glossary rule, file a blocker at
`./memory/blockers/agent-04-construct--{term}.md` with HIGH severity and coordinator
attention.

## Confidence

HIGH. The 20 questions cover every load-bearing claim in the mission brief with no
redundancy; evidence sources are named for each; gaps are enumerated separately.
