# 9. Discussion

## 9.1 What Dynamic/ME-JEPA Demonstrates

The runtime is a domain-portable JEPA system in which:

- **One Rust binary (`context-graph`) provides the entire surface.**
  Twenty-nine subcommands compose into one immutable bundle per
  research-smoke run. New domains require only a TOML domain pack
  (App. A); the runtime carries no domain-specific branches.

- **Twenty-five RocksDB column families plus an append-only audit log
  store every persistent state.** Every mutating subcommand writes a
  `dj_audit_log` row; every reading is a separate code path that
  cannot mutate the source of truth.

- **Action-conditioned latent prediction is verifiable end-to-end on a
  single RTX 5090.** Five real O\*NET-backed seeds of `paper_small_1k`
  finish in well under two hours; the integration FSV suite passes in
  190.92 s; the `release-c` gate (5 seeds + aggregate + container build
  + container verify + manual edges) fits in ~2 hours of wall time.

- **Multi-seed BCa CIs and paired sign-flip permutation tests are
  reported uniformly,** with the paper-cited number coming from a CSV
  in the aggregate bundle, not from a hand-typed table cell.

- **The runtime fails closed.** Eleven data-shape and CLI-shape edge
  cases per career bundle, six aggregate-level edges, and seven release-
  level edges all produced typed error codes during manual edge audits
  (§6) and *did not mutate the filesystem* unless a real bundle was
  being created.

## 9.2 What the Public-Taxonomy Benchmark Does Not Demonstrate

**`career_taxonomy.v1` is not a labor-market or recruiting benchmark.**
The walk generator deterministically expands O\*NET 30.2 occupations
along the related-occupation graph and synthesizes
state/action/outcome rows from the public taxonomy. The data manifest's
`provenance_notes` field reads verbatim:

> "Generated deterministically from O\*NET 30.2 downloadable text
> database files: Occupation Data, Job Zones, Skills, and Related
> Occupations. This is a public taxonomy benchmark, not real recruiting
> history."

(Source:
`tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_seed42/data_manifest.json`
field `sources[0].provenance_notes`.)

Treating the trained predictor as a model of real career outcomes
violates the artifact license and is explicitly disclaimed in the
README, the paper, and the data manifest.

## 9.3 Failure Modes Observed and Fixed

We document failure modes openly because they are part of the
artifact's evidence chain. Each was found through FSV and persisted
as a failed bundle:

1. **Seed-45 / Seed-46 A3 ablation gate failure** before
   `--career-ablation-training-seed` separation. Root cause: ablation
   training seed defaulted to the main career seed, so A3's
   negative-control no-action target became seed-sensitive at small
   data scale. Fix: ablations now stay on the source-fixture control
   seed unless explicitly overridden, and the run manifest records
   `career_ablation_training_seed_effective` and
   `_origin`. The two failed bundles are preserved as evidence
   ([reports/20260430_seed45_46_release_aggregate_fsv.md](../smokedocs/reports/20260430_seed45_46_release_aggregate_fsv.md)).

2. **All-pass aggregate read failure** because the empty
   `table_seed_failures.csv` (correct for an all-pass aggregate) was
   rejected by a generic CSV reader expecting at least one data row.
   Fix: allow empty rows only for that specific table; metric tables
   still fail closed when empty. The failed aggregate
   `multiseed_42_46_v3` is preserved.

3. **Inspection commands silently creating missing RocksDB paths.**
   Found during smoke-B FSV; `inspect-counts`/`inspect-cf` previously
   used the writer/open path and would `create_if_missing(true)` the
   target. Fix: read-only opens via
   `TeleologicalStoreConfig.create_missing_column_families = false`
   and a hidden `--allow-missing` flag for explicit before-state
   reads ([reports/20260501_local_smoke_b_tier_fsv.md](../smokedocs/reports/20260501_local_smoke_b_tier_fsv.md)).

4. **HP sweep `batch_size=32` proxy failure at 120 events.** Root
   cause: 120-event proxy was too small for the transition ablation
   path; the optimizer-step budget was not preserved across batch-size
   cells. Fix: HP sweeps now use the verified 1K real-O\*NET scale
   and scale epochs across batch-size cells (e.g. `batch_size=32` cells
   persist `epochs=2000` to keep step count constant). The failed
   sweep bundle is preserved.

