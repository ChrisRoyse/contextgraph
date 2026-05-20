# 06 — Operations, FSV, Runbook, DR, Test Suite

**Last refreshed:** 2026-05-19.

Source files covered: `docs/futurebuild/specs/FSV-PROTOCOL.md`, `AGENTS.md`, `CLAUDE.md`, `docs2/AICodingAgentSuperPrompt.md`, `docs2/hardeningprompt.md`, `scripts/cc/*.sh`, `crates/context-graph-mejepa-hygiene/src/{quota,gc,reports,retention,witness_compress}.rs`, `crates/context-graph-mejepa-shift-subscriber/src/`, `crates/context-graph-mcp/src/{daemon.rs, daemon_validate.rs, health_probe.rs, monitoring.rs, deprecation.rs}`, `crates/context-graph-mejepa-train/Cargo.toml` (test inventory).

---

## 1. FSV Protocol (the verification contract)

`docs/futurebuild/specs/FSV-PROTOCOL.md` is the binding contract for every task. Universal protocol in `docs2/AICodingAgentSuperPrompt.md`; project-level specifics here.

### 1.1 Source-of-truth first (§1)

> *A return value is a claim. The Source of Truth is the verdict.*

Before declaring a task complete, answer all four:
1. **What state changed?** File, CF row, metric, queue item, report, external record.
2. **Where does that state live?** Exact path, CF name + key, metric name, external identifier.
3. **How is it read back independently?** Separate DB open, file read, checksum, source command.
4. **What exact value is expected?** Literal value, schema, count delta, hash, bounded range.

If any answer is unknown → not done.

### 1.2 Trigger → Process → Outcome (§2)

Every FSV-able feature has this shape. Document before implementation: trigger (MCP / CLI / hook / scheduler), process (functions executed), outcome (physical SoT readback with expected value).

### 1.3 Hard rules (§3)

1. **No mocks** for completion gates.
2. **No fallbacks** that hide required-output failure.
3. No verifier weakening, ignored tests, silent bypasses.
4. **Tests must fail when the feature is broken.**
5. Missing CFs / files / models / env vars / malformed inputs → fail closed with `SCREAMING_SNAKE_CASE` codes.
6. **Assume the task is wrong** until live files / symbols / CF names prove it current.
7. Prefer smallest implementation that satisfies the issue + matches local patterns.

### 1.4 Synthetic data (§4) + boundary cases (§5)

Synthetic input allowed when **deterministic, distinguishable, representative, boundary-rich, privacy-safe, cleanup-tagged**. Mocked subsystems are NOT acceptable completion evidence. ≥3 boundary classes per code path (out of: empty/missing, single, max, max+1, min, min-1, wrong type, unicode/control-byte, duplicate, out-of-order, concurrent, authz/tenant, time edges, resource exhaustion).

### 1.5 Required evidence (§6)

| Evidence | Command |
|---|---|
| Build | `cargo build --release -p <crate>` |
| Test | `cargo test -p <crate>` (or scoped with reason) |
| Clippy | `cargo clippy -p <crate> --no-deps --tests --examples -- -D warnings` |
| **SoT readback artifact** | Production: `/zfs/archive/contextgraph/fsv/<task>-fsv/<artifact>.json` with `allPassed: true`. `/mnt/d/contextgraph/fsv/...` only when issue explicitly scopes a historical-only proof. |
| **Independent RocksDB verification** | Flush/drop/reopen through production opener (or separate handle), decode the same row back. |
| **GitHub issue closeout** | CLAIM before code; RESOLVED / PAUSE / BLOCKED after with command evidence, FSV path, hashes, residual risk. |

Doc-only tasks may replace build/test/clippy with direct file existence + checksum + GitHub-state readbacks.

Minimum SoT shape:
```json
{
  "task": "TASK-EXAMPLE-NNN",
  "sourceOfTruth": "<CF/key or absolute file path>",
  "trigger": "<exact command or action>",
  "expected": "<expected value>",
  "actual": "<actual value read independently>",
  "cases": [],
  "allPassed": true
}
```

### 1.6 Audit-the-task checklist (§7) — before starting

```bash
test -f <path>
rg "pub (struct|enum|fn) <symbol>" crates/ --type rust
rg "CF_MEJEPA_<NAME>" crates/
test -f docs/futurebuild/specs/FSV-PROTOCOL.md
git log --oneline -5 -- <path>
```

If audit contradicts the task → update the issue or escalate before writing code.

### 1.7 Occam evaluation (§9)

For non-trivial designs, record in the issue: approach chosen, alternatives considered, why chosen has fewer moving parts / stronger SoT evidence, what evidence would change the decision.

### 1.8 Failure modes that mean NOT done (§10)

Green tests are insufficient if any of these:
- Test checks a constant instead of exercising the implementation.
- Tested branch is inactive.
- CF row written but never flushed or reopened.
- Test uses a different opener or serializer than production.
- Missing values hidden with defaults.
- Async producer tasks spawned but not joined before readback.
- Evidence proves existence only, not field-level correctness.

### 1.9 Issue closeout (§11)

Every worked issue ends with exactly one of **RESOLVED** / **PAUSE** / **BLOCKED**. Comment must include files changed, commands run, FSV artifact path + hash, SoT readback summary, failures encountered, residual risk, follow-up issues. No `./memory/` files written.

---

## 2. Operator Runbook

### 2.1 Daily (every active day)

| Time | Action | Tool |
|---|---|---|
| Start | Run the six-issue GitHub queries from `AGENTS.md` | `gh issue list ...` |
| Start | Skim active `type:context` pin + any `priority:p0` open | `gh` |
| Throughout | Capture every observed defect / smell / pattern as an issue (Doctrine §3.2 File rule) | `gh issue create` |
| End | RESOLVED / PAUSE / BLOCKED on every worked issue (§11) | `gh issue comment` |
| Heal status | Check daemon liveness + heal status | `mejepa_daemon_status`, `mejepa_heal_status` |
| Subscriber | Confirm watermark progressing | `mejepa_subscriber_status` |

