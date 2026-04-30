# 36 - Three-Paper Joint Roadmap

## Goal

Sequence and cross-cite **three peer-reviewable papers** that together
establish the scientific foundation of the closed-loop architecture:

1. **DDA + Meaning Compression** (PAPER.md, ~ready, draft mode);
2. **ME-JEPA + FSV** (smokedocs 14-25 path, paper-ready in ~6 months);
3. **TCT + Constellation Guard** (Cases 1, 2, 3 fully measured;
   ~3-6 months out).

The three papers are **deliberately separate** (not a single
megapaper) for two reasons: they have different methodological centers
of gravity, and three citation anchors compound brand equity faster
than one.

## Why Three, Not One

| Paper | Center Of Gravity | Venue Fit |
| --- | --- | --- |
| DDA + Meaning Compression | Theoretical / counting identity / scaling | NeurIPS, ICLR, MLSys |
| ME-JEPA + FSV | Systems / runtime / auditability / reproducibility | NeurIPS Datasets & Benchmarks, MLSys, OSDI |
| TCT + Constellation Guard | Method / alignment / generative quality | NeurIPS, ICLR, CVPR (for video case) |

A single mega-paper would dilute every claim. Three separate papers
each occupy a distinct sub-field; cross-cite gravity grows
multiplicatively.

## Per-Paper Status

### Paper 1 — DDA + Meaning Compression (PAPER.md)

**Status**: drafted, ~88-citation locked pool, has Case 3 measured at
0.961 SECS, Case 1 at 5/5 FSV. Cases 2 measurement is projected.

**Path to submission**:

- Measure Case 2 (talking-head) — see doc 33 / sub-tasks below — to
  upgrade Case 2 from "projected" to "measured".
- Run mutual-information embedder audit (doc 30) on at least 3
  corpora; replace the "up to 91 signals" upper-bound headline with a
  measured $k_{\text{eff}}$.
- Add cross-encoder SECS triangulation (doc 38) to lift Case 3 from
  encoder-matched to cross-verified.
- Final pass on §2 Related Work for any post-PAPER.md publications.
- arXiv submit; venue submit (NeurIPS 2026 cycle if timing allows;
  ICLR 2027 otherwise).

**Estimate**: 8-12 weeks to revised manuscript with measurements
locked.

### Paper 2 — ME-JEPA + FSV (smokedocs 14-25)

**Status**: smoke program working through paper-small 1K. Smokedocs
14-25 specify the path to a paper-ready milestone.

**Path to submission**: per smokedocs 19; concretely:

- Multi-seed completion (doc 15);
- Decoder head + top-k (doc 16);
- HP sweeps + collapse diagnostics (doc 22);
- Reproducibility artifact + Zenodo DOI (doc 20);
- Compliance + ethics (doc 21);
- Paper drafting (doc 19) — 6 weeks of focused writing.

**Estimate**: 4-6 months to arXiv preprint with artifact badge.

### Paper 3 — TCT + Constellation Guard

**Status**: described in PAPER.md §5 but currently shares the DDA
manuscript. Should be **split out** as a standalone paper because:

- The TCT method is independent of the DDA counting identity.
- The Constellation Guard predicate is independently citable in
  alignment / RLHF-alternative discussions.
- A standalone paper is a stronger anchor for the agent platform
  (doc 34) and the voice product (doc 31).

**Path to submission**:

- Lift §5 + §6 of PAPER.md into a standalone manuscript.
- Add multi-speaker / multi-language Case 3 measurement (per the
  voice product workstream; doc 31 dependency).
- Add Case 2 (talking-head) measurement — same ablation as PAPER.md.
- Position against InstructGPT, DPO, Constitutional AI on the
  per-output verifiability axis; preference-benchmark contests
  remain explicitly out of scope.
- Submit to a generative-modeling or alignment venue.

**Estimate**: 4-6 months from PAPER.md decision to lift; can run
in parallel with Paper 1 finalization.

## Cross-Citation Graph

