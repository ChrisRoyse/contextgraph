# 06 — Operations, FSV, Runbook, DR, Test Suite

**Source files:** `docs/futurebuild/specs/FSV-PROTOCOL.md`, `AGENTS.md`, `CLAUDE.md`, `docs2/AICodingAgentSuperPrompt.md`, `docs2/hardeningprompt.md`, `scripts/cc/*.sh`, `crates/context-graph-mejepa-hygiene/src/{quota,gc,reports,retention,witness_compress}.rs`, `crates/context-graph-mejepa-shift-subscriber/src/`, `crates/context-graph-mcp/src/{daemon.rs, daemon_validate.rs, health_probe.rs, monitoring.rs, deprecation.rs}`.

---

## 1. FSV protocol — the manual SoT contract

`docs/futurebuild/specs/FSV-PROTOCOL.md` is binding contract for every task. Universal protocol in `docs2/AICodingAgentSuperPrompt.md`.

### 1.1 Source-of-truth first

> *A return value is a claim. The Source of Truth is the verdict.*

Before declaring a task complete, answer all four:
1. **What state changed?** File, CF row, metric, queue item, report, external record.
2. **Where does that state live?** Exact path, CF name + key, metric name, external identifier.
3. **How is it read back independently?** Separate DB open, file read, checksum, source command.
4. **What exact value is expected?** Literal value, schema, count delta, hash, bounded range.

If any answer is unknown → not done.

### 1.2 Trigger → Process → Outcome

Every FSV-able feature has this shape. Document before implementation: trigger (MCP / CLI / hook / scheduler), process (functions executed), outcome (physical SoT readback with expected value).

### 1.3 Hard rules

1. **No mocks** for completion gates.
2. **No fallbacks** that hide required-output failure.
3. No verifier weakening, ignored tests, silent bypasses.
4. **Tests must fail when the feature is broken.**
5. Missing CFs / files / models / env vars / malformed inputs → fail closed with `SCREAMING_SNAKE_CASE` codes.
6. **Assume the task is wrong** until live files / symbols / CF names prove it current.
7. Prefer smallest implementation satisfying the issue + matching local patterns.

### 1.4 Synthetic data + boundary cases

Synthetic input is allowed when **deterministic, distinguishable, representative, boundary-rich, privacy-safe, cleanup-tagged**. Mocked subsystems are NOT acceptable completion evidence. ≥3 boundary classes per code path.

### 1.5 Required evidence

| Evidence | Command |
|---|---|
| Build | `cargo build --release -p <crate>` |
| Test | `cargo test -p <crate>` |
| Clippy | `cargo clippy -p <crate> --no-deps --tests --examples -- -D warnings` |
| **SoT readback artifact** | Production: `/zfs/archive/contextgraph/fsv/<task>-fsv/<artifact>.json` (when applicable) |
| **Independent RocksDB verification** | Flush/drop/reopen through production opener, decode same row back |
| **GitHub issue closeout** | CLAIM before code; RESOLVED / PAUSE / BLOCKED after with command evidence, FSV path, hashes, residual risk |

### 1.6 Audit-the-task checklist — before starting

```bash
test -f <path>
rg "pub (struct|enum|fn) <symbol>" crates/ --type rust
rg "CF_MEJEPA_<NAME>" crates/
test -f docs/futurebuild/specs/FSV-PROTOCOL.md
git log --oneline -5 -- <path>
```

If the audit contradicts the task → update the issue or escalate before writing code.

### 1.7 Failure modes that mean NOT done

Green tests are insufficient if any of these:
- Test checks a constant instead of exercising implementation.
- Tested branch is inactive.
- CF row written but never flushed or reopened.
- Test uses a different opener or serializer than production.
- Missing values hidden with defaults.
- Async producer tasks spawned but not joined before readback.
- Evidence proves existence only, not field-level correctness.
- **Self-referential static-source tests** — tests that grep their own file for forbidden literals always self-match (use a separate tool/script).

### 1.8 Issue closeout

Every worked issue ends with exactly one of **RESOLVED** / **PAUSE** / **BLOCKED**. The comment must include files changed, commands run, FSV artifact path + hash (when applicable), SoT readback summary, failures encountered, residual risk, and follow-up issues.

### 1.9 Manual SoT contract (binding under #633 / #655)

Every issue tied to the #633 hardening gate MUST follow this contract; auto-FSV JSON alone is not admissible evidence. Auto-FSV harness ecosystem (`crates/*/examples/*_fsv.rs`) is being retired per #655.

