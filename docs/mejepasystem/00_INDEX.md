# ME-JEPA System Documentation — Master Index

> **Purpose.** Authoritative technical reference for the **ME-JEPA-Code Reality Truth System** as it exists in the ContextGraph repository today AND as the futurebuild plan specifies it when complete. Every section is sourced from the live tree (`crates/`, `docs/futurebuild/`, `docs/ruvectorfindings/`, `.claude/`, `memory/`) — no aspirations beyond what those documents commit us to.
>
> **Audience.** Engineers reading the codebase cold, operators running the daemon, researchers comparing the system to JEPA/SSL literature, future contributors picking up a TASK-* file.
>
> **Authority order** (when docs conflict): `docs/ruvectorfindings/specs/constitution.md` → `docs/futurebuild/00_INDEX.md` → `CLAUDE.md §11` → these docs. These docs **describe**; they do not **decide**.

---

## 1. The single sentence

ME-JEPA-Code is a Joint Embedding Predictive Architecture that, given (code change + context), outputs a calibrated, multi-head prediction of what the deterministic reality oracle (Docker test runner) would say if the change were actually executed — and answers four additional questions (existence, edge cases / latent bugs / tech debt / perf / security / accuracy / cost concerns, reality impact) about every change, every time.

---

## 2. Reading order

| # | Document | Use when |
|---|---|---|
| 01 | [`01_system_overview.md`](01_system_overview.md) | First contact; lay of the land |
| 02 | [`02_source_code_map.md`](02_source_code_map.md) | Locating a crate / file / module |
| 03 | [`03_configuration.md`](03_configuration.md) | Tuning a knob or env var |
| 04 | [`04_database_schema.md`](04_database_schema.md) | Adding a CF or auditing storage |
| 05 | [`05_ast_chunker_subsystem.md`](05_ast_chunker_subsystem.md) | Multi-language chunking |
| 06 | [`06_embedder_substrate.md`](06_embedder_substrate.md) | Working on E1–E21 / routing |
| 07 | [`07_instruments_panel.md`](07_instruments_panel.md) | The 15 instrument slots + Panel(5120) |
| 08 | [`08_dda_tct_enrichment.md`](08_dda_tct_enrichment.md) | DDA signals + TCT Gτ guard + constellation |
| 09 | [`09_predictor_multi_head.md`](09_predictor_multi_head.md) | Predictor trunk + 10 heads |
| 10 | [`10_training_loop_utml.md`](10_training_loop_utml.md) | UTML formula + training certs + sampler |
| 11 | [`11_inference_verify_lifecycle.md`](11_inference_verify_lifecycle.md) | `verify()`, conformal, OOD, verdicts |
| 12 | [`12_self_healing_scheduler.md`](12_self_healing_scheduler.md) | Drift / EWC++ / A/B/C / continual backprop |
| 13 | [`13_data_flywheel.md`](13_data_flywheel.md) | Shift subscriber + reward channels |
| 14 | [`14_corpus_phase0.md`](14_corpus_phase0.md) | 300×8 SWE-bench mutation corpus |
| 15 | [`15_witness_chain.md`](15_witness_chain.md) | SHAKE-256 chain + ed25519 + Merkle |
| 16 | [`16_hygiene_tier_compression.md`](16_hygiene_tier_compression.md) | Tier 8/7/5/3-bit + 115 GB quota + GC |
| 17 | [`17_prediction_taxonomy_reference.md`](17_prediction_taxonomy_reference.md) | Q1–Q5 + every head + every class |
| 18 | [`18_mcp_tool_reference.md`](18_mcp_tool_reference.md) | Every `mcp__cgreality__*` tool |
| 19 | [`19_cli_reference.md`](19_cli_reference.md) | `context-graph-cli` commands |
| 20 | [`20_hooks_layer.md`](20_hooks_layer.md) | Claude Code hook integration |
| 21 | [`21_loop_contract.md`](21_loop_contract.md) | The single outer-loop spec |
| 22 | [`22_operator_runbook.md`](22_operator_runbook.md) | Daily / weekly / monthly ops |
| 23 | [`23_disaster_recovery.md`](23_disaster_recovery.md) | Backup, restore, DR drill |
| 24 | [`24_security_adversarial.md`](24_security_adversarial.md) | Secret redaction + adversarial robustness |
| 25 | [`25_multi_agent_isolation.md`](25_multi_agent_isolation.md) | Multi-tenant / multi-session isolation |
| 26 | [`26_testing_strategy.md`](26_testing_strategy.md) | Unit + integration + FSV + chaos + load |
| 27 | [`27_api_stability_versioning.md`](27_api_stability_versioning.md) | MCP / schema semver + replay reproducibility |
| 28 | [`28_future_domains_mejepa_x.md`](28_future_domains_mejepa_x.md) | ME-JEPA-Voice / -Image / -X template |
| 29 | [`29_verification_report.md`](29_verification_report.md) | Codebase health snapshot — counts, metrics, version |

