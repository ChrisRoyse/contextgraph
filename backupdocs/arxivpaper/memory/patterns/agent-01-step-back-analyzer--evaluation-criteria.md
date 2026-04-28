---
namespace: patterns
agent_id: agent-01-step-back-analyzer
timestamp: 2026-04-16T15:15:00Z
status: active
tags: evaluation-criteria, arxiv-readiness, pass-fail-tests, review-gates
---

# Evaluation Criteria — arXiv Submission Readiness

This file defines the **pass/fail tests** every downstream agent uses to decide whether its
output is acceptable. These criteria are the operationalization of the principles in
`agent-01-step-back-analyzer--guiding-principles.md`.

A manuscript is **arXiv-ready** iff it passes all nine gates below with confidence ≥ HIGH
on gates marked (H), and confidence ≥ MEDIUM on gates marked (M).

---

## Gate G1 (H) — Provenance Integrity

**Criterion.** 100% of numeric claims in the manuscript body have a provenance tag:
either (a) a citation to a measured source, or (b) an explicit projected/pending marker.

**Evaluator.** Agent-34 (confidence-quantifier) produces a table with one row per numeric
claim: `{claim, value, tier(T1..T4), source, page/line}`. Agent-43 (reproducibility-checker)
audits the table.

**Pass.** Zero bare numbers in prose.
**Fail.** Any numeric value without a provenance marker.

---

## Gate G2 (H) — Definition-Before-Use

**Criterion.** The constructs DDA, meaning compression, TCT, and identity-locked generation
each have exactly one canonical definition (written by agent-04), and every use in the
manuscript is consistent with that definition.

**Evaluator.** Agent-04 writes `docs/arxivpaper/memory/patterns/agent-04-construct-definer--canonical-definitions.md`.
Agent-42 (adversarial-reviewer) cross-checks every usage.

**Pass.** One definition per construct; every usage consistent; first use in §3 (or earlier
sections cite forward).
**Fail.** Semantic drift (a construct means different things in §3 vs §7).

---

## Gate G3 (H) — Novelty Delta Specificity

**Criterion.** For each of the four Core Claims (DDA, meaning compression, TCT,
sample-complexity collapse), §2 Related Work names the nearest precedent and states the
delta in one sentence. Delta statements are narrow and falsifiable.

**Evaluator.** Agent-36 (literature-review-writer) produces a 4-row table
`{claim, nearest precedent, delta}`. Agent-42 reviews for overclaim.

