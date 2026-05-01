# 15 — Risk Register and Open Questions

## 1. Risk register

Order: highest residual risk first. *Residual* = after listed
mitigation.

### R1 — Pairwise MI audit reveals heavy redundancy

| Field | Value |
| --- | --- |
| **Severity** | Medium-High |
| **Likelihood** | Medium |
| **Description** | The KSG audit on career\_taxonomy A1 returns $n_{\text{eff,strict}} < 0.85 \cdot N_d$, i.e. the panel is more redundant than Axiom 2 assumes; the constructive MC upper bound becomes a loose ceiling. |
| **Mitigation** | Honest disclosure: report measured $n_{\text{eff}}$ in the v1.1 paper update; tighten the constructive headline to "up to $S_1(d)$" with measured $S_1^{\text{eff}}(d)$ as the secondary number. The runtime contribution (the audit itself) is the answer to the question. |
| **Residual** | Low. The *measurement* is the contribution; the result direction does not invalidate the upgrade. |

### R2 — Constellation guard rejects too aggressively in early calibration

| Field | Value |
| --- | --- |
| **Severity** | Medium |
| **Likelihood** | Medium |
| **Description** | The 10th-percentile default $\tau$ is too high for some modalities, so $G_\tau$ rejects most candidates and `no_accepted_candidate` becomes common. |
| **Mitigation** | (a) Per-modality `calibration_percentile` override in TOML (default 10 keeps current behaviour). (b) The `g_tau_acceptance_curve.svg` plot makes the sensitivity visible. (c) `recalibrate-threshold` provides a documented re-calibration path with `supersedes_uuid`. |
| **Residual** | Low-Medium. Operators see the rejection rate immediately; tuning is per-domain. |

### R3 — Storage growth from pairwise readings

| Field | Value |
| --- | --- |
| **Severity** | Low-Medium |
| **Likelihood** | High (it WILL happen, by design) |
| **Description** | A 1{,}000-event career bundle gains 45{,}000 pairwise rows, ~30 % disk-size increase per bundle. Multi-domain bundles scale linearly. |
| **Mitigation** | RocksDB Lz4 compression is already on; pair rows compress well (mostly small floats + flags). Bundle size grows but stays well within the 7.4 GiB existing artifact archive's headroom. |
| **Residual** | Low. Quantitative impact reviewed in `07_storage_schema_changes.md` §6. |

### R4 — v2 schema bump breaks existing v1 reproducibility

| Field | Value |
| --- | --- |
| **Severity** | Medium |
| **Likelihood** | Low (by design) |
| **Description** | A reviewer with a v1 paper-small bundle attempts to reproduce with a v2 binary; gets a typed `BUNDLE_SCHEMA_VERSION_MISMATCH` and is confused. |
| **Mitigation** | Per CLAUDE.md no-backwards-compat rule, this is by design. The release artifact for v1 (`mejepa_5090_artifact_v1.0.0_rc5_sif_verified`) remains valid and reproducible from the v1 binary; the v2 release is a separate artifact with its own version number. The v2.0 release-D archive includes a `MIGRATION_FROM_V1.md` clearly explaining the relationship. |
| **Residual** | Low. The v1 artifact remains an immutable reference. |

### R5 — KSG estimator variance on small per-pair samples

| Field | Value |
| --- | --- |
| **Severity** | Medium |
| **Likelihood** | Medium |
| **Description** | KSG-1 with $k = 5$ has known bias on $n < 200$. Smoke bundles run KSG on $n = 120$ events; the result may vary 0.1–0.2 nats across re-runs. |
| **Mitigation** | (a) Smoke bundles run KSG only at `--sample-size 120` for completeness, not as paper-headline numbers. (b) Paper-small (1{,}000-event) bundles are the headline. (c) Bootstrap CI with 1{,}000 iters + fixed seed gives a documented uncertainty window. (d) MINE estimator option provides a cross-check. |
| **Residual** | Low. The paper headline is anchored at $n = 1{,}000$. |

### R6 — Subject-specific constellation LOO too low

| Field | Value |
| --- | --- |
| **Severity** | Medium |
| **Likelihood** | Medium |
| **Description** | Cluster-specific constellations built from $\sim 250$ events may have LOO $< 0.85$, especially if the cluster has high intra-class variance. `build-constellation` rejects with typed error. |
| **Mitigation** | (a) Docs make `--allow-low-loo` available with explicit override + provenance flag. (b) The TOML allows per-subject `require_loo_min` to be set lower. (c) Fallback: drop subject-specific constellation and use only the global one for affected domains. |
| **Residual** | Low-Medium. Operationally visible at construction time. |

