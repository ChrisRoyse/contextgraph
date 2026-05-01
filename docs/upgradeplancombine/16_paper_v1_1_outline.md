# 16 — Research Paper v1.1 Outline (Update from v1.0)

This document specifies how the v1 paper (`docs/5090jepa/researchpaper/`)
gets updated for v2.0 release. The intent is a *revision*, not a
new manuscript. The v1.0 contributions remain valid; the v1.1
revision adds the combined-runtime contributions on top.

## 1. Title update

Working title for v1.1:

```text
Dynamic / ME-JEPA: An Audited, Domain-Portable World-Model Runtime
on a Single RTX 5090
— with Meaning-Compressed Audited Execution Traces and a Frozen-
   Constellation Runtime Guard
```

(Subtitle change reflects the v2.0 contributions.)

## 2. Section-by-section diff

### Abstract

- New sentence (insert after action-contrast accuracy line):
  "The combined runtime additionally treats every persisted bundle
  as a meaning-compressed audited execution trace, with a measured
  meaning-compression ratio of XX (career\_taxonomy A1, $n_{\text{eff,strict}}
  = X.X$ from a KSG-1 pairwise mutual-information audit) and a
  frozen-constellation runtime guard $G_\tau$ AND-ed with the
  utility verdict on every plan candidate."
- Existing scope discipline preserved.

### §1 Introduction

- Contribution #4 is rewritten to incorporate meaning compression:
  "**A new training-data class with a measured meaning-compression
  ratio.** Each immutable bundle is structurally an *audited
  execution trace with cryptographic provenance*; the v2.0 runtime
  adds (i) per-event $\binom{N_d}{2}$ pairwise-instrument features
  raising the constructive Slice-1 yield from $2N_d + 4$ to
  $N_d^2 + N_d + 4$ structured signals per event; (ii) a per-bundle
  meaning-compression ratio $\mathrm{MC}(\Phi, D)$ reported as a
  paper-table column; (iii) a KSG pairwise mutual-information audit
  reporting an effective signal count $n_{\text{eff}}$ that
  empirically replaces the upper-bound ‘up to' headline."

- Contribution #6 is unchanged (release artifact); the v2.0 version
  number is updated.

### §2 Related Work

- §2.6 (Synthetic Data, Model Collapse, Process Supervision) is
  unchanged; the line `§7 extends this taxonomy with a class of
  training data that the ME-JEPA runtime produces by construction:
  *audited execution traces with cryptographic provenance*` is
  retained verbatim.
- New paragraph at end of §2.6: "PAPER.md (Royse, 2026) names this
  class as a fourth taxonomic compression entry — *meaning
  compression* — measured in structured signals per raw input.
  Section 7 instantiates the class on the runtime's frozen
  instrument panel; Section 8.7 reports the measured pairwise
  mutual-information audit."

### §3 The Dynamic / ME-JEPA Runtime

- §3.2 (Storage) is updated to say "28 typed RocksDB column
  families" and to add the three new CFs to the table.
- §3.3 (Multi-Instrument Latent Panel) is extended:
  "Pairwise instrument features $\rho_{j,k}(\phi_j(e), \phi_k(e))$
  for $1 \le j < k \le N_d$ are persisted as first-class records
  in `dj_pairwise_readings`. The panel record references both the
  per-instrument readings and the pairwise readings."
- §3.4 (Predictor and Objective) is unchanged for v2.0.
- New §3.6 — *The Combined Runtime Guard $G_\tau$*: specifies the
  utility AND $G_\tau$ predicate per the §6 design doc.
- New §3.7 — *The Constellation Set $C_P$*: specifies $C_P$
  construction per the §4 design doc.
- §3.8 (Subcommand Surface) updated to "36 subcommands" with the
  six new ones added in a new column of the table.
- §3.9 (Hardware) unchanged.

### §4 The `career_taxonomy.v1` Public-Taxonomy Benchmark

- Section title bumped to `career_taxonomy.v2`.
- §4.3 (Schema) is unchanged.
- §4.4 (Splits, Provenance, Edge Cases) — edge-case row count rises
  from 11 to 26.
- New §4.5 — *Constellation Subjects*: documents the `global` plus
  4 cluster-specific constellations declared by the v2 TOML.
- §4.6 (Reusability) — paragraph notes that v2 introduces the
  pairwise + constellation features; future domain packs benefit
  from the same machinery without code changes.

### §5 Experiments

- §5.1 (Runtime Universality) updated: "the same code path runs
  three domains *with constellation-guarded planning*."
- §5.2 (Career Next-State Prediction) — headline metrics unchanged
  for v2.0 (predictor architecture is unchanged); table 1 adds the
  v2 measured MC ratio column.
