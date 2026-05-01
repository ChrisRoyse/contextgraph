# 04 — Constellation Design ($C_P$ Object, Phase-1 Protocol, Persistence)

This document specifies the new constellation object, how it is
constructed, how it is persisted in RocksDB, and how its integrity is
verified.

## 1. The $C_P$ object

A constellation $C_P$ is an immutable, frozen, unit-norm centroid set
defined per (domain, optional subject). Mathematically, following
PAPER.md §5.1:

$$
C_P \;=\; \bigl\{(m, c_m, h_m, \text{loo}_m) : m \in \mathcal{M}_d \bigr\},
\qquad
c_m \in \mathbb{S}^{d_m - 1},
$$

where:
- $m$ is a modality (= one of the panel slots $1, \dots, N_d$);
- $c_m$ is the L2-normalised mean of the L2-normalised per-sample
  reference projections under $\phi_m$;
- $h_m$ is the SHA-256 of $\phi_m$'s frozen weights at the time the
  centroid was built (carried verbatim from
  `dj_instrument_registry.instrument_artifact_hash`);
- $\text{loo}_m$ is the leave-one-out cosine stability score
  (`build-constellation` rejects $\text{loo}_m < 0.85$ with a typed
  error unless `--allow-low-loo` is passed and recorded in provenance).

Two constellation kinds ship in the upgrade:

**Global constellation** ($\text{subject} = \texttt{<domain>.global}$).
Built from every normalised state in the domain's bundle. One per
domain. Used as the default $G_\tau$ target unless a subject-specific
constellation is declared.

**Subject-specific constellation** (e.g.\
$\text{subject} = \texttt{career\_taxonomy.cluster.software}$). Built
from the subset of events whose declared categorical field equals the
subject. Used when the domain's TOML declares a
`[[constellation.subjects]]` block.

## 2. The TOML domain-pack extension

Domain packs gain an optional `[[constellation]]` section. Backwards
incompatible per CLAUDE.md (no migration shims); the schema-version
bump from `1` to `2` is signalled by the new top-level field
`schema_version = 2`.

Example for `career_taxonomy.v2.toml`:

```toml
[domain]
id = "career_taxonomy"
version = "2.0.0"          # schema-version bump
title  = "Career Taxonomy - Combined ME-JEPA + DDA Benchmark"
schema_version = 2

# ... existing [time], [[state.fields]], [[action.fields]],
#     [[outcome.fields]], [[entity.fields]] unchanged ...

[constellation.global]
modalities = "all"          # or list specific instrument slots
require_loo_min = 0.85
calibration_percentile = 10  # 10th-percentile floor of same-session band
calibration_set_size = 100   # disjoint from training corpus

[[constellation.subjects]]
id = "cluster.software"
field = "occupation_cluster"
value = "software"
modalities = "all"
require_loo_min = 0.85

[[constellation.subjects]]
id = "cluster.data"
field = "occupation_cluster"
value = "data"
modalities = "all"
require_loo_min = 0.85

[[constellation.subjects]]
id = "cluster.systems"
field = "occupation_cluster"
value = "systems"
modalities = "all"
require_loo_min = 0.85

[[constellation.subjects]]
id = "cluster.management"
field = "occupation_cluster"
value = "management"
modalities = "all"
require_loo_min = 0.85
```

The validator rejects:
- Subjects whose `field` is not in `[[state.fields]]`.
- Subjects whose `value` is not in the field's declared `variants`
  list.
- Constellation declarations without a `require_loo_min`.
- Schema version mismatches between the TOML and the runtime.

## 3. Phase-1 construction protocol

Triggered by the new `dynamicjepa build-constellation` subcommand
(specified in `08_cli_and_mcp_surface.md`). The protocol is:

