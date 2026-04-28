---
namespace: reports
agent_id: agent-43-reproducibility-checker
timestamp: 2026-04-17T12:45:00Z
status: complete
supersedes: none
tags: final-report, agent-43, reproducibility, arxiv-preparation
---

# Final Report — Agent-43 Reproducibility Checker

## 0. Task completion statement

Completed. Four files written:
1. `memory/discoveries/agent-43-reproducibility-checker--audit.md`
2. `memory/patterns/agent-43-reproducibility-checker--reproducibility-appendix-draft.md`
3. `memory/handoff/agent-43-to-44--reproducibility-ready.md`
4. `memory/reports/agent-43-reproducibility-checker--final-report.md` (this file)

## 1. Executive summary

**Overall reproducibility level**: ⚠️ **CONDITIONALLY REPRODUCIBLE**.
The paper's TCT method (§5) is specified to a level an independent
team can re-implement at concept level, but weight hashes, training
seeds, and hardware are absent from the body. The three case studies
are reproducible at different levels:
- Case 1 (Shakespeare): byte-identical (PROJECTED, pending release).
- Case 2 (Santa): protocol-level (by design; PROJECTED end-to-end).
- Case 3 (voice clone): byte-identical headline (MEASURED), but has
  **three source-trace discrepancies** against `alldata.md` that must
  be resolved.

**Biggest gaps**:
1. **§6.3.4 Table 2 does not match its cited source** (`alldata.md`
   §5.5). The paper substitutes the headline 0.961/0.975 values for
   the ablation-internal 0.939/0.950 values. Δ = +0.015 is
   arithmetically consistent but row labels are mis-attributed.
   Severity: HIGH — a reproducer running the ablation will get
   different numbers and conclude the paper is wrong.
2. **§6.3.3 STOP-3 paste-edit** (from agent-42) claims two separate
   seed chains at T=0.3 — NOT source-supported in `alldata.md`.
   Severity: HIGH — asserts a reproducibility claim the author must
   substantiate or withdraw.
3. **§6.3.2 N=14 ambiguity** — the clone DNSMOS/UTMOS is N=14 and
   the real-mic recording is N=1, but §6.3.2 attributes N=14 to
   "comparison set" without disambiguation. Severity: MEDIUM.
4. **IC-6 asymmetric-release labels missing from §6.1/§6.2/§6.3**.
   A reader has no way to tell what is released.
5. **`qwen3-tts-foricl-v1` is cited as a pipeline version but never
   defined** — no manifest, no commit hash.
6. **$\lambda_{\text{voice}}$ and $\lambda_{\text{emotion}}$ missing
   from §5.6 Case 2 block** (Finding 5.5 carry from agent-42).
7. **9-embedder STYLE_CENTROID panel not listed in §6.1.2** — the
   phrase "9 embedders" appears but the 9 are in agent-27 only.
8. **Hardware, wall-clock, CUDA, fp precision, random seeds (training
   and inference) all absent from body.**
9. **Data-availability and subject-consent statements not drafted**
   anywhere in §4–§9.
10. **Code-availability statements not drafted** — IC-6 commitment
    to three-tier release is not in body.

**Appendix required**: **YES.** A §15 Reproducibility Appendix is
mandatory. 13 content items (A1–A13) are drafted in the patterns
file. Without it, even protocol-level reproducibility is blocked.

## 2. Files written

- `memory/discoveries/agent-43-reproducibility-checker--audit.md`
  — Full audit with §1 (TCT), §2 (Case 1), §3 (Case 2), §4 (Case 3
  with three source discrepancies), §5 (systems), §6 (required
  appendix items A1–A13), §7 (cross-cutting), §8 (summary), §9
  (prioritised recommendations), §10 (author checklist).
- `memory/patterns/agent-43-reproducibility-checker--reproducibility-appendix-draft.md`
  — Paste-ready §15 appendix draft. 16 sub-sections: scope,
  hyperparameters, embedder manifest, sampling, session structure,
  JSON schema, hardware, source reconciliation, N=14 disambiguation,
  bootstrap reproducibility, three deferred measurements, alldata.md
  crosswalk, consent/ethics, verifier checklist, known limitations,
  references.
- `memory/handoff/agent-43-to-44--reproducibility-ready.md` — Handoff
  to apa-citation-specialist with explicit citation-validation asks:
  verify VALL-E 2 0.881, verify embedder/scorer citation URLs,
  confirm master pool is 30–60 citations per mission brief.