### R7 — $G_\tau$ false-reject rate exceeds 10 % on calibration set

| Field | Value |
| --- | --- |
| **Severity** | Low |
| **Likelihood** | Medium |
| **Description** | The 10th-percentile $\tau$ is calibrated to admit 90 % of calibration-set events; new states produced by the predictor may sometimes fall below $\tau$ even for valid trajectories, generating spurious rejections. |
| **Mitigation** | The combined verdict ANDs $G_\tau$ with utility, so a $G_\tau$ rejection that retains utility-acceptance still produces a valid candidate (just with a recorded rejection in the audit). The `verdict` and `gtau_verdict` fields are independently logged. |
| **Residual** | Low. Logging captures the behaviour; it's a measurable rate, not a hidden defect. |

### R8 — Compute-budget overrun on release-D

| Field | Value |
| --- | --- |
| **Severity** | Low |
| **Likelihood** | Low |
| **Description** | Release-D's MI audit + 5-seed pairwise materialisation may exceed the v1 release-C 2-hour budget. |
| **Mitigation** | Estimated v2 release-D wall time is ~3 h (Phase 5 §6). The local CI gate is intentionally local; no cloud minutes are billed. The 5-seed bundles can run sequentially or in parallel (host has 30 logical cores). |
| **Residual** | Low. Wall time is bounded. |

### R9 — Determinism drift across re-runs of MI audit

| Field | Value |
| --- | --- |
| **Severity** | Low |
| **Likelihood** | Medium (KSG nearest-neighbour search non-determinism) |
| **Description** | KSG nearest-neighbour search may resolve ties non-deterministically; bootstrap resampling introduces additional variance. |
| **Mitigation** | (a) Fixed seed for bootstrap. (b) Fixed event ordering at audit time (input vector indices preserved). (c) Tolerance band recorded in `reference/tolerances_v2.0.json`. (d) Use deterministic tie-breaking in the nearest-neighbour search. |
| **Residual** | Low. Cross-run variation stays within tolerance. |

### R10 — Predictor schema change required for pairwise input

| Field | Value |
| --- | --- |
| **Severity** | Low (v2.0); Medium (v2.1) |
| **Likelihood** | Low (v2.0); Medium (v2.1) |
| **Description** | v2.0 keeps the predictor input layout unchanged (per `04_constellation_design.md` §10). v2.1 may add pairwise features as predictor input, requiring a one-time architecture change. |
| **Mitigation** | Defer to v2.1; preserve v1 baseline cosine numbers across v1↔v2 to verify the predictor was unchanged. v2.1 architecture change is a separate PR with its own ablation. |
| **Residual** | Low. Not in v2.0 scope. |

### R11 — Operator confusion: too many subcommands

| Field | Value |
| --- | --- |
| **Severity** | Low |
| **Likelihood** | Medium |
| **Description** | 36 subcommands is a lot. New operators may not know which to invoke first. |
| **Mitigation** | The orchestrator `research-smoke` runs every new subcommand automatically; manual invocation is for debugging only. The CLI `--help` output groups commands per `Table 2 of 04_method_system` style. |
| **Residual** | Low. The "happy path" stays one command. |

### R12 — Bundle-version handling in MCP server

| Field | Value |
| --- | --- |
| **Severity** | Low |
| **Likelihood** | Low |
| **Description** | The MCP server may not propagate the v1↔v2 schema-version mismatch error cleanly to the calling agent. |
| **Mitigation** | Standard MCP error mapping: typed errors return as JSON with the error code; the agent sees structured error output. |
| **Residual** | Low. |

## 2. Open implementation questions

These questions are flagged for resolution during the implementation
phase. Each has a default decision that the implementer can override
during PR review.

### Q1 — Should `compute-mc-ratio` accept the bundle root or the RocksDB path?

**Default:** Both. CLI args mutually exclusive. RocksDB path for
in-place audits; bundle root for retrospective bundle audits.

### Q2 — Should pairwise readings include the embedder version of *both* instruments in the row, or only j?

**Default:** Both. Storage cost is $2 \times 32 = 64$ extra bytes
per row; correctness is unambiguous.

