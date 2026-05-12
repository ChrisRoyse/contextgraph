# 03 — Configuration

Source files covered: `Cargo.toml` (workspace), `rust-toolchain.toml`, `config/`, `configs/`, `.claude/settings.json`, `crates/context-graph-mejepa-embedders/src/config.rs`, `crates/context-graph-mcp/src/daemon.rs`, `crates/context-graph-mcp/src/daemon_validate.rs`, `crates/context-graph-mejepa-hygiene/src/quota.rs`, `crates/context-graph-mejepa/src/config.rs`, `crates/context-graph-mejepa-train/src/config.rs`, `scripts/systemd/mejepa-daemon.service`, `docs/futurebuild/16_STORAGE_LOCATION_POLICY.md`, `docs/futurebuild/19_OPERATOR_RUNBOOK.md`.

> **Status**: Some keys below are **shipped** (live in current code); some are **futurebuild** (in `docs/futurebuild/`). Tagged in the **Status** column.

---

## 1. Where configuration lives

| Surface | File / location | Format | Loaded by | Lifecycle |
|---|---|---|---|---|
| Workspace toolchain | `rust-toolchain.toml` | TOML | rustup | per-build |
| Cargo workspace | `Cargo.toml` (root) | TOML | cargo | per-build |
| Per-crate features | `crates/*/Cargo.toml` | TOML | cargo | per-build |
| MCP daemon args | CLI flags + env | argv | `context-graph-mcp::main` | startup |
| Daemon TOML | `~/.config/mejepa/daemon.toml` (futurebuild) | TOML | daemon at startup | hot-reload not yet — restart required |
| Path policy / governance | `config/governed_paths.json` (mostly gutted post-2026-05-07) | JSON | MCP server | startup |
| Embedder routing / digests | `config/models_config.toml` | TOML | `context-graph-mejepa-embedders` | startup |
| Model file location | `/mnt/d/cabdru/contextgraph/models/<embedder_id>/` | filesystem | embedder loaders | first-use lazy load |
| Embedder calibration | `data/utl_calibration/` (E17 + others) | binary fixtures | embedder loaders | first-use |
| Reward specs (RETIRED inner-loop) | `crates/*/src/reward_specs/**` | code | `crates/context-graph-benchmark` | build-time |
| Hook config | `.claude/settings.json` | JSON | Claude Code | session-start |
| Hook scripts | `scripts/cc/*.sh` | bash | Claude Code via settings.json | per-event |
| systemd unit | `scripts/systemd/mejepa-daemon.service` | systemd | `systemctl --user enable --now` | host startup |
| BlackBoxFix ledger | `~/.blackbox/` | SQLite + JSON | `context-graph-cli blackbox` | per-call |
| Active runtime root pointer | `/mnt/d/contextgraph/state/cgreality/active_runtime_root` | filesystem (single file) | SessionStart hook | session start |

---

## 2. MCP daemon arguments

Source: `crates/context-graph-mcp/src/main.rs` (`clap` derived struct).

| Flag | Type | Default | Description |
|---|---|---|---|
| `--daemon-server-only` | bool (flag) | false | Start in long-running daemon mode (supervised tokio tasks, no Claude Code attach) |
| `--d-root` | PathBuf | `/mnt/d/contextgraph` | Durable data root; daemon refuses to start if missing or not writable (`MEJEPA_D_DRIVE_UNAVAILABLE`) |
| `--daemon-port` | u16 | 3100 | TCP/unix socket port for daemon-mode IPC |
| `--http-port` | u16 | 3101 | HTTP health/readiness probe port (futurebuild Phase F via `TASK-OBS-014`) |
| `--rocksdb-path` | PathBuf | `<d-root>/storage/contextgraph-rocksdb` | RocksDB store location |
| `--shift-log-dir` | PathBuf | `<d-root>/runtime/cgreality-shift-log` | Per-session shift log JSONL directory |
| `--log-dir` | PathBuf | `<d-root>/logs` | Daemon log output (rotated daily) |
| `--lock-dir` | PathBuf | `<d-root>/state/locks` | PID + flock + unix socket location |
| `--enable-e5-causal` | bool | false | Enable retired E5 surface (operator override only; otherwise fails closed) |
| `--enable-e11-entity` | bool | false | Enable retired E11 surface (operator override only) |
| `--rate-limit-feedback-per-hour` | u32 | 100 | `mejepa_record_agent_feedback` rate limit per `agent_id` |

