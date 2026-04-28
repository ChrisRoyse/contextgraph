---
namespace: patterns
agent_id: agent-03-self-ask-decomposer
timestamp: 2026-04-16T15:45:00Z
status: active
depends_on:
  - memory/decisions/agent-03-self-ask-decomposer--essential-questions.md
tags: section-ownership, routing, arxiv, canonical-layout
---

# Question-to-Section Ownership Map

This is the routing table. For every one of Q01–Q20 (locked in the decisions file), it
states (a) the primary section that owns the answer, (b) any secondary section that must
re-reference the answer, (c) the primary owning agent, (d) secondary agents that read
from the answer, and (e) the glossary terms the section must consume.

Section numbering follows the arXiv-canonical layout locked in P05:

- §0 Abstract
- §1 Introduction
- §2 Related Work
- §3 Framework (DDA + meaning compression)
- §4 Systems (Context Graph + ClipCannon)
- §5 Method (TCT)
- §6 Case Studies (Results)
- §7 Discussion
- §8 Limitations
- §9 Conclusion
- (Appendix) Reproducibility, pseudocode

---

## Ownership table

| Q     | Primary §  | Secondary §     | Primary agent (writer / analyst) | Secondary readers                 | Glossary terms consumed                          |
|-------|------------|------------------|----------------------------------|-----------------------------------|--------------------------------------------------|
| Q01   | §1         | §3.3             | agent-35 introduction-writer     | agent-36, agent-42                | #26 data wall, #27 third path                    |
| Q02   | §2.1 / §3.3| §8               | agent-36 lit-review-writer       | agent-21, agent-42                | #1 DDA, #28 model collapse                       |
| Q03   | §2         | §1               | agent-36                         | agent-35, agent-06                | #1 DDA, #26 data wall                            |
| Q04   | §3.1 / §3.4| §4, §5           | agent-04 construct-definer       | agent-21, agent-37, agent-38      | #1 DDA, #14 orthogonal (V11 hedge), #5 embedder  |
| Q05   | §3.1 / §3.5| §2 (TurboQuant)  | agent-04                         | agent-36, agent-21                | #2 meaning compression                           |
| Q06   | §5         | §2.6             | agent-04, agent-37               | agent-25, agent-42                | #3 TCT, #12 constellation guard, #13 alignment   |
| Q07   | §5.1–§5.3  | §6.3 linkage     | agent-25 method-designer         | agent-37, agent-38, agent-43      | #3 TCT, #7 centroid, #8 constellation, #9 refset |
| Q08   | §5.3       | §6.3, §8         | agent-25                         | agent-29, agent-38                | #11 per-modality threshold, #17 same-session band|
| Q09   | §2         | §5, §6.3         | agent-36                         | agent-10, agent-38                | #3 TCT, #16 SECS, #13 alignment                  |
| Q10   | §4.1       | §3.4 (N=13 link) | agent-09 methodology-scanner     | agent-37, agent-38                | #5 embedder, #22 Context Graph                   |
| Q11   | §4.2       | §6.2 linkage     | agent-09                         | agent-37, agent-38                | #5 embedder, #6 modality, #23 ClipCannon         |
| Q12   | §6.3 (T1)  | §1 abstract      | agent-38 results-writer          | agent-34, agent-43                | #16 WavLM SECS, #18 DNSMOS, #19 UTMOS, #20 cross-encoder, #21 T1 |
| Q13   | §6.1, §6.2 | §8 (pending)     | agent-38                         | agent-34, agent-43                | #21 T2/T3 tiers, #29 forensic report             |
| Q14   | §6.3       | §2 (VALL-E 2)    | agent-38                         | agent-36, agent-29                | #16 SECS, #20 cross-encoder                      |
| Q15   | §3.6 / §5.4| §6.1–§6.3        | agent-21 theory-builder          | agent-38, agent-34                | #4 identity-locked, #9 reference set             |
| Q16   | §8         | §3.4 (V11)       | agent-29 validity-guardian       | agent-30, agent-42                | #14 orthogonal (hedge), #11 threshold, #21 tiers |
| Q17   | §6 per-case + §8.4 | §7 discussion | agent-38, agent-39               | agent-42                          | #29 forensic report, #30 flow-matching sign      |
| Q18   | §8.5 + Appendix | §5, §6        | agent-43 reproducibility-checker | agent-37, agent-25                | #10 frozen, #5 embedder, #21 tiers               |
| Q19   | §8.6       | §9               | agent-31 ethics-reviewer         | agent-39                          | #4 identity-locked                               |
| Q20   | §9 / §8.7  | §8               | agent-40 conclusion-writer       | agent-18, agent-19, agent-42      | #14 orthogonal (hedge)                           |

---

## Section-indexed view (how many questions own each section)

| §        | Questions owned        | Notes                                                    |
|----------|------------------------|----------------------------------------------------------|
| §1       | Q01 (primary)          | Re-cites: Q02, Q03, Q04, Q06, Q12 (abstract paragraph)   |
| §2       | Q02, Q03, Q09          | Related-work delta paragraphs per P07                    |
| §3       | Q04, Q05               | §3.3 re-cites Q02; §3.4 owns DDA counting derivation     |
| §4       | Q10, Q11               | T2 constructive evidence (line counts, CFs, tests)       |
| §5       | Q06, Q07, Q08          | Method spec + runtime guard; pseudocode only (FM-06)     |
| §6       | Q12, Q13, Q14, Q15, Q17| Header tags: §6.1 T2+T3, §6.2 T2+T3, §6.3 T1             |
| §7       | (shared) Q17 discussion| Forensics elevated to discussion; no new numbers         |
| §8       | Q16, Q18, Q19, Q20     | Limits + reproducibility + ethics + future work          |
| §9       | Q20                    | Conclusion echoes §8.7                                   |
| Appendix | Q18                    | Hyperparameters, pseudocode, command lines               |

---

## Routing rules for downstream agents

- **agent-04 (construct-definer)** pulls Q04, Q05, Q06 into a single locked definitions file.
  It is the authority that agents 35–41 quote from.
- **agent-21 (theory-builder)** pulls Q04, Q05, Q06, Q15 and produces the §3 theoretical
  framework scaffold.
- **agent-25 (method-designer)** pulls Q06, Q07, Q08 into §5 method spec + pseudocode.
- **agent-09 (methodology-scanner)** pulls Q10, Q11 into §4 systems descriptions.
- **agent-36 (lit-review-writer)** pulls Q02, Q03, Q09 and is responsible for §2's per-line
  deltas (P07: novelty narrow, not sweeping).
- **agent-38 (results-writer)** pulls Q12, Q13, Q14, Q15, Q17 and is the **only** writer
  that produces numbers; everyone else cites §6.
- **agent-29 (validity-guardian)** owns Q16; every other §8 Limitations contributor defers.
- **agent-31 (ethics-reviewer)** owns Q19.
- **agent-43 (reproducibility-checker)** owns Q18; produces per-case replicator checklists.

---

## Forbidden handoff patterns

- No question may be answered in §0 Abstract before its primary section has fixed the
  answer. The abstract agent (41) is last in the writing cohort.
- No writer may introduce numbers that do not appear in §6; per-case headline numbers
  come from Q12 (T1) or are T3-flagged via Q13.
- §2 may reference definitions but may NOT redefine DDA / meaning compression / TCT —
  those definitions live in §3/§5 and are owned by Q04/Q05/Q06 respectively.
- §4 systems descriptions must NOT claim performance; T2 constructive only.

---

## Change log

- 2026-04-16 — Initial lock by agent-03-self-ask-decomposer.