---

## 3. How current state vs target state is marked

Each doc uses these tags in section headers and tables:

| Tag | Meaning |
|---|---|
| **shipped** | Code + FSV evidence land in the live tree at HEAD; covered by `cargo test` |
| **partial** | Scaffolding exists; deep integration or production data is pending |
| **futurebuild** | Specified in `docs/futurebuild/`; not yet in code |
| **deferred** | Acknowledged but explicitly out of scope for the futurebuild plan |
| **retired** | Removed from active operation; fails closed with `MEJEPA_*_RETIRED` or `CCREALITY_*_RETIRED` |

These map 1:1 to the status taxonomy in `docs/futurebuild/02_CURRENT_STATE_HONEST.md` §§1–4.

---

## 4. What this doc set is NOT

- **Not the constitution.** Immutable rules live in `docs/ruvectorfindings/specs/constitution.md`. These docs cite it; they do not amend it.
- **Not the implementation plan.** Sequencing + effort estimates live in `docs/futurebuild/12_IMPLEMENTATION_PHASES.md` and `13_TASK_INDEX.md`.
- **Not the UTML derivation.** Theory lives in `docs2/TheoryOfMachineLearning.md` and `docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md`. We consume it, not re-prove it.
- **Not the SMAP shared mind.** Team narrative lives in `./memory/`. CLAUDE.md §6 governs that surface.
- **Not the operator's role spec.** `CLAUDE.md §11` is canonical for the per-turn outer-loop contract.

---

## 5. Glossary (one-line definitions)

| Term | Definition |
|---|---|
| **ME-JEPA-Code** | The predictive truth system being built. JEPA-class with **frozen externally-defined target instruments** (structural collapse immunity). |
| **Panel** | A 5,120-d fixed-shape vector assembled from 15 instrument slots (per chunk × per shift). Persisted to `CF_MEJEPA_PANELS`. |
| **Instrument** | A frozen, deterministic function from a typed event → a fixed-dim feature subvector. 15 of them, summing to 5,120-d. |
| **DDA** | Diagonal-Dominance-Augmentation: every input × N frozen embedders → n·(N + N²) structured signals (cosines + pairwise cosines + cached MI). |
| **TCT** | Teleological Constellation Training: per-(category × entity × language × embedder) centroids + per-embedder threshold τ_m → deterministic geometric guard. |
| **Gτ guard** | The TCT runtime test: `cos(embedder_m(predicted_panel), centroid_m) ≥ τ_m` for every embedder. Failure → `Verdict::GuardRejected`. |
| **Shift** | One (file, before_sha, after_sha, line_diff, cargo_check_result) tuple. Captured by hooks; appended to `cgreality-shift-log/<session>.jsonl`. |
| **RealityPrediction** | The unified output of `mejepa_verify` / `mejepa_predict_latest`. Carries verdict + Q1–Q5 surfaces + provenance. |
| **Verdict** | `Pass` / `Fail` / `OutOfDistribution` / `Abstain` / `GuardRejected`. |
| **SurpriseEvent** | An agent-side feedback record bound to a `prediction_id`. `Confirmed` / `Surprise` / `Omission` / `Calibration`. |
| **UTML** | The four-factor learning formula `L_step = ΔP × ΔK × ΔΩ × ΔΞ`. All factors clamped to [0, 1]; multiplicative gating. |
| **Witness chain** | Append-only SHAKE-256 73-byte chain + ed25519 segment signing + Stage 0/F/Merkle certificates. The audit substrate. |
| **SMAP** | Shared Mind Agent Protocol. `./memory/` 8-namespace tree (`context/decisions/discoveries/handoff/journal/blockers/patterns/reports/`). |
| **FSV** | Forensic Source Verification. The per-task evidence protocol: read source-of-truth back, ≥3 boundary cases, no mocks. Defined by `docs/futurebuild/specs/FSV-PROTOCOL.md`. |
| **Reality-shift principle** | "No alteration without current state. No alteration without a delta back into perception." Hooks enforce. |
| **GENERIC-ONLY** | No per-task or per-language hard-coding in the trained model. Mental check: "would this primitive be load-bearing for every SWE-bench task in every language?" |
| **D-drive policy** | All durable state lives under `/mnt/d/contextgraph/`. Nothing growing on C:\. Locked decision. |
| **Daemon mode** | `cargo run --release -p context-graph-mcp -- --daemon` — long-running supervised process hosting the shift subscriber, self-optimization scheduler, nightly GC, and weekly eval as tokio tasks. |