---

## 3. Daemon TOML keys (`~/.config/mejepa/daemon.toml` — futurebuild Phase F)

Source: `docs/futurebuild/19_OPERATOR_RUNBOOK.md §6` + `docs/futurebuild/03_ARCHITECTURE_BLUEPRINT.md §4.5`.

### 3.1 Scheduler section

| Key | Type | Default | Valid range | Description | Status |
|---|---|---|---|---|---|
| `scheduler.observe_tick_ms` | u64 | 250 | 100–2000 | Pending-shift drain frequency | partial |
| `scheduler.drift_tick_secs` | u64 | 60 | 10–600 | Rolling drift window snapshot frequency | partial |
| `scheduler.active_learning_tick_secs` | u64 | 600 | 60–3600 | Active-learning queue drain | partial |
| `scheduler.promote_tick_secs` | u64 | 3600 | 600–86400 | A/B/C promotion gate check | partial |
| `scheduler.continual_backprop_tick_secs` | u64 | 1800 | 600–7200 | Dormant-unit detector | partial |
| `scheduler.constellation_freshness_tick_secs` | u64 | 3600 | 600–86400 | Stale-cell refit check | partial |
| `scheduler.emergency_eviction_tick_secs` | u64 | 600 | 60–3600 | Quota overflow check | partial |
| `scheduler.observe_drain_deadline_ms` | u64 | 200 | 50–1000 | Per-tick observe deadline | partial |
| `scheduler.dormancy_threshold` | f32 | 0.15 | 0.05–0.5 | Re-init when dormant_fraction exceeds | partial |
| `scheduler.constellation_max_age_days` | u32 | 90 | 30–365 | Stale-cell threshold for refit | shipped |

### 3.2 Drift section

| Key | Type | Default | Range | Description | Status |
|---|---|---|---|---|---|
| `drift.soft_threshold` | f32 | 0.88 | 0.70–0.95 | Lower bound for Soft drift class | shipped |
| `drift.hard_threshold` | f32 | 0.85 | 0.65–0.92 | Lower bound for Hard drift class | shipped |
| `drift.catastrophic_threshold` | f32 | 0.80 | 0.50–0.90 | Lower bound for Catastrophic class | shipped |
| `drift.rolling_window_size` | u32 | 1000 | 100–10000 | Attempts in rolling window | shipped |
| `drift.bayesian_small_window_n` | u32 | 50 | 10–500 | Beta-posterior trigger when N < this | futurebuild Phase E |
| `drift.surprise_event_weight` | f32 | 2.0 | 1.0–10.0 | Multiplier on `Surprise` events in rolling window | futurebuild Phase E |

### 3.3 Calibration / conformal section

| Key | Type | Default | Range | Description | Status |
|---|---|---|---|---|---|
| `conformal.coverage_target` | f32 | 0.90 | 0.50–0.99 | Stated coverage (α = 1 - target) | shipped |
| `conformal.coverage_tolerance` | f32 | 0.02 | 0.005–0.10 | Acceptable empirical coverage deviation | shipped |
| `conformal.calibration_window_size` | u32 | 1000 | 100–10000 | Rolling tuples for non-conformity score fit | shipped |
| `conformal.recalibrate_ece_trigger` | f32 | 0.03 | 0.005–0.10 | ECE threshold to trigger recalibrate | shipped |
| `ood.score_threshold` | f32 | 0.50 | 0.05–0.95 | `MEJEPA_INFER_OOD_REFUSE` cutoff | shipped |
| `ood.hard_threshold` | f32 | 0.85 | 0.50–0.99 | `MEJEPA_VERIFY_OUT_OF_DISTRIBUTION_HARD` cutoff | shipped |

### 3.4 Hygiene / quota section

Source: `crates/context-graph-mejepa-hygiene/src/quota.rs`.

