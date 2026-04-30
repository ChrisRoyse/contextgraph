# 14 - Remaining Work Master Plan

This document is the index for everything still required between the current
implemented state (smokedocs 13, 2026-04-30) and a paper-submittable +
artifact-badged release. Docs 00-13 are the original plans and remain canonical
for scope, claims, and evidence schema. Docs 15-25 below define the work that
is **not yet implemented**.

## Source Of Truth For "What Is Done"

The current implementation status comes from:

- `smokedocs/13_current_implementation_and_benchmark_status.md`;
- `smokedocs/reports/20260430_paper_small_1k_fsv.md`;
- `smokedocs/reports/20260430_multiseed_status.md`;
- `smokedocs/reports/20260430_seed_control_patch.md`;
- `smokedocs/reports/20260430_seed43_failure_analysis.md`;
- `smokedocs/reports/20260430_web_research_notes.md`;
- last 2 days of `git log` on `main`.

A single passed paper-small 1K bundle exists
(`20260430T_paper_small_1k_v1`), one explicit-seed=42 passed bundle
(`20260430T_paper_small_1k_seed42_v1`), one preserved failed seed=43 bundle,
and one seed=42 v2 run in flight.

## Gap Matrix

| ID | Capability | Original Doc | Status | Plan Doc |
| --- | --- | --- | --- | --- |
| G1 | 3+ seed paper-small with mean/std + bootstrap CI | 07 §Statistical Reporting | partial (2 passed, 1 in flight, 1 failed) | 15 |
| G2 | Decoder head for top-k occupation + skill-vector reconstruction | 07 §Core Metrics | not started | 16 |
| G3 | Paper-full scaling (10K-250K events) | 02 §Feasible Demo Sizes | not started; envelope sketched only | 17 |
| G4 | ESCO replication / crosswalk | 11 §Open Questions Q3 | not started | 18 |
| G5 | arXiv paper drafting + figures + supplementary | 10 outline only | outline exists, no prose | 19 |
| G6 | Reproducibility artifact (container, Zenodo DOI, ACM badging) | 12 (refs only) | not started | 20 |
| G7 | Compliance / ethics / broader impact | 11 §Risks (compliance) | language placeholder only | 21 |
| G8 | Hyperparameter sweeps + advanced collapse diagnostics | 06 §A5, 07 §Collapse | A5 only; rank/eigenvalue/alignment-uniformity not implemented | 22 |
| G9 | CI smoke regression / drift detection | 09 (manual) | manual only | 23 |
| G10 | Multi-step rollout + planning horizon | 02 §What Not To Spend GPU On Yet | one-step only | 24 |
| G11 | Privacy gate / Phase 12 licensed-data path | 01 §Claims We Must Not Make | unbuilt | 25 |

## Dependency Graph

```text
G1 multiseed ──────────────┐
                           ├──> G5 paper drafting ──> G6 artifact ──> release
G2 decoder + top-k ────────┤                          ▲
G8 HP sweeps + collapse ───┤                          │
G7 compliance/ethics ──────┴──────────────────────────┘
                                                       │
G4 ESCO replication ───────> G3 paper-full scaling ───┤
G9 CI regression ──────> protects all above ──────────┤
G10 multi-step ────> appendix/extension only ─────────┘
G11 privacy gate ──> Phase 12 (out of paper scope)
```

Critical path to paper submission: **G1 → G2 → G8 → G5 → G6**. ESCO (G4) and
paper-full (G3) strengthen claims but are not blockers if the paper-small
auditability story is the headline. Multi-step (G10) and Phase 12 (G11) are
explicitly post-submission.

## Recommended Execution Order

