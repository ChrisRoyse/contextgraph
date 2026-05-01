# 1. Introduction

Predictive learning of world representations has converged on
*joint-embedding predictive architectures* (JEPAs), in which a model
predicts target representations from context representations rather than
reconstructing raw pixels, frames, or tokens [Assran et al., 2023;
LeCun, 2022; Bardes et al., 2024]. The architectural argument is that
useful structure lives in representation space; predicting at that level
focuses model capacity on what matters for downstream planning and
control [Ha & Schmidhuber, 2018; Hafner et al., 2023; Schrittwieser et
al., 2019].

This paper is *not* a representational-quality contribution to that
line. We accept the JEPA premise and ask a different, systems-level
question: **what would it take to make a complete latent world-model
runtime—data ingest, panel materialization, action-conditioned training,
planning, surprise recording, and artifact provenance—physically
auditable on a single commodity high-end GPU?**

The answer matters because:

1. **JEPA results are usually irreproducible at the artifact level.**
   Most JEPA papers release encoder weights, but datasets are large,
   licenses are restrictive, training is multi-node, and the path from
   raw event to predictor weights is rarely a single deterministic
   pipeline that a third party can re-run end-to-end and physically
   verify. ACM artifact-evaluation badges (Available, Functional,
   Reusable) explicitly require "documented, consistent, complete,
   exercisable" artifacts with permanent retrieval [ACM, 2020]. The
   NeurIPS Paper Checklist [NeurIPS, 2024] makes reproducibility a
   first-class submission requirement (omission is desk-rejection).
   Pineau et al. [2020] documented the gap.

2. **Occupational-domain ML benefits from row-level provenance.** Even
   when the *model* is small, the data pipeline benefits from having
   per-row source hashes and license attribution recorded inline.
   Constructing a public-taxonomy benchmark from O\*NET 30.2
   [USDOL/ETA, 2026] under CC BY 4.0 gives a small, fully reproducible
   substrate in which every transition can be traced back to a public
   source row.

3. **Single-GPU feasibility is a publication-quality property.** A
   single RTX 5090 (sm\_120, 32~GiB VRAM, CUDA 13.2) running on commodity
   hardware is enough for a paper-small career benchmark with multiple
   seeds, ablations, and audit edge cases (§7). Restricting compute to
   one host removes "hidden cluster" reproduction barriers and forces
   training-budget honesty (Google, 2023).

We present **Dynamic/ME-JEPA**, a Rust runtime that meets these
constraints on a single RTX 5090. The runtime is invoked as one binary
(`context-graph dynamicjepa …`) with subcommands for domain-pack
registration, event ingest, panel materialization, dataset compilation,
CUDA training, prediction, planning, surprise recording, multi-seed
aggregation, and reference checking. The command surface is small (29
subcommands, all enumerated in §3). The storage substrate is 25
DynamicJEPA-prefixed RocksDB column families plus an append-only audit
log (§3.2). The training engine uses Candle/CUDA on a Rust workspace
pinned by `Cargo.lock`. The reproducibility artifact ships
deterministically-built Docker and Apptainer (SIF) images with full
SHA-256 hash chains (§8).

## 1.1 Contributions

1. **Domain-pack abstraction.** A TOML domain pack is the only thing a
   new domain ships. The runtime reuses storage, training, planning,
   and FSV verbatim (§3.1, App. A). We instantiate three domains:
   `counter_world` (sanity), `gridworld_5x5` (compositional next-state
   prediction), and `career_taxonomy.v1` (the new public-taxonomy
   benchmark).

2. **Career-taxonomy public benchmark.** §4 specifies the event/state/
   action/outcome schema and an O\*NET-30.2-grounded walk generator that
   starts at Software Developers `15-1252.00` and restricts to core
   computer occupations. 1{,}016 source occupations, 218 selected
   occupations, 330 selected related-occupation edges; generated
   fixtures of 120 events for smoke and 1{,}000 events for paper-small.

3. **Strict full-state verification protocol.** §6 defines FSV-1
   through FSV-8 and shows their evidence rows in `dj_verification_runs`
   plus 11 edge-case rows in `paper_tables/table_edge_cases.csv` per
   bundle.

4. **Multi-seed paper-small results with BCa CI and paired permutation
   tests.** §7 reports five-seed validation/test cosine, validation
   action-contrast accuracy, random-init baseline, and shuffled-target
   baseline for `career_taxonomy`, plus paired tests of A1–A5 against
   A0. Hand-typed numbers are forbidden; every cited number is read
   from a CSV in the verified release-C aggregate bundle and the
   release-candidate archive.

5. **Local reproducibility artifact.** §8 documents the deterministic
   Docker + SIF build chain, the `reproduce.sh verify-aggregate`
   command, and the artifact archive layout. The archive is a *release
   candidate*; Zenodo DOI publication is the only remaining final-release
   blocker (§8.5).

## 1.2 Scope

Dynamic/ME-JEPA is a *runtime contribution* on a *public benchmark
contribution*. The model is intentionally small: 480, 16{,}960, and
15{,}344 trainable parameters in counter\_world, gridworld\_5x5, and
career\_taxonomy respectively. The point is not "small model beats large
model" — it is that *every stage* of a domain-portable JEPA system can
be physically verified on commodity hardware, and that a public-taxonomy
benchmark can be the first such substrate.
