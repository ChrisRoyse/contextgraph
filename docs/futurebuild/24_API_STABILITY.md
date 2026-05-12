# 24 — API Stability and Versioning

> **Purpose.** Specify the stability + deprecation policy for the public-facing surface: MCP tools, `RealityPrediction` schema, error codes, CLI commands. Operators and external clients depend on these; ad-hoc breaking changes are toxic.

---

## 1. Versioning scheme

Semantic versioning at the **system** level: `vMAJOR.MINOR.PATCH`.

- **MAJOR** — incompatible changes (removing a tool, changing an enum to add a non-default variant in a way that breaks deserialization, dropping a CF without migration).
- **MINOR** — additive changes (new tool, new optional JSON field, new documented enum variant, new CF). Persisted binary schema changes still require explicit migration/readback.
- **PATCH** — bug fixes, perf improvements, internal refactors with no API change.

Current target: ME-JEPA-Code v0.x (pre-1.0). Pre-1.0 means MAJOR can change, but MINOR/PATCH discipline still applies.

**v1.0 ship gate:** the ≥0.95 ship gate is met (per `01_VISION_TRUTH_SYSTEM.md §8`). At that point, MAJOR breaking changes require deprecation cycle (see §3).

---

## 2. Compatibility guarantees

### 2.1 MCP tool surface

| Tool | Stability tier |
|---|---|
| `mejepa_verify` | Stable (v0.4+); response shape extensions only |
| `mejepa_predict_latest` | Stable (v0.4+) |
| `mejepa_record_agent_feedback` | Stable from Phase A onwards |
| `mejepa_predict_what_if`, `mejepa_explain_prediction` | Beta (implemented Phase B, 2026-05-12) |
| `mejepa_constellation_inspect` | Stable |
| `mejepa_subscriber_status`, `mejepa_heal_status`, `mejepa_quota_status`, `mejepa_vram_budget_report` | Stable |
| `mejepa_eval_run`, `mejepa_ship_gate_status`, `mejepa_active_learning_queue`, `mejepa_eval_build_graph` | Stable |
| `mejepa_audit_pairwise_mi` | Stable |
| `mejepa_observe_shift`, `mejepa_capture_audit` | Stable |
| `mejepa_load_learner_state`, `mejepa_routing_lookup`, `mejepa_rollback_to`, `mejepa_promote_approval` | Stable |
| `mejepa_gc_run`, `mejepa_witness_compress` | Stable |
| Retired tools (`mcp__cgreality__reality_run_attempt`) | fail-closed; surface stays for clear error |

Stable tools commit to additive-only changes. Beta tools may evolve.

### 2.2 `RealityPrediction` schema

- Adding a new field with default → minor bump
- Adding a new `Verdict` variant → minor bump; daemon-side decoding must fail closed if a persisted binary value cannot be decoded into the active schema
- Adding a new `FailureModeClass` → minor bump (extractors fall back to `Other(name)`)
- Removing a field → major bump (with deprecation cycle)
- Renaming a field → major bump (with field-alias support during deprecation)
- Binary RocksDB value schema changes require explicit migration or fresh CF creation. Do not silently default old serialized predictions into a new shape.

### 2.3 Error codes

`MEJEPA_*` error code names are **stable**. New error codes can be added (minor bump). Renames or removals require deprecation cycle.

### 2.4 CLI commands

`context-graph-cli mejepa <subcommand>` is **stable** at the subcommand level. Flags can be added (minor bump). Removing a flag or subcommand requires deprecation cycle.

### 2.5 RocksDB CFs

CF names are **stable**. Adding new CFs is minor; removing requires major + migration tool. CF value schema follows `RealityPrediction` rules (additive fields OK).

---

## 3. Deprecation cycle

For breaking changes:

```
v0.X.0 — feature exists; works as documented
v0.X+1.0 — feature still works; deprecation warning emitted on every use
v0.X+2.0 — feature still works; deprecation error emitted; opt-in via flag to silence
v1.0.0 — feature removed (only at major version boundaries, not pre-1.0)
```

For pre-1.0 breaking changes, an operator-facing announcement is required (changelog entry; SMAP decision file; release note).

---

## 4. Schema migration (D1)

When a CF schema must change incompatibly:

```
1. Add new CF with v2 schema
2. Provide migration tool: context-graph-cli mejepa storage migrate-cf <old> <new> [--dry-run]
3. Daemon startup validates that the active CF schema version is readable; if migration is required and has not been run, startup fails closed with a precise migration error
4. After migration period, old CF is removed in a major version bump
```

The migration tool reads old entries, transforms via a deterministic mapper, writes to new CF, validates round-trip, optionally archives old CF to backup.

For pre-1.0 (current state): schema migrations are explicit operator-run commands, not silent.

For post-1.0: optional automatic migration on daemon startup is allowed only with explicit operator opt-in and pre/post source-of-truth readback.

---

## 5. Replay reproducibility (D5)

Every `RealityPrediction` includes a `provenance` block with the predictor, constellation, and calibration versions. To replay a prediction:

```
mcp_call mejepa_inspect_prediction --prediction-id <id>
# Returns full provenance: weight version, constellation version, calibration version

context-graph-cli mejepa replay --prediction-id <id>
# Reproduces the prediction byte-for-byte by:
# 1. Loading the historical weights / constellation / calibration
# 2. Re-running the predictor on the same panel
# 3. Asserting the output matches the persisted RealityPrediction
```

This is the cross-version reproducibility guarantee. If `mejepa replay` ever produces a mismatch, that's a serious bug (non-determinism in the predictor or storage corruption).

`TASK-OBS-011-prediction-replay-tool.md` covers this.

---

## 6. Backwards compatibility for clients

External MCP clients (e.g., scripts, CI bots, third-party agents) should:

1. Pin to a daemon version range (e.g., `cgreality >= 0.4, < 0.5`).
2. Tolerate extra JSON fields, but treat unknown required enum variants or undecodable persisted values as explicit errors.
3. Read the daemon's reported version: `mcp_call optimizer_recall_recommendations` returns `daemon_version` in the response metadata; clients can adapt accordingly.

---

## 7. Versioning the futurebuild docs

The futurebuild docs themselves don't ship a version, but each doc has a `Date authored:` line. When a doc is materially revised, add a `Date revised:` line + a one-line revision note at the bottom.

`memory/decisions/` files are append-only; revisions create a new file with `superseded_by: <old-file>`.

---

## 8. What this commits us to

Tasks (in `tasks/api/`):

- `TASK-API-001` — semantic version emission in MCP responses (`mcp_response.daemon_version`)
- `TASK-API-002` — `context-graph-cli mejepa storage migrate-cf` command
- `TASK-API-003` — replay tool (`context-graph-cli mejepa replay`)
- `TASK-API-004` — deprecation framework (warning emission + opt-in error escalation)
- `TASK-API-005` — changelog format + first changelog entries
