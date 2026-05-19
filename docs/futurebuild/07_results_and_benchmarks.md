# 07 — Results, Benchmarks, and FSV Evidence History

**Last refreshed:** 2026-05-19.

Source: aiwonder `/zfs/archive/contextgraph/fsv/` for post-migration production evidence; `/mnt/d/contextgraph/fsv/` only for legacy/local evidence; git log (`git log --oneline --all`); GitHub issue comments; per-issue RESOLVED comments. #369 tracks remaining legacy-local revalidation.

**Purpose:** preserve the proof-of-work that future agents would otherwise lose to time. Every entry here references a physical artifact (FSV JSON, sha256, CF) that can be re-verified.

---

## 1. Current Ship-Gate Status (2026-05-18)

| Metric | Target | Latest measurement | Source |
|---|---|---|---|
| `prediction_oracle_correlation` | ≥ 0.95 (stable × 4 rolling windows × all Python mutation cells) | **0.866667** (8/30 holdout) | `/mnt/d/contextgraph/fsv/swebench-300x8-ship-gate-fsv/ship-gate-fsv.json` (pre-aiwonder evidence; #369 to revalidate post-migration) |
| Per-fingerprint ship-gate windows | 4 consecutive green per fingerprint | 4 windows materialized over 30 adversarial KnownBad fingerprints with `source_corpus=python-adversarial-v1` | `CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS` |
| Live-session integration (TASK-PY-G-056 / #275 via #366) | 5 real SWE-bench session traces, ≥4/5 oracle agreement, p99 <30s, edge cases green | PASS: `allPassed=true`, 9 trace rows, 12 live prediction rows, 1 active-learning row, 9 runtime JSONL files | `/zfs/archive/contextgraph/fsv/task-py-g-056-live-session-fsv/run-20260518T212128Z/live_session_fsv.json` (sha256 `4fe5d4af…`); runtime `/zfs/archive/contextgraph/runtime/live-session-fsv/`; readback mode in `task_py_g_056_live_session_fsv` |
| Bidirectional ablation result (TASK-EK-012) | `λ_inv = 0` baseline vs `{0.25, 0.5, 1.0}` | All variants measured `0.8666666746139526`; best Δ vs baseline `0.0`; production stays `λ_inv = 0.0` | `/mnt/d/contextgraph/fsv/task-ek-012-bidirectional-ablation-fsv/run-20260518T123249Z/bidirectional_ablation_fsv.json` (sha256 `a98f9e04…`); report `/mnt/d/contextgraph/exports/ek-012/bidirectional-ablation-report.md` (sha256 `8da50218…`) |
| Patch-similarity holdout top-1 same-failure-mode accuracy (TASK-PY-G-042) | — | **1.0** (`status=pass`) | `/mnt/d/contextgraph/fsv/task-py-g-042-patch-similarity-exemplar-fsv/run-1779075407903/patch_similarity_exemplar_fsv.json` (sha256 `177cc3a7…`) |
| Adversarial corpus refusal rate (TASK-PY-G-060 / TASK-FP-009) | 0 `Pass@conf > 0.95` on lookalikes | 30/30 adversarial rows persist in `CF_MEJEPA_ADVERSARIAL_CORPUS`; 30 adversarial KnownBad fingerprints persisted | `phase-f-adversarial-corpus-fsv/`, `task-fp-009-adversarial-fingerprint-integration-fsv/` |
| Secret redaction p99 (10KB input) | < 1 ms | **< 1 ms p99**, 13/13 fixtures + 5/5 edge cases PASS | `scripts/swebench_lite_ops/python_secret_redaction.py`; sha256 `356fc2e4` (PHASE 1 closeout 2026-05-15) |
| Composite reward signals (TASK-PY-G-075) | `all_passed=true` | PASS | `/mnt/d/contextgraph/fsv/task-py-g-075-composite-reward-fsv/run-1779049005345/composite_reward_fsv.json` |
| Cold-start verdict policy (TASK-PY-G-061) | `all_passed=true` (Abstain when n_supporting<50) | PASS | `/mnt/d/contextgraph/fsv/task-py-g-061-cold-start-verdict-fsv/cold_start_verdict_fsv.json` |
| Online observe loop (TASK-PY-G-053) | `all_passed=true` | PASS | `/mnt/d/contextgraph/fsv/task-py-g-053-online-observe-fsv/run-20260518T163259Z-3847047/online_observe_fsv.json` (sha256 `a6d685ca…`) |
| Negative-action ablation falsifier (TASK-PY-G-054) | `all_passed=true` | PASS | `/mnt/d/contextgraph/fsv/task-py-g-054-negative-action-ablation-fsv/run-20260518T165320Z-3900174/negative_action_ablation_fsv.json` (sha256 `e0175958…`) |
| EWC catastrophic-forgetting protection (TASK-PY-G-065) | `allPassed=true` | PASS | `/mnt/d/contextgraph/fsv/task-py-g-065-ewc-fsv/run-20260518T151520Z/ewc_fsv.json` (sha256 `7c9cd8fc…`) |
| Continual backprop (TASK-PY-G-066) | `allPassed=true` | PASS | `/mnt/d/contextgraph/fsv/task-py-g-066-continual-backprop-fsv/run-20260518T160305Z/continual_backprop_fsv.json` (sha256 `d38c9781…`) |
| Novel-pattern detection (TASK-PY-G-067) | `all_passed=true` | PASS | `/mnt/d/contextgraph/fsv/task-py-g-067-novel-pattern-fsv/run-1779051462932/novel_pattern_fsv.json` |
| Replay buffer (TASK-PY-G-068) | `all_passed=true` | PASS | `/mnt/d/contextgraph/fsv/task-py-g-068-replay-buffer-fsv/run-1779049993557-3834881/replay_buffer_fsv.json` |
| Compression-progress CPΦ (TASK-EK-001) | monotonicity over window | shipped | `/mnt/d/contextgraph/fsv/task-ek-001-compression-progress-fsv/run-1779059471712-4091732/compression_progress_fsv.json` (sha256 `e810d2a0…`) |
| Instrument proposal (TASK-EK-002) | absence-as-existence surface live | shipped | `/mnt/d/contextgraph/fsv/task-ek-002-propose-instrument-fsv/propose_instrument_fsv.json` (sha256 `a9a5c08f…`) |
| Curiosity reward (TASK-EK-006) | per-window distributions + calibration | shipped | `/mnt/d/contextgraph/fsv/task-ek-006-curiosity-fsv/curiosity_fsv.json` (sha256 `cef7b6e5…`) |
| Project-ingest locality (TASK-AIWONDER-016 / #367) | physical CF readback proven under aiwonder project roots | PASS | `/zfs/archive/contextgraph/fsv/task-aiwonder-016-project-ingest-locality-fsv/run-20260518T204439Z/project_ingest_fsv.json` (sha256 `f1f013b5…`) |
| Project-report locality (TASK-AIWONDER-016 / #367) | report.json + report.md + CF row under aiwonder project roots | PASS | `/zfs/archive/contextgraph/fsv/task-aiwonder-016-project-report-locality-fsv/run-20260518T210045Z/project_report_fsv.json` (sha256 `00011920…`) |
| Synthetic-Python stress (TASK-PY-G-071) | 30 known-answer cases | PASS | `/mnt/d/contextgraph/fsv/task-py-g-071-synthetic-corpus-fsv/run-20260518T184812Z/synthetic_stress_fsv.json` (sha256 `b8e5c712…`) |
| PostToolUse capture (TASK-PY-G-069) | bind ≥90% test outcomes to a prediction | shipped | `/mnt/d/contextgraph/fsv/task-py-g-069-posttooluse-capture-fsv/run-1779062220556-12961/posttooluse_capture_fsv.json` |
| Stop self-verify (TASK-PY-G-070) | blocks unverified Stop with exit code 2 | shipped | `/mnt/d/contextgraph/fsv/task-py-g-070-stop-self-verify-fsv/run-1779063364468-91155/stop_self_verify_fsv.json` (sha256 `70e1ab51…`) |
| Observe missing-prediction quarantine (#337) | fail-closed CF_MEJEPA_SIGNAL_DROP_LOG | shipped | `/mnt/d/contextgraph/fsv/task-py-g-337-observe-missing-prediction-fsv/run-1779067066572/observe_missing_prediction_fsv.json` (sha256 `66427f65…`) |
| Per-cell constellation freshness (TASK-PY-G-052) | scheduler integration | shipped | `/mnt/d/contextgraph/fsv/task-py-g-052-constellation-freshness-fsv/run-1779052689893/constellation_freshness_fsv.json` |
| Q1 claim graph extractor (TASK-PY-G-041) | bytes-on-disk reconciliation | shipped | `/mnt/d/contextgraph/fsv/task-py-g-041-claim-graph-extractor-fsv/run-1779077268576/claim_graph_extractor_fsv.json` |
| Per-head DDA projection (TASK-PY-G-043) | 30-chunk FSV | shipped | `/mnt/d/contextgraph/fsv/task-py-g-043-per-head-dda-projection-fsv/run-1779078323947/head_projection_fsv.json` |
| Real-corpus constellation smoke (TASK-PY-G-095 / #395) | 7 spread aiwonder shards, 12 active embedders, 2,688 packed records, 84 per-cell centroid rows, 66 pairwise relationships, 20 candidate failure-shape rows; `flat_vector_concat_used=false` | PASS | `/zfs/archive/contextgraph/fsv/task-py-g-095-real-corpus-constellation-smoke-fsv/python_real_corpus_constellation_smoke_fsv.json` (sha256 `552de73b…`), independent readback `/zfs/archive/contextgraph/fsv/task-py-g-095-real-corpus-constellation-smoke-fsv/independent_readback.json`, external edge readback `/zfs/archive/contextgraph/fsv/task-py-g-095-real-corpus-constellation-smoke-fsv/external_edge_case_readback.json` |
| Constellation training objective smoke (TASK-PY-G-096 / #396) | Real aiwonder 391-shard forward-cache joined to the 300×8 oracle index; 353 training rows, 1,440 per-slot vector rows, 2,727 row-pair observations, 66 pairwise relationships; per-slot/pairwise/contradiction/temporal/oracle/anti-compensation certificate; local baseline ladder plus historical 0.866667 context; `flat_vector_concat_used=false`; promotion refused | PASS | `/zfs/archive/contextgraph/fsv/task-py-g-096-constellation-training-objective-fsv/python_constellation_training_objective_fsv.json` (sha256 `9a58e747…`), certificate sha256 `a5ec541d…`, independent readback `/zfs/archive/contextgraph/fsv/task-py-g-096-constellation-training-objective-fsv/independent_readback.json` (sha256 `52498f5b…`), external edge readback sha256 `37f1dc39…` |
| Constellation capability-discovery ablation suite (TASK-PY-G-093 / #393) | Consumes #396 arrays; 3 baselines + 6 ablations over 353 rows; flat-vector baseline is comparison-only and never ship-gate eligible; every ship-gate-eligible #396-derived scalar constellation ablation is demoted vs the local baseline ladder; Q4/Q5/OOD label gaps are explicit, not fabricated | PASS | `/zfs/archive/contextgraph/fsv/task-py-g-093-constellation-ablation-suite-fsv/python_constellation_ablation_suite_fsv.json` (sha256 `d74f8eae…`), report sha256 `6fad7569…`, metrics sha256 `5a7118c9…`, independent readback sha256 `a1870713…`, external edge readback sha256 `3924c948…` |
| Cross-embedder constellation tensors (TASK-PY-G-088 / #388) | Real aiwonder 391-shard forward-cache sampled across 8 spread shards; 12 active slots preserved; 516 per-cell/window/slot distribution tensors with centroid and variance vectors; 2,838 per-cell/window upper-triangle pair tensors; 1,400 valid pair rows; 8 full cell/windows with all 66 active pair rows; surfaces classified as redundant/independent/mixed/contradictory/unsupported; `flat_vector_concat_used=false` | PASS | `/zfs/archive/contextgraph/fsv/task-py-g-088-cross-embedder-correlation-tensors-fsv/python_constellation_cross_embedder_tensors_fsv.json` (sha256 `a547af92…`), artifacts under `/zfs/archive/contextgraph/corpus/constellation-tensors-v1/`: slot tensors sha256 `57885d37…`, pairwise tensors sha256 `310a4e5e…`, report sha256 `91dc0f98…`, manifest sha256 `e6a07bb1…` |
| Per-slot calibration and OOD guard (TASK-PY-G-089 / #389) | Consumes #388 tensors plus #396 training rows; emits 12 global slot calibrations, 84 fail-closed per-cell slot rows, 66 pair calibrations, strict conjunction OOD policy, and edge proof that missing/stale/nonfinite/one-bad-slot panels reject even when an aggregate score would pass; `flat_vector_concat_used=false`; not ship-gate-countable until #127/#129/#130 rerun under this guard | PASS | `/zfs/archive/contextgraph/fsv/task-py-g-089-per-slot-calibration-ood-fsv/python_per_slot_calibration_ood_fsv.json` (sha256 `24075e0b…`), artifacts under `/zfs/archive/contextgraph/state/calibration/per-slot-v1/`: per-slot sha256 `f5ef54b4…`, per-cell sha256 `19e88e2e…`, per-pair sha256 `ef149e91…`, policy sha256 `6d95857a…`, report sha256 `71781512…` |

### 1.1 Aiwonder Corpus Data Map For Constellation Training

The active Python ship-gate data lives in three separate aiwonder artifacts that must not be confused:

| Artifact | Path | Role |
|---|---|---|
| 300×8 SWE-bench Lite Python corpus + oracle labels | `/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json` | 2,400 entries; `oracle_all_passed` is the #396 label source, joined by `(task_instance_id, mutation_category)` |
| AST chunk store | `/zfs/hot/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl` | Chunk metadata and stable row keys: `task_instance_id`, `workspace_state`, `chunk_id`, `source_sha256` |
| Full packed embedder forward cache | `/zfs/archive/contextgraph/corpus/python-forward-cache-v1-aiwonder-prod-20260515-191116Z/` | 391 completed shards; per-shard packed manifests point to each embedder's record/index JSONL; active embedders are `e1,e2,e3,e4,e6,e7,e8,e9,e10,e12,e13,e14` |
| Cross-embedder constellation tensor store | `/zfs/archive/contextgraph/corpus/constellation-tensors-v1/` | TASK-PY-G-088 output: `slot_distribution_tensors.jsonl`, `pairwise_correlation_tensors.jsonl`, `constellation_tensor_report.json`, `constellation_tensor_manifest.json`; FSV at `/zfs/archive/contextgraph/fsv/task-py-g-088-cross-embedder-correlation-tensors-fsv/python_constellation_cross_embedder_tensors_fsv.json` |
| Per-slot calibration and OOD guard | `/zfs/archive/contextgraph/state/calibration/per-slot-v1/` | TASK-PY-G-089 output: `per_slot_calibration.jsonl`, `per_cell_slot_calibration.jsonl`, `per_pair_calibration.jsonl`, `ood_guard_policy.json`, `per_slot_calibration_report.json`; FSV at `/zfs/archive/contextgraph/fsv/task-py-g-089-per-slot-calibration-ood-fsv/python_per_slot_calibration_ood_fsv.json` |

TASK-PY-G-096 writes its certificate and generated arrays under `/zfs/archive/contextgraph/fsv/task-py-g-096-constellation-training-objective-fsv/artifacts/`: `constellation_training_rows.jsonl`, `constellation_slot_vectors.jsonl`, `constellation_pairwise_observations.jsonl`, `constellation_pairwise_relationships.jsonl`, and `constellation_training_certificate.json`.

---

## 2. Phase-by-phase status

### 2.1 Phase A (predictor skeleton)

**Status:** code/FSV shipped (independent audit 2026-05-12 verified 35/35 items present).
**Issues:** #3–#19 all CLOSED including umbrella #15.
**FSV (legacy):** `mejepa-cf-fsv/`, `corpus-fsv/`, `mejepa-predict-fsv/`, `mejepa-heal-fsv/`, `mejepa-phase-e-fsv/`.

### 2.2 Phase B (schema + prediction + API)

**Status:** code/FSV shipped.
**FSV:** `contextgraph-phase-b-fsv/`, `phase-b-label-analyzer-fsv/`.

### 2.3 Phase C (corpus expansion to 300×8 Python SWE-bench Lite)

**Status:** code/FSV shipped.
**FSV:** `swebench-300x8-ship-gate-fsv/`, `swebench-python-ast-corpus-fsv/`, `python-ast-atomic-fsv/`, `corpus-resume-repair-20260512T030000Z/`, `corpus-durable-root-20260511/`, `python-swebench-dda-tct-issue-backlog-fsv/`.
**Corpus:** 300 instances × 8 mutations = 2400 rows for Python; 399,777 chunk rows on aiwonder `/zfs/hot/contextgraph/` chunks.jsonl (820 MB).

### 2.4 Phase D (enrichment substrate — DDA/TCT/AST chunking)

**Status:** code/FSV shipped.
**FSV:** `phase-d-fsv/`, `dda-prediction-consumption-fsv/`, `d-series-blind-spots-fsv/`.

### 2.5 Phase E (self-optimization policy code/FSV)

**Status:** code/FSV shipped.
**FSV:** `mejepa-phase-e-fsv/phase_e_self_optimization_fsv.json`.

### 2.6 Phase F (production operations, observability, runbook, DR, API)

**Status:** code/FSV-complete; all Phase F implementation issues CLOSED as of 2026-05-13; `phase:f-followup` empty as of 2026-05-13.

**Phase F FSV inventory** (legacy `/mnt/d/contextgraph/fsv/phase-f-*`; historical unless revalidated under a current aiwonder issue):
`phase-f-active-learning-cli-fsv`, `phase-f-adversarial-corpus-fsv`, `phase-f-agent-identity-fsv`, `phase-f-catastrophic-event-fsv`, `phase-f-chaos-failpoints-fsv`, `phase-f-closure-ledger-fsv`, `phase-f-cross-language-label-gating-fsv`, `phase-f-daemon-status-fsv`, `phase-f-deprecation-framework-fsv`, `phase-f-e2e-happy-path-fsv`, `phase-f-gpu-wsl-telemetry-fsv`, `phase-f-heal-ticker-telemetry-fsv`, `phase-f-health-probe-fsv`, `phase-f-inspect-prediction-fsv`, `phase-f-label-transfer-gating-fsv`, `phase-f-load-test-fsv`, `phase-f-mi-heatmap-fsv`, `phase-f-multi-agent-fsv`, `phase-f-operator-override-fsv`, `phase-f-override-sampler-fsv`, `phase-f-pause-active-learning-fsv`, `phase-f-pause-cli-fsv`, `phase-f-pause-consumer-fsv`, `phase-f-per-head-calibration-fsv`, `phase-f-prediction-replay-fsv`, `phase-f-renderers-fsv`, `phase-f-restart-safety-fsv`, `phase-f-retention-fsv`, `phase-f-runbook-actionability-fsv`, `phase-f-runbook-cli-fsv`, `phase-f-runbook-coverage-fsv`, `phase-f-soak-test-fsv`, `phase-f-storage-exhaustion-fsv`, `phase-f-system-cost-fsv`, `phase-f-system-cost-producers-fsv`, `phase-f-toolchain-availability-fsv`, `phase-f-toolchain-startup-gate-fsv`, `phase-f-weekly-dashboard-fsv`, `phase-f-weekly-eval-fsv`, `phase-f-witness-quarantine-fsv`, `phase-f-workflow-sha-pins-fsv`.

### 2.7 Phase G (ship-gate convergence — IN FLIGHT)

**Status:** 21 Phase G tasks code/FSV shipped (TASK-G-001 through TASK-G-021 plus TASK-PY-G-* shipping continuously). 169 phase:g-labeled issues closed; 81 open after the 2026-05-19 #388 closeout.

**Phase G FSV directories** (legacy `/mnt/d/contextgraph/fsv/phase-g-*`; #369 classifies each as legacy-local, aiwonder-current, or needs-aiwonder-rerun):
`phase-g-adversarial-retrain-fsv`, `phase-g-cell-exemptions-fsv`, `phase-g-cell-status-fsv`, `phase-g-cli-github-curl-403-fsv`, `phase-g-conformal-recalibration-fsv`, `phase-g-convergence-eta-fsv`, `phase-g-failing-cell-root-cause-fsv`, `phase-g-failure-fingerprint-fsv`, `phase-g-failure-mode-class-metrics-fsv`, `phase-g-fingerprint-classifier-fsv`, `phase-g-fingerprint-mcp-tools-fsv`, `phase-g-fingerprint-ship-gate-fsv`, `phase-g-holdout-rotation-fsv`, `phase-g-label-quality-fsv`, `phase-g-operator-override-recalibration-fsv`, `phase-g-oracle-flakiness-quarantine-fsv`, `phase-g-per-cell-promotion-gate-fsv`, `phase-g-public-ci-cross-validation-fsv`, `phase-g-ship-gate-dashboard-fsv`, `phase-g-ship-gate-stability-fsv`, `phase-g-state-transfer-weekly-markdown-fsv`, `phase-g-targeted-mutation-expansion-fsv`, `phase-g-telemetry-feedback-fsv`, `phase-g-training-holdout-drift-fsv`, `phase-g-training-pipeline-orchestrator-fsv`, `phase-g-unknown-fingerprint-active-learning-fsv`, `phase-g-weekly-cell-triage-fsv`.

**TASK-G-001 through TASK-G-021** (per phase-g-roadmap §7 inventory):
- TASK-G-001 training-pipeline orchestrator — sha256 evidence captured 2026-05-13
- TASK-G-002 per-cell convergence driver — sha256 captured 2026-05-13
- TASK-G-003 targeted mutation expansion
- TASK-G-004 conformal recalibration on promotion
- TASK-G-005 holdout rotation
- TASK-G-006 four-window ship-gate stability
- TASK-G-007 per-cell exemptions
- TASK-G-008 operator dashboard
- TASK-G-009 production-telemetry feedback
- TASK-G-016 per-cell promotion gate
- TASK-G-017 training/holdout KL drift telemetry (`MEJEPA_TRAINING_HOLDOUT_DRIFT`, sha256 `59c345ec`)
- TASK-G-018 weekly operator cell triage (sha256 `b83e440d`)
- TASK-G-019 oracle flakiness quarantine (sha256 `f5dc3dea`)
- TASK-G-020 per-failure-mode-class precision/recall reporting
- TASK-G-021 per-cell convergence ETA tracking

### 2.8 Phase H (residue cleanup — inner-loop retirement)

**Status:** complete (`/mnt/d/contextgraph/fsv/phase-h-*` proves audit). 14 phase:h issues closed.

**FSV:** `phase-h-agent-roster-retirement-fsv`, `phase-h-ccreality-engine-retired-fsv`, `phase-h-claude-inner-loop-reference-fsv`, `phase-h-deletion-list-audit-fsv`, `phase-h-no-inner-actor-models-fsv`, `phase-h-reality-loop-binary-absence-fsv`, `phase-h-retired-mcp-tools-fsv`, `phase-h-scripts-config-inner-llm-residue-fsv`, `phase-h-smap-superseded-decisions-fsv`.

### 2.9 EK series (explanatory-knowledge programme — EPIC #321)

**Status:** in flight; many sub-tasks shipped.

| Task | Status | Evidence |
|---|---|---|
| TASK-EK-001 — Compression-progress CPΦ + MCP tool | ✅ shipped | `task-ek-001-compression-progress-fsv/` (sha256 `e810d2a0…`) |
| TASK-EK-002 — propose_instrument MCP tool | ✅ shipped | `task-ek-002-propose-instrument-fsv/` (sha256 `a9a5c08f…`) |
| TASK-EK-003 — mincut_panel MCP tool (Stoer-Wagner) | ✅ shipped | `task-ek-003-mincut-panel-fsv/` |
| TASK-EK-006 — Curiosity reward (compression-progress driven) | ✅ shipped | `task-ek-006-curiosity-fsv/` (sha256 `cef7b6e5…`) |
| TASK-EK-010 — intent elicitation | RETIRED by #358 (intent modeling retired) | (historical FSV; not active) |
| TASK-EK-012 — Bidirectional corpus + inverse-map head | ✅ shipped (neutral ablation) | `task-ek-012-bidirectional-corpus-fsv/` + `task-ek-012-bidirectional-ablation-fsv/` |
| TASK-EK-014 — Operator-contribution surface | in progress | `task-ek-014-operator-contributions-fsv/` |
| TASK-EK-015 — Credibility-weighted operator override | in progress | `task-ek-015-credibility-override-fsv/` |
| TASK-EK-016 — Chunk foundationality (PageRank) | in progress | `task-ek-016-foundationality-fsv/` |
| TASK-EK-016-ext — Embedder foundationality | in progress | `task-ek-016-ext-embedder-foundationality-fsv/` |
| TASK-EK-017 — Library foundationality | in progress | `task-ek-017-library-foundationality-fsv/` |
| TASK-EK-018 — Self-designed/trained/promoted embedders (APEX) | scaffolding shipped | `task-ek-018-self-designed-embedders-fsv/` + 018A-J |
| TASK-EK-018A — Dynamic registration scaffolding | ✅ shipped | `task-ek-018a-dynamic-registration-fsv/` |
| TASK-EK-018B — Absence-queue | ✅ shipped | `task-ek-018b-absence-queue-fsv/` |
| TASK-EK-018C — Falsification gate | ✅ shipped | `task-ek-018c-falsification-gate-fsv/` |
| TASK-EK-018D — Algorithmic embedder synthesis | ✅ shipped | `task-ek-018d-algorithmic-embedder-synthesis-fsv/` |
| TASK-EK-018E — Learned head synthesis | ✅ shipped | `task-ek-018e-learned-head-synthesis-fsv/` |
| TASK-EK-018F — TransE on corpus entity graph (E11 replacement) | ✅ shipped | `task-ek-018f-entity-kge-embedder-fsv/` |
| TASK-EK-018G — Freeze + audit machinery | ✅ shipped | `task-ek-018g-freeze-audit-fsv/` |
| TASK-EK-018H — Operator-in-the-loop gates | ✅ shipped | `task-ek-018h-operator-gates-fsv/` |
| TASK-EK-018I — VRAM budget integration | ✅ shipped | `task-ek-018i-dynamic-vram-fsv/` |
| TASK-EK-018J — Adversarial-injection mitigation | ✅ shipped | `task-ek-018j-adversarial-injection-fsv/` |

### 2.10 FP series (failure-shape fingerprints — EPIC #309)

**Status:** TASK-FP-001..010 + TASK-FP-101..105 code/FSV shipped through 2026-05-17.

| Task | Status | Evidence |
|---|---|---|
| TASK-FP-001/002 — first-class fingerprint catalog | ✅ shipped | `task-fp-002-seed-fingerprint-materializer-fsv/` |
| TASK-FP-003 — fingerprint τ calibration | ✅ shipped | `task-fp-003-fingerprint-tau-calibration-fsv/` |
| TASK-FP-005 — Unknown/OOD fingerprint active-learning queue | ✅ shipped | (per commit 702b5d35) |
| TASK-FP-006 — 7 `mejepa_fingerprint_*` MCP tools | ✅ shipped | `mcp-fingerprint-tools-fsv/` |
| TASK-FP-007 — fingerprint ship gate (4-window) | ✅ shipped | `phase-g-fingerprint-ship-gate-fsv/` |
| TASK-FP-008 — fingerprint catalog stability (EWC Fisher + CBP dormancy) | ✅ shipped | `task-fp-008-fingerprint-stability-fsv/` |
| TASK-FP-009 — adversarial fingerprint integration | ✅ shipped | `task-fp-009-adversarial-fingerprint-integration-fsv/` |
| TASK-FP-010 — matched_fingerprint on RealityPrediction (live) | ✅ shipped | `task-fp-010-matched-fingerprint-live-fsv/` |
| TASK-FP-101 — SIGReg regularizer | ✅ shipped | `task-fp-101-sigreg-fsv/` |
| TASK-FP-102 — Hierarchical JEPA (file/function/AST/chunk) | ✅ shipped | `task-fp-102-hierarchical-jepa-fsv/` |
| TASK-FP-103 — Latent action best-of-K search | ✅ shipped | `task-fp-103-latent-search-fsv/` |
| TASK-FP-104 — Hardwired objective + safety constraints | ✅ shipped | `task-fp-104-objective-safety-fsv/` |
| TASK-FP-105 — Counterfactual repair ranking | ✅ shipped | `task-fp-105-rank-candidates-fsv/` |

### 2.11 RWD series (reward-signal completeness — EPIC #315)

**Status:** TASK-RWD-201..210 + TASK-RWD-315 shipped.

| Task | Status | Evidence |
|---|---|---|
| TASK-RWD-201 — Cyclomatic complexity | ✅ shipped | `task-rwd-201-cyclomatic-fsv/` |
| TASK-RWD-202 — Coverage | ✅ shipped | `task-rwd-202-coverage-fsv/` |
| TASK-RWD-203 — Diff topology | ✅ shipped | `task-rwd-203-diff-topology-fsv/` |
| TASK-RWD-204 — Q4 security (Bandit + Semgrep) | ✅ shipped | `task-py-g-046-q4-security-label-fsv/` |
| TASK-RWD-205 — Type graph (Python call-site type evidence) | ✅ shipped | `task-rwd-205-type-graph-fsv/` |
| TASK-RWD-206 — Runtime telemetry + symbol profiles | ✅ shipped | `task-rwd-206-telemetry-fsv/` |
| TASK-RWD-207 — Live-session state rows | ✅ shipped | `task-rwd-207-live-session-fsv/` |
| TASK-RWD-208 — Static analysis (mypy + pyright + ruff) | ✅ shipped | `task-py-g-074-static-analysis-fsv/` |
| TASK-RWD-209 — Composite (coverage + churn + complexity + doc + dep-CVE) | ✅ shipped | `task-py-g-075-composite-reward-fsv/` |
| TASK-RWD-210 — Prioritised replay buffer (surprise + per-cell gap) | ✅ shipped | `task-py-g-068-replay-buffer-fsv/` |
| TASK-RWD-211 — Novel-cluster ontology growth audit | ✅ shipped | `task-py-g-067-novel-pattern-fsv/` |
| TASK-RWD-315 — Reward-signal audit (fail-closed signal drop log) | ✅ shipped | `task-rwd-315-reward-audit-fsv/` |

### 2.12 PY-DATA series (data substrate; aiwonder migration)

**Status:** majority shipped. TASK-PY-DATA-001 through TASK-PY-DATA-075 visible in FSV directory. Includes corpus fingerprinting, AST parsing, oracle baselines, forward cache, sharded CUDA scheduler, batch planner, hot-staging, resume gates, embedder contribution audit, manifest/forward/intelligence-matrices/storage-design smokes, post-#249 smoke pipeline, full-AST-corpus generation.

**Key result:** aiwonder forward-cache substrate complete at `391/391` shards (per 2026-05-17 status refresh).

### 2.13 Migration epic (aiwonder — EPIC #296)

**Status:** in flight. Migration AIWONDER-001 through AIWONDER-018 across `phase:migration` label. **Closed:** #296 EPIC context, #298 install Rust toolchain (some still in-progress), #308 parallel-isolated packed_records merging, #313 parallel-isolated consolidation crash fix.

**Key results:**
- `cg_pydata016_aiwonder_prod_par6_20260515T211318Z` tmux session, `CG_MEJEPA_PY_FORWARD_PARALLEL_EMBEDDERS=6`, GPU 67% util / 11.9 GB VRAM, ETA ~21 h (vs ~58 h sequential).
- SWE-bench Lite upstream downloaded fresh to aiwonder `/zfs/hot/contextgraph/.cache/huggingface/hub/datasets--princeton-nlp--SWE-bench_Lite/` (300 test + 23 dev, 12 repos).
- Substrate: chunks.jsonl (820 MB, 399,777 rows, 300 inst × 8 mut), frozen-instrument-registry (78 KB), HF cache (5 MB) on `/zfs/hot/contextgraph/`.
- Storage policy locked: #363 — aiwonder `/zfs/archive/contextgraph/` durable; #364 enforce post-migration root; #365 fail-closed on local durable roots in production.

---

## 3. Closed `type:decision` ADRs (locked rules; do NOT relitigate without operator approval)

| # | Title | Locked since | Impact |
|---|---|---|---|
| #363 | aiwonder is durable SoT for post-migration ContextGraph Phase G | 2026-05-15 | Storage policy (see `03_storage_schema.md §5`) |
| #358 | Retire intent/north-star modeling from ME-JEPA | 2026-05-18 | `mejepa_elicit_intent` retired; live-session operator-goal embedding removed |
| #397 | ME-JEPA consequence-prediction truth-system contract | 2026-05-18 | "Predicting reality" means predicting consequences of code being live; MCP presents calibrated consequence evidence to the operator/agent; no hidden intent inference |
| #341 | TASK-EK-018 self-designed/trained/promoted embedders (APEX deliverable of EPIC #321) | 2026-05 | Establishes EK-018 sub-task tree |
| #315 | Reward-signal completeness audit — maximum capture from durable reality (LeCun-aligned) | 2026-05 | EPIC anchoring TASK-RWD-* |
| #314 | Full JEPA architectural alignment per Yann LeCun (2026-05-17 transcript) | 2026-05-17 | EPIC anchoring TASK-FP-101..105 |
| #309 | Failure-Shape Fingerprints as the first-class ME-JEPA training object | 2026-05 | EPIC anchoring TASK-FP-001..010 |
| #212 | WSL ext4 hot staging with D-drive durable SoT | superseded by #363 | Historical; legacy local rule |

Closed `type:decision` issues are immutable architectural decisions per doctrine §3.10.

---

## 4. Compute / Throughput Benchmarks

| Workload | Measurement | Source / commit |
|---|---|---|
| Aiwonder parallel-isolated forward (per-shard wall) | ~21 h total for 391 shards (≈3.2 min/shard) at par6, vs ~58 h sequential | tmux `cg_pydata016_aiwonder_prod_par6_20260515T211318Z` |
| WSL baseline per-shard (calibration) | shard 0 ~1336 s with 1024-chunk amortization | tmux `cg_pydata016_full_shardsize-1024-20260515-173510Z` |
| Tier compression (8/7/5/3-bit groupwise scalar quantization) | Ratios 3.89× / 4.43× / 6.12× / 9.91× (incl. 22-byte header); cosine 0.99999 / 0.99997 / 0.99960 / 0.99192 on 768-dim Gaussian | 2026-05-09c memory note; `crates/context-graph-tier-compression/` FSV |
| SWE-bench gold mode (one instance, real Docker) | 23.7 s `resolved=true`, 22 tests all Pass | 2026-05-09e memory note; `swebench/sweb.eval.x86_64.sympy_1776_sympy-20590:latest` |
| SWE-bench custom mode (same image) | 23.0 s same outcome | 2026-05-09e memory note |
| Secret redaction | < 1 ms p99 on 10 KB input | TASK-PY-G-059 / #278 PHASE 1 sha256 `356fc2e4` |
| Patch-similarity HNSW (usearch over 4-signature concatenation) | top-1 same-failure-mode accuracy 1.0 on holdout | TASK-PY-G-042 sha256 `177cc3a7…` |
| MCP+Storage test suite (2026-02-26 snapshot) | 1,179 passed, 0 failed | memory note |
| Full workspace test (2026-02-23 snapshot, pre-Phase G expansion) | 6,353 passed, 0 failed | memory note |
| Quantized embedder benchmarks (TASK-EMB-022) | 13 quantized CFs benched via `crates/context-graph-benchmark/[[bin]]` `maxsim_bench`, `causal_relationships_bench`, `hybrid_effectiveness_bench`, `e11_quality_bench` | per Cargo.toml |

---

## 5. Audit Pass History (hidden-brokenness audits #1 through #15)

| Audit | Date | Findings | Status |
|---|---|---|---|
| Audit #1 | 2026-02-15 | 52 (H+M+L) | ALL fixed |
| Audit #2 | 2026-02-17 | 31 | ALL fixed |
| Audit #3 | 2026-02-18 | 40 | ALL fixed |
| Audit #4 | 2026-02-20 | 33 | ALL fixed |
| Audit #5 | 2026-02-20 | 46 (8H, 16M, 22L) | 43 fixed, 3 false positives |
| Audit #6 | 2026-02-21 | 11 | 9 fixed, 1 false positive |
| Audit #7 / #8 | 2026-02-21 | 28 + test fixes | ALL fixed |
| Audit #9 | 2026-02-21 | 19 + ~37K dead-code LOC | 12 fixed + 2 code-simplifier refinements; dead code classified separate PR |
| Audit #10 | 2026-02-22 | 48 (7H, 20M, 20L, 1I) | ALL fixed |
| Audit #11 | 2026-02-22 | 42 (9H, 14M, 17L, 2I) | ALL fixed |
| Audit #12 | 2026-02-22 | 79 (16H, 33M, 25L, 5I) + ~5,100 LOC dead | 18 fixed, 62 remaining at the time |
| Audit #13 | 2026-02-23 | 67 (7H, 24M, 28L, 8I) + ~7,100 LOC dead | ALL fixed |
| Audit #14 | 2026-02-23 | 92 (13H, 33M, 46L) | ALL fixed |
| Audit #15 | 2026-02-23 | 48 (1H, 14M, 33L) | ALL fixed |
| Memory Review | 2026-02-24 | 22 (3H, 9M, 10L) + 2 code-simplifier refinements | ALL fixed; Grade A−; deep FSV 68/68 PASS |

Documents (where retained): `docs/HIDDEN_BROKENNESS_AUDIT_*.md`, `docs/MEMORY_REVIEW_2026_02_24.md`.

---

## 6. Notable bugs caught by FSV (failure of "tests would have caught it" → success of "FSV did catch it")

| Bug | How caught | Fix |
|---|---|---|
| `off_by_one` operator extracted fractional parts of floats | Real-Python mutation test | Token-stream guard |
| `subtle_flip` picked `>` of Python `->` arrow | Real-Python mutation test | Lexer-aware operator detection |
| Mutation corpus `+` in Docker run_id rejected by Docker container-name regex | Real Docker oracle execution against `sympy-20590:latest` image | run_id sanitizer added |
| Repo-atomic split greedy starves small buckets | Real 25-repo split eval | LPT pre-sort + lowest-current/target-ratio bin-pack |
| f64 JSON round-trip 1-ULP shift | Split provenance comparison | Re-derive via canonical hash, not stored field |
| Embedder `E5` causal cosine on `CodeStore` returned raw `[-1, 1]` instead of `[0, 1]` | Audit #10 / code-simplifier | Normalize at write site |
| `Pipelinebuilder.k()` dead | Audit | Removed |
| `bincode + skip_serializing_if` broken | Audit | Switch to JSON for provenance/SourceMetadata |
| `RwLockReadGuard` (parking_lot) `!Send` across `.await` | Audit | Drop guard before await |
| HNSW empty index when newly-enabled embedders ship without explicit rebuild | E2 sinusoidal fix | Trigger rebuild on embedder enable |
| Hard_negative_scale ≥ 2.0 + temperature 0.05 → catastrophic collapse | Training collapse | hard_negative_scale=1.0 |
| Train/inference prefix mismatch (`"search_document:"` required by nomic-embed) | Embedder eval | Prefix at both train and infer |
| E14 phase-B quantized fixture broken | TASK-PY-DATA-* FSV | Fixture regenerated (commit f22b3509) |
| #313 — parallel-isolated consolidation `shutil.Error` on shard 300 crashed prod after 23.8h | aiwonder production crash | Race fix |
| #295 — #199 resume launch scanned full D-drive FSV root (perf hit) | Operator observation | Scope-restricted resume scan |
| `Pipelinebuilder.k()` setting never propagated | Audit | Pass k through to execute_stages |
| Pipeline-internal env_api_token regex re-matched `[REDACTED_*]` markers | TASK-PY-G-059 FSV | Negative lookahead |
| Pipeline-internal env_password regex missed dict-literal form | TASK-PY-G-059 FSV | Optional quote around key + key surface expansion |

**Theme:** FSV protocol's "no mocks for completion gates + boundary cases + independent readback" caught real bugs that ordinary unit tests miss.

---

## 7. Aggregate Legacy FSV Directory Inventory (327 entries on `/mnt/d/contextgraph/fsv/`)

Categorized:

| Category | Count | Sample entries |
|---|---|---|
| Phase A–H umbrella | ~25 | `phase-d-fsv`, `phase-h-*` |
| Phase F | ~40 | `phase-f-*` |
| Phase G | ~27 | `phase-g-*` |
| TASK-PY-DATA-* (substrate / aiwonder migration) | ~80 | `task-py-data-001..075-*` |
| TASK-PY-G-* (Phase G atomic tasks) | ~30 | `task-py-g-001..075` |
| TASK-EK-* (EPIC #321 explanatory knowledge) | ~24 | `task-ek-001..018j-*` |
| TASK-FP-* (EPIC #309/#314 fingerprint/JEPA) | ~13 | `task-fp-002..010-*`, `task-fp-101..105-*` |
| TASK-RWD-* (EPIC #315 reward-signal) | ~7 | `task-rwd-201..207-*`, `task-rwd-315-*` |
| Issue-numbered legacy | ~10 | `issue-159-*`, `issue-175-*`, `issue-214-*`, `issue-259-*`, `issue-313-*` |
| Smoke / setup / corpus | ~20 | `first-time-setup`, `swebench-300x8-ship-gate-fsv`, `corpus-*`, `swebench-lite-300x8-20260511`, `temporal-embedders-fsv`, `test-capture-fsv`, `mcp-test-suite-156-fsv` |
| Misc / per-feature | balance | `active-learning-fsv`, `dda-prediction-consumption-fsv`, `docs-fsv`, `legacy-ast-chunker-parity-fsv`, `mejepa-cf-fsv`, etc. |

**Production aiwonder FSV target** (`/zfs/archive/contextgraph/fsv/`): 0 directories as of 2026-05-18 — migration in progress; #369 (TASK-AIWONDER-018) tracks revalidation of post-migration FSV artifacts on aiwonder.

---

## 8. Weekly Export Cadence (operator-visible analytics)

`/zfs/archive/contextgraph/exports/weekly/<YYYY-MM-DD>/` (aiwonder; `/mnt/d/contextgraph/exports/weekly/...` legacy). Per `06_operations_and_fsv.md §2.2`:

- `weekly.md` — TASK-G-018 operator cell triage (top 10 failing cells/actions)
- `ablation_report.md` — TASK-PY-G-054 negative-action ablation
- `compression_progress.md` (rendered into `weekly.md` section) — TASK-EK-001 CPΦ
- `convergence_eta.md` — TASK-G-021 per-cell convergence ETA
- `holdout_drift.md` — TASK-G-017 training/holdout KL drift
- `failure_mode_class_metrics.md` — TASK-G-020 per-class precision/recall
- `oracle_flakiness.md` — TASK-G-019 quarantined corpus tasks
- `ek-012/bidirectional-ablation-report.md` — TASK-EK-012 ablation (sha256 `8da50218…`)

---

## 9. What This Doc Does NOT Cover

- Per-FSV-artifact byte-exact JSON contents — read the file at the cited path.
- Statistical significance of the 0.866667 holdout vs 0.95 gate — that's a Phase G open analysis (TASK-G-002 cell status, TASK-G-021 ETA).
- Operator's local hardware benchmarks (CPU/disk/GPU profiling) — outside scope.
- Aiwonder hardware spec — see TASK-AIWONDER-017 (host capability utilization audit) when shipped.
