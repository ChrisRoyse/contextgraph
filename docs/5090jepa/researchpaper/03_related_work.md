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
multi-step rollouts are an explicit deferred axis (§10–§11, App.).

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
O\*NET 30.2, distributed as text files under CC BY 4.0. The European
Skills, Competences, Qualifications and Occupations (ESCO) classification
[European Commission, 2025] is the EU counterpart; ESCO is a planned
cross-walk replication source.

Two recent career-trajectory dataset papers are directly relevant as
future extensions: KARRIEREWEGE [Senger et al., 2024] (career path
prediction over an ESCO taxonomy) and JobHop [Johary et al., 2025]
(large-scale anonymized resume trajectories). The only ingested source
in the present runtime is O\*NET 30.2.

## 2.5 Auditable ML Systems

The runtime's evidence-bundle design is informed by MLflow Tracking
[MLflow, 2024] (run, experiment, artifact, metric lineage). The
reproducibility ladder — pinned base image digest, deterministic
build, deterministic tar export, container DOI — follows Docker
reproducible-build guidance [Docker, 2025] and Apptainer SIF-from-
docker-archive guidance [Apptainer, 2025]. PyTorch's reproducibility
notes [PyTorch, 2026] are relevant because we inherit the same caveat:
exact GPU result reproducibility across releases or platforms is *not*
guaranteed; cross-run comparisons therefore use structural + tolerance
checks, not byte-equal model-file diffs (§6, §9).

## 2.6 Synthetic Data, Model Collapse, and Process Supervision

Recent work establishes a non-trivial taxonomy of training-data
classes and their effect on model quality under recursive training.
Shumailov et al. [2023; 2024] introduced *model collapse*: training
generative models on recursively generated data leads to irreversible
loss of distribution tails and eventual convergence to a degenerate
distribution. The result holds for VAEs, GMMs, and LLMs, and is the
canonical reason for caution about LLM-only synthetic corpora.
Gerstgrasser et al. [2024] subsequently showed that *accumulating*
real and synthetic data avoids the collapse pattern that *replacement*
triggers — distinguishing the dangerous recursion-with-replacement
case from the safer accumulation case.

In parallel, the highest-quality synthetic reasoning corpora have
moved away from pure LLM generation toward *verifier-grounded* data.
Lightman et al. [2023] introduced process supervision and released
PRM800K, a corpus of 800{,}000 step-level human-feedback labels for
mathematical reasoning. Jung et al. [2025] proposed code-execution-
grounded supervision: extracting deterministic step-by-step traces
from program execution and converting them into chain-of-thought
supervision, exploiting the fact that program execution is verifiable,
step-by-step, and causally linked to outcomes. Open-source verifier-
grounded corpora such as SYNTHETIC-1 [Prime Intellect, 2025] (1.4M
verified reasoning traces) and Enigmata [Seed, 2025] (synthetic
verifiable puzzles) sit in this category.

§7 extends this taxonomy with a class of training data that the
ME-JEPA runtime produces by construction: *audited execution traces
with cryptographic provenance*, in which the bulk of the bytes are
deterministic functions of a public source rather than model outputs,
the small model-output slice is paired with deterministic observed
targets and external baselines, and counterfactual ablations and
failed runs are persisted as first-class records.

## 2.7 Positioning

This is a *systems* contribution: a domain-portable JEPA runtime with
end-to-end provenance, a public-taxonomy benchmark, and a structured
training-data corpus emitted as a side effect of every run. It is not
a representational-quality contribution to image or video JEPA, and it
is not an evaluation of any large-scale foundation model.
