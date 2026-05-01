# Abstract

Joint-embedding predictive architectures (JEPA) learn world representations
by predicting target representations rather than reconstructing raw
observations [Assran et al., 2023; Bardes et al., 2024]. Most JEPA work
emphasizes representation quality on natural images or video; relatively
little work makes the entire training, planning, and audit pipeline
*physically verifiable* on commodity high-end hardware. We present
Dynamic/ME-JEPA, a domain-pack-driven JEPA runtime whose data ingest,
multi-instrument latent panel materialization, action-conditioned
predictor training, candidate-action planning, surprise recording, and
artifact registry operations are each backed by typed RocksDB column
families and an append-only audit log, with full-state verification (FSV)
that physically re-reads every persisted output. We instantiate the
runtime on three domains via the same code path: `counter_world` (480
trainable parameters), `gridworld_5x5` (16{,}960), and a new
`career_taxonomy.v1` public-taxonomy benchmark (15{,}344). The career
benchmark is deterministically generated from the O*NET 30.2 text
database (Creative Commons Attribution 4.0; USDOL/ETA non-endorsement
clause reproduced verbatim) starting at O\*NET-SOC `15-1252.00` Software
Developers and walking the core computer-occupation related-occupation
graph; the source ZIP SHA-256 and per-edge walk hash are recorded in
every generated bundle. On a single RTX 5090 (sm\_120, 32~GiB VRAM, CUDA
13.2, Rust 1.95) we report five-seed paper-small (1{,}000 events per
seed, seeds {42, 43, 44, 45, 46}) results with BCa bootstrap confidence
intervals (10{,}000 iterations, seed `20260430`) and paired sign-flip
permutation tests against the action-removed A3 ablation baseline.
Career-taxonomy validation cosine is $0.905 \pm 0.044$ across five seeds
(BCa 95\% CI [0.861, 0.931]); test cosine is $0.868 \pm 0.068$ (BCa
[0.806, 0.912]); validation action-contrast accuracy is $0.960 \pm
0.043$. The action-removed ablation A3 collapses to validation cosine
0.790 (single-cell), exactly as the runtime predicts and verifies. The
reproducibility artifact is a local Docker + Singularity (SIF) release
candidate with deterministic Docker tar
SHA-256 `099f053…7d4cd80e`, SIF SHA-256 `30b531c…f2825`, and archive
SHA-256 `8cccf49…43ecd`; the only remaining final-release blocker is
Zenodo DOI publication. We make no recruiting, hiring-decision,
real-person, fairness, or labor-market validity claims; the contribution
is a verifiable systems result on a public taxonomy benchmark, with
domain-pack abstraction so future domains add no new audit primitives.
