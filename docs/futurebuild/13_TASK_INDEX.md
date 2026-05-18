# 13 — Task Index (Atomic Task DAG)

> **Purpose.** Atomic, sequenced, traceable task list. Each TASK-* corresponds to one Level-4 task per `prdtospec.md` — sized for one focused engineer-session, with explicit definition-of-done and dependencies. Atomic task files live under [`tasks/<domain>/`](tasks/).
>
> **Authority.** `12_IMPLEMENTATION_PHASES.md` orders the phases; this document orders the tasks within and across phases.
>
> **Binding verification contract.** Every TASK-* file in this DAG inherits from [`specs/FSV-PROTOCOL.md`](specs/FSV-PROTOCOL.md). The protocol is the *how*; the task file is the *what*. Picking up a task without first reading the protocol means you skip the SoT-readback, boundary-case, and Occam's-razor requirements — and the task is not done without them. **`ASSUME THE TASK IS WRONG`**: audit every file path, symbol name, and CF reference in the task against the live tree before implementing (see `FSV-PROTOCOL.md §7`).

---

## Domain prefixes

| Prefix | Domain |
|---|---|
| `TASK-FLYWHEEL-NNN` | data flywheel: daemon, scheduler, oracle binding, CFs |
| `TASK-PREDICT-NNN` | live prediction API, multi-head, RealityPrediction, hooks |
| `TASK-ENRICH-NNN` | DDA / TCT enrichment: neural embedder forwards, DDA signal compute |
| `TASK-LOOP-NNN` | loop refactor + inner-loop residue cleanup |
| `TASK-HEAL-NNN` | self-healing scheduler + drift + dissipation + promotion |
| `TASK-CORPUS-NNN` | per-language operators + 300×8 corpus run |
| `TASK-OBS-NNN` | observability: weekly eval cron, MI heatmap, GPU telemetry, CLI |
| `TASK-UTML-NNN` | learning-formula extensions: per-head deltas, cert cols, lambda ramp |
| `TASK-FP-NNN` | failure-shape fingerprint catalog + LeCun JEPA-alignment pillars (per [`28_LECUN_JEPA_ALIGNMENT.md`](28_LECUN_JEPA_ALIGNMENT.md); GitHub EPICs #309 + #314) — TASK-FP-001..010 land the typed fingerprint catalog; TASK-FP-101..105 land the five remaining LeCun JEPA pillars (SIGreg / hierarchical / latent-search / objective+safety / counterfactual-rank) |
| `TASK-RWD-NNN` | reward-signal completeness from durable reality (per [`29_REWARD_SIGNAL_COMPLETENESS.md`](29_REWARD_SIGNAL_COMPLETENESS.md); GitHub EPIC #315) — TASK-RWD-201..212 close the 8-tier reward-signal-completeness audit (static-quality / runtime-telemetry / live-session / constellation refresh) |

## Current gate

The A-F and H implementation issues are closed for the Phase G handoff. Analyzer-backed instrument parity (#4), production telemetry (#11), UTML carrier reconciliation (#13), Phase E self-optimization, Phase F operations, and Phase H residue cleanup all have source-of-truth FSV. The latest holdout remains `0.866667 < 0.95`; that is Phase G model-quality convergence, not an A-F/H implementation blocker. The active queue is now Python-first ME-JEPA-Code convergence: close the 8 Python mutation-category cells on the 300×8 SWE-bench Lite Python corpus before deferred non-Python broadening.

**2026-05-17 TASK-EK-001 / #322 compression-progress completion:** `TrainingCertificate.conditional_description_length_bits` now records the conditional description length of the frozen target panel for new certs while preserving historical cert compatibility when absent. `mcp__cgreality__mejepa_compression_progress`, `context-graph-cli mejepa compression-progress`, and the weekly `Compression Progress` section read `CF_MEJEPA_TRAIN_CERTS`, compute `CPΦ(t) = bits(t-1) - bits(t)`, report rolling mean CPΦ, and fail closed on missing/non-finite/regressing cert streams. FSV `/mnt/d/contextgraph/fsv/task-ek-001-compression-progress-fsv/run-1779059471712-4091732/compression_progress_fsv.json` sha256 `e810d2a0cb8c8a07521c37aa866ef3f94de6cc49d76bdf495a3443c0b3db97a4` (`all_passed=true`, direct/MCP/CLI parity, weekly section rendered, empty/single/regression/missing-bits/non-finite boundaries covered). This unblocked the #334 measurement dependency; #334 is now complete with neutral ablation result.

**2026-05-18 intent/north-star retirement:** #358 is the binding correction: ContextGraph / ME-JEPA-Code must not model, classify, synthesize, infer, or embed private human intention. TASK-EK-010 / #332 and TASK-EK-011 / #333 are retired; #359 removes the runtime north-star surfaces, #360 removes live-session goal-vector rows, and #361 cleans this task ledger. Future EK work must target measurable code-reality signals, oracle outcomes, prompt hashes, session context, or operator-approved objectives without treating them as hidden intent.

**2026-05-18 TASK-EK-003 / #324 mincut-panel completion:** `CF_MEJEPA_MINCUT_REPORTS` now persists `MincutReport` rows for structural-hole panels over inline weighted graphs, DDA embedder-similarity rows, TCT constellation centroids, and failure-fingerprint co-similarity graphs. The core API `mejepa_mincut_panel`, MCP tool `mcp__cgreality__mejepa_mincut_panel`, and CLI `context-graph-cli mejepa mincut-panel` all use the same sync-write/readback path and invoke no inner LLM. FSV `/mnt/d/contextgraph/fsv/task-ek-003-mincut-panel-fsv/run-1779071998864/mincut_panel_fsv.json` sha256 `96dd719a55ec5024a48da69a6598d12cd92ec395a96362245c3ea023c87bef30` (`all_passed=true`, Stoer-Wagner Fig. 1 cut `4.0`, DDA/TCT/fingerprint graph sources, reopened CF count `6`, all-zero degenerate warning, disconnected/two-node/self-loop edge cases).

**2026-05-18 TASK-EK-002 / #323 instrument-proposal completion:** `CF_MEJEPA_INSTRUMENT_PROPOSALS` now stores candidate frozen-instrument proposals and promotion decisions. `context_graph_mejepa::instrument_proposal` composes Unknown-fingerprint active-learning clusters, pairwise-MI signatures, and latest mincut structural-hole partitions into ranked residual proposals with default `K_min=30`; MCP exposes `mcp__cgreality__mejepa_propose_instrument` and `mcp__cgreality__mejepa_promote_instrument_proposal` without invoking an inner LLM. FSV `/mnt/d/contextgraph/fsv/task-ek-002-propose-instrument-fsv/propose_instrument_fsv.json` sha256 `a9a5c08f0b3df1bc875050688c9b59dffc7d01f96c15589121d6ed6f03b8b4a3` (`passes=true`, residual cosine `1.0`, reopened proposal CF rows `3`, accepted + rejected_by_falsification decisions, all five edge cases).

**2026-05-18 TASK-EK-006 / #327 curiosity reward completion:** active-learning entries now carry durable `curiosity_score` values computed from the default compression-progress proxy `conformal_width x OOD score x per-cell support gap`. `context-graph-cli mejepa active-learning list --ranked-by curiosity` and `mcp__cgreality__mejepa_active_learning_queue` can return curiosity-ranked rows; `CF_MEJEPA_CURIOSITY_TELEMETRY` persists per-window distributions plus predicted-vs-actual CPΦ calibration evidence; weekly markdown renders `Curiosity Ranking`. FSV `/mnt/d/contextgraph/fsv/task-ek-006-curiosity-fsv/curiosity_fsv.json` sha256 `cef7b6e576875fd1df8d92f849c4e4adf032ac6d834ac967e94b6eaddb30646f` (`passes=true`, known expected-delta cases within 5%, curiosity top-rank readback, label calibration within tolerance, zero-width score `0.0`, weekly section rendered).

**2026-05-18 TASK-PY-G-007 / #122 pairwise-MI substrate completion:** `CF_MEJEPA_PAIRWISE_MI` now persists full pairwise-MI audit rows keyed by `(corpus_shard_hash, embedder_pair, ts)`, and `mcp__cgreality__mejepa_audit_pairwise_mi` can write/read the CF alongside the CSV audit. The audit returns redundancy histograms, adaptive per-slot weights, and measured `effectiveSignalCount`; `TASK-EK-003` mincut can now read `PanelGraphSource::PairwiseMiMatrix` directly from that CF. Real aiwonder FSV `/mnt/d/contextgraph/fsv/task-py-g-007-pairwise-mi-fsv/run-1779073845986/pairwise_mi_fsv.json` sha256 `5147a8a8359106835c4d113c8959a183ba4ddc776fabbea8bd4cf28cea624dca` (`passed=true`, 12 active embedders, 66 persisted pair rows, 1,000 common forward-cache samples, byte-identical CF row readback, reopened matrix equality, `effectiveSignalCount=9.7171335`, mincut source rows `66`). Synthetic known-MI FSV also passed at `/mnt/d/contextgraph/fsv/task-py-g-007-pairwise-mi-fsv/run-1779073213592/pairwise_mi_fsv.json` (`13` slots, `78` rows, clone MI `1.0`, independent MI `0.0`).

**2026-05-18 TASK-PY-G-042 / #261 patch-similarity exemplar completion:** `CF_MEJEPA_PATCH_SIMILARITY_META` now stores the active patch-similarity index metadata. `context_graph_mejepa::patch_similarity` materializes a schema-versioned usearch HNSW graph over L2-normalized `(E_AST, E_CFG, E_DataFlow, E_TypeGraph)` signatures, persists graph/record/meta files under `/mnt/d/contextgraph/storage/patch-similarity-index-v1/`, and `MeJepaCompiler` can populate `RealityPrediction.closest_exemplars` when the index dir and expected corpus snapshot hash are configured. FSV `/mnt/d/contextgraph/fsv/task-py-g-042-patch-similarity-exemplar-fsv/run-1779075407903/patch_similarity_exemplar_fsv.json` sha256 `177cc3a7310b4ad1c496ab4ed74b812e6895988d0e9fbd9947698df9ee8b3287` (`status=pass`, row count `40`, vector dim `1152`, metadata CF readback and reopened readback true, live prediction `closest_exemplars_count=5`, holdout top-1 same-failure-mode accuracy `1.0`, zero-norm/stale-hash/no-neighborhood/k>row-count edges covered).

**2026-05-18 TASK-PY-G-041 / #260 claim-graph extractor completion:** `CF_MEJEPA_CLAIM_RECONCILIATION` now stores schema-versioned Q1 claim-reconciliation rows keyed by `prediction_id x claim_id`. `context_graph_mejepa::claim_graph` deterministically extracts Added/Modified/Removed/Renamed/Fixed/Tested/WillPass/WillFail claims from agent prose, resolves file/symbol/line hints including qualified Python tests, validates reachable `repo_after_sha`, reconciles Python targets against bytes-on-disk plus AST chunks, fails closed on unsupported/non-UTF8/non-Python targets, and sync-writes JSON record bytes with flush/readback/reopen proof. FSV `/mnt/d/contextgraph/fsv/task-py-g-041-claim-graph-extractor-fsv/run-1779077268576/claim_graph_extractor_fsv.json` sha256 `ff5ade6a7b410e2d46a48eca245b9514239b3eae28de65c7578cf49536c37cb9` (`status=pass`, `50/50` real responses with claims, Added/Modified match rate `1.0`, CF rows `0 -> 13 -> 13`, all required edge cases passed). The artifact records an honest source gap tracked by #338: `runtime/agent-history` had 4 injection JSONL files but `0` agent-response rows, so the sampled real responses came from durable test-outcome comment payloads plus GitHub issue comments.

**2026-05-18 TASK-PY-G-043 / #262 per-head DDA projection completion:** `context_graph_mejepa::head_projection` now implements the `04_DDA_TCT §3` head-specific DDA slice table, fail-closed `HEAD_PROJECTION_MISSING_SLICE` / `HEAD_PROJECTION_SCHEMA_MISMATCH` / `HEAD_PROJECTION_NO_DDA` errors, and durable schema rows in `CF_MEJEPA_HEAD_PROJECTION_REGISTRY`. FSV `/mnt/d/contextgraph/fsv/task-py-g-043-per-head-dda-projection-fsv/run-1779078323947/head_projection_fsv.json` sha256 `dff41ba10cffa744eaa7921e78b359ca4f022fd5945b0b8cc18b7ee639201784` (`all_passed=true`, 30 representative Python chunk DDA rows read from `CF_MEJEPA_DDA_SIGNALS`, Oracle head `3` scalars/chunk, Failure-mode head `5` scalars/chunk, Reasoning head `2` pairwise scalars/chunk, `10` registry rows reopened).

**2026-05-18 TASK-PY-G-073 / #292 predicted-works completion:** `RealityPrediction` now has additive Q2 positive-evidence fields `predicted_works`, `predicted_uncovered_paths`, and `predicted_flaky_tests`. `context_graph_mejepa::prediction_surfaces` emits deterministic source-backed works/uncovered/flaky candidates from observable patch/test/exemplar signals only; `Verdict::Pass` requires zero `predicted_failed_tests` and at least one high-confidence `PredictedWorks`; MCP `mejepa_explain_prediction` exposes the new fields. FSV `/mnt/d/contextgraph/fsv/task-py-g-073-predicted-works-fsv/run-1779103251424/predicted_works_fsv.json` sha256 `e5e2a64e3d880071387a08edb407914ff0006ec4f21ea0263196e1a5b546e4bb` (`allPassed=true`, 6 `CF_MEJEPA_LIVE_PREDICTIONS` rows reopened, KnownGood binary search work claim has function name + 3 supporting embedders + 1 KnownGood exemplar, broken/spec-violating cases cannot Pass, neutral whitespace still emits works, uncovered/flaky subheads fire, per-head Brier scores <= `0.05`).

**2026-05-18 TASK-PY-G-059 / #278 secret-redaction completion:** `scripts/swebench_lite_ops/python_ast_chunks.py` now emits schema-v2 chunks with raw `source_text` retained for provenance and a separate redacted view for embedding. `python_forward_cache_fsv.rs` consumes `redacted_source_text` for embedder input, input hashes, cache keys, quarantine sizing, manifest summaries, and packed-record metadata; legacy chunks without a redacted view remain marked `legacy_no_redacted_view`. `CF_MEJEPA_REDACTION_LOG` is registered and receives sync-written/readback event rows plus clean zero-count rows. FSVs: `/mnt/d/contextgraph/fsv/task-py-g-059-secret-redaction-fsv/python_secret_redaction_fsv.json` sha256 `f008b01248321b8b59f124c1b71debe0768e62c5814207dac11fe6418b7e9c18`; `/mnt/d/contextgraph/fsv/task-py-g-059-secret-redaction-fsv/secret_redaction_forward_runner_fsv.json` sha256 `d2830fc7097ea9e5963efd10e23f6c1314d4152953d0834cd2fb03abd68bb448`.

**2026-05-18 TASK-PY-G-062 / #281 project-Merkle-cache completion:** `context_graph_mejepa::project_cache` now ships the reality-compiler cache substrate: D-root SQLite `cache.db`, BLAKE3 file rows, chunk/path de-duplication, deterministic embedding rows with embedder-version invalidation, lazy import-dependent rescan queueing, source/test/doc/config role tagging, `.gitignore` honoring through `git ls-files --exclude-standard`, `watcher.status`, and a 250 ms debounced scan entry point. FSV `/mnt/d/contextgraph/fsv/task-py-g-062-merkle-cache-fsv/project_merkle_cache_fsv.json` sha256 `0cf1522d3d5acecc1dad2d9ef0f85fd94d59cc65cdba7981d8eb01a9d445e16d` (`all_passed=true`, SQLite integrity `ok`, empty repo, one-file edit, delete, 1000-file batch, same-content move de-dupe, E7-only version bump). The original non-git fail-closed boundary is superseded by #282 for operator project-ingest source paths.

**2026-05-18 TASK-PY-G-063 / #282 project-ingestion MCP completion:** `mcp__cgreality__mejepa_project_ingest` now ingests permitted project directory trees or direct project files, using git enumeration when available and recursive supported-file discovery otherwise. It materializes `/mnt/d/contextgraph/projects/<project_id>/{cache,panels,predictions,report}`, routes `.py` through the Python AST chunker, excludes `test_*` / `*_test*` from `SourceOnly`, includes tests as `role:Test` for `SourceAndTests`, removes the public `dbPath` override, and persists per-file `RealityPrediction` rows in the project-local `CF_MEJEPA_LIVE_PREDICTIONS` with deterministic physical keys. FSV `/mnt/d/contextgraph/fsv/task-py-g-063-project-ingest-fsv/run-20260518T133058Z/project_ingest_fsv.json` sha256 `7514bfb3c49b3a490761fb08f817ce7b9594209f0675e3dbe28e6d90381446b8` (`allPassed=true`, MCP canonical+alias dispatch, 3 happy rows with physical key readback, empty repo, unchanged incremental `0` writes, 5001-file batch, mixed encoding/binary fail-closed, scope switch, non-git directory, direct file).

**2026-05-17 TASK-PY-G-069 / #288 prediction-verification completion:** the Claude Code PostToolUse Bash capture loop now binds test outcomes back to live ME-JEPA predictions. `scripts/cc/parse_test_output.py` covers pytest, cargo test, unittest, jest, and vitest; `scripts/cc/hook_post_tool_use_test_capture.sh` writes durable `runtime/test-outcomes/<session>.jsonl` evidence, appends `/mnt/d/contextgraph/runtime/posttooluse-mejepa-capture.jsonl`, fails closed with `MEJEPA_VERIFICATION_PARSE_FAILURE`, emits `MEJEPA_HOOK_TIMEOUT`, and calls `context-graph-cli mejepa bind-test-outcomes`. The CLI writes flushed/read-back rows to `CF_MEJEPA_PREDICTION_VERIFICATIONS` and sends Refuted predictions into `CF_MEJEPA_ACTIVE_LEARNING_QUEUE` at `score=6.0`. FSV `/mnt/d/contextgraph/fsv/task-py-g-069-posttooluse-capture-fsv/run-1779062220556-12961/posttooluse_capture_fsv.json` sha256 `cc35795fcd4e722f7f116f6176ca65d2b92bf5921e3e37137000618b6aa36e9c` (`all_passed=true`, confirmed/refuted/inconclusive rows, reopened CF readback, non-test no-write, 6x queue weight).

**2026-05-17 TASK-PY-G-070 / #289 Stop self-verification completion:** `.claude/settings.json` now runs `scripts/cc/stop-mejepa-self-verify.sh` on Stop before the existing CCreality recommendation gate. The hook calls `context-graph-cli mejepa stop-self-verify`, reads `CF_MEJEPA_LIVE_PREDICTIONS` for the current session, ignores `Verdict::Abstain`, treats only Confirmed/Refuted rows in `CF_MEJEPA_PREDICTION_VERIFICATIONS` as verified, and exits 2 with structured stderr listing every unverified `prediction_id` plus the top-10 recommended Bash commands. It honors `stop_hook_active`, operator disable via `~/.config/mejepa/daemon.toml::stop_hook_enabled = false`, and records daemon/CLI-unreachable cases as `MEJEPA_STOP_HOOK_DAEMON_UNREACHABLE` without blocking on infrastructure failure. Hook log SoT: `/mnt/d/contextgraph/runtime/stop-mejepa-self-verify.jsonl`. FSV `/mnt/d/contextgraph/fsv/task-py-g-070-stop-self-verify-fsv/run-1779063364468-91155/stop_self_verify_fsv.json` sha256 `70e1ab51c6f2a8dac0ad7d5a5781ccc23f529decd2b87db1a9a6a9cb7caf30d0` (`all_passed=true`, blocked before test, allowed after #288 verification row, all-Abstain allow, 50 unverified predictions listed, command list truncated 50 to 10).

**2026-05-18 #337 observe missing-prediction quarantine completion:** `SchedulerState::tick_observe` now treats a hard-labeled test outcome with no matching live prediction as a durable signal-drop event instead of a daemon-health error. Matched outcomes write/read back both `CF_MEJEPA_ORACLE_VERDICTS` and `CF_MEJEPA_PREDICTION_VERIFICATIONS`; no-prediction outcomes write/read back `CF_MEJEPA_SIGNAL_DROP_LOG` with `MEJEPA_TEST_OUTCOME_NO_PREDICTION`; skipped outcomes remain non-hard-label skips; malformed events still fail closed. FSV `/mnt/d/contextgraph/fsv/task-py-g-337-observe-missing-prediction-fsv/run-1779067066572/observe_missing_prediction_fsv.json` sha256 `66427f6578dca766aef94b8f76d90c7418b388f2d70a1c747e667b3ffd99052d`.

**2026-05-15 Python data-foundation gate:** the post-#249 Python AST chunk source of truth is `/mnt/d/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl`, sha256 `c8117f15e6c7a8b6e8849298557ce7cbcb3cfeb2dabf252c0cb16edc02480400`, with `399777` chunks across all `2400` task/category rows. The 1,000-chunk smoke field has DDA, TCT, pairwise, readiness, Q1-Q5 tensor artifacts, end-to-end sample evidence, per-embedder Q1-Q5 contribution artifacts, and a TCT completeness audit, so it may feed smoke-dry-run ME-JEPA training. The smoke intelligence-matrix SoT is `/mnt/d/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/intelligence-matrices/summary.json` with FSV `/mnt/d/contextgraph/fsv/task-py-data-062-post249-smoke-intelligence-fsv/smoke_intelligence_matrices_fsv.json` (`passes=true`): 66 valid pairwise rows, 696 stratified coverage rows, 330 estimator-validity rows, and three observed coverage tiers (`0.886`, `0.988`, `0.991`). The contribution SoT is `/mnt/d/contextgraph/corpus/python-embedder-intelligence-v1/contribution/` with FSV `/mnt/d/contextgraph/fsv/task-py-data-055-embedder-contribution-fsv/python_embedder_contribution_fsv.json` (`passes=true`): 60 head/embedder rows, 840 feature metrics, 34 shared-credit pairs, 96 stratified rows, and explicit edge states proving too-few labels fail closed, redundant embedders share credit, missing/quarantined rows stay masked, and Q1 is not overclaimed while all smoke existence labels are true. The TCT completeness SoT is `/mnt/d/contextgraph/corpus/python-tct-completeness-v1/` with FSV `/mnt/d/contextgraph/fsv/task-py-g-039-tct-completeness-fsv/python_tct_completeness_fsv.json` (`passes=true`): 8 category rows, 93 cell rows, 8 reward-dimension rows, 123 missing-signal rows, and 5 edge cases. It proves all smoke memberships and embedder pairs are accounted, while also preserving the non-green truth that sparse/fail-closed cells still block production/full-corpus training. The proof-domain isolation SoT is `/mnt/d/contextgraph/corpus/python-proof-domain-isolation-v1/proof_domain_isolation_report.json` with FSV `/mnt/d/contextgraph/fsv/task-py-g-037-proof-domain-isolation-fsv/python_proof_domain_isolation_fsv.json` (`passes=true`): 0 forbidden repo rows, 0 forbidden open-issue rows, and 6 edge cases covering empty active scope, misleading prerequisite language, archived references, current-guard issue text, multiline exclusion text, and GitHub issue batch limits. The full-corpus forward cache is now superseded by the 2026-05-17 aiwonder completion below; first end-to-end reward-field sample evidence is tracked by #211 and `/mnt/d/contextgraph/fsv/task-py-data-028-reward-field-sample-fsv/python_reward_field_sample_fsv.json`.

**2026-05-17 aiwonder forward-cache completion:** the production 12-embedder forward-cache substrate is complete at `/zfs/archive/contextgraph/corpus/python-forward-cache-v1-aiwonder-prod-20260515-191116Z/`. Direct aiwonder readback shows `python_forward_cache_checkpoint.json` sha256 `4c14119eb152eb21f861d7841d5d5f5b8210c6282f60cf1c5d3e057b44878e42`, `status=ready_for_final_aggregate`, `phase=final`, `passes=true`; `python_forward_cache_corpus_manifest.json` sha256 `d8b3e0b9d0af2681ff794dbcb51e8515d8b00be55b2b61d1da89961d478ea2ca`, `status=passed`, `passes=true`; `391/391` shard dirs; `4,797,324/4,797,324` accounted pairs; `4,665,772` cache records; `131,552` quarantine rows; `154,110` batch-telemetry rows; and `12` active embedders. Issue #199 is closed; #313's shard-300 parallel-isolated hot-stage race is fixed and closed with FSV sha256 `900630890d588b5edb3719bf03786cc4d7fc36d0c3af8fd2564a123740fb9b16`. TASK-FP-001 typed catalog storage is complete under #309 with FSV `/mnt/d/contextgraph/fsv/phase-g-failure-fingerprint-fsv/run-1779018842783/failure_fingerprint_fsv.json` sha256 `916d8650e62bb3acdf9464468d0ada3d58804b495f13d9a741e3718ded84cecd`. TASK-FP-002 production materialization is complete at `/zfs/archive/contextgraph/corpus/python-failure-fingerprints-seed-v1/`; TASK-FP-003 production tau calibration is complete at `/zfs/archive/contextgraph/corpus/python-failure-fingerprints-calibrated-v1/` with FSV `/zfs/archive/contextgraph/fsv/task-fp-003-fingerprint-tau-calibration-fsv/production-20260517T132403Z/fingerprint_tau_calibration_fsv.json` sha256 `1c7f98c2840484cecd391415d6af8f63cfa374efa913fd5791abf6f3b6e0ff21` (`all_passed=true`): 97 calibrated fingerprints, 97 calibration records, 4,665,772 joined records, 7,413,631 calibrated vector observations, tau range `0.049051024765955006..1.0`, and all tau sets in range. TASK-FP-004 live classifier integration is code/FSV-shipped with `/mnt/d/contextgraph/fsv/phase-g-fingerprint-classifier-fsv/run-1779025535586/fingerprint_classifier_fsv.json` sha256 `f966fc4ccd81c32ed0d09517d553e38bcc93cf586244258e8b33ab83d7fda99c` (`all_passed=true`). TASK-FP-005 Unknown/OOD active-learning queueing is code/FSV-shipped with `/mnt/d/contextgraph/fsv/phase-g-unknown-fingerprint-active-learning-fsv/run-1779026451760/unknown_fingerprint_active_learning_fsv.json` sha256 `4d9c75d531d0e597bfa7538d0ff1b972c81c68613cf57ffc7f3c410b398249a6` (`all_passed=true`). TASK-FP-006 MCP catalog tools are code/FSV-shipped with `/mnt/d/contextgraph/fsv/phase-g-fingerprint-mcp-tools-fsv/run-1779028452513/fingerprint_mcp_tools_fsv.json` sha256 `49d599417b0b3c09a86e85696f9cb34136018bd71ff791d630aa87712d3ee98b` (`all_passed=true`): 7 MCP tools, 4 fail-closed edge cases, and reopened RocksDB catalog readback count 2. TASK-FP-007 four-window per-fingerprint ship-gate metrics are code/FSV-shipped with `CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS`, MCP combined ship-gate fields, and FSV `/mnt/d/contextgraph/fsv/phase-g-fingerprint-ship-gate-fsv/run-20260517T145447Z/fingerprint_ship_gate_fsv.json` sha256 `aee10774fd437ab1a1e266464934b244ff9a822999f87d5e140d8a117bef6501` (`all_passed=true`). TASK-FP-008 core stability source-of-truth is code/FSV-shipped with `CF_MEJEPA_FINGERPRINT_FISHER`, `CF_MEJEPA_FINGERPRINT_DORMANCY`, `FingerprintEwcPenalty`, `FingerprintCbpReset`, `MEJEPA_FINGERPRINT_FISHER_MISSING`, and `mejepa_fingerprint_catalog_stability`; FSV `/mnt/d/contextgraph/fsv/task-fp-008-fingerprint-stability-fsv/run-20260517T1538Z/fingerprint_catalog_stability_fsv.json` sha256 `02f08c7d91ab7130dea3119bd43f4186a2627f8521ea4196b74efc69d6fce5ec` (`all_passed=true`). Deeper predictor/scheduler invocation is future hardening outside the closed #309/#317 acceptance gate.

**2026-05-17 TASK-FP-010 live evidence completion:** `RealityPrediction` now persists `matched_fingerprint` and `unknown_candidate_id`; `MeJepaCompiler::compile_with_fingerprint_observation` attaches matched catalog evidence for production forward-cache observations; compatible panel-slot catalogs attach during normal compile; and MCP `mejepa_predict_latest` / `mejepa_explain_prediction` expose the evidence plus optional fingerprint references. FSV `/mnt/d/contextgraph/fsv/task-fp-010-matched-fingerprint-live-fsv/run-1779033429758/matched_fingerprint_live_fsv.json` sha256 `9702f487c5b1f23f3ebf24652b87997b009f64e692d9ce707df2312d8dac7898` (`all_passed=true`, 7 cases).

**2026-05-17 TASK-PY-G-060 / TASK-FP-009 adversarial completion:** `CF_MEJEPA_ADVERSARIAL_CORPUS` now stores 30 adversarial Python rows from `tests/fixtures/adversarial` and `/mnt/d/contextgraph/corpus/python-adversarial-v1/` has the copied patch artifacts. Corpus FSV `/mnt/d/contextgraph/fsv/task-py-g-060-adversarial-corpus-fsv/run-1779034913258/adversarial_corpus_cf_fsv.json` sha256 `a54960288c805dfa10cc5bcb2e74446bebf0be65c9f4ecf0bc496dec9a65ba21` (`all_passed=true`). TASK-FP-009 materializes 30 canonical adversarial KnownBad fingerprints with `source_corpus=python-adversarial-v1`, adds four passing fingerprint ship-gate windows, verifies 30/30 Fail classifications and per-kind recall thresholds, and extends `mejepa_fingerprint_list` with `sourceCorpus` filtering. FSV `/mnt/d/contextgraph/fsv/task-fp-009-adversarial-fingerprint-integration-fsv/run-1779034921824/adversarial_fingerprint_integration_fsv.json` sha256 `726fe43a2550c536ea3e813dd8f95cc2ae2acdca8b35892d7c5a3b86dcf4bbb5` (`all_passed=true`). TASK-FP-103 latent-action search is shipped with FSV `/mnt/d/contextgraph/fsv/task-fp-103-latent-search-fsv/run-1779038591152/latent_search_fsv.json` sha256 `f31de8c1d4a0035731170e4a24640dc04d16095c2525ceacde84e7b0c8167e4e`; TASK-FP-104 objective+safety is shipped with FSV `/mnt/d/contextgraph/fsv/task-fp-104-objective-safety-fsv/run-1779040194889/objective_safety_fsv.json` sha256 `31cda8fa8c7bb0ce980947786b42d104f6ea1d2cde42a6bef88d6cc1aff288a1`; TASK-FP-105 counterfactual ranking is shipped with FSV `/mnt/d/contextgraph/fsv/task-fp-105-rank-candidates-fsv/run-1779041036578/rank_candidates_fsv.json` sha256 `9502a7a753df459eabe671c2d8e293b6f5de9a1be5d652a18ea9adde206c5dea`; and Pillar 9 System 1/System 2 framing is documented in the Five Questions/product-boundary docs plus MCP tool descriptions. Under #315, TASK-RWD-205 call-site type graph is shipped with `CF_MEJEPA_TYPE_GRAPH_SIGNALS` and FSV `/mnt/d/contextgraph/fsv/task-rwd-205-type-graph-fsv/run-1779054424421/type_graph_fsv.json` sha256 `8accd4429949c27910ee1132ba5bc9d3f41aecbe709322596846e01a698ae0f0`; TASK-RWD-206 runtime telemetry is shipped with `CF_MEJEPA_TELEMETRY_SIGNALS` and FSV `/mnt/d/contextgraph/fsv/task-rwd-206-telemetry-fsv/run-1779055248001/telemetry_fsv.json` sha256 `9006bdcc1e5376e39e2ecddccc8c3b78061f2bb18eee7b9956a196694a753016`; TASK-RWD-207 live session is shipped with `CF_MEJEPA_LIVE_SESSION_SIGNALS` and FSV `/mnt/d/contextgraph/fsv/task-rwd-207-live-session-fsv/run-1779055666755/live_session_fsv.json` sha256 `5303b57211152666d147e662fb708f5b5d37642f38e7036b73f442341af66516`; TASK-RWD-210 prioritized replay buffer is shipped with `CF_MEJEPA_REPLAY_BUFFER`, `BatchSampler::next_replay_batch`, k_replay windows, retention/quota policy, and FSV `/mnt/d/contextgraph/fsv/task-py-g-068-replay-buffer-fsv/run-1779049993557-3834881/replay_buffer_fsv.json` sha256 `cf4f4a68080b127d7af07e5ae66c85d99c585fd999442063fc256f15d0689b77`; and final reward-audit acceptance is shipped with `mcp__cgreality__mejepa_audit_reward_signals`, `CF_MEJEPA_SIGNAL_DROP_LOG`, and FSV `/mnt/d/contextgraph/fsv/task-rwd-315-reward-audit-fsv/run-1779062502366-35012/reward_signal_audit_fsv.json` sha256 `c271752d7177a4bf40588fda0c8eedef53f404f0e68155e801db8a57da4467e8` (`all_passed=true`, 8/8 tiers green, boundary cases fail closed). Next ordered critical path: model-quality convergence against the `0.95` holdout ship gate.

---

## Phase A — Operational Unblock (12 substrate tasks + moved cleanup/observability)

| ID | Title | Phase | Blocked by | Effort |
|---|---|---|---|---|
| `TASK-FLYWHEEL-001` | MCP daemon mode + supervised tokio tasks + systemd template + D-drive validator | A | none | complete at code/FSV level; fresh-host enablement proof remains |
| `TASK-FLYWHEEL-002` | Self-optimization scheduler skeleton (7 tickers calling shipped APIs) | A | FLYWHEEL-001 | complete |
| `TASK-FLYWHEEL-003` | Test-outcome capture hook (parse pytest/cargo-test/jest) | A | none | **complete (2026-05-12)** |
| `TASK-FLYWHEEL-004` | Captured test-outcome oracle binding | A | FLYWHEEL-003 | **complete (2026-05-12 code path)** |
| `TASK-FLYWHEEL-005` | `ActiveLearningKind::AgentSurprise` variant + queue extension | A | none | **complete (2026-05-11)** |
| `TASK-FLYWHEEL-006` | New CFs: `CF_MEJEPA_DDA_SIGNALS`, `CF_MEJEPA_FAILURE_EXEMPLARS`, `CF_MEJEPA_AGENT_FEEDBACK` | A | none | **complete (2026-05-11)** |
| `TASK-FLYWHEEL-010` | Unified supervised-task health registry powering `/ready` | F | FLYWHEEL-001, OBS-014 | **complete (#87)** — `supervised_task_health.json` records heartbeat/restart health for all daemon tasks; `/ready` aggregates it |
| `TASK-PREDICT-001` | UserPromptSubmit prediction-injection hook | A | FLYWHEEL-001 | **complete (2026-05-12; #274 hardening FSV 2026-05-17)** — ≤4 KB renderer, agent-history source-of-truth, cold-start block, adversarial escaping, Abstain reason, and fail-closed MCP/malformed behavior |
| `TASK-PREDICT-002` | `mejepa_record_agent_feedback` MCP tool + sampling-weight bump wiring | A | FLYWHEEL-005, FLYWHEEL-006 | **complete (2026-05-12; no feedback rate limit)** |
| `TASK-LOOP-001` | Apply new `.claude/loop.md` content from `14_NEW_LOOP_MD.md` | A | FLYWHEEL-001 | **complete (already applied)** |
| `TASK-LOOP-002` | Update CLAUDE.md §11.11/§13/§3 + agent README retirements | H | LOOP-001 | moved to Phase H residue cleanup; not an A-D ME-JEPA substrate blocker |
| `TASK-LOOP-007` | Hook output paths → D-drive `cgreality-shift-log/` | A | none | **complete (2026-05-11 — code already shipped, audit doc updated)** |
| `TASK-OBS-005` | C-drive lint in CI | A | none | **complete (2026-05-12)** |
| `TASK-OBS-006` | FSV path migration sweep | A | none | **complete (2026-05-12)** |
| `TASK-OBS-007` | Per-slice quota observability extension | F | none | **complete** — `mejepa_daemon_status` and `mejepa_quota_status` expose quota slices; storage-exhaustion/session-cleanup FSV read back quota state |

**Phase A substrate status:** complete for the ME-JEPA Phase E/G handoff. The moved Phase H residue rows are now closed.

---

## Phase B — Multi-Head Extension (core tracked tasks)

| ID | Title | Phase | Blocked by | Effort |
|---|---|---|---|---|
| `TASK-PREDICT-003` | `mejepa_predict_what_if` MCP tool | B | PREDICT-005 | **complete (2026-05-12)** |
| `TASK-PREDICT-004` | `mejepa_explain_prediction` MCP tool | B | PREDICT-005 | **complete (2026-05-12)** |
| `TASK-PREDICT-005` | Multi-head stack (8 new heads + per-head feature projections) | B | none | **code_complete (2026-05-12; DDA feature hardening remains Phase D)** |
| `TASK-PREDICT-006` | Rich `RealityPrediction` schema | B | PREDICT-005 | **complete (2026-05-12)** |
| `TASK-PREDICT-007` | Agent claim graph extractor (deterministic) | B | none | **complete (2026-05-12; TASK-PY-G-041/#260 persistence/reconciliation hardening complete 2026-05-18)** |
| `TASK-PREDICT-008` | Reality-impact reconciler (Q5) | B | PREDICT-007 | **complete (2026-05-12)** |
| `TASK-PREDICT-LABEL-001` | Failure-mode label source (pytest traceback parser → FailureModeClass) | B | CORPUS-002 | **code_complete (2026-05-12 unified deterministic extractor)** |
| `TASK-PREDICT-LABEL-002` | Edge-case label source (coverage delta + symbolic-execution proxy + per-language pattern catalog) | B | none | **partial/code_complete: deterministic text extractor; coverage/symbolic hardening open** |
| `TASK-PREDICT-LABEL-003` | Tech-debt label source (clippy/pylint/eslint warning delta + radon/lizard) | B | none | **partial/code_complete: deterministic text extractor; analyzer deltas open** |
| `TASK-PREDICT-LABEL-004` | Dead-code label source (vulture/udeps/eslint-no-unused) | B | none | **partial/code_complete: taxonomy/schema exists; analyzer deltas open** |
| `TASK-PREDICT-LABEL-005` | Perf label source (pytest-benchmark / cargo bench / massif delta extractors) | B | none | **partial/code_complete: patch-surface heuristic; benchmark parsers open** |
| `TASK-PREDICT-LABEL-006` | Security label source (bandit/cargo-audit/eslint-security/semgrep delta) | B | none | **partial/code_complete: Python Bandit/Semgrep deltas shipped via TASK-RWD-204; non-Python SAST/CVE deltas deferred** |
| `TASK-PREDICT-LABEL-007` | Accuracy label source (test-output metric regex extractor) | B | none | **partial/code_complete: metric keyword extractor; full regex catalog open** |
| `TASK-PREDICT-LABEL-008` | Cost label source (CI minute counter + token telemetry + dep-count delta) | B | none | **partial/code_complete: dependency-count delta; CI/token telemetry open** |
| `TASK-PREDICT-LABEL-009` | Reasoning-class weak supervisor (E_Reasoning ⟂ E_Diff agreement) | B | PREDICT-005 | **partial/code_complete: field/schema present; embedder-agreement supervisor open** |
| `TASK-PREDICT-LABEL-010` | Toolchain availability fail-closed audit | F | none | **complete (#61 + startup gate follow-up)** |
| `TASK-PREDICT-LABEL-011` | Cross-language label gating | F | PREDICT-LABEL-001..009 | **complete (#62)** |
| `TASK-PREDICT-009` | `ActiveLearningKind::AgentSurprise` integration (consumer side) | B | FLYWHEEL-005 | **complete (2026-05-11/12)** |
| `TASK-UTML-001` | Per-head ΔP/ΔK/ΔΞ tracking | B | PREDICT-005 | **complete/code_complete (2026-05-12)** |
| `TASK-UTML-002` | Per-head cert columns in `CF_MEJEPA_TRAIN_CERTS` | B | UTML-001 | **complete for Phase A-D (2026-05-12; cert readback + carrier fields FSV)** |
| `TASK-UTML-003` | `ΔΞ_global = min_h(ΔΞ_h)` aggregation | B | UTML-001 | **complete for persisted learning-signal carrier; BatchSampler policy belongs to Phase E** |
| `TASK-UTML-004` | `agent_surprise_severity_score` factor in sampling_weight | B | PREDICT-002 | **complete** |
| `TASK-UTML-005` | Per-head Fisher matrices | E | UTML-001 | **complete/code+FSV (2026-05-13)** |
| `TASK-UTML-006` | Per-head dormancy detection | E | UTML-001 | **complete/code+FSV (2026-05-13)** |
| `TASK-UTML-007` | Per-head loss coefficient λ ramp in heal scheduler | E | HEAL-001, UTML-001 | **complete/code+FSV (2026-05-13)** |
| `TASK-UTML-008` | Per-head dissipation detection | E | UTML-001 | **complete/code+FSV (2026-05-13)** |
| `TASK-UTML-009` | Per-cell state-transfer T in eval reports | F | UTML-001 | **complete (#63)** — weekly eval reports persist per-cell state-transfer diagnostics; FSV at `/mnt/d/contextgraph/fsv/utml-009-fsv/per_cell_state_transfer_fsv.json` |

**Phase B/E/F code/FSV status (2026-05-13):** primary prediction/API/schema/label/UTML carrier work is complete with FSV at `/mnt/d/contextgraph/fsv/contextgraph-phase-b-fsv/phase-b-prediction-fsv.json` and `/mnt/d/contextgraph/fsv/contextgraph-utml-fsv/utml-fsv-summary.json`; Phase E completes Fisher, dormancy, lambda ramp, and dissipation with evidence in `/mnt/d/contextgraph/fsv/mejepa-phase-e-fsv/phase_e_self_optimization_fsv.json`; Phase F completes per-cell state-transfer reporting with evidence in `/mnt/d/contextgraph/fsv/utml-009-fsv/per_cell_state_transfer_fsv.json`. Remaining quality work belongs to Phase G convergence: full analyzer/toolchain label deltas and holdout metrics.

---

## Phase C — Corpus Completion (4 tasks, ops-heavy)

| ID | Title | Phase | Blocked by | Effort |
|---|---|---|---|---|
| `TASK-CORPUS-001` | Per-language mutation operators (10 langs × 8 = 80) | C | none | **complete (2026-05-12; real-toolchain verified)** |
| `TASK-CORPUS-002` | 300×8 SWE-bench Lite Python corpus generation under `/mnt/d/contextgraph/corpus/` (Docker) | C | none | **complete (2400/2400 verified, 2026-05-12)** |
| `TASK-CORPUS-003` | Per-language fixture corpora (64 tasks per language × 8 categories) | C | CORPUS-001 | **complete (5120/5120 verified, 2026-05-12)** |
| `TASK-CORPUS-004` | Combined split applied to Python + archived non-Python fixture corpora | C | CORPUS-002, CORPUS-003 | **complete (split FSV passed, 2026-05-12); active ship gate consumes Python only** |

**Phase C status:** complete. Sources of truth: Python corpus `/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/`, archived non-Python fixture corpus `/mnt/d/contextgraph/corpus/phase-c-non-python-fixtures-v1-20260512/`, combined evidence `/mnt/d/contextgraph/fsv/corpus-fsv/split_full.json`. Phase G uses the Python corpus as the active ship corpus.

---

## Phase D — Neural Embedder Forwards (7 tasks)

| ID | Title | Phase | Blocked by | Effort |
|---|---|---|---|---|
| `TASK-ENRICH-001` | `EmbedderForward` trait + active content forwards: pretrained E1/E6/E7/E8/E10/E12/E13/E14, algorithmic E2/E3/E4/E9 | D | none | **complete (2026-05-12; full pretrained/routing FSV passed)** |
| `TASK-ENRICH-002` | DDA signal computation per chunk with per-embedder cosine, strict upper-triangle pairwise cosine/MI, blind-spot z-scores | D | ENRICH-001, FLYWHEEL-006 | **complete (2026-05-12)** |
| `TASK-ENRICH-003` | Populate the new CFs with real signals | D | ENRICH-002 | **complete (2026-05-12; live algorithmic DDA rows FSV writes/readbacks 100 shifts)** |
| `TASK-ENRICH-004` | Embedder cache layer | D | ENRICH-001 | **complete (2026-05-12; persistent cache, corrupt-entry fail-closed, LRU/telemetry evidence closed in #17)** |
| `TASK-ENRICH-005` | VRAM budget gate enforcement at startup | D | ENRICH-001 | **complete for Phase A-D (production telemetry window + VRAM readback FSV)** |
| `TASK-ENRICH-006` | Per-embedder FSV (real model file, deterministic input → expected output) | D | ENRICH-001 | **complete (2026-05-12; 8 pretrained + 4 algorithmic + 143 routing rows read back)** |
| `TASK-ENRICH-007` | Per-language stdlib registry for `ReinventedWheel` | D | none | **complete (2026-05-12)** |

**Phase D/E status (updated 2026-05-13):** enrichment substrate is code/FSV-shipped for full pretrained/routing forwards, DDA compute/store, live shift-subscriber DDA rows, persistent cache/LRU telemetry, inference CF opening, TCT refresh support, production telemetry windows, analyzer-backed semantic heads, UTML carrier evidence, and stdlib registry. Autonomous Phase E now ships on top of that substrate. The non-green 0.95 prediction-oracle threshold remains Phase G convergence work.

---

## Phase E — Continuous Self-Optimization (code/FSV shipped 2026-05-13)

| ID | Title | Phase | Blocked by | Effort |
|---|---|---|---|---|
| `TASK-HEAL-001` | Self-optimization scheduler full implementation | E | Phase A-D substrate verified | **complete/code+FSV** |
| `TASK-HEAL-002` | Per-cell stratified drift detection | E | HEAL-001 | **complete/code+FSV** |
| `TASK-HEAL-003` | Dissipation detection | E | HEAL-001, UTML-008 | **complete/code+FSV** |
| `TASK-HEAL-004` | Bayesian small-window drift (Jeffreys/Beta posterior) | E | HEAL-002 | **complete/code+FSV** |
| `TASK-HEAL-005` | Production constellation refresh policy | E | HEAL-001 | **complete/code+FSV** |
| `TASK-HEAL-006` | `mejepa_promote_approval` MCP tool | E | HEAL-001 | **complete/code+FSV** |
| `TASK-HEAL-007` | Emergency eviction policy validation | E | none | **complete/code+FSV** |
| `TASK-HEAL-008` | Drift attribution and alert policy stream | E | HEAL-002 | **complete/code+FSV** |
| `TASK-UTML-008` | Per-head dissipation detection | E | UTML-001 | **complete/code+FSV** |

**Phase E FSV source of truth:** `/mnt/d/contextgraph/fsv/mejepa-phase-e-fsv/phase_e_self_optimization_fsv.json`, with durable readback from `CF_MEJEPA_MODEL_PROMOTIONS`, `CF_MEJEPA_HEAL_REPORTS`, `CF_MEJEPA_DRIFT_WINDOW`, and `self_optimization_status.json`.

---

## Phase F — Production Observability + Operator Runbook + DR + API (complete 2026-05-13)

> **Status (2026-05-13).** Phase F expanded well beyond the original 10 OBS tasks during multi-agent execution. The backlog spans CLI, observability, DR, API stability, security, integration testing, and follow-up consumer wiring. Source-of-truth tracker readback on 2026-05-13 shows **0 open Phase F implementation issues**: all 37 atomic Phase F issues plus all 15 `phase:f-followup` issues are closed with FSV evidence under `/mnt/d/contextgraph/fsv/`.

### Closed OBS tasks

| ID | Title | Status / Evidence |
|---|---|---|
| `TASK-OBS-001` | Weekly evaluation production feed/dashboard/runbook | **complete** — daemon weekly-eval scheduler runs; writer persists `CF_MEJEPA_EVAL_REPORTS` + `/mnt/d/contextgraph/exports/eval/<date>/weekly.{md,json}` |
| `TASK-OBS-002` | Pairwise MI heatmap renderer | **complete** — `mejepa_audit_pairwise_mi` MCP tool + CSV-to-PNG/markdown renderer |
| `TASK-OBS-003` | Daemon status aggregator MCP tool | **complete** — `mejepa_daemon_status` aggregates subscriber + heal + quota + VRAM |
| `TASK-OBS-004` | Per-prediction-class running calibration tracker | **complete** — `eval::per_head_calibration_tracker.rs` exposes rolling ECE per head |
| `TASK-OBS-007` | Prediction explanation surface completion (re-scoped) | **complete (#42)** — `mejepa_explain_prediction` + `mejepa_inspect_prediction` cover the full provenance trace |
| `TASK-OBS-008` | WSL GPU telemetry hardening | **complete** — `crates/context-graph-mcp/src/telemetry/gpu_wsl.rs` with retry + N/A detection |
| `TASK-OBS-009` | `mejepa_inspect_prediction` MCP tool | **complete (#44, commit 7f7e5c1d/e4baea50)** — full provenance trace including DDA chunks |
| `TASK-OBS-010` | Active-learning operator CLI | **complete (#45, commit 1f538ce1)** — `mejepa active-learning {list,label,dismiss}` — FSV sha256:a02788b1 |
| `TASK-OBS-011` | Prediction replay tool | **complete (#46)** — `mejepa replay --prediction-id <hex>` byte-for-byte replay |
| `TASK-OBS-012` | Reasoning-trace renderer | **complete (#47, commit 5662a744/a8474ace)** — `scripts/render_reasoning_trace.py` (HTML/markdown + saliency heatmap + counterfactuals) |
| `TASK-OBS-013` | System cost telemetry (resource counters, no spend caps) | **complete (#48, commit 0379df9c)** — `crates/context-graph-mejepa/src/system_cost.rs` tracks CUDA µs / tokens / RocksDB bytes; explicitly forbids `cost_estimate_usd` |
| `TASK-OBS-014` | Health + readiness HTTP probes | **complete (#49, commit 5aebf4ac/fcd3234f)** — `crates/context-graph-mcp/src/health_probe.rs`; opt-in via `CONTEXT_GRAPH_HEALTH_PROBE=1` |
| `TASK-OBS-015` | Log rotation policy | **complete (#50, commit 366d7e2a)** — `scripts/systemd/mejepa-log-rotate.{service,timer}` daily 03:15 |
| `TASK-OBS-016` | Executive summary renderer | **complete (#51, commit 5662a744)** — `scripts/render_executive_summary.py`; ≤200 words per render |

### Closed RUN (operator runbook) tasks

| ID | Title | Status / Evidence |
|---|---|---|
| `TASK-RUN-002` | Operator CLI: daemon command | **complete (#73, commit 4d801592)** — `mejepa daemon-status` reads `self_optimization_status.json` |
| `TASK-RUN-003` | Operator CLI: weekly report command | **complete (#74)** — `mejepa report-weekly --date <YYYY-MM-DD>` reads `CF_MEJEPA_EVAL_REPORTS` |
| `TASK-RUN-005` | Operator CLI: hygiene command | **complete (#75, commit 366d7e2a)** — `mejepa hygiene {cold-audit, aggressive-cold-tier-down}` |
| `TASK-RUN-006` | Operator CLI: rollback command | **complete (#76)** — `mejepa rollback --target-witness-offset <u64>` wraps `AbcPromoter::rollback_to` |
| `TASK-RUN-007` | Operator CLI: eval command | **complete (#77)** — `mejepa eval-ship-gate` reads latest EvalReport |
| `TASK-RUN-008` | Operator CLI: pause command | **complete (#78, commit baeef6b0)** — `mejepa pause --duration-mins <n>` + `mejepa_pause_predictions` MCP tool |
| `TASK-RUN-009` | Operator CLI: session cleanup command | **complete (#79, commit 0379df9c)** — `mejepa session-cleanup --session-id <32-hex>` |
| `TASK-RUN-012` | Incident response checklist verification | **complete (#80, commit a54803fa)** — `scripts/ci/runbook_coverage.sh` enforces `MEJEPA_*` alert ↔ code parity |

### Closed DR (disaster recovery) tasks

| ID | Title | Status / Evidence |
|---|---|---|
| `TASK-DR-003` | Storage verify CLI | **complete (#54, commit 9ccec509)** — `mejepa storage-verify --db-path <p>` with `set_paranoid_checks(true)` + per-CF row counts |
| `TASK-DR-004` | Storage restore-cf CLI | **complete (#55)** — `mejepa storage restore-cf` replaces target CF rows from a source DB with runbook FSV readback |
| `TASK-DR-007` | Restore verification monthly cron | **complete (#56, commit f036afac)** — `scripts/dr/verify_backup.sh` + `scripts/systemd/mejepa-dr-verify.{service,timer}` (first Sunday 04:00) |
| `TASK-DR-008` | Quarterly DR drill | **complete (#57, commit c710dfc4)** — `scripts/dr/dr_drill.sh` restore + corrupt SST + re-restore + RTO budget check |

### Closed API stability tasks

| ID | Title | Status / Evidence |
|---|---|---|
| `TASK-API-002` | Storage migrate-cf command | **complete (#58)** — `mejepa storage migrate-cf` copies rows from source to target CF with fail-closed non-empty target handling |
| `TASK-API-004` | Deprecation framework | **complete (#59, commit edf19c75)** — `crates/context-graph-mcp/src/deprecation.rs`; default warn / opt-in `CONTEXT_GRAPH_DEPRECATION_LEVEL=error` |

### Closed PREDICT + label + UTML tasks (Phase F hardening)

| ID | Title | Status / Evidence |
|---|---|---|
| `TASK-PREDICT-012` | Operator override prediction tool | **complete (#60, commit 377f0884)** — `operator_override.rs` + `CF_MEJEPA_OPERATOR_OVERRIDES` + `mejepa_operator_override_prediction` MCP tool; 6.0× sampling-weight multiplier persisted |
| `TASK-PREDICT-LABEL-010` | Toolchain availability fail-closed audit | **complete (#61, commit 0379df9c; updated 2026-05-17 for TASK-RWD-204/208)** — `crates/context-graph-mejepa/src/toolchain_detect.rs`; 19-binary default catalogue across 11 languages, including mypy/ruff/pyright/python3 for Python static-analysis/type/runtime labels plus Bandit/Semgrep for Q4 security labels |
| `TASK-PREDICT-LABEL-011` | Cross-language label gating | **complete (#62, commit 0379df9c)** — `crates/context-graph-mejepa/src/cross_language_label_gating.rs`; same-root-cause + similarity > 0.85 + correlation ≥ 0.60 ⇒ transfer at 0.5× weight |
| `TASK-PREDICT-LABEL-012` | Cross-language label gating consumer | **complete (#86)** — `label_transfer_audit` + `apply_phase_b_labels_with_transfer_gate()` + `CF_MEJEPA_LABEL_TRANSFER_DECISIONS`; weekly surfaces include accept/reject ratios per language pair |
| `TASK-UTML-009` | Per-cell state-transfer T in eval reports | **complete (#63)** — weekly eval reports carry per-cell state-transfer diagnostics; FSV sha256:63234e2d |

### Closed integration test tasks

| ID | Title | Status / Evidence |
|---|---|---|
| `TASK-TEST-001` | End-to-end happy-path harness | **complete (#64, commit 062d413d/a0e77b38)** — `crates/context-graph-mejepa/tests/e2e.rs` + `tests/e2e/{happy_path.rs, harness.rs}` |
| `TASK-TEST-002` | Restart-safety E2E harness | **complete (#65)** — daemon restart + watermark replay FSV at `/mnt/d/contextgraph/fsv/phase-f-restart-safety-fsv/` |
| `TASK-TEST-003` | Multi-agent E2E harness | **complete (#66)** — concurrent-session isolation FSV at `/mnt/d/contextgraph/fsv/phase-f-multi-agent-fsv/` |
| `TASK-TEST-004` | Catastrophic-event E2E harness | **complete (#67)** — catastrophic-event FSV at `/mnt/d/contextgraph/fsv/phase-f-catastrophic-event-fsv/` |
| `TASK-TEST-005` | Storage-exhaustion E2E harness | **complete (#52)** — storage-exhaustion FSV at `/mnt/d/contextgraph/fsv/phase-f-storage-exhaustion-fsv/` |
| `TASK-TEST-006` | Chaos harness with failpoints | **complete (#53)** — failpoint chaos FSV at `/mnt/d/contextgraph/fsv/phase-f-chaos-failpoints-fsv/` |
| `TASK-TEST-007` | Load test harness | **complete (#68)** — load-test FSV at `/mnt/d/contextgraph/fsv/phase-f-load-test-fsv/` |
| `TASK-TEST-008` | Soak test harness + nightly cron | **complete (#69)** — soak-test FSV at `/mnt/d/contextgraph/fsv/phase-f-soak-test-fsv/` |
| `TASK-TEST-009` | Toolchain fail-closed matrix | **complete (#70, commit 237b4aef)** — `crates/context-graph-mejepa/tests/toolchain_fail_closed.rs` (5 matrix assertions across 16 binaries) |
| `TASK-TEST-010` | Adversarial corpus for security robustness | **complete (#71)** — adversarial-corpus FSV at `/mnt/d/contextgraph/fsv/phase-f-adversarial-corpus-fsv/` |
| `TASK-TEST-011` | CI workflow updates for Phase F harnesses | **complete (#72, commit a56ceda3)** — `.github/workflows/{e2e,chaos,load,soak}.yml`; ubuntu-latest only; conditional harness discovery |

### Closed Phase F follow-up hardening tasks

| ID | Title | Status / Evidence |
|---|---|---|
| `TASK-PREDICT-013` | Pause-state consumer in `MeJepaCompiler::verify` | **complete (#82)** |
| `TASK-PREDICT-014` | Operator-override BatchSampler 6x upweight consumer | **complete (#83)** |
| `TASK-OBS-017` | `SystemCostCounters` producer wiring | **complete (#84)** |
| `TASK-FLYWHEEL-009` | Toolchain-audit startup fail-closed gate | **complete (#85)** |
| `TASK-PREDICT-LABEL-012` | Cross-language gating consumer + decisions audit CF | **complete (#86)** |
| `TASK-FLYWHEEL-010` | Unified `SupervisedTaskHealth` registry powering `/ready` | **complete (#87)** |
| `TASK-API-006` | Deprecation framework call-site audit | **complete (#88)** |
| `TASK-RUN-014` | Runbook actionability lint | **complete (#89)** |
| `TASK-FLYWHEEL-011` | Witness chain auto-quarantine on integrity-check failure | **complete (#90)** |
| `TASK-RUN-013` | Session-cleanup quota/GC-history reconciliation | **complete (#91)** |
| `TASK-SEC-009` | Agent identity attestation | **complete (#92)** |
| `TASK-HYG-002` | Retention policy for feedback + override CFs | **complete (#93)** |
| `TASK-OBS-018` | Heal-scheduler ticker resource burn telemetry | **complete (#94)** |
| `TASK-TEST-012` | Pin Phase F workflow Actions to SHA | **complete (#95)** |
| `TASK-FLYWHEEL-012` | Pause + active-learning enqueue suppression | **complete (#96)** |

**Phase F closure status (2026-05-13):** closed. GitHub issue readback shows no open issues with `phase:f-followup` and no open `[Phase F]` implementation issues. Remaining open work is Phase G convergence, not Phase F implementation.

**Phase F FSV index (selected evidence files):**

- `/mnt/d/contextgraph/fsv/phase-f-active-learning-cli-fsv/active_learning_cli_fsv.json` — sha256:a02788b1
- `/mnt/d/contextgraph/fsv/phase-f-runbook-cli-fsv/runbook_cli_fsv.json` — sha256:9c6cce66a2c227ce12479b690b33bcfa12cdfdaede3e375588ef3bdde059f706 (`all_passed=true`, `readback_equal=true`)
- `/mnt/d/contextgraph/fsv/phase-f-renderers-fsv/renderers_fsv.json` — sha256:6fe3b26e9bcbb3b418e5942a4e3038187ab8af627dedd15b3c7c6f880bc8a460
- `/mnt/d/contextgraph/fsv/phase-f-deprecation-framework-fsv/deprecation_framework_fsv.json` — sha256:180bb18146e2efea31203794a8712fa616a33c9ef053c36d7c7b505d4f36f23d
- `/mnt/d/contextgraph/fsv/phase-f-health-probe-fsv/health_probe_fsv.json` — sha256:ae96204da5a9ba6529775c1fa7fa1bc4c68aa671918fa90de8d38c9c0a9d2fac
- `/mnt/d/contextgraph/fsv/phase-f-toolchain-availability-fsv/toolchain_availability_fsv.json` — sha256:02b6e6fbf9e6d647647837741a94e94d5edafd6f4fb2b537a9acdcfe18284667
- `/mnt/d/contextgraph/fsv/phase-f-cross-language-label-gating-fsv/cross_language_label_gating_fsv.json` — sha256:873764c6e7504b1626c0cbb024d8edd809036af2e7666f5c397df89bd3ccaf34
- `/mnt/d/contextgraph/fsv/phase-f-label-transfer-gating-fsv/label_transfer_consumer_fsv.json` — sha256:8edc6686832b28360b974e84eb067c0821e3731d100f5d24319259f5e52bce9a
- `/mnt/d/contextgraph/fsv/phase-f-system-cost-fsv/system_cost_fsv.json` — sha256:cb76b6ee937fc9a968511db158e400abca0aaeb344643387bc6ce0abc6212b6b
- `/mnt/d/contextgraph/fsv/phase-f-operator-override-fsv/operator_override_fsv.json` — sha256:5eea66f1744b8e954a54ab2b9ea0be334e95861e4f73fc037b6977d91a46f99a
- `/mnt/d/contextgraph/fsv/phase-f-pause-cli-fsv/pause_cli_fsv.json` — sha256:1228d14e4a56afa7a67f8c17fd2fdeb5d2103c67e052d2fcfd8729d800b6df63
- `/mnt/d/contextgraph/fsv/utml-009-fsv/per_cell_state_transfer_fsv.json` — sha256:63234e2d1b492a18ca4533d750a1aa589206b02b060e4f0c3546654f4f078e0e
- `/mnt/d/contextgraph/fsv/phase-f-restart-safety-fsv/restart_safety_fsv.json`
- `/mnt/d/contextgraph/fsv/phase-f-multi-agent-fsv/multi_agent_fsv.json`
- `/mnt/d/contextgraph/fsv/phase-f-catastrophic-event-fsv/catastrophic_event_fsv.json`
- `/mnt/d/contextgraph/fsv/phase-f-storage-exhaustion-fsv/storage_exhaustion_fsv.json`
- `/mnt/d/contextgraph/fsv/phase-f-chaos-failpoints-fsv/chaos_failpoints_fsv.json`
- `/mnt/d/contextgraph/fsv/phase-f-load-test-fsv/load_test_fsv.json`
- `/mnt/d/contextgraph/fsv/phase-f-soak-test-fsv/soak_test_fsv.json`
- `/mnt/d/contextgraph/fsv/phase-f-adversarial-corpus-fsv/adversarial_corpus_fsv.json`
- `/mnt/d/contextgraph/fsv/phase-f-agent-identity-fsv/agent_identity_mcp_fsv.json` — sha256:1184ff5264454a1e78df6405cfd297370d0331d907be228e74e10f48e77c86d5
- `/mnt/d/contextgraph/fsv/phase-f-retention-fsv/feedback_override_retention_fsv.json` — sha256:80c285e6a2297492e5149b525ffc27bafb2170279321bba13b56115a9778f477
- `/mnt/d/contextgraph/fsv/phase-f-witness-quarantine-fsv/witness_quarantine_fsv.json`

---

## Phase G — Python ME-JEPA-Code Ship-Gate Convergence (active issue set)

| ID | Title | Phase | Blocked by | Effort |
|---|---|---|---|---|
| `TASK-G-001` | End-to-end corpus training-pipeline orchestrator (`context-graph-cli mejepa train --corpus <root> --split <train\|calibration\|holdout>`) | G | Phase C + D substrate | **complete (#97)** — writes/readbacks `CF_MEJEPA_PANELS`, `CF_MEJEPA_DDA_SIGNALS`, `CF_MEJEPA_LIVE_PREDICTIONS`, `CF_MEJEPA_ORACLE_VERDICTS`, and `CF_MEJEPA_TRAIN_CERTS`; FSV at `/mnt/d/contextgraph/fsv/phase-g-training-pipeline-orchestrator-fsv/training_pipeline_orchestrator_fsv.json` |
| `TASK-G-002` | Python per-mutation-category cell convergence verification driver (`context-graph-cli mejepa cell-status --threshold 0.95`) | G | `CF_MEJEPA_EVAL_REPORTS` | **complete (#98)** — reads the latest eval report, lists Python mutation-category cells with correlation, holdout count, distance to threshold, last update timestamp, and exits non-zero for below-threshold or missing cells; FSV at `/mnt/d/contextgraph/fsv/phase-g-cell-status-fsv/cell_status_fsv.json`; follow-up `TASK-PY-G-036` hardens the public surface to reject stale 80-cell gate assumptions with FSV at `/mnt/d/contextgraph/fsv/task-py-g-036-stale-gate-fsv/task_py_g_036_stale_gate_fsv.json` |
| `TASK-G-003` | Targeted training-data expansion for `off_by_one`, `subtle_flip`, and `swap_variable` Python cells | G | Mutation generators | **complete (#99)** — expands source-level operators for range/slice boundaries, membership/identity/any-all flips, function parameters, tuple assignment, loop locals, and zip/f-string variable uses; FSV materializes 600 rows per target category, validates every generated module with `py_compile`, and proves original/pass vs mutated/fail through pytest at `/mnt/d/contextgraph/fsv/phase-g-targeted-mutation-expansion-fsv/targeted_mutation_expansion_fsv.json` |
| `TASK-G-004` | Conformal recalibration after each A/B/C promotion | G | Phase E promotion path | **complete (#100)** — `AbcPromoter::commit` calls `recalibrate_conformal()` before active weights flip, persists `target_coverage=0.90` and empirical coverage to `CF_MEJEPA_CALIBRATION_HISTORY`, and blocks coverage outside `[0.88, 0.92]` with `MEJEPA_CONFORMAL_COVERAGE_OUT_OF_BAND`; FSV at `/mnt/d/contextgraph/fsv/phase-g-conformal-recalibration-fsv/conformal_recalibration_promotion_fsv.json` |
| `TASK-G-005` | Holdout corpus rotation policy to avoid stale holdout leakage | G | Phase E promotion path | **complete (#101)** — after every four successful promotions, deterministically moves 10% of train into holdout and 10% of holdout into train, persisting the rotation event and full after-split readback to `CF_MEJEPA_HEAL_REPORTS`; FSV at `/mnt/d/contextgraph/fsv/phase-g-holdout-rotation-fsv/holdout_rotation_fsv.json` |
| `TASK-G-006` | Four-consecutive-rolling-window stability assertion plus ship gate | G | `CF_MEJEPA_EVAL_REPORTS`, `CF_MEJEPA_MODEL_PROMOTIONS`, `mejepa_ship_gate_status` | **complete (#102)** — MCP ship-gate status now returns `consecutivePassingWindows`, resets on failing windows or model-promotion reset events, and requires four consecutive 0.95+ windows before `shipGatePassed=true`; FSV at `/mnt/d/contextgraph/fsv/phase-g-ship-gate-stability-fsv/ship_gate_stability_fsv.json` |
| `TASK-G-007` | Per-cell exemption policy for impossible cells | G | `config/cell_exemptions.toml`, `mejepa_ship_gate_status` | **complete (#103)** — MCP ship-gate status loads active operator-approved exemptions, excludes exempt cells from the effective ship-gate correlation, filters exempt per-cell failures, and still reports the exempt cell correlation plus reason for visibility; FSV at `/mnt/d/contextgraph/fsv/phase-g-cell-exemptions-fsv/cell_exemptions_fsv.json` |
| `TASK-G-008` | Ship-gate operator dashboard with Python per-cell drill-down | G | `context-graph-cli mejepa cell-status`, `mejepa_ship_gate_status` | **complete (#104)** — `scripts/render_ship_gate_dashboard.py` renders markdown/HTML from `CF_MEJEPA_EVAL_REPORTS`-backed JSON with per-cell heatmap, holdout counts, last updates, statuses, exemptions, and representative prediction links; FSV at `/mnt/d/contextgraph/fsv/phase-g-ship-gate-dashboard-fsv/ship_gate_dashboard_fsv.json`; follow-up `TASK-PY-G-035` makes the final bundle Python-gate specific |
| `TASK-G-009` | Production telemetry -> training feedback loop wiring | G | `CF_MEJEPA_PRODUCTION_TELEMETRY`, `CF_MEJEPA_HEAL_REPORTS` | **complete (#105)** — the `telemetry_feedback` heal-scheduler ticker reads the latest production telemetry window, queues recalibration when conformal ECE implies coverage outside `[0.88, 0.92]`, queues a full-retrain candidate when prediction-oracle agreement is below `0.95`, and persists typed `HealReport` actions; FSV at `/mnt/d/contextgraph/fsv/phase-g-telemetry-feedback-fsv/telemetry_feedback_fsv.json` |
| `TASK-G-010` | Adversarial-corpus retrain cycle integration | G | TASK-TEST-010 adversarial fixtures, BatchSampler | **complete (#106)** — `BatchSampler` reserves the configured 20% adversarial mix-in, training certificates persist adversarial counts/indices/fallbacks, and the Phase F adversarial corpus is replayed through the retrain-cycle FSV with `non_pass_rate=1.0`; FSV at `/mnt/d/contextgraph/fsv/phase-g-adversarial-retrain-fsv/phase_g_adversarial_retrain_fsv.json` |
| `TASK-G-011` | Per-cell state-transfer T diagnostic in weekly eval | G | TASK-UTML-009 | **complete (#107)** — weekly eval markdown renders a `state_transfer_per_cell` table from persisted per-cell diagnostics, including unavailable and low-transfer cells; FSV at `/mnt/d/contextgraph/fsv/phase-g-state-transfer-weekly-markdown-fsv/state_transfer_weekly_markdown_fsv.json` |
| `TASK-G-012` | Cross-validate Python holdout correlation on a public CI corpus | G | TASK-G-001 | **complete (#108)** — `context-graph-cli mejepa cross-validate-public-ci` identifies GHALogs, ingests a 560-row public-CI subset through the same panel/DDA/prediction/oracle/cert writer, compares public-CI cells against a Lite baseline, and proves reopened CF readback; FSV at `/mnt/d/contextgraph/fsv/phase-g-public-ci-cross-validation-fsv/public_ci_cross_validation_fsv.json` |
| `TASK-G-013` | Drift attribution root-cause classification for failing cells | G | `CF_MEJEPA_EVAL_REPORTS`, TASK-G-002 | **complete (#109)** — failing eval cells are classified as insufficient training data, embedder signal gap, label noise, oracle flakiness, distribution shift, or unknown; weekly markdown renders the source-backed root-cause table; FSV at `/mnt/d/contextgraph/fsv/phase-g-failing-cell-root-cause-fsv/failing_cell_root_cause_fsv.json` |
| `TASK-G-014` | Re-baseline calibration after operator overrides | G | TASK-PREDICT-012, `SystemCostCounters`, `telemetry_feedback` | **complete (#110)** — the `telemetry_feedback` tick tracks `overrides_consumed_since_calibration` from the operator-override sampler counter, queues `TriggerReason::OperatorOverrideRecalibration` after >100 consumed overrides, dedupes the current threshold crossing, and preserves a watermark for future 101-override windows; FSV at `/mnt/d/contextgraph/fsv/phase-g-operator-override-recalibration-fsv/operator_override_recalibration_fsv.json` |
| `TASK-G-015` | Label-quality audit for active Python cells plus archived language evidence | G | `CF_MEJEPA_FAILURE_EXEMPLARS` | **complete (#111)** — `scripts/audit_label_quality.py` reads `CF_MEJEPA_FAILURE_EXEMPLARS` via `ldb scan --hex`, reports labels-per-row mean/min/max, non-trivial-signal ratio, unavailable rows, and unknown-language rows to `/mnt/d/contextgraph/exports/eval/<date>/label_quality.md`; FSV at `/mnt/d/contextgraph/fsv/phase-g-label-quality-fsv/label_quality_fsv.json`; Phase G consumes this as Python gate evidence |
| `TASK-G-016` | Per-cell promotion gate documentation + reporting | G | Phase E promotion path | **complete (#112)** — `AbcPromoter::evaluate` and the live A/B/C winner path now require every baseline holdout cell to hold-or-improve within 0.005 correlation tolerance before promotion; violating candidates fail closed with `MEJEPA_PROMOTION_CELL_REGRESSION`; FSV at `/mnt/d/contextgraph/fsv/phase-g-per-cell-promotion-gate-fsv/per_cell_promotion_gate_fsv.json` |
| `TASK-G-017` | Holdout-vs-training distribution drift monitor | G | `CF_MEJEPA_PRODUCTION_TELEMETRY`, `telemetry_feedback` | **complete (#113)** — `ProductionTelemetryWindow` persists the KL divergence between training-batch and holdout `(mutation_category, language)` distributions, validates the sustained-batch threshold, and the `telemetry_feedback` ticker emits `MEJEPA_TRAINING_HOLDOUT_DRIFT` as a durable `CF_MEJEPA_HEAL_REPORTS` action when `KL > 0.5` for at least 100 batches; FSV at `/mnt/d/contextgraph/fsv/phase-g-training-holdout-drift-fsv/training_holdout_drift_fsv.json` |
| `TASK-G-018` | Weekly eval -> operator-actionable cell triage list | G | `weekly.md`, `EvalReport.failing_cell_classifications` | **complete (#114)** — weekly eval markdown renders an `Operator Cell Triage` top-10 table ranked by distance to the 0.95 cell target and maps root causes to operator actions (`expand_corpus`, `request_adversarial_sample`, `recalibrate`, `mark_exempt`); FSV at `/mnt/d/contextgraph/fsv/phase-g-weekly-cell-triage-fsv/weekly_cell_triage_fsv.json` |
| `TASK-G-019` | Oracle determinism / flakiness audit per cell | G | TASK-G-001, corpus oracle observations | **complete (#115)** — `mejepa-corpus verify --oracle-repeat-runs 3 --oracle-repeat-output <json>` produces repeat-oracle observations, `context-graph-cli mejepa oracle-flakiness-audit` writes `config/corpus_quarantine.toml`, and train ingestion skips quarantined tasks before train/holdout SoT writes; FSV at `/mnt/d/contextgraph/fsv/phase-g-oracle-flakiness-quarantine-fsv/oracle_flakiness_quarantine_fsv.json` |
| `TASK-G-020` | Per-failure-mode-class precision/recall holdout breakdown | G | `EvalReport`, `FailureModeClass` labels | **complete (#116)** — `EvalReport.per_failure_mode_class` carries precision, recall, F1, confusion counts, thresholds, and weakness strings for all 36 `FailureModeClass` values; weekly markdown and dashboard JSON render the map; FSV at `/mnt/d/contextgraph/fsv/phase-g-failure-mode-class-metrics-fsv/failure_mode_class_metrics_fsv.json` |
| `TASK-G-021` | Convergence ETA tracker per cell | G | `CF_MEJEPA_EVAL_REPORTS` rolling windows | **complete (#117)** — `eval/convergence_tracker.rs` fits a linear model over the last N eval windows, emits `estimated_passing_window` plus a confidence interval per cell, and renders the ETA in weekly triage/dashboard/cell-status surfaces; FSV at `/mnt/d/contextgraph/fsv/phase-g-convergence-eta-fsv/convergence_eta_fsv.json` |
| `TASK-PY-G-001..TASK-PY-G-040` | Remaining Python ME-JEPA-Code convergence tasks: Python reward-signal coverage, Q1-Q5 calibration, Python training-cycle execution, final burn-in, and post-green guardrails | G | TASK-G-001 where corpus rows are required | open GitHub issues #118-#131, #148-#154, #162-#173, #178-#182; #158/#159 are complete; non-Python #160/#161 are closed as deferred |
| `TASK-EK-001` | Compression progress as primary explanatory-knowledge claim + `mejepa_compression_progress` MCP/CLI/weekly surfaces | G/EK | `CF_MEJEPA_TRAIN_CERTS`, training certificates | **complete (#322)** — optional `TrainingCertificate.conditional_description_length_bits` schema field, CPΦ rolling-window report, MCP tool, CLI command, weekly `Compression Progress` section, PAPER.md/CHANGELOG documentation, and fail-closed boundary cases; FSV `/mnt/d/contextgraph/fsv/task-ek-001-compression-progress-fsv/run-1779059471712-4091732/compression_progress_fsv.json` sha256 `e810d2a0cb8c8a07521c37aa866ef3f94de6cc49d76bdf495a3443c0b3db97a4` |
| `TASK-EK-002` | Absence-as-existence instrument proposal + promotion MCP surfaces | G/EK | `CF_MEJEPA_ACTIVE_LEARNING_QUEUE`, `CF_MEJEPA_PAIRWISE_MI`, `CF_MEJEPA_MINCUT_REPORTS`, `CF_MEJEPA_INSTRUMENT_PROPOSALS` | **complete (#323)** — `InstrumentProposal` rows, default `K_min=30`, deterministic cluster/mincut/MI composition, MCP propose/promote tools, accepted/rejected/under-review decision readback, and all five edge cases; FSV `/mnt/d/contextgraph/fsv/task-ek-002-propose-instrument-fsv/propose_instrument_fsv.json` sha256 `a9a5c08f0b3df1bc875050688c9b59dffc7d01f96c15589121d6ed6f03b8b4a3` |
| `TASK-EK-006` | Compression-progress curiosity reward for active-learning queue | G/EK | `CF_MEJEPA_ACTIVE_LEARNING_QUEUE`, `CF_MEJEPA_CURIOSITY_TELEMETRY`, active-learning CLI/MCP, weekly markdown | **complete (#327)** — active-learning rows carry `curiosity_score`, the CLI/MCP can rank by curiosity, per-window distributions and CPΦ calibration evidence persist to `CF_MEJEPA_CURIOSITY_TELEMETRY`, and weekly markdown renders `Curiosity Ranking`; FSV `/mnt/d/contextgraph/fsv/task-ek-006-curiosity-fsv/curiosity_fsv.json` sha256 `cef7b6e576875fd1df8d92f849c4e4adf032ac6d834ac967e94b6eaddb30646f` |
| `TASK-EK-010` | Retired north-star / intent elicitation surface | G/EK | none | **retired (#358/#359)** — removed from active product scope because ME-JEPA must not model, classify, synthesize, infer, or embed private human intention |
| `TASK-EK-012` | Bidirectional corpus training: forward + inverse loss plus four-variant 300x8 ablation | G/EK | `CF_MEJEPA_TRAIN_CERTS`, trainer config, inverse loss, 300x8 corpus | **complete (#334)** — `TrainingConfig.inverse_map_coefficient` defaults to `0.0`, `TrainingCertificate.inverse_map_quality_bits` is optional/historical-compatible, `loss::inverse` composes `L_forward + lambda_inv * L_inverse`, `MeJepaPredictor` has a shared-trunk inverse head that decodes input panel + 16-d action latent from the target panel, positive lambda without a real inverse provider or structured patch-diff/tool-call `InverseActionTarget` rows fails closed, and the real 300x8 ablation ran `lambda_inv in {0,0.25,0.5,1.0}`. Substrate FSV `/mnt/d/contextgraph/fsv/task-ek-012-bidirectional-corpus-fsv/run-1779105980292/bidirectional_corpus_fsv.json` sha256 `794641d760196a5bd4bf348297bed9529583b0af08d52a7e67f4abdb72f12a68`; ablation FSV `/mnt/d/contextgraph/fsv/task-ek-012-bidirectional-ablation-fsv/run-20260518T123249Z/bidirectional_ablation_fsv.json` sha256 `a98f9e04f991a262f064b4fddfb82e3832db6eb2e1055198940c8a2ab350b9e1`; report `/mnt/d/contextgraph/exports/ek-012/bidirectional-ablation-report.md` sha256 `8da50218775cb1ef2743c323b4708cf632fe16cdbfe96db475b73db7f3643587`. Result is neutral: all variants measured `0.8666666746139526`, best delta vs baseline `0.0`, all per-cell regression gates passed, production remains `lambda_inv=0.0`. |
| `TASK-PY-G-069` | PostToolUse Bash test-result capture -> prediction verification CF | G/feedback | `CF_MEJEPA_LIVE_PREDICTIONS`, `runtime/test-outcomes/<session>.jsonl` | **complete (#288)** — `context-graph-cli mejepa bind-test-outcomes` persists `CF_MEJEPA_PREDICTION_VERIFICATIONS`, logs `/mnt/d/contextgraph/runtime/posttooluse-mejepa-capture.jsonl`, handles ambiguous output as Inconclusive, leaves non-test Bash output unwritten, and queues Refuted rows at 6x active-learning weight; FSV `/mnt/d/contextgraph/fsv/task-py-g-069-posttooluse-capture-fsv/run-1779062220556-12961/posttooluse_capture_fsv.json` sha256 `cc35795fcd4e722f7f116f6176ca65d2b92bf5921e3e37137000618b6aa36e9c` |
| `TASK-PY-G-070` | Stop-hook self-verification gate for live predictions | G/feedback | `CF_MEJEPA_LIVE_PREDICTIONS`, `CF_MEJEPA_PREDICTION_VERIFICATIONS` | **complete (#289)** — `context-graph-cli mejepa stop-self-verify` lists non-Abstain predictions lacking Confirmed/Refuted verification, `scripts/cc/stop-mejepa-self-verify.sh` exits 2 with structured stderr + top-10 commands, honors `stop_hook_active`/operator disable, and logs daemon-unreachable warnings without blocking infrastructure failures; FSV `/mnt/d/contextgraph/fsv/task-py-g-070-stop-self-verify-fsv/run-1779063364468-91155/stop_self_verify_fsv.json` sha256 `70e1ab51c6f2a8dac0ad7d5a5781ccc23f529decd2b87db1a9a6a9cb7caf30d0` |
| `TASK-PY-G-337` | Observe ticker no-prediction quarantine for captured test outcomes | G/feedback | `CF_MEJEPA_LIVE_PREDICTIONS`, `CF_MEJEPA_PREDICTION_VERIFICATIONS`, `CF_MEJEPA_SIGNAL_DROP_LOG` | **complete (#337)** — matched observe-ticker outcomes still bind to oracle verdicts and now also to prediction-verification rows; missing-prediction outcomes persist `MEJEPA_TEST_OUTCOME_NO_PREDICTION` signal-drop rows without leaving `last_error.observe`; malformed rows still fail closed; FSV `/mnt/d/contextgraph/fsv/task-py-g-337-observe-missing-prediction-fsv/run-1779067066572/observe_missing_prediction_fsv.json` sha256 `66427f6578dca766aef94b8f76d90c7418b388f2d70a1c747e667b3ffd99052d` |

**Phase G status (2026-05-18):** active, Python-first, and now post-forward-cache-completion. The orchestrator proves corpus rows can enter the source-of-truth reward pipeline with `input_task_count = output_panel_count = logical_dda_signal_count / 462 = oracle_verdict_count`. The completed aiwonder substrate proves the full `399777`-chunk, 12-embedder forward cache is physically materialized: `391/391` shards, `4,797,324/4,797,324` accounted pairs, and `python_forward_cache_corpus_manifest.json` `status=passed`. The current blocker is no longer data-scale completion, tau calibration, classifier plumbing, Unknown/OOD queue persistence, MCP catalog tooling, per-fingerprint ship-gate storage/status, TASK-FP-008 source-of-truth stability storage/readback, TASK-FP-010 live prediction evidence, TASK-FP-009/#279 adversarial integration, TASK-EK-001 compression-progress measurement, TASK-EK-012 trainer substrate/config/cert/inverse-head/action-target/ablation readback, TASK-PY-G-007 pairwise-MI CF/mincut substrate, TASK-PY-G-069 PostToolUse prediction-verification binding, or TASK-PY-G-070 Stop self-verification enforcement; it is model-quality convergence. TASK-FP-001 now has typed catalog CFs/model/storage FSV; TASK-FP-002 is production-complete with 97 seed fingerprints, 13 KnownGood, 84 KnownBad, 4,665,772 joined records, 632,872 reference rows, and all 12 embedders per fingerprint under `/zfs/archive/contextgraph/corpus/python-failure-fingerprints-seed-v1/`. TASK-FP-003 is production-complete with calibrated output under `/zfs/archive/contextgraph/corpus/python-failure-fingerprints-calibrated-v1/`: 97 calibration records, 7,413,631 calibrated vector observations, and tau range `0.049051024765955006..1.0`. TASK-FP-004 is code/FSV-shipped: the compiler can classify a 12-embedder observation against `CF_MEJEPA_FAILURE_FINGERPRINTS` and emit Pass/Fail/OOD/Abstain with ranked fingerprint evidence. TASK-FP-005 is code/FSV-shipped: no-known-match classifications can enqueue durable Unknown candidates in `CF_MEJEPA_ACTIVE_LEARNING_QUEUE` and suggest deterministic new-fingerprint clusters. TASK-FP-006 is code/FSV-shipped: MCP can list, inspect, classify, suggest new clusters, label Unknown candidates, promote canonical references, and recalibrate per-fingerprint tau with RocksDB readback. TASK-FP-007 is code/FSV-shipped: four-window per-fingerprint accuracy/precision and Unknown/OOD recall are persisted in `CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS` and combined with the existing per-cell eval gate in `mejepa_ship_gate_status`. TASK-FP-008 now has code/FSV-shipped `CF_MEJEPA_FINGERPRINT_FISHER` and `CF_MEJEPA_FINGERPRINT_DORMANCY` rows plus the `mejepa_fingerprint_catalog_stability` MCP readback surface. TASK-FP-010 is code/FSV-shipped: `RealityPrediction` carries `matched_fingerprint` / `unknown_candidate_id`, compiler callers can attach explicit forward-cache observation evidence, and MCP latest/explain surfaces expose the evidence. TASK-PY-G-060/TASK-FP-009 is code/FSV-shipped: 30 adversarial corpus rows persist in `CF_MEJEPA_ADVERSARIAL_CORPUS`, 30 adversarial KnownBad fingerprints persist with `source_corpus=python-adversarial-v1`, and four fingerprint ship-gate windows include the adversarial rows. TASK-EK-001/#322 is code/FSV-shipped: training certs carry conditional description length bits, `mejepa_compression_progress` exposes CPΦ, and the weekly report renders source-backed compression progress. TASK-EK-012/#334 is complete with a neutral four-variant 300x8 ablation: all lambdas measured `0.8666666746139526`, best delta vs baseline `0.0`, all per-cell gates passed, and production remains `lambda_inv=0.0`. TASK-PY-G-007/#122 is code/FSV-shipped against real aiwonder samples: `CF_MEJEPA_PAIRWISE_MI` stores 66-row 12-active-embedder pairwise-MI matrices, MCP emits redundancy/adaptive-weight/`N_eff` summaries, and mincut reads the persisted matrix as `pairwiseMiMatrix`. TASK-PY-G-069/#288 is code/FSV-shipped: PostToolUse Bash test outcomes now create durable prediction-verification rows and Refuted active-learning pressure. TASK-PY-G-070/#289 is code/FSV-shipped: Stop events now block on actionable unverified non-Abstain predictions until #288 writes Confirmed/Refuted evidence. The cold-start verdict policy (`TASK-PY-G-061` / #280) is code/FSV-shipped and now prevents under-supported TCT cells from emitting overconfident non-abstain verdicts. The latest verified model-quality blocker remains `prediction_oracle_correlation = 0.866667 < 0.95`. The open issue queue exists to make the Python SWE-bench path real end to end: Python code changes are parsed into AST chunks, exploded through frozen instruments, DDA, and TCT, trained into calibrated Q1-Q5/fingerprint `RealityPrediction`, served through local MCP stdio, fed back to AI agents, and continually improved by oracle outcomes, Edit shifts, and agent/operator feedback. Non-Python fixture expansion and proof-domain harnesses are explicitly deferred until this Python gate is green.

### Python Ship-Gate Dependency Map (TASK-PY-G-001 / #158)

This is the active execution-order map for the Python ME-JEPA-Code ship gate. The path starts at a SWE-bench Lite Python row and ends at a calibrated MCP `RealityPrediction`; gridworld, proof-domain, and non-Python harnesses are not active prerequisites. Any task, command, or report that reintroduces a stale 80-cell `(mutation_category x language)` gate into this Python-only path must fail closed through `TASK-PY-G-036` before it can influence release decisions.

Trigger -> Process -> Outcome for the active gate:

```text
SWE-bench Lite Python row
  -> Python AST chunks + typed task metadata
  -> frozen target instruments (oracle, witness, type/claim/impact panels)
  -> DDA/TCT reward-signal constellation
  -> checkpointed Python training + calibration
  -> Q1-Q5 RealityPrediction heads
  -> MCP stdio prediction surface
  -> durable feedback/replay loop
  -> four-window 0.95 Python ship-gate verdict
```

Boundary invariants for the dependency map:

- Empty or missing corpus input: no downstream task may synthesize rows; the runner must fail closed before panel/DDA/TCT writes.
- Malformed Python or invalid metadata: the row must be quarantined or rejected with a typed error; it must not enter calibration as a weak label.
- Maximum batch: the active corpus gate is `300 x 8 = 2400` Python mutation rows; max+1 is an error, not truncation.
- Scope guard: non-Python rows and proof-domain fixtures are archived evidence only until `TASK-PY-G-034` accepts the Python bundle.

| Order | Task | Issue | Stage | Direct predecessors | Direct successors | Source-of-truth state |
|---:|---|---:|---|---|---|---|
| 1 | `TASK-PY-G-001` Python ship-gate dependency map | #158 | graph | `TASK-G-001..021` complete Phase G substrate | `TASK-PY-G-036`, `TASK-PY-G-037`, `TASK-PY-G-002` | **complete (#158)** — `docs/futurebuild/13_TASK_INDEX.md`, GitHub issue graph |
| 2 | `TASK-PY-G-036` Reject stale 80-cell gates | #178 | scope guard | `TASK-PY-G-001` | `TASK-PY-G-037`, `TASK-PY-G-002`, `TASK-PY-G-038` | CLI/MCP gate output rejects non-Python active gates |
| 3 | `TASK-PY-G-037` Isolate proof-domain harnesses | #179 | scope guard | `TASK-PY-G-001`, `TASK-PY-G-036` | `TASK-PY-G-002`, `TASK-PY-G-038` | **complete (#179)** — `python-proof-domain-isolation` scans active repo paths and open issues, rejects proof-domain Python prerequisite drift, and writes FSV at `/mnt/d/contextgraph/fsv/task-py-g-037-proof-domain-isolation-fsv/python_proof_domain_isolation_fsv.json` |
| 4 | `TASK-PY-G-002` Checkpointed Python training runner | #159 | runner | `TASK-PY-G-001`, `TASK-PY-G-036`, `TASK-PY-G-037` | `TASK-PY-G-018`, `TASK-PY-G-022`, `TASK-PY-G-023` | **complete (#159)** — `context-graph-cli mejepa checkpointed-train` writes split checkpoints, resumes from manifest, persists `CF_MEJEPA_EVAL_REPORTS`, and exports eval JSON; FSV at `/mnt/d/contextgraph/fsv/issue-159-checkpointed-python-training-fsv/run-20260513T231346Z/issue_159_fsv.json` |
| 5 | `TASK-PY-G-018` SWE-bench Verified Python subset | #149 | corpus | `TASK-PY-G-002` | `TASK-PY-G-022`, `TASK-PY-G-017`, `TASK-PY-G-004` | Python task rows and verified subset manifest |
| 6 | `TASK-PY-G-022` Python oracle pass/fail audit | #153 | oracle | `TASK-PY-G-018`, `TASK-PY-G-002` | `TASK-PY-G-017`, `TASK-PY-G-005`, `TASK-PY-G-010` | oracle verdict rows and flake/quarantine evidence |
| 7 | `TASK-PY-G-017` Python leakage detector | #148 | corpus guard | `TASK-PY-G-022`, `TASK-PY-G-018` | `TASK-PY-G-009`, `TASK-PY-G-030`, `TASK-PY-G-012`, `TASK-PY-G-013` | leakage report and quarantined row readback |
| 8 | `TASK-PY-G-009` Frozen instrument coverage audit | #124 | target panel | `TASK-PY-G-017` | `TASK-PY-G-007`, `TASK-PY-G-008` | frozen instrument coverage rows |
| 9 | `TASK-PY-G-007` Python embedder MI rotation | #122 | embedding | `TASK-PY-G-009` | `TASK-PY-G-008`, `TASK-PY-G-039` | **complete (#122)** — `CF_MEJEPA_PAIRWISE_MI` stores real aiwonder 12-active-embedder/66-pair matrices with byte-identical readback, MCP returns redundancy histogram/adaptive weights/`effectiveSignalCount`, and `TASK-EK-003` mincut consumes the persisted matrix; FSV `/mnt/d/contextgraph/fsv/task-py-g-007-pairwise-mi-fsv/run-1779073845986/pairwise_mi_fsv.json` |
| 10 | `TASK-PY-G-008` Python TCT signal-density audit | #123 | TCT | `TASK-PY-G-009`, `TASK-PY-G-007` | `TASK-PY-G-039`, `TASK-PY-G-019` | TCT density report over Python cells |
| 11 | `TASK-PY-G-039` TCT completeness audit | #181 | TCT | `TASK-PY-G-008`, `TASK-PY-G-007` | `TASK-PY-G-003`, `TASK-PY-G-019` | per-row TCT completeness readback |
| 12 | `TASK-PY-G-019` Python sample-efficiency analysis | #150 | data quality | `TASK-PY-G-008`, `TASK-PY-G-039` | `TASK-PY-G-004`, `TASK-PY-G-020` | sample-efficiency report by mutation category |
| 13 | `TASK-PY-G-003` Per-head Q1-Q5 distillation | #118 | heads | `TASK-PY-G-039` | `TASK-PY-G-010`, `TASK-PY-G-011`, `TASK-PY-G-012`, `TASK-PY-G-013`, `TASK-PY-G-020` | Q1-Q5 head weights and calibration rows |
| 14 | `TASK-PY-G-010` Q1 claim existence at corpus scale | #125 | Q1 | `TASK-PY-G-003`, `TASK-PY-G-022` | `TASK-PY-G-027`, `TASK-PY-G-038` | claim existence predictions vs file-system SoT |
| 15 | `TASK-PY-G-011` Q3 failure-mode calibration | #126 | Q3 | `TASK-PY-G-003`, `TASK-PY-G-022` | `TASK-PY-G-006`, `TASK-PY-G-028`, `TASK-PY-G-038` | failure-mode labels and calibrated Q3 bins |
| 16 | `TASK-PY-G-012` Q4 concern confidence calibration | #127 | Q4 | `TASK-PY-G-003`, `TASK-PY-G-017` | `TASK-PY-G-006`, `TASK-PY-G-038` | security/perf/tech-debt concern calibration |
| 17 | `TASK-PY-G-013` Q5 impact vs shift-log | #128 | Q5 | `TASK-PY-G-003`, `TASK-PY-G-017` | `TASK-PY-G-038` | shift-log impact readback vs predicted Q5 impact |
| 18 | `TASK-PY-G-020` Predictor ablation harness | #151 | diagnostics | `TASK-PY-G-003`, `TASK-PY-G-019` | `TASK-PY-G-027`, `TASK-PY-G-031` | head/instrument ablation metrics |
| 19 | `TASK-PY-G-014` Python OOD detection gate | #129 | safety | `TASK-PY-G-003`, `TASK-PY-G-039` | `TASK-PY-G-027`, `TASK-PY-G-030` | OOD score distribution and fail-closed gate |
| 20 | `TASK-PY-G-006` Python conformal RCA classifier | #121 | RCA | `TASK-PY-G-011`, `TASK-PY-G-012` | `TASK-PY-G-015`, `TASK-PY-G-028`, `TASK-PY-G-031` | conformal RCA class rows |
| 21 | `TASK-PY-G-015` Python Gtau guard audit | #130 | calibration guard | `TASK-PY-G-006`, `TASK-PY-G-003` | `TASK-PY-G-027` | Gtau guard evidence and rejected-boundary rows |
| 22 | `TASK-PY-G-021` Python E_Trace gating | #152 | trace guard | `TASK-PY-G-003`, `TASK-PY-G-006` | `TASK-PY-G-027` | E_Trace gate rows and rejected traces |
| 23 | `TASK-PY-G-028` Python error taxonomy | #166 | errors | `TASK-PY-G-006`, `TASK-PY-G-011` | `TASK-PY-G-031`, `TASK-PY-G-034` | typed Python error taxonomy and dashboard mapping |
| 24 | `TASK-PY-G-005` Catastrophic Python cell quarantine | #120 | safety | `TASK-PY-G-022`, `TASK-PY-G-017` | `TASK-PY-G-030`, `TASK-PY-G-027` | quarantined catastrophic cell rows |
| 25 | `TASK-PY-G-023` Active-learning consumer | #154 | feedback | `TASK-PY-G-002`, `TASK-PY-G-022` | `TASK-PY-G-016`, `TASK-PY-G-024` | active-learning queue events |
| 26 | `TASK-PY-G-016` Feedback cooldown for Python | #131 | feedback guard | `TASK-PY-G-023` | `TASK-PY-G-024`, `TASK-PY-G-040` | cooldown state and duplicate/replay rejection |
| 27 | `TASK-PY-G-024` Feedback retrain consumer | #163 | feedback retrain | `TASK-PY-G-023`, `TASK-PY-G-016` | `TASK-PY-G-025`, `TASK-PY-G-040` | feedback replay rows and retrain trigger |
| 28 | `TASK-PY-G-040` Feedback replay-weight FSV | #182 | feedback FSV | `TASK-PY-G-024`, `TASK-PY-G-016` | `TASK-PY-G-034` | replay weight before/after and row equality |
| 29 | `TASK-PY-G-025` Eval-after-promotion hook | #162 | promotion | `TASK-PY-G-024`, `TASK-PY-G-040` | `TASK-PY-G-026`, `TASK-PY-G-027`, `TASK-PY-G-032` | promotion eval report persisted after candidate flip |
| 30 | `TASK-PY-G-026` Checkpoint provenance | #164 | provenance | `TASK-PY-G-025` | `TASK-PY-G-027`, `TASK-PY-G-033` | checkpoint manifest, hashes, train/eval commit |
| 31 | `TASK-PY-G-027` Python 0.95 confidence gate | #165 | gate | `TASK-PY-G-010`, `TASK-PY-G-014`, `TASK-PY-G-015`, `TASK-PY-G-020`, `TASK-PY-G-021`, `TASK-PY-G-026` | `TASK-PY-G-029`, `TASK-PY-G-032`, `TASK-PY-G-033`, `TASK-PY-G-038` | Python-only confidence/correlation gate |
| 32 | `TASK-PY-G-004` Python generalization audit | #119 | generalization | `TASK-PY-G-018`, `TASK-PY-G-019` | `TASK-PY-G-034` | unseen/generalization audit report |
| 33 | `TASK-PY-G-030` Unseen Python holdout quarantine | #169 | holdout guard | `TASK-PY-G-017`, `TASK-PY-G-005`, `TASK-PY-G-014` | `TASK-PY-G-032` | holdout quarantine manifest |
| 34 | `TASK-PY-G-031` Model-quality RCA dashboard | #170 | operator RCA | `TASK-PY-G-006`, `TASK-PY-G-020`, `TASK-PY-G-028` | `TASK-PY-G-034`, `TASK-PY-G-035` | RCA dashboard JSON/markdown |
| 35 | `TASK-PY-G-029` Live Python patch backfill | #168 | live data | `TASK-PY-G-027` | `TASK-PY-G-038`, `TASK-PY-G-032`, `TASK-PY-G-034` | live patch backfill rows and outcomes |
| 36 | `TASK-PY-G-032` Four-window Python burn-in | #167 | stability | `TASK-PY-G-025`, `TASK-PY-G-027`, `TASK-PY-G-029`, `TASK-PY-G-030` | `TASK-PY-G-033`, `TASK-PY-G-034` | four rolling windows, all Python cells >= 0.95 |
| 37 | `TASK-PY-G-033` Fail-closed release switch | #171 | release gate | `TASK-PY-G-026`, `TASK-PY-G-027`, `TASK-PY-G-032` | `TASK-PY-G-038`, `TASK-PY-G-034` | release switch rejects unknown or stale gate state |
| 38 | `TASK-PY-G-038` MCP RealityPrediction FSV | #180 | MCP | `TASK-PY-G-010`, `TASK-PY-G-011`, `TASK-PY-G-012`, `TASK-PY-G-013`, `TASK-PY-G-027`, `TASK-PY-G-029`, `TASK-PY-G-033`, `TASK-PY-G-037` | `TASK-PY-G-034` | MCP stdio `RealityPrediction` readback |
| 39 | `TASK-PY-G-034` Python acceptance bundle | #172 | acceptance | `TASK-PY-G-004`, `TASK-PY-G-028`, `TASK-PY-G-029`, `TASK-PY-G-031`, `TASK-PY-G-032`, `TASK-PY-G-033`, `TASK-PY-G-038`, `TASK-PY-G-040` | `TASK-PY-G-035` | acceptance bundle with issue/FSV/doc hashes |
| 40 | `TASK-PY-G-035` Post-green drift guardrails | #173 | post-green | `TASK-PY-G-031`, `TASK-PY-G-034` | Python ship gate may stay green under drift monitors | drift guardrail alerts and rollback hooks |

---

## Phase H — Inner-Loop Residue Cleanup (expanded cleanup)

| ID | Title | Phase | Blocked by | Effort |
|---|---|---|---|---|
| `TASK-LOOP-003` | Mark superseded SMAP decisions | H | LOOP-001 | **complete (#132)** — five retired inner-actor decision files have `status: superseded` + `superseded_by: 2026-05-09--single-outer-loop-mejepa-no-local-llm.md`; FSV at `/mnt/d/contextgraph/fsv/phase-h-smap-superseded-decisions-fsv/smap_superseded_decisions_fsv.json` |
| `TASK-LOOP-004` | Retire legacy MCP tools | H | none | **complete (#133)** — five retired cgreality compatibility names are excluded from tool definitions and fail closed with `CCREALITY_ENGINE_RETIRED`; FSV at `/mnt/d/contextgraph/fsv/phase-h-retired-mcp-tools-fsv/retired_mcp_tools_fsv.json` |
| `TASK-LOOP-005` | Audit absence of inner-actor model files | H | none | **complete (#134)** — `scripts/audit/no_inner_actor_models.sh`; FSV at `/mnt/d/contextgraph/fsv/phase-h-no-inner-actor-models-fsv/no_inner_actor_models_fsv.json` |
| `TASK-LOOP-006` | Delete stale `target/release/reality-loop` binary | H | none | **complete (#135)** — `scripts/ci/phase_h_reality_loop_binary_absence_fsv.sh`; FSV at `/mnt/d/contextgraph/fsv/phase-h-reality-loop-binary-absence-fsv/reality_loop_binary_absence_fsv.json` |
| `TASK-LOOP-008` | Verify `11_DELETION_LIST_INNER_LOOP` fully applied | H | LOOP-003..006, LOOP-009..012 | **complete (#136)** — entry-by-entry audit report at `memory/reports/phase-h-deletion-list-audit.md`, retired skill cleanup, stale `memory/context/current-phase.md` rewrite, and FSV at `/mnt/d/contextgraph/fsv/phase-h-deletion-list-audit-fsv/deletion_list_audit_fsv.json` |
| `TASK-LOOP-009` | Update CLAUDE.md inner-loop references | H | none | **complete (#137)** — remaining references are retirement/deny/fail-closed guidance; FSV at `/mnt/d/contextgraph/fsv/phase-h-claude-inner-loop-reference-fsv/claude_inner_loop_reference_fsv.json` |
| `TASK-LOOP-010` | Update agents/README.md retirements | H | none | **complete (#138)** — retired agent frontmatter + active roster FSV at `/mnt/d/contextgraph/fsv/phase-h-agent-roster-retirement-fsv/agent_roster_retirement_fsv.json` |
| `TASK-LOOP-011` | `CCREALITY_ENGINE_RETIRED` gate audit | H | LOOP-004 | **complete (#139)** — all five retired endpoints have unit fail-closed coverage, one JSON-RPC round-trip structured-error test, and fmt/check/clippy FSV at `/mnt/d/contextgraph/fsv/phase-h-ccreality-engine-retired-fsv/ccreality_engine_retired_gate_fsv.json` |
| `TASK-LOOP-012` | Remove orphaned scripts/config inner-LLM references | H | LOOP-005 | **complete (#140)** — `scripts/ci/phase_h_scripts_config_inner_llm_residue_fsv.sh` classifies scripts/config matches as retired-audit, generated snapshot, or explicit `historical, do not reanimate`; active disallowed count 0; four retired Shakespeare local-runtime helpers abort; FSV at `/mnt/d/contextgraph/fsv/phase-h-scripts-config-inner-llm-residue-fsv/scripts_config_inner_llm_residue_fsv.json` |

**Phase H status:** complete. GitHub issue readback on 2026-05-13 showed no open `phase:h` issues after #136 and #140 closed.

---

## Total task count: **~115 atomic tasks across 13 domains.**

| Domain | Task count | Effort sum |
|---|---|---|
| FLYWHEEL | 7 | ~5.5 days |
| PREDICT | 12 + 11 LABEL | ~12 days |
| ENRICH | 7 | ~13 days |
| LOOP | 7 | ~1.5 days |
| HEAL | 8 | ~6.5 days |
| CORPUS | 4 | ~10 days |
| OBS | 16 | ~12 days |
| UTML | 9 | ~3.5 days |
| TESTING | 11 | ~13 days |
| RUNBOOK | 12 | ~5.5 days |
| DR | 8 | ~4 days |
| SECURITY | 8 | ~3.5 days |
| BOOTSTRAP | 3 | ~2 days |
| API | 5 | ~2.5 days |
| **Total** | **~115** | **~95 days** |

> The original 8 phases (A–H per `12_IMPLEMENTATION_PHASES.md`) absorb the new TESTING / RUNBOOK / DR / SECURITY / BOOTSTRAP / API domains: TESTING + RUNBOOK + DR map mostly to Phase F (production observability) and Phase A (basic readiness); SECURITY tasks distribute across Phases A, B, E; BOOTSTRAP fits in Phases A and C; API stretches across A and F. The total fresh implementation effort grows from ~57 to ~95 engineer-days, then Phase G runs continuously.

> Per-domain task detail: see [`tasks/<domain>/_all_tasks.md`](tasks/) for compact entries; Phase A has detailed individual task files for the 8 critical-path items.

---

## Dependency DAG (high-level)

```
FLYWHEEL-001  ──┬─→ FLYWHEEL-002 ──→ HEAL-001 ──→ HEAL-002,003,004,005,007
                │
                ├─→ PREDICT-001 (injection hook)
                │
                ├─→ LOOP-001 ──→ LOOP-002,003
                │
                └─→ LOOP-007 (hook output paths)

FLYWHEEL-005,006 ──→ PREDICT-002 ──→ UTML-004

FLYWHEEL-003 ──→ FLYWHEEL-004 (oracle binding)

PREDICT-005 (multi-head) ──┬─→ PREDICT-003,004,006,007
                            ├─→ PREDICT-LABEL-009
                            ├─→ UTML-001 ──┬─→ UTML-002,003,005,006,008
                            │              └─→ UTML-007 ←── HEAL-001
                            └─→ OBS-004,009

CORPUS-001 ──→ CORPUS-003 ──→ CORPUS-004 ←── CORPUS-002

ENRICH-001 ──→ ENRICH-002 ──→ ENRICH-003
            ──→ ENRICH-004,005,006

OBS-001 ←── HEAL-002, UTML-009

LOOP-* (cleanup) — independent except LOOP-002 ←── LOOP-001
```

---

## Parallelization opportunities

These tasks can run in parallel without conflict:

- Phase C is complete; future corpus broadening can parallelize by language if adding scraped/public CI corpora beyond the deterministic fixture seed.
- Future Phase G model-quality experiments by cell/language/head, if the 0.95 convergence gate needs targeted retraining pressure.
- Cleanup tasks in Phase H — all independent

Phase F and Phase H no longer have parallelizable implementation work. A two-engineer pairing is now only relevant for Phase G convergence experiments and optional future corpus broadening.

---

## Atomic task file convention

Each `tasks/<domain>/TASK-<DOMAIN>-NNN-<slug>.md` follows the [`prdtospec.md`](../../docs2/prdtospec.md) Level-4 format:

```yaml
---
task_id: TASK-EXAMPLE-001
title: Example task title
phase: A
status: open|code_complete|complete
blocked_by: []
blocks: [TASK-EXAMPLE-002]
effort: 0.5d
spec_refs: [SPEC-EXAMPLE.md, TECH-EXAMPLE.md]
---

## Goal
...

## Inputs (read these)
- crates/context-graph-mcp/src/main.rs
- crates/context-graph-mejepa-shift-subscriber/src/lib.rs
- ...

## Outputs (write these)
- crates/context-graph-mcp/src/daemon.rs (NEW)
- crates/context-graph-mcp/src/main.rs (extended with --daemon flag)
- scripts/systemd/mejepa-daemon.service (NEW)

## Definition of done
- ...

## FSV evidence path
- /mnt/d/contextgraph/fsv/mcp-daemon-fsv/...
```

The atomic task files are under [`tasks/`](tasks/) — see the directory tree at `tasks/<domain>/`.

---

## Phase G+ — LeCun JEPA Alignment (TASK-FP-101..105)

> Source: [`28_LECUN_JEPA_ALIGNMENT.md`](28_LECUN_JEPA_ALIGNMENT.md) + GitHub EPIC #314 (companion to fingerprint EPIC #309).

| ID | Title | Pillar | Phase | Blocked by | Effort |
|---|---|---|---|---|---|
| `TASK-FP-101` | SIGReg regularizer for predictor (differentiable characteristic-function sketch beside VICReg) | 2 — anti-collapse info-max | G+ | ✅ complete; FSV `/mnt/d/contextgraph/fsv/task-fp-101-sigreg-fsv/run-1779036305137/sigreg_fsv.json` sha256 `b2d4995d4de9539916558f51cdd1423081af94962868c18077653c0d075e80c7` | shipped 2026-05-17 |
| `TASK-FP-102` | Hierarchical JEPA predictor (file → function → AST → chunk) | 3 — hierarchical | G+ | ✅ complete; FSV `/mnt/d/contextgraph/fsv/task-fp-102-hierarchical-jepa-fsv/run-1779037368822/hierarchical_jepa_fsv.json` sha256 `5dfce2ebe37d2b52a6ad036ec89e22f659c014fa3ff9c543b123000f2d7f9bf1` | shipped 2026-05-17 |
| `TASK-FP-103` | Latent-space candidate-patch search (`mejepa_search_latent_actions`) | 5 — latent planning | G+ | ✅ complete; FSV `/mnt/d/contextgraph/fsv/task-fp-103-latent-search-fsv/run-1779038591152/latent_search_fsv.json` sha256 `f31de8c1d4a0035731170e4a24640dc04d16095c2525ceacde84e7b0c8167e4e` | shipped 2026-05-17 |
| `TASK-FP-104` | Objective-driven cost + hardwired safety constraints | 6 — objective + safety | G+ | ✅ complete; FSV `/mnt/d/contextgraph/fsv/task-fp-104-objective-safety-fsv/run-1779040194889/objective_safety_fsv.json` sha256 `31cda8fa8c7bb0ce980947786b42d104f6ea1d2cde42a6bef88d6cc1aff288a1` | shipped 2026-05-17 |
| `TASK-FP-105` | `mejepa_rank_candidates` MCP tool (counterfactual repair ranking) | 7 — best-of-K | G+ | ✅ complete; FSV `/mnt/d/contextgraph/fsv/task-fp-105-rank-candidates-fsv/run-1779041036578/rank_candidates_fsv.json` sha256 `9502a7a753df459eabe671c2d8e293b6f5de9a1be5d652a18ea9adde206c5dea` | shipped 2026-05-17 |

Pillars 1 (frozen panel), 2 (anti-collapse SIGReg), 4 (action-conditioned), 8 (calibrated uncertainty), and 9 (System 1 vs 2 framing) are SHIPPED/DESIGNED with FSV-backed implementation where code changed and doc/tool readback where wording changed. Pillar 10 (reward-signal completeness) is the TASK-RWD-201..212 set below.

---

## Phase G+ — Reward-Signal Completeness (TASK-RWD-201..212)

> Source: [`29_REWARD_SIGNAL_COMPLETENESS.md`](29_REWARD_SIGNAL_COMPLETENESS.md) + GitHub EPIC #315.

| ID | Title | Tier | Phase | Adopts | Effort |
|---|---|---|---|---|---|
| `TASK-RWD-201` | `E_Cyclomatic` instrument (cyclomatic / cognitive / Halstead / depth) | 4 — static quality | G+ | ✅ complete; FSV `/mnt/d/contextgraph/fsv/task-rwd-201-cyclomatic-fsv/run-1779042449826/cyclomatic_fsv.json` sha256 `293d939dc2cccf5a70890be1154986e01b21289e1172714b4e694d9a9c68e2e8` | shipped 2026-05-17 |
| `TASK-RWD-202` | `E_Coverage` instrument (line-level coverage delta per file) | 4 — static quality | G+ | ✅ complete; FSV `/mnt/d/contextgraph/fsv/task-rwd-202-coverage-fsv/run-1779043488412/coverage_fsv.json` sha256 `dc72ce7768a76dee175bbf4af5131004b421ad999ba699a9ac7a5c50ce98fa0d` | shipped 2026-05-17 |
| `TASK-RWD-203` | `E_DiffTopology` instrument (hunks per file, size distribution, entropy) | 4 — static quality | G+ | ✅ complete; FSV `/mnt/d/contextgraph/fsv/task-rwd-203-diff-topology-fsv/run-1779043047838/diff_topology_fsv.json` sha256 `749b120421d37413a2ae54d67ad18f755855dcd337d30dbbb75dd99b3aa98c60` | shipped 2026-05-17 |
| `TASK-RWD-204` | `E_SecuritySignals` instrument (auth / input-sanit / secret patterns) | 4 — static quality | G+ | ✅ complete; FSV `/mnt/d/contextgraph/fsv/task-py-g-046-q4-security-label-fsv/run-1779047326597/q4_security_label_fsv.json` sha256 `f2481f3c3e7221baf78a2b0654187621dc7bf6d46950373e8e6fcaf281f45ed6` | shipped 2026-05-17 |
| `TASK-RWD-205` | `E_TypeGraph` full instrument (call-site type evidence) | 3 — static graph | G+ | ✅ complete; FSV `/mnt/d/contextgraph/fsv/task-rwd-205-type-graph-fsv/run-1779054424421/type_graph_fsv.json` sha256 `8accd4429949c27910ee1132ba5bc9d3f41aecbe709322596846e01a698ae0f0` | shipped 2026-05-17 |
| `TASK-RWD-206` | `E_Telemetry` instrument (latency / VRAM / cost prediction features) | 5 — runtime telemetry | G+ | ✅ complete; FSV `/mnt/d/contextgraph/fsv/task-rwd-206-telemetry-fsv/run-1779055248001/telemetry_fsv.json` sha256 `9006bdcc1e5376e39e2ecddccc8c3b78061f2bb18eee7b9956a196694a753016` | shipped 2026-05-17 |
| `TASK-RWD-207` | `E_LiveSession` instrument (session state + prompt/session context) | 7 — live session | G+ | ✅ updated by #360; stores prompt transcript hash, dominant prompt terms, prior verdict rows, and operator override rows without goal or intent embeddings | shipped 2026-05-17; corrected 2026-05-18 |
| `TASK-RWD-208` | Static-analysis (mypy + pyright + ruff) reward signal | 4 — static quality | G+ | ✅ complete; FSV `/mnt/d/contextgraph/fsv/task-py-g-074-static-analysis-fsv/run-1779045296443/static_analysis_fsv.json` sha256 `16c9ee15273c9df3d5bc03a9bfd7d46113d58266f49fa7af571b3f5f031624b0` | shipped 2026-05-17 |
| `TASK-RWD-209` | Coverage + churn + complexity + doc + dep-CVE reward | 4 — static quality | G+ | ✅ complete; FSV `/mnt/d/contextgraph/fsv/task-py-g-075-composite-reward-fsv/run-1779049005345/composite_reward_fsv.json` sha256 `f0eb9b5c5cf9a401f4911da30ed053db7a948bc8fc72e899d3837415084669db` | shipped 2026-05-17 |
| `TASK-RWD-210` | Prioritised replay buffer for M(t) consolidation | 7 — live session | G+ | ✅ complete; FSV `/mnt/d/contextgraph/fsv/task-py-g-068-replay-buffer-fsv/run-1779049993557-3834881/replay_buffer_fsv.json` sha256 `cf4f4a68080b127d7af07e5ae66c85d99c585fd999442063fc256f15d0689b77` | shipped 2026-05-17 |
| `TASK-RWD-211` | Novel-pattern detection + auto constellation extension | 6 — constellation | G+ | ✅ complete; FSV `/mnt/d/contextgraph/fsv/task-py-g-067-novel-pattern-fsv/run-1779051462932/novel_pattern_fsv.json` sha256 `6043269fa9340a611939e454e358f4b4437a82c6ff5afb3c990247749a288743` | shipped 2026-05-17 |
| `TASK-RWD-212` | Per-cell constellation freshness audit + scheduler | 6 — constellation | G+ | ✅ complete; FSV `/mnt/d/contextgraph/fsv/task-py-g-052-constellation-freshness-fsv/run-1779052689893/constellation_freshness_fsv.json` sha256 `eeaae4c68658fad63615e2622030f1f3fe1f3f17f51ee071d65b0b450a6502af` | shipped 2026-05-17 |

**Completeness gate**: ✅ complete as of 2026-05-17. `mcp__cgreality__mejepa_audit_reward_signals` reads durable RocksDB source-of-truth CFs, requires row-backed per-tier coverage, reports fingerprint feature-span / replay / active-learning / prediction-verification / freshness status, and samples fail-closed `CF_MEJEPA_SIGNAL_DROP_LOG` rows. FSV `/mnt/d/contextgraph/fsv/task-rwd-315-reward-audit-fsv/run-1779062502366-35012/reward_signal_audit_fsv.json` sha256 `c271752d7177a4bf40588fda0c8eedef53f404f0e68155e801db8a57da4467e8` (`all_passed=true`, 8/8 tiers at `1.0`, missing CFs block acceptance, corrupt signal-drop rows fail closed). See `29_REWARD_SIGNAL_COMPLETENESS.md §5`.

The atomic task files are under [`tasks/`](tasks/) — see the directory tree at `tasks/<domain>/`.
