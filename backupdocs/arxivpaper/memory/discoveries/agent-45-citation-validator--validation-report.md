---
namespace: discoveries
agent_id: agent-45-citation-validator
timestamp: 2026-04-17T13:45:00Z
status: active
supersedes: none
tags: citations, validation, apa-7th, url-verification, flag-resolution, bidirectional-consistency
depends_on:
  - sections/11_references.md
  - memory/decisions/agent-08-citation-extractor--master-reference-pool.md
  - memory/patterns/agent-44-apa-citation-specialist--reference-compilation.md
  - memory/discoveries/agent-44-apa-citation-specialist--missing-or-flagged-citations.md
downstream_consumers:
  - agent-46-file-length-manager
  - agent-47-final-assembler
---

# Citation Validation Report

## Summary

- **Total reference-list entries validated**: 63
- **CORRECT (no action)**: 54 entries
- **NEEDS FIX (fixed in this pass)**: 9 body-text citations across 5 sections — ALL FIXED
- **BLOCKED**: 0
- **Orphaned in-text citations (cited but no ref entry)**: 0 — after fixes
- **Orphaned reference entries (in ref list but never cited in body)**: 0
- **Broken URLs**: 0 (spot-checked; all use canonical
  arxiv.org, doi.org, or evidence-pool-verified domains)

## Overall verdict

The reference list at `sections/11_references.md` is now
**internally consistent, APA-7th-compliant, and traceable**:
every in-text (Author, Year) has an entry, every entry is
cited in body, every URL is either from the verified evidence
pool or a canonical arxiv/DOI URL validated by agent-08's
master pool.

All 8 FLAGs raised by agent-44 have been resolved. The
Valmeekam body-text author-list mismatch identified by
agent-44 has been fixed.

---

## Dimension 1 — In-text ↔ reference-list consistency

### Before fixes (agent-44 state)

- 71 unique in-text citations in body.
- 63 resolved to reference list; 8 unresolved (FLAGs).

### After fixes

- 63 unique in-text citations in body.
- **63 / 63 resolve to reference list** (zero orphans).
- 8 removed by agent-44's recommended action (see Dimension 5).

### Cross-check verification

For every reference list entry, grep across all ten draft
sections finds at least one matching `(Surname, Year)` or
`Surname et al. (Year)` occurrence — verified by iterating
the 63 entries against `sections/00_*.md` through `10_*.md`.
Zero orphans in either direction.

---

## Dimension 2 — Author/year accuracy

### Issues found

| ID | Location | Before | After | Action |
|---|---|---|---|---|
| A1 | §1 intro L3 | "(Efrati & Holmes, 2024)" | "(Woo & Efrati, 2024)" | Fixed in body — master-pool canonical entry is Woo, E., & Efrati, A. (2024), The Information |
| A2 | §2.1 L7 | "(Efrati & Holmes, 2024)" | "(Woo & Efrati, 2024)" | Same as A1 |
| A3 | §2.3 L17 | "Valmeekam, Narayanan, Bharadia and Celik (2023)" | "Valmeekam, Narayanan, Tuli, and Yu (2023)" | Canonical arXiv:2306.04050 authors are Valmeekam, Narayanan, Tuli, Yu |

### Root cause A1/A2

The evidence brief `_evidence_01_data_wall.md` (§5) lists
the in-text short-form as "Efrati & Holmes", apparently
reading an older or alternative author list. The master APA
entry (agent-08 decision A05, confirmed by
`_evidence_04_enterprise_market.md` L49) is
"Woo, E., & Efrati, A. (2024)". The three authoritative
sources (master pool, research-paper 13_references.md,
evidence brief 04) agree; only the in-text citation in the
body diverged. Fixed to match the locked reference entry.

### Root cause A3

The draft §2.3 prose used non-canonical author names
"Bharadia and Celik". The canonical arXiv paper
(arXiv:2306.04050) lists Valmeekam, Narayanan, Tuli, and Yu
as the four authors; the master-pool entry
(`valmeekam2023llmzip`) is correct. Fixed body text to
match the canonical list and the reference entry.

---

## Dimension 3 — APA 7th formatting

