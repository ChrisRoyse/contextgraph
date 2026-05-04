# 19 - One-Paper Drafting, Figures, And Supplementary Materials

## Goal

Draft **one comprehensive paper** that covers the entire monolith:
DDA + Meaning Compression + ME-JEPA + FSV + TCT + Constellation Guard
+ the integrated runtime as measured on the unified codebase. Replaces
the prior three-paper roadmap. The paper is the v1.0 monolith's
academic deliverable; everything in smokedocs 14-35 ultimately serves
this one document and its archived artifact.

## Why One Paper, Not Three

A single paper has:

- **One citation gravity**: every reader hits the synthesis at once.
- **One DOI**: simpler artifact + Zenodo + ACM badging story.
- **One submission cycle**: aligns with one venue calendar; no
  cross-paper revision drift.
- **One brand anchor**: future work cites "the monolith paper", not
  three sub-papers.

The 9-page-main-body limit (NeurIPS 2026 / MLSys / ICLR) is achievable
because the paper is **systems-first**: the empirical anchors (Case 3
voice 0.961 SECS, Case 1 Shakespeare 5/5 FSV, ME-JEPA paper-full 50K
evidence, MI audit numbers) are concise; the architecture is the bulk.
Detailed measurements + ablations live in the appendix (no page
limit).

## Working Title

```text
Auditable Identity-Locked AI on a Single RTX 5090:
Meaning Compression, Source-of-Truth Verification, and Constellation Guard
on an Integrated 437,000-Line Stack
```

The 437K LOC number = ~370K Context Graph + ~67K ClipCannon (pulled
from PAPER.md and `docs2/alldata.md` accounting). Update at
submission time with the actual unified count.

## Venue Targeting

Primary: **NeurIPS Datasets & Benchmarks** track (the contribution is
benchmark + runtime + verifiability protocol). Secondary: **MLSys**
(systems framing). Tertiary: **ICLR** with substantial ablations.

The paper must satisfy NeurIPS Paper Checklist (mandatory; desk reject
if missing). 9-page main body + appendix; checklist does not count
toward page limit.

## Section Structure (9 Pages Main Body)

### Abstract (≤200 words)

Lead: "single-system auditable, identity-locked, meaning-compressed AI
infrastructure on commodity hardware." State three constructs (DDA,
TCT, FSV) and the headline numbers: Case 3 voice SECS, Case 1
Shakespeare 5/5, ME-JEPA paper-full top-k acc, MI audit
$k_{\text{eff}}$, Constellation Guard rejection rate. State the DOI
in the abstract footer.

### 1. Introduction (~1 page)

- The data wall + Shumailov collapse + auditability gap + alignment-
  verifiability gap, framed as **one** problem with **one** answer.
- The integrated stack as the answer: same frozen embedder panel
  serves as retriever + labeller + runtime guard, with FSV at every
  step and ME-JEPA as the predictor on top.
- Contributions list (5-6 items), all from the same monolith.

### 2. Related Work (~0.75 page)

- JEPA family + collapse mitigations.
- Career trajectory benchmarks (O*NET, ESCO).
- Compression-as-intelligence + auditable ML systems.
- Scalar-reward alignment as the comparison anchor for §5
  per-output verifiability.

Tighter than the three-paper structure would have been; this is the
trade-off.

### 3. The Monolith System (~1.5 pages)

This is the systems half. One architecture diagram (F1).

- Domain packs as the universal lever.
- Typed event ingest path.
- DDA substrate (13 frozen text embedders + 7 video modalities).
- ClipCannon 23-stage pipeline as the video specialization.
- ME-JEPA panel + trainer + artifact registry.
- Constellation Guard predicate at inference.
- FSV bundle schema as the output contract.

### 4. DDA + Meaning Compression (~1 page)

- Counting identity (PAPER.md §3.1).
- Up-to 91 signals per input at $N=13$; up-to 28 per video clip at
  $M=7$.
- Mutual-information audit (smokedoc 30) result: measured
  $k_{\text{eff}}$ on three corpora.
- Fourth-category positioning vs bit / weight / activation
  compression.

### 5. TCT + Constellation Guard (~1 page)

- Three-phase TCT method (PAPER.md §5).
- Per-output guard predicate $G_\tau$.
- Per-output verifiability vs scalar-reward alignment
  (InstructGPT / DPO / Constitutional AI) — pool-relative, not a
  benchmark contest.

### 6. ME-JEPA Career Benchmark (~0.75 page)

- Public taxonomy benchmark grounded in O*NET (+ optional ESCO
  crosswalk per smokedoc 18).
- Event / state / action / outcome schema (verbatim from smokedoc 05).
- Why public-taxonomy benchmark, not recruiting model.
- Walk generator + per-edge hash.

### 7. Experiments + Results (~2 pages)

Six measured anchors:

1. Case 3 voice SECS multi-speaker (smokedoc 32 in old plan; folded
   in here): mean ± BCa CI, with cross-encoder triangulation.
2. Case 1 Shakespeare LoRA: 5/5 FSV pass + emergent zero-shot
   cross-lingual transfer (PAPER.md).