1. **Define the Source of Truth.** Exact file path on aiwonder ZFS, RocksDB CF/key, systemd unit name, Docker container/image, ZFS dataset, model cache path, metric/log path, or external oracle artifact. No vague references.
2. **Capture BEFORE.** Manually read the SoT (via `ssh croyse@aiwonder.mst.com`, `contextgraph-shell`, `ldb`, `cat`, `gh`, `find -printf`, `sha256sum`, etc.) and paste the observed state directly into the issue comment.
3. **Execute the trigger.** Concrete command with synthetic-known input where possible.
4. **Capture AFTER.** Independently re-read the SoT (preferably from a new shell to avoid stale state) and paste the observed delta.
5. **Exercise happy path + ≥3 edge classes.** Edge classes: empty/missing input, malformed input, oversize input, concurrent access, durability across restart, partial failure, idempotent re-run.
6. **On mismatch:** stop, root-cause, fix or file a linked blocker issue, repeat manual FSV after fix. Do not paper-over with a re-run.
7. **No child closes from a return code or generated JSON alone.** The issue comment must show direct readback.
8. **No mounted-drive evidence.** Aiwonder ZFS only (`/zfs/archive/contextgraph/...`, `/zfs/hot/contextgraph/...`). `/mnt/d` / `/mnt/c` references are blockers, not evidence.

**Why this contract.** Return values lie. Direct bytes/rows readback by a human-or-agent that did not author the harness is the bias-defeating verification primitive.

---

## 2. Operator runbook

### 2.1 Daily

| Time | Action | Tool |
|---|---|---|
| Start | Run six-issue GitHub queries from `AGENTS.md` | `gh issue list ...` |
| Start | Skim active `type:context` pin + `priority:p0` open | `gh` |
| Throughout | Capture observed defect / smell / pattern as issue (Doctrine §3.2 File rule) | `gh issue create` |
| End | RESOLVED / PAUSE / BLOCKED on every worked issue | `gh issue comment` |
| Heal status | Check daemon liveness + heal status | `mejepa_daemon_status`, `mejepa_heal_status` |
| Subscriber | Confirm watermark progressing | `mejepa_subscriber_status` |
| Mistake loop | Check `mejepa_mistake_loop_status` for `mistake_repeat_rate` trend | shipped #523 |

### 2.2 Weekly

| Action | Tool / Output |
|---|---|
| Operator cell triage | `mejepa_weekly_eval_dashboard` + `/zfs/archive/contextgraph/exports/weekly/<date>/weekly.md` (top-10 failing cells/actions) |
| Compression progress | `mejepa_compression_progress` → renders into weekly markdown |
| Negative-action ablation | `exports/weekly/<date>/ablation_report.md` |
| Oracle flakiness quarantine | weekly export |
| Holdout-vs-training KL drift | `MEJEPA_TRAINING_HOLDOUT_DRIFT` metric |
| Per-failure-mode-class precision/recall | weekly export |
| Per-cell convergence ETA | weekly export |
| Q4 freeze monitor | `mejepa_weekly_eval_dashboard.q4Freeze` — fails closed if any frozen Q4 CF grows |

### 2.3 On promotion

1. Read `mejepa_ship_gate_status` (four-window stability).
2. Verify per-cell green/yellow/red.
3. First-promotion / catastrophic-class → operator approval via `mejepa_promote_approval`.
4. Active pointer flips → `CF_MEJEPA_ACTIVE_POINTERS` updated.
5. Audit: `CF_MEJEPA_MODEL_PROMOTIONS` row inserted.
6. Watch `mejepa_heal_status` for post-promotion drift.

### 2.4 On incident