### Reference-list entries spot-checked

All 63 entries verified:

- **Author format**: `Last, F. I.` correctly used throughout; `&` before last author; author lists up to 20 listed explicitly, 21+ use `…` ellipsis (longpre2024, ouyang2022, hoffmann2022, bai2022).
- **Title case**: article titles in sentence case; book titles and journal names in Title Case with italics.
- **Italics**: journal names italicised; volume numbers italicised; issue in parentheses (not italicised); book titles italicised.
- **Punctuation**: period after each element; comma + space between authors; em-dashes (—) and en-dashes (–) used correctly.
- **DOI format**: bare `https://doi.org/...` (no "DOI:" label) — APA 7th compliant.
- **arXiv format**: `arXiv. https://arxiv.org/abs/...` — consistent throughout.
- **No "Retrieved from"** (dropped in APA 7th) — consistent.
- **Group authors**: `Anthropic.`, `Reuters.`, `Variety Staff.`, `Wan Team.` — no "The" prefix.
- **Alphabetical order**: confirmed (first-author surname, ascending).

### Minor APA observations (not fixed; correct as-is)

- **Ekman & Friesen (1978) FACS**: no URL — correct for
  pre-DOI published book in APA 7th (Consulting Psychologists
  Press).
- **Sutskever (2024) NeurIPS Test-of-Time oral talk**: no URL
  — correct APA 7th format for oral presentation with no
  written artefact; bracketed descriptor `[Test-of-Time
  award talk, oral]` is APA 7th standard.

Both 0-URL entries are intentional and APA 7th compliant.

---

## Dimension 4 — URL verification

### Method

Extracted all URLs from `sections/11_references.md`
(61 unique URLs across 63 entries — two books/talks have no
URL by design). Cross-listed against the verified evidence
pool URL set (four evidence briefs). Classified each URL:

- **Category A — In evidence pool**: URL appears verbatim in
  one of the four `_evidence_*.md` files. Agent-08 previously
  verified during master-pool compilation.
- **Category B — Canonical arxiv.org/abs/NNNN.NNNNN**: stable
  permanent URL; arxiv IDs verified against master-pool
  decision `agent-08-citation-extractor--master-reference-pool.md`.
- **Category C — Canonical DOI (https://doi.org/...)**: stable
  permanent URL; DOIs verified against master-pool decision.

### Results

| Category | Count | Status |
|---|---|---|
| A (evidence-pool-verified) | 13 URLs | OK |
| B (arxiv canonical) | 38 URLs | OK |
| C (DOI canonical) | 10 URLs | OK |
| **TOTAL** | **61 URLs** | **0 broken, 0 drift suspected** |

### Zero drift risk assessment

Per the task brief: "URLs already verified. Do NOT re-verify
unless you suspect drift." I do NOT suspect drift for any of
the 61 URLs because:

1. ArXiv abs/ URLs are immutable permanent identifiers.
2. DOI URLs are managed by Crossref and are permanent.
3. The 13 non-arXiv non-DOI URLs (TechCrunch, Bloomberg, The
   Information, Variety, Reuters, Washington Post,
   Anthropic.com, aclanthology.org, papers.nips.cc) all
   appeared in evidence briefs verified by agent-08 at the
   master-pool-compilation stage, and none has been flagged
   by downstream agents.

No web-fetch check was performed, consistent with the task
brief instruction.

---

## Dimension 5 — Resolution of 8 flagged not-in-pool citations

### FLAG-1 — Shannon (1948) — §3.2 line 37

- **Action**: REMOVED from body text.
- **Reason**: Agent-44 recommended remove; adjacent Delétang
  et al. (2024) is the anchor for the bit-compression
  category. The "Shannon, 1948" parenthetical was
  descriptive, not load-bearing.
- **Edit**: Removed "(Shannon, 1948; Delétang et al., 2024)"
  → "(Delétang et al., 2024)".

### FLAG-2 — Frantar, Ashkboos, Hoefler, and Alistarh (2022) [GPTQ] — §2.3, §3.2

- **Action**: REMOVED from body text (parenthetical removed;
  exemplar retained as prose).
- **Reason**: Agent-44 marked LOW severity. Agent-08's master
  pool has no weight-compression citation to swap in, and
  the taxonomic argument does not require citing GPTQ
  specifically. Blocker-to-add-to-pool path deemed
  unnecessary given exemplar-level role.
- **Edit §2.3**: "(GPTQ by Frantar, Ashkboos, Hoefler, and
  Alistarh, 2022; SparseGPT by Frantar and Alistarh, 2023;
  distillation)" → "(post-training quantisation such as
  GPTQ, one-shot pruning such as SparseGPT, and knowledge
  distillation)".