- §5.3 (Action-Conditioned Planning) — pass criterion updated:
  "Pass requires (a) the five guard decisions are not all identical;
  (b) the selected action is the highest-utility *accepted*
  candidate (combined utility AND $G_\tau$); (c) the model artifact
  hash referenced by every prediction equals the current career
  model artifact's recomputed hash."
- §5.4 (Ablation Matrix) — A0–A5 unchanged; new A6 *runtime guard
  ablation* compares utility-only vs. utility AND $G_\tau$ on the
  same predictor.
- §5.5 (Auditability and Failure Behavior) — edge-case count rises
  to 29; FSV-9..12 added to the harness list.
- §5.6 (Single-GPU Feasibility) — wall time updated; release-D
  budget noted.
- §5.7 (Deferred Axes) — pairwise-features-as-predictor-input,
  Phase-2 conditioning, and cross-domain transfer all listed as
  v2.1+ axes.

### §6 Full-State Verification

- §6.1 (FSV Rule) — unchanged.
- §6.2 (FSV Harnesses) — list extends from 8 to 12 harnesses;
  per-harness summary added for FSV-9..12 from the §9 design doc.
- §6.3 (Edge-Case Catalog) — count rises to 29.
- §6.4 (Read-Only Inspection Hardening) — unchanged.
- §6.5 (Hash Reference for the Bundle) — extended for v2.

### §7 Bundles as a Training-Data Class

This is the section that grows the most in v1.1.

- §7.1 (Synthetic-Data Taxonomy) — unchanged.
- §7.2 (Anatomy of a Bundle) — extended to incorporate Slice-1
  pairwise readings and Slice-3 constellation-integrity records.
  Per-event Slice-1 yield is the paper's new headline number.
- §7.3 (Properties Relative to Existing Classes) — table updated
  to reflect v2.0's higher signal-density-per-raw-input.
- §7.4 (Collapse-Hazard Analysis) — strengthened: the Slice-1
  share grows from $\sim 60$ % to $\sim 78$ %, *improving* the
  collapse-hazard ratio.
- §7.5 (Useful For) — same 7 use cases; Reasoning #2 (process-
  supervised reasoning) gains the constellation-guard verdict as
  a per-step label.
- New §7.6 — *Meaning-Compression Ratio in Practice*: reports
  measured MC for each domain (counter\_world, gridworld\_5x5,
  career\_taxonomy A0–A5).
- New §7.7 — *Pairwise Mutual-Information Audit*: reports the
  KSG-1 measurement, $n_{\text{eff,strict}}$ per domain, and the
  per-domain Axiom-2 verdict.
- §7.8 (Naming the Class) — unchanged.
- §7.9 (Recommendations for Downstream Training) — unchanged.

### §8 Results

This is the section that gets the new tables and figures.

- §8.1 (Headline Multi-Seed Prediction Metrics) — predictor cosines
  unchanged from v1; tables 5 and 6 reproduce the v1 numbers as a
  baseline.
- New §8.2 — *Meaning-Compression Ratio per Domain* (Table 7
  reproduces `paper_tables/table_mc_ratio.csv`).
- New §8.3 — *Pairwise Mutual-Information Summary* (Table 8
  reproduces `paper_tables/table_pairwise_mi_summary.csv`; Figure 1
  is the heatmap).
- New §8.4 — *Combined Guard Acceptance* (Figure 2: the
  $G_\tau$-acceptance-curve plot; Figure 3: signal-density
  distribution).
- §8.5 (Multi-Seed Ablation Table) — unchanged numerics.
- §8.6 (Paired Sign-Flip Permutation Tests) — unchanged.
- §8.7 (Domain Runtime Coverage) — extended row counts for the new
  CFs.
- §8.8 (Single-GPU Feasibility) — wall time updated.
- §8.9 (Failed Runs as Evidence) — preserved bundles list updated.

### §9 Reproducibility Artifact

- §9.1 (Reviewer Path) — Docker invocation updated for v2 image.
- §9.2 (Container Strategy) — both v1 and v2 SIFs documented; the
  v2 SIF inherits from a new CUDA-base-pinned-by-digest at v2.0
  build time.
- §9.3 (Build Determinism) — unchanged in protocol; new digests.
- §9.4 (Artifact Layout) — extended for the v2 release-candidate
  archive `mejepa_5090_artifact_v2.0.0_rcN`.
- §9.5 (Reviewer Reproducibility Mapping) — unchanged.

### §10 Discussion

- §10.1 (What Dynamic / ME-JEPA Demonstrates) — last bullet point
  upgraded:
  "**Each bundle is itself a structured training-data artifact.**
  §7 develops the argument [...]. The v2.0 runtime now reports a
  measured meaning-compression ratio per bundle and a measured
  effective signal count $n_{\text{eff}}$ from a KSG pairwise
  mutual-information audit, replacing the v1 'approximately
  independent' hedge with a measurement."
