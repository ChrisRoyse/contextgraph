# Abstract

Joint-embedding predictive architectures (JEPA) learn world representations
by predicting target representations rather than reconstructing raw
observations [Assran et al., 2023; Bardes et al., 2024]. Most JEPA work
emphasizes representation quality on natural images or video; relatively
little work makes the entire training, planning, and audit pipeline
*physically verifiable* on commodity high-end hardware. I present
Dynamic/ME-JEPA, a domain-pack-driven JEPA runtime whose data ingest,
multi-instrument latent panel materialization, action-conditioned
predictor training, candidate-action planning, surprise recording, and
artifact registry operations are each backed by typed RocksDB column
families and an append-only audit log, with full-state verification (FSV)
that physically re-reads every persisted output. I instantiate the
runtime on three domains via the same code path: `counter_world` (480
trainable parameters), `gridworld_5x5` (16{,}960), and a new
`career_taxonomy.v1` public-taxonomy benchmark (15{,}344). The career
benchmark is deterministically generated from the O*NET 30.2 text
database (CC BY 4.0) starting at O\*NET-SOC `15-1252.00` Software
Developers and walking the core computer-occupation related-occupation
graph; the source ZIP SHA-256 and per-edge walk hash are recorded in
every generated bundle. On a single RTX 5090 (sm\_120, 32~GiB VRAM, CUDA
13.2, Rust 1.95) I report five-seed paper-small (1{,}000 events per
seed, seeds {42, 43, 44, 45, 46}) results with BCa bootstrap confidence
intervals (10{,}000 iterations, seed `20260430`) and paired sign-flip
permutation tests against the action-removed A3 ablation baseline.
Career-taxonomy validation cosine is $0.905 \pm 0.044$ across five seeds
(BCa 95\% CI [0.861, 0.931]); test cosine is $0.868 \pm 0.068$ (BCa
[0.806, 0.912]); validation action-contrast accuracy is $0.960 \pm
0.043$. The action-removed ablation A3 collapses to validation cosine
0.790, exactly as the runtime predicts and verifies. Beyond the
verification result, I argue that each immutable bundle constitutes a
new training-data class — *audited execution traces with cryptographic
provenance* — distinct from web text, pure model-generated synthetic
data, and rejection-sampled chain-of-thought. The bulk of the bundle is
a deterministic function of the public source ZIP rather than a model
output; model outputs are paired in the same record with deterministic
observed targets and random-init / shuffled-target baselines; and
counterfactual ablations and failed runs are persisted as first-class
records, preserving the distribution tails whose loss defines model
collapse. The reproducibility artifact is a local Docker + Singularity
(SIF) release candidate with deterministic Docker tar SHA-256
`099f053…7d4cd80e`, SIF SHA-256 `30b531c…f2825`, and archive SHA-256
`8cccf49…43ecd`. The contribution is a domain-portable JEPA runtime
whose entire evidence chain — data, training, planning, and artifact
provenance — is reproducible from a single binary on commodity hardware
and is itself a structured corpus suitable for downstream agentic,
process-supervised, and audit-aware training.
