# 27 — API Stability + Versioning

Source files covered: `docs/futurebuild/24_API_STABILITY.md`, `crates/context-graph-mejepa/src/provenance.rs`, `crates/context-graph-mejepa/src/types.rs`, `crates/context-graph-mejepa-cf/src/lib.rs`.

> Semver + deprecation cycle + replay reproducibility for MCP tools, `RealityPrediction` schema, and RocksDB CFs.

---

## 1. Semver policy

Pre-1.0 (current): **schema changes wipe DB** per `memory/feedback_no_backwards_compat.md`. No migration shims. Synthetic data only.

Post-1.0 (futurebuild `TASK-API-002`): semver with:

| Version bump | Cause |
|---|---|
| MAJOR | Breaking change to a stable MCP tool signature; removal of a CF; removal of a head from `RealityPrediction` |
| MINOR | New MCP tool; new optional field on `RealityPrediction`; new CF; new prediction class enum variant |
| PATCH | Bug fix; performance improvement; doc-only |

---

## 2. MCP tool stability

Per `TASK-API-001`. Tools fall into three classes:

| Class | Tools | Stability |
|---|---|---|
| **Stable** | `mejepa_verify`, `mejepa_predict_latest`, `mejepa_record_agent_feedback`, `mejepa_subscriber_status`, `mejepa_heal_status`, `mejepa_quota_status`, `mejepa_eval_run`, `mejepa_ship_gate_status`, `mejepa_active_learning_queue`, `mejepa_constellation_inspect`, `optimizer_record_recommendation` | MAJOR bump to break |
| **Beta** | `mejepa_predict_what_if`, `mejepa_explain_prediction`, `mejepa_observe_shift`, `mejepa_capture_audit`, all futurebuild additions | MINOR bump to break in dev cycles |
| **Internal** | `optimizer_witness_chain_verify`, `optimizer_witness_chain_diff`, `_repair_legacy`, `optimizer_compute_influence`, `optimizer_bandit_*` | no stability guarantee |

---

## 3. Schema extension policy

Per `RealityPrediction`:

- **Adding fields**: always additive (new optional field with default `[]` or `None`). Old callers see same response shape + new fields they can ignore.
- **Adding enum variants**: additive. Old serialized values still deserialize (Rust's `#[serde(other)]` catches new variants).
- **Removing fields**: requires MAJOR bump + deprecation cycle (mark `#[deprecated]` for 2 minor versions before removal).
- **Renaming fields**: forbidden; add new field + deprecate old.
- **Reshaping a field's type**: requires MAJOR bump.

Per `RocksDB CF`:

- **Adding a CF**: additive; `create_missing_column_families=true` handles first-start.
- **Removing a CF**: data orphaned on disk (RocksDB tolerates extra CFs); MAJOR bump.
- **Changing CF key/value layout**: pre-1.0 wipe; post-1.0 requires migration tool (`TASK-API-002`).

---

## 4. `provenance.predictor_version`

Every `RealityPrediction` and `TrainingCertificate` carries:

```rust
pub struct PredictionProvenance {
    pub predictor_version: String,        // "v0.4.2-bf16-cuda13.2"
    pub constellation_version: String,    // CF_MEJEPA_CONSTELLATION's version_id
    pub calibration_version: String,      // conformal calibration version
    pub active_pointer: String,           // weights SHA
}
```

Clients that need cross-prediction version comparison use these as version keys. Replay tool (`TASK-OBS-011-prediction-replay-tool`, futurebuild) reproduces a prediction byte-for-byte from `(predictor_version, calibration_version, constellation_version, active_pointer)`.

---

## 5. `code_version` provenance

Every persistent artifact (panel, training cert, prediction, agent feedback) binds:

```jsonc
{
  "code_version": "git-sha-<hex>",
  "frozen_at": "2026-05-11T22:05:00Z",
  "corpus_sha": "<sha>",
  "embedder_versions": { "E1": "...", "E7": "...", ... },
  "witness_hash": "shake256:73:..."
}
```

This enables:
- Byte-equal reproduction of a past prediction given the same `(code_version, embedder_versions, corpus_sha, calibration_version, constellation_version, active_pointer)` tuple.
- Forensic root-cause: "what code version produced this prediction?"

---

## 6. Deprecation cycle (post-1.0)

```
Version N:        feature ships; stable
Version N+1:      deprecation announced via #[deprecated] attribute + docs warning
Version N+2:      deprecation continues; emit runtime warning when called
Version N+3:      removed (MAJOR bump)
```

For MCP tools: stub remains in place returning `MEJEPA_*_DEPRECATED` for at least one minor version before removal.

---

## 7. Backward-compatibility shim policy

Per `memory/feedback_no_backwards_compat.md`: **NEVER add backwards-compat shims unless operator explicitly requests.**

Pre-production. Synthetic data only. Wipe DB on schema change. Never write migration shims.

Post-1.0 (when shims become necessary): each shim is `TASK-API-NNN`-tracked + documented in `docs/futurebuild/24_API_STABILITY.md`.

---

## 8. Replay reproducibility

The `provenance` block on every `RealityPrediction` is the audit-substrate closure. A future client can:

1. Lookup `provenance.active_pointer` in `CF_MEJEPA_WEIGHT_BLOBS` → load weights
2. Lookup `provenance.constellation_version` in `CF_MEJEPA_CONSTELLATION` → load centroids
3. Lookup `provenance.calibration_version` in calibration storage → load thresholds
4. Run `mejepa_verify(<patch>, <context>)` against the same inputs → byte-equal `RealityPrediction`

This is the **`TASK-OBS-011 prediction-replay-tool`** (futurebuild Phase F).

---

## 9. Changelog

Per `TASK-API-005`. `CHANGELOG.md` at workspace root, maintained per release. Categories:

```
## [vX.Y.Z] - YYYY-MM-DD

### Added
- New MCP tool: ...
- New CF: ...
- New prediction class variant: ...

### Changed
- (additive only pre-1.0; SemVer post-1.0)

### Deprecated
- ...

### Removed
- ...

### Fixed
- bug XYZ in ...

### Security
- vulnerability fix
```

---

## 10. MCP protocol-level versioning

| Protocol | Version |
|---|---|
| JSON-RPC | 2.0 (frozen) |
| MCP | per Anthropic spec |
| ME-JEPA tool family | namespaced `mcp__cgreality__mejepa_*` (versioned via tool-set version field in `get_capability_matrix`) |

Operator can call `get_capability_matrix` to read the current tool roster + their semver tags.

---

## 11. Cross-references

[`04_database_schema.md §3`](04_database_schema.md) (schema versioning), [`15_witness_chain.md`](15_witness_chain.md) (provenance binding), [`17_prediction_taxonomy_reference.md`](17_prediction_taxonomy_reference.md) (schema-stable enum extension).