---

## 6. Source-of-truth pointers

For every domain claim in this doc set, the canonical reference:

| Domain | Source of truth |
|---|---|
| Vision + thesis | `docs/futurebuild/01_VISION_TRUTH_SYSTEM.md` |
| Honest current state | `docs/futurebuild/02_CURRENT_STATE_HONEST.md` |
| Reference architecture | `docs/futurebuild/03_ARCHITECTURE_BLUEPRINT.md` |
| Multi-embedder / DDA | `docs/futurebuild/04_DDA_TCT_CODE_ENRICHMENT.md` + `docs/ruvectorfindings/13_*.md` |
| Embedder roster | `docs/futurebuild/05_EMBEDDER_SIGNAL_MAP.md` + `docs/ruvectorfindings/12_*.md` |
| Loop refactor | `docs/futurebuild/06_REALITY_LOOP_REFACTOR.md` + `14_NEW_LOOP_MD.md` |
| Flywheel orchestration | `docs/futurebuild/07_TRAINING_DATA_FLYWHEEL.md` |
| Agent MCP API | `docs/futurebuild/08_LIVE_PREDICTION_AGENT_API.md` |
| Self-healing | `docs/futurebuild/09_SELF_HEALING_AUTONOMY.md` |
| Multi-head UTML | `docs/futurebuild/10_LEARNING_FORMULA_MEJEPA_TRUTH.md` + `docs/ruvectorfindings/11_*.md` |
| Deletion list | `docs/futurebuild/11_DELETION_LIST_INNER_LOOP.md` |
| Phase plan | `docs/futurebuild/12_IMPLEMENTATION_PHASES.md` |
| Task DAG | `docs/futurebuild/13_TASK_INDEX.md` |
| Loop literal text | `docs/futurebuild/14_NEW_LOOP_MD.md` |
| Prediction taxonomy | `docs/futurebuild/15_PREDICTION_TAXONOMY.md` |
| Storage policy | `docs/futurebuild/16_STORAGE_LOCATION_POLICY.md` |
| Blind spots audit | `docs/futurebuild/17_BLIND_SPOTS_AND_MISSING_CAPABILITIES.md` |
| Integration / chaos testing | `docs/futurebuild/18_INTEGRATION_AND_E2E_TESTING.md` |
| Operator runbook | `docs/futurebuild/19_OPERATOR_RUNBOOK.md` |
| Disaster recovery | `docs/futurebuild/20_DISASTER_RECOVERY_AND_BACKUP.md` |
| Cold start | `docs/futurebuild/21_COLD_START_AND_BOOTSTRAP.md` |
| Adversarial robustness | `docs/futurebuild/22_ADVERSARIAL_ROBUSTNESS.md` |
| Multi-agent isolation | `docs/futurebuild/23_MULTI_AGENT_ISOLATION.md` |
| API stability | `docs/futurebuild/24_API_STABILITY.md` |
| Future domains | `docs/futurebuild/25_FUTURE_DOMAINS_MEJEPA_X.md` |
| Constitution (Level 1) | `docs/ruvectorfindings/specs/constitution.md` |
| FSV protocol | `docs/futurebuild/specs/FSV-PROTOCOL.md` |
| Operator role | `CLAUDE.md §11` |
| Per-tick contract | `.claude/loop.md` (replaced by `docs/futurebuild/14_NEW_LOOP_MD.md`) |
| SMAP protocol | `docs2/SHARED-MIND-AGENT-PROTOCOL.md` |
| UTML theory | `docs2/TheoryOfMachineLearning.md` |

---

## 7. The 8 mutation categories (Phase 0 corpus)

For cross-reference: every category × language × entity-type × embedder combination forms a constellation cell (≈ 13,728 cells at full scale). Source: `crates/context-graph-mejepa-corpus/src/mutation/*` and `docs/futurebuild/04_DDA_TCT_CODE_ENRICHMENT.md §4`.

| # | Category | Purpose | Operator behavior |
|---|---|---|---|
| 1 | `KnownGood` | Baseline positive class | Echoes the upstream gold patch verbatim |
| 2 | `SubtleFlip` | Logic-flip class | Swaps `==`↔`!=`, `<`↔`>=`, `&&`↔`\|\|` per language |
| 3 | `OffByOne` | Boundary class | Perturbs integer literals on integer-typed sites |
| 4 | `SwapVariable` | Variable confusion | Renames a local variable; introduces use of wrong name |
| 5 | `DeleteTestCall` | Test deletion | Removes one `assert`/`assert_eq`/`expect` call |
| 6 | `WrongFile` | Cross-file misdirection | Applies the gold patch to an adjacent file |
| 7 | `OverEngineer` | Useless complexity | Adds an unused helper function/method |
| 8 | `CompileError` | Hard failure | Introduces a parse error |

