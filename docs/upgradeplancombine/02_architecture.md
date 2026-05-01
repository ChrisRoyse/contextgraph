# 02 — Unified Architecture

This document is the load-bearing block diagram for the combined
ME-JEPA × Meaning-Compression runtime. Every other design doc in this
directory refines one of the boxes drawn here.

## 1. The pre-upgrade flow (recap)

ME-JEPA today processes one event through this pipeline:

```
                                 +-------------------------+
   raw event (typed) ----------> | adapter validate        |
   (TOML domain pack +           | typed-error-on-bad-data |
   JSONL payload)                +-----------+-------------+
                                             |
                                             v
                            +----------------+----------------+
                            | normalised state s_t            |
                            | + action a_t + outcome o_t      |
                            | (typed RocksDB rows)            |
                            +----------------+----------------+
                                             |
                                             v
                            +----------------+----------------+
                            | instrument panel: phi_m(...)    |
                            | for m = 1 .. N                  |
                            | -> dj_instrument_readings (10K rows
                            |   per 1K career bundle, N=10)   |
                            +----------------+----------------+
                                             |
                                             v
                            +----------------+----------------+
                            | latent panel record             |
                            | -> dj_latent_panels             |
                            +----------------+----------------+
                                             |
            +------------- next-state target s_{t+1} ----+
            v                                            v
+-----------+----------+                  +--------------+--------+
| trajectory + dataset |                  | predictor P_psi       |
| -> dj_trajectories,  |---transition---->| trained on (s, a, o,  |
|    dj_dataset_shards |                  |  s') panels           |
+-----------+----------+                  +--------------+--------+
            |                                            |
            v                                            v
+-----------+--------------+              +--------------+----------+
| training run + artifact  | <----artifact| predictions:            |
| -> dj_training_runs,     |    hash      | dj_predictions          |
|    dj_model_artifacts    |              | (6 per bundle)          |
+--------------------------+              +--------------+----------+
                                                         |
                                                         v
                                          +--------------+----------+
                                          | plan trace + utility +  |
                                          | guard verdict (heur.)   |
                                          | -> dj_plan_traces +     |
                                          |    dj_guard_decisions   |
                                          +--------------+----------+
                                                         |
                                                         v
                                          +--------------+----------+
                                          | surprise event (if delta|
                                          |   exceeds threshold)    |
                                          | -> dj_surprise_events   |
                                          +-------------------------+

                              +-------------------------------------+
                              | dj_audit_log (every operation +     |
                              |   FSV verifier readback)            |
                              +-------------------------------------+
```

Total per-bundle persisted bytes are dominated by `dj_audit_log` and
`dj_instrument_readings`. The runtime guard verdict in
`dj_guard_decisions` is a utility-threshold heuristic per domain; no
frozen reference geometry exists.

## 2. The post-upgrade flow

The combined runtime adds three new boxes (drawn with `===`) and
augments two existing ones (drawn with `***`):