```text
Paper 1 (DDA + MC)  ─────cites─────►  Paper 2 (ME-JEPA + FSV)  [DDA→panel bridge, doc 27]
Paper 1 (DDA + MC)  ─────cites─────►  Paper 3 (TCT + CG)         [shared embedder panel, frozen geometry]
Paper 2 (ME-JEPA + FSV) ──cites────►  Paper 1 (DDA + MC)         [signals as instrument readings]
Paper 2 (ME-JEPA + FSV) ──cites────►  Paper 3 (TCT + CG)         [Constellation Guard for ME-JEPA outputs, doc 28]
Paper 3 (TCT + CG)  ─────cites─────►  Paper 1 (DDA + MC)         [centroid set built from DDA panel]
Paper 3 (TCT + CG)  ─────cites─────►  Paper 2 (ME-JEPA + FSV)    [example domain: career taxonomy w/ FSV]
```

Every cross-cite is real and load-bearing, not ceremonial. The reader
moving through the citation graph **must** see all three to understand
any one fully — that is the point.

## Submission Sequencing

| Month | Action |
| --- | --- |
| M1-M2 | Finalize PAPER.md measurements (Case 2, MI audit, cross-encoder); arXiv submit |
| M3-M4 | Split TCT paper from PAPER.md; finalize multi-speaker Case 3; arXiv submit |
| M5-M7 | ME-JEPA paper drafting against finalized smokedocs evidence |
| M8 | Paper 2 arXiv submit + ME-JEPA Zenodo DOI |
| M3+ | Venue submissions per individual paper deadlines |

The DDA paper goes first because it is closest to ready and locks
the naming ("Derived Data Abundance", "Meaning Compression",
"Teleological Constellation Training"). Once those names are on arXiv
they are the canonical references for the other two papers.

## Reproducibility Discipline (All Three)

Each paper requires:

- public DOI (Zenodo for code + bundle);
- container image (Apptainer + Docker; doc 20);
- one-command reproduce script (doc 20);
- ACM Available + Functional + Reusable badging packet (doc 20);
- NeurIPS reproducibility checklist filled to bundle paths (doc 20);
- shared CI (doc 23) so a tagged release reproduces deterministically.

Three papers, three Zenodo DOIs, one shared CI. The shared CI is the
discipline that prevents drift across papers — if Paper 2's claim
references a number from Paper 1's bundle, the CI verifies they still
match at every release.

## Citation Strategy

- **Self-cite** carefully: each paper acknowledges the others, but the
  abstracts stand alone. A reviewer reading any one should not need
  the others to evaluate that paper's claims.
- **Cross-publish** code: every paper links to a shared GitHub org
  (Teleox.ai or Leapable.ai) with one repo per paper plus a
  meta-repo (`teleox-stack`) that links them.
- **Conference talk strategy**: speak about the **stack** as a whole
  (the closed loop, doc 26), not just the paper, in venue talks.
  This keeps the brand at the synthesis level even when individual
  papers are at sub-fields.

## Risks

- **Reviewer "this is three papers in one"**: forestalled by the
  three-paper structure. Reviewers of any single paper should not
  feel the boundaries are artificial.
- **Cross-paper dependencies break**: if Paper 1 is rejected and a
  major revision changes the DDA construct names, Paper 2 and Paper 3
  inherit drift. Mitigation: lock construct names in a `terminology.md`
  file shared across papers; any change requires updating all three.
- **Venue timing**: NeurIPS deadline (May), ICLR (Sept/Oct), MLSys
  (Oct), CVPR (Nov), KDD (Feb). Build a calendar and slot each paper.

## Coordination With Other Docs

- Doc 19 (paper drafting): operationalizes Paper 2.
- Doc 20 (reproducibility): the shared artifact discipline.
- Doc 30 (MI audit): unblocks Paper 1's empirical claim.
- Doc 38 (cross-encoder triangulation): unblocks Paper 1's voice claim.
- Doc 33 (Case 2 completion): unblocks Paper 1 and Paper 3 talking-head
  measurement.
- Doc 32 (multi-speaker Case 3): improves Paper 1 and Paper 3.

## Acceptance

The three-paper roadmap is **complete** when:

- all three arXiv preprints are live with Zenodo DOIs;
- at least one is venue-accepted (or under invited revision);
- citation graph is symmetric (each paper cites each other);
- shared CI verifies every claim is still recomputable.