---

## 8. The 5 verdict classes

Source: `crates/context-graph-mejepa/src/types.rs` (via `docs/futurebuild/15_PREDICTION_TAXONOMY.md §2`).

| Verdict | Trigger | Remediation hint |
|---|---|---|
| `Pass` | conformal lower bound ≥ ship-gate AND no high-severity Q4 AND every claim reconciles | trust the agent's done signal |
| `Fail` | ≥1 predicted_failed_test at confidence ≥ failure threshold | agent must address predicted failure path |
| `OutOfDistribution` | `ood_score ≥ ood_threshold` OR conformal interval width > `width_threshold` | re-attempt with smaller scope; flag to active-learning queue |
| `Abstain` | conformal interval too wide to commit | request more context; do not block but do not pass |
| `GuardRejected` | any embedder's cosine to its constellation centroid < `τ_m` | predicted panel lies outside the manifold of acceptable predictions |

---

## 9. The 13 active embedders (code-domain)

Source: `crates/context-graph-mejepa-embedders/src/routing.rs` (`docs/futurebuild/05_EMBEDDER_SIGNAL_MAP.md §1`).

E1, E2, E3, E4, E6, E7, E8, E9, E10, E12, E13, E14, E17 = 13 active. E5 (causal asymmetric) and E11 (KEPLER entity) are **retired** post-2026-05-09 and fail closed with `MEJEPA_EMBEDDER_RETIRED`. E15, E16, E19, E21 are non-code learner-state slots reserved for ME-JEPA-X domains. E18 and E20 carry partial signal at low weight.

---

## 10. The 15 instrument slots (Panel composition)

Source: `crates/context-graph-mejepa-instruments/src/slot.rs` (`docs/futurebuild/03_ARCHITECTURE_BLUEPRINT.md §3`).

| # | Slot | Dim | Built on | Status |
|---|---|---|---|---|
| 1 | `E_AST` | 384 | Qodo + AST-token transformer | partial (deterministic shipped) |
| 2 | `E_CFG` | 256 | E8 graph-attention over basic blocks | partial |
| 3 | `E_DataFlow` | 256 | E11 graph-attention over def-use chains | partial |
| 4 | `E_TypeGraph` | 256 | E11 + analyzer lattice (Pyright/mypy/rust-analyzer/...) | partial |
| 5 | `E_Test` | 384 | E1 on test-fn descriptions | partial |
| 6 | `E_Trace` | 512 | E2/E3/E4 sequence transformer over runtime traces | partial |
| 7 | `E_Diff` | 256 | Qodo diff-edge encoder | partial |
| 8 | `E_Witness` | 256 | Sequence transformer over 73-byte chain entries | **shipped** |
| 9 | `E_Oracle` | 128 | Test outcome aggregator (pass/fail/skip/error fractions + 10-class exception one-hot) | **shipped** |
| 10 | `E_Problem` | 1024 | E1 on SWE-bench problem text | partial |
| 11 | `E_CommitMsg` | 384 | E1 on commit text | partial |
| 12 | `E_StaticAnalysis` | 256 | Scalar aggregates over clippy/pylint/eslint/bandit/cargo-audit | **shipped** |
| 13 | `E_Runtime` | 256 | Scalar walltime/RSS/coverage Δ | partial |
| 14 | `E_Reasoning` | 384 | E17 AffectText on agent CoT | partial |
| 15 | `Scalars` | 128 | BFS depth, blame age, churn, complexity, coverage Δ, repo-health | partial |
| **Total** | | **5,120** | | |

---

## 11. The 10 prediction heads (multi-head predictor)

Source: `docs/futurebuild/03_ARCHITECTURE_BLUEPRINT.md §3` + `15_PREDICTION_TAXONOMY.md §3–4`.