```text
INPUT:
  domain_pack_id (uuid)
  subject_id (str, e.g. "global" or "cluster.software")
  reference event-set selector (TOML field+value or "all")

PROCEDURE for each modality m in M_d:
  1. SELECT all events e_i in the running RocksDB whose normalised
     state matches the selector. Reject if count < 5.
  2. READ phi_m(e_i) from dj_instrument_readings for each i. Reject
     if any reading is missing (FSV-3 already prevents this).
  3. Drop any reading with ||phi_m(e_i)||_2 < 1e-6 ("zero-norm
     reject"). Persist the count of dropped readings.
  4. Per-sample L2-normalise: tilde_v_i := phi_m(e_i) / ||phi_m(e_i)||_2.
  5. Mean: bar_v := (1 / kept_count) * sum_i tilde_v_i.
  6. L2-normalise the mean: c_m := bar_v / ||bar_v||_2.
  7. Compute LOO stability:
        loo_m := (1 / kept_count) * sum_i cos(c_m^{-i}, c_m)
     where c_m^{-i} is the centroid recomputed dropping sample i.
     Reject (typed error LOO_BELOW_THRESHOLD) if loo_m < require_loo_min.
  8. PERSIST: dj_constellations row keyed by
     (domain_pack_id, subject_id, modality_id) with value
     ConstellationCentroid {
       centroid: Vec<f32>,                         // L2-normalised
       instrument_artifact_hash: [u8; 32],
       reference_set_count: u32,
       kept_count: u32,
       loo_stability: f32,
       calibration_percentile: u8,
       calibration_set_size: u32,
       built_at_unix_ms: u64,
       built_by_run_id: Option<RunId>,
     }
  9. PERSIST: corresponding dj_audit_log row with
     operation_kind = "build_constellation".

OUTPUT (machine-readable):
  per-modality centroid (truncated head + tail), kept_count, dropped
  count, loo_stability, instrument_artifact_hash, dj_constellations row
  uuid.
```

The `built_by_run_id` field is optional so a constellation built from
a smoke bundle can be reused across paper-small bundles for the same
domain pack version.

## 4. Persistence: `dj_constellations` CF

Storage details specified in detail in `07_storage_schema_changes.md`;
summary here:

- **Key:** `[domain_pack_uuid (16 B)][subject_id_hash (16 B)][modality_id (4 B LE)]`
- **Value:** `ConstellationCentroid` serialised JSON (per V1 lesson:
  bincode + skip\_serializing\_if breaks; JSON is the workspace
  default).
- **Read paths:**
  - `dynamicjepa get-constellation --domain X --subject Y` — primary
    operator-facing.
  - `inspect-cf --cf dj_constellations --limit N` — generic.
  - Internal: `context_graph_storage::dynamicjepa::get_constellation()`
    helper.

## 5. FSV-10: Constellation integrity

A new FSV harness runs after every `build-constellation` invocation
and again at end-of-bundle:

1. Re-read the persisted `dj_constellations` row.
2. Re-load all reference events from `dj_normalized_states` using the
   recorded selector (TOML field+value).
3. Re-read all `dj_instrument_readings` for those events.
4. Re-run the centroid construction (steps 4–6 of §3).
5. Verify $\lVert c_m^{\text{rebuild}} - c_m^{\text{persisted}}\rVert_2
   < 10^{-5}$.
6. Verify `instrument_artifact_hash` recorded matches current
   `dj_instrument_registry`.
7. Recompute LOO stability and verify within $\pm 10^{-4}$ of the
   persisted value.

If any check fails, write a typed FSV failure row
(`fsv_check = "constellation_integrity"`) and surface a non-zero
return code. Record the result either way in the bundle's
`fsv_report.json`.

## 6. Threshold calibration (`dj_threshold_calibrations` CF)

Each constellation row in `dj_constellations` is paired with one
`dj_threshold_calibrations` row per modality. The calibration follows
PAPER.md §5.4 same-session-band protocol:

