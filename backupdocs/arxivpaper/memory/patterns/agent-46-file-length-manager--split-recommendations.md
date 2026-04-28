---
namespace: patterns
agent_id: agent-46-file-length-manager
timestamp: 2026-04-17T14:25:00Z
status: active
tags: split-recommendations, compression-hints, no-structural-splits, page-budget
---

# Split Recommendations

## TL;DR

- **No file splits are required.** The 1,500-line hard rule is not violated anywhere.
  The largest file (`07_case_studies.md`) is at 139 lines — 9.3% of the cap.
- **The word-count target IS violated.** Body prose is ~16,931 words vs the
  mission-brief target of 8,000–12,000 words. Typeset estimate puts the full
  paper at 35–55 pages depending on LaTeX template, against the ≤30-page arXiv
  constraint.
- Since the hard rule is "do not rewrite content," agent-46 does **not** perform
  compression itself. The recommendations below are **hints** for the
  coordinator or a downstream compression pass.

## Structural Splits

**NONE REQUIRED.** Every section file is already at arXiv publication
granularity. No file should be split further; that would create orphan
sub-sections and break the coordinator's `build.sh` assembly order.

## Compression Hints (Not Mandatory Edits)

If the coordinator decides to meet the 30-page budget before camera-ready,
tighten in this priority order. Each recommendation cites the section and
identifies *where* prose can be compressed, **never what to remove**. The
author and coordinator retain full editorial authority.

### Priority 1 — §6 Case Studies (`07_case_studies.md`, 3,240 words)

- **Case 1 Shakespeare (§6.1)** and **Case 2 Santa (§6.2)** are both labelled
  **PROJECTED** in the section. In an arXiv v1, projected case studies can be
  compressed to a single descriptive paragraph each plus one deferred-measurement
  note. Full protocol detail can move to a supplementary appendix.
- **Case 3 Voice Cloning (§6.3)** is the MEASURED anchor. This is the paper's
  empirical spine — do NOT compress this. If anything, the measured numbers
  (0.961 mean SECS, DNSMOS P.808 parity, UTMOS parity) should remain fully
  present.
- **Projected compression savings if §6.1/§6.2 trim 40%**: ~600–800 words
  (~1.3–1.8 pages two-column).

### Priority 2 — §5 TCT Method (`06_method_tct.md`, 2,552 words)

- §5.5 "Three-layer enforcement stack for text generation" and §5.6
  "Case-specific protocols" duplicate some framing from §5.2–§5.4. A single
  bridging paragraph can replace the overlap.
- Equation-heavy subsections (§5.1 constellation, §5.4 runtime guard) are
  already tight. Do NOT compress math.
- **Projected savings if overlap is factored out**: ~300–400 words.

### Priority 3 — §2 Related Work (`02_related_work.md`, 2,498 words)

- §2.1 (data wall) and §2.2 (synthetic-data mitigations) are both densely
  cited. Consider tightening to one-sentence-per-reference format where
  possible.
- §2.6 "Gaps this paper addresses" reiterates claims made in §1 Introduction.
  This subsection can be compressed to ~150 words.
- **Projected savings**: ~300–500 words.

### Priority 4 — §7 Discussion (`08_discussion.md`, 2,367 words)

- §7.1–§7.3 are load-bearing interpretation; do NOT touch.
- §7.4 "Scope of the collapse-silence claim" overlaps with §3.3 "Scope of the
  collapse-silent claim" in the framework section. One of the two should
  point to the other rather than restate.
- §7.6 "Implications" can tighten — 1–2 paragraphs would suffice.
- **Projected savings**: ~200–400 words.

### Priority 5 — §1 Introduction (`01_introduction.md`, 1,616 words)

- Mildly long for an arXiv intro. Typical arXiv cs.LG intros are ~1,000–1,400
  words. No specific subsection flagged — just a general 10–15% trim pass.
- **Projected savings**: ~150–250 words.

## Total Projected Compression

If all five priorities execute at the midpoint of their ranges:
**~1,550–2,350 words saved → ~3.4–5.2 pages in two-column LaTeX**.

| Scenario | Body words | Full words | Pages @ 450 w/p | Pages @ 550 w/p |
|---|---:|---:|---:|---:|
| Current                    | 16,931 | 19,421 | 43.2 | 35.3 |
| After Priority 1 only      | ~16,200 | ~18,700 | 41.6 | 34.0 |
| After P1+P2+P3             | ~15,500 | ~18,000 | 40.0 | 32.7 |
| After all five priorities  | ~14,700 | ~17,200 | 38.2 | 31.3 |
| Minimum realistic target   | ~13,500 | ~15,900 | 35.3 | 28.9 |

Hitting **≤ 30 pages at 450 w/p** requires compressing body prose to ~13,500
words. That's an additional ~1,200-word pass beyond the five priorities above,
which would need either (a) trimming one of the three case studies further,
or (b) moving appendix-suitable content out of the body. Neither is
agent-46's call.

Hitting **≤ 30 pages at 550 w/p (dense LaTeX)** is achievable with just P1+P2+P3.

## What Agent-46 Will NOT Do

- Will NOT rewrite any prose.
- Will NOT delete any references.
- Will NOT split any file.
- Will NOT override the citation validator's decisions (agent-45 locked
  citations; any further prose change that touches a citation must be
  referred back).

## What the Coordinator / Agent-47 SHOULD Decide

1. Whether to target ≤30 pages in arXiv v1 or carry the overshoot into v1
   and address in a v2 revision.
2. Which LaTeX template to use (determines the pages-per-word ratio).
3. Whether to move appendix-candidate material (pseudocode blocks, full
   protocol details for Cases 1 and 2) to a supplementary document.
4. Whether to invoke agent-45 (citation validator) for a second pass after
   any compression that touches cited sentences.

## Confidence

- **HIGH** that no structural splits are needed — zero files over 1,500 lines.
- **HIGH** on the page-overshoot assessment — 16,931 body words is well over
  the 12,000-word target regardless of LaTeX template.
- **MEDIUM-HIGH** on compression priority ranking — based on subsection
  overlap analysis and measured-vs-projected load-bearing distinction.
- **MEDIUM** on projected savings — a real compression pass by an editor
  could exceed these estimates or fall short depending on how aggressively
  clarity is traded for concision.

## Changelog

- 2026-04-17 — Initial recommendations drafted by agent-46-file-length-manager.