| # | Head | Output | Status |
|---|---|---|---|
| 1 | Panel | predicted_panel[t=2] in 5,120-d latent | **shipped** |
| 2 | Oracle | per-test pass/fail logits + count | **shipped** |
| 3 | Failure-mode | logits over `FailureModeClass` per chunk | **futurebuild** Phase B |
| 4 | Edge-case | logits over `EdgeCaseClass` per chunk × `not_covered_by_tests` boolean | **futurebuild** Phase B |
| 5 | Tech-debt | logits over `DebtClass` (smell/complexity/dead/redundant) per chunk | **futurebuild** Phase B |
| 6 | Perf | predicted (cpu_us_delta, rss_kb_delta, alloc_count_delta) per chunk | **futurebuild** Phase B |
| 7 | Security | logits over `SecurityConcernClass` per chunk | **futurebuild** Phase B |
| 8 | Accuracy | (accuracy_metric_delta, calibration_delta) for ML code | **futurebuild** Phase B |
| 9 | Cost | (CI_minutes_delta, token_cost_delta) | **futurebuild** Phase B |
| 10 | Reasoning | logits over `ReasoningClass` for the agent's prose | **futurebuild** Phase B |

---

## 12. The 19 live ME-JEPA MCP tools

Per `docs/ruvectorfindings/specs/_README.md`. Full enumeration in [`18_mcp_tool_reference.md`](18_mcp_tool_reference.md).

`mejepa_verify`, `mejepa_predict_latest`, `mejepa_constellation_inspect`, `mejepa_heal_status`, `mejepa_rollback_to`, `mejepa_load_learner_state`, `mejepa_routing_lookup`, `mejepa_vram_budget_report`, `mejepa_gc_run`, `mejepa_quota_status`, `mejepa_witness_compress`, `mejepa_observe_shift`, `mejepa_subscriber_status`, `mejepa_capture_audit`, `mejepa_eval_run`, `mejepa_ship_gate_status`, `mejepa_active_learning_queue`, `mejepa_eval_build_graph`, `mejepa_audit_pairwise_mi`.

Plus the broader `mcp__cgreality__*` family (reality-shift readback, optimizer recommendations, witness-chain verify, etc.) → [`18_mcp_tool_reference.md`](18_mcp_tool_reference.md).

---

## 13. The 8 implementation phases

Source: `docs/futurebuild/12_IMPLEMENTATION_PHASES.md`. Effort estimates assume single engineer.

| Phase | Goal | Effort | Status |
|---|---|---|---|
| A — Operational Unblock | Daemon + prediction-injection + agent-feedback + D-drive enforcement | 5–7d | partial (code/FSV shipped; fresh-host install proof pending) |
| B — Multi-Head Extension | 8 new prediction heads + label sources + per-head deltas | 10–14d | futurebuild |
| C — Corpus Completion | 300×8 SWE-bench Lite + 10 per-language operators | 7–10d | partial (Python 1,200/2,400 rows; non-Python pending) |
| D — Neural Embedder Forwards | Replace deterministic placeholders with real Candle CUDA forwards | 10–15d | partial (wrappers exist for E1/E6/E7/E8/E10/E12/E13/E14) |
| E — Continuous Self-Optimization | Scheduler policy + per-cell drift + dissipation + λ ramp | 5–7d | partial (skeleton + 7 tickers shipped; full autonomous policy pending) |
| F — Production Observability | Weekly eval cron + MI heatmap + WSL GPU telemetry + dashboard | 5–7d | futurebuild |
| G — Ship-Gate Convergence | Run continuously until per-cell correlation ≥ 0.95 stable | ongoing | n/a — operational |
| H — Inner-Loop Residue Cleanup | Apply deletion list; supersede SMAP files; retire legacy tools | 1–2d | partial |

---

## 14. The reality-loop contract (one-liner)

Each `/loop` firing is one Claude Code turn. The turn (a) reads SMAP context + futurebuild docs + spec status, (b) picks the highest-priority open work item (build or capture), (c) executes end-to-end with FSV evidence, (d) records `optimizer_record_recommendation` + a SMAP `./memory/reports/...md` report, (e) ends. The daemon's continuous self-optimization scheduler consumes every shift / oracle outcome / agent-feedback event automatically — no manual `observe()` calls.

Full text: [`21_loop_contract.md`](21_loop_contract.md) + `.claude/loop.md`.

---

## 15. Provenance of this doc set

| Field | Value |
|---|---|
| Authored | 2026-05-11 |
| Sources | `docs/futurebuild/` (27 files), `docs/ruvectorfindings/` (master plan + specs), `CLAUDE.md`, `crates/` (25 crates), `memory/` (SMAP) |
| Scope | The full ME-JEPA-Code system: shipped + futurebuild + retired residue |
| Status | active reference — keep in sync with `docs/futurebuild/02_CURRENT_STATE_HONEST.md` |
| Not in scope | the SMAP narrative itself (lives in `memory/`); the constitution (lives in `specs/`); the per-tick contract (lives in `.claude/loop.md`) |

If something here contradicts a futurebuild doc, the futurebuild doc wins; file a fix as a docs-only PR.