5. **Ruby host verifier inside CUDA container.** Initial
   `reproduce.sh verify-aggregate` shelled out to a Ruby script; the
   CUDA base image has no Ruby. Fix: the verifier was moved into the
   compiled Rust CLI (`dynamicjepa check-release-aggregate-reference`).

6. **Container determinism failure on Docker iidfile compare.** Local
   image unpacking conflicts with `rewrite-timestamp` ordering. Fix:
   gate compares full deterministic Docker tar exports
   (`type=docker,rewrite-timestamp=true` with `SOURCE_DATE_EPOCH`).

7. **Container `libcuda.so.1` load failure without `--gpus all`.**
   Fix: release-C runs container verification under the NVIDIA
   container runtime with `--gpus all`.

8. **Stale release reference.** The first release reference recorded
   numbers from a pre-current implementation. Fix: the reference was
   recaptured from a preserved passed five-seed aggregate.

9. **Initial release-candidate (rc1) missing a packaged verifier
   binary.** Fix: the packager now requires `CONTEXT_GRAPH_BIN` and
   copies it into `bin/context-graph` with a recorded
   `manifests/context_graph_ldd.txt` dynamic-link readback.

10. **SIF passed without runtime verification.** Initial packager
    accepted any path called `*.sif`. Fix: packager now requires
    `singularity inspect --json`, `singularity sif list`, and
    `singularity exec --nv --cleanenv --no-home ... verify-aggregate`
    *before* it will include a supplied SIF.

# 10. Limitations

## 10.1 Modeling Limitations

- **Model size.** All trained models in this paper are tiny: 480
  (counter\_world), 16{,}960 (gridworld\_5x5), and 15{,}344 (career
  A1) trainable parameters. They are sufficient to demonstrate the
  runtime and the FSV protocol, but they are *not* state-of-the-art
  for any task. We make no claim that they would scale; that is
  future work. The 5090's 32~GiB VRAM allows much larger models, but
  the headline contribution is the audit chain, not raw capacity.

- **One step rollouts only.** $H=1$ action-conditioned prediction is
  the only mode trained. Multi-step ($H>1$) direct vs. autoregressive
  rollout is a deferred axis
  ([smokedoc 24](../smokedocs/24_multistep_rollout_and_planning_horizon.md)).

- **No decoder head.** The paper-small runtime is latent-only. Top-k
  occupation prediction and skill-vector reconstruction would require
  a decoder; that is a deferred axis
  ([smokedoc 16](../smokedocs/16_decoder_head_and_topk_metrics.md)).
  The "career-taxonomy" Table 1/2 numbers are *latent* MSE/cosine,
  not occupation-prediction accuracy.

- **Single hardware platform.** All numbers come from one RTX 5090
  host (sm\_120, CUDA 13.2, Rust 1.95). PyTorch's reproducibility
  guidance [PyTorch, 2026] reminds us that exact GPU-bit reproducibility
  across releases or platforms is not guaranteed; the release reference
  uses tolerance-based comparison rather than byte equality of weights.

## 10.2 Statistical Limitations

- **n = 5 seeds.** The paired sign-flip permutation test floor at
  $n=5$ is $p = 2/32 = 0.0625$, which sits above the conventional
  $\alpha = 0.05$ threshold. We therefore report deltas with BCa CI
  and explicit sign-flip $p$ rather than as "significantly different".
  A sample-size increase to $n=6$ would lower the floor to $p =
  0.03125$ and resolve the threshold; we have not done this in v1.0.

- **A0/A2/A3/A4/A5 stddev = 0 across seeds.** This is the deliberate
  consequence of the protocol's choice to fix the ablation training
  seed at the source-fixture control seed for all five main career
  seeds (only A1 varies the main career seed). It means the
  cross-seed variance reported for those rows is non-informative
  about generalization; the *across-ablations* comparison is what is
  informative.

- **No multiple-comparison correction.** A0–A5 are compared against
  A0 each, but A_i vs A_j for $i,j \neq 0$ are not multiple-tested.
  This is a deliberate design decision documented in
  [smokedoc 15](../smokedocs/15_multiseed_completion_and_statistical_reporting.md);
  the paper-safe consequence is that we never claim "A_i
  significantly outperforms A_j for $i,j \neq 0$".