| Key | Type | Default | Range | Description | Status |
|---|---|---|---|---|---|
| `quota.hard_gb` | f64 | 115.0 | 10–8192 | Hard quota on `/mnt/d/contextgraph/` (env: `CG_MEJEPA_DISK_QUOTA_GB`) | shipped |
| `quota.emergency_eviction_gb` | f64 | 110.0 | 5–8000 | Eviction begins at this usage | shipped |
| `quota.warn_gb` | f64 | 100.0 | 5–8000 | Warning log threshold | shipped |
| `gc.nightly_hour_local` | u8 | 3 | 0–23 | Local hour for nightly GC (default 03:00) | partial (cron pending) |
| `gc.shift_log_retention_days` | u32 | 30 | 7–365 | Per-session JSONL retention | shipped |
| `gc.witness_merkle_compress_days` | u32 | 90 | 30–730 | Chain segments compressed at this age | shipped |
| `gc.cold_tier_ttl_days` | u32 | 365 | 30–3650 | `CF_MEJEPA_DDA_SIGNALS` cold-tier TTL (per slice) | partial |

### 3.5 Embedder section

Source: `crates/context-graph-mejepa-embedders/src/config.rs` + `crates/context-graph-mejepa-embedders/src/vram.rs`.

| Key | Type | Default | Description | Status |
|---|---|---|---|---|
| `embedder.models_root` | PathBuf | `/mnt/d/cabdru/contextgraph/models` | Per-embedder model file root | shipped |
| `embedder.calibration_root` | PathBuf | `data/utl_calibration` | Calibration fixtures (E17 etc.) | shipped |
| `embedder.vram_budget_gb` | f32 | 29.0 | RTX 5090 has 32 GB; budget reserves headroom | shipped |
| `embedder.lazy_load_e15_e21` | bool | true | Non-code learner-state lazy-load | shipped |
| `embedder.precision` | enum | `bf16` | `f32` / `bf16` / `f16`; production = `bf16` | shipped |
| `embedder.allow_e5_causal` | bool | false | Override to load retired E5 (operator only) | shipped |
| `embedder.allow_e11_entity` | bool | false | Override to load retired E11 (operator only) | shipped |
| `embedder.cache_size_gb` | f32 | 5.0 | Per-(embedder, chunk_sha) cache budget | futurebuild Phase D |

### 3.6 Per-head loss coefficients (multi-head training)

Source: `docs/futurebuild/10_LEARNING_FORMULA_MEJEPA_TRUTH.md §8`.

| Key | Default | Ramped to | Ramp trigger | Status |
|---|---|---|---|---|
| `loss.alpha_panel` | 1.0 | 1.0 (fixed) | n/a | shipped |
| `loss.beta_pairwise` | 0.3 | 0.3 (fixed) | n/a | shipped |
| `loss.gamma_action` | 0.4 | 0.4 (fixed) | n/a | shipped |
| `loss.delta_constellation` | 0.2 | 0.2 (fixed) | n/a | shipped |
| `loss.epsilon_anti_pole_vicreg` | 0.5 | 0.5 (fixed) | n/a | shipped |
| `loss.zeta_uncertainty` | 0.1 | 0.1 (fixed) | n/a | shipped |
| `loss.eta_invariant` | 0.1 | 0.1 (fixed) | n/a | shipped |
| `loss.lambda_oracle` | 0.5 | 0.5 (fixed once shipped) | n/a | shipped |
| `loss.lambda_failure_mode` | 0.05 | 0.30 | F1 > 0.6 | futurebuild Phase B |
| `loss.lambda_edge_case` | 0.05 | 0.20 | uncovered-branch recall > 0.5 | futurebuild Phase B |
| `loss.lambda_tech_debt` | 0.05 | 0.10 | PR-comment alignment > 0.5 | futurebuild Phase B |
| `loss.lambda_perf` | 0.05 | 0.15 | benchmark MAE < 10% | futurebuild Phase B |
| `loss.lambda_security` | 0.05 | 0.20 | static-analyzer agreement > 0.7 | futurebuild Phase B |
| `loss.lambda_accuracy` | 0.05 | 0.10 | ML-test metric MAE < 5% | futurebuild Phase B |
| `loss.lambda_cost` | 0.02 | 0.05 | CI-time MAE < 20% | futurebuild Phase B |
| `loss.lambda_reasoning` | 0.05 | 0.10 | reasoning-class agreement > 0.6 | futurebuild Phase B |

Ramping increment: `+0.05 / week` once trigger crosses, until steady-state. Source: `crates/context-graph-mejepa-train/src/sampler/`.

### 3.7 Training section

