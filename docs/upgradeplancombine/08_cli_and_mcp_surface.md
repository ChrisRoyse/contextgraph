# 08 — CLI and MCP Surface Additions

This document specifies the new CLI subcommands and matching MCP tool
endpoints. The existing 29 subcommands are preserved verbatim; six new
subcommands are added (and the `research-smoke` orchestrator is
extended to call them automatically).

## 1. New subcommands (CLI)

The `crates/context-graph-cli/src/commands/dynamicjepa.rs`
`DynamicJepaCommands` enum gains six variants:

| New variant | Operation kind | Purpose |
| --- | --- | --- |
| `BuildConstellation(BuildConstellationArgs)` | `dynamicjepa_build_constellation` | Phase-1 centroid construction (§4 of `04_constellation_design.md`) |
| `ListConstellations(ListConstellationsArgs)` | `dynamicjepa_list_constellations` | Iterate `dj_constellations` |
| `GetConstellation(GetConstellationArgs)` | `dynamicjepa_get_constellation` | Single-row read by (domain, subject, modality) |
| `CalibrateThreshold(CalibrateThresholdArgs)` | `dynamicjepa_calibrate_threshold` | Same-session-band 10th-percentile calibration |
| `ReCalibrateThreshold(ReCalibrateThresholdArgs)` | `dynamicjepa_recalibrate_threshold` | Supersedes a calibration row with a new one |
| `AuditPairwiseMi(AuditPairwiseMiArgs)` | `dynamicjepa_audit_pairwise_mi` | KSG / MINE pairwise MI audit (§3 of `10_pairwise_mi_audit.md`) |
| `ComputeMcRatio(ComputeMcRatioArgs)` | `dynamicjepa_compute_mc_ratio` | Aggregate `signal_yield` over the audit log |

Final subcommand surface count: 29 + 7 = 36.

## 2. Argument signatures

### 2.1 `build-constellation`

```text
context-graph dynamicjepa build-constellation \
  --db <path> \
  --domain <domain_id> \
  --domain-version <semver> \
  --subject <subject_id> \                # e.g. "global", "cluster.software"
  [--require-loo-min <f32>] \
  [--allow-low-loo] \                     # records override in provenance
  [--source-event-selector <field>=<value>] \
  --json
```

Constraint: `--source-event-selector` must match a TOML
`[[constellation.subjects]]` block in the running domain pack, or
must be `subject_id=global` for the always-on global subject.

Output (JSON):

```json
{
  "status": "ok",
  "constellation_uuid": "...",
  "domain_pack_id": "...",
  "subject_id": "cluster.software",
  "modalities": [
    {
      "modality_id": 1,
      "centroid_head": [0.123, ...],     // first 8 dims for sanity
      "centroid_tail": [..., 0.045],     // last 8 dims for sanity
      "kept_count": 412,
      "dropped_zero_norm": 0,
      "loo_stability": 0.91,
      "instrument_artifact_hash": "sha256:..."
    },
    ...
  ],
  "audit_id": "..."
}
```

### 2.2 `list-constellations`

```text
context-graph dynamicjepa list-constellations \
  --db <path> \
  [--domain <domain_id>] \
  [--subject <subject_id>] \
  [--limit N] \
  [--offset N] \
  --json
```

Lists `dj_constellations` rows; defaults to all constellations in the
DB.

### 2.3 `get-constellation`

```text
context-graph dynamicjepa get-constellation \
  --db <path> \
  --domain <domain_id> \
  --subject <subject_id> \
  --modality <modality_id> \
  [--include-vector] \                  # include the full centroid (default: head + tail only)
  --json
```

### 2.4 `calibrate-threshold`

```text
context-graph dynamicjepa calibrate-threshold \
  --db <path> \
  --domain <domain_id> \
  --subject <subject_id> \
  --modality <modality_id> \
  --calibration-set-selector <field>=<value> \   # disjoint from training
  [--percentile <0..100>] \              # default 10
  --json
```

