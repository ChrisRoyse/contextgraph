# 17 — Blind Spots and Missing Capabilities

> **Purpose.** What the previous docs (00-16) missed or under-specified. Each blind spot becomes either (a) a new spec + tasks (when the gap is structural), (b) a referenced clarification inside an existing spec (when the gap is detail), or (c) a deferred follow-up (when the gap is real but explicitly out of scope for the futurebuild plan). This document is the audit trail of "what else."

---

## Critical blind spots → new docs + specs

| # | Blind spot | Why it matters | Resolution |
|---|---|---|---|
| 1 | **Integration / end-to-end / chaos / load testing strategy** | Phase A through H ship FSV evidence per task, but no spec describes the cross-cut tests that prove the *whole system* works under realistic load and partial failure | NEW `18_INTEGRATION_AND_E2E_TESTING.md` + `SPEC-TESTING.md` + `TECH-TESTING.md` + tasks |
| 2 | **Operator runbook for daily / weekly / incident operations** | Daemon mode ships in Phase A but no doc tells the operator what to do every day, every week, when an alert fires | NEW `19_OPERATOR_RUNBOOK.md` + `SPEC-RUNBOOK.md` + `TECH-RUNBOOK.md` |
| 3 | **Disaster recovery + backup strategy for `/mnt/d/contextgraph/`** | All durable state lives on D-drive; what happens if D-drive fails or corrupts? Where are the backups? | NEW `20_DISASTER_RECOVERY_AND_BACKUP.md` + `SPEC-DR.md` + `TECH-DR.md` + tasks |
| 4 | **Cold start: first prediction on a fresh deploy with no training data** | Predictor starts with random weights or seeded weights? Conformal calibration with N=0? Constellation fallback? | NEW `21_COLD_START_AND_BOOTSTRAP.md` |
| 5 | **Adversarial robustness against agents that try to fool ME-JEPA into Pass** | Agent could craft patches that game the predictor; no spec addresses this | NEW `22_ADVERSARIAL_ROBUSTNESS.md` + `SPEC-SECURITY.md` extension |
| 6 | **Multi-agent / multi-tenant isolation** | Multiple agents using same daemon: does session_id isolate? rate limits per agent_id? cross-session leakage? | NEW `23_MULTI_AGENT_ISOLATION.md` |
| 7 | **API stability + versioning of MCP tools and `RealityPrediction` schema** | Schema is described as additive but no formal semver / deprecation policy | NEW `24_API_STABILITY.md` |
| 8 | **Future domains (ME-JEPA-Voice / -Image / -X) template** | Architecture is described as transferable but no doc specifies the template for adapting to a new modality | NEW `25_FUTURE_DOMAINS_MEJEPA_X.md` |
| 9 | **Documentation map (meta)** | 17+ docs in futurebuild + ruvectorfindings + memory; new contributors need a single map | NEW `26_DOCUMENTATION_MAP.md` |

## Detail-level gaps → clarifications in existing docs