**Pass.** All four rows populated; each delta is specific (e.g., "TurboQuant compresses
weights to preserve activations; meaning compression compresses semantics to preserve
identity under generation — different object, different metric").
**Fail.** Any delta is vague ("we're the first to combine X and Y").

---

## Gate G4 (H) — No-Marketing Regex

**Criterion.** The assembled `PAPER.md` body contains zero occurrences (case-insensitive) of
the forbidden-term list: revolutionary, breakthrough, game-changing, best-in-class,
unprecedented, world-class, industry-leading, solves AGI, the future of, paradigm shift.
Teleox.ai / Leapable.ai appear only in Acknowledgements and the disclosure footnote.

**Evaluator.** Agent-42 runs a regex scan. The regex is stored in this file for
reproducibility:
```
\b(revolutionary|breakthrough|game[- ]changing|best[- ]in[- ]class|
unprecedented|world[- ]class|industry[- ]leading|paradigm shift|
the future of|solves? AGI)\b
```

**Pass.** Zero matches.
**Fail.** Any match outside Acknowledgements / Disclosure.

---

## Gate G5 (H) — Length Budget

**Criterion.** Assembled `PAPER.md` prose body (excluding references, appendix, figures) is
between **7,500 and 12,500 words**. Typeset estimate ≤ 30 pages. References: 30–60 entries.

**Evaluator.** Agent-46 (file-length-manager) produces a word count and reference count.

**Pass.** Both bounds satisfied.
**Fail.** Over-long (>12.5K words) or under-cited (<30 refs).

---

## Gate G6 (H) — Reproducibility Completeness

**Criterion.** For each of the three case studies and each of the two systems, the manuscript
(body + appendix) supplies:
  1. Input data provenance (count, source, curation).
  2. Model architecture / embedder list with dimensions.
  3. Hyperparameters actually used (learning rate, batch, epochs, LoRA rank, etc.).
  4. Evaluation metric definitions with the exact computation.
  5. Seed / determinism note (or explicit non-determinism disclosure).
  6. A pointer to the reproduction script / command (or "source-proprietary" flag with
     reason).

**Evaluator.** Agent-43 (reproducibility-checker) produces a 5×6 matrix; every cell must be
filled or explicitly marked "N/A: <reason>".

**Pass.** Matrix complete; no unexplained blanks.
**Fail.** Any missing cell without rationale.

---

## Gate G7 (M) — Citation Validity

**Criterion.** Every cited URL resolves (HTTP 200 or known-stable DOI). Every reference
follows APA 7th. The four evidence briefs (`_evidence_01` through `_evidence_04`) are the
primary pool; additions by other agents are allowed but must pass the same URL check.

**Evaluator.** Agent-45 (citation-validator) produces a URL-health report. Agent-44
(apa-citation-specialist) validates APA format.

**Pass.** 100% URL resolution, 100% APA conformance (≥ medium confidence acceptable because
transient URL failures don't necessarily mean stale sources).
**Fail.** >5% URL failures, or any APA malformation that affects retrievability.

---

## Gate G8 (M) — Empirical Tier Labeling

**Criterion.** Every §6 (Results) subsection explicitly states the evidence tier of its
numbers. Case 3 is T1 (measured). Cases 1 and 2 are T2+T3 (constructive + projected).

**Evaluator.** Agent-38 (results-writer) labels each subsection header with its tier.
Agent-42 cross-checks.

**Pass.** Every §6 subsection labeled.
**Fail.** Any results paragraph conflates tiers.

---

## Gate G9 (M) — Limitations Completeness

**Criterion.** §8 Limitations names (non-exhaustively): (a) sample-size limits of Case 3
(10 sentences, single subject), (b) pending-run status of Cases 1 and 2, (c) scope limits of
DDA's "N + C(N,2)" count (correlation not true orthogonality), (d) non-generalization claims
outside the specific protocols tested, (e) evaluation-metric limits (SECS is one score; DNSMOS
/ UTMOS are MOS proxies; WavLM is English-biased), (f) author-as-sole-evaluator bias.

**Evaluator.** Agent-29 (validity-guardian) produces the limitations list; agent-42 audits
for missing items.

**Pass.** All six named explicitly; no "limitations" section that is just handwaving.
**Fail.** Limitations reads as marketing ("our main limitation is how much opportunity remains").

---

## Gate tie-breakers (coordinator only)

When gates conflict (e.g., G5 length vs G6 reproducibility completeness), the coordinator
invokes this precedence, highest first:

1. G1 Provenance — never sacrifice
2. G4 No-Marketing — never sacrifice
3. G2 Definition-Before-Use — never sacrifice
4. G6 Reproducibility — prefer in-body, overflow to appendix
5. G3 Novelty Delta — prefer crisp over complete
6. G8 Tier Labeling
7. G9 Limitations
8. G7 Citation Validity (transient URLs tolerated)
9. G5 Length — cut rhetoric first, method last

---

## Summary scoreboard (for agent-48 final sign-off)

| Gate | Confidence | Evaluator | Remediation cost if failing |
|------|-----------|-----------|------------------------------|
| G1 Provenance | H required | 34, 43 | LOW (add citations / flags) |
| G2 Defn-Before-Use | H required | 04, 42 | LOW (add glossary entry) |
| G3 Novelty Delta | H required | 36, 42 | MEDIUM (may require lit review revision) |
| G4 No-Marketing | H required | 42 | LOW (find-and-replace) |
| G5 Length | H required | 46 | MEDIUM (requires cuts) |
| G6 Reproducibility | H required | 43 | HIGH (may require new runs) |
| G7 Citation Validity | M acceptable | 44, 45 | LOW (verify URLs) |
| G8 Tier Labeling | M acceptable | 38, 42 | LOW (annotation pass) |
| G9 Limitations | M acceptable | 29, 42 | LOW (one section rewrite) |

If any H-gate is FAILING at agent-48, the submission is held and a remediation cycle is
invoked (specific remediation agent selected by cost column).
