---
namespace: decisions
agent_id: agent-31-ethics-reviewer
timestamp: 2026-04-16T20:15:00Z
status: active
locked: true
supersedes: none
depends_on:
  - memory/context/mission-brief.md
  - memory/discoveries/agent-30-risk-analyst--fmea-risk-register.md
  - memory/handoff/agent-30-to-31--risks-ready.md
downstream_consumers:
  - agent-32-contradiction-analyzer
  - agent-37-methodology-writer
  - agent-42-adversarial-reviewer
  - agent-47-submission-packager
  - agent-48-final-reviewer
tags: competing-interests, coi, commercial-affiliation, teleox, leapable, paste-ready, locked
answers_questions: [Q07, Q16]
---

# §Competing Interests — Paste-Ready Disclosure

## 0. Placement guidance

arXiv does not enforce a competing-interests section, but cs.LG
submissions with commercial affiliations are routinely flagged by
moderators and by downstream peer-review venues (NeurIPS / ICML /
ICLR) that follow ACM / IEEE / NIH disclosure norms. Placement
options, in order of preference:

1. **Page 1 footnote** on the author block, visible to reviewers at
   first glance. Preferred for arXiv v1.
2. **Standalone §Competing Interests** immediately after §Ethics and
   before §9 Conclusion. Preferred for downstream venue submission.
3. **Last paragraph of Acknowledgements.** Weakest placement; use only
   if §-count is a hard constraint.

agent-47 should include the disclosure in at least two of the three
placements for redundancy.

Word count: **~180 words** in the full disclosure; **~60 words** in
the page-1 footnote variant.

---

## 1. Full §Competing Interests disclosure (paste-ready)

> ## Competing Interests
>
> The author declares the following affiliations and funding relevant
> to this work:
>
> **Employment and equity.** The author is the founder and principal
> technical officer of **Teleox.ai** (primary affiliation) and the
> founder of **Leapable.ai** (companion marketplace venture). Both
> entities are early-stage for-profit companies. The author holds
> founder-level equity in both and receives compensation from at least
> one of them during the period of this research. The research
> described in this manuscript was conducted at and with resources
> provided by these entities; the Context Graph and ClipCannon
> software systems described in §4 are developed under the author's
> control within these entities.
>
> **Funding.** This research received no external grant funding. All
> compute, engineering labour, and data-collection costs were borne
> by the author and by Teleox.ai / Leapable.ai operating budgets.
>
> **Non-financial interests.** The author is the sole subject of
> Case 3 (voice clone). This is disclosed in §Ethics and §6.3 opener;
> it is a structural feature of the case study, not a concealed
> relationship.
>
> **Scope of claims.** No claim in this manuscript is made about any
> commercial product of Teleox.ai or Leapable.ai. All reported
> methodology, measurements, and artefacts are research contributions
> re-implementable independently of either entity's proprietary
> services. The Teleox.ai and Leapable.ai names appear in this
> manuscript only in this disclosure and in the Acknowledgements; no
> vendor product is cited, endorsed, or evaluated in the body of the
> paper.
>
> **Data and artefact availability.** Open-source pipeline code and
> evaluation protocols are released publicly (see §Ethics and the
> reproducibility appendix); subject-specific identity artefacts are
> withheld (see §Ethics).

---

## 2. Page-1 footnote (compressed variant)

> *† The author is founder and principal technical officer of
> Teleox.ai and founder of Leapable.ai; both are early-stage for-profit
> entities and the author holds founder-level equity in each. No
> external grant funding was received. No commercial product of either
> entity is claimed, cited, or evaluated in this manuscript. The
> author is also the sole subject of Case 3 (voice clone). See
> §Ethics and §Competing Interests for full disclosure.*

---

## 3. FM-01 compliance scan (for agent-42)

Agent-42 (adversarial-reviewer) must verify, before submission, that
the body of the manuscript is clean of vendor-marketing language. The
following regex patterns should return **zero matches** in §1 through
§9, and **bounded matches** (at most one reference each) in §Ethics,
§Competing Interests, and Acknowledgements:

| Pattern | Allowed matches in body (§1–§9) | Allowed matches in disclosures |
|---|---|---|
| `Teleox\.ai` or `Teleox` | 0 | ≥ 1 |
| `Leapable\.ai` or `Leapable` | 0 (in body); 1 allowed in §7 only as "companion marketplace sidebar" per mission-brief §Hard constraints | ≥ 1 |
| `our product` | 0 | 0 |
| `our method` (first-person plural for methodology) | 0 — replace with "the method" / "TCT" | 0 |
| `our solution` | 0 | 0 |
| `revolutionary`, `breakthrough`, `groundbreaking` | 0 | 0 |
| `state-of-the-art` (as a claim) | 0 | 0 |

Body text uses third-person neutral voice ("the method", "this
framework", "TCT"). First-person-plural "we" is reserved for authorial
actions ("we measure", "we report", "we disclose") and is not used to
claim ownership of methodology as a commercial asset.

---

## 4. Vendor-name body-text regex scan recommendation

Agent-47 should run, before submission:

```bash
grep -nE 'Teleox|Leapable|our product|our solution|revolutionary|groundbreaking' docs/arxivpaper/sections/*.md
```

Expected: matches **only** in `sections/ethics.md` (or equivalent),
`sections/competing_interests.md` (or equivalent), and
`sections/acknowledgements.md`. Any match in a body section (§1–§9) is
a blocker per agent-01 FM-01 and agent-30 R1-02.

---

## 5. Cross-references

- **R4-03 Commercial-affiliation COI** (agent-30 FMEA): this disclosure
  is the primary mitigation. Residual RPN after disclosure = 2 (LOW).
- **R1-02 Desk-reject "marketing paper"** (agent-30 FMEA): this
  disclosure plus the FM-01 regex pass is the primary mitigation.
  Residual RPN = 2 (LOW).
- **FM-01 Vendor-marketing creep** (agent-01): regex scan above is the
  enforcement.

---

## 6. Confidence

**HIGH** that the page-1 footnote plus full §Competing Interests
section together satisfy modern-venue disclosure norms (ACM, IEEE,
NIH, and the NeurIPS 2024 Responsible Research Conduct checklist).

**HIGH** that the FM-01 regex pass is enforceable by agent-42 +
agent-47.

**HIGH** that the scope-of-claims paragraph pre-empts the most common
reviewer objection ("this is a product pitch").

**MEDIUM** on whether a downstream venue will demand an
institutional-affiliation statement beyond what is disclosed (e.g.,
"what oversight body approves your ethical conduct as a
non-university researcher"); principal author may need to pre-arrange
an independent ethics advisor. Agent-31 passes this carry to agent-47
for pre-submission check.

## Changelog

- 2026-04-16 — Initial issue by agent-31-ethics-reviewer. Locked.