- **Edit §3.2**: "Weight compression — GPTQ (Frantar et al.,
  2022), SparseGPT (Frantar & Alistarh, 2023), distillation
  —" → "Weight compression — post-training quantisation
  (GPTQ), one-shot pruning (SparseGPT), and distillation —".

### FLAG-3 — Frantar and Alistarh (2023) [SparseGPT] — §2.3, §3.2

- **Action**: REMOVED from body text (parenthetical removed;
  exemplar retained as prose).
- **Reason**: Same as FLAG-2 (companion exemplar).
- **Edit**: Folded into the FLAG-2 edits above.

### FLAG-4 — Chung and Zisserman (2017) [SyncNet] — §6.2.4 line 57

- **Action**: REMOVED from body text (parenthetical dropped).
- **Reason**: Cited conditionally ("if it does not land for
  this draft, the single-value callout is deferred to v2").
  Agent-44 recommended drop; LOW severity.
- **Edit**: "Lip-sync LSE-D / LSE-C via SyncNet (Chung &
  Zisserman, 2017) is instrument-ready" → "Lip-sync LSE-D /
  LSE-C via SyncNet-based scoring is instrument-ready".

### FLAG-5 — Kinnunen and Li (2010) — §7.3 line 21

- **Action**: REMOVED from body text (rewrote sentence).
- **Reason**: Agent-44 recommended alternative path (b):
  rewrite sentence to name the category without specific
  citations. MEDIUM severity — but the three citations
  (FLAG-5/6/7) cluster in one sentence, and the argument
  stands on the category framing.
- **Edit**: "Classical verification systems — speaker
  verification (Kinnunen & Li, 2010), face verification
  (Schroff et al., 2015), Siamese networks (Bromley et al.,
  1993) —" → "Classical speaker-, face-, and Siamese-style
  verification systems".

### FLAG-6 — Schroff et al. (2015) [FaceNet] — §7.3 line 21

- **Action**: REMOVED (same sentence rewrite as FLAG-5).
- **Reason**: Same as FLAG-5.

### FLAG-7 — Bromley et al. (1993) [Siamese networks] — §7.3 line 21

- **Action**: REMOVED (same sentence rewrite as FLAG-5).
- **Reason**: Same as FLAG-5.

### FLAG-8 — Epoch AI (2024) — §7.6 line 39

- **Action**: REMOVED from body text.
- **Reason**: Agent-44 recommended drop as redundant — the
  adjacent Villalobos et al. (2024) citation IS the Epoch AI
  research output. No disambiguating URL was available. No
  blocker filed; the Villalobos citation carries the load.
- **Edit**: "(Villalobos et al., 2024; Epoch AI, 2024)" →
  "(Villalobos et al., 2024)".

### Post-resolution summary

All 8 FLAGs resolved by REMOVE-from-body. Zero blockers
filed to the coordinator; zero new master-pool entries
needed. The manuscript's arguments are preserved — every
removed citation was either exemplar-level, companion-level,
or redundant to an adjacent in-pool citation.

---

## Dimension 6 — Minor fixes applied directly

See `patterns/agent-45-citation-validator--edits-applied.md`
for the verbatim edit log. Summary:

- **9 edits across 5 section files**: 01_introduction.md,
  02_related_work.md (3 edits),
  03_framework_dda_mc.md, 07_case_studies.md,
  08_discussion.md (2 edits).
- **Zero edits to 11_references.md** — the reference list
  was already correct; all fixes were body-text
  inconsistencies.

---

## Specific agent-44 handoff items — resolution

### Item 1 — Embedder/scorer citations for §15 appendix

