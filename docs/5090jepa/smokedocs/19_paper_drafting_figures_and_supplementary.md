# 19 - Paper Drafting, Figures, And Supplementary Materials

## Goal

Turn smokedoc 10's section outline into a concrete, evidence-backed manuscript
plan. The paper is a systems paper with an empirical demonstration; the
narrative spine is auditability + single-GPU feasibility, with the
career-taxonomy benchmark as the empirical anchor.

## Venue Targeting

Primary target: **arXiv preprint** + **NeurIPS Datasets & Benchmarks** track or
**MLSys**. NeurIPS D&B is well-aligned because the contribution is a benchmark
+ runtime, not a new model. MLSys is well-aligned because of the systems +
single-GPU framing.

Secondary target: workshop submission to a **reproducibility-focused** venue
(e.g. NeurIPS Reproducibility track, ML Reproducibility Challenge) that
specifically values the FSV protocol.

The paper must satisfy NeurIPS Paper Checklist (mandatory, included in the
LaTeX style file, papers without it are desk-rejected). 9-page main body +
appendix; checklist does not count toward page limit.

## Section-By-Section Drafting Plan

Word counts are targets, not contracts.

### Abstract (≤200 words)

Lead: "single-GPU auditable latent world-model runtime with a career-taxonomy
benchmark." State the three claims (universality, auditability,
single-RTX-5090). Cite the headline number from the multi-seed aggregate
(BCa CI). State the artifact DOI in the abstract footer.

Anti-pattern: do not lead with the model architecture. Lead with the systems
contribution.

### 1. Introduction (~1.5 pages)

- Para 1: latent prediction over reconstruction, why JEPA family matters.
- Para 2: the auditability gap — most JEPA work reports metrics without
  source-of-truth evidence.
- Para 3: the career-domain motivation — why an auditable benchmark matters
  for any work that touches occupations.
- Para 4: single-GPU feasibility — democratizing the experimental setup.
- Para 5: contributions, ending with "all evidence in this paper is
  recoverable from a Zenodo-archived bundle".

### 2. Related Work (~1 page)

Three subsections:

- **JEPA family**: I-JEPA, V-JEPA, V-JEPA 2, V-JEPA 2.1, T-JEPA, LeJEPA, and
  Apple's Rethinking-JEPA frozen-teacher work. Cite VICReg as collapse
  prevention, and SIGReg / LeJEPA as alternatives we did not adopt and why.
- **Career trajectory datasets**: O*NET, ESCO, Karrierewege, JobHop,
  OpenResume. Position our benchmark as **complementary**, not competitive —
  ours is an auditable taxonomy benchmark, not a trajectory prediction
  dataset.
- **Auditability and reproducibility in ML systems**: NIST AI RMF, ACM
  artifact-evaluation, NeurIPS reproducibility checklist, MLflow patterns.

### 3. System (~2 pages)

This is the systems half of the paper. Write to figure F1 (architecture
diagram).

- Domain packs as the universality lever.
- Typed event ingest path.
- Multi-instrument latent panels.
- Trajectory + dataset compiler.
- JEPA trainer + artifact registry + recomputed file hashes.
- Prediction / planning / surprise events.
- **Full State Verification protocol** — explain why every claim in §6 has a
  bundle path.

### 4. Career Taxonomy Benchmark (~1.5 pages)

- Data sources and provenance (O*NET, optionally ESCO).
- Event / state / action / outcome schema (cite smokedoc 05 verbatim).
- Why "public taxonomy benchmark" not "recruiting model" (cite smokedoc 01).
- Walk generator + per-edge hash.
- Splits and leakage policy.

### 5. Experiments (~1 page)

Concise listing tied to smokedoc 06:

- Experiment A — Runtime universality (counter_world, gridworld, career).
- Experiment B — Career next-state prediction.
- Experiment C — Action-conditioned planning.
- Experiment D — A0-A5 ablations.
- Experiment E — Auditability / failure stress.
- Experiment F — 5090 feasibility.

Tables named here, not pasted. Pasted into §6.

### 6. Results (~2 pages)

Generated from `paper_tables/` only. Manual numbers are forbidden. Six tables:

1. T1 Domain Runtime Coverage.
2. T2 Prediction Metrics (multi-seed, BCa CI).
3. T3 Career Ablations (multi-seed, paired tests).
4. T4 Auditability And Failure Behavior.
5. T5 Single-GPU Feasibility.
6. T6 Top-k Occupation + Skill Reconstruction (from doc 16).

Three figures:

