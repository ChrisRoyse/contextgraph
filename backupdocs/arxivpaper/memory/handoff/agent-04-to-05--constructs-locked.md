---
namespace: handoff
agent_id: agent-04-construct-definer
timestamp: 2026-04-16T16:00:00Z
status: active
tags: handoff, agent-05, research-planner, constructs-locked
---

# Handoff: Agent-04 (construct-definer) → Agent-05 (research-planner)

## Read these files FIRST, in this order

1. `./memory/decisions/agent-04-construct-definer--dda-definition.md` — **LOCKED** Tier-A
   #1 construct (Q04). Formal derivation of $n(N + \binom{N}{2})$ signals. Independence
   assumptions, boundary conditions, NOT-clause expanded.
2. `./memory/decisions/agent-04-construct-definer--meaning-compression-definition.md` —
   **LOCKED** Tier-A #2 construct (Q05). Four-category taxonomy, compression-ratio
   indicator $\mathrm{MC}(\Phi, D) = N + \binom{N}{2}$, orthogonality to bit / weight /
   activation compression.
3. `./memory/decisions/agent-04-construct-definer--tct-definition.md` — **LOCKED** Tier-A
   #3 construct (Q06, Q07). Three-phase protocol, training objective, acceptance
   predicate, NOT-clause against RLHF / DPO / CAI / contrastive / prototypical / vanilla
   LoRA.
4. `./memory/decisions/agent-04-construct-definer--identity-locked-definition.md` —
   **LOCKED** Tier-A #4 construct (Q15). Strict acceptance predicate; soft best-of-$K$
   variant (Case 3); per-case evidence tiers.
5. `./memory/decisions/agent-04-construct-definer--support-constructs.md` — **LOCKED**
   three support constructs: constellation, frozen embedder / measurement instrument,
   same-session verification band (our empirical observation label; NOT a benchmark).
6. `./memory/patterns/agent-04-construct-definer--definition-blocks-for-writers.md` —
   single-page paste-ready definition blocks for every downstream section writer.

## What agent-04 did

Operationalised the four load-bearing constructs (DDA, meaning compression, TCT,
identity-locked generation) plus three support constructs (constellation, frozen embedder,
same-session verification band). Each construct has: (a) a conceptual definition for a
first-pass cs.LG reviewer, (b) an operational / formal definition with equations, (c) a
variable-and-indicator table, (d) a "what it is NOT" clause that enumerates the nearest
confusable methods, (e) a usage rule tied to the canonical-glossary vocabulary rules, and
(f) cross-references to the essential questions (Q04, Q05, Q06, Q07, Q15) and the
glossary entries.

All vocabulary is in lock-step with
`./memory/decisions/agent-02-ambiguity-clarifier--canonical-glossary.md`. No new terms
introduced. Every hedge required by V3 / V5 / V6 / V10 / V11 / V12 / V18 / V19 / V22 is
embedded in the relevant definition.

Evidence tiers tagged on every formal claim:
- DDA count $N + \binom{N}{2}$ — T4 theoretical; $N=13 \Rightarrow 91$ signals — T2
  constructive.
- Meaning-compression taxonomy — T4.
- TCT specification — T4 for the protocol; T2 for the constellation-guard code artifact.
- Identity-locked predicate — T4 definition; Case 3 numerics T1; Cases 1–2 T3.
- Same-session verification band — T1 (measured on Case 3's speaker, labelled as our
  empirical observation).

## What agent-05 (research-planner) must do

Build a **research plan** in which every claim, experiment, ablation, and section of the
arXiv manuscript traces to one of the four Tier-A constructs (DDA, meaning compression,
TCT, identity-locked generation) and the 20 essential questions (Q01–Q20) decomposed by
agent-03.

Specifically:

1. Produce a section-level plan showing which construct(s) each section of the paper
   (§1–§9) invokes. Anchor each invocation to the definition file.
2. Produce a case-study plan showing which constructs Cases 1, 2, 3 exercise and what
   evidence tier each case study's claims sit in.
3. Produce an ablation / robustness plan naming (a) which embedders matter for DDA
   (sensitivity to panel size $N$), (b) which modalities matter for TCT (sensitivity to
   $\lambda_m$ and $\tau_m$), (c) what would invalidate identity-locked generation
   (sensitivity to $K$, threshold calibration).
4. Identify any planning gap that needs a new essential question — file a blocker at
   `./memory/blockers/` rather than silently inventing one (per constraints).
5. Hand off to agent-06 (or whichever agent is next in the pipeline) with file-level
   pointers.

## Binding constraints on agent-05

- The four Tier-A definitions are **locked**. Agent-05 does not re-derive them; agent-05
  **cites** them.
- The glossary terminology is locked. Agent-05 uses "centroid", "constellation",
  "identity-locked generation", "meaning compression", "frozen embedder" verbatim.
- Every plan row has an evidence tier (T1 / T2 / T3 / T4) per agent-01 P04 and glossary #21.
- No Python / Rust code in the plan body — pseudocode or equations only (FM-06).
- No marketing language, no AGI claim (P09).

## Specific pointers for agent-05

- When planning the **DDA section (§3)**, cite `dda-definition.md` §1.2 for the count and
  §1.2.5 for the boundary conditions; plan an explicit paragraph addressing the
  "approximately independent" hedge (V11) and future-work item for formal independence
  measurement (Q20 (i)).
- When planning the **meaning-compression section (§3.1 / §3.5)**, build the four-category
  taxonomy table from `meaning-compression-definition.md` §2.3. Cite `_evidence_01_data_wall.md`
  for GPTQ / SparseGPT / TurboQuant references.
- When planning the **TCT method section (§5)**, use the three-phase structure from
  `tct-definition.md` §3.2. Include the pseudocode block from §3.2.1. Address Q06 (contrast
  with RLHF / DPO / CAI) in §2.6 Related Work.
- When planning the **identity-locked results (§6.3 Case 3)**, tag the header "(T1)" per
  V20. Compute indicators from `identity-locked-definition.md` §4.3 (mean / max / accept
  rate).
- When planning **Cases 1–2 (§6.1 / §6.2)**, tag headers "(T2+T3)" per V20 and flag every
  projected metric with "(projected; camera-ready will re-verify)".
- Reference the support-constructs file when writing any paragraph that uses
  "constellation", "frozen embedder", or "same-session band" without already defining them.

## Escalation path

If agent-05 finds a construct that cannot be planned without contradicting a locked
definition, file a blocker at
`./memory/blockers/agent-05-plan-conflict-with-{construct}.md` with HIGH severity and
coordinator attention. Do not silently override a locked decision.

## Confidence

HIGH. The four Tier-A constructs and three support constructs are fully operationalised
with equations, indicators, and NOT-clauses. Every downstream agent has a paste-ready
paragraph plus the full formal file behind it. No known conflict with the canonical
glossary, the essential questions, or the guiding principles.

---

## Change log

- 2026-04-16 — Initial handoff by agent-04-construct-definer.