| Symptom | First action | Diagnostic |
|---|---|---|
| Many `Refuted` rows | `mejepa_active_learning_queue` (rank by curiosity) | active learning queue |
| OOD spike | `mejepa_audit_pairwise_mi`; check `mejepa_explain_prediction.per_slot_ood_reasons`; inspect `CF_MEJEPA_OOD_HARVEST` + selected `CF_MEJEPA_OOD_CALIBRATIONS` for `OOD_GATE_OVER_FLAGGING`, `OOD_CELL_AUC_REGRESSION`, `OOD_HARVEST_EMPTY`, `OOD_CALIBRATOR_MISSING` | `CF_MEJEPA_PAIRWISE_MI`, OOD CFs |
| Watermark stalled | `mejepa_subscriber_status` + check `/zfs/archive/contextgraph/runtime/shift-log.jsonl` size + tail health | shift-subscriber |
| Quota overflow | `mejepa_quota_status` then `mejepa_gc_run` | `CF_MEJEPA_QUOTA_STATE`, `CF_MEJEPA_GC_HISTORY` |
| Stuck heal | `mejepa_heal_status` + `mejepa_daemon_status` | `CF_MEJEPA_HEAL_REPORTS`, `CF_MEJEPA_DRIFT_HISTORY` |
| Witness chain bloat | `mejepa_witness_compress` | `CF_MEJEPA_WITNESS_CHAIN`, `CF_MEJEPA_WITNESS_SEGMENT_META` |
| Mistake repeat rate climbing | `mejepa_mistake_history` filter by cell; investigate consolidation | `CF_MEJEPA_MISTAKE_LOG`, `CF_MEJEPA_ONLINE_HEAD_STATE` |
| Stop turn lacks manual evidence | Add GitHub issue comment with direct SoT readback + any required manual recommendation artifact | `scripts/cc/hook_stop.sh` |

### 2.5 Build, test, lint

```bash
cargo build --release -p context-graph-cli       # workhorse CLI
cargo build --release -p context-graph-mcp       # MCP server
cargo build --release -p <touched-crate>         # after any change
cargo check --workspace --tests --examples       # MANDATORY after changing any shared type
cargo test                                        # full suite
cargo test -p <touched-crate>                     # scoped
cargo clippy --no-deps --tests --examples -- -D warnings
cargo fmt
```

`cargo check` is NOT a substitute for `cargo build --release`. `cargo check --workspace --tests --examples` IS REQUIRED after touching any type, trait, or function signature that crosses crate boundaries — `cargo build --release` does NOT compile test or example targets.

### 2.6 GitHub Free-plan CI budget

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
| UserPromptSubmit | inject context | Inject latest `RealityPrediction` row into agent context | `CF_MEJEPA_LIVE_PREDICTIONS` |
| PostToolUse (Bash matcher) | `posttooluse-mejepa-capture.sh` | Capture pytest/cargo-test/unittest/jest/vitest output, bind covered-chunk-overlap matches to current `prediction_id`, write `CF_MEJEPA_PREDICTION_VERIFICATIONS`, log capture JSONL, enqueue Refuted rows at 6× active-learning weight | `CF_MEJEPA_PREDICTION_VERIFICATIONS` |
| Stop | `hook_stop.sh` | Require structured handoff/recommendation marker for current turn. Does not perform automated ME-JEPA self-verification; closure evidence must be manual GitHub issue comments with direct SoT readback. | recommendation artifact + issue comment |
| Observe ticker | `observe-ticker.sh` | Hard-label test outcomes; matched → `CF_MEJEPA_PREDICTION_VERIFICATIONS`; unmatched → `CF_MEJEPA_SIGNAL_DROP_LOG` (fail-closed, no daemon-health poisoning) | `CF_MEJEPA_PREDICTION_VERIFICATIONS` + `CF_MEJEPA_SIGNAL_DROP_LOG` |
| Pre-edit / post-edit | (governed path gate removed; hooks emit pre/post SHA + transition logs) | Tracks edit lineage | hook log |

---

## 4. Disaster recovery + backup

### 4.1 What is sacred

| Asset | Where | Recovery |
|---|---|---|
| RocksDB CFs (production) | `/zfs/archive/contextgraph/` | Restore from ZFS snapshot |
| FSV evidence (historical proof) | `/zfs/archive/contextgraph/fsv/` | Per-task issue tracking + JSON hashes; some evidence is single-event |
| Operator `.env` / `.env.*` / `*.pem` / `*.key` / `credentials.json` | various | Operator-managed; backed up externally |
| Production corpus (forward cache) | `/zfs/archive/contextgraph/corpus/python-forward-cache-*/` | Regenerable from SWE-bench Lite + embedders (~24h GPU) |
| Shift log JSONL | `/zfs/archive/contextgraph/runtime/shift-log.jsonl` | Append-only — replayable; subscriber rebuilds panels |

### 4.2 What is regenerable

- All predictions (replay against shift log).
- All training checkpoints (re-run trainer from replay buffer).
- Project-ingest caches (re-ingest from source repos).
- Weekly exports.
- Embedder model weights (re-download from HF cache).

### 4.3 Backup posture

- **ZFS snapshots** on aiwonder for `/zfs/archive/contextgraph/`. Daily snapshots; weekly retention; monthly cold.
- **HF cache** is regenerable.
- **No off-site backup** of synthetic training corpora (too large, regenerable).
- **Off-site backup required** for: `/zfs/archive/...fsv/` (FSV historical proof) and `/zfs/archive/contextgraph/runtime/shift-log.jsonl` (replay source). Tracked by **#380 / TASK-AIWONDER-020** — open p1.