- F1 System architecture diagram (already named).
- F2 Validation cosine across epochs, multi-seed band.
- F3 Ablation forest plot (paired delta + BCa CI per ablation row).

### 7. Discussion (~1 page)

What the system proves, what it does not. Failure modes (use the failed
seed-43 bundle as a worked example of fail-closed behavior). Generalization
ceilings.

### 8. Limitations (~0.5 pages)

- Public-taxonomy benchmark is not real hiring outcome prediction.
- Small models are demonstration-scale.
- Single hardware platform (Blackwell sm_120).
- 250K tier may be single-seed only.
- Privacy/legal gates depend on Phase 12 (doc 25), not on this paper.

### 9. Reproducibility (~0.5 pages)

Hardware, software, data sources and hashes, command log, evidence bundle
schema, container image, Zenodo DOI. One paragraph per item; everything
traceable to docs 09 and 20.

### Appendices

- A — Domain pack specs (verbatim TOML excerpts).
- B — Full State Verification evidence summary.
- C — Failed runs and root-cause diagnostics (worked example: seed-43).
- D — Hyperparameter sweeps and collapse diagnostics (doc 22).
- E — ESCO replication and crosswalk (doc 18).
- F — NIST AI RMF + NeurIPS Checklist mapping (doc 21, 20).

## Figure Pipeline

Figures are generated, not hand-drawn except F1.

| Figure | Source | Generator |
| --- | --- | --- |
| F1 architecture | hand-drawn (TikZ or SVG) | `figures/architecture.tex` |
| F2 val cosine band | `paper_tables/training_metrics.json` per seed | `scripts/figures/plot_val_cosine_band.py` |
| F3 ablation forest | `aggregate_*/paper_tables/table_paired_ablation_tests.csv` | `scripts/figures/plot_ablation_forest.py` |
| F4 walk graph sample | bundle's walk CSV + occupation labels | `scripts/figures/plot_walk_sample.py` |
| F5 collapse diagnostics | `paper_tables/collapse_diagnostics.csv` (doc 22) | `scripts/figures/plot_collapse.py` |
| F6 evidence diagram | bundle layout + CF list | `scripts/figures/plot_evidence_diagram.py` |

All generators take a bundle path or aggregate path as their only argument and
emit deterministic output (set Python random seed; lock matplotlib version).

## Table Pipeline

LaTeX `\input{}` from CSVs through `csvsimple` or `pgfplotstable`. The Makefile
target `make tables` regenerates all `.tex` table fragments from the bundle
CSVs. Hand-edited table files in the paper repo are forbidden.

```text
paper/
  tables/
    auto/
      table_prediction_metrics.tex
      table_ablation.tex
      table_topk.tex
      ...
    Makefile
    generator.py
```

The CI step runs `make tables` and fails if `git diff` is non-empty after,
catching any drift between bundle and paper.

## Supplementary Materials

The supplementary archive (one ZIP):

- full bundle directory for the headline run;
- aggregate bundle directory;
- container image (squashfs `.sif` or Docker image tarball, also DOI-archived);
- LaTeX source of the paper;
- generated table CSVs (already in bundles, included for reviewer convenience);
- README with one-command reproduction script (`make reproduce`).

The supplementary is also archived to Zenodo with its own DOI; the paper cites
both the code DOI and the supplementary DOI.

## Writing Schedule

| Week | Activity |
| --- | --- |
| 1 | Doc 22 (collapse diagnostics) + multi-seed (doc 15) finalized; tables auto-generate |
| 2 | Doc 16 (decoder head) finalized; T6 added; figures F2/F3 generated |
| 3 | Doc 18 (ESCO) finalized if path holds; appendix E drafted |
| 4 | First full draft (sections 1-9 + appendices A-F) |
| 5 | Internal review; figure polish; checklist completion |
| 6 | Final pass; arXiv submission; Zenodo DOI; venue submission |

This is the budget for a focused single-author paper. With co-authors, parallel
drafting of §3 and §6 saves ~1 week.

## Anti-Patterns To Avoid

- Hand-typed numbers anywhere in the manuscript.
- Claims about "the model predicts careers" without taxonomy qualifier.
- Architecture-first framing in the abstract.
- Ablations reported without paired tests.
- Top-k accuracy without ECE.
- Plots without bundle path in the caption.
- Reproducibility section that points to GitHub instead of a DOI.

## Acceptance

This work is done when:

- the paper repo (`paper/`) exists and Makefile regenerates tables/figures from
  the headline bundle and aggregate bundle;
- arXiv preprint is uploaded with a Zenodo DOI cross-link;
- venue submission packet (PDF + supplementary + checklist) is prepared.