### 2.2 Weekly

| Action | Tool / Output |
|---|---|
| Operator cell triage | `mejepa_weekly_eval_dashboard` + `/zfs/archive/contextgraph/exports/weekly/<date>/weekly.md` (TASK-G-018 top-10 failing cells/actions) |
| Compression progress | `mejepa_compression_progress` → renders into weekly markdown via `render_compression_progress_weekly_section` (TASK-EK-001) |
| Negative-action ablation | `/zfs/archive/contextgraph/exports/weekly/<date>/ablation_report.md` (TASK-PY-G-054) |
| Oracle flakiness quarantine | `CF_MEJEPA_*` + weekly export (TASK-G-019) |
| Holdout-vs-training KL drift telemetry | `MEJEPA_TRAINING_HOLDOUT_DRIFT` metric (TASK-G-017) |
| Per-failure-mode-class precision/recall | TASK-G-020 reporting |
| Per-cell convergence ETA | TASK-G-021 |

### 2.3 On promotion (heal scheduler proposes a new active model)

1. Read `mejepa_ship_gate_status` (TASK-G-006 four-window stability).
2. Verify per-cell green/yellow/red (TASK-G-016 promotion gate).
3. First-promotion / catastrophic-class → operator approval via `mejepa_promote_approval` (TASK-EK-018H).
4. Active pointer flips → `CF_MEJEPA_ACTIVE_POINTERS` updated.
5. Audit: `CF_MEJEPA_MODEL_PROMOTIONS` row inserted.
6. Watch `mejepa_heal_status` for post-promotion drift (TASK-G-004 conformal recalibration).

### 2.4 On incident (test flakiness / drift spike / OOD spike)

| Symptom | First action | Diagnostic CF / tool |
|---|---|---|
| Many `Refuted` rows in `CF_MEJEPA_PREDICTION_VERIFICATIONS` | `mejepa_active_learning_queue` (rank by curiosity per TASK-EK-006) | active learning queue |
| OOD spike | `mejepa_audit_pairwise_mi` for embedder redundancy/synergy collapse; check `mejepa_explain_prediction.per_slot_ood_reasons` (TASK-PY-G-089) for slot-specific drivers; inspect harvest at `CF_MEJEPA_OOD_HARVEST` + selected `CF_MEJEPA_OOD_CALIBRATIONS` for `OOD_GATE_OVER_FLAGGING`, `OOD_CELL_AUC_REGRESSION`, `OOD_HARVEST_EMPTY`, or `OOD_CALIBRATOR_MISSING` | `CF_MEJEPA_PAIRWISE_MI`, `CF_MEJEPA_OOD_HARVEST`, `CF_MEJEPA_OOD_CALIBRATIONS` |
| Watermark stalled | `mejepa_subscriber_status` + check `/zfs/archive/contextgraph/runtime/shift-log.jsonl` size + tail health | shift-subscriber |
| Quota overflow | `mejepa_quota_status` then `mejepa_gc_run` | `CF_MEJEPA_QUOTA_STATE`, `CF_MEJEPA_GC_HISTORY` |
| Stuck heal | `mejepa_heal_status` + `mejepa_daemon_status` | `CF_MEJEPA_HEAL_REPORTS`, `CF_MEJEPA_DRIFT_HISTORY` |
| Witness chain bloat | `mejepa_witness_compress` | `CF_MEJEPA_WITNESS_CHAIN`, `CF_MEJEPA_WITNESS_SEGMENT_META` |
| Stop hook blocking unnecessarily | `~/.config/mejepa/daemon.toml::stop_hook_enabled = false` OR set `stop_hook_active=true` in env | `scripts/cc/stop-mejepa-self-verify.sh` |

### 2.5 Build, test, lint (CLAUDE.md §14)

```bash
cargo build --release -p context-graph-cli       # workhorse CLI
cargo build --release -p context-graph-mcp       # MCP server
cargo build --release -p <touched-crate>         # after any change
cargo test                                        # full suite
cargo test -p <touched-crate>                     # scoped
cargo clippy --no-deps --tests --examples -- -D warnings
cargo fmt
```

`cargo check` is NOT a substitute for `cargo build --release`.

### 2.6 GitHub Free-plan CI budget (Doctrine §3.20)

- `runs-on: ubuntu-latest` only.
- `timeout-minutes:` declared (≤30 default, ≤60 cap).
- Actions pinned to SHA (not tag).
- Cron: ≤ weekly for SAST, daily for stale.
- Never enable paid features.

---

## 3. Hooks (Claude Code outer-loop integration)

`scripts/cc/` — pre/post/stop hook scripts.

