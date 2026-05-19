# 07 — Results, Benchmarks, and FSV Evidence History

**Last refreshed:** 2026-05-19.

Source: aiwonder `/zfs/archive/contextgraph/fsv/` for post-migration production evidence; `/mnt/d/contextgraph/fsv/` only for legacy/local evidence; git log (`git log --oneline --all`); GitHub issue comments; per-issue RESOLVED comments. #369 tracks remaining legacy-local revalidation.

**Purpose:** preserve the proof-of-work that future agents would otherwise lose to time. Every entry here references a physical artifact (FSV JSON, sha256, CF) that can be re-verified.

---

## 1. Current Ship-Gate Status (2026-05-19)

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
| Stop-hook circuit breaker (TASK-PY-G-080 / #381) | preserves fail-closed unverified prediction blocking while writing bounded BLOCKED/operator-action artifacts for CLI-missing, DB-missing/stale, daemon-unreachable, verify-timeout, and repeated-timeout cases | PASS | `/zfs/archive/contextgraph/fsv/task-py-g-080-stop-hook-circuit-breaker-fsv/run-20260519T031222Z-457165/stop_hook_circuit_breaker_fsv.json` (sha256 `20b75ce31fa8ada4349fcd7ed0da657a7e8c6e4b32d709d66f00772bebab32d5`), independent readback sha256 `2f4b33d8941964b96870823f013815f4a3211dc0dbacf8bfb04453b0b50391cb` |
| Observe missing-prediction quarantine (#337) | fail-closed CF_MEJEPA_SIGNAL_DROP_LOG | shipped | `/mnt/d/contextgraph/fsv/task-py-g-337-observe-missing-prediction-fsv/run-1779067066572/observe_missing_prediction_fsv.json` (sha256 `66427f65…`) |
| Per-cell constellation freshness (TASK-PY-G-052) | scheduler integration | shipped | `/mnt/d/contextgraph/fsv/task-py-g-052-constellation-freshness-fsv/run-1779052689893/constellation_freshness_fsv.json` |
| Q1 claim graph extractor (TASK-PY-G-041) | bytes-on-disk reconciliation | shipped | `/mnt/d/contextgraph/fsv/task-py-g-041-claim-graph-extractor-fsv/run-1779077268576/claim_graph_extractor_fsv.json` |
| Per-head DDA projection (TASK-PY-G-043) | 30-chunk FSV | shipped | `/mnt/d/contextgraph/fsv/task-py-g-043-per-head-dda-projection-fsv/run-1779078323947/head_projection_fsv.json` |
| ConstellationObservation schema (TASK-PY-G-086 / #386) | Slot-map schema enumerates every packed panel slot, keeps per-slot hashes/provenance/calibration cells, stores pairwise relationships separately, and rejects missing/duplicate/wrong-dim/non-finite/unknown-field/calibration-drift cases | PASS | `/zfs/archive/contextgraph/fsv/task-py-g-086-constellation-observation-schema-fsv/constellation_observation_schema_fsv.json` (sha256 `9c1b6b870373472b3f8ab1fb999b6e87c62895c828337f0e2fc687c182e8ce11`) |
| Slot-preserving late-fusion predictor (TASK-PY-G-087 / #387) | Live predictor no longer uses early `linear(CONCAT_INPUT_DIM, hidden_dim)` as the semantic path; inputs encode each instrument slot separately, late-fuse after slot identity is explicit, and decode outputs per slot. Auxiliary heads now emit per-slot logits before late class fusion. No-compensation trace proves one bad slot cannot be hidden by aggregate distance | PASS | `/zfs/archive/contextgraph/fsv/task-py-g-087-slot-preserving-late-fusion-predictor-fsv/slot_preserving_late_fusion_predictor_fsv.json` (sha256 `9f2c5380794420ca5c4a6a11ca819b731377003b0fbe806f9b2463ab8c6b8d8a`), independent readback sha256 `3a597d3a92160f549785593a86d71f73380809f8f721907cb648463c9c48db12` |
| Real-corpus constellation smoke (TASK-PY-G-095 / #395) | 7 spread aiwonder shards, 12 active embedders, 2,688 packed records, 84 per-cell centroid rows, 66 pairwise relationships, 20 candidate failure-shape rows; `flat_vector_concat_used=false` | PASS | `/zfs/archive/contextgraph/fsv/task-py-g-095-real-corpus-constellation-smoke-fsv/python_real_corpus_constellation_smoke_fsv.json` (sha256 `552de73b…`), independent readback `/zfs/archive/contextgraph/fsv/task-py-g-095-real-corpus-constellation-smoke-fsv/independent_readback.json`, external edge readback `/zfs/archive/contextgraph/fsv/task-py-g-095-real-corpus-constellation-smoke-fsv/external_edge_case_readback.json` |
| Constellation training objective smoke (TASK-PY-G-096 / #396) | Real aiwonder 391-shard forward-cache joined to the 300×8 oracle index; 353 training rows, 1,440 per-slot vector rows, 2,727 row-pair observations, 66 pairwise relationships; per-slot/pairwise/contradiction/temporal/oracle/anti-compensation certificate; local baseline ladder plus historical 0.866667 context; `flat_vector_concat_used=false`; promotion refused | PASS | `/zfs/archive/contextgraph/fsv/task-py-g-096-constellation-training-objective-fsv/python_constellation_training_objective_fsv.json` (sha256 `9a58e747…`), certificate sha256 `a5ec541d…`, independent readback `/zfs/archive/contextgraph/fsv/task-py-g-096-constellation-training-objective-fsv/independent_readback.json` (sha256 `52498f5b…`), external edge readback sha256 `37f1dc39…` |
| Constellation capability-discovery ablation suite (TASK-PY-G-093 / #393) | Consumes #396 arrays; 3 baselines + 6 ablations over 353 rows; flat-vector baseline is comparison-only and never ship-gate eligible; every ship-gate-eligible #396-derived scalar constellation ablation is demoted vs the local baseline ladder; Q4/Q5/OOD label gaps are explicit, not fabricated | PASS | `/zfs/archive/contextgraph/fsv/task-py-g-093-constellation-ablation-suite-fsv/python_constellation_ablation_suite_fsv.json` (sha256 `d74f8eae…`), report sha256 `6fad7569…`, metrics sha256 `5a7118c9…`, independent readback sha256 `a1870713…`, external edge readback sha256 `3924c948…` |
| Cross-embedder constellation tensors (TASK-PY-G-088 / #388) | Real aiwonder 391-shard forward-cache sampled across 8 spread shards; 12 active slots preserved; 516 per-cell/window/slot distribution tensors with centroid and variance vectors; 2,838 per-cell/window upper-triangle pair tensors; 1,400 valid pair rows; 8 full cell/windows with all 66 active pair rows; surfaces classified as redundant/independent/mixed/contradictory/unsupported; `flat_vector_concat_used=false` | PASS | `/zfs/archive/contextgraph/fsv/task-py-g-088-cross-embedder-correlation-tensors-fsv/python_constellation_cross_embedder_tensors_fsv.json` (sha256 `a547af92…`), artifacts under `/zfs/archive/contextgraph/corpus/constellation-tensors-v1/`: slot tensors sha256 `57885d37…`, pairwise tensors sha256 `310a4e5e…`, report sha256 `91dc0f98…`, manifest sha256 `e6a07bb1…` |
| Per-slot calibration and OOD guard (TASK-PY-G-089 / #389) | Consumes #388 tensors plus #396 training rows; emits 12 global slot calibrations, 84 fail-closed per-cell slot rows, 66 pair calibrations, strict conjunction OOD policy, and edge proof that missing/stale/nonfinite/one-bad-slot panels reject even when an aggregate score would pass; `RealityPrediction.per_slot_ood_reasons` now persists slot/embedder/calibration-specific OOD reasons and MCP explain/inspect surfaces expose them; `flat_vector_concat_used=false`; not ship-gate-countable until #127/#129/#130 rerun under this guard | PASS | Calibration FSV: `/zfs/archive/contextgraph/fsv/task-py-g-089-per-slot-calibration-ood-fsv/python_per_slot_calibration_ood_fsv.json` (sha256 `24075e0b…`); live-surface FSV: `/zfs/archive/contextgraph/fsv/task-py-g-089-per-slot-ood-reasons-live-fsv/run-1779156050260/task_py_g_089_per_slot_ood_reasons_live_fsv.json` (sha256 `198bc651ef5075c30676e80625cd8600fe36c3bbc0e1d47ade4662537700a924`); artifacts under `/zfs/archive/contextgraph/state/calibration/per-slot-v1/`: per-slot sha256 `f5ef54b4…`, per-cell sha256 `19e88e2e…`, per-pair sha256 `ef149e91…`, policy sha256 `6d95857a…`, report sha256 `71781512…` |
| Calibrated constellation ablation rerun (TASK-PY-G-097 / #398) | Consumes #396 arrays, #388 tensor store, and #389 per-slot calibration/OOD state; emits 3 baselines plus 7 ablations, 12 per-slot contribution metrics, and all 66 per-pair contribution rows with insufficient-support markers where needed; proves `flat_vector_concat_used_for_ship_gate_eligible_paths=false`; proves missing/stale/not-global-ready calibration fails closed; proves one-bad-slot rejection still wins even when aggregate mean would pass; recommendation is `no_promotion` because calibrated full constellation Brier `0.7539461538461542` does not beat best local baseline Brier `0.008555205797287593` | PASS | `/zfs/archive/contextgraph/fsv/task-py-g-097-constellation-ablation-with-calibration-fsv/python_constellation_ablation_with_calibration_fsv.json` (sha256 `90039db274eb4e37b4fc03ed3bb115b184149c00f3cf2fc684118dff492b854e`), report sha256 `a65b320ac08208d26eaeb57eefb90e7bdd5e994d0662527edb399d7ffa06ed35`, metrics sha256 `dc4e3cbe7ec2f95e029030e4fc9550f8a01791267c6b6618d5482bfe4937ad11`, independent readback sha256 `8b1c0f147692179ce3e50b29a90208dcf143ba8e666f8869d588f4801799d0b6`, external edge readback sha256 `bc7fa442c8e05aa9b5574e3d2a9e95465bc8eedc01282fcb80bf3220964ce473` |
| Anti-compensation FSV suite (TASK-PY-G-094 / #394) | Consumes #388 tensor dimensions and #389 per-slot/pair calibration; emits six adversarial fixtures proving corrupted, missing, stale, and dimension-shifted slots plus contradictory pair and temporal-jump cases cannot be accepted through aggregate evidence; each fixture writes prediction, OOD, Gτ, Q4, active-learning, and explanation surfaces; code scan prevents flat-vector ship-gate eligibility regressions | PASS | `/zfs/archive/contextgraph/fsv/task-py-g-094-anti-compensation-fsv/python_anti_compensation_fsv.json` (sha256 `57f710b98fab31d2548bc6da8397487ac960f7cd02d9659df5335f0b017d32a1`), report sha256 `1d68f304dfc2356633596e05eacdf3ae61f89a053fd5c224a830cb9ea9477047`, fixture rows sha256 `ccd5cd7745b307894c6ee4622c812dfd7a95cb1dfaef684cbaaca0ae4c88dc57`, active-learning rows sha256 `edc62bb8fb0728e4660bfe60039108580fc65cf45cc06bad198b88e458a54f26`, independent readback sha256 `d6b57ec03f1af887d89192925cc3b84177e5857a77075072ae659a83ef7555c5`, external edge readback sha256 `c540a9ddca6a7c2cf96624d8bdb2c9433d6c416e36600a3d4053ca9d98a16131` |
| Live-session OOD corpus harvest (TASK-PY-G-057 / #276) | Harvests live predictions above the OOD threshold into `CF_MEJEPA_OOD_HARVEST`, writes OOD calibration reports to `CF_MEJEPA_OOD_CALIBRATIONS`, projects harvested rows into `CF_MEJEPA_ACTIVE_LEARNING_QUEUE` at 4x weight, and provides review rows with RocksDB byte anchors; aiwonder FSV seeds 50 live rows, harvests 5 deliberate OOD rows, quarantines orphan/degenerate panels, tiers down expired rows, downweights operator-labeled in-distribution rows, and flags `OOD_GATE_OVER_FLAGGING` at false-positive-rate `0.5` | PASS | `/zfs/archive/contextgraph/fsv/task-py-g-057-ood-harvest-fsv/run-20260519T022705Z/ood_harvest_fsv.json` (sha256 `bc3f9519f157f657fba9fea573fb2dfef4c256221f6a4ffdfec949e88b987b60`) |
| Secret/delayed holdout Goodhart guard (TASK-PY-G-077 / #374) | Guard protocol is green but current production conclusion is fail-closed `underpowered`: no complete four-window train-visible public gate exists yet. Corpus split proof shows train-visible IDs `2160`, holdout IDs `240`, overlap `0`; the guard blocks #165 if public windows are green while holdout evidence is missing, red, overlapping, or lacks stable IDs | PASS / underpowered | `/zfs/archive/contextgraph/fsv/task-py-g-077-secret-holdout-fsv/python_secret_holdout_fsv.json` (sha256 `16c1a5a68fe2c79616410ad204e7843c86f5da333b7662772fd3e0d8c814233e`), independent readback sha256 `0a429bc62b95ac25f198acd04822c81744632c628a5dc8e6b8c552fb80205e03` |
| Oracle-noise and ship-gate statistical-power bound (TASK-PY-G-076 / #376) | Fail-closed result: `underpowered`. Current `/zfs/archive` evidence lacks repeated-oracle rows for the 300×8 corpus and lacks four consecutive production gate windows with paired prediction/oracle rows; #165/#167 cannot claim `>= 0.95 x 4 windows` yet | PASS / underpowered | `/zfs/archive/contextgraph/fsv/task-py-g-076-oracle-noise-power-fsv/python_oracle_noise_power_fsv.json` (sha256 `1324dab06dea5228923265514f5c9fb1d566ddaf6e59ab143b3417426734bae7`), independent readback sha256 `d7c707f73756533c872f0b2375f603851bf0c6e5c18c11ae08e960998da2885f` |
| Q4 fail-closed trust gate (TASK-PY-G-078 / #377) | Raw Q4 field presence no longer implies trust. The gate requires each Q4 head to have producer rows, Q4 calibration rows, and exact active-slot evidence for `e1,e2,e3,e4,e6,e7,e8,e9,e10,e12,e13,e14`; missing producers, producer-without-calibration, calibration-without-slots, and wrong-slot names fail closed; single-head support unlocks only that head; all-head durable-catalog support unlocks Q4 trust eligibility | PASS | `/zfs/archive/contextgraph/fsv/task-py-g-078-q4-trust-gate-fsv/q4_trust_gate_fsv.json` (sha256 `0af1726a6e104e7cb70c694c22e40a91b96dc6c323b3679f827d86617aeca30a`), independent readback sha256 `5d4b752ec5fca9eda24fe78dbee649e9a00cb20c1435e3a2f65860886f75bcaa` |
| Q4 perf label producer (TASK-PY-G-047 / #266) | Real bounded `python3` benchmark + cProfile fixture emits pytest-benchmark-compatible timing JSON and real `.prof` artifacts; 30 raw artifact rows, 60 canonical `CF_MEJEPA_Q4_PERF_LABELS` rows, direct `(corpus_row_id, metric)` readback, unstable/no-timing/budget/improvement edges green. Q4 perf producer support is true, but Q4 trust remains false because #127 calibration and separately verified slot evidence are still required | PASS / producer-only | `/zfs/archive/contextgraph/fsv/task-py-g-047-q4-perf-label-producer-fsv/q4_perf_label_producer_fsv.json` (sha256 `8fdd89be848a1c8f0420a13d9c13e326cfd23377d89c4e88d9c61fca50880a83`), independent readback sha256 `e19fe0561f28d7bedf445711640318d2af26b11192fbe5dc7d2b427ee9ee0ae3` |
| Q4 perf real-analyzer re-proof (TASK-PY-G-098 / #399) | Re-runs the #266 perf producer harness under a separate aiwonder FSV root with real bounded `python3` benchmark and cProfile commands, real `.prof` artifacts, 30 raw artifact rows, 60 `CF_MEJEPA_Q4_PERF_LABELS` rows, and external `ldb` readback of deterministic key `task-py-g-098-1779173936485:label:test_linear`. The re-proof decouples its evidence root from the shared Q4 catalog, which remains anchored to canonical #266 producer root; Q4 perf trust remains false without #127 calibration and exact-slot evidence | PASS / re-proof | `/zfs/archive/contextgraph/fsv/task-py-g-098-q4-perf-real-analyzer-fsv/q4_perf_real_analyzer_fsv.json` (sha256 `69d84cf47b35f45b61dc9ee49ee1f2700538459dad6c69386d5b7551cc00df63`), independent readback sha256 `c4ac201239aef4c94af135ca31e1ca1478b6d58b0a19ecea749abd8336f70107` |
| Q4 accuracy label producer (TASK-PY-G-048 / #267) | Real bounded `python3` metric fixture emits pre/post metric JSON; 30 raw artifact rows, 30 canonical `CF_MEJEPA_Q4_ACCURACY_LABELS` rows, source-test-aware `(corpus_row_id, metric, source_test)` readback, duplicate metric preservation, loss/error lower-is-better direction, parse/unstable/missing-baseline quarantines, and quarantine reopen proof are green. Q4 accuracy producer support is true, but Q4 trust remains false because #127 calibration and separately verified slot evidence are still required | PASS / producer-only | `/zfs/archive/contextgraph/fsv/task-py-g-048-q4-accuracy-label-producer-fsv/q4_accuracy_label_producer_fsv.json` (sha256 `6ad4e68d5f5cfb229b5da918529dc6fc6c40c97b2226e9b2365aa598a58295a4`), independent readback sha256 `0575ee3c74c2935eab30ed6d7a517ee56c4c78670a9d2be578a837bd315a9d26` |
| Q4 cost label producer (TASK-PY-G-049 / #268) | Real bounded `python3` cost fixture materializes pre/post requirements and wheel artifacts, verifies the accepted patch shape adding `numpy==2.0` and `scipy==1.14`, parses pytest-compatible wall-clock output plus requirements/wheel metadata, and writes 30 raw artifact rows plus 90 canonical `CF_MEJEPA_Q4_COST_LABELS` rows. Dependency removal/improvement direction, manifest-precondition quarantine, missing-manifest absence, wheel-build failure quarantine, noisy walltime CI-only quarantine, prediction-surface adapter, and Q4 evidence-catalog update are green. Q4 cost producer support is true, but Q4 trust remains false because #127 calibration and separately verified slot evidence are still required | PASS / producer-only | `/zfs/archive/contextgraph/fsv/task-py-g-049-q4-cost-label-producer-fsv/q4_cost_label_producer_fsv.json` (sha256 `10a53f7b75b938e53989dc0059cb40bd632ba5d232f58794375b63b5b087de93`), independent readback sha256 `f2f249ad24aeec070649306d3e598bc3c5c30e0c9d649b40fb3fba7b37f93d60` |
| Q4 reasoning-class label producer (TASK-PY-G-050 / #269) | Observed agent-prose / oracle-outcome harvest fixture writes 300 canonical `CF_MEJEPA_Q4_REASONING_LABELS` rows plus 5 producer-heldout `CF_MEJEPA_HEAD_CALIBRATIONS` rows; centroid checkpoint under `/zfs/archive/contextgraph/state/mejepa-heads/reasoning-head-v1/`; `Hedging` F1 `1.0`, `Overclaiming` F1 `1.0`; empty prose → `none`, code-only → `code_only`, non-English → `unsupported`, harvest `<200` → `unknown_insufficient_harvest`. Q4 reasoning producer support is true, but Q4 trust remains false because #127 calibration and separately verified slot evidence are still required | PASS / producer-only | `/zfs/archive/contextgraph/fsv/task-py-g-050-q4-reasoning-label-producer-fsv/q4_reasoning_label_producer_fsv.json` (sha256 `788be9c14a4f0eeb5c73bf231fe48d470d3476883f69cfbe53b5a27652aaaf6b`), independent readback sha256 `a02021559110c364e3a5ecbf82e6db046d142a44e0c312c6078705b3ec57cddc` |

**Q4 producer proof note (2026-05-19):** #399 is closed as a real-analyzer re-proof for perf; #400 remains the open p1 follow-up to re-prove the cost producer from real pytest/requirements/wheel artifacts and manifest guards before #127 consumes cost as calibration-ready evidence. #266/#268 remain producer-only context and not ship-gate trust evidence until calibration and exact-slot evidence exist.

### 1.1 Aiwonder Corpus Data Map For Constellation Training

The active Python ship-gate data lives in three separate aiwonder artifacts that must not be confused:

| Artifact | Path | Role |
|---|---|---|
| 300×8 SWE-bench Lite Python corpus + oracle labels | `/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json` | 2,400 entries; `oracle_all_passed` is the #396 label source, joined by `(task_instance_id, mutation_category)` |
| AST chunk store | `/zfs/hot/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl` | Chunk metadata and stable row keys: `task_instance_id`, `workspace_state`, `chunk_id`, `source_sha256` |
| Full packed embedder forward cache | `/zfs/archive/contextgraph/corpus/python-forward-cache-v1-aiwonder-prod-20260515-191116Z/` | 391 completed shards; per-shard packed manifests point to each embedder's record/index JSONL; active embedders are `e1,e2,e3,e4,e6,e7,e8,e9,e10,e12,e13,e14` |
| Cross-embedder constellation tensor store | `/zfs/archive/contextgraph/corpus/constellation-tensors-v1/` | TASK-PY-G-088 output: `slot_distribution_tensors.jsonl`, `pairwise_correlation_tensors.jsonl`, `constellation_tensor_report.json`, `constellation_tensor_manifest.json`; FSV at `/zfs/archive/contextgraph/fsv/task-py-g-088-cross-embedder-correlation-tensors-fsv/python_constellation_cross_embedder_tensors_fsv.json` |
| Per-slot calibration and OOD guard | `/zfs/archive/contextgraph/state/calibration/per-slot-v1/` | TASK-PY-G-089 output: `per_slot_calibration.jsonl`, `per_cell_slot_calibration.jsonl`, `per_pair_calibration.jsonl`, `ood_guard_policy.json`, `per_slot_calibration_report.json`; calibration FSV at `/zfs/archive/contextgraph/fsv/task-py-g-089-per-slot-calibration-ood-fsv/python_per_slot_calibration_ood_fsv.json`; live `RealityPrediction.per_slot_ood_reasons` FSV at `/zfs/archive/contextgraph/fsv/task-py-g-089-per-slot-ood-reasons-live-fsv/run-1779156050260/task_py_g_089_per_slot_ood_reasons_live_fsv.json` |
| Slot-preserving predictor FSV | `/zfs/archive/contextgraph/fsv/task-py-g-087-slot-preserving-late-fusion-predictor-fsv/` | TASK-PY-G-087 output: `slot_preserving_late_fusion_predictor_fsv.json`, `independent_readback.json`, and no-compensation trace artifacts; validates per-slot encoders, late fusion, per-slot decoders, slot-aware auxiliary heads, and one-bad-slot rejection |
| Calibrated ablation report | `/zfs/archive/contextgraph/fsv/task-py-g-097-constellation-ablation-with-calibration-fsv/` | TASK-PY-G-097 output: `python_constellation_ablation_with_calibration_fsv.json`, `artifacts/constellation_ablation_with_calibration_report.json`, `artifacts/constellation_ablation_with_calibration_metrics.jsonl`, `independent_readback.json`, and `external_edge_case_readback.json`; result is `no_promotion` |
| Anti-compensation guard fixtures | `/zfs/archive/contextgraph/fsv/task-py-g-094-anti-compensation-fsv/` | TASK-PY-G-094 output: `python_anti_compensation_fsv.json`, `artifacts/anti_compensation_report.json`, `artifacts/anti_compensation_fixture_results.jsonl`, `artifacts/anti_compensation_active_learning_rows.jsonl`, `independent_readback.json`, and `external_edge_case_readback.json`; proves all six fixture paths remain non-ship-gate-eligible despite aggregate-pass conditions |
| Live-session OOD harvest FSV | `/zfs/archive/contextgraph/fsv/task-py-g-057-ood-harvest-fsv/run-20260519T022705Z/` | TASK-PY-G-057 output: `ood_harvest_fsv.json` plus `ood-harvest.rocksdb`; validates `CF_MEJEPA_OOD_HARVEST`, `CF_MEJEPA_OOD_CALIBRATIONS`, and `CF_MEJEPA_ACTIVE_LEARNING_QUEUE` readback |
| Secret/delayed holdout guard FSV | `/zfs/archive/contextgraph/fsv/task-py-g-077-secret-holdout-fsv/` | TASK-PY-G-077 output: `python_secret_holdout_fsv.json` and `independent_readback.json`; proves train-visible/holdout stable-ID disjointness and fails closed until four eligible train-visible plus secret/delayed holdout windows exist |
| Q4 trust gate FSV | `/zfs/archive/contextgraph/fsv/task-py-g-078-q4-trust-gate-fsv/` | TASK-PY-G-078 output: `q4_trust_gate_fsv.json`, `independent_readback.json`, and `q4_evidence_catalog_fixture.json`; proves raw Q4 observations are display-only until producer/calibration/exact-slot evidence exists |
| Q4 perf label producer FSV | `/zfs/archive/contextgraph/fsv/task-py-g-047-q4-perf-label-producer-fsv/` | TASK-PY-G-047 output: `q4_perf_label_producer_fsv.json`, `independent_readback.json`, real fixture scripts/profiles, and per-run RocksDB; validates `CF_MEJEPA_Q4_PERF_LABELS` canonical row/metric readback and producer-only Q4 catalog support |
| Q4 perf real-analyzer re-proof FSV | `/zfs/archive/contextgraph/fsv/task-py-g-098-q4-perf-real-analyzer-fsv/` | TASK-PY-G-098 output: `q4_perf_real_analyzer_fsv.json`, `independent_readback.json`, real fixture script/profiles, and per-run RocksDB; validates external `ldb` readback for `CF_MEJEPA_Q4_PERF_LABELS` key `task-py-g-098-1779173936485:label:test_linear` while preserving the canonical #266 Q4 catalog producer root |
| Q4 accuracy label producer FSV | `/zfs/archive/contextgraph/fsv/task-py-g-048-q4-accuracy-label-producer-fsv/` | TASK-PY-G-048 output: `q4_accuracy_label_producer_fsv.json`, `independent_readback.json`, real fixture script, raw metrics wrappers, and per-run RocksDB; validates `CF_MEJEPA_Q4_ACCURACY_LABELS` source-test-aware row/metric readback, duplicate metric preservation, quarantine persistence, and producer-only Q4 catalog support |
| Q4 cost label producer FSV | `/zfs/archive/contextgraph/fsv/task-py-g-049-q4-cost-label-producer-fsv/` | TASK-PY-G-049 output: `q4_cost_label_producer_fsv.json`, `independent_readback.json`, real fixture script, accepted patch fixture, raw cost wrappers, and per-run RocksDB; validates `CF_MEJEPA_Q4_COST_LABELS` kind-aware readback, CI/dependency/wheel cost labels, quarantine persistence, prediction-surface adapter output, and producer-only Q4 catalog support |
| Q4 reasoning label producer FSV | `/zfs/archive/contextgraph/fsv/task-py-g-050-q4-reasoning-label-producer-fsv/` | TASK-PY-G-050 output: `q4_reasoning_label_producer_fsv.json`, `independent_readback.json`, raw observed-prose harvest wrapper, checkpoint, and per-run RocksDB; validates `CF_MEJEPA_Q4_REASONING_LABELS`, producer-heldout `CF_MEJEPA_HEAD_CALIBRATIONS`, Q4 trust catalog producer support, and fail-closed edge classes |
| Q4 perf raw outputs | `/zfs/archive/contextgraph/corpus/python-q4-perf-labels-v1/` | TASK-PY-G-047 and TASK-PY-G-098 raw benchmark/cProfile JSON wrappers for each producer/re-proof row; cProfile binary `.prof` artifacts live under the matching FSV fixture directory |
| Q4 accuracy raw outputs | `/zfs/archive/contextgraph/corpus/python-q4-accuracy-labels-v1/` | TASK-PY-G-048 raw pre/post metric JSON wrappers for each producer row; generated by bounded real `python3` commands and linked from the FSV |
| Q4 cost raw outputs | `/zfs/archive/contextgraph/corpus/python-q4-cost-labels-v1/` | TASK-PY-G-049 raw pre/post cost JSON wrappers for each producer row; generated by bounded real `python3` commands and linked from the FSV |
| Q4 reasoning raw outputs | `/zfs/archive/contextgraph/corpus/python-q4-reasoning-labels-v1/` | TASK-PY-G-050 raw observed-prose harvest wrappers; the 2026-05-19 run wrote `task-py-g-050-1779172669266/reasoning_harvest.json` (sha256 `f356ea9fcedf6a96995e5fdc284f5e212adeb7f67eb7b92de72c5dd8efd8541a`) |

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
| TASK-PY-G-047 — Q4 perf labels (benchmark + cProfile) | ✅ shipped | `task-py-g-047-q4-perf-label-producer-fsv/` |
| TASK-PY-G-048 — Q4 accuracy labels | ✅ shipped | `task-py-g-048-q4-accuracy-label-producer-fsv/` |
| TASK-PY-G-049 — Q4 cost labels | ✅ shipped | `task-py-g-049-q4-cost-label-producer-fsv/` |
| TASK-PY-G-050 — Q4 reasoning labels | ✅ shipped | `task-py-g-050-q4-reasoning-label-producer-fsv/` |
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