### 4.4 DR drill

1. Pause prediction serving: `mejepa_pause_predictions`.
2. Take fresh ZFS snapshot of `/zfs/archive/contextgraph/`.
3. Restore snapshot to test mountpoint.
4. Open RocksDB through production opener; assert `MEJEPA_HYGIENE_REFERENCED_CF_COUNT` CFs present.
5. Replay last N shift-log entries; confirm watermark advances + panels materialize.
6. Validate `mejepa_predict_latest` returns rows.
7. Resume prediction serving.

---

## 5. Observability

| Surface | Source |
|---|---|
| Daemon liveness | `daemon_status` (`crates/context-graph-mcp/src/daemon.rs`) |
| Shift subscriber metrics (latency, watermark, throughput) | `SubscriberMetrics`, `LatencySnapshot` |
| Quota state | `mejepa_quota_status` + `CF_MEJEPA_QUOTA_STATE` |
| GC report | `mejepa_gc_run` + `CF_MEJEPA_GC_HISTORY` |
| Heal status | `mejepa_heal_status` + `CF_MEJEPA_HEAL_REPORTS` |
| Ship-gate dashboard (per-cell correlation over 4 rolling windows) | `mejepa_ship_gate_status` + `CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS` |
| Production telemetry | `CF_MEJEPA_PRODUCTION_TELEMETRY` |
| Training-holdout KL drift | `MEJEPA_TRAINING_HOLDOUT_DRIFT` |
| Weekly cell triage | `weekly.md` |
| Oracle flakiness | weekly export |
| Per-failure-mode-class precision/recall | weekly export |
| Compression progress (CPΦ) | `mejepa_compression_progress` + `CF_MEJEPA_TRAIN_CERTS` |
| Active learning queue | `mejepa_active_learning_queue` + `CF_MEJEPA_ACTIVE_LEARNING_QUEUE` + `CF_MEJEPA_CURIOSITY_TELEMETRY` |
| Signal drop log | `CF_MEJEPA_SIGNAL_DROP_LOG` |
| OOD harvest | `CF_MEJEPA_OOD_HARVEST` + `CF_MEJEPA_OOD_CALIBRATIONS` |
| OOD serving gate | strict `mejepa_verify` selected-calibrator consumption |
| Per-slot OOD reasons | `RealityPrediction.per_slot_ood_reasons` + `/zfs/archive/contextgraph/state/calibration/per-slot-v1/` |
| Mistake loop status | `mejepa_mistake_loop_status` + `CF_MEJEPA_MISTAKE_LOG` + `CF_MEJEPA_ONLINE_HEAD_STATE` |
| Daemon health endpoint | `health_probe.rs` |
| Multi-agent monitoring | `monitoring.rs` |

Telemetry emitted via `tracing`. JSON output via `tracing-subscriber` `json` feature; env filter via `env-filter`.

---

## 6. Hygiene

`crates/context-graph-mejepa-hygiene/src/`:

| Module | Responsibility | MCP surface |
|---|---|---|
| `categories.rs` | Categorize CF rows for retention | — |
| `gc.rs` | GC pass + report | `mejepa_gc_run` |
| `quota.rs` | Quota enforcement | `mejepa_quota_status` |
| `retention.rs` | Retention policies (TTL, importance, replay weight) | — |
| `tier.rs` + `tier_frozen.rs` + `tier_ops.rs` | Hot → warm → frozen transitions | — |
| `witness_compress.rs` | Witness-chain Merkle compression | `mejepa_witness_compress` |

---

## 7. Test suite inventory

### 7.1 Counts

