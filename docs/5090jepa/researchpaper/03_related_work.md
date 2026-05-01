# 2. Related Work

We anchor this work in four lines of literature.

## 2.1 Joint-Embedding Predictive Architectures

I-JEPA [Assran et al., 2023] introduced the image-domain JEPA in which a
context block predicts the representation of target blocks in a
self-distilled latent space. V-JEPA [Bardes et al., 2024] extended the
idea to video by feature prediction, with downstream evaluations showing
that latent prediction recovers strong representations without
reconstruction, negatives, or text supervision. The official Meta/FAIR
V-JEPA repository [Meta, 2024] has been the reference engineering
implementation. LeCun's "A Path Towards Autonomous Machine Intelligence"
[LeCun, 2022] frames this family as a step toward predictive systems
that reason in abstract representation space.

Dynamic/ME-JEPA does *not* claim to improve on I-JEPA or V-JEPA on their
benchmarks. The architectural change is in *how target representations
are sourced*: instead of an EMA target encoder, ME-JEPA uses a panel of
*frozen, externally-defined* per-domain instruments declared by the
domain pack (§3). The published ME-JEPA design notes
([backupdocs/mejepa/01_research_context.md](../../backupdocs/mejepa/01_research_context.md))
make the narrower claim: external frozen targets *eliminate target-encoder
collapse* but do not eliminate predictor degeneracy (e.g. shuffled-target
shortcut, ignored action conditioning), so VICReg-style diagnostics
[Bardes, Ponce & LeCun, 2022] and ablations are still required.

Earlier self-supervised work that informs the predictor regularization
choices: BYOL [Grill et al., 2020] (asymmetric online/target,
stop-gradient as collapse mitigation) and Contrastive Predictive Coding
[van den Oord et al., 2019] (latent forecast objective). VICReg
specifically supplies the variance/covariance/invariance regularization
applied as ablation A5 (variance/covariance regularization removed).

## 2.2 World Models and Latent Planning

Dynamic/ME-JEPA's planning, surprise, and guard plane is informed by:
World Models [Ha & Schmidhuber, 2018] (compressed spatial-temporal
representation feeding a controller); the Dreamer line including
DreamerV3 [Hafner et al., 2023] (one configuration generalizing across
many tasks); and MuZero [Schrittwieser et al., 2019] (planning with a
learned model rather than a given dynamics function). We use the
imagined-rollout idea conceptually for the candidate-action planner
(§3.4) but constrain rollouts to one step in the present runtime; H>1
multi-step rollouts are an explicit deferred axis (§9, App.).

## 2.3 Generalist and Multi-Modal Architectures

Gato [Reed et al., 2022] demonstrated multi-modal multi-task multi-embodiment
*tokenized policies*; Perceiver IO [Jaegle et al., 2022] showed that one
attention-based architecture handles arbitrary structured I/O. ME-JEPA
borrows neither of these as engineering, but inherits the design
constraint they imposed on the field: a *general-purpose* latent runtime
must not hard-code modality. The domain-pack abstraction (§3.1) is our
answer: each domain ships a TOML manifest declaring state schema, action
schema, instruments, and panel slots, and the runtime carries no
domain-specific branches.

## 2.4 Career and Occupational Taxonomies

The U.S. Department of Labor and Employment Training Administration's
O\*NET database [USDOL/ETA, 2026] is the canonical public occupation
taxonomy used here. The current release at the time of writing is
O\*NET 30.2, distributed as text files under CC BY 4.0 with an explicit
USDOL/ETA non-endorsement clause for modifications. The European
Skills, Competences, Qualifications and Occupations (ESCO) classification
[European Commission, 2025] is the EU counterpart; ESCO is referenced
as a future cross-walk replication source (§9, App. E in the design
plan).

Two recent career-trajectory dataset papers are directly relevant as
*future* extensions but not used in the present artifact: KARRIEREWEGE
[Senger et al., 2024] (career path prediction over an ESCO taxonomy)
and JobHop [Johary et al., 2025] (large-scale anonymized resume
trajectories). We read both during construction of §4 to confirm that
the career-trajectory benchmark space exists, but neither is loaded
into the present run; the only ingested source is O\*NET 30.2. This
distinction is paper-safety-critical: we make a *taxonomy walk*
benchmark contribution, not a real-person trajectory contribution.

## 2.5 Auditable ML Systems

The runtime's evidence-bundle design is informed by MLflow Tracking
[MLflow, 2024] (run, experiment, artifact, metric lineage) and ACM
Artifact Review and Badging [ACM, 2020] (Available / Functional /
Reusable badge criteria). The reproducibility ladder—pinned base image
digest, deterministic build, deterministic tar export, container DOI—
follows Docker reproducible-build guidance [Docker, 2025] and Apptainer
SIF-from-docker-archive guidance [Apptainer, 2025]. The ACM
Reproducibility / Replicability community resources [ACM REP, 2025]
provide the broader badge ecosystem context. PyTorch's reproducibility
notes [PyTorch, 2026] are relevant because we inherit the same caveat:
exact GPU result reproducibility across releases or platforms is *not*
guaranteed; cross-run comparisons therefore use structural + tolerance
checks, not byte-equal model-file diffs (§6, §8).

## 2.6 What This Paper Is Not

This is not:

- a representational-quality contribution to image or video JEPA;
- a recruiting prediction or hiring-decision system;
- an evaluation of any large-scale career foundation model;
- a fairness contribution across protected classes;
- a Phase 12 privacy-gated real-data study (deferred).

§11 expands these non-claims with corresponding NIST AI RMF mappings.
