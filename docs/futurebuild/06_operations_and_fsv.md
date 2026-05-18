# 06 — Operations, FSV, Runbook, DR, Test Suite

**Last refreshed:** 2026-05-18.

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
| **SoT readback artifact** | Production: `/zfs/archive/contextgraph/fsv/<task>-fsv/<artifact>.json` with `allPassed: true`. `/mnt/d/contextgraph/fsv/...` only when issue explicitly scopes a local-only smoke. |
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
| OOD spike | `mejepa_audit_pairwise_mi` to see embedder redundancy/synergy collapse | `CF_MEJEPA_PAIRWISE_MI` |
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
| Stop | `stop-mejepa-self-verify.sh` | Invoke `context-graph-cli mejepa stop-self-verify`. Block Stop with exit code 2 when non-Abstain predictions lack Confirmed/Refuted rows. Honors `stop_hook_active` and `~/.config/mejepa/daemon.toml::stop_hook_enabled = false`. Logs `/zfs/archive/contextgraph/runtime/stop-mejepa-self-verify.jsonl`. (TASK-PY-G-070 / #289) | `CF_MEJEPA_PREDICTION_VERIFICATIONS` + log |
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
- **Off-site (or operator-side) backup required** for: ZFS snapshots of `/zfs/archive/...fsv/` (FSV historical proof) and `/zfs/archive/contextgraph/runtime/shift-log.jsonl` (replay source).

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

### 7.1 Counts (2026-05-18)

- **Source `.rs` files**: 1,567 under `crates/*/src/`
- **Example files**: 189 under `crates/*/examples/` (mostly `*_fsv.rs` FSV harnesses)
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
| **FSV harnesses** | `crates/*/examples/*_fsv.rs` | 189 files. Each = one task. Output: JSON SoT readback artifact under `/mnt/d/contextgraph/fsv/<task>-fsv/` (legacy) or `/zfs/archive/contextgraph/fsv/<task>-fsv/` (post-migration target). |
| **Adversarial harnesses** | `crates/context-graph-mejepa/examples/phase_f_adversarial_corpus_fsv.rs`, `adversarial_corpus.rs` (30 cases) | TASK-PY-G-060 / TASK-FP-009 |
| **Synthetic stress harness** | `examples/task_py_g_071_synthetic_stress_fsv.rs` (alias) + `synthetic_stress.rs` (30 known-answer) | TASK-PY-G-071 |
| **End-to-end stress harness** | `project_stress.rs` | TASK-PY-G-072 |
| **Benchmark suite** | `crates/context-graph-benchmark/` (excluded from default build; 87K LOC; 4 `[[bin]]` targets) | TASK-EMB-* benchmarks |

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

# Benchmarks (explicit)
cargo bench --release -p context-graph-benchmark --bench maxsim_bench
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