| Event | Script | What it does | SoT |
|---|---|---|---|
| UserPromptSubmit | inject context | Inject latest `RealityPrediction` row into agent context (TASK-PY-G-055 / #274) | `CF_MEJEPA_LIVE_PREDICTIONS` |
| PostToolUse (Bash matcher) | `posttooluse-mejepa-capture.sh` | Capture pytest/cargo-test/unittest/jest/vitest output, bind covered-chunk-overlap matches to current `prediction_id`, write `CF_MEJEPA_PREDICTION_VERIFICATIONS`, log `/zfs/archive/contextgraph/runtime/posttooluse-mejepa-capture.jsonl`, enqueue Refuted rows at 6× active-learning weight (TASK-PY-G-069 / #288) | `CF_MEJEPA_PREDICTION_VERIFICATIONS` |
| Stop | `stop-mejepa-self-verify.sh` | Invoke `context-graph-cli mejepa stop-self-verify`. Block Stop with exit code 2 when non-Abstain predictions lack Confirmed/Refuted rows. Honors `stop_hook_active` and `~/.config/mejepa/daemon.toml::stop_hook_enabled = false`. Logs `/zfs/archive/contextgraph/runtime/stop-mejepa-self-verify.jsonl`. TASK-PY-G-080 / #381 adds bounded BLOCKED/operator-action artifacts plus timeout breaker state for CLI-missing, DB-missing/stale, daemon-unreachable, verify-timeout, and repeated-timeout paths. | `CF_MEJEPA_PREDICTION_VERIFICATIONS` + log + blocked artifact |
| Observe ticker | `observe-ticker.sh` | Hard-label test outcomes; outcomes matched to a live prediction write `CF_MEJEPA_PREDICTION_VERIFICATIONS`. Outcomes with no live prediction → fail-closed `CF_MEJEPA_SIGNAL_DROP_LOG` row instead of poisoning daemon health (TASK-PY-G-337 / #337) | `CF_MEJEPA_PREDICTION_VERIFICATIONS` + `CF_MEJEPA_SIGNAL_DROP_LOG` |
| Pre-edit / post-edit | (hooks emit pre/post SHA + transition logs; governed path gate removed 2026-05-07) | Tracks edit lineage | hook log |

---

## 4. Disaster Recovery + Backup

### 4.1 What is sacred (cannot be recreated)

| Asset | Where | Recovery path |
|---|---|---|
| RocksDB CFs (production) | `/zfs/archive/contextgraph/` (aiwonder) | Restore from ZFS snapshot |
| FSV evidence (historical proof) | `/zfs/archive/contextgraph/fsv/` (target) / `/mnt/d/contextgraph/fsv/` (legacy 327 dirs) | Per-task issue tracking + JSON hashes; some evidence is single-event and irreplaceable |
| Operator `.env` / `.env.*` / `*.pem` / `*.key` / `credentials.json` | various | Operator-managed; backed up externally |
| Production corpus (forward cache) | `/zfs/archive/contextgraph/corpus/python-forward-cache-v1-aiwonder-prod-<TS>/shards/` | Regenerable from SWE-bench Lite + embedders (re-run TASK-PY-DATA-016) but very expensive (391 shards) |
| Shift log JSONL | `/zfs/archive/contextgraph/runtime/shift-log.jsonl` | Append-only — replayable; subscriber rebuilds panels |

### 4.2 What is regenerable

- All predictions (replay against shift log).
- All training checkpoints (re-run trainer from replay buffer).
- Project-ingest caches (re-ingest from source repos).
- Weekly exports.
- Embedder model weights (re-download from HF cache).

### 4.3 Backup posture

- **ZFS snapshots** on aiwonder for `/zfs/archive/contextgraph/`. Daily snapshots; weekly retention; monthly cold.
- **HF cache** is regenerable (re-download).
- **No off-site backup** of synthetic training corpora (too large, regenerable).
- **Off-site (or operator-side) backup required** for: ZFS snapshots of `/zfs/archive/...fsv/` (FSV historical proof) and `/zfs/archive/contextgraph/runtime/shift-log.jsonl` (replay source). Tracked by **#380 / TASK-AIWONDER-020 (Off-host backup for `/zfs/archive` proof history)** — open p1 follow-up to #296.

### 4.4 DR drill (when last run: track via GitHub `phase:f-followup` label)

1. Pause prediction serving: `mejepa_pause_predictions`.
2. Take fresh ZFS snapshot of `/zfs/archive/contextgraph/`.
3. Restore snapshot to test mountpoint.
4. Open RocksDB through production opener; assert `MEJEPA_HYGIENE_REFERENCED_CF_COUNT` CFs present.
5. Replay last N shift-log entries; confirm watermark advances + panels materialize.
6. Validate `mejepa_predict_latest` returns rows.
7. Resume prediction serving.

---

## 5. Observability

| Surface | Metric / dashboard | Source |
|---|---|---|
| Daemon liveness | `daemon_status` | `crates/context-graph-mcp/src/daemon.rs` |
| Shift subscriber metrics | latency, watermark, throughput | `crates/context-graph-mejepa-shift-subscriber/src/status.rs::SubscriberMetrics`, `LatencySnapshot` |
| Quota state | per-CF utilization, eviction count | `mejepa_quota_status` + `CF_MEJEPA_QUOTA_STATE` |
| GC report | rows touched per CF | `mejepa_gc_run` + `CF_MEJEPA_GC_HISTORY` |
| Heal status | drift window age, Fisher snapshot age, next planned action | `mejepa_heal_status` + `CF_MEJEPA_HEAL_REPORTS` |
| Ship-gate dashboard | per-cell `prediction_oracle_correlation` over 4 rolling windows | `mejepa_ship_gate_status` + `CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS` (TASK-G-006/008) |
| Production telemetry | per-event time-series | `CF_MEJEPA_PRODUCTION_TELEMETRY` (TASK-G-009) |
| Training-holdout KL drift | rolling KL divergence | `MEJEPA_TRAINING_HOLDOUT_DRIFT` metric (TASK-G-017) |
| Weekly cell triage | top-10 failing cells/actions | `weekly.md` (TASK-G-018) |
| Oracle flakiness | quarantined corpus tasks | TASK-G-019 + weekly export |
| Per-failure-mode-class precision/recall | per-class metrics | TASK-G-020 |
| Compression progress (CPΦ) | conditional description length bits / window | `mejepa_compression_progress` + `CF_MEJEPA_TRAIN_CERTS` (TASK-EK-001) |
| Active learning queue | size + per-source counts + curiosity ranking | `mejepa_active_learning_queue` + `CF_MEJEPA_ACTIVE_LEARNING_QUEUE` + `CF_MEJEPA_CURIOSITY_TELEMETRY` (TASK-EK-006) |
| Signal drop log | fail-closed reward-signal drops | `CF_MEJEPA_SIGNAL_DROP_LOG` (TASK-RWD-315) |
| OOD harvest | live-session above-threshold harvest, false-positive-rate audit, orphan/degenerate quarantine, expired tier-down | `CF_MEJEPA_OOD_HARVEST` + `CF_MEJEPA_OOD_CALIBRATIONS` (TASK-PY-G-057 / #276) |
| OOD serving gate | strict `mejepa_verify` selected-calibrator consumption, τ_ood threshold, empty-harvest fallback, missing/corrupt fail-closed, cell-regression flagging, cold-cell precedence | `CF_MEJEPA_OOD_CALIBRATIONS` + `/zfs/archive/contextgraph/fsv/task-py-g-014-ood-gate-fsv/run-20260519T104711Z/` (TASK-PY-G-014 / #129) |
| Per-slot OOD reasons | per-slot/embedder/pair OOD evidence persisted on each prediction row | `RealityPrediction.per_slot_ood_reasons` + `/zfs/archive/contextgraph/state/calibration/per-slot-v1/` (TASK-PY-G-089 / #389) |
| Daemon health endpoint | structured health probe | `crates/context-graph-mcp/src/health_probe.rs` |
| Multi-agent monitoring | per-agent + per-session liveness | `crates/context-graph-mcp/src/monitoring.rs` |

Telemetry emitted via `tracing` (workspace dep). JSON output via `tracing-subscriber` `json` feature; env filter via `env-filter`.

---

## 6. Hygiene (Phase 6 storage operations)

`crates/context-graph-mejepa-hygiene/src/`:

| Module | Responsibility | MCP surface |
|---|---|---|
| `categories.rs` | Categorize CF rows for retention | — |
| `entry.rs` | Row entry abstraction | — |
| `gc.rs` (~16K) | GC pass + report | `mejepa_gc_run` |
| `mcp.rs` | MCP wrappers | — |
| `ops.rs` | Hygiene primitive ops | — |
| `quota.rs` (~10K) | Quota enforcement | `mejepa_quota_status` |
| `quota_types.rs` | Quota DTOs | — |
| `reports.rs` | Report renderers | (weekly) |
| `retention.rs` | Retention policies (TTL, importance, replay weight) | — |
| `storage.rs` | RocksDB ops | — |
| `tier.rs` + `tier_frozen.rs` + `tier_ops.rs` | Hot → warm → frozen transitions; frozen tier handling | — |
| `witness_compress.rs` (~21K) | Witness-chain Merkle compression | `mejepa_witness_compress` |

FSV examples (per Cargo.toml): `merkle_compression_fsv`, `quota_overflow_fsv`, `gc_stress_fsv`, `phase_f_storage_exhaustion_fsv`, `phase_f_feedback_override_retention_fsv`.

---

## 7. Test Suite Inventory

### 7.1 Counts (2026-05-19)

- **Source `.rs` files**: 1,600 under `crates/*/src/`
- **Example files**: 210 under `crates/*/examples/` (mostly `*_fsv.rs` FSV harnesses)
- **Test files**: 258 under `crates/*/tests/`

Historical totals from memory:
- Total MCP+Storage tests at 2026-02-26: **1,179 passed, 0 failed** (post unified_neighbors FSV)
- ME-JEPA-foundation crates (2026-05-09e snapshot, mid-buildup): **251 tests**
- Total workspace (2026-02-23 snapshot before Phase G expansion): **6,353 passed, 0 failed**

Current counts shift weekly; the source of truth is `cargo test --release` (or per-crate `-p`). FSV count is per-issue: each closed Phase G issue documents its specific test pass count in its RESOLVED comment.

### 7.2 Test categories

| Category | Where | Notes |
|---|---|---|
| **Unit tests** | inline `#[cfg(test)] mod tests` per source file + `tests.rs` modules | most coverage |
| **Integration tests** | `crates/*/tests/*.rs` | 258 files |
| **FSV harnesses** | `crates/*/examples/*_fsv.rs` | 210 files. Each = one task. Output: JSON SoT readback artifact under `/mnt/d/contextgraph/fsv/<task>-fsv/` (legacy) or `/zfs/archive/contextgraph/fsv/<task>-fsv/` (post-migration target). |
| **Adversarial harnesses** | `crates/context-graph-mejepa/examples/phase_f_adversarial_corpus_fsv.rs`, `adversarial_corpus.rs` (30 cases) | TASK-PY-G-060 / TASK-FP-009 |
| **Synthetic stress harness** | `examples/task_py_g_071_synthetic_stress_fsv.rs` (alias) + `synthetic_stress.rs` (30 known-answer) | TASK-PY-G-071 |
| **End-to-end stress harness** | `project_stress.rs` | TASK-PY-G-072 |

### 7.3 How to run

```bash
# Full workspace test
cargo test --release

# Scoped to a crate
cargo test --release -p context-graph-mejepa

# Single FSV harness
cargo run --release -p context-graph-mejepa --example phase_f_adversarial_corpus_fsv

# Clippy + tests + examples (FSV-PROTOCOL §6.3)
cargo clippy -p <crate> --no-deps --tests --examples -- -D warnings

```

### 7.4 Key fixtures / utilities

- `context-graph-test-utils` — shared deterministic helpers. Cargo description: **"Shared test utilities for context-graph workspace — NO MOCK DATA"**. Mocks are forbidden for completion gates (FSV-PROTOCOL §3).
- `crates/context-graph-mejepa/src/fixtures.rs` — predictor test fixtures.
- `crates/context-graph-mejepa-corpus/src/tests.rs` — 81+ corpus unit tests including real `apply_mutation` on real Python.
- `crates/context-graph-mejepa-instruments/src/tests.rs` — 27+ instrument unit tests.

### 7.5 Code-simplifier discipline (CLAUDE.md §13)

Doctrine §16 / §17.1 / pre-completion check. Apply on every shipped primitive:
- Preserve functionality.
- Files ≤ 500 lines, functions ≤ 30 lines, cyclomatic ≤ 10.
- Reduce nesting; avoid nested ternaries; prefer `match` / `if let` / explicit `if / else if / else`.
- Choose clarity over brevity.

`file-length-manager` agent splits files at 1500 lines (last-resort safety net).

---

## 8. Security / Adversarial Robustness

Universal protocol → doctrine §14.2. Project-specific:

| Defense | Status | Source |
|---|---|---|
| Secret redaction (19 patterns, < 1ms p99 on 10KB, fail-closed on non-UTF8) | code/FSV-shipped 2026-05-15 (PHASE 1) | `scripts/swebench_lite_ops/python_secret_redaction.py`; wrapper at `crates/context-graph-mejepa/src/secret_redaction.rs`; sha256 `356fc2e4` |
| Cross-language label gating | code/FSV-shipped (Phase F) | `crates/context-graph-mejepa/src/cross_language_label_gating.rs` |
| Adversarial-embedder-injection mitigation | code/FSV-shipped (TASK-EK-018J) | `crates/context-graph-mejepa/src/adversarial_embedder_injection.rs` |
| 30-case adversarial Python corpus | code/FSV-shipped (TASK-PY-G-060) | `adversarial_corpus.rs` + `CF_MEJEPA_ADVERSARIAL_CORPUS` |
| Negative-action ablation falsifier | code/FSV-shipped (TASK-PY-G-054) | `CF_MEJEPA_ABLATION_REPORTS` + weekly `ablation_report.md` |
| Witness chain (ed25519 + Merkle) | code/FSV-shipped | `crates/context-graph-witness/` |
| Reality-loop legacy tools fail closed | code/FSV-shipped (Phase H) | `crates/context-graph-mcp/src/deprecation.rs::RETIRED_CGREALITY_TOOLS` |
| Inner-LLM compile-error if reanimated | code/FSV-shipped (Phase H) | `crates/context-graph-mcp/src/lib.rs:13` |
| Path-policy gate (server-side) | REMOVED 2026-05-07 (behavioral discipline only) | — |
| No HTTP from action paths | enforced behaviorally | — |
| No destructive git | enforced behaviorally | — |

---

## 9. Multi-Agent Isolation

| Mechanism | Source |
|---|---|
| Daemon validation | `crates/context-graph-mcp/src/daemon_validate.rs` |
| Multi-agent observability | `mejepa_daemon_status` + `monitoring.rs` |
| Per-session prediction binding | `(sessionId, prediction_id)` keys in `CF_MEJEPA_LIVE_PREDICTIONS` |
| Decoded session hex32 | `decode_session_hex32` in shift-subscriber models |
| `agent:*` GitHub labels | `agent:claude` / `agent:codex` |

---

## 10. API Stability

ME-JEPA-Code is pre-production (no SLA, no backwards compatibility per CLAUDE.md §6.6 / doctrine §15-equiv). Schema changes → wipe and rebuild. Future: TBD post-ship-gate; tracked in old 24_API_STABILITY doc (archived).

---

## 11. Cross-References

- Universal doctrine → `docs2/AICodingAgentSuperPrompt.md`
- Project addendum (invariants, paths) → `CLAUDE.md`
- Per-turn agent contract → `AGENTS.md`
- 14-axis hardening deep-dive → `docs2/hardeningprompt.md`
- Storage policy + CF inventory → `03_storage_schema.md`
- MCP tool reference → `04_mcp_tools.md`
- Predictor pipeline → `05_predictor_pipeline.md`
- Historical FSV evidence + ablation results → `07_results_and_benchmarks.md`
- Roadmap + open work → `08_roadmap_and_open_work.md`

---

## 12. What This Doc Does NOT Cover

- Exact test pass count workspace-wide (run `cargo test --release` for live count).
- Per-test timing or flake history (not centrally tracked beyond TASK-G-019 oracle flakiness quarantine).
- Operator personal hardware setup (`~/.config/mejepa/daemon.toml`, GPU driver versions, CUDA toolkit install) — outside scope, see CLAUDE.md §2 + AIWONDER-* issues.
- Off-aiwonder operator backup procedures — operator responsibility.

---

## 13. FSV Protocols for the 2026-05-19 Binary Doctrine

Three new FSV protocols enforce the binary-Q1/Q2/Q5 + labeled + mistake-driven + cross-panel-triangulated contract. Each protocol slots into the universal FSV-PROTOCOL §1.1–§1.5 (Source-of-truth, Trigger→Process→Outcome, hard rules, evidence, independent readback) — the additions below are protocol-specific assertions every FSV harness for these features must enforce.

### 13.1 Failure-mode labeling FSV — Level 0 SHIPPED via #413; Level 1/2/3 planned via #404

**Level 0 — SHIPPED 2026-05-19.** The deterministic label compiler (`python_auto_label_discovery.py`) ran on the full aiwonder corpus with FSV PASS. Reference assertions enforced by the shipped FSV at `/zfs/archive/contextgraph/fsv/task-py-g-117-auto-label-discovery-usefulness-fsv/python_auto_label_discovery_fsv.json` (manifest sha256 `3cd0e3c6…`):

1. **Slot identity preserved.** `slot_identity_preserved=true` and `flat_vector_concat_used=false` recorded in the FSV manifest; the compiler refuses to emit any label that would require cross-slot vector concatenation.
2. **No /mnt/d active I/O.** FSV records the input/output paths and asserts none reference `/mnt/d/...`.
3. **Manual review reproduction.** Compiler reproduces 12/12 deterministic conclusions of the prior 2026-05-19 manual pattern review at `/zfs/archive/contextgraph/fsv/task-py-g-110-manual-pattern-review/run-20260519T141122Z/manual_pattern_review.json` (sha256 `48a6cc61…`).
4. **Coverage.** Every readable chunk in the 399,777-row chunk corpus emits at least one Level-0 label; every readable slot output emits a region label; every supported same-row pair emits a relationship label or typed unsupported reason; every supported group emits a signature or typed insufficient-support reason.
5. **Leakage policy.** Outcome labels (oracle/docker/test-phase/code-state) marked `leakage_policy:target_side_only`; live predictor inputs do NOT include them; FSV enforces by walking the registry and rejecting any non-target label with `leaky_target=true`.
6. **Usefulness gate.** Of 1,800+ candidates, 491 entered the accepted registry; rejected labels carry typed reasons (`below_support`, `below_lift`, `unstable_across_partitions`, `degenerate_pass_only`, `degenerate_fail_only`, `leaky_target_on_live_input`).
7. **Independent readback.** Every output JSONL byte-replayed after a clean re-read; sha256 of each artifact recorded in the FSV manifest.

**Level 1/2/3 — Planned FSV assertions (epic #404 / TASK-PY-G-110), on top of the shipped Level-0 accepted registry:**

1. **Aggregation coverage.** Every shipped accepted label in the Level-0 registry maps to exactly one Level-1 group (no orphan accepted labels).
2. **Operator-rename idempotence.** Rename a group's name via `mejepa_label_failure_cluster`, flush, drop, reopen via production opener, decode the row back. Required: `group_id` unchanged, `operator_approved_name` updated, history preserved in `CF_MEJEPA_FAILURE_MODE_LEVEL1.audit_log`.
3. **Coverage on the holdout.** For every `Verdict::Fail` row in the 30-instance holdout window, `CF_MEJEPA_NAMED_PREDICTIONS` must contain a non-empty `NamedFailureMode`. Required: `named_mode_coverage_on_fail == 1.0`.
4. **Exemplar-oracle agreement.** For each named failure mode, the top-1 closest exemplar's oracle outcome must match the prediction's oracle outcome ≥ 85% of the time. Required: `closest_exemplar_oracle_agreement ≥ 0.85`.
5. **Vocabulary-seed audit.** Each Level-2 skill name must trace back to either a published seed source (Aleithan 2025 / Yang 2024 / SWE-Bench Pro 2025 / our 8 mutations / our adversarial corpus) OR an operator-approved extension. Required: `unsourced_skills == 0`.

### 13.2 Cross-panel triangulation FSV (epic #405 / TASK-PY-G-111)

Required SoT assertions:

1. **Both panels on same instances.** Every gate-relevant prediction in the holdout window has rows in both `CF_MEJEPA_LIVE_PREDICTIONS` (Panel A) AND `CF_MEJEPA_PANEL_B_OBSERVATIONS` (Panel B) AND `CF_MEJEPA_CROSS_PANEL_AGREEMENT`. Required: `unmatched_predictions == 0`.
2. **Independent Panel-B readback.** Flush, drop, reopen via production opener. Decode the same `prediction_id` row from `CF_MEJEPA_PANEL_B_OBSERVATIONS`. Required: byte-equality with the pre-flush row.
3. **Goodhart-detection edge.** Inject a synthetic divergence where Panel A `corr ≥ 0.95` but Panel B `corr < 0.85`. Required: `CROSS_PANEL_GOODHART_DETECTED` flag fires on the matching window in `CF_MEJEPA_CROSS_PANEL_AGREEMENT`, AND the heal scheduler records a `ship_gate_blocked_by_cross_panel` flag.
4. **Encoder non-overlap audit.** Panel A encoders and Panel B encoders must use distinct model artifacts; SHA-256 manifest pins differ for every learned-encoder slot. Required: `panel_a_b_artifact_overlap_count == 0` on the 5 learned-encoder slot pairs.
5. **VRAM budget audit.** Panel B encoders never resident during normal inference; only loaded during the evaluation worker's pass. Required: `panel_b_resident_during_inference == false` measured against `mejepa_vram_budget_report`.

### 13.3 Mistake-loop convergence FSV (epic #406 / TASK-PY-G-112)

Required SoT assertions:

1. **Mistake landing.** Inject a deliberate wrong prediction. Required: `CF_MEJEPA_MISTAKE_LOG` contains a new row with the correct `predicted_verdict` / `ground_truth_verdict` / `cell_key` / `panel_signature_hash`. Independent readback after flush/drop/reopen.
2. **Replay-buffer boost.** After mistake row is written, `CF_MEJEPA_MISTAKE_REPLAY_PRIORITY[mistake_id].boost_factor == 8.0` AND `CF_MEJEPA_REPLAY_BUFFER` has a corresponding priority-boosted entry.
3. **Convergence on second presentation.** Present the same `panel_signature_hash` to the predictor again after one trainer step. Required: `Verdict` matches ground truth on the second presentation. Persisted as a regression case in `examples/task_py_g_112_mistake_loop_convergence_fsv.rs` with byte-replayable evidence.
4. **Exact-panel unrelated-control isolation.** The shipped #406 online-head update is key-local exact-panel state, not a parameter-level Fisher/EWC update. Required: an unrelated control `panel_signature_hash` is predicted before and after the update, its verdict is unchanged, and `unrelated_cell_accuracy_delta` is within ±0.02. The #524 guard must reject any report that sets `claims_fisher_ewc_protection=true` without a durable Fisher/EWC row key.
5. **`mistake_repeat_rate` measurement.** Compute the rolling rate across a 100-mistake window. Required for ship eligibility: `mistake_repeat_rate ≤ 0.05`. Byte-readable from `CF_MEJEPA_MISTAKE_LOG` aggregations + `mejepa_mistake_loop_status`.

### 13.4 Q4 freeze audit FSV (one-time, as part of the doctrine commit)

Required SoT assertions (one-time at the doctrine landing):

1. **No new Q4 producer writes after 2026-05-19.** Read `CF_MEJEPA_Q4_*` row counts at landing; persist as `q4_freeze_baseline.json`. Subsequent ingest runs MUST not increase these counts. Required: `q4_row_count_delta == 0` weekly.
2. **No Q4 contribution to predictor loss.** Audit the trainer's loss-coefficient configuration: every Q4-related coefficient (`inverse_map_coefficient` for Q4 heads, any per-head coefficient on Q4 modules) equals 0. Required: `q4_loss_coefficient_max == 0.0`.
3. **No Q4 in ship-gate metric.** Audit `mejepa_ship_gate_status` and `swebench_300x8_ship_gate_fsv` to confirm they read only Q2 oracle correlation, not Q4 trust eligibility. Required: `q4_inputs_to_ship_gate == 0`.
4. **Read-only Q4 surface still works.** `mejepa_predict_latest` returns historical Q4 fields for predictions made before 2026-05-19; new predictions return empty Q4 fields. Required: a one-time FSV that probes both pre- and post-freeze predictions and confirms.

These four assertions are bundled into a single FSV harness: `examples/q4_doctrine_freeze_fsv.rs` under `context-graph-mejepa`, with output at `/zfs/archive/contextgraph/fsv/q4-doctrine-freeze-fsv/` and a recurring weekly recheck driven by the `mejepa_weekly_eval_dashboard` pipeline.

### 13.5 Disjoint calibration partition (10th-percentile floor)

Per `01_overview.md §1.1` and ClipCannon PAPER §5.4 same-session-band-floor pattern: every τ in `CF_MEJEPA_FAILURE_FINGERPRINTS` (Gτ thresholds), `CF_MEJEPA_OOD_CALIBRATIONS` (OOD τ), and the new `CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS` confidence cutoffs MUST be computed from a calibration partition that is **disjoint from both train and eval**. The standing FSV `task-py-g-077-secret-holdout-fsv` already verifies disjoint partitioning at the corpus level (`overlap=0`); a new audit `examples/threshold_calibration_provenance_fsv.rs` walks every τ in the live RocksDB and asserts:

- The τ has a recorded `calibration_partition_id` field.
- The partition's instance-ID list does NOT overlap with the train partition's instance-IDs (`overlap_with_train == 0`).
- The partition's instance-ID list does NOT overlap with the eval partition (`overlap_with_eval == 0`).
- The τ value matches the 10th-percentile floor of authentic-sample cosines on the calibration partition, recomputed at audit time (tolerance ±1e-4).

Failures of any assertion fail closed with `THRESHOLD_CALIBRATION_NONDISJOINT` and block ship eligibility for that cell.

### 13.6 Information-physics FSV protocols (planned, #511 / #512 / #513 / #514)

Source: `docs/INFORMATION_PHYSICS_AND_STARS_AS_VECTORS_REPORT.md`. Four new FSV harnesses enforce the info-physics extensions on top of the existing critical-path assertions:

**#511 / TASK-IP-001 — entropy auxiliary loss FSV.** Required assertions:

1. **No-regression baseline.** With λ=0 the trainer's `L_step` must be byte-equal to a baseline reference recorded at `/zfs/archive/contextgraph/fsv/task-ip-001-entropy-loss-fsv/baseline_l_step.json`. Required: `lambda_zero_l_step_byte_equal == true`.
2. **Monotonic entropy decrease.** Across N≥4 training windows with λ > 0, the Kraskov entropy estimate of the latent must monotonically decrease (allowing a small noise floor). Required: `latent_entropy_monotonicity_violations == 0`.
3. **Brier no-regression.** Holdout Brier score with λ > 0 must be ≤ baseline Brier + 0.005. If exceeded, λ auto-zeroes via the calibrator; FSV records the auto-zeroing event.
4. **Degenerate-latent fail-closed.** Inject a constant latent. Required: `UTML_LATENT_ENTROPY_DEGENERATE` typed error on the next training step; no NaN/-∞ propagation.

**#512 / TASK-IP-002 — panel ECC FSV.** Required assertions:

1. **Clean-panel syndrome zero.** Every cleanly-built `Panel` has `panel.syndrome() == 0`. Required across the 30-instance holdout window.
2. **2-slot corruption recoverable.** Synthetic 1-slot and 2-slot corruption are reconstructed correctly; reconstructed `Panel` byte-equals the original up to the recovered slots. Required: 100% recovery rate within radius `t = 2`.
3. **3+-slot corruption detected.** Synthetic 3-slot corruption triggers `PANEL_ECC_UNCORRECTABLE` and writes a `CF_MEJEPA_MISTAKE_LOG` row. Required: 100% detection rate; zero false negatives.
4. **All #394 fixtures preserved.** Existing `python_anti_compensation_fsv.rs` fixtures continue to be rejected. Required: regression suite unchanged.

**#513 / TASK-IP-003 — connectome topology FSV.** Required assertions:

1. **Rich-club above null.** At least 5 of 8 Python cells have rich-club coefficient φ > random-graph null (configuration-model permutation test, n_perms=1000, p < 0.05).
2. **Reproducibility across splits.** Two random splits of the corpus identify rich-club pairs with Jaccard similarity ≥ 0.8.
3. **Modularity above null.** At least 5 of 8 cells have modularity Q above the null distribution (p < 0.05).
4. **Ablation re-run.** #398-style ablation with topology features enabled vs disabled, matched corpus + calibration. Correlation comparison reported and persisted in FSV.

**#514 / TASK-IP-005 — cold-load behavioural FSV.** Required assertions:

1. **Promotion snapshot persisted.** Every promotion writes `CF_MEJEPA_COLD_LOAD_SNAPSHOTS[promotion_id]` with `oracle_correlation` and `per_cell_verdict_distribution`. Required: `snapshot_persist_count_per_promotion == 1`.
2. **Cold-load correlation drift.** After fresh-process reload, 30-instance holdout correlation must be within `0.02` of the snapshot value. Required: `correlation_drift ≤ 0.02`.
3. **Cell verdict distribution drift.** Per-cell distributions must be within `5%` of the snapshot. Required: `max_per_cell_distribution_drift ≤ 0.05`.
4. **Three independent cold-load runs.** Three sequential runs on different aiwonder restart cycles all pass; FSV JSONs pinned by sha256 at `/zfs/archive/contextgraph/fsv/task-ip-005-cold-load-fsv/run-{1,2,3}/`.
5. **ECC syndrome consistency (after #512).** Each cold-load run reproduces the same panel ECC syndromes for the holdout instances as the original promotion-time run. Required: `syndrome_drift_per_panel == 0`.

### 13.7 Creativity-engine FSV protocols (post-0.95, #516 / #517 / #518 / #519 / #520)

Source: `docs/INFORMATION_PHYSICS_AND_STARS_AS_VECTORS_REPORT.md` + creativity-engine synthesis. All five FSV harnesses are post-0.95; they validate the autonomous-bounded-growth + storage-management loop. None override existing §13 protocols.

**#516 / TASK-CR-001 — Novelty + Utility oracle FSV.** Required assertions:

1. **E1-rotation rejection.** A synthetic candidate that is a deterministic rotation of E1's output → REJECTED with `NOVELTY_BELOW_TAU`; max per-slot correlation > τ_novelty recorded.
2. **Random-Gaussian rejection.** A candidate emitting independent Gaussian noise per chunk → REJECTED with `UTILITY_BELOW_DELTA`; orthogonal but contributes no oracle-correlation lift.
3. **Informative + orthogonal acceptance.** A candidate that adds a deterministically-derivable structural signature uncorrelated with existing slots AND improves per-cell oracle correlation by ≥ δ_utility → ACCEPTED; the per-cell Δ_oracle_correlation persisted in the audit row.
4. **Holdout regression rejection.** A candidate that lifts the train partition's correlation but regresses the disjoint holdout → REJECTED with `HOLDOUT_REGRESSION`.
5. **Audit table readback.** Independent re-open of `CF_MEJEPA_EMBEDDER_PROPOSAL_AUDIT` reads all four decision rows; sha256 pinned.

**#517 / TASK-CR-002 — Mistake-driven embedder demand FSV.** Required assertions:

1. **No-demand path.** A mistake where slot-attribution has a clear winner (max per-slot attribution > τ_attribution_floor) → no demand row written; the standard #406 mistake-loop path runs.
2. **Demand path.** A mistake where all per-slot attribution scores are below τ_attribution_min → `CF_MEJEPA_EMBEDDER_DEMAND` row written with the residual signature, closest-inadequate slots, cell ID, and source mistake ID.
3. **End-to-end composition.** Pipe a demand row's residual signature into the #516 novelty+utility oracle as a candidate seed; confirm the gating path runs (accept or reject) and the resulting audit row references the demand row.
4. **Idempotence.** Re-running the same mistake with the same attribution does not write duplicate demand rows; existing demand row is updated with incremented `recurrence_count`.

**#518 / TASK-CR-003 — Embedder retirement audit FSV.** Required assertions:

1. **Dominated-embedder identification.** Inject a synthetic E1-rotation embedder into the active panel; periodic retirement audit must identify it as dominated by E1, run ablation, confirm `Δ_oracle_correlation < δ_retire`, write retirement audit row.
2. **Grace period.** Embedder marked `archived_pending_grace_period` for K windows; during grace period it is still computed on new chunks; after K windows passes without re-emergence of need, transitioned to `archived` and stops running on new chunks.
3. **Informative-embedder preservation.** An informative non-redundant embedder → NOT identified for retirement; retirement audit row NOT written.
4. **End-to-end storage savings.** Measure pre-retirement and post-retirement storage; the freed bytes match the predicted `storage_bytes_freed` field in the audit row within 5% tolerance.
5. **Operator override.** Force-retire via `context-graph-cli mejepa retire-embedder <id>` writes the audit row with `operator_attribution` populated; independent ldb readback confirms.

**#519 / TASK-CR-004 — Domain-routed sparse panel FSV.** Required assertions:

1. **Per-domain routing.** A Python chunk fires universal slots + Python-extension slots; Rust-extension slots NOT fired (filled_mask bits explicitly zero). Same for Rust chunk in the opposite direction.
2. **Cross-domain reclassification.** A chunk's domain changes (Python → TypeScript by re-classifier); old extension's filled_mask bits clear; new extension's bits set; universal subset unchanged.
3. **Storage measurement.** Total storage for a mixed-domain corpus is 3-5× smaller than the same chunks under a unified panel that fires every embedder on every chunk; reported delta matches prediction within 10%.
4. **Predictor compatibility.** The slot-preserving late-fusion predictor (#387) handles the variable-K filled_mask correctly; per-cell oracle correlation does NOT regress vs the unified-panel baseline within `±0.005`.
5. **Domain registry readback.** Independent ldb readback of `CF_MEJEPA_DOMAIN_REGISTRY` returns the same domain manifest as the in-memory state.

**#520 / TASK-CR-005 — Episodic-to-semantic consolidation FSV.** Required assertions:

1. **Settled-chunk consolidation.** A chunk near its cluster centroid (distance < outlier_threshold) with stable cluster assignment for ≥ stability_window weeks → consolidated to `(cluster_id, residual)`.
2. **Outlier preservation.** A chunk above the outlier threshold OR with unstable cluster assignment → NOT consolidated; remains raw in `CF_MEJEPA_PANELS`.
3. **Lossless reconstruction.** Settled chunk's full panel reconstructed as `cluster_centroid + residual`; reconstructed panel byte-equals original within float epsilon (1e-6 per element).
4. **Storage measurement.** Pre-consolidation corpus size vs post-consolidation; predicted savings match measured to within 5% tolerance.
5. **Witness chain integrity.** Consolidation audit row includes `before_witness_hash` and `after_witness_hash`; both readable from independent ldb; provenance chain from raw to consolidated is verifiable.