- **Source `.rs` files**: 1,600 under `crates/*/src/`
- **Example files**: 210 under `crates/*/examples/` (mostly `*_fsv.rs` FSV harnesses; being retired per #655)
- **Test files**: 258 under `crates/*/tests/`

Current counts shift weekly; SoT is `cargo test --release`.

### 7.2 Test categories

| Category | Where | Notes |
|---|---|---|
| **Unit tests** | inline `#[cfg(test)] mod tests` + `tests.rs` modules | most coverage |
| **Integration tests** | `crates/*/tests/*.rs` | 258 files |
| **FSV harnesses** | `crates/*/examples/*_fsv.rs` | 210 files; being retired (#655). Manual SoT readback is the replacement. |
| **Adversarial** | `phase_f_adversarial_corpus_fsv.rs`, `adversarial_corpus.rs` (30 cases) | TASK-PY-G-060 / TASK-FP-009 |
| **Synthetic stress** | `synthetic_stress.rs` (30 known-answer) | TASK-PY-G-071 |
| **End-to-end stress** | `project_stress.rs` | TASK-PY-G-072 |

### 7.3 How to run

```bash
cargo test --release                              # full workspace
cargo test --release -p context-graph-mejepa     # scoped to crate
cargo run --release -p context-graph-mejepa --example phase_f_adversarial_corpus_fsv  # single FSV
cargo clippy -p <crate> --no-deps --tests --examples -- -D warnings
```

### 7.4 Key fixtures / utilities

- `context-graph-test-utils` — shared deterministic-by-shape, pseudo-random-by-content helpers for storage/serialization/shape tests only. NOT real embedder/corpus evidence; forbidden for ME-JEPA completion gates.
- `crates/context-graph-mejepa/src/fixtures.rs` — predictor test fixtures.
- `crates/context-graph-mejepa-corpus/src/tests.rs` — 81+ corpus unit tests including real `apply_mutation` on real Python.
- `crates/context-graph-mejepa-instruments/src/tests.rs` — 27+ instrument unit tests.

### 7.5 Code-simplifier discipline

Per CLAUDE.md §13 / doctrine §16:
- Preserve functionality.
- Files ≤ 500 lines, functions ≤ 30 lines, cyclomatic ≤ 10.
- Reduce nesting; avoid nested ternaries; prefer `match` / `if let` / explicit `if / else if / else`.
- Choose clarity over brevity.

`file-length-manager` agent splits files at 1500 lines (last-resort safety net).

---

## 8. Security / adversarial robustness

Universal protocol → doctrine §14.2. Project-specific:

| Defense | Source |
|---|---|
| Secret redaction (19 patterns, <1ms p99 on 10KB, fail-closed on non-UTF8) | `scripts/swebench_lite_ops/python_secret_redaction.py`; wrapper at `crates/context-graph-mejepa/src/secret_redaction.rs` |
| Cross-language label gating | `crates/context-graph-mejepa/src/cross_language_label_gating.rs` |
| Adversarial-embedder-injection mitigation | `crates/context-graph-mejepa/src/adversarial_embedder_injection.rs` |
| 30-case adversarial Python corpus | `adversarial_corpus.rs` + `CF_MEJEPA_ADVERSARIAL_CORPUS` |
| Negative-action ablation falsifier | `CF_MEJEPA_ABLATION_REPORTS` + weekly `ablation_report.md` |
| Witness chain (ed25519 + Merkle) | `crates/context-graph-witness/` |
| Reality-loop legacy tools fail closed | `crates/context-graph-mcp/src/deprecation.rs::RETIRED_CGREALITY_TOOLS` |
| Inner-LLM compile-error guard | `crates/context-graph-mcp/src/lib.rs:13` (`feature = "llm"` triggers `compile_error!`) |
| No HTTP from action paths | enforced behaviorally |
| No destructive git | enforced behaviorally |

---

## 9. Multi-agent isolation

| Mechanism | Source |
|---|---|
| Daemon validation | `daemon_validate.rs` |
| Multi-agent observability | `mejepa_daemon_status` + `monitoring.rs` |
| Per-session prediction binding | `(sessionId, prediction_id)` keys in `CF_MEJEPA_LIVE_PREDICTIONS` |
| Decoded session hex32 | `decode_session_hex32` in shift-subscriber models |
| `agent:*` GitHub labels | `agent:claude` / `agent:codex` |

---

## 10. API stability

ME-JEPA-Code is pre-production (no SLA, no backwards compatibility per CLAUDE.md §6.6). Schema changes → wipe and rebuild. Future: TBD post-ship-gate.

---

## 11. Manual full-system FSV gate (#633)

#633 manual full-system FSV + hardening gate is active P0 work. Each of the 17 child sub-tasks requires its own manual SoT readback per §1.9:

| # | Domain (Source of Truth) |
|---|---|
| #634 | aiwonder host, ZFS roots, GPU, Docker, MPS, env: `ssh croyse@aiwonder.mst.com`, `nvidia-smi`, `docker ps`, `zfs list`, `mount`, env files |
| #635 | aiwonder model cache + no-download embedder readiness: `/zfs/hot/contextgraph/models/`, `~/.cache/huggingface/hub/`, SHA-256 manifest pins |
| #636 | 300×8 corpus + AST chunks + forward cache + labels + skills: `/zfs/archive/contextgraph/corpus/`, `state/labels/`, jsonl line counts + sha256 |
| #637 | SWE-bench Docker oracle + weak-cell outcome truth: `swebench.harness.run_evaluation` end-to-end on real instance, per-instance `report.json` |
| #638 | ME-JEPA `RealityPrediction` Q1/Q2/Q3/Q5 product path: `CF_MEJEPA_LIVE_PREDICTIONS` rows, `CF_MEJEPA_CLAIM_RECONCILIATION`, `CF_MEJEPA_REALITY_IMPACT` |
| #639 | MCP product tools + retired surfaces + JSON-RPC state: `cargo run -p context-graph-mcp -- --mode stdio`, all 216 `mejepa_*` tools |
| #640 | Slot-preserving panels + frozen-instrument execution: `Panel` round-trip, instrument zero-gradient guard, `MEJEPA_INSTRUMENT_GRADIENT_LEAK` |
| #642 | RocksDB CF registry + data integrity + retention + restore: `ALL_HYGIENE_REFERENCED_CFS` canonical opener, `canonical_hygiene_cfs_match_group_union` |
| #643 | Trainer + mistake loop + replay + checkpoints + Fisher/EWC: `mejepa-train` end-to-end, `CF_MEJEPA_MISTAKE_LOG`, `CF_MEJEPA_ONLINE_HEAD_STATE` |
| #644 | CLI + hooks + shift log + path discipline + no mounted-drive: `context-graph-cli`, `scripts/cc/*.sh`, `runtime/shift-log.jsonl`, `check_no_c_drive_paths.sh` |
| #645 | Build + tests + clippy + CI + secrets + SBOM/provenance: `cargo build --release`, `cargo test`, `cargo clippy -- -D warnings`, `.github/workflows/` |
| #646 | Observability + evidence retention + backup + DR: `tracing`, `mejepa_daemon_status`, ZFS snapshots, off-host backup gap (#380) |
| #647 | Performance + scalability + capacity + cost: RTX 5090 wall-clock + VRAM + GPU util, per-shard timing, RocksDB write throughput |
| #648 | Security hardening for code + aiwonder ops: secret redaction, ed25519 witness chain, no HTTP from action paths, `feature = "llm"` compile_error |
| #649 | Documentation + GitHub issue ledger truth: `docs/futurebuild/*`, `CLAUDE.md`, `AGENTS.md`, every closed issue's RESOLVED comment vs current source |
| #650 | Benchmark + gate-metric honesty audit: `swebench_300x8_ship_gate_fsv` re-verified manually, #532 degenerate-cell contract |
| #651 | Operational practice + RCA discipline: doctrine §3 file rule, §3.5 milestone comments, §5 RCA five-whys, §11 forensic discipline |

Each child closes by RESOLVED-with-manual-readback or BLOCKED-with-named-blocker.

---

## 12. FSV protocols for the binary doctrine

Universal FSV-PROTOCOL §1.1–§1.5 applies. Additions below are protocol-specific assertions every FSV harness must enforce.

### 12.1 Failure-mode labeling FSV (#404, #413, #414 — shipped)

**Level 0 (shipped):**
1. `slot_identity_preserved=true` and `flat_vector_concat_used=false` recorded in FSV manifest.
2. Manual review reproduction.
3. **Coverage:** every chunk in 399,777-row corpus emits ≥1 Level-0 label.
4. **Leakage policy:** outcome labels marked `leakage_policy:target_side_only`; live predictor inputs do NOT include them.
5. **Usefulness gate:** 491 entered the accepted registry; rejected labels carry typed reasons.
6. **Independent readback:** every JSONL byte-replayed after clean re-read; sha256 recorded.

**Level 1/2/3 hierarchy (shipped):**
1. Hierarchy rows materialized: 8 Level-1, 157 Level-2, 379 Level-3, 3,169 Level-3 memberships, 536 named predictions.
2. Independent readback: writes all hierarchy rows, flushes, drops, reopens, decodes every target CF, asserts count equality.
3. Runtime resolver consumption: persisted Level-3 membership resolves through `AbilityContext`.
4. No collapsed panel: `flat_vector_concat_used=false` and `slot_identity_preserved=true`.
5. Leakage policy: `target_outcomes_used_for_calibration=true` while `target_labels_used_as_live_inputs=false`.
6. Holdout coverage: 114/114 holdout Fail rows resolve to a FailLikely named mode.
7. Closest-exemplar agreement ≥ 0.85 threshold.

### 12.2 Cross-panel triangulation FSV (#405, planned)

1. **Both panels on same instances.** Every gate-relevant prediction has rows in `CF_MEJEPA_LIVE_PREDICTIONS` AND `CF_MEJEPA_PANEL_B_OBSERVATIONS` AND `CF_MEJEPA_CROSS_PANEL_AGREEMENT`. Required: `unmatched_predictions == 0`.
2. **Independent Panel-B readback.** Flush, drop, reopen via production opener. Byte-equality with pre-flush row.
3. **Goodhart-detection edge.** Inject synthetic divergence: Panel A `corr ≥ 0.95` but Panel B `corr < 0.85` → `CROSS_PANEL_GOODHART_DETECTED` fires AND heal scheduler records `ship_gate_blocked_by_cross_panel`.
4. **Encoder non-overlap audit.** Panel A and B encoders use distinct model artifacts; SHA-256 manifest pins differ on every learned-encoder slot pair.
5. **VRAM budget audit.** Panel B encoders never resident during normal inference; only loaded during evaluation worker's pass.

### 12.3 Mistake-loop convergence FSV (#406 — shipped)

1. **Mistake landing.** Inject deliberate wrong prediction. `CF_MEJEPA_MISTAKE_LOG` contains new row with correct fields. Independent readback after flush/drop/reopen.
2. **Replay-buffer boost.** `CF_MEJEPA_MISTAKE_REPLAY_PRIORITY[mistake_id].boost_factor == 8.0` AND corresponding priority-boosted entry in `CF_MEJEPA_REPLAY_BUFFER`.
3. **Convergence on second presentation.** Present same `panel_signature_hash` after one trainer step → `Verdict` matches ground truth. Byte-replayable.
4. **Exact-panel unrelated-control isolation.** Unrelated control `panel_signature_hash` predicted before/after update; verdict unchanged; `unrelated_cell_accuracy_delta` within ±0.02.
5. **`mistake_repeat_rate` measurement.** Rolling rate across 100-mistake window. Required for ship: `≤ 0.05`. Byte-readable.
6. **#524 guard.** Reject reports claiming `claims_fisher_ewc_protection=true` without a durable Fisher/EWC row key.

### 12.4 Q4 freeze audit FSV (shipped one-time)

1. **No new Q4 producer writes after freeze.** Read `CF_MEJEPA_Q4_*` row counts at landing; persist as `q4_freeze_baseline.json`. Required: `q4_row_count_delta == 0` weekly.
2. **No Q4 contribution to predictor loss.** `Q4_LOSS_COEFFICIENT_MAX == 0.0`.
3. **No Q4 in ship-gate metric.** `q4_inputs_to_ship_gate == 0`.
4. **Read-only Q4 surface still works.** Pre-freeze predictions return historical Q4 fields; post-freeze predictions return empty Q4 fields.

### 12.5 Disjoint calibration partition (10th-percentile floor)

Every τ in `CF_MEJEPA_FAILURE_FINGERPRINTS`, `CF_MEJEPA_OOD_CALIBRATIONS`, `CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS` confidence cutoffs MUST be computed from a calibration partition disjoint from train and eval. Audit asserts:

- Recorded `calibration_partition_id` field.
- Instance-ID list does NOT overlap with train (`overlap_with_train == 0`).
- Instance-ID list does NOT overlap with eval (`overlap_with_eval == 0`).
- τ value matches 10th-percentile floor of authentic-sample cosines on calibration partition (tolerance ±1e-4).

Any failure → `THRESHOLD_CALIBRATION_NONDISJOINT` and ship eligibility blocked for that cell.

### 12.6 Information-physics FSV protocols (planned)

**#511 / TASK-IP-001 — entropy auxiliary loss:**
1. λ=0 → `L_step` byte-equal to baseline reference.
2. Monotonic Kraskov entropy decrease across N≥4 windows with λ>0.
3. Holdout Brier with λ>0 ≤ baseline + 0.005; else λ auto-zeroes.
4. Degenerate latent → `UTML_LATENT_ENTROPY_DEGENERATE` typed error.

**#512 / TASK-IP-002 — panel ECC:**
1. Clean-panel syndrome zero across holdout.
2. 1-slot and 2-slot corruption recoverable (100% within radius `t=2`).
3. 3+-slot corruption detected → `PANEL_ECC_UNCORRECTABLE` + mistake log row.
4. All #394 anti-compensation fixtures preserved.

**#514 / TASK-IP-005 — cold-load behavioural FSV:**
1. Every promotion writes `CF_MEJEPA_COLD_LOAD_SNAPSHOTS[promotion_id]`.
2. Cold-load correlation drift ≤ 0.02 vs snapshot.
3. Per-cell verdict distribution drift ≤ 5%.
4. Three independent cold-load runs all pass.
5. ECC syndrome consistency (after #512) — `syndrome_drift_per_panel == 0`.

### 12.7 Creativity-engine FSV protocols (post-0.95)

**#516 / TASK-CR-001 — Novelty + Utility oracle:**
1. E1-rotation candidate → REJECTED with `NOVELTY_BELOW_TAU`.
2. Random-Gaussian candidate → REJECTED with `UTILITY_BELOW_DELTA`.
3. Informative + orthogonal candidate → ACCEPTED.
4. Holdout-regression candidate → REJECTED with `HOLDOUT_REGRESSION`.

**#517 / TASK-CR-002 — Mistake-driven embedder demand:**
1. Mistake with clear slot winner → no demand row.
2. Mistake with all per-slot attribution below τ → demand row written.
3. End-to-end composition with #516 novelty+utility oracle.
4. Idempotent re-runs increment `recurrence_count`.

**#518 / TASK-CR-003 — Embedder retirement:**
1. Synthetic E1-rotation embedder → identified as dominated → retirement audit row.
2. Grace period: `archived_pending_grace_period` for K windows.
3. Informative non-redundant embedder → NOT identified.
4. Storage savings match prediction within 5%.
5. Operator override via `context-graph-cli mejepa retire-embedder` writes audit row with operator attribution.

**#519 / TASK-CR-004 — Domain-routed sparse panel:**
1. Per-domain routing: Python chunk fires universal + Python-extension only.
2. Cross-domain reclassification: filled_mask bits update correctly.
3. Storage 3-5× smaller than unified panel.
4. Per-cell oracle correlation does NOT regress vs unified baseline within ±0.005.

**#520 / TASK-CR-005 — Episodic-to-semantic consolidation:**
1. Settled chunk (near centroid + stable assignment ≥ stability_window) → consolidated.
2. Outlier chunk → preserved raw.
3. Lossless reconstruction: `cluster_centroid + residual` byte-equals original within 1e-6.
4. Predicted storage savings match measured within 5%.
5. Witness chain integrity: `before_witness_hash` + `after_witness_hash` readable; provenance verifiable.

### 12.8 Neuroscience-inspired reserve FSV protocols (DO NOT START until current queue concludes)

8 reserve issues (#538-#545). If activated:

**#538 Predictive-coding head FSV:**
1. Top-down per-slot prediction matches `InstrumentSlot::dim()` per slot.
2. Per-slot residual computed in same-embedder space (no cross-embedder dim compare; `MEJEPA_TCT_DIM_MISMATCH` on violation).
3. Independent RocksDB readback of `CF_MEJEPA_PER_SLOT_PREDICTIONS` after flush/drop/reopen.
4. Ablation: predictor-with-PC-head vs current late-fusion head; persist correlation delta against real Panel A baseline.
5. OOD-by-residual rank-orders OOD calibrator at Spearman ≥ 0.7.

**#539 Schema-prioritized replay FSV:**
1. ≥80% of each batch shares same Level-2 skill ID.
2. Reverse replay: N-leaf pathway produces N reverse-temporal update rows.
3. `synthetic=true` flag set on all `CF_MEJEPA_GENERATIVE_REPLAY_AUDIT` rows; fail if any row leaks into `CF_MEJEPA_LIVE_PREDICTIONS`.
4. `mistake_repeat_rate` across schema clusters drops relative to baseline replay.

**#540 Hierarchical predictive coding FSV:**
1. Per-level error scalars byte-replayable.
2. Predicted-but-not-fired skill triggers `SkillLifecycleAuditRow` decision.
3. Ablation: backward-prediction-hierarchy vs aggregator-only-hierarchy; persist correlation + Brier deltas.

**#541 Mixed-selectivity binding FSV:**
1. Slot identity preserved at binding inputs (same-embedder scalar summaries only).
2. Anti-compensation (#394) holds with binding layer active.
3. Per-basis-function attribution composable with #391 slot attribution.
4. Ablation: mixed-selectivity vs linear pairwise statistics (#390); persist deltas.

**#542 / #543 / #544 / #545:** design-influence on host-epic FSVs (#416 / #519 / #392 / #517 respectively); no separate top-level FSV harness.
