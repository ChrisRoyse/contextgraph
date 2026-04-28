---
namespace: handoff
agent_id: agent-43-reproducibility-checker
timestamp: 2026-04-17T12:40:00Z
status: active
supersedes: none
tags: handoff, agent-44, apa-citation-specialist, reproducibility-ready
---

# Handoff: Agent-43 (reproducibility-checker) → Agent-44 (apa-citation-specialist)

## Read these files FIRST, in this order

1. `memory/discoveries/agent-43-reproducibility-checker--audit.md` —
   full reproducibility audit. Read §4.2 (Case-3 source cross-check
   with three discrepancies), §6 (gaps requiring §15 appendix), §7
   (cross-cutting issues), and §8 (posture summary) in that order.
2. `memory/patterns/agent-43-reproducibility-checker--reproducibility-appendix-draft.md`
   — paste-ready §15 appendix draft. The citation-validator's scope
   touches §15.3 (embedder / scorer manifest) and §15.16 (references)
   most directly.
3. `memory/handoff/agent-42-to-43--critique-ready.md` — the upstream
   adversarial critique; agent-44's citation validation must check
   that the stop-ship edits agent-42 specified preserve citation
   integrity (especially STOP-3 / STOP-4 / CA-12 tag removal — do any
   citations get dropped in those edits?).
4. `sections/06_method_tct.md` and `sections/07_case_studies.md` —
   the sections citation validation targets.

## What agent-43 found (executive summary for agent-44)

### Reproducibility verdict
- **§5 TCT method**: ⚠️ Partially reproducible from body alone.
  Protocol is specified; hashes / seeds / hardware missing.
- **§6.1 Case 1**: ⚠️ Partially reproducible. 9-embedder panel not
  listed in body; training hyperparameters not in body; IC-6
  byte-identical label missing.
- **§6.2 Case 2**: ⚠️ Partially reproducible (by design). $\lambda_{\text{voice}}$
  and $\lambda_{\text{emotion}}$ missing from §5.6 (Finding 5.5 carry
  from agent-42). IC-6 protocol-level label missing.
- **§6.3 Case 3**: ⚠️ Conditionally reproducible — **three source
  mismatches** against `alldata.md` need resolution:
  - §6.3.4 Table 2 rows do not match alldata.md §5.5 (paper
    substitutes headline 0.961/0.975 for the ablation-internal
    0.939/0.950).
  - §6.3.3 STOP-3 "two separate seed chains" claim is not
    source-supported.
  - §6.3.2 N=14 ambiguity — clone N=14 but real N=1; paper reads as
    N=14 for both.

### Cross-cutting
- **IC-6 asymmetric-release posture is not labelled** in §6 body.
  Three one-sentence edits required (§6.1.1, §6.2.1, §6.3.1).
- **`qwen3-tts-foricl-v1` is undefined**. Dangling version tag cited
  three times; no manifest.
- **§15 Reproducibility appendix is REQUIRED**. 13 content items
  (A1–A13) drafted in the patterns file.

## Citation-specific handoff (agent-44's domain)

### Citation risks agent-43 flagged that touch agent-44

1. **Embedder / scorer citations**. Agent-43 appendix §15.3 tables
   WavLM (Chen et al. 2022), ECAPA-TDNN (Desplanques et al. 2020),
   DNSMOS (Reddy et al. 2022), UTMOS (Saeki et al. 2022), SigLIP
   (Zhai et al. 2023), ColBERTv2 (Santhanam et al. 2022), SPLADE
   (Formal et al. 2021), KEPLER (Wang et al. 2021), e5-large-v2
   (Wang et al. — version?), nomic-embed (Nussbaum et al. — URL?),
   EchoMimicV3 (Meng et al. 2025), DPO (Rafailov et al. 2023),
   Flow-matching (Lipman et al. 2023), Qwen3-TTS (Qwen Team 2024
   model card). Every citation must have a verified URL in one of
   `docs/researchpaper/_evidence_01_*.md` through
   `_evidence_04_*.md`.

2. **STOP-3 paste-ready edit in agent-42 patterns** adds a paragraph
   to §6.3.3 that is arguably not source-supported. Agent-44 should
   verify that NO citation appears in the STOP-3 replacement text
   that is not already in the bibliography, and that the VALL-E 2
   number (0.881 WavLM-large-sv) cited in the STOP-4 / Finding 2.3
   edit has a traceable source (agent-33 CA-3 flags this).