## 10.3 Data Limitations

- **Public taxonomy ≠ real-person trajectories.** §4.5 elaborates;
  any real-data extension belongs in a separate Phase 12 with
  privacy gates ([smokedoc 25](../smokedocs/25_privacy_gate_real_data_phase12.md)).

- **U.S.-centric occupation taxonomy.** O\*NET's coverage reflects
  U.S. occupational structure; cross-national replication on ESCO
  is a deferred axis
  ([smokedoc 18](../smokedocs/18_esco_crosswalk_and_replication_study.md)).

- **Restricted occupation walk.** The current generator restricts to
  core computer occupations rooted at Software Developers
  `15-1252.00`. This is sufficient for the runtime contribution but
  is not coverage-balanced across O\*NET's 1{,}016 occupations or
  across O\*NET's job-zone distribution.

## 10.4 Reproducibility Limitations

- **No Zenodo DOI yet.** §8.5 documents the gap. The release archive
  is a *release candidate*; ACM "Available" badging requires a
  permanent DOI [ACM, 2020], which is the only remaining
  final-release blocker.

- **Release reference is host-specific.** Re-running `make reference`
  on a different sm\_120 / different driver / different
  `Cargo.lock` may produce a slightly different reference hash and
  is correctly rejected by the gate. Cross-host portability of the
  reference is *future work* requiring a controlled multi-host study.

- **No CI on a hosted runner.** The CI gate is intentionally local
  and self-hosted to keep evidence on disk and avoid GitHub-billed
  minutes [GitHub Docs, 2025]. A reviewer with no RTX 5090 access
  cannot run release-C; the artifact requires the documented
  hardware.

# 11. Ethics, Compliance, and Broader Impact

This work touches occupational prediction technology. We follow
NIST AI RMF Core [NIST, 2024], the NeurIPS Paper Checklist
[NeurIPS, 2024], and the project's compliance/ethics doc
([smokedoc 21](../smokedocs/21_compliance_ethics_broader_impact.md)).

## 11.1 What This Work Does *Not* Promise

Restate verbatim:

- **No recruiting claim.**
- **No prediction validity for individual humans.**
- **No production deployment guarantees.**
- **No compliance claim with any specific labor regulation.**
- **No fairness claim across protected classes.**
- **No Phase 12 privacy gate evidence (deferred).**

These appear in §4.5, in the release `README.md`, and in the
artifact DOI page upon publication.

## 11.2 Intended Use

Permitted:

- Research and educational replication of the runtime, the FSV
  protocol, and the public-taxonomy benchmark.
- Use as a reference architecture for auditable latent world-model
  runtimes in other domains (counter\_world, gridworld, new domain
  packs).
- Comparison baseline for future career-taxonomy benchmarks.

Not permitted:

- Real recruiting / hiring decisions.
- Career counseling tools that present predictions to end users
  without explicit "research demonstrator" disclosure.
- Compliance / labor-market regulatory reporting.
- Any use that relies on the public-taxonomy benchmark as a proxy for
  individual outcomes.

## 11.3 Dual-Use Disclosure

Career-trajectory inference at population scale could be used for
labor-market surveillance. Our artifact uses public taxonomies, not
subject-level data; the public-taxonomy benchmark cannot be retrained
on subject data without violating the artifact license and Phase 12
gate. A re-skinned tool could present model outputs as career
recommendations without disclaimers; the disclaimers are required in
README and any downstream that cites the work.

O\*NET / ESCO are constructed taxonomies and have their own gaps
(emerging occupations underrepresented). The paper does not claim to
correct or audit those gaps.

## 11.4 Risk Register

Order: highest residual risk first. "Residual" = after the listed
mitigations.