```
                                +-------------------------+
   raw event (typed) ---------->| adapter validate        |
                                +-----------+-------------+
                                            |
                                            v
                            +---------------+----------------+
                            | normalised state s_t           |
                            +---------------+----------------+
                                            |
                                            v
                            +---------------+----------------+
                            | instrument panel: phi_m(...)   |
                            | -> dj_instrument_readings      |
                            +---------------+----------------+
                                            |
                                            v
                            +===============+================+
                            | PAIRWISE FEATURE LAYER (new)   |
                            | rho_{j,k}(phi_j(x), phi_k(x))  |
                            | for 1 <= j < k <= N            |
                            | -> dj_pairwise_readings        |
                            |    (45 rows per career event   |
                            |     at N=10)                   |
                            +===============+================+
                                            |
                                            v
                            +---------------+----------------+
                            | latent panel + binding         |
                            | -> dj_latent_panels (extended  |
                            |   schema gains pairwise refs)  |
                            +---------------+----------------+
                                            |
            +------------- next-state target s_{t+1} ----+
            v                                            v
+-----------+--------+                       +-----------+----------+
| trajectory+dataset |                       | predictor P_psi      |
+-----------+--------+                       +-----------+----------+
            |                                            |
            v                                            v
            +========================================+   |
            | CONSTELLATION SET C_P (new, frozen)    |   |
            | one centroid per modality per domain   |   |
            |  -> dj_constellations                  |   |
            | constructed once at registration       |   |
            +================+=======================+   |
                             |                           |
                             v                           v
                   +---------+-----------+   +-----------+--------+
                   | THRESHOLD CALIB.    |   | predictions:       |
                   | per (domain,        |   | dj_predictions     |
                   |  modality, subject) |   |                    |
                   | -> dj_threshold_    |   |                    |
                   |    calibrations     |   |                    |
                   +---------+-----------+   +-----------+--------+
                             |                           |
                             v                           v
                +------------+---------------------------+--------+
                | *** GUARD UPGRADE ***                          |
                | combined verdict =                             |
                |   utility_pass(candidate) AND                  |
                |   forall m in M_d:                             |
                |     cos(phi_m(predicted_panel), c_m) >= tau_m  |
                | -> dj_guard_decisions (extended schema)        |
                +------------+-----------------------------------+
                             |
                             v
                +------------+-------------+
                | plan trace + selection   |
                | -> dj_plan_traces        |
                +------------+-------------+
                             |
                             v
                +------------+-------------+
                | surprise event           |
                | -> dj_surprise_events    |
                +--------------------------+

  +========================================================+
  | *** AUDIT LOG UPGRADE ***                              |
  | dj_audit_log gains signal_yield: u32 per row           |
  | aggregator reports MC ratio bundle-level               |
  +========================================================+
```

The same panel `{phi_m}` appears in three semantic roles, exactly as
PAPER.md §3.4 prescribes:

| Role | Where in pipeline | Output CF |
| --- | --- | --- |
| Labeller | At ingest, on every event | `dj_instrument_readings`, `dj_pairwise_readings` |
| Retriever | When a downstream search MCP tool is called | (existing search code paths; no schema change) |
| Runtime guard | At plan time, on each candidate's predicted panel | `dj_guard_decisions` (extended) |

No parameter is updated across the three roles. Constellation centroids
are computed once at constellation-build time (subcommand
`build-constellation`) and frozen.

## 3. The three byte-slices, post-upgrade

The v1 paper §7 introduced the three-slice anatomy of a bundle. The
combined runtime preserves the slicing and shifts the proportions
toward Slice 1.

| Slice | What's in it | Pre-upgrade share | Post-upgrade share |
| --- | --- | ---: | ---: |
| 1. Deterministic-from-public-source | raw events, normalized states, instrument readings, **pairwise readings (new)**, panels, transitions, dataset shards | $\sim$60 % | $\sim$78 % |
| 2. Tiny-model-output paired with observed targets | predictions, plan traces, guard cosine traces (new), surprise events | $\sim$8 % | $\sim$12 % |
| 3. Independent verifier-readback | audit log, FSV reports, edge-case rows, ablation rows, **constellation integrity / threshold calibration / pairwise MI rows (new)** | $\sim$32 % | $\sim$10 % |

(Numbers are rough heuristics for a 1{,}000-event career\_taxonomy
bundle and are formalised in `07_storage_schema_changes.md`.)

The shift is *toward* Slice 1 — the lowest-collapse-hazard slice —
because the pairwise readings dominate the new bytes and they are
deterministic functions of the source event.

## 4. The two paper-level objects

The combined runtime makes two top-level objects first-class:

**(a) The DDA-extended derived dataset $D'$.** For each event:
- $N$ per-instrument readings,
- $\binom{N}{2}$ pairwise interaction features,
- 1 trajectory link, 1 transition link.

For a career bundle: $|D'_{\text{career-1k}}| = 1000 \cdot (10 + 45)
+ \text{provenance} = 55{,}000$ DDA rows + audit log.

**(b) The constellation set $C_P$ per (domain, optional subject).** For
career\_taxonomy:
- $C_{\text{career.global}}$ = panel of $M_d = N = 10$ centroids built
  from the full 1{,}000-event corpus' instrument readings (one centroid
  per instrument);
- $C_{\text{career.cluster.<software,data,systems,management>}}$ = four
  cluster-specific subject-style constellations, each derived from the
  subset of events whose `occupation_cluster` matches the subject.

Subject-specific constellations are optional — the global one is
sufficient for the in-scope cases. Domain packs declare which
constellations are required via a new TOML section
`[[constellation.subjects]]` (specified in
`04_constellation_design.md`).

