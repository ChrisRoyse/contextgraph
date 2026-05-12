# 19 — CLI Reference

Source files covered: `crates/context-graph-cli/src/main.rs`, `crates/context-graph-cli/src/commands/`, `crates/context-graph-cli/src/bin/`, `crates/context-graph-mejepa-corpus/src/bin/mejepa-corpus.rs`, `crates/context-graph-mejepa-train/src/bin/mejepa-train.rs`, `crates/context-graph-mejepa/src/bin/mejepa-heal-drill.rs`.

> **15 top-level commands; ~150 sub-subcommands.** Three binaries: `context-graph-cli` (main), `context-graph` (symlink alias), `blackbox` (BlackBoxFix ledger).

---

## 1. Global flags

| Flag | Type | Default | Purpose |
|---|---|---|---|
| `-v` / `--verbose` | count (0–3+) | 0 | Log level: 0→warn, 1→info, 2→debug, 3+→trace |

---

## 2. Command tree (top-level)

| Group | Sub-subcommands | Purpose |
|---|---|---|
| `session` | 2 | Session state persistence |
| `hooks` | 8 | Claude Code hook entrypoints |
| `memory` | 4 | Context injection + memory capture |
| `topic` | 3 | HDBSCAN topic clustering |
| `divergence` | 1 | Divergence detection |
| `dynamicjepa` | 44 | DynamicJEPA full surface |
| `learning` | 7 | UTL learning events |
| `utl` | 7 | UTL learner-state |
| `setup` | (direct) | Initialize Claude Code hooks |
| `warmup` | (direct) | Pre-load 14 embedding models |
| `watch` | (direct) | Markdown file watcher |
| `export` | 2 | Parquet / JSONL exports |
| `backfill-e14` | (direct) | Backfill E14 BGE-M3 vectors |
| `blackbox` | 44+ | BlackBoxFix Phase 1 ledger |
| `ccreality` | 1 | Post-edit harness transition |

---

## 3. `session`

| Subcommand | Purpose |
|---|---|
| `restore-identity` | Restore session state from storage (TASK-SESSION-12) |
| `persist-identity` | Persist session state to storage (TASK-SESSION-13) |

---

## 4. `hooks`

Each invokes the corresponding `crates/context-graph-cli/src/commands/hooks/*.rs` handler. Bounded timeouts.

| Subcommand | Timeout | Flags |
|---|---|---|
| `session-start` | 5,000 ms | `--session-id`, `--previous-session-id?`, `--stdin`, `--format` |
| `pre-tool` | 500 ms FAST PATH (NO DB) | `--session-id` (req), `--tool-name`, `--stdin`, `--fast-path`, `--format` |
| `post-tool` | 3,000 ms | `--session-id` (req), `--tool-name`, `--success`, `--stdin`, `--format` |
| `prompt-submit` | 2,000 ms | `--session-id` (req), `--prompt`, `--stdin`, `--format` |
| `session-end` | 30,000 ms | `--session-id` (req), `--duration-ms`, `--stdin`, `--generate-summary`, `--format` |
| `pre-compact` | 20,000 ms | `--session-id` (req), `--stdin`, `--format` |
| `task-completed` | 20,000 ms | `--session-id` (req), `--stdin`, `--format` |
| `generate-config` | n/a | `--output-dir`, `--force`, `--hooks` (csv), `--shell` |

---

## 5. `memory`

| Subcommand | Behavior |
|---|---|
| `inject-context` | Inject context from memory store (UserPromptSubmit / SessionStart hook backend); `query?`, `--budget` |
| `inject-brief` | < 200-token brief for PreToolUse hook; reads `TOOL_DESCRIPTION` / `TOOL_NAME` env |
| `capture-memory` | Capture hook description (`HookDescription` source) |
| `capture-response` | Capture Claude responses (`ClaudeResponse` source) |

---

## 6. `topic`

| Subcommand | Flags |
|---|---|
| `portfolio` | `--format` (brief/standard/verbose) — emerges from weighted multi-space clustering, threshold ≥ 2.5 |
| `stability` | `--hours` (1–24, default 6); churn rate + entropy + phase breakdown |
| `detect` | `--force` (recalculate); requires ≥ 3 memories (HDBSCAN) |

---