| # | Gap | Where addressed |
|---|---|---|
| D1 | Schema migration path when CFs evolve | Already CLAUDE.md says "no backwards compat" but storage 4 needs explicit guidance for the moment we DO need it (post-1.0) | `24_API_STABILITY.md §5` |
| D2 | Per-language toolchain availability detection (agent's environment may lack `cargo`, `tsc`, `dotnet`) | `SPEC-PREDICT.md` REQ-PREDICT-LABEL-* depend on toolchains; need fallback policy | `18 §6` + new task `TASK-PREDICT-LABEL-010-toolchain-fallback` |
| D3 | Privacy / secret redaction for code being embedded | Not addressed; sensitive material in patches | `22 §4` + new `TASK-PREDICT-010-secret-redaction-pre-embed` |
| D4 | Drift attribution: when drift fires, which root cause (data shift / model stale / spec change)? | `09_SELF_HEALING_AUTONOMY.md` mentions but doesn't specify | new `TASK-HEAL-008-drift-attribution` |
| D5 | Cross-version reproducibility: predictor v0.4 → v0.5 should reproduce predictions | mentioned via `provenance` field; needs replay tool | new `TASK-OBS-011-prediction-replay-tool` |
| D6 | Reasoning-trace visualization for operators | `SPEC-OBSERVABILITY.md` REQ-OBS-FB-06 mentions inspect; needs HTML/markdown render | new `TASK-OBS-012-reasoning-trace-renderer` |
| D7 | Cost / token usage telemetry for the system itself (CUDA cycles, agent token cost of feedback events) | not tracked | new `TASK-OBS-013-system-cost-telemetry` |
| D8 | Plugin architecture for new prediction heads / new label sources | hardcoded today; future-friendly | `25_FUTURE_DOMAINS_MEJEPA_X.md §2` |
| D9 | Graceful degradation when one head fails | not specified — assume crash propagates | new `TASK-PREDICT-011-per-head-graceful-degradation` |
| D10 | Cross-language label noise (Python failure mode applied to Rust via patch-similarity) | conceptual issue; needs gating | `22 §6` + `TASK-PREDICT-LABEL-011-cross-language-label-gating` |
| D11 | Concurrent agent edits to the same file (race conditions in shift log) | shift log is per-session JSONL, but external mtime conflicts across sessions | `23 §3` |
| D12 | The 3-attempt-park rule needs a formal data structure | `06_REALITY_LOOP_REFACTOR.md §6` mentions; needs CF + tool | new `TASK-FLYWHEEL-007-park-list-cf` |
| D13 | Operator can override / fork ME-JEPA's prediction inline (training signal + dataset addition) | implied by active-learning queue; not exposed as direct tool | new `TASK-PREDICT-012-operator-override-prediction-tool` |
| D14 | Cross-cutting integration tests (provenance integrity, watermark restart-safety, scheduler+drift+heal end-to-end) | only per-task FSV exists | covered in `18_INTEGRATION_AND_E2E_TESTING.md` |
| D15 | Health & readiness endpoints (HTTP probe, not just MCP) | for k8s/systemd health checks | new `TASK-OBS-014-health-readiness-endpoint` |
| D16 | Log aggregation and rotation policy | `logs/` mentioned but no rotation rules | new `TASK-OBS-015-log-rotation-policy` |
| D17 | Per-(category × language) ship-gate exemption (some cells may be impossible) | impossible cells should be marked exempt, not blocking ship gate | new `TASK-EVAL-001-cell-exemption-policy` |
| D18 | Prediction explanation for non-technical operators (executive summary) | rich `RealityPrediction` is verbose; need executive view | new `TASK-OBS-016-executive-summary-renderer` |
| D19 | Data retention policy beyond GC (legal / compliance hold) | none currently | new `TASK-HYG-001-data-retention-policy` |
| D20 | Cross-machine / multi-node deployment (operator runs daemon on different hardware) | implicitly single-machine; document the boundary | `23 §6` |

## Out-of-scope for the futurebuild plan (operator-decided)

| # | Item | Why out of scope now |
|---|---|---|
| O1 | Web UI dashboard | the CLI + weekly markdown reports cover operator needs; web is opt-in later |
| O2 | Multi-region / cloud deployment | local single-machine is the design point; cloud is future work |
| O3 | Federated training across operators | requires multi-org consent; deferred |
| O4 | ME-JEPA-Voice / -Image / etc. concrete implementation | template documented in `25`; concrete builds wait for operator priority |
| O5 | Training the underlying embedders (E1, E7, E14, etc.) | embedders are frozen by design (constitution); fine-tuning is an explicit future-domain choice |
| O6 | LLM-augmented label generation | per `2.5 No-inner-LLM rule`; only deterministic / analyzer-based labels for now |
| O7 | Real-time UI for operator approval (web/Slack integration) | CLI + weekly report sufficient for v1 |

---

## How this audit closes

After applying this audit:

- The futurebuild doc count grows from 17 (`00–16`) to **27** (`00–26`).
- Functional specs grow from 6 to **10** (added: `SPEC-TESTING`, `SPEC-RUNBOOK`, `SPEC-DR`, `SPEC-SECURITY`).
- Technical specs grow from 6 to **10** (matching).
- Atomic tasks grow from 70 to **~110**.
- Phases stay at 8 (A-H), but Phase H absorbs the cleanup tasks AND the security/runbook/DR/testing tasks land across phases as new entries in `13_TASK_INDEX.md` (NB: `13` is updated separately in `TASK-INDEX-PATCH-001`).

Every blind spot listed above is now either fixed (new doc/spec/task), or explicitly accepted (out of scope, with rationale). No gaps left silent.