## 5. The runtime guard predicate, unified form

The combined runtime guard predicate per candidate action $a$ in plan
context $s_t$:

$$
\text{accept}(a)
  = \underbrace{\text{utility}(s_t, a) \geq u_{\min}}_{\text{existing utility verdict}}
  \;\wedge\;
  \underbrace{\forall m \in \mathcal{M}_d:\;\;
    \cos\!\bigl(\phi_m(\hat{s}_{t+1}^{(a)}),\; c_m^{(d, \text{subject})}\bigr)
    \geq \tau_m}_{\text{TCT $G_\tau$, new}}
$$

where:
- $\mathcal{M}_d$ is the set of guard-active modalities declared by the
  domain pack (default: all $N$ panel slots);
- $\hat{s}_{t+1}^{(a)}$ is the predictor's predicted next panel for
  candidate action $a$;
- $c_m$ is the frozen centroid for modality $m$ in the chosen
  constellation (global or subject-specific);
- $\tau_m$ is calibrated per-domain pre-deployment, persisted in
  `dj_threshold_calibrations`, never tuned on evaluation data.

Both verdicts and their inputs are persisted on the same
`dj_guard_decisions` row, so a reviewer can recompute the predicate
post-hoc from durable storage alone.

## 6. The MC ratio as a first-class metric

Every `dj_audit_log` row gains a `signal_yield: u32` field. The
aggregator command `dynamicjepa compute-mc-ratio` sums yields across
the bundle, divides by the raw-event count, and emits both:

- a single bundle-level MC ratio (number of structured signals per raw
  event), and
- a per-stage breakdown (ingest, panel, pairwise, predict, plan,
  audit).

For career\_taxonomy at 1{,}000 events, the expected aggregate is
$\sim$175 / event (Slice 1+2+3 combined), with $\sim$110 / event in
Slice 1 alone after the pairwise layer lands.

## 7. The three-role composition rule, formalised for ME-JEPA

PAPER.md §3.4 specifies the rule. The combined runtime states it as an
axiom of the codebase:

> **Axiom (Three-Role Identity).** For each domain $d$ and each
> instrument $m$ declared by domain $d$'s TOML manifest, every code
> path in the runtime that requires a vector projection of an event
> (whether for retrieval, labelling, predictor input, predictor target,
> or guard-time scoring) reads the *same* frozen function
> $\phi_m^{(d)}$. No code path is permitted to instantiate an alternate
> projection of the same name; no code path may update the parameters
> of $\phi_m^{(d)}$.

Enforcement: the existing `instrument_artifact_hash` field in
`dj_instrument_registry` already records the SHA-256 of each
instrument's weights at registration. The upgrade adds a per-bundle
FSV-10 check (`04_constellation_design.md` §5) verifying that *every*
record consumed during the bundle (instrument readings, pairwise
readings, constellation centroids, guard cosines) carries the same
`instrument_artifact_hash` for the matching $m$ — i.e., the panel did
not silently change identity mid-run.

## 8. Read-back contract

For each new persisted record, FSV requires that *exactly one* read-
only code path can recompute the record from the source of truth:

| New CF / field | Read-back path |
| --- | --- |
| `dj_pairwise_readings` | `inspect-cf --cf dj_pairwise_readings` decodes the (event, j, k) tuple; FSV-9 recomputes from `dj_instrument_readings`. |
| `dj_constellations` | `get-constellation --domain X --subject Y` reads back the centroid + LOO + reference-set manifest; FSV-10 re-runs Phase 1 on identical inputs and asserts $\lVert c_m^{\text{rebuild}} - c_m^{\text{persisted}}\rVert_2 < 10^{-5}$. |
| `dj_threshold_calibrations` | `get-threshold --domain X --modality Y` reads the persisted $\tau_m$ + percentile + $|S_m|$; FSV-11 verifies that the calibration set used was disjoint from training. |
| `dj_guard_decisions.cosine_to_centroid_per_modality` | Stored fp32 per modality; FSV-11 recomputes from `dj_predictions` + `dj_constellations`. |
| `dj_audit_log.signal_yield` | Aggregated by `compute-mc-ratio`; FSV-12 sums across the bundle and checks against the stage-level expected counts. |

These read-back contracts are the design constraint the rest of this
plan respects.