## 7. `divergence`

| Subcommand | Flags |
|---|---|
| `check` | `-H, --hours` (1–48, default 2), `--json`, `-v, --verbose`; uses semantic embedders only (E1, E5–E7, E10, E12, E13); excludes temporal E2–E4 |

---

## 8. `dynamicjepa` (44 ops)

### 8.1 Domain pack

| Op | Behavior |
|---|---|
| `register-domain` | TOML domain pack → RocksDB CFs; `--db`, `--file`, `--json` |
| `list-domains` | `--db`, `--limit`, `--offset`, `--json` |
| `get-domain` | `--db`, `--id`, `--domain-version` (default 1.0.0), `--json` |

### 8.2 Inspection / audit

`inspect-counts`, `inspect-cf`, `verify-audit-witness`, `list-instrument-readings`.

### 8.3 Event / adapter

`ingest-event`, `run-adapter`.

### 8.4 Panel / binding

`materialize-panel`, `get-panel`, `bind`, `list-bindings`.

### 8.5 Trajectory / dataset

`compile-trajectories`, `get-trajectory`, `list-trajectories`, `compile-dataset`, `get-dataset-shard`, `inspect-dataset`.

### 8.6 Training / artifacts

`train`, `get-training-run`, `get-artifact`, `predict`, `plan`, `record-surprise`.

### 8.7 Constellation / threshold

`build-constellation`, `list-constellations`, `get-constellation`, `calibrate-threshold`, `recalibrate-threshold`.

### 8.8 Audit / analysis

`compute-mc-ratio`, `audit-pairwise-mi`.

### 8.9 Cross-domain

`cross-domain-transfer`.

### 8.10 Semantic / corpus

`build-semantic-index`, `validate-corpus-diversity`, `attribute-test-delta`, `compare-shadow-utility`.

### 8.11 Getters

`get-prediction`, `get-plan-trace`, `get-surprise`.

### 8.12 Verification domains

`verify-counter-world`, `verify-gridworld`, `verify-career-taxonomy`.

### 8.13 Research / benchmarking

`research-smoke`, `hp-sweep`, `multiseed-aggregate`, `capture-reference`, `check-reference`, `check-release-aggregate-reference`.

---

## 9. `learning`

| Subcommand | Behavior |
|---|---|
| `count`, `list`, `get` | Counts + listing + UUID fetch on `CF_LEARNING_EVENTS` |
| `export-events-jsonl` | Export learning events as JSONL tensor rows |
| `export-dataset-jsonl` | Export learner-training matrix rows as JSONL |
| `record-events-jsonl` | Store real LearningEvent rows from JSONL + read back |
| `record-synthetic` | Store deterministic synthetic learning event + read back |

---

## 10. `utl`

| Subcommand | Behavior |
|---|---|
| `list-embedders` | E15–E21 learner-state embedder registry + calibration datasets |
| `fixture` | Deterministic local calibration fixture summary |
| `preflight-assets` | Inspect required E15–E21 model files + real calibration data; `--models-root` (default `models`), `--calibration-root`, `--allow-missing` |
| `count` | Rows in learner-state SoT CFs |
| `record-synthetic` | `--storage`, `--learner-id`, `--trace-id`, `--timestamp` |
| `get-delta` | Fetch persisted UTL delta log |
| `get-m` | Fetch persisted M(t) trace |

---

## 11. `setup`

| Flag | Purpose |
|---|---|
| `--generate-hooks` | Creates `.claude/settings.json` + `.claude/hooks/*.sh` |

---

## 12. `warmup`

| Flag | Default | Purpose |
|---|---|---|
| `--skip-if-warm` | true | Skip if models already warm |
| `--timeout-secs` | 300 | Timeout |

Loads all 14 production embedding models into VRAM (20-30s on RTX 5090).

---

## 13. `watch`

| Flag | Default | Purpose |
|---|---|---|
| `-p, --path` (req) | — | Directory to watch for `.md` files |
| `-s, --session-id` | `file-watcher-session` | Session ID for captured memories |
| `--db-path` (env: `CONTEXT_GRAPH_DATA_DIR`) | — | Database path |

Chunks `.md` files (200w + 50w overlap) → `MDFileChunk` memories in `TeleologicalMemoryStore`.