- `memory/reports/agent-43-reproducibility-checker--final-report.md`
  — this file.

## 3. Method

Ten-item reproducibility checklist (data, models, embedders, training
HPs, inference HPs, metrics, hardware, seeds, code availability, data
availability) applied to four scope units (§5 TCT method, §6.1 Case 1,
§6.2 Case 2, §6.3 Case 3). Status ratings: ✅ / ⚠️ / ❌.

For Case 3 (the MEASURED anchor, highest-stakes), an additional
cross-check: §6.3 body vs. `tasks/newdata/alldata.md` (cited source
document). Three discrepancies found and documented.

Upstream decision files (agent-25 / 26 / 27 / 28) were read in full
to determine whether each missing body item is recoverable in an
appendix (A1–A13 of the audit §6).

## 4. Key findings (beyond audit)

- **The paper is reproducibility-fragile at the source-trace level,
  not at the concept level.** The underlying claims (geometric-
  proximity per-output verifiability, 0.961 mean WavLM SECS, the
  three-layer stack, the seven-modality constellation) are well
  specified. The risk is that a reviewer running a source-integrity
  check on §6.3.4 will find the Table 2 row-to-source mismatch and
  question the whole measurement.
- **An appendix CAN close almost every body-level gap** (items 1–13
  in audit §6.1). But items 4, 5, 6 must also be addressed in body
  (IC-6 labels, Table 2 reconciliation, STOP-3 substantiation).
- **The asymmetric-release posture is the governing reproducibility
  frame** (IC-6) and is absent from the body. Three one-sentence
  inserts in §6.1.1 / §6.2.1 / §6.3.1 close this.
- **Upstream agent-42 STOP-3 paste-edit, if applied literally,
  introduces an unsubstantiated two-run claim.** Agent-43 recommends
  that agent-46 (coherence-checker) and the author jointly decide
  whether to retain the two-run claim (and add archive pointer) or
  withdraw it (and report the temperature-sweep $T=0.3$ as the primary
  result).

## 5. Confidence

**HIGH** on the three Case-3 source discrepancies (§4.2.1 / §4.2.2
/ §4.2.3 of the audit). Verified by line-level reading of
`alldata.md` at lines 1385–1533 and 1792–1870 against §6.3 of
`sections/07_case_studies.md`.

**HIGH** that a §15 Reproducibility appendix is required. Without it,
even the protocol-level reproducibility for §5 and §6.1 / §6.2 is
blocked on missing hashes and seeds.

**HIGH** that items 1–6 of audit §9 (CRITICAL) move the paper from
"conditionally reproducible" to "reproducible at disclosed scope".

**MEDIUM** that I caught every reproducibility issue. I did NOT
re-run the pipeline or compute fresh numbers; my audit is
specification-vs-source + ten-item-checklist based.

**HIGH** that my scrutiny list for agent-44 (handoff) is complete
for APA / citation validation of repro-appendix-adjacent content.

## 6. Memory Storage Protocol (N/A per constraints)

The constraints file rules `npx claude-flow` hooks are ignored;
file-based memory only. All four output files are on disk per the
agent instructions.

## 7. Recommendations to author / coordinator

**CRITICAL (arXiv v1 blockers)**:
1. Resolve §6.3.4 Table 2 source mismatch — either re-label rows or
   re-run ablation.
2. Resolve §6.3.3 STOP-3 two-run claim — substantiate or withdraw.
3. Resolve §6.3.2 N=14 ambiguity.
4. Draft §15 Reproducibility appendix from patterns file.
5. Draft §Ethics statement per IC-6 three-tier release.
6. Add IC-6 labels to §6.1.1 / §6.2.1 / §6.3.1.

**HIGH (strongly recommended)**:
7. Add $\lambda_{\text{voice}}$ / $\lambda_{\text{emotion}}$ to §5.6.
8. Define `qwen3-tts-foricl-v1` in the appendix.
9. State open-source status of Context Graph and ClipCannon.
10. List the 9 STYLE_CENTROID embedders in §6.1.2.
11. State GPU / wall-clock / CUDA in §15.
12. Pin bootstrap library version in §6.3.2.

Total estimated fix time: **6–8 hours** for CRITICAL items, **2
hours** for HIGH items.

## 8. Changelog

- 2026-04-17 — Final report by agent-43-reproducibility-checker.