Reads the existing constellation centroid + the calibration set's
events, computes per-event cosines, sets $\tau_m$ to the chosen
percentile, persists `dj_threshold_calibrations` row.

Refuses if the calibration set overlaps the constellation's reference
set: typed error `CALIBRATION_SET_NOT_DISJOINT`.

### 2.5 `recalibrate-threshold`

```text
context-graph dynamicjepa recalibrate-threshold \
  --db <path> \
  --domain <domain_id> \
  --subject <subject_id> \
  --modality <modality_id> \
  --supersedes-uuid <calibration-uuid> \
  --calibration-set-selector <field>=<value> \
  [--percentile <0..100>] \
  --reason <free-form-string> \           # required, recorded
  --json
```

Writes a new `dj_threshold_calibrations` row with `supersede_seq` =
previous + 1, references `--supersedes-uuid`, requires
`--reason`. The original row is *not* deleted.

### 2.6 `audit-pairwise-mi`

```text
context-graph dynamicjepa audit-pairwise-mi \
  --db <path> \
  --domain <domain_id> \
  [--sample-size N] \                  # default 1000 events
  [--estimator ksg|mine] \             # default ksg
  [--ksg-k N] \                        # default 5 nearest neighbours
  [--mine-iters N] \                   # default 5000 if MINE
  [--seed N] \                         # bootstrap seed
  --output-dir <path> \                # writes JSON + CSV + heatmap SVG
  --json
```

Reads up to `sample-size` events, retrieves their per-instrument
readings, estimates pairwise MI using the chosen estimator, writes
the artifacts, and reports the $N_{\text{eff}}$ summary.

### 2.7 `compute-mc-ratio`

```text
context-graph dynamicjepa compute-mc-ratio \
  --db <path> \
  --domain <domain_id> \
  [--bundle-root <path>] \             # if reading from an immutable bundle
  --output-dir <path> \                # writes table_mc_ratio.csv
  --json
```

Aggregates `dj_audit_log.signal_yield` by `operation_kind`, then
computes `MC = total_signals / event_count`. Outputs the per-stage
breakdown plus the bundle-level total and the per-domain expected
upper bound from `03_signal_yield_math.md`.

## 3. New MCP tools

Each new subcommand has a matching MCP tool defined in
`crates/context-graph-mcp/src/handlers/tools/dynamicjepa_tools.rs`:

| MCP tool name | Underlying CLI |
| --- | --- |
| `mcp__context-graph__dj_build_constellation` | `build-constellation` |
| `mcp__context-graph__dj_list_constellations` | `list-constellations` |
| `mcp__context-graph__dj_get_constellation` | `get-constellation` |
| `mcp__context-graph__dj_calibrate_threshold` | `calibrate-threshold` |
| `mcp__context-graph__dj_recalibrate_threshold` | `recalibrate-threshold` |
| `mcp__context-graph__dj_audit_pairwise_mi` | `audit-pairwise-mi` |
| `mcp__context-graph__dj_compute_mc_ratio` | `compute-mc-ratio` |

Following the pre-existing MCP convention:
- Each tool's input JSON schema mirrors the CLI args verbatim.
- Each tool's output is the same JSON the CLI emits with `--json`.
- `additionalProperties: false` on every input schema (per memory
  lesson `additionalProperties:false blocks unlisted params`).
- Inspection tools (`list-`, `get-`) use strict `allow_missing=false`
  per memory lesson `read-only inspection commands no longer create
  missing RocksDB paths`.

## 4. Extensions to `research-smoke`

The orchestrator gains seven new stages to populate the new CFs and
write the new evidence. Every stage runs end-to-end without operator
intervention; no new flags required for the happy path.

