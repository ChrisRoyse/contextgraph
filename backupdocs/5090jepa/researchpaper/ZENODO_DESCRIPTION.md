# Zenodo Record Update — `10.5281/zenodo.19953951`

This file is the proposed replacement description / metadata for the
existing Zenodo record so it reflects every contribution the artifact
actually contains, and so it functions as a selling point to the
target audience (technically-curious readers who care about JEPA,
world models, agentic / process-supervised training, reproducibility,
and synthetic-data quality). Paste the **Description** section into
the Zenodo "Description" field and update **Keywords**, **Related
Identifiers**, and **Notes** as listed.

---

## Title

(unchanged) **Context Graph DynamicJEPA RTX 5090 O\*NET Artifact**

Optional subtitle to tighten the pitch (paste at the start of the
description if Zenodo allows): *An audited, domain-portable JEPA
runtime on a single RTX 5090 — and a new class of training data.*

## Description (paste into Zenodo)

> **What this is, in one paragraph.** A complete, single-binary
> JEPA-style world-model runtime that ingests typed events, learns
> latent next-state predictions, plans across candidate actions,
> records surprise, and writes every persisted operation through 25
> typed RocksDB column families plus an append-only audit log on a
> single RTX 5090. A separate verifier reads everything back. The
> same code path runs three domains (`counter_world`,
> `gridworld_5x5`, and a new public-taxonomy career benchmark
> `career_taxonomy.v1` derived from O\*NET 30.2). The reproducibility
> artifact is a deterministically built Docker + Singularity (SIF)
> image with full SHA-256 hash chains and a one-command
> `verify-aggregate` reproduction path.
>
> **Why it matters (selling-point summary).**
>
> 1. **One binary, three domains, by TOML alone.** A new domain ships
>    as a strict TOML manifest; the Rust runtime carries no domain-
>    specific code branches. Twenty-nine subcommands compose into one
>    immutable evidence bundle per run.
> 2. **Five-seed paper-small results on commodity hardware.** Career-
>    taxonomy validation cosine 0.905 ± 0.044 (BCa 95\% CI 0.861 –
>    0.931); test cosine 0.868 ± 0.068 (BCa 0.806 – 0.912); validation
>    action-contrast accuracy 0.960 ± 0.043. The negative-action
>    ablation A3 collapses to validation cosine 0.790 — exactly as
>    the design predicts and the verifier confirms. Every cited
>    number reads from a CSV in the verified release bundle. Hand-
>    typed numbers are forbidden.
> 3. **A reproduction reviewer can actually run.** One-command
>    `./reproduce.sh verify-aggregate` returns
>    `{status: ok, diff_count: 0, manifest: passed, fsv: passed,
>    passed_count: 5}` byte-for-byte. The Docker tar SHA-256
>    `099f053…7d4cd80e` and the SIF SHA-256 `30b531c…f2825` are
>    deterministic across two consecutive builds.
> 4. **Each evidence bundle is also a new class of training data.**
>    The artifact argues, with citations, that a bundle is an
>    instance of a training-data class distinct from web text,
>    pure model-generated synthetic data, and rejection-sampled
>    chain-of-thought: *audited execution traces with cryptographic
>    provenance.* Most of the bytes are deterministic functions of
>    the public O\*NET ZIP rather than a model output; model outputs
>    are paired in the same record with the deterministic observed
>    target plus random-init / shuffled-target baselines;
>    counterfactual ablations and failed runs are persisted as
>    first-class records. The result is a corpus that, when used
>    with care, carries substantially lower model-collapse hazard
>    (Shumailov et al., *Nature* 2024) than typical synthetic
>    training data, and that is unusually well-suited to training
>    agentic, process-supervised, and audit-aware downstream
>    models.
> 5. **Failed runs are kept on purpose.** Most synthetic-data
>    pipelines throw rejected samples away; this artifact preserves
>    them. That choice is the difference between recursion-with-
>    replacement (which Shumailov showed causes collapse) and
>    accumulation (which Gerstgrasser et al.\ 2024 showed avoids it).
>
> **Who this is for.** Anyone working on JEPA-family systems,
> world models, agentic training pipelines, audit-aware ML
> deployments, reproducibility infrastructure, synthetic-data
> quality, or the next generation of training corpora for
> process-supervised reasoning. The artifact is a working
> reference architecture, not a paper-only sketch: the binary
> runs, the bundle reproduces, the SIF verifies.
>
> **What's in the archive.**
>
> - `bin/context-graph` — the single Rust binary that runs the
>   entire pipeline (ingest, panel, train, predict, plan, verify).
> - `container/contextgraph-mejepa.docker.tar` (Docker image) and
>   `container/contextgraph-mejepa.sif` (Apptainer / SingularityCE
>   image) — both built deterministically from the same source.
> - `bundles/` — the five-seed paper-small evidence bundles plus
>   the multi-seed aggregate, each containing typed RocksDB stores,
>   FSV reports, paper tables, plots, and per-bundle manifests.
> - `paper_tables_reference/` — the exact CSVs cited in the paper.
> - `reference/reference_release_v1.0.0.json` — the checked-in
>   numeric + structural release reference.
> - `reproduce.sh` — the reviewer command that reproduces every
>   number.
> - Full SHA-256 chain in `archive_file_hashes.json` and
>   `release_manifest.json`.
>
> **Hardware.** NVIDIA GeForce RTX 5090 (sm\_120, 32 GiB VRAM,
> CUDA 13.2). Reproduction requires a comparable GPU; the artifact
> uses the NVIDIA container runtime and `--gpus all`.
>
> **Headline numbers, again, for indexing.** Career-taxonomy
> validation cosine 0.905 ± 0.044, BCa 95\% CI 0.861 – 0.931
> (n = 5 seeds). Counter\_world 0.99963 / 0.99916 (480 params).
> Gridworld\_5x5 0.99473 / 0.99472 (16{,}960 params). Full FSV
> integration suite passes in 190.92 s. The full release-C gate
> (5 seeds + aggregate + container build + container verify +
> manual edges) fits in ~2 hours of wall time on the documented
> host.
>
> **Where the paper is.** A two-column publication-ready LaTeX
> manuscript ships with the source repository at
> `docs/5090jepa/researchpaper/paper.tex` and an arXiv-targeted
> version of `paper.pdf` is bundled in this Zenodo deposit. The
> paper develops the runtime, the public-taxonomy benchmark, the
> full-state-verification protocol, and the new training-data
> class argument.