3. **Finding 2.3 STOP-4 "VALL-E 2 at approximately 0.881"** is a
   direct citation to Chen et al. 2024 (VALL-E 2); agent-44 must
   verify the 0.881 number is present in that source (adversarial
   reviewer is concerned it may be a secondary-source citation).

4. **`docs/inspect/08_training_audit.md`** is cited as an internal
   document in §6.2.2. Not a published citation; agent-44 should
   leave it as a `arXiv v1: repository path` citation and flag for
   v2 as a commit-pinned GitHub URL.

5. **`docs2/LORA_STYLE_TRANSFER_COMPLETE_REPORT.md`** is cited in
   §6.1.3 (Case 1 forensic report). Same treatment as item 4.

6. **Footnote introduction of the "88-citation pool" phrase**
   (agent-42 Finding 1.2). Agent-44 should confirm that after
   global-replace the reference list still has 88 entries (or 30–60
   per mission-brief constraints).

### Citation completeness

Mission brief §"arXiv compliance targets" specifies 30–60 references
total. The current master reference pool is 88 (agent-08). Agent-44
should:
- Verify every citation in the body is in the master pool.
- Confirm every master-pool entry has a verified URL in
  `_evidence_*.md`.
- Check that the 30–60 target is achievable (if 88 is too many,
  identify the 28+ citations that are in the master pool but NOT in
  any body section — they can be pruned).

### Items agent-43 did NOT check (explicitly downstream of agent-44)

- Full APA 7 formatting of every reference entry.
- Verified-URL audit against a dated snapshot.
- DOI resolution for every DOI-bearing entry.
- ORCID resolution for first authors.
- In-text citation format (Author, YYYY) vs. (Author et al. YYYY).
- Page-range citation for book chapters.

## Suggested sequence for agent-44

1. Read `memory/discoveries/agent-43-reproducibility-checker--audit.md`
   §4.2 first — confirm the three Case-3 discrepancies do not
   introduce missing citations.
2. Skim `memory/patterns/agent-43-reproducibility-checker--reproducibility-appendix-draft.md`
   §15.3 and §15.16 — the pinned embedder/scorer references.
3. Run APA formatting pass on the master reference pool.
4. Cross-check every body citation against the verified-URL evidence
   briefs.
5. Verify VALL-E 2 0.881 is in Chen et al. 2024 (special attention).
6. Flag any "arXiv v1: repository path" internal refs for v2
   upgrading.

## Files agent-43 produced

1. `memory/discoveries/agent-43-reproducibility-checker--audit.md`
   (main audit; ten-item checklist × 4 scope units; Case 3 source
   cross-check; cross-cutting issues; summary table).
2. `memory/patterns/agent-43-reproducibility-checker--reproducibility-appendix-draft.md`
   (paste-ready §15 Reproducibility appendix, ~1,700 lines).
3. `memory/handoff/agent-43-to-44--reproducibility-ready.md` (this
   file).
4. `memory/reports/agent-43-reproducibility-checker--final-report.md`
   (final report).

## Confidence

**HIGH** that the three Case-3 source mismatches (§4.2.1 / §4.2.2 /
§4.2.3 of the audit) are correct findings. alldata.md was read in
full at the relevant sections.

**HIGH** that §15 appendix is required. Agent-47 (final assembler)
must ensure §15 is present in the shipped manuscript.

**HIGH** that items 1–6 of audit §9 (CRITICAL recommendations) move
the paper from conditionally reproducible to reproducible at the
disclosed IC-6 scope.

**MEDIUM** that I caught every reproducibility issue. I did NOT
re-run the pipeline; my audit is specification-vs-source and
ten-item-checklist based.

## Dependencies I'm leaving for downstream

- **Agent-44**: APA formatting of §15 references table; verify
  VALL-E 2 citation; prune master pool to 30–60 if needed.
- **Agent-46 (coherence-checker)**: ensure the §15 appendix cross-
  references back to §5 / §6 body correctly; ensure IC-6 labels
  appear consistently where inserted.
- **Agent-47 (final-assembler)**: ship §15 appendix with the
  manuscript; import the three body-text edits (§6.1.1, §6.2.1,
  §6.3.1 IC-6 labels); resolve the three Case-3 source mismatches
  (§6.3.2, §6.3.3, §6.3.4).
- **Author**: pin weight SHA-256 for all embedders / scorers at
  submission time; fill hardware / wall-clock rows; confirm
  $\lambda_{\text{voice}}$ / $\lambda_{\text{emotion}}$ values for
  Case 2; confirm the 10 raw SECS values for Case 3 appendix.

## Changelog

- 2026-04-17 — Handoff written by agent-43-reproducibility-checker.