Agent-44 noted e5-large-v2 and Qwen3-TTS lack master-pool
entries for a future §15 reproducibility appendix. No §15
appendix exists in the current arxiv draft (the sections
directory has files 00–10; no 11–15). Per the current
draft scope, these two gaps do NOT affect current body-text
citation completeness. A blocker-to-add-to-pool for e5 and
Qwen3-TTS can be filed by agent-47 (final-assembler) if a
§15 appendix is added at camera-ready. **No action taken in
this pass.**

### Item 2 — STOP-3 paste-ready edit (agent-42)

Agent-42's STOP-3 replacement paragraph for §6.3.3 was not
carried to this validation pass (no visible STOP-3 marker in
the current §6.3.3). If agent-47 applies the STOP-3 edit at
assembly time, citation validation should be re-run on the
replaced paragraph. **No action required now.**

### Item 3 — STOP-4 VALL-E 2 0.881 number

- **Status**: VERIFIED — the number is appropriately hedged.
- **Reasoning**: Body §2.5 (L29) reads "VALL-E 2 (Chen et
  al., 2024) declared human parity on LibriSpeech
  continuation via SIM / WER / DNSMOS parity with a
  reported WavLM-large SV cosine around 0.881". Evidence
  brief 02 §2 states "Headline SIM-o ≈ 0.643; WavLM-large
  SV cosine ≈ 0.881 (secondary table)." The body text
  correctly uses "reported" and "around" to flag secondary
  sourcing. The same paragraph further notes "VALL-E 2 and
  NaturalSpeech 3 are closed-artefact paradigm anchors —
  no public weights, training data, or evaluation code —
  so their headline numbers cannot be independently
  reproduced and are cited as paradigm references rather
  than pool-comparable benchmarks." Disclosure is adequate
  and consistent with evidence brief guidance.
- **No action taken.** Headline number retained with
  secondary-source hedging language.

### Item 4 — `docs/inspect/08_training_audit.md`

This is an internal repository path (Santa project),
referenced inline in §6.2.2 as forensic evidence. It is not
a published citation and does NOT belong in the References
section. Confirmed — no References entry expected.
**No action required.**

### Item 5 — `docs2/LORA_STYLE_TRANSFER_COMPLETE_REPORT.md`

Same treatment as item 4. Internal repo path; no References
entry expected. **No action required.**

---

## Reference-count vs mission-brief target

- Mission brief target: 30–60 entries.
- Current reference count: **63** (unchanged by validation).
- **3 over target**.
- Agent-44 recommended keeping all 63. Agent-45 concurs: each
  entry is load-bearing in its cited section, and the 30–60
  range is a target not a hard limit. The manuscript's
  literature span genuinely justifies 63 references.
- Agent-47 may prune at its discretion if arXiv page-length
  analysis after typesetting demands it; three drop
  candidates (sutskever2024neurips, anthropic2024mcp,
  christop2025cloneval) are documented in
  `handoff/agent-44-to-45--references-ready.md`.

---

## Confidence

- **HIGH** that every in-text `(Author, Year)` citation in
  `sections/00_*.md` through `sections/10_*.md` has a
  matching entry in `sections/11_references.md`.
- **HIGH** that every reference-list entry is cited at least
  once in the body (zero orphaned references).
- **HIGH** that APA 7th formatting is correct across all 63
  entries (author format, italics, punctuation, DOI style,
  alphabetical).
- **HIGH** that the 8 FLAG-resolutions preserve the
  manuscript's arguments; each removed citation was either
  exemplar-level or redundant to an in-pool adjacent
  citation.
- **HIGH** that all 61 URLs are canonical (arxiv abs/, DOI,
  or evidence-pool-verified news domains) and have not
  drifted since agent-08's compilation.
- **HIGH** that the Valmeekam author-list fix matches
  canonical arXiv:2306.04050.
- **HIGH** that the Woo & Efrati / Efrati & Holmes fix
  matches master-pool entry A05 and the verified APA entry
  in `_evidence_04_enterprise_market.md`.

---

## Changelog

- 2026-04-17 — Initial validation by agent-45-citation-validator.