## Keywords (replace existing list)

```
JEPA
ME-JEPA
joint-embedding predictive architecture
world model
audited execution traces
training data
synthetic data
model collapse
process supervision
reproducibility
RTX 5090
Apptainer
Singularity
SIF
Docker
deterministic builds
RocksDB
Rust
O*NET
career taxonomy
public benchmark
full-state verification
provenance
agentic training
```

## Authors

**Chris Royse** (sole author).
ORCID: *(add if available)*.
Affiliation: Independent researcher.
Email: `chrisroyseai@gmail.com`.

## Version

`v1.0.0-rc5` (current); plan to publish `v1.0.0` once any post-deposit
issues are resolved. The version string is also recorded in
`release_manifest.json`.

## License

(unchanged) **PolyForm Noncommercial License 1.0.0**.
Bundle data and generated paper tables: **CC BY 4.0**.
Third-party data (O\*NET 30.2): retains the original CC BY 4.0
license; full attribution in the bundle's `data_manifest.json` and
`THIRD_PARTY_LICENSES/ONET.md`.

## Related identifiers (add)

- `IsSupplementTo` — `https://github.com/ChrisRoyse/contextgraph`
  (source repository).
- `IsDerivedFrom` — `https://www.onetcenter.org/dl_files/database/db_30_2_text.zip`
  (O\*NET 30.2 public source ZIP, hashed at
  `b5479271…7453741` in every bundle's `data_manifest.json`).
- `Cites` — Shumailov et al.\ 2024 (model collapse, *Nature*
  631:755 – 759) DOI `10.1038/s41586-024-07566-y`.
- `Cites` — Gerstgrasser et al.\ 2024 (accumulation avoids
  collapse) arXiv `2404.01413`.
- `Cites` — Lightman et al.\ 2023 (process supervision /
  PRM800K) arXiv `2305.20050`.
- `Cites` — Jung et al.\ 2025 (code-execution-grounded
  supervision) arXiv `2506.10343`.
- `Cites` — Assran et al.\ 2023 (I-JEPA) arXiv `2301.08243`.
- `Cites` — Bardes et al.\ 2024 (V-JEPA) arXiv `2404.08471`.

## Notes (add to Zenodo "Notes" field)

> Reproduction requires NVIDIA GPU (sm\_120 / Ada Lovelace)
> and the NVIDIA container runtime. Apptainer / SingularityCE 4.x
> recommended for the SIF execution path; Docker 24+ for the
> Docker tar path. Per PyTorch's reproducibility notes, exact GPU
> bit-reproducibility across releases or hardware is not
> guaranteed; the release reference therefore uses a structural
> + numeric-tolerance schema rather than byte-equal model-weight
> diffs. Failed runs are intentionally preserved in the archive as
> evidence; the artifact's training-data argument depends on this
> property (see the paper, §7).

## Files (verify after upload; should already be present)

- `mejepa_5090_artifact_v1.0.0_rc5_sif_verified.tar.gz` (7.4 GB,
  SHA-256 `8cccf4980893fe949db9b50b8111e592e72ab8419438c4cd08e10c8974943ecd`).
- `checksums.txt`.

**Optional additions to upload alongside the existing files** (each
small, makes the deposit more self-contained for readers landing
here directly):

- `paper.pdf` — the two-column publication-ready manuscript.
- `paper.tex`, `references.bib` — LaTeX source.
- `CITATION.cff` — citation metadata.
- `README.md` (the artifact's top-level README).

## Suggested commit message for any associated repo update

```
docs(zenodo): rewrite v1.0.0-rc5 description to highlight the
audited-execution-traces training-data class, multi-seed paper-small
results (val cosine 0.905 ± 0.044), deterministic Docker + SIF
reproducibility chain, and target audience of JEPA / world-model /
agentic-training researchers.
```
