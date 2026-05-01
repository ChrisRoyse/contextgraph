# EXP-2 — Pairwise Mutual Information Audit on the 14-Embedder Panel

## Single-Sentence Goal

Replace the paper's headline "**up to 91 structured signals per input**" with an **empirically measured $N_{\mathrm{eff}}$** across all $\binom{14}{2} = 91$ embedder pairs of the Context Graph production panel.

## Why This Is The Highest-Leverage Single Experiment

1. It tightens the load-bearing claim of the DDA framework (Proposition P1, Section 3.1 of the paper).
2. It is the precondition for credible domain forks (genomics, drug discovery, materials).
3. It produces deliverables (heatmap, $N_{\mathrm{eff}}$ number) that are visually publishable, paper-ready, and YouTube-ready in one cycle.
4. It is fully under your control — no IRB, no consent process, no external partnership.
5. Estimated runtime: **8–14 calendar days end-to-end**, ~5 hours wall-clock compute on RTX 5090.

## Headline Deliverables

| Deliverable | Format | Audience |
|---|---|---|
| Redundancy heatmap | PNG + PDF, 14×14, log-scale colormap | Paper figure, video, blog |
| $N_{\mathrm{eff}}$ number | Single scalar with bootstrap CI | Paper headline replacement |
| Per-pair MI table | CSV, 91 rows × {MINE, KSG, redundancy, recommendation} | Paper appendix |
| Spectrum effective-rank plot | PNG + PDF | Paper figure |
| Embedder-swap recommendation | Markdown | Internal action; future panel design |
| Paper §3.1 + §6.1 patch | Markdown drop-in | arXiv revision |
| YouTube script | Markdown, 8–12 min | Channel |
| Blog post | Markdown, 1500–2500 words | teleox.ai blog |
| Reproducible Python harness | `scripts/exp2/` | GitHub release |

## Success Criteria

The experiment is **done** when all of the following are true:

- [x] All 91 pair MI estimates have both MINE/InfoNCE *and* partitioned-KSG values with bootstrap 95% CIs.
- [x] $N_{\mathrm{eff}}$ scalar reported with method (one of: redundancy-adjusted, spectrum-entropy, Schur-complement) and bootstrap CI.
- [x] Redundancy heatmap rendered with all 14 embedder labels.
- [x] At least one cross-validation: MINE and KSG agree to within ±0.1 nats on ≥80% of pairs, OR documented disagreement with rationale.
- [x] FSV pass: full pipeline replays with frozen seed → identical numbers within fp32 precision.
- [x] Paper §3.1 and §6.1 redrafted with the measured numbers.
- [x] YouTube video published.
- [x] arXiv revision submitted.

## Doc Map

| # | Doc | Purpose |
|---|---|---|
| 00 | [README](00_README.md) | This document |
| 01 | [Constitution](01_constitution.md) | Immutable rules. Read before touching anything else |
| 02 | [PRD](02_prd.md) | What we are building, why, and for whom |
| 03 | [Methodology](03_methodology.md) | MI estimation methods (InfoNCE, partitioned-KSG), bootstrap, effective-rank computation. Math + code references |
| 04 | [Data Corpus](04_data_corpus.md) | Stratified sampling design across 8 corpora, 800 inputs, validation rules |
| 05 | [Technical Spec](05_technical_spec.md) | File layout, dependencies, architecture, I/O contracts |
| 06 | [Atomic Tasks](06_atomic_tasks.md) | Level-4 task list with dependencies, inputs, outputs, definitions of done |
| 07 | [Validation Harness](07_validation_harness.md) | FSV protocol, replay tests, sanity checks |
| 08 | [Output Deliverables](08_output_deliverables.md) | Detailed spec for each artefact above |
| 09 | [Distribution Plan](09_distribution_plan.md) | arXiv revision, YouTube, blog, tweet thread |

## Quick Start (For The Operator)

```bash
# 1. Read the constitution.
cat docs/exp2/01_constitution.md

# 2. Set up the workspace.
mkdir -p scripts/exp2 && cd scripts/exp2
python -m venv .venv && source .venv/bin/activate
pip install -r requirements.txt    # see 05_technical_spec.md §3

# 3. Verify the substrate.
python verify_environment.py        # task T-001

# 4. Run the pipeline.
python run_exp2.py --config exp2.yml

# 5. FSV.
python fsv.py --replay artefacts/run_$(date +%Y%m%d).json
```

The `run_exp2.py` orchestrator runs every task from [06_atomic_tasks.md](06_atomic_tasks.md) in dependency order. Each task is independently replayable.

## Estimated Calendar

| Day | Task block |
|-----|-----------|
| 1 | T-001 to T-004: environment, corpus stratification |
| 2-3 | T-005 to T-008: embedding extraction + persistence |
| 4-5 | T-009 to T-011: InfoNCE/MINE estimator implementation + per-pair runs |
| 6-7 | T-012 to T-014: KSG estimator + cross-validation |
| 8 | T-015 to T-017: effective-rank methods + bootstrap |
| 9-10 | T-018 to T-020: visualization, writeup |
| 11 | T-021 to T-023: FSV, replay test, paper patch |
| 12-14 | T-024 to T-026: YouTube video, blog, arXiv submit |

## Compute Budget

- ~5 hours wall-clock GPU compute on RTX 5090 (training 91 InfoNCE networks + KSG).
- ~30 GB VRAM peak (parallel network training).
- ~10 GB disk for artefacts.
- ~$0 incremental cost (your existing hardware).

## Risk Score

**LOW.** All primitives required already exist in your stack. The novel work is one Python script (~1500 LOC) + the methodology choice. Failure modes are well-understood (see [07_validation_harness.md](07_validation_harness.md) §4).

## What This Doesn't Cover

- Cross-encoder triangulation (EXP-1 in your paper) — separate experiment.
- $n_m$ sample-complexity sweep (EXP-3) — depends on EXP-2 outcome.
- Domain-fork validation (genomics / drug discovery panels) — wait for EXP-2 result first.

## Single Most Important Outcome

A heatmap that, on first viewing, makes the viewer think *"oh, so the 14 embedders carry roughly $N_{\mathrm{eff}}$ effective bits of independent information per input."* Whether $N_{\mathrm{eff}}$ ends up at 30 or 70, the **fact of measurement** is the contribution. The number itself is secondary.