```text
research-smoke (extended):
  1. (existing) preflight ONet ZIP
  2. (existing) register domain packs
  3. (existing) ingest events + run adapters
  4. (existing) materialize panels
  4.5 (new)    materialize pairwise readings (compute-on-write extension
               of step 4)
  5. (existing) compile trajectories + dataset shards
  6. (existing) train predictor
  7. (new)     build-constellation for each (domain, subject) pair
               declared by domain TOML
  8. (new)     calibrate-threshold for each modality of each
               constellation
  9. (existing) predict + plan + record-surprise — uses new combined
               guard
  10. (new)     audit-pairwise-mi (sample 100 events for each domain;
               full audit deferred to release-D)
  11. (existing) verify-* (extended FSV-9 to FSV-12)
  12. (new)     compute-mc-ratio
  13. (existing) write paper tables + plots (extended with the four
               new tables and three new plots)
```

Stages 7, 8, 10, 12 are gated by `--bundle-schema-version 2`; passing
`--bundle-schema-version 1` reproduces the v1 behaviour for
backward-debuggability (v1 binaries are still preferred for v1
bundles, per `07_storage_schema_changes.md` §9).

## 5. Backward-compatibility on the CLI

The 29 existing subcommands are preserved with identical signatures.
Per CLAUDE.md no-backwards-compat:
- Existing subcommands operating on v1 bundles continue to do so.
- Existing subcommands operating on v2 bundles see the extended
  schemas (bigger `dj_audit_log` rows, bigger `dj_guard_decisions`
  rows). Existing readers in the codebase deserialise the new fields
  as `Option::None` when present in v1 / `Some(...)` when present in
  v2; serde's default-on-missing handles this without code change in
  v1 readers.

The reverse direction (v1 binaries reading v2 bundles) fails closed at
the manifest-version check.

## 6. New schemas for the wrapper guards

`research-smoke` already has wrapper-level fail-closed checks (e.g.,
`paper_small` without `--onet-text-zip` rejects). Three new wrapper
checks land:

| Wrapper edge | Trigger | Typed error |
| --- | --- | --- |
| Schema version mismatch | `--bundle-schema-version` $\notin \{1, 2\}$ | `WRAPPER_SCHEMA_VERSION_INVALID` |
| Constellation declared without instrument support | TOML `[[constellation]]` references a missing instrument id | `WRAPPER_CONSTELLATION_INSTRUMENT_MISSING` |
| Calibration-set selector overlap | `calibrate-threshold --calibration-set-selector` matches events used to build the centroid | `WRAPPER_CALIBRATION_SET_NOT_DISJOINT` |

These add to the existing 11-edge career bundle matrix, bringing the
total to 26 edges + 3 wrapper edges = 29 verified edges per career
bundle (or 11 + 6 + 9 + 3 = 29 — same count by another decomposition).

## 7. Help-text discipline

Every new subcommand's `--help` text:
- Names the operation kind verbatim (matches `audit_log` rows).
- Names the CFs the subcommand reads / writes.
- Names the FSV harness that audits the subcommand's output.
- Names typed errors raised on edge cases (with a one-line summary).

This is the "operator can debug from `--help` alone" discipline the
existing subcommands already follow.

## 8. Open questions left for the implementation phase

- **Concurrency**: should `build-constellation` for multiple
  modalities run in parallel? (Yes, per modality; row writes batched
  into a single WriteBatch.)
- **Schema migration helper**: is there a zero-arg `verify-schema-version`
  subcommand for the bundle? (Yes; deferred to Phase 1 of
  `13_implementation_phases.md`.)
- **MCP-tool rate limits**: does the audit-pairwise-mi MCP tool need
  a max-sample-size cap? (Yes; cap at 5{,}000 events per call; full
  audits go via CLI in CI.)
- **Default percentile for calibration**: 10 across the board, or
  per-modality? (10 by default; per-modality override via TOML
  `calibration_percentile` field; Phase 3 fixes per-domain choice.)

These are tracked in `15_risks_and_open_questions.md`.
