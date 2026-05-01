# Dynamic/ME-JEPA: Source-of-Truth Verified Latent World Modeling on a Single RTX 5090

**Working title.** A Domain-Pack-Driven, Auditable Joint-Embedding Predictive Architecture
With Public-Taxonomy Career Transition Benchmarking and a Local Docker+Apptainer
Reproducibility Artifact.

**Author.** Chris Royse (`chrisroyseai@gmail.com`).

**Status as of 2026-05-01.** Local Docker + Singularity (SIF) release candidate
`mejepa_5090_artifact_v1.0.0_rc5_sif_verified`; Zenodo DOI not yet
published. All headline numbers and hashes in this manuscript are read back
from `tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2*` and
`tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc5_sif_verified*`
on the verified RTX 5090 host described in §2.

**Repository.** `https://github.com/ChrisRoyse/contextgraph` (this is a
prepared release candidate in `CITATION.cff`; the public DOI on Zenodo has
not yet been issued).

**Reproduction.** The exhaustive layout of the manuscript is split across
the files in this directory; this `00_main.md` file is the canonical entry
point. The paper is intentionally written as a set of `\input{}`-style
fragments so that table CSVs and FSV reports can be cited by absolute
filesystem path:

| File | Section |
| --- | --- |
| `01_abstract.md` | Abstract (≤200 words) |
| `02_introduction.md` | §1 Introduction |
| `03_related_work.md` | §2 Related Work |
| `04_method_system.md` | §3 The Dynamic/ME-JEPA Runtime |
| `05_benchmark_career_taxonomy.md` | §4 Career-Taxonomy Public Benchmark |
| `06_experiments.md` | §5 Experiments and Ablations |
| `07_full_state_verification.md` | §6 Full-State Verification |
| `08_results.md` | §7 Results |
| `09_reproducibility_artifact.md` | §8 Reproducibility Artifact |
| `10_discussion_limitations_ethics.md` | §9 Discussion, §10 Limitations, §11 Ethics & Broader Impact |
| `11_references.md` | References (verified URLs only) |
| `12_appendix.md` | Appendix A–F: domain packs, FSV evidence, failed runs, hashes |

The contributions, in order:

1. **A domain-pack-driven JEPA runtime.** A single Rust binary (`context-graph
   dynamicjepa …`) ingests typed event streams from arbitrary domains
   (counter\_world, gridworld\_5x5, career\_taxonomy) using TOML-declared
   domain packs, materializes multi-instrument latent panels into RocksDB
   column families, trains an action-conditioned JEPA predictor on the RTX
   5090 with Candle/CUDA, runs candidate-action planning with
   constellation-style guard decisions, and records surprise events—all
   while writing every operation through one of 25 typed column families
   plus an append-only audit log. §3.

2. **A public-taxonomy career benchmark `career_taxonomy.v1`.** Generated
   deterministically from the official O*NET 30.2 text database
   ([onetcenter.org](https://www.onetcenter.org/database.html), CC BY 4.0)
   starting from O\*NET-SOC `15-1252.00` Software Developers and walking
   the core computer-occupation related-occupation graph. Every generated
   row records the source ZIP SHA-256
   `b5479271931796b838f7173dc0f673a9ec961b7833ac87168fd11e92e7453741`, the
   per-edge walk hash, and the CC BY 4.0 + USDOL/ETA non-endorsement
   clause. §4.

3. **A strict full-state verification (FSV) protocol.** Every claim in §7
   has a corresponding RocksDB row, file SHA-256, and decoded readback
   physically separate from the command return code. Eleven edge cases
   per career bundle prove the system fails closed on missing license
   manifest, invalid action, wrong skill-vector dimension, out-of-bounds
   job zone, and empty input. §6.

4. **Statistically reported, multi-seed paper-small results on the RTX
   5090.** Five real O\*NET-backed seeds {42, 43, 44, 45, 46} of
   `paper_small_1k` (1,000 career events each), aggregated with BCa
   bootstrap (10,000 iterations, seed `20260430`, confidence 0.95) and a
   paired sign-flip permutation test against the A0 ablation baseline.
   Career-taxonomy validation cosine across the five seeds is
   $0.905 \pm 0.044$ (BCa 95% CI [0.861, 0.931]) at 15{,}344 trainable
   parameters. The negative-action ablation A3 collapses to validation
   cosine 0.790 (single-cell) — exactly as the framework predicts and
   verifies through `dj_training_runs` decoded readback. §7.

5. **A reproducibility artifact at the local Docker+SIF release-candidate
   stage.** `Dockerfile` pinned by digest
   `sha256:d266e59b88c295bc5fa0e4cef9064eaff84939381b09e2c3d76a5532a303e42d`
   on `nvidia/cuda:13.2.0-devel-ubuntu24.04`. Two consecutive deterministic
   `docker buildx build` exports produce the same tar SHA-256
   `099f0535e9f4f5dc8e07d0afb550ef1e3197657b52ddc7561ef11304f89dd80e`. The
   Apptainer SIF built from that tar via the documented `docker-archive`
   bootstrap agent has SHA-256
   `30b531c77966c46524383adcceea9de7983ee251935a68740c2f3009695f2825`.
   The release-candidate archive has SHA-256
   `8cccf4980893fe949db9b50b8111e592e72ab8419438c4cd08e10c8974943ecd`.
   Zenodo DOI publication is the only remaining final-release blocker.
   §8.

6. **Explicit, paper-safe non-claims.** This work does *not* claim
   recruiting prediction validity, hiring decision support, real-person
   trajectory modeling, fairness across protected classes, or compliance
   with NYC Local Law 144 / EU AI Act Annex III. Phase 12 privacy gates
   for licensed real subject-level data are deferred and explicitly
   out of scope. §10, §11.

The remainder of this manuscript is organized as the section files listed
above. Numerical claims in §7 are auto-readable from
`tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_multiseed_aggregate/paper_tables/*.csv`;
hash claims in §8 are auto-readable from
`tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc5_sif_verified/release_manifest.json`
and `archive_file_hashes.json`. Every table or hash cited is recoverable
from the artifact archive and bears a reference path that survives the
paper.
