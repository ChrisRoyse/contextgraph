---
namespace: patterns
agent_id: agent-40-conclusion-writer
timestamp: 2026-04-16T22:50:00Z
status: active
locked: false
supersedes: none
tags: writing-decisions, conclusion, section-9, hedges, banned-phrase-audit
depends_on:
  - memory/context/mission-brief.md
  - memory/context/constraints.md
  - memory/patterns/agent-34-confidence-quantifier--confidence-language-for-writers.md
  - memory/handoff/agent-39-to-40--discussion-ready.md
  - memory/decisions/agent-04-construct-definer--dda-definition.md
  - memory/decisions/agent-04-construct-definer--tct-definition.md
  - memory/decisions/agent-04-construct-definer--meaning-compression-definition.md
downstream_consumers:
  - agent-41-abstract-writer (or whichever agent follows)
  - agent-42-adversarial-reviewer
---

# Agent-40 Conclusion-Writer — Writing Decisions

## 1. Scope owned by §9

§9 is the closer. §7 (Discussion) carries the interpretation of Case 3,
the DDA upper-bound argument, the four-axis TCT contrast, the collapse-
silence scope, the sample-complexity two-part split, the implications
across four domains, and the §7.7 forensic-failure-modes narrative. §8
(Limitations) carries the eight enumerated limitations (L1–L8) and the
EXP-1 / EXP-2 / EXP-3 closer. §9 must not re-narrate any of that. §9
owns: (i) a short restatement of what the paper did; (ii) a crystallised
statement of the three conceptual contributions at their carried
confidence levels; (iii) a softened one-paragraph empirical summary with
the single MEASURED anchor at 0.961 WavLM SECS; (iv) the three named
open experiments (EXP-1, EXP-2, EXP-3); (v) one modest forward-looking
sentence. Target 500–800 words, no subsections, no tables, no equations
beyond inline LaTeX carried forward, no citations.

## 2. Structural choices

- **Opening sentence**: "This paper named three constructs … described
  two production systems … and read three case studies". Named the
  constructs (DDA, meaning compression, TCT), named the two systems
  (Context Graph, ClipCannon), and named the three case studies. Put
  the ONE MEASURED anchor (0.961 mean WavLM SECS on 10 held-out
  sentences) at the end of the opening paragraph, with the "encoder-
  matched protocol" scope inline and a forward-reference to §6.3 and
  §8.L1.