Source: `crates/context-graph-mejepa-train/src/config.rs`.

| Key | Type | Default | Range | Description | Status |
|---|---|---|---|---|---|
| `training.learning_rate` | f32 | 5e-4 | 1e-5 – 1e-2 | AdamW LR | shipped |
| `training.weight_decay` | f32 | 0.04 | 0.0 – 0.5 | AdamW weight decay | shipped |
| `training.batch_size` | u32 | 64 | 1 – 1024 | Training batch | shipped |
| `training.num_epochs` | u32 | 10 | 1 – 1000 | Default epochs for full-retrain | shipped |
| `training.cosine_lr_min` | f32 | 1e-6 | 0 – 1e-3 | Cosine schedule floor | shipped |
| `training.holdout_eval_every_steps` | u32 | 1000 | 100 – 100000 | Holdout eval cadence | shipped |
| `training.full_retrain_every_attempts` | u32 | 10000 | 1000 – 1000000 | Periodic full retrain interval | shipped |
| `training.sampling_weight_decay` | f32 | 0.995 | 0.9 – 0.9999 | Per-day decay (half-life ~140d) | shipped |
| `training.sampling_weight_min` | f32 | 0.05 | 0.001 – 0.5 | Drop from epoch below this | shipped |
| `training.operator_override_multiplier` | f32 | 6.0 | 1.0 – 100.0 | Gold-label boost | shipped |
| `training.agent_surprise_multiplier` | f32 | 2.0 | 1.0 – 20.0 | Per-severity-score bump factor | futurebuild Phase A |

### 3.8 Predictor section

Source: `crates/context-graph-mejepa/src/config.rs` + `crates/context-graph-mejepa/src/predictor/`.

| Key | Type | Default | Description | Status |
|---|---|---|---|---|
| `predictor.num_layers` | u32 | 6 | Transformer depth | shipped |
| `predictor.hidden_dim` | u32 | 1024 | Hidden dim | shipped |
| `predictor.num_heads` | u32 | 8 | Attention heads | shipped |
| `predictor.activation` | enum | `gelu` | `gelu` / `relu` / `swiglu` | shipped |
| `predictor.precision` | enum | `bf16` | `f32` / `bf16` | shipped |
| `predictor.cuda_device_index` | u32 | 0 | CUDA device | shipped |

### 3.9 Ship-gate section

| Key | Type | Default | Description | Status |
|---|---|---|---|---|
| `ship_gate.correlation_threshold` | f32 | 0.95 | Per-cell prediction-oracle correlation | shipped |
| `ship_gate.rolling_windows_required` | u32 | 4 | Consecutive windows ≥ threshold | shipped |
| `ship_gate.shrinkage_n_min` | u32 | 50 | Bayesian shrinkage when N < this | shipped |
| `ship_gate.per_language_coverage_min` | f32 | 0.88 | Per-language coverage lower bound | shipped |
| `ship_gate.per_language_coverage_max` | f32 | 0.92 | Per-language coverage upper bound | shipped |
| `ship_gate.ood_auc_min` | f32 | 0.85 | OOD AUC floor | shipped |

### 3.10 Active-learning section

| Key | Type | Default | Description | Status |
|---|---|---|---|---|
| `active_learning.top_n` | u32 | 10 | Per-tick high-priority entries to drain | shipped |
| `active_learning.surface_period_secs` | u32 | 600 | Operator-surface interval | shipped |
| `active_learning.severity_threshold` | enum | `Medium` | Min severity for AgentSurprise priority | futurebuild Phase A |

### 3.11 Subscriber section

| Key | Type | Default | Description | Status |
|---|---|---|---|---|
| `subscriber.poll_interval_ms` | u64 | 250 | JSONL tail polling | shipped |
| `subscriber.lag_alert_threshold` | u32 | 500 | Unprocessed shifts / 1 min triggers alert | shipped |
| `subscriber.lag_alert_window_secs` | u32 | 60 | Window for lag-rate computation | shipped |
| `subscriber.flap_restart_limit` | u32 | 5 | Restarts / 5 min before `MEJEPA_SCHEDULER_FLAPPING` | shipped |
| `subscriber.exponential_backoff_cap_secs` | u32 | 60 | Restart backoff cap | shipped |

---

## 4. Environment variables