3. Case 2 talking-head (after smokedoc-deleted-33 fix): SECS / FVD /
   LSE numbers.
4. ME-JEPA paper-full 50K headline: prediction metrics, top-k acc,
   skill reconstruction, all multi-seed BCa CI.
5. ESCO crosswalk replication: parity table.
6. Constellation Guard rejection rate per case.

Auto-generated from `paper_tables/`. Hand-typed numbers forbidden.

### 8. Discussion + Limitations (~0.75 page)

- What the monolith proves.
- What is bounded: public-taxonomy benchmark, not real recruiting;
  single hardware platform; single sample-complexity operating
  point on Case 3.
- Where Phase 12 (smokedoc 25) is required for real subject-level
  data.

### 9. Reproducibility (~0.25 page)

Hardware, software, dataset hashes, container DOI, command log,
reproduce script. One paragraph; everything traceable to artifact
DOI.

## Appendices (No Page Limit)

- A — Domain pack specs (verbatim TOML excerpts).
- B — Full bundle schema (smokedoc 29).
- C — Failed runs and root-cause diagnostics (worked example: seed-43
  ablation failure).
- D — HP sweeps + collapse diagnostics (smokedoc 22).
- E — ESCO replication + crosswalk decoder (smokedoc 18).
- F — Compliance mapping: NIST AI RMF + EU AI Act + NeurIPS Checklist
  (smokedoc 21).
- G — Cross-encoder SECS triangulation results.
- H — Pairwise mutual-information audit per (corpus, embedder pair).
- I — Per-output Constellation Guard cosine distributions.
- J — Multi-step rollout compounding analysis (if smokedoc 24
  optional work is included).

## Figure Pipeline

Generated from bundles, not hand-drawn except F1.

| Figure | Source | Generator |
| --- | --- | --- |
| F1 monolith architecture | hand-drawn (TikZ) | `figures/architecture.tex` |
| F2 val cosine band, multi-seed | `paper_tables/training_metrics.json` | `scripts/figures/plot_val_cosine_band.py` |
| F3 ablation forest | aggregate paired tests | `scripts/figures/plot_ablation_forest.py` |
| F4 walk graph sample | bundle walk CSV | `scripts/figures/plot_walk_sample.py` |
| F5 collapse diagnostics (rank, A+U scatter) | sweep bundle | `scripts/figures/plot_collapse.py` |
| F6 evidence diagram (CFs + bundle layout) | bundle structure | `scripts/figures/plot_evidence_diagram.py` |
| F7 SECS distribution (cross-encoder) | voice eval bundle | `scripts/figures/plot_secs.py` |
| F8 Constellation Guard cosine histogram | guard decision parquet | `scripts/figures/plot_guard_hist.py` |

All generators take bundle path as their only argument; deterministic
output; matplotlib version locked.

## Table Pipeline

LaTeX `\input{}` from CSVs through `csvsimple` or `pgfplotstable`.
The Makefile target `make tables` regenerates all `.tex` table fragments
from the bundle CSVs. Hand-edited table files in the paper repo are
forbidden. CI step (smokedoc 23) runs `make tables` and fails if
`git diff` is non-empty after — catching any drift between bundle and
paper.

## Supplementary Materials

The supplementary archive (one ZIP, archived to Zenodo with the same
DOI as the paper):

- full bundle directory for the v1.0 headline run;
- aggregate bundle directory (multi-seed);
- Apptainer SIF + Docker image tarball;
- LaTeX source of the paper;
- generated table CSVs;
- README with `reproduce.sh paper_full_50k --seeds 42,43,44,45,46`;
- video walkthrough of the monolith (5-10 min) for reviewer
  on-ramping.

## Writing Schedule

| Week | Activity |
| --- | --- |
| 1-2 | Smokedocs 14-35 sub-tasks complete; tables auto-generate |
| 3-4 | Sections 3-7 drafted from finalized bundle evidence |
| 5 | Sections 1, 2, 8, 9 drafted; appendices populated |
| 6 | Internal review; figure polish; checklist completion |
| 7 | Final pass; arXiv submission; Zenodo DOI; venue submission |

7 weeks of focused writing assuming all smokedoc-14-35 sub-tasks are
complete. Realistic given multi-seed + decoder + HP sweeps + paper-full
+ ESCO + audit are all parallelizable on the integrated runtime.

## Anti-Patterns

- Hand-typed numbers anywhere in the manuscript.
- Architecture-first abstract.
- Claims about "predicts careers" without taxonomy qualifier.
- Ablations without paired tests + BCa CI.
- Top-k accuracy without ECE.
- Reproducibility section pointing to GitHub instead of DOI.
- Section 7 reading as a list of numbers without per-anchor narrative.
- Splitting the work back into multiple papers under reviewer
  pressure.

## Acceptance

This work is done when:

- the paper repo (`paper/`) exists and `make all` regenerates
  tables/figures from the headline + aggregate bundles deterministically;
- arXiv preprint is uploaded with Zenodo DOI cross-link;
- venue submission packet is prepared (PDF + supplementary +
  checklist);
- a future reader can hit one DOI and reach the entire monolith story.