- Collect a **disjoint calibration set** $S_m$ (events not used to
  build $c_m$, not used in the bundle's training shard) with
  $\lvert S_m \rvert \ge 50$ for any subject's
  `calibration_set_size` field, default 100.
- Compute $s_{\text{cal}, i} := \cos(\phi_m(s_i), c_m)$ for each
  $s_i \in S_m$.
- Set $\tau_m := $ `calibration_percentile`-th percentile of
  $\{s_{\text{cal},i}\}$ — default 10th-percentile, nominal 10 % false
  reject.
- Persist:

```text
ThresholdCalibration {
  domain_pack_uuid: [u8; 16],
  subject_id_hash: [u8; 16],
  modality_id: u32,
  tau: f32,
  percentile: u8,
  calibration_set_count: u32,
  calibration_event_uuids_sample: Vec<[u8;16]>,  // first 16 for audit
  calibration_set_disjoint_proof: bool,           // verified by FSV-11
  calibrated_at_unix_ms: u64,
}
```

- **Calibration discipline rule (CLAUDE.md "no fallback"):**
  $\tau_m$ is **never** retuned on evaluation data. Subsequent
  adjustment requires a typed `re-calibrate-threshold` subcommand,
  which writes a *new* row with a `supersedes_uuid` field; the old row
  remains in the bundle so audit cannot lose history.

## 7. Domain-pack defaults

Each shipped domain pack version gets a default constellation set:

| Domain | Default constellation(s) |
| --- | --- |
| `counter_world.v2` | `global` only; $N=2$, no subject splits. |
| `gridworld_5x5.v2` | `global` only; $N=3$. |
| `career_taxonomy.v2` | `global`, `cluster.software`, `cluster.data`, `cluster.systems`, `cluster.management` (5 constellations × 10 modalities = 50 centroid rows). |

Smoke and paper-small bundles run `build-constellation` for **all
declared subjects** as part of the existing `research-smoke` flow.

## 8. Provenance and source-of-truth chain

For every constellation row, the chain back to source:

```
public O*NET ZIP (sha256 b5479271…7453741)
  -> deterministic walk csv (per-edge hash)
    -> dj_raw_events
      -> dj_normalized_states (subject-filtered selector applies here)
        -> dj_instrument_readings (frozen phi_m, hash recorded)
          -> dj_constellations (centroid = mean of normalised readings)
            -> dj_threshold_calibrations (10th-percentile of disjoint calibration)
```

Every link in this chain is a single CF lookup. FSV-10 + FSV-11
together verify every link from `dj_constellations` and
`dj_threshold_calibrations` back to the source ZIP.

## 9. Edge cases (typed-error catalog)

The new construction path adds these typed errors to the edge-case
matrix in `paper_tables/table_edge_cases.csv`:

| Edge | Trigger | Typed error code |
| --- | --- | --- |
| Reference set too small | <5 matching events for the subject | `CONSTELLATION_REFERENCE_SET_TOO_SMALL` |
| Zero-norm rejections > 50 % | majority of $\phi_m(e_i)$ have norm < $10^{-6}$ | `CONSTELLATION_ZERO_NORM_MAJORITY` |
| LOO below threshold | $\text{loo}_m <$ `require_loo_min` | `CONSTELLATION_LOO_BELOW_THRESHOLD` |
| Instrument hash mismatch | `dj_instrument_registry` artifact hash differs at build vs read | `CONSTELLATION_INSTRUMENT_HASH_MISMATCH` |
| Calibration set overlap | calibration set shares events with reference set | `CALIBRATION_SET_NOT_DISJOINT` |
| Subject field undeclared | TOML `field` not in `[[state.fields]]` | `CONSTELLATION_SUBJECT_FIELD_UNDECLARED` |
| Subject value undeclared | TOML `value` not in field's `variants` | `CONSTELLATION_SUBJECT_VALUE_UNDECLARED` |
| Schema mismatch | TOML `schema_version != 2` while runtime expects 2 | `CONSTELLATION_SCHEMA_VERSION_MISMATCH` |
| Re-calibration without supersession | `re-calibrate-threshold` without `--supersedes-uuid` | `CALIBRATION_SUPERSESSION_REQUIRED` |

These nine new edge cases extend the existing 11-case career-bundle
edge matrix to 20.

## 10. What downstream code consumes constellations

The new guard predicate (`06_guard_upgrade_g_tau.md`) is the primary
consumer. Secondary consumers under the upgrade:

- The retrieval search code path is *unchanged* — search continues to
  use raw instrument similarity; the constellation is exclusively a
  *guard* object, not a *retrieval* object.
- The predictor training loop is *unchanged* in v2.0 of this upgrade
  — the constellation is **not** used as a Phase-2 loss term yet.
  Phase-2 conditioning ($\mathcal{L}_{\mathrm{TCT}}$ from PAPER.md
  §5.3) is a deliberate v2.1 axis, not v2.0; this preserves the
  existing predictor training behaviour and isolates the constellation
  guard's contribution to the runtime guard slot only.
- The MCP tools `mcp__context-graph__compile_constellation` and
  `mcp__context-graph__derive_constellation` already exist in the
  workspace for an unrelated (Context Graph–side) constellation
  notion; the upgrade names the new ME-JEPA constellation
  consistently to avoid collision (`dj_constellation_*` prefixes
  throughout).

Why no Phase-2 conditioning in v2.0: keeps the changeset reviewable;
preserves the existing v1 multi-seed numbers as a clean baseline; and
makes the *runtime guard upgrade* the only behavioural change in v2.0
so its contribution to bundle quality is isolable.