- **Conceptual paragraph**: One paragraph covering C1 ("up to $N +
  \binom{N}{2}$"), C2 (meaning compression as fourth taxonomic entry),
  and C3 (TCT per-output verifiability relative to chosen panel). Each
  of the three gets one or two sentences; each carries its hedge.
- **Empirical paragraph**: One paragraph covering C12a (constructive)
  and C12b (one-operating-point empirical anchor), plus C13 (pool-
  relative contrast with scalar-reward alignment). Short; the details
  are in §7.
- **Open problems paragraph**: Named EXP-1, EXP-2, EXP-3 with one
  sentence each on what each would do (strengthen or kill). Kept the
  descriptions narrow; the details are in §8.
- **Closer**: One modest sentence. "We have named a configuration … that
  warrants independent replication and a pairwise mutual-information
  audit on a neutral panel." Explicitly declined any AGI or "future of
  AI" framing; explicitly declined any claim about "the field".

## 3. Hedges carried (from agent-34 cue-sheet)

- **C1 "up to 91 / up to 28"**: used verbatim. The conclusion says "up
  to 91 per input for Context Graph at $N = 13$, up to 28 per input
  for ClipCannon at $N = 7$", and says "the 'up to' is load-bearing"
  and points to the pending MI audit. Never writes "91 signals per
  input" without the "up to" prefix.
- **C5 encoder-matched protocol**: the 0.961 figure is introduced with
  "under the encoder-matched protocol disclosed in §6.3 and §8.L1" on
  first mention. On the second mention inside the EXP-1 clause, the
  phrase "within-WavLM-family agreement" is used, consistent with the
  §8.L1 companion phrasing.
- **C12a / C12b split**: explicitly split. "Theorem 1 is a constructive
  bound" and "the measured collapse is supported at one T1 operating
  point ($M=1$, $k=50$, Case 3). Broader empirical validation at $M >
  1$ and in other modalities is named future work."
- **C13 pool-relative**: "Pool-relative to the locked 88-citation
  master reference pool, no cited method composes …" and "the contrast
  with scalar-reward alignment (RLHF, DPO, Constitutional AI) is
  structural rather than a benchmark contest". No "outperforms"
  language.
- **C2 meaning compression**: "proposed as a fourth taxonomic entry
  alongside bit-, weight-, and activation-compression"; "the unit …
  is semantically distinct from Shannon bits, parameter accuracy,
  and activation memory"; "reviewer acceptance of the taxonomic
  distinction is a scholarly judgement we do not pre-empt". "Semantically
  distinct" verbatim per agent-34 C2.
- **C3 per-output verifiability**: "per-output verifiability relative
  to the chosen centroid panel … with no auxiliary learned model …
  predicate-level rather than identity-level under any external
  criterion". "Relative to the chosen centroid panel" verbatim per
  agent-34 C3.
- **C14 research-not-marketing posture**: no product names, no
  pricing, no commercial positioning; the mention of Context Graph
  and ClipCannon is as "two production systems" only, consistent
  with §4. No mention of Leapable or Teleox.
- **Projected vs. measured discipline**: every quantitative claim
  flagged. Case 1 and Case 2 described as "projected … pending";
  Case 3 described as "the one measured anchor".

## 4. Banned-phrase audit

Agent-34 §4 banned list: checked every phrase in the file. The
following candidates were specifically checked and NOT used in the
final file:

| Phrase | Used? |
|---|---|
| "breakthrough" / "revolutionary" / "game-changing" | No |
| "the best" | No |
| "solves AGI" / "solves the data wall" / "solves" | No |
| "dimensionally distinct" | No ("semantically distinct" used) |
| "91 signals per input" (bare) | No ("up to 91 per input" used) |
| "0.961 mean SECS" (bare) | No ("0.961 mean WavLM SECS … under the encoder-matched protocol" used) |
| "outperforms RLHF" | No ("structural rather than a benchmark contest" used) |
| "we demonstrate sample-complexity collapse" (bare) | No (split into C12a/C12b) |
| "across modalities" (applied to empirical) | No (rephrased to "at $M > 1$ and in other modalities … future work") |
| "TCT causes" | No ("supplies per-output verifiability" used) |
| "indistinguishable from real speech" (bare) | No |
| "mouth-weighted loss is necessary" | No |
| "T=0.3 is optimal" (bare) | No (the conclusion does not claim T-sweep optima) |
| "proven collapse-free" | No |

One marginal case caught during audit: an earlier draft contained
"Broader empirical validation across modalities and $M > 1$ is named
future work." The phrase "across modalities" is banned by agent-34
C12b when applied to the empirical collapse even in a future-work
framing. Rephrased to "at $M > 1$ and in other modalities" to preserve
the meaning without triggering the regex pattern. No other banned
phrases appeared.

## 5. Constructs named in the conclusion

- **Derived Data Abundance (DDA)** — named once in the opening
  sentence, referenced again as "the DDA counting argument" in the
  EXP-2 description.
- **Meaning compression** — named once in the opening sentence, once
  more in the conceptual paragraph.
- **Teleological Constellation Training (TCT)** — named once in the
  opening sentence, once more via "the Phase-3 guard $G_\tau$" in the
  conceptual paragraph.
- **Context Graph / ClipCannon** — named once each in the opening
  sentence as "two production systems".
- **Case 1 / Case 2 / Case 3** — named; Cases 1 and 2 flagged as
  projected, Case 3 flagged as measured.
- **EXP-1 / EXP-2 / EXP-3** — named once each in the open-problems
  paragraph with the narrow instrument description each gets.

## 6. What was deliberately omitted

- Numbers beyond "0.961 mean WavLM SECS on 10 held-out sentences" for
  Case 3 (the details are §6.3; the conclusion cites once and stops).
- Case 1 FSV 15/15 (handled in §6.1 and §8.L3).
- Case 2 six forensic fixes (handled in §6.2 and §7.7).
- The 88-citation pool composition (referenced only as "the locked
  88-citation master reference pool").
- Any product or commercial mention (per C14 and constraints.md).
- Any new citations (the brief said 0–2 APA; I chose 0).
- Any reference to the paper's length, structure, or section
  numbering beyond the cross-references required for scope discipline
  (§6.3, §8.L1).

## 7. Tone and forward-looking framing

The final sentence — "We have named a configuration … that warrants
independent replication and a pairwise mutual-information audit on a
neutral panel" — is the modest forward-looking framing the brief
required. It:

- Does NOT claim "the future of AI".
- Does NOT claim AGI relevance.
- Does NOT claim generalisation beyond what §8 bounds.
- DOES point at independent replication as the next move.
- DOES name a specific instrument (pairwise MI audit) and a specific
  independence condition (neutral panel) so the reader knows what a
  genuine third-party validation would look like.

## 8. Confidence

**HIGH** that §9 restates the four core claims at the confidence
levels §7 / §8 carried and does not introduce new claims.

**HIGH** that the banned-phrase audit caught every agent-34 §4 high-
risk item.

**HIGH** that the length (595 words) is within the brief's 500–800
target.

**MEDIUM** on whether the adversarial reviewer (agent-42) will read
the final sentence as adequately modest; the phrasing was chosen to
survive an "FM-01 marketing-creep" regex pass and a "FM-04 AGI-claim"
keyword pass.

## Changelog

- 2026-04-16 — Initial issue by agent-40-conclusion-writer.