### Q3 — Should LOO stability be computed in fp32 or fp64?

**Default:** fp32. Matches the rest of the panel; tolerance bands
are set accordingly. fp64 path can be added if Phase 5 audits show
LOO drift exceeding tolerance.

### Q4 — Should the calibration-set selector be a TOML-declared field or a CLI argument?

**Default:** TOML default + CLI override. The TOML
`[constellation.subjects].calibration_selector` is the documented
default; `--calibration-set-selector` overrides for ad-hoc audits.

### Q5 — Should `audit-pairwise-mi` run by default during Tier B (smoke-b)?

**Default:** No. KSG audit costs ~80 s on a paper-small bundle;
not worth the per-Tier-B latency. Tier B runs at smoke-bundle scale
($n = 120$); Tier D runs the full $n = 1{,}000$ audit. This matches
the existing tier discipline.

### Q6 — When a domain has no `[[constellation.subjects]]` declared, should `build-constellation` build a global constellation by default?

**Default:** Yes. counter\_world.v2 and gridworld\_5x5.v2 declare no
subjects but still get a global constellation built automatically. The
global constellation is required for the combined guard predicate to
have a target; declaring subjects is optional.

### Q7 — Should the v2 release artifact bundle the v1 release artifact for comparison?

**Default:** No. The v1 artifact (`mejepa_5090_artifact_v1.0.0_rc5_sif_verified`)
remains independently archived. The v2 README points to the v1
artifact for reviewers who want bit-equal v1 reproduction.

### Q8 — Should the v1.1 paper update be a separate arXiv submission or a v2 of the existing one?

**Default:** v2 of the existing manuscript. The combined runtime
strengthens the v1 claims rather than introducing different ones; an
arXiv revision is cheaper than a new submission and preserves the
v1 reference history.

### Q9 — Should the `audit-pairwise-mi` output write to a content-addressed location (sha256 of inputs)?

**Default:** No. The audit writes to `--output-dir` per-call and
records the seed in the JSON output. Content-addressing is
desirable but adds CFs and complicates the FSV chain; revisit in
v2.2.

### Q10 — Should the constellation centroid use FP32 + `f32::cos` or BF16?

**Default:** fp32 throughout the constellation pipeline. Mixed-
precision (bf16 generator + fp32 cosine) is the existing v1 rule
in `paper.tex` §3.4; v2 inherits and extends to the pairwise math
unchanged.

## 3. Locked decisions (decided in advance, not revisitable in PRs)

These decisions are codified across the design docs to prevent
implementer drift:

- **No backwards compatibility.** Per CLAUDE.md. v2 binaries refuse
  v1 bundles; v1 binaries refuse v2 bundles; no migration shims.
- **Schema version field on every persisted JSON manifest.** All
  bundle-level files include `bundle_schema_version: 2`.
- **Frozen embedders, no exception.** No code path may update
  $\phi_m$'s parameters. Adapters attach to the predictor, not the
  panel.
- **Constellations are immutable after build.** `recalibrate-threshold`
  supersedes the threshold but never the centroid; rebuilding a
  centroid produces a new `dj_constellations` row with a new uuid.
- **Pairwise readings are computed-on-write.** No lazy computation.
- **The combined guard is verdict := utility AND $G_\tau$.** Not OR,
  not weighted average.
- **The audit log signal-yield is a closed lookup table.** New
  operation kinds require an explicit yield definition; unknown
  operations contribute zero (and are flagged by FSV-12).
- **The MI audit is not a Tier B requirement.** Audit lives in Tier
  D / release-D only.
- **The cross-domain transfer pilot is v2.1.** Not v2.0.

## 4. Decisions deferred to v2.1+

Explicitly out of scope:

- Phase-2 conditioning ($\mathcal{L}_{\mathrm{TCT}}$ in PAPER.md §5.3
  — predictor trained against constellation).
- Best-of-K resampling on $G_\tau$ rejection.
- Cross-encoder triangulation (no second encoder family in v2.0
  scope).
- ESCO crosswalk (smokedoc 18).
- Multi-step rollouts (smokedoc 24).
- Decoder head + top-k metrics (smokedoc 16).
- Real subject-level data (smokedoc 25).
- Higher-order MI ($k$-tuple, $k > 2$).
- Cross-event temporal MI.
- Conditional MI given action / outcome.

These are tracked in the v2.1+ axes list (16_paper_v1_1_outline.md
§9).