---

## 14. `export`

| Subcommand | Flags |
|---|---|
| `training-corpus` | `--format parquet`, `--out` (parent must exist), `--storage`, `--batch-size` (default 500), `--limit?` — schema `(memory_id: Utf8, record_bytes: Binary)` |
| `dynamicjepa-episodes` | DynamicJEPA bundle plan + edge-case evidence to JSONL |

---

## 15. `backfill-e14`

Idempotent backfill of empty `e14_bge_m3_dense` fields via native `BgeM3DenseModel`.

| Flag | Default | Purpose |
|---|---|---|
| `--storage` (env: `CONTEXT_GRAPH_STORAGE_PATH`) | — | Storage path |
| `--models-dir` | `./models` | Holds `bge-m3-dense/` snapshot |
| `--dry-run` | false | Count without model load/writes |
| `--limit?` | — | Cap on total fingerprints |
| `--scan-batch` | 2000 | Per-`list` call upper bound |
| `--log-every` | 100 | Log progress |

---

## 16. `blackbox` (separate `blackbox` binary; BlackBoxFix Phase 1)

44+ ops covering: ledger (schema v14), connectors (GitHub, GitLab, Terraform, K8s, MCP), episode export, dashboard, enterprise controls, SIEM export, scorecard, privacy, release gates, packaging, risk, reward compiler.

```bash
blackbox <subcommand>
```

Full surface in `crates/context-graph-cli/src/commands/blackbox.rs` (1,303 KB file).

---

## 17. `ccreality`

| Subcommand | Flags |
|---|---|
| `record-harness-transition` | `--run-id`, `--attempt`, `--session-id`, `--tool-use-id`, `--tool-name`, `--file-path` (req), `--before-sha256`, `--after-sha256`, `--preedit-state-path`, `--after-source-path`, `--lines-added`, `--lines-removed`, `--git-diff-stat`, `--cargo-status`, `--cargo-stderr-tail`, `--timeout-seconds` (default 20), `--format` |

Records post-edit harness transition through MCP tool `optimizer_record_harness_transition`. Invoked by the PostToolUse hook script.

---

## 18. Separate binaries

### 18.1 `mejepa-corpus` (in `context-graph-mejepa-corpus`)

| Subcommand | Purpose |
|---|---|
| `generate` | Generate corpus under D-root |
| `verify` | Re-run Docker oracle verification |
| `stats` | Per-category + per-repo stats |
| `prepare-images` | Pre-pull SWE-bench Docker images |
| `cleanup` | Remove stale corpus runs |

### 18.2 `mejepa-train` (in `context-graph-mejepa-train`)

Training driver. Reads dataset shards → trunk + heads forward + multi-loss + cert chain + holdout eval. Flags include batch size, LR, epochs, checkpoint output.

### 18.3 `mejepa-heal-drill` (in `context-graph-mejepa`)

Synthetic drift drill: injects 90% → 84% drift over 1000 obs; scheduler detects within 1k more obs; A/B/C runs; winner promoted. Used as FSV harness.

### 18.4 `mejepa-verify` (in `context-graph-mejepa`)

Inference CLI: `verify <patch> --context <ctx>` → `Verdict`.

---

## 19. Operator-facing CLI commands (futurebuild Phase F)

Per `docs/futurebuild/19_OPERATOR_RUNBOOK.md §9`:

```bash
context-graph-cli mejepa active-learning list [--top-n N]
context-graph-cli mejepa active-learning label <id> <label> <reason>
context-graph-cli mejepa active-learning dismiss <id> <reason>
context-graph-cli mejepa hygiene cold-audit
context-graph-cli mejepa hygiene aggressive-cold-tier-down
context-graph-cli mejepa rollback --version <version>
context-graph-cli mejepa eval run --force
context-graph-cli mejepa eval ship-gate
context-graph-cli mejepa daemon status
context-graph-cli mejepa daemon restart
context-graph-cli mejepa daemon stop
```

These wrap the MCP tools for easier shell-side use.

---

## 20. Cross-references

[`18_mcp_tool_reference.md`](18_mcp_tool_reference.md), [`20_hooks_layer.md`](20_hooks_layer.md), [`22_operator_runbook.md`](22_operator_runbook.md).