| Variable | Used by | Default | Purpose |
|---|---|---|---|
| `CG_MEJEPA_DISK_QUOTA_GB` | hygiene | 115.0 | Override hard quota |
| `CONTEXT_GRAPH_STORAGE_PATH` | CLI + storage | `<d-root>/storage/contextgraph-rocksdb` | RocksDB path |
| `CONTEXT_GRAPH_DATA_DIR` | `watch` cli | (CWD) | Memory-store base dir |
| `MEJEPA_INSTRUMENT_GRADIENT_LEAK` | training | unset | When set, fail-close `compute_delta_xi` on any non-zero target_collapse (default behavior — leave unset to keep enforced) |
| `MEJEPA_OBSERVE_DRIFT_TEST_OVERRIDE` | tests | unset | Override drift class for tests only |
| `TOOL_DESCRIPTION` | hook scripts | unset | Hook context injection |
| `TOOL_NAME` | hook scripts | unset | Per-tool injection |
| `SESSION_SUMMARY` | hook scripts | unset | Session-end summarization |
| `RUST_LOG` | all | `info` | tracing-subscriber level |
| `CUDA_VISIBLE_DEVICES` | embedders, predictor | (host default) | Pin GPU |
| `HF_HOME` | tokenizers | `~/.cache/huggingface` | HF tokenizer cache (not for weights — weights live under D-drive) |

---

## 5. Hook configuration (`.claude/settings.json`)

Source: `.claude/settings.json` + `scripts/cc/hook_*.sh`.

### 5.1 Hook timeouts

| Hook | Timeout | Purpose |
|---|---|---|
| `SessionStart` | 5,000 ms | Inject active runtime root |
| `PreToolUse` (Edit/Write) | 500 ms (FAST PATH — no DB) | Record pre-edit SHA |
| `PostToolUse` (Edit/Write) | 3,000 ms | Record after_sha + cargo check + shift-log append |
| `UserPromptSubmit` | 2,000 ms | Inject latest `RealityPrediction` markdown |
| `SessionEnd` | 30,000 ms | Final summary generation |
| `PreCompact` | 20,000 ms | SMAP narrative dump |
| `TaskCompleted` | 20,000 ms | Learnings extraction |
| `Stop` | (synchronous) | Require `optimizer_record_recommendation` |

### 5.2 Hook script paths (project-relative)

```
scripts/cc/hook_session_start.sh
scripts/cc/hook_pre_tool_use_edit.sh        # records pre-edit SHA; gate REMOVED 2026-05-07
scripts/cc/hook_post_tool_use_edit.sh       # runs cargo check, appends shift-log
scripts/cc/hook_user_prompt_submit.sh       # injects last 3 shifts + (futurebuild) latest RealityPrediction
scripts/cc/hook_stop.sh                     # validates optimizer_record_recommendation was called
scripts/cc/hook_pre_compact.sh              # SMAP narrative dump
```

Each shell script invokes `context-graph-cli hooks <subcommand>` per `crates/context-graph-cli/src/commands/hooks/args.rs:325-372`.

---

## 6. Embedder model artifacts (`models_config.toml`)

Source: `config/models_config.toml` + `docs/futurebuild/05_EMBEDDER_SIGNAL_MAP.md §5`.

Each embedder declares: model path, SHA-256 digest, dim, family, tokenizer path, VRAM cost. Daemon refuses to start on any digest mismatch (`MEJEPA_EMBEDDER_DIGEST_MISMATCH`).

| Embedder | Path under `/mnt/d/cabdru/contextgraph/models/` | Approx VRAM (fp16) |
|---|---|---|
| E1 | `intfloat/e5-large-v2.gguf` | 0.6 GB |
| E6 | (sparse codebook) | 0 |
| E7 | `Qodo/Qodo-Embed-1-1.5B.gguf` | 1.5 GB |
| E8 | (adapter on E7) | <1 GB |
| E9 | (HDC codebook) | <0.1 GB |
| E10 | `intfloat/multilingual-e5-large.gguf` | 0.7 GB |
| E12 | `colbert-ir/colbertv2.0/` | 0.5 GB |
| E13 | `naver/splade-v3/` | 0.4 GB |
| E14 | `Salesforce/codet5p-110m-embedding.gguf` | 0.4 GB |
| E17 | `cardiffnlp/twitter-roberta-base-sentiment-latest/` | 0.5 GB |
| E18, E20 | (small adapters) | <0.2 GB |
| **Total active code-domain** | | **~6.5 GB** |