| Order | Doc | Why First |
| ---: | --- | --- |
| 1 | 23 (CI smoke regression) | Protects every later run from silent drift |
| 2 | 15 (multi-seed + stats) | Finish the basic ≥3-seed evidence floor |
| 3 | 22 (HP sweeps + collapse diagnostics) | Tightens A0-A5 numbers + adds rank/uniformity |
| 4 | 16 (decoder head + top-k) | Adds the metrics the paper currently disclaims |
| 5 | 17 (paper-full scaling) | Optional headline: 10K events with full FSV |
| 6 | 18 (ESCO crosswalk) | Demonstrates non-O*NET portability |
| 7 | 21 (compliance/ethics) | Required before any external release |
| 8 | 20 (artifact + Zenodo + ACM) | Locks in reproducibility for review |
| 9 | 19 (paper drafting + figures) | Writes from finalized evidence bundle |
| 10 | 24 (multi-step) | Appendix/extension; not blocker |
| 11 | 25 (privacy gate) | Phase 12; explicitly future work |

## Completion Criteria For "Paper-Ready"

The release is paper-ready only when **all** of the following hold:

1. ≥3 paper-small seeds passed with mean/std + bootstrap CI in `paper_tables/`.
2. Decoder head trained, top-k occupation + skill-vector cosine reported with
   FSV evidence and not disclaimed.
3. A0-A5 ablations replicated under multi-seed with paired bootstrap + sign-flip
   permutation tests.
4. Collapse diagnostics include VICReg variance, off-diagonal covariance,
   effective rank, and alignment+uniformity from Wang & Isola.
5. Container image (Apptainer + Docker) builds the binary, ingests an O*NET ZIP
   from local mount, and reproduces a bundle whose hashes match the reference
   bundle.
6. Code + bundle archived to a Zenodo DOI; arXiv preprint links to the DOI.
7. NIST AI RMF compliance statement, NeurIPS reproducibility checklist, and ACM
   artifact-evaluation packet are all populated from `paper_tables/` and bundle
   JSON, not hand-typed numbers.
8. CI runs the research-smoke wrapper at least daily and on every PR that
   touches `crates/context-graph-dynamicjepa` or `crates/context-graph-cli`.
9. No paper claim depends on data the bundle cannot prove on disk.

## Anti-Goals

These are **out of scope for the paper version** and must not block submission:

- Real recruiting / licensed subject-level data (deferred to Phase 12, doc 25);
- Multi-step planning horizons beyond H=1 (deferred to doc 24);
- Production deletion propagation FSV against a vault (deferred to doc 25);
- Large encoders or >5M-parameter career models (out of envelope per doc 02);
- Replacement of VICReg with SIGReg / LeJEPA (research detour; track in doc 22
  as optional);
- Re-running counter_world / gridworld at larger scale (already saturated).

## Document Map (15-25)

| Doc | Scope |
| --- | --- |
| 15 | Multi-seed completion, BCa bootstrap CI, paired sign-flip permutation tests |
| 16 | Decoder head architecture, top-k occupation + skill-vector reconstruction, FSV extension |
| 17 | Paper-full scaling: 1K → 10K → 50K-250K, VRAM/runtime budget |
| 18 | ESCO replication + O*NET crosswalk; Karrierewege/JobHop/OpenResume optional |
| 19 | Paper drafting plan: section-by-section prose targets, figure pipeline, supplementary materials |
| 20 | Reproducibility artifact: Apptainer + Docker, Zenodo DOI, ACM artifact-evaluation packet |
| 21 | Compliance: NIST AI RMF mapping, O*NET attribution, broader impact, dual-use disclosure |
| 22 | HP sweeps (LR, latent dim, batch) + advanced collapse diagnostics (rank, alignment+uniformity) |
| 23 | CI smoke regression + golden-bundle hash policy + drift detection |
| 24 | Multi-step rollout (H>1), planning horizon, error compounding analysis |
| 25 | Phase 12 privacy gate: licensed-data path, k-anonymity / DP, deletion FSV |

## Cross-Reference Discipline

Every later doc must:

- name the bundle JSON paths and CFs that will hold its evidence;
- name the new CLI flags / verifiers that must be added;
- name the failure modes it must fail closed on;
- name which gate in doc 04 it extends or replaces;
- record any new data sources in `data_manifest.json` schema (doc 03).