| Risk | Severity | Likelihood | Mitigation | Residual |
| --- | --- | --- | --- | --- |
| Misuse for hiring decisions | High | Low (small + disclaimed) | Explicit non-claim; intended-use list; abstract qualifier | Low |
| Career counseling deployment | Medium | Medium | License terms + README; no decoder for individual-level predictions | Low–Medium |
| Cherry-picking seeds | Medium | Low | Multi-seed BCa + paired tests required for any "significantly" claim; failed bundles preserved | Low |
| Silent metric drift in container | Medium | Low | CI weekly reference-hash check (when free CI is enabled) | Low |
| O\*NET attribution gap | Low | Medium | Attribution in README, paper, manifest, container build log | Low |
| Dual-use as labor-market surveillance | Medium | Low | Scope is taxonomy walks, not real subjects | Low |
| Reviewer cannot reproduce | Medium | Low | Apptainer + Docker + reference hash + diff report (§8) | Low |
| Reproducibility breaks on non-sm\_120 | Low | Medium | Document tested archs; provide expected metric variance | Low |

## 11.5 O\*NET License + Non-Endorsement Compliance

Concrete requirements (each is enforced):

| Requirement | Where Enforced |
| --- | --- |
| O\*NET attribution text in artifact | `README.md`, paper, every `data_manifest.json` |
| Non-endorsement clause verbatim | `README.md`, paper, container build log, `THIRD_PARTY_LICENSES/ONET.md` |
| Source ZIP SHA-256 recorded | `data_manifest.json`, paper §4.1 |
| Allowed-use field set | `data_manifest.json` field = `research-demo` |
| No suggestion that O\*NET endorses the work | review of README + paper before submission |

The verbatim O\*NET non-endorsement clause as recorded in
`data_manifest.json`:

> "O\*NET 30.2 Database, Creative Commons Attribution 4.0 International
> License (CC BY 4.0). Generated fixture modifies O\*NET database
> information for a public taxonomy benchmark; USDOL/ETA has not
> approved, endorsed, or tested these modifications."

## 11.6 NIST AI RMF Mapping (Condensed)

| Function | Sub-category | Evidence |
| --- | --- | --- |
| GOVERN | Policies, processes, procedures documented | smokedocs 00–25, project policies |
| GOVERN | Accountability for risk | maintainer named in `CITATION.cff`; this paper §11 |
| MAP | Context, sector, intended/foreseeable uses | §11.2 |
| MAP | Categorization | "research demonstrator, public-taxonomy benchmark" |
| MAP | Risks identified | §11.4 |
| MAP | Third-party data/software risk | §8 license + dependency lockfiles |
| MEASURE | Metrics for trustworthiness characteristics | §7 + collapse diagnostics CSV |
| MEASURE | Documentation of performance | every bundle's `paper_tables/`; multi-seed aggregate |
| MEASURE | Independent verification | §6 FSV protocol + manual edge audits |
| MANAGE | Risk responses prioritized | §11.4 register order |
| MANAGE | Risk treatment | failure-mode tests, fail-closed defaults, explicit non-claims |
| MANAGE | Continuous monitoring | doc-23 free local CI gate |
| MANAGE | Incident disclosure | failed-bundle preservation policy + worked seed-43 example |

## 11.7 Acknowledgments

The runtime builds on prior JEPA work [Assran et al., 2023; Bardes et
al., 2024] for the latent-prediction premise; on VICReg [Bardes et al.,
2022] for the variance/covariance regularization; on World Models
[Ha & Schmidhuber, 2018], Dreamer [Hafner et al., 2023], and MuZero
[Schrittwieser et al., 2019] for the planning-with-a-learned-model
framing; and on Perceiver IO [Jaegle et al., 2022] and Gato [Reed et
al., 2022] for the generality argument. The career-taxonomy substrate
uses O\*NET 30.2 [USDOL/ETA, 2026] under CC BY 4.0 with the
non-endorsement clause; ESCO [European Commission, 2025] is referenced
as a planned cross-walk source. The reproducibility chain follows
Docker reproducible-build guidance [Docker, 2025; 2024], Apptainer SIF
documentation [Apptainer, 2025], and ACM Artifact Review and Badging
[ACM, 2020] under MLflow Tracking [MLflow, 2024] artifact lineage
patterns. Statistical reporting follows BCa bootstrap [Efron &
Narasimhan, 2020] and PyTorch reproducibility caveats [PyTorch, 2026].
The compliance/ethics surface follows NIST AI RMF Core [NIST, 2024]
and NeurIPS Paper Checklist [NeurIPS, 2024].

No funding source is acknowledged; this is a single-author research
artifact (`CITATION.cff`).