---

## 7. Config validation rules

| Rule | Source | Behavior |
|---|---|---|
| D-root must exist and be writable | `daemon_validate.rs` | Refuse start with `MEJEPA_D_DRIVE_UNAVAILABLE` |
| All required subdirs auto-created on first start | `daemon_validate.rs` | `mkdir -p` for each path in §5 of `16_STORAGE_LOCATION_POLICY.md` |
| Embedder digests must match `models_config.toml` | `context-graph-mejepa-embedders/src/digest.rs` | Refuse load with `MEJEPA_EMBEDDER_DIGEST_MISMATCH` |
| E17 requires calibration cert | `context-graph-mejepa-embedders/src/calibration.rs` | Lazy-load rejects |
| VRAM budget gate | `context-graph-mejepa-embedders/src/vram.rs` | Refuse start over budget; `mejepa_vram_budget_report` MCP tool |
| Constellation freshness ≤ 90 days | `context-graph-mejepa-tct/src/freshness.rs` | Refuse without operator override (`MEJEPA_INFER_GTAU_STALE_CONSTELLATION`) |
| Panel dimensionality must equal 5,120 | `context-graph-mejepa-instruments/src/panel_json.rs` | Reject with `MEJEPA_INFER_DIM_MISMATCH` |
| All floats finite | per-instrument encoders | Reject with namespace-specific errors |
| Corpus output path must be under `<d-root>/corpus/` | `context-graph-mejepa-corpus/src/generate.rs` | Reject `/tmp` + C-drive paths |
| FSV evidence path must be under `<d-root>/fsv/` | `crates/*/examples/*_fsv.rs` (per `TASK-OBS-006`) | Lint enforces |
| C-drive paths in code forbidden | CI lint `TASK-OBS-005` | grep-asserts no `/home/`, `~/.cache/`, `/tmp/contextgraph-` |

---

## 8. Configuration loading order

Source: `crates/context-graph-mcp/src/main.rs` + `crates/context-graph-mejepa/src/config.rs`.

```
1. Hardcoded defaults (each crate's config.rs)
2. (futurebuild) Daemon TOML at ~/.config/mejepa/daemon.toml — overrides defaults
3. CLI flags — override TOML
4. Environment variables — override CLI flags for specific keys (CG_MEJEPA_DISK_QUOTA_GB, RUST_LOG)
5. SHA-pinned model digests (models_config.toml) — REJECT on mismatch, no override
6. Constellation max-age — REJECT on stale unless --force flag from operator
```

No silent fallback. Every load is fail-closed with a structured error.

---

## 9. systemd unit

Source: `scripts/systemd/mejepa-daemon.service`.

```ini
[Unit]
Description=ME-JEPA-Code daemon (MCP server + supervised tokio tasks)
After=network.target

[Service]
Type=simple
ExecStart=/home/cabdru/contextgraph/target/release/context-graph-mcp \
    --daemon-server-only \
    --d-root /mnt/d/contextgraph \
    --daemon-port 3100 \
    --http-port 3101
Restart=on-failure
RestartSec=10s
StandardOutput=append:/mnt/d/contextgraph/logs/mejepa-daemon.stdout.log
StandardError=append:/mnt/d/contextgraph/logs/mejepa-daemon.stderr.log

# Resource limits
MemoryMax=64G
TasksMax=infinity

[Install]
WantedBy=default.target
```

Operator command:

```bash
cp scripts/systemd/mejepa-daemon.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now mejepa-daemon
systemctl --user status mejepa-daemon
```

---

## 10. What configuration does NOT control

- **Frozen instruments**: per-instrument output is fully determined by the typed-event input + the constitution. No knob can alter `E_Oracle`'s output format, `E_Witness`'s sequence transformer, etc.
- **Frozen embedder weights**: model files are SHA-pinned. Training does not modify them.
- **Constellation cells once constructed**: requires operator force-refit; no automatic mutation.
- **Witness chain**: append-only; signing keys live in operator keyring; no config controls integrity.
- **The 8 mutation categories**: hardcoded enum; cannot be configured away.
- **Mode A / Mode B switching**: retired post-pivot; no mode flag.
