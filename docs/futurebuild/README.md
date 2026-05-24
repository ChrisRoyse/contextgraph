# futurebuild — ME-JEPA-Code Reference

Source-grounded reference for AI agents working on ContextGraph / ME-JEPA-Code. 8 docs + protocol.

- **Source code:** `/home/cabdru/contextgraph/crates/` (22 active workspace crates)
- **Authority:** `docs2/AICodingAgentSuperPrompt.md` (universal doctrine) > `CLAUDE.md` (project addendum) > this suite

---

## One-line system summary

ME-JEPA-Code is a **binary reality predictor**: for any AI-generated code change it returns `Pass | Fail` against the Docker oracle, with a named failure mode + closest exemplars on Fail (Q3), and claim-exists (Q1) and shift-event (Q5) binary predictions as additional grounded surfaces. A planned butterfly-effect pathway tree (#416) surfaces top-K (3–5) most-probable downstream futures per candidate patch as chains of Q1/Q2/Q5 binary leaves.

---

## Read order

| # | Doc | When |
|---|---|---|
| 1 | [`01_overview.md`](01_overview.md) | First time. What it is, tech stack, entry points, subsystem summary, ambiguity boundary. |
| 2 | [`02_source_map.md`](02_source_map.md) | Crate/file ownership. Dependency graph. Binary targets. |
| 3 | [`03_storage_schema.md`](03_storage_schema.md) | Touching durable state. ~200 RocksDB CFs. aiwonder storage policy. |
| 4 | [`04_mcp_tools.md`](04_mcp_tools.md) | Calling/implementing an MCP tool. 216 tools (69 `mejepa_*` product surface). |
| 5 | [`05_predictor_pipeline.md`](05_predictor_pipeline.md) | Predictor work: embedders, instruments, panels, heads, TCT, corpus, training, heal, mistake loop. |
| 6 | [`06_operations_and_fsv.md`](06_operations_and_fsv.md) | Operating: FSV protocol, runbook, DR, observability, test suite. |
| 7 | [`07_results_and_benchmarks.md`](07_results_and_benchmarks.md) | Current measurement, shipped milestones, FSV evidence catalog. |
| 8 | [`08_roadmap_and_open_work.md`](08_roadmap_and_open_work.md) | Open issues, critical path, post-ship plans. |

Plus: [`specs/FSV-PROTOCOL.md`](specs/FSV-PROTOCOL.md) — mandatory verification contract. [`_archived/`](_archived/) — read-only history.

---

## Ship gate

```
prediction_oracle_correlation ≥ 0.95
  stable × 4 consecutive rolling windows
  per mathematically-valid (mutation_category × language) mixed cell
  on 300×8 SWE-bench Lite Python corpus
  using Q2 Docker-oracle agreement as the binary verdict
  AND cross-panel oracle correlation ≥ 0.95 on Panel B (#405)
  AND |corr_panel_a − corr_panel_b| ≤ 0.05
```

**Current measurement:** raw Pearson `0.4843192995` (latest ship-gate FSV `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/`). Honest gap to 0.95 = `0.4657`.

**Degenerate-cell rule (#532):** Pearson used only when oracle target has both Pass and Fail rows. Single-class cells use target-class recall ≥ 0.95, actual-class confidence ≥ 0.95, Brier ≤ 0.05, ECE ≤ 0.05 with power diagnostics.

Once met → human review of AI-generated Python patches becomes statistically unjustified.

---

## Critical path (#712 — pinned authoritative ordering)

| Rank | Issue | Action |
|---|---|---|
| 1 | **#683** ✅ closed 2026-05-23 | Unified real training/checkpoint export with public trainer contract; production `trained_checkpoint_manifest_path` fail-closed |
| 2 | **#709 + #715** | Inter-embedder distinctness gate; rebuild v2 static AST cache with 9 active slots (`e1,e6,e7,e8,e9,e10,e12,e13,e14`); `e2/e3/e4` retired for static AST by #704/#714 |
| 3 | **#615** | delete_test_call Docker oracle batch over 1,131 deterministic candidates → rebuild AST → labels → #392 |
| 4 | **#618 + #619** | Per-slot OOD aggregation; slot-preserving panel projections (doctrine §1.5ter repair) |
| 5 | **#695** | Replace rank-lift Goodhart metric with raw Pearson + isotonic/Platt |
| 6 | **#620/#621/#622/#684/#685/#686/#687/#688/#689/#690/#692/#696/#699/#705/#706** | Eliminate hardcoded/no-op telemetry, pseudo-cert confidence, dead public training surfaces |
| 7 | **#633 + #655 + #656 + #657 (and children)** | Manual full-system FSV/hardening gate, auto-FSV retirement, heal-drill truthfulness, geometry-label bakeoff |
| **8.5** | **#744 + #745 + #746 (causal-embedder track)** | **Post-substrate, parallel.** Auto-labeler extension to causal-pair corpora + in-slot causal embedder replacement (E5/E11/E14 → C_TemporalCommit / C_CommonsenseCausal / C_BugFix) + low-signal embedder audit. **Aiwonder-only execution.** |
| 8 | **#392 / #511 / #405 / #379** | Trajectory ECE / entropy aux loss / Panel B / multi-file holdout — **blocked behind ranks 1–7** |

Post-0.95 work (pathway exploration #416, skill linkage #417, info-physics #511/#512/#514, creativity engine #516–#520, neuroscience reserve #538–#545) stays parked until the gate closes.

---

## Blind-spot audit (2026-05-23) — 17 new issues filed

Parallel epistemic audit identified 14 unfiled blind spots + 3 causal-embedder follow-ups. Aggregator: **#739**. Key entries:

| # | Title | Rank by audit |
|---|---|---|
| **#734** | Panel sufficiency MI proof — most existential | #1 |
| **#723** | Oracle noise floor re-measurement | #2 |
| **#724** | Train/serve distribution mismatch | #3 |
| **#725** | Iterative-refinement predictor head | #4 |
| **#733** | Incremental forward cache | #5 |
| **#744 / #745 / #746** | Causal-embedder track (CR-006/007/008) | parallel post-substrate |

Two cheapest highest-leverage probes: **#734** panel-sufficiency MI audit (3-5 days) and **#723** oracle noise-floor re-measurement (~2 hours aiwonder Docker). If either fails, the #712 critical path framing is invalidated.

---

## Active `type:context` pins (read at session start)

| # | Title |
|---|---|
| **#712** | Sherlock-validated critical path — authoritative ordering |
| **#739** | BLINDSPOT-AUDIT-2026-05-23 — 14 unfiled blind spots + 3 causal-embedder follow-ups |
| **#633** | Manual full-system FSV + hardening gate (parent of #634–#651) |
| **#310** | META Issue refactor — Phase G execution-order map |
| **#383** | Gate-active issue order must be evidence-weighted |
| **#397** | ME-JEPA consequence-prediction truth-system contract |
| **#227** | Python ME-JEPA data explosion runtime policy |

---

## Aiwonder-only execution (project doctrine)

Per `CLAUDE.md §6.7` + closed #372: every action that touches durable state MUST execute on the **aiwonder datacenter PC**.

| Surface | aiwonder location |
|---|---|
| Production corpus / FSV / runtime / state | `/zfs/archive/contextgraph/...` (ZFS archive pool) |
| Hot model cache / repo / scheduler | `/zfs/hot/contextgraph/...` (ZFS hotpool) |
| GPU compute | RTX 5090 (CUDA 13.2, candle 0.10.2-cuda) |
| Docker oracle invocation | aiwonder venv per commit `bf73c61f` |
| Operator entry | `ssh croyse@aiwonder.mst.com`; `contextgraph-shell` |

**Forbidden:** `/mnt/d`, `/mnt/c`, any WSL mount; local-host RocksDB for durable read/write; non-aiwonder Docker harness; off-aiwonder `cargo run` touching durable state. CI lint `scripts/ci/check_no_c_drive_paths.sh` enforces.

Manual SoT readback per `06 §1.9` is binding for closeout (auto-FSV `_fsv.rs` examples retired per #655).