- §10.2 (Failure Modes) — unchanged narrative; v2 deviations
  reported as Phase-by-phase issues encountered during the upgrade
  (`13_implementation_phases.md` §10 fail-soft ordering).

### §11 Limitations

- §11.1 (Modeling) — single-GPU and small-model carry forward; new
  bullet on "constellation guard requires per-domain calibration
  ($\tau$ does not transport)."
- §11.2 (Statistical) — unchanged; v2 measurements use the same
  BCa CI + sign-flip protocol.
- §11.3 (Data) — unchanged; pairwise MI audit is one new
  measurement on the existing public taxonomy benchmark.
- §11.4 (Reproducibility) — v2 release reference is host-specific;
  same caveat as v1.
- §11.5 (Acknowledgments) — adds: "the upgrade integrates the
  Derived Data Abundance counting identity, the meaning-compression
  ratio, and the Teleological Constellation Training runtime guard
  predicate from `docs/PAPER.md`."

## 3. New figure list

Three new figures land in v1.1:

- **Figure 1**: pairwise MI heatmap for career\_taxonomy A1
  (10x10 lower triangle).
- **Figure 2**: signal-density-per-event distribution stacked-bar
  histogram across slices.
- **Figure 3**: $G_\tau$-acceptance curve as a function of
  threshold percentile, per domain.

## 4. New table list

Four new tables land in v1.1:

- **Table 7**: MC ratio per domain (`table_mc_ratio.csv`).
- **Table 8**: Pairwise MI summary
  (`table_pairwise_mi_summary.csv`).
- **Table 9**: Constellation guard acceptance
  (`table_constellation_guard.csv`).
- **Table 10**: Threshold calibration record
  (`table_threshold_calibration.csv`).

Plus extended versions of v1 tables 1–6 with new columns / rows.

## 5. Reference list updates

`docs/5090jepa/researchpaper/references.bib` gains entries for:

- The pairwise MI estimator literature (Kraskov, Stögbauer,
  Grassberger 2004 — the KSG paper; Belghazi et al.\ 2018 — MINE).
- One-line citation of the in-repo `docs/PAPER.md` design notes.

The existing 25 references remain.

## 6. Out-of-paper artefacts

The combined runtime ships:

- v2.0 release-candidate archive
  `mejepa_5090_artifact_v2.0.0_rcN_sif_verified.tar.gz`.
- Updated `paper.tex` and `references.bib` in
  `docs/5090jepa/researchpaper/`.
- Updated v2 paper-tables-reference under
  `tmp/mejepa_release_artifacts/...rcN/paper_tables_reference/`.
- Updated `CITATION.cff` with v2.0 version string.

## 7. The v1 → v1.1 diff stays small

By design:

- Predictor architecture and weights are unchanged.
- Existing prediction metrics (Tables 5 and 6) stay numerically
  identical for the same bundle.
- Existing ablation matrix (A0–A5) preserves all v1 numbers.
- Existing edge-case rows preserve their semantics.
- Existing FSV harnesses (FSV-1 through FSV-8) are untouched.

The diff is a *layer added on top* of v1, not a rewrite. Reviewers
who have read v1 see the upgrade as additive, with the new
contributions clearly delimited.

## 8. arXiv revision discipline

For an arXiv revision (recommended over a new submission):

- v1.1 is uploaded as a new version of the existing arXiv preprint;
  the v1.0 PDF remains in the version history.
- The Zenodo record (DOI `10.5281/zenodo.19953951`) is updated to
  v2.0 metadata via the same edit-mode protocol previously used.
- The v2.0 artifact tarball is uploaded as a new file slot in the
  Zenodo record; the v1 tarball remains for reproducibility of
  v1 bundles.

## 9. Bibliographic novelty claim, restated

PAPER.md's pool-relative novelty argument (no cited paper composes
multi-modal conjunction over $M \ge 1$ frozen encoders with frozen
inference-time targets and per-output acceptance) carries forward.
The v2.0 runtime is the first instance of that composition that:

- emerges from a domain-pack-driven runtime rather than from
  per-modality bespoke pipelines;
- produces a meaning-compressed audited execution trace per run;
- ships a deterministic public-source benchmark for which every
  number is reproducible.

The pool relative to which novelty is claimed is the union of the v1
ME-JEPA paper's 25-reference pool and PAPER.md's 88-citation pool.
v1.1's novelty claim is that no cited paper instantiates the combined
runtime jointly.

## 10. Single-author voice carries forward

Per the v1 paper's enforcement, the v1.1 update remains single-
author (Chris Royse) throughout. No "we"; no co-author markers; the
v1 paper.tex's clean header layout is preserved.
