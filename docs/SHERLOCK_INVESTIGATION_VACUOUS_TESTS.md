# Sherlock Holmes Investigation — Vacuous Tests in ContextGraph / ME-JEPA-Code

**Case ID:** SHERLOCK-2026-05-19-VACUOUS-TESTS
**Date:** 2026-05-19
**Subject:** All `#[test]` / `#[tokio::test]` functions in the 22 active Rust workspace crates plus `tests/e2e/`
**Doctrine basis:**
- `CLAUDE.md` §15.13 — *"Never write tests that pass when the project is broken"*
- `docs/futurebuild/specs/FSV-PROTOCOL.md` §3.4 — *"Tests must fail when the feature is broken"*
- `docs2/AICodingAgentSuperPrompt.md` §4 — verification protocol (source-of-truth readback)

**Scope:** 5,966 `#[test]` / `#[tokio::test]` attribute sites across 22 active workspace members. `backupdocs/memory.archive/` excluded. `let _ = ...` discard-result patterns reviewed in 30 files. `#[ignore]` audit covers all 22 occurrences. CI workflow cross-reference covers all 8 `.github/workflows/*.yml` files.

**Method:** Linear Sequential Unmasking (LSU) — examined code BEFORE reading test docstrings / commit messages / PR descriptions. Cold Read each suspect; ran the Contradiction Engine (does the test name match what it actually proves?); for each HIGH finding generated ≥3 hypotheses for what production code mutation should kill the test, then verified one would NOT be caught.

---

## Executive Summary — Top 5 Worst Vacuous Tests (ranked by production failure they currently mask)

| Rank | Severity | File:Line | Test | What failure it masks |
|---|---|---|---|---|
| **1** | CRITICAL | `tests/e2e/soak.rs:541-556` | `task_test_008_soak_harness_repeats_load_and_verifies_nightly_timer` | "30-minute soak" never actually soaks. The `requested_minutes: 30` field is dead metadata — never consumed by a sleep, deadline, or duration budget. The test runs 2 cycles of the load harness (≈seconds total) and claims to verify p99 latency stability over a 30-minute window. **A real memory leak or thread-stall manifesting after >10 seconds would never trip this test.** |
| **2** | CRITICAL | `crates/context-graph-embeddings/src/warm/loader/tests/loader_tests.rs:173-177` | `test_fail_fast_on_allocation_error` | `if cuda_init_result.is_err() { return; }` — textbook doctrine §15.13 violation. The test asserts "fail-fast on VRAM exhaustion," but on every machine without CUDA (i.e. every Free-plan `runs-on: ubuntu-latest` runner), `initialize_cuda_for_test()` returns Err and the test **returns Ok before any assertion fires**. If `allocate_model_vram` is broken to leak VRAM silently instead of failing, this test passes on every CI runner. |
| **3** | HIGH | `.github/workflows/chaos.yml:56` (workflow bug, not a test) | (storage_exhaustion harness) | Chaos workflow looks for `crates/context-graph-mejepa/tests/storage_exhaustion.rs`. The actual shim lives at `crates/context-graph-mejepa-hygiene/tests/storage_exhaustion.rs`. **The chaos workflow silently skips storage-exhaustion testing**, prints `"Phase F chaos: no chaos harnesses present yet; skipping."` (false — they ARE present, just in the wrong crate) and returns success. TASK-TEST-005's `task_test_005_storage_exhaustion_recovers_within_quota` never runs in CI. |
| **4** | HIGH | `crates/context-graph-embeddings/tests/global_provider_fsv.rs:240-256` | `fsv_edge_long_queries` | Test runs 1000 iterations of `is_warm_initialized()` and `warm_status_message()`, discarding every return value with `let _ = ...`. Title says "verify repeated calls don't accumulate state incorrectly" but the test verifies nothing about state. If `is_warm_initialized()` is broken to leak Arc handles per call, or `warm_status_message()` is broken to always return `"Ready"` regardless of state, the test still passes. |
| **5** | HIGH | `tests/e2e/heal_ticker_telemetry.rs:87-103` | `task_test_011_heal_ticker_telemetry_persists_*` | The "scheduler panic" the test exercises is a **synthetic panic literally written by the test** inside `catch_unwind`, not a real scheduler internal failure: `panic!("synthetic scheduler panic for heal ticker telemetry FSV")`. The test then asserts `record_heal_scheduler_restart()` increments the counter. **It tests "if I call counter.inc(), the counter increments" — not "if the scheduler actually panics in production, the restart counter increments."** The heal-scheduler's real panic-detection path is never exercised. |

---

## Detailed Findings (Severity-ordered)

### CRITICAL

#### V-001 (CRITICAL) — Soak test doesn't soak
**File:** `tests/e2e/soak.rs:541-556` (+ shim at `crates/context-graph-mejepa/tests/soak.rs`)
**Test:** `task_test_008_soak_harness_repeats_load_and_verifies_nightly_timer`
**CI:** Runs in `.github/workflows/soak.yml:63` with `--include-ignored --nocapture`

**Suspect code:**
```rust
const DEFAULT_SOAK_MINUTES: u64 = 30;     // soak.rs:16
// ...
let profile = SoakProfile::try_new(
    requested_minutes_from_env()?,         // defaults to 30
    cycle_count_from_env()?.unwrap_or(2),
    DEFAULT_MAX_P99_GROWTH_RATIO,
)?;
// ...
pub struct SoakProfile {
    pub requested_minutes: u64,            // <-- only ever serialized to JSON report
    pub cycle_count: usize,
    pub max_p99_growth_ratio: f64,
    pub accelerated: bool,                 // <-- hardcoded `true`, never read for branching
}
```

**Why it cannot fail (or fails too weakly):**
`requested_minutes` is stored in the JSON report and used only to template the SoakExport payload (`export.requested_minutes = profile.requested_minutes`). It is NEVER used to drive a sleep, deadline, loop budget, or duration check. The actual workload is two iterations of the load harness — milliseconds-to-seconds, NOT 30 minutes. The `assert!(report.cycle_summaries.len() >= 2)` check is trivially satisfied (2 cycles always ran), and the cross-cycle p99 stability check (`p99_growth_ratio <= 2.0 || p99_jitter_ms <= 250`) compares the two short bursts to each other, not to any real long-running baseline.

**Hypotheses considered:**
1. The test is intentionally accelerated and the production soak runs via the systemd unit `mejepa-soak-nightly.service` only — *partially supported*: the test DOES verify the systemd timer config strings (lines 252-256), but it still mis-names itself as the soak test and is what CI actually exercises. The CI workflow `soak.yml` does NOT install or run the systemd unit. So the soak signal CI claims to ship is fictional.
2. The `accelerated: true` flag must downstream-affect a real timer — *eliminated*: grep for `accelerated` shows only the struct field and one `format!()` in the StepEvidence trigger string. Never branched on.
3. The cycle count is a credible proxy for soak duration — *eliminated*: max_cycles=4 and DEFAULT_CYCLES=2; under no reachable value does the test approach 30 minutes of work.

**Mutation that should — but does not — fail:** Introduce a slow leak in `MejepaCompiler::verify` that adds 50ms per call after the 1000th call (manifesting at ~minute 15 of real soak). The test runs ~50 calls per cycle × 2 cycles = ~100 calls and never trips the leak threshold.

**Doctrine citation:** CLAUDE.md §6.5 "GENERIC-ONLY in the trained model" — by analogy, integration tests should exercise the production scenario. FSV-PROTOCOL.md §5.13 "Boundary cases — Time edges."

---

#### V-002 (CRITICAL) — Early-return-on-error masquerading as fail-fast verification
**File:** `crates/context-graph-embeddings/src/warm/loader/tests/loader_tests.rs:159-201`
**Test:** `test_fail_fast_on_allocation_error`
**CI:** This test is `#[cfg(feature = "candle")]`. `candle` is in the crate's default features (line 91 of `crates/context-graph-embeddings/Cargo.toml`). **However, NO CI workflow runs `cargo test -p context-graph-embeddings`.** It is only ever built (`cargo build`) and would only run during a local `cargo test` invocation.

**Suspect code:**
```rust
let cuda_init_result = loader.initialize_cuda_for_test();

// If CUDA feature is not enabled, initialization returns Ok but allocator is None
// In that case, allocate_model_vram will return CudaInitFailed (expected behavior)
if cuda_init_result.is_err() {
    // CUDA init failed - this is acceptable in non-CUDA environments
    // The fail-fast behavior is working correctly
    return;
}
```

**Why it cannot fail:** When `initialize_cuda_for_test()` returns Err (the common case on any non-GPU host, including every ubuntu-latest runner), the test silently returns BEFORE any assertion runs. The comment "fail-fast behavior is working correctly" asserts something the test never actually verifies — the test merely exits.

**Hypotheses considered:**
1. CUDA init failure is genuinely orthogonal to allocation failure and should not block the test — *eliminated by doctrine*: the test claims to verify fail-fast on allocation error. If allocation cannot be tested without CUDA, the test should be `#[ignore]`-d on non-CUDA hosts, not silently passed.
2. The `WarmError::CudaInitFailed` match arm at line 192 catches the no-CUDA case — *eliminated*: that arm is only reached if `cuda_init_result.is_ok()` AND `allocate_model_vram` returns Err. When `cuda_init_result` is itself Err, the early `return` short-circuits before the match.
3. There's an alternative test that runs on non-CUDA hosts — *no evidence*; grep for `test_fail_fast_on_allocation_error_no_cuda` finds nothing.

**Mutation that should — but does not — fail:** Break `allocate_model_vram` to silently round down VRAM requests beyond the budget instead of returning Err. On non-CUDA hosts (every CI runner), the test still passes via the early return. On a developer's CUDA workstation, the test catches it. **The gate is non-deterministic by host capability.**

**Recommended fix:** Replace `if cuda_init_result.is_err() { return; }` with `#[ignore = "requires CUDA"]` annotation and split the no-CUDA path into its own test that explicitly asserts `WarmError::CudaInitFailed` is returned from `allocate_model_vram`.

---

### HIGH

#### V-003 (HIGH) — Chaos workflow looks in wrong crate for storage exhaustion shim
**File:** `.github/workflows/chaos.yml:54-65`
**Affected test:** `tests/e2e/storage_exhaustion.rs:600` (`task_test_005_storage_exhaustion_recovers_within_quota`)

**Suspect code (workflow):**
```yaml
for harness in \
  crates/context-graph-mejepa/tests/chaos_failpoints.rs \
  crates/context-graph-mejepa/tests/storage_exhaustion.rs; do
  if [[ -f "${harness}" ]]; then
    ...
  fi
done
if [[ "${ran_any}" -eq 0 ]]; then
  echo "Phase F chaos: no chaos harnesses present yet; skipping."
fi
```

**Reality (verified by `find` 2026-05-19):**
- `crates/context-graph-mejepa/tests/chaos_failpoints.rs` — EXISTS, gets run ✓
- `crates/context-graph-mejepa/tests/storage_exhaustion.rs` — **DOES NOT EXIST**
- `crates/context-graph-mejepa-hygiene/tests/storage_exhaustion.rs` — exists (the actual shim location)

**Why it cannot fail:** The chaos workflow conditional executes `cargo test -p context-graph-mejepa --test storage_exhaustion` only if the file path exists under the `-mejepa` crate. It doesn't, so the entire storage-exhaustion CI gate is silently inactive. `ran_any` stays `1` (chaos_failpoints ran), so the "no chaos harnesses" sentinel branch never triggers — the failure is invisible.

**Hypotheses considered:**
1. Storage exhaustion is intentionally not part of the chaos workflow — *eliminated by comments*: the workflow's leading comment names "TASK-TEST-005 (storage exhaustion)" as one of the targets.
2. There's a different chaos test runner — *eliminated by grep*: only `chaos.yml` references `storage_exhaustion`.
3. The shim was deliberately moved to `-mejepa-hygiene` for crate-ownership reasons and the workflow was updated separately — *partially supported*: the shim IS in the right crate from a code-ownership standpoint, but the workflow was never updated.

**Mutation that should — but does not — fail:** Break `quota_check_and_evict` to do nothing (no rows ever evicted). The unit test `task_test_005_storage_exhaustion_recovers_within_quota` would fail (its FSV ensures `report.evicted.is_empty() == false`). But the chaos CI workflow never runs it, so a PR could ship this bug.

**Recommended fix:** Update `chaos.yml:55` to read either:
- `crates/context-graph-mejepa-hygiene/tests/storage_exhaustion.rs` (and change `-p context-graph-mejepa` to `-p context-graph-mejepa-hygiene` correspondingly), or
- Add a new shim file at the expected path that re-exports the test.

---

#### V-004 (HIGH) — Tests that discard every return value
**File:** `crates/context-graph-embeddings/tests/global_provider_fsv.rs`
**Tests:**
- `fsv_edge_long_queries` (lines 240-256)
- `fsv_edge_api_contract` (lines 302-330, partial)
- `fsv_concurrent_status_access` (lines 141-165, partial — has a counter assert at the end)

**Suspect code (`fsv_edge_long_queries`):**
```rust
for i in 0..1000 {
    let _ = is_warm_initialized();
    let _ = warm_status_message();
    if i % 250 == 0 {
        println!("Iteration {}: status = {}", i, warm_status_message());
    }
}
println!("COMPLETED 1000 iterations without issues");
```

**Why it cannot fail:** No assertion ever runs against return values. If `is_warm_initialized()` is broken to always return false (or leak memory, or panic only on the 1001st call), the test passes. The test name claims "Long Query Handling" and the comment claims "verify repeated calls don't accumulate state incorrectly" but the test verifies neither.

**Hypotheses considered:**
1. The test is verifying panic-freedom (smoke test) — *partially supported by the API-contract test comment* "Should never panic, only return values or errors." But that's a panic-freedom assertion ONLY, not a correctness assertion. Doctrine §15.13 says "tests must fail when the feature is broken" — panic-freedom is one property; the test name implies more.
2. The println in `if i % 250 == 0` is the validation — *eliminated*: stdout isn't asserted on.
3. There's external monitoring that asserts on state — *no evidence*; this is a unit test under `tests/`.

**Mutation that should — but does not — fail:**
- `warm_status_message()` broken to always return literal `"Ready"`: vacuous test still passes; `fsv_edge_api_contract`'s `assert!(!status.is_empty())` (line 319) trivially passes too.
- `is_warm_initialized()` broken to perform an O(N) scan over thousands of model entries on every call: the 1000 iterations × 100 threads × O(N) work in `fsv_concurrent_status_access` could OOM or timeout, but the assertion `assert_eq!(completed, 10)` only counts successful thread join — slow tests are not failing tests.

---

#### V-005 (HIGH) — Synthetic panic that always fires
**File:** `tests/e2e/heal_ticker_telemetry.rs:87-103`
**Test:** `task_test_011_heal_ticker_telemetry_persists_observe_runs_and_restart_counts`

**Suspect code:**
```rust
let previous_panic_hook = std::panic::take_hook();
std::panic::set_hook(Box::new(|_| {}));
let panic_result = std::panic::catch_unwind(|| {
    panic!("synthetic scheduler panic for heal ticker telemetry FSV");   // <-- ALWAYS panics
});
std::panic::set_hook(previous_panic_hook);
if panic_result.is_err() {        // <-- ALWAYS Err (panic always fires)
    counters.record_heal_scheduler_restart();
}
let after_restart_snapshot = counters.snapshot();
ensure!(
    after_restart_snapshot.heal_ticker_telemetry_total.scheduler_restart_count_total == 1,
    "restart counter did not increment"
);
```

**Why it fails too weakly:**
- The `panic!()` is a literal in the test body, not a real scheduler internal failure.
- The `if panic_result.is_err()` branch is guaranteed-taken because `panic_result` is set by `catch_unwind` of a `panic!()` call. Equivalent to `let _ = ...; counters.record_heal_scheduler_restart();`.
- The ensure! that the counter == 1 verifies only that `record_heal_scheduler_restart()` increments the counter by exactly 1 — a property of a single atomic add.

The TEST CLAIM is heal-scheduler restart-counter behavior under real scheduler panics. The ACTUAL CLAIM PROVEN is: `counter.fetch_add(1)` increments a u64 by 1.

**Hypotheses considered:**
1. The scheduler is intentionally not exercised because spawning it is heavyweight — *plausible, but doctrine §3.4 requires a real trigger*. The actual production trigger (a scheduler tick that panics inside `state.run_tick`) is not invoked.
2. There's a sibling test that does invoke a real scheduler panic — *grep `state.run_tick` in `tests/e2e/`*: only `heal_ticker_telemetry.rs:73` invokes `run_tick`, and only with `TickName::Observe`, the happy-path tick. No scheduler-internal panic is ever exercised.
3. The counter is genuinely the only state-of-truth and counter+= verification IS the spec — *acceptable interpretation if the test name reflects this*; but the name "heal ticker telemetry" implies the heal-scheduler restart pipeline, not just the counter primitive.

**Mutation that should — but does not — fail:** Break `state.run_tick(TickName::Restart)` to silently skip counter increment when an actual scheduler panic occurs. The test never invokes `Restart`, so the bug is invisible.

---

#### V-006 (HIGH) — Performance-pair test discards Result, doesn't verify computation
**File:** `crates/context-graph-core/src/similarity/tests.rs:278-296`
**Test:** `test_performance_pair_under_5ms`

**Suspect code:**
```rust
#[tokio::test]
async fn test_performance_pair_under_5ms() {
    let engine = DefaultCrossSpaceEngine::new();
    let fp1 = create_test_fingerprint(1);
    let fp2 = create_test_fingerprint(2);
    let config = CrossSpaceConfig::default();

    let start = std::time::Instant::now();
    let _ = engine.compute_similarity(&fp1, &fp2, &config).await;   // <-- Result DISCARDED
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_millis() < 5,
        "Took {}ms, expected <5ms",
        elapsed.as_millis()
    );
}
```

**Why it fails too weakly:** If `compute_similarity` is broken to return `Err(...)` immediately on entry (no computation), the test PASSES MORE STRONGLY because the Err-return is even faster. If it's broken to return a NaN similarity, the test passes. If it's broken to deliberately call `tokio::yield_now()` 1000 times (slow but no real work), the test fails on slow CI runners — false-negative, not catching the real bug.

The COMPLEMENTARY test 16 lines below (`test_batch_100_under_50ms`) DOES assert `results.len() == 100` after `.unwrap()`, so a complete subsystem break would be caught there. But the PAIR test in isolation is vacuous.

**Mutation that should — but does not — fail:** Replace `compute_similarity` body with `Err(SimilarityError::ProductionBroken)`. The test still passes.

---

### MEDIUM

#### V-007 (MEDIUM) — UUID timing benchmark verifies no UUID property
**File:** `crates/context-graph-embeddings/tests/benchmark_test.rs:432-457`
**Test:** `test_uuid_generation_timing`

```rust
for _ in 0..iterations {
    let _ = Uuid::new_v4();
}
// ... assert!(avg_ns < 1_000, ...)
```

**Why it fails too weakly:** The test name claims "UUID generation timing." If `Uuid::new_v4()` is replaced with a function returning all-zero UUIDs (or duplicate UUIDs), the test passes — likely faster, in fact. The test does NOT verify UUIDs are valid, unique, or have v4 bit-pattern. The Rust compiler may even eliminate the call as dead code in release mode, making `avg_ns` ≈ 0.

**Mitigating context:** This is a benchmark test, not a correctness test. Categorized MEDIUM rather than HIGH because the doctrine cares about the *correctness* gate. Reasonable fix: store UUIDs in a `Vec` and assert `dedupe_count == iterations` to verify both performance AND correctness.

---

#### V-008 (MEDIUM) — Token-storage round-trip checks length but not values
**File:** `crates/context-graph-storage/src/teleological/search/token_storage.rs`
**Tests:**
- `test_single_token` (lines 768-785)
- `test_max_tokens` (lines 787-804)

**Suspect code:**
```rust
let tokens = generate_test_tokens(1);
storage.store(id, &tokens).expect("Store failed");
let retrieved = storage.get(id).expect("Get failed");
assert!(retrieved.is_some());
assert_eq!(retrieved.unwrap().len(), 1);   // <-- only length checked
```

**Why it fails too weakly:** If the storage backend is broken to return `Some(vec![vec![]; 1])` regardless of input, the test passes. Compare with `test_token_round_trip` 285 lines above (lines 469-503), which DOES verify token-by-token, value-by-value equality with `(o - r).abs() < 1e-6`. The single/max variants drop that property.

**Mutation that should — but does not — fail:** Make `RocksDbTokenStorage::get` deserialize tokens incorrectly (e.g., always return `Vec::new()` for each token's f32 vector, but return the right outer length). Length-only tests pass; value-checking test fails.

---

#### V-009 (MEDIUM) — `assert!(sim_X_Y.is_some())` without value verification
**File:** `crates/context-graph-core/src/clustering/fingerprint_matrix.rs:719-720`
**Test:** `test_incremental_update`

```rust
let sim_0_2 = matrix.get_similarity(0, 2);
let sim_1_2 = matrix.get_similarity(1, 2);
let sim_2_2 = matrix.get_similarity(2, 2);

assert!(sim_0_2.is_some());        // <-- no value check
assert!(sim_1_2.is_some());        // <-- no value check
assert_eq!(sim_2_2, Some(1.0));    // <-- self-similarity verified
```

**Why it fails too weakly:** If `update_matrix_incremental` is broken to insert `Some(NaN)` or `Some(0.0)` for all cross-similarities, the test passes. The symmetry check at lines 724-725 (`get_similarity(0,2) == get_similarity(2,0)`) is also satisfied by NaN-or-0.0 (broken-but-consistent).

**Mutation that should — but does not — fail:** Make incremental update skip the actual cosine similarity computation and write 0.0 to every off-diagonal cell. Test passes.

---

#### V-010 (MEDIUM) — Batch retrieve checks `is_some` but not values
**File:** `crates/context-graph-core/src/stubs/teleological_store_stub/tests.rs:99-109`
**Test:** `test_batch_store_and_retrieve`

```rust
let fingerprints: Vec<_> = (0..10).map(|_| create_test_fingerprint()).collect();
let ids: Vec<_> = fingerprints.iter().map(|fp| fp.id).collect();
let stored_ids = store.store_batch(fingerprints).await.unwrap();
assert_eq!(stored_ids.len(), 10);
let retrieved = store.retrieve_batch(&ids).await.unwrap();
assert_eq!(retrieved.len(), 10);
assert!(retrieved.iter().all(|r| r.is_some()));   // <-- only Some-ness checked
```

**Why it fails too weakly:** If `retrieve_batch` is broken to return 10 random fingerprints (none matching the requested ids), the test passes. No content / id equality check.

**Mutation that should — but does not — fail:** Make `retrieve_batch` map every input id to the same hardcoded fingerprint. Test passes.

---

#### V-011 (MEDIUM) — `pass: true` hardcoded in e2e harness evidence rows
**File:** `tests/e2e/harness.rs:199, 218`

```rust
happy_path.push(StepEvidence {
    step: "predict_readback".to_string(),
    // ...
    pass: true,    // <-- LITERAL, not derived from any check
});
// ...
happy_path.push(StepEvidence {
    step: "inject_markdown".to_string(),
    // ...
    pass: true,    // <-- LITERAL
});
```

**Why it fails too weakly:** The `all_passed = happy_path.iter().all(|step| step.pass)` aggregator at the top-level test cannot distinguish these literal-true rows from rows that were verified. While `ensure!()` calls earlier in the function DO catch real failures (so a totally broken predict path would fail before reaching `pass: true`), the **evidence JSON written to disk is misleading** — operators reading the FSV report cannot distinguish actually-verified-pass from "we forgot to compute pass."

**Mutation that should — but does not — fail:** Break `RealityPrediction` serialization to omit the `Prediction ID` field from `render_prediction_markdown`. The `ensure!(injected.contains("Reality prediction") && injected.contains("Prediction ID"))` line 205-207 catches it. But if someone WEAKENS that ensure to only check "Reality prediction", the `pass: true` evidence row hides the regression. **The literal `true` is a latent doctrine §3 (FSV evidence integrity) hazard.**

**Recommended fix:** Replace literal `true` with the actual condition (e.g., `pass: predictions_after == predictions_before + 1 && first_prediction.prediction_id != [0; 16]`).

---

#### V-012 (MEDIUM) — Sort-by-length test only verifies position [0]
**File:** `crates/context-graph-embeddings/src/batch/types/tests.rs:291-320`
**Test:** `test_batch_queue_drain_batch_sorts_by_length`

```rust
let batch = queue.drain_batch().unwrap();
assert_eq!(batch.inputs.len(), 3);

// Verify ordering by checking content
if let ModelInput::Text { content, .. } = &batch.inputs[0] {
    assert_eq!(content, "Short");
}
```

**Why it fails too weakly:** Only `batch.inputs[0]` is checked. If sort is broken to put short first and then leave the next two in the wrong order (`["Short", "This is a very long sentence ...", "Medium length text"]`), the test passes. The `if let` is silently skipped if `batch.inputs[0]` isn't a `Text` variant — though by construction it always is, so this is a partial check, not vacuous in the limit.

**Mutation that should — but does not — fail:** Break the sort to only correctly position the shortest element and leave the rest in original order. Test passes.

---

### LOW

#### V-013 (LOW) — `assert!(report_path.is_file())` after writing the report
**Files:**
- `tests/e2e/happy_path.rs:13`
- `tests/e2e/catastrophic_event.rs:791`

```rust
write_report_with_readback(&report, &report_path).expect("write E2E FSV report");
// ...
assert!(report_path.is_file());   // <-- file was just successfully written + fsync'd
```

**Why it fails too weakly:** `write_report_with_readback` performs the write, fsync, and read-back equality check internally. By the time it returns Ok, the file definitively exists. The subsequent `is_file()` check is redundant. If `write_report_with_readback` is broken to silently no-op (impossible: it has its own ensure!), the test catches it there, not here.

**Mitigating context:** This is cosmetic noise rather than a true vacuous assertion (the operation it observes IS real). Listed LOW for hygiene.

---

#### V-014 (LOW) — `assert!(res.is_ok())` without unwrap
**File:** `crates/context-graph-mejepa-corpus/src/split_tests.rs:180`
**Test:** `assert_no_test_patch_leakage_accepts_distinct`

```rust
let res = assert_no_test_patch_leakage(&report.assignments, |task_id| {
    map.get(task_id).map(|s| s.as_str())
});
assert!(res.is_ok());
```

**Mitigating context:** This is paired with `assert_no_test_patch_leakage_rejects_collision` (lines 183-193) which DOES verify the error code on the failure path. So the function isn't fully untested — but the success-path is. If the function is broken to always return Ok regardless of leakage, both the rejection test catches it (via expect_err). Listed LOW because the paired test compensates.

---

#### V-015 (LOW) — `assert!(retrieved.iter().all(|r| r.is_some()))` in batch retrieve
Covered as V-010 above — listed here only to note the broader pattern.

---

## Robust Under Mutation — Exoneration List

These tests LOOKED suspicious during the cold-read but were verified to catch their stated property under the canonical mutation:

| File:Line | Test | Why it's actually robust |
|---|---|---|
| `tests/e2e/restart_safety.rs:797` | `task_test_002_restart_safety_resumes_without_loss_or_duplicate` | The `run_restart_safety_fsv()` body has 8 internal `ensure!()` + `assert!()` checks against the live RocksDB SoT (watermark, live predictions count, panel rows count); a mutation to skip watermark persistence is caught at line 152 (`state.watermark.last_consumed_byte_offset == first_expected_offset`). |
| `tests/e2e/multi_agent.rs:867` | `task_test_003_multi_agent_sessions_and_feedback_are_isolated` | Uses `std::sync::Barrier` for true concurrent execution (3 threads), then verifies per-session prediction counts === 1 and distinct prediction IDs === 3 (lines 165-169). A mutation to clone predictions across sessions would fail at `per_session_counts.values().all(|count| *count == 1)`. |
| `tests/e2e/load.rs:5` | `task_test_007_load_harness_persists_prediction_and_feedback_metrics` | Asserts exact `cf_counts_after[CF_MEJEPA_LIVE_PREDICTIONS] == 24` (6 agents × 4 edits). A mutation to silently drop predictions under concurrent load would fail this equality. |
| `tests/e2e/chaos_failpoints.rs:751` | `task_test_006_chaos_failpoints_fail_closed_with_sot_readback` | Uses real `fail::cfg(FP_PREDICTION_WRITE, "return")` failpoint injection, then `expect_err()`. A mutation to bypass the failpoint check would fail because the production code WOULD succeed and the expect_err would unwrap-panic. |
| `tests/e2e/storage_exhaustion.rs:600` | `task_test_005_storage_exhaustion_recovers_within_quota` | Seeds 6 × 1MiB rows, verifies `over_budget == true` post-seed, runs real `quota_check_and_evict()`, verifies non-empty eviction list AND `<= 110% of cap`. Genuine round-trip. *(Caveat: this test is not actually invoked by CI — see V-003.)* |
| `tests/e2e/pause_active_learning.rs`, `pause_consumer.rs`, `adversarial_corpus.rs`, `catastrophic_event.rs` | (all 4 task_test_* functions) | Each has detailed per-step `pass` evidence computed from CF row counts / readback equality / structured error code matches. Spot-check at adversarial_corpus.rs:838 etc. shows real `before == after` and `err.code() == "EXPECTED_CODE"` checks. |
| `crates/context-graph-core/src/embeddings/stubs.rs:425-545` | All `test_stub_*` tests | Verify determinism (same input → same output), distinguishability (different input → different output), bounds (indices within vocab), AND dimensions. Solid. |
| `crates/context-graph-core/src/teleological/services/profile_manager/tests.rs:96` | `test_find_best_match_finds_correct_profile` | `is_some()` followed by `.unwrap()` + `assert_eq!(matched.profile_id.as_str(), "code_implementation")`. Value-verified. |
| `crates/context-graph-embeddings/src/warm/registry/tests_queries.rs:238, 245` | `test_failed_entries_lists_failed_models` | `is_some()` followed by `.unwrap()` + `assert_eq!(*code, 102)` + `assert_eq!(msg, "CUDA error")`. Value-verified. |
| `crates/context-graph-core/src/causal/scm.rs:505` | `test_*` (path is_some) | Followed by value/length verification — confirmed by neighborhood read. |
| `#[should_panic(expected = "FAIL FAST")]` and `#[should_panic(expected = "CONSTITUTION VIOLATION AP-007: ...")]` patterns | 40+ panic tests across `context-graph-core`, `context-graph-embeddings`, `context-graph-storage` | Specific panic message expectations — a panic from setup that doesn't match the expected substring causes the test to fail. |

---

## Special Section A — `#[ignore]` Audit

22 `#[ignore]` annotations total. None unannotated (`#[ignore]` without a reason). All have a documented reason in their attribute string. Audit results:

| # | File:Line | Reason | Acceptable? | Notes |
|---|-----------|--------|-------------|-------|
| 1-8 | `crates/context-graph-cuda/src/hdbscan/tests.rs:17,27,45,101,152,174,222,253` | "requires GPU" | ✓ ACCEPTABLE | Standard environment-conditional gating. Run with `--ignored` on a GPU host. **Caveat:** no CI workflow invokes `cargo test -p context-graph-cuda --ignored`, so these never run anywhere. |
| 9-10 | `crates/context-graph-mcp/src/handlers/tests/tools_call.rs:77,117` | "requires production E1-E14 embedding models under CONTEXT_GRAPH_MODELS_PATH" | ✓ ACCEPTABLE | Heavy-model gating. **Caveat:** ditto — no CI workflow runs them. |
| 11-14 | `crates/context-graph-mcp/src/handlers/tests/search_periodic_test.rs:104,365,416,507` | same | ✓ ACCEPTABLE | ditto. |
| 15-17 | `crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/tests.rs:137,155,196` | "requires real BAAI/bge-m3 weights under the ME-JEPA model artifact root" | ✓ ACCEPTABLE | ditto. |
| 18 | `crates/context-graph-cli/benches/cli_benchmarks.rs:971` | "Run explicitly: cargo test --test cli_benchmarks -- --ignored" | ⚠ AMBIGUOUS | The reason is a USAGE comment, not a justification. The `#[ignore]` attribute itself lacks a `=` value here (just `#[ignore]` with comment). Acceptable but inconsistent with other entries. |
| 19 | `crates/context-graph-cli/tests/dynamicjepa_phase10_fsv.rs:132` | "runs the full 8,000-event gridworld verifier and trains a real model" | ✓ ACCEPTABLE | Heavy-test gating documented. |
| 20 | `crates/context-graph-storage/src/teleological/indexes/hnsw_impl/tests.rs:398` | "builds 10K-vector HNSW index (~5-15s); perf benchmark — run with --ignored" | ✓ ACCEPTABLE | Perf-only. |
| 21 | `crates/context-graph-cli/tests/dynamicjepa_research_smoke_fsv.rs:538` | "requires the real O*NET 30.2 text ZIP" | ✓ ACCEPTABLE | External data dependency. |

**No `#[ignore]` is "hiding a known failure."** All have plausible environmental/heavyweight reasons. **However**, the CI gap is real: only `soak.yml` and `load.yml` invoke `--include-ignored`, and only on the `mejepa` crate's `soak` / `load` test bin (which have no `#[ignore]`d functions). The `mcp`, `cuda`, `embeddings`, and `cli` crates' `#[ignore]`d tests never run in CI.

---

## Special Section B — Feature-Gated Tests That Never Run

5 tests under `#[cfg(feature = "candle")]` in `crates/context-graph-embeddings/src/warm/loader/tests/loader_tests.rs`. `candle` is in the embedded crate's `default` features list (Cargo.toml line 91), so they would compile under default `cargo build` / `cargo test -p context-graph-embeddings`.

**However:** the `.github/workflows/ci.yml` file does NOT invoke `cargo test`. It only runs `scripts/ci/check_no_c_drive_paths.sh` and `scripts/swebench_lite_ops.py python-proof-domain-isolation`. None of the workflows (ci, ci_smoke_a/b/c, chaos, e2e, load, soak) issue `cargo test -p context-graph-embeddings`.

**Verdict:** The `loader_tests.rs` test suite — including V-002's masked fail-fast test — **never executes in any CI workflow**. A local developer running `cargo test --workspace` would exercise them, but the `loader_tests.rs:173 if cuda_init_result.is_err() { return; }` short-circuit would still bypass the actual assertion on every non-CUDA host.

Cross-referenced workflows:
| Workflow | `cargo test` target | Tests it actually runs |
|---|---|---|
| ci.yml | (none — only path checks) | none |
| ci_smoke_a.yml | (delegates to `scripts/5090jepa/local_ci_gate.sh smoke-a`) | dynamicjepa smoke A only |
| ci_smoke_b.yml | same pattern | dynamicjepa smoke B |
| ci_smoke_c.yml | same pattern | dynamicjepa smoke C |
| chaos.yml | `cargo test -p context-graph-mejepa --test chaos_failpoints` + (missing) `--test storage_exhaustion` | chaos_failpoints only; storage_exhaustion silently skipped (V-003) |
| e2e.yml | `cargo test -p context-graph-mejepa --test e2e` + `--test toolchain_fail_closed` | happy_path + restart_safety + multi_agent + toolchain_fail_closed |
| load.yml | `cargo test -p context-graph-mejepa --test load -- --include-ignored` | load.rs only |
| soak.yml | `cargo test -p context-graph-mejepa --test soak -- --include-ignored --nocapture` | soak.rs only (which doesn't actually soak — V-001) |

**Tests in `context-graph-core`, `context-graph-embeddings`, `context-graph-storage`, `context-graph-cli`, `context-graph-mcp`, `context-graph-cuda`, `context-graph-mejepa-corpus`, `context-graph-mejepa-cf`, `context-graph-mejepa-instruments`, `context-graph-mejepa-tct`, `context-graph-mejepa-train`, `context-graph-mejepa-hygiene`, `context-graph-mejepa-shift-subscriber`, `context-graph-mejepa-embedders`, `context-graph-witness`, `context-graph-solver`, `context-graph-mincut`, `context-graph-tier-compression`, `context-graph-paths`, `context-graph-dynamicjepa`, `context-graph-test-utils` — never run in CI.**

Per FSV-PROTOCOL.md §6, the per-task evidence requirement is `cargo test -p <touched-crate>` "with a stated reason" if scoped. The doctrine carries that contract for the *task-level* gate, but the *project-level* CI gate is silent on most of the workspace. This is structurally a doctrine §3.20 (GitHub Free-plan budget) tradeoff, not a vacuous-test issue per se — but combined with V-001 / V-002 it means the only safety net (operator local `cargo test --workspace`) is also where the vacuous tests pass silently.

---

## Special Section C — Do the 13 root-level e2e tests actually exercise chaos / load / soak / multi-agent / restart-safety?

| Test file | Claim | Actual behavior | Verdict |
|---|---|---|---|
| `tests/e2e/happy_path.rs` | "E2E loop runs end-to-end" | Real: make_edit → verify → predict → inject markdown → feedback → test_outcome → oracle. All against live RocksDB SoT. ✓ | **GENUINE** |
| `tests/e2e/restart_safety.rs` | "Subscriber resumes without loss or duplicate after restart" | Real: writes JSONL shifts → drops subscriber → reopens → verifies watermark progression, live-prediction count, panel rows, duplicate-skip behavior, malformed-JSONL fail-closed, backwards-watermark rejection, zero-session rejection. 4 boundary cases. ✓ | **GENUINE** |
| `tests/e2e/multi_agent.rs` | "Multi-agent sessions and feedback isolated" | Real: 3 std::thread workers + `std::sync::Barrier` → concurrent verify calls → per-session prediction count == 1 → distinct prediction IDs == 3 → unknown-prediction-feedback boundary → duplicate-feedback-key boundary → zero-session boundary. ✓ | **GENUINE** |
| `tests/e2e/load.rs` | "Load harness persists prediction and feedback metrics" | Real: 6 agents × 4 edits = 24 concurrent verify calls with Barrier sync. Exact CF row count assertion. p99 latency budget. ✓ | **GENUINE** |
| `tests/e2e/soak.rs` | "Repeats load and verifies nightly timer" + "30-minute soak" implied by DEFAULT_SOAK_MINUTES=30 | Runs 2 cycles of load.rs (≈seconds total). `requested_minutes` is dead metadata. Does verify systemd timer file content. **Does NOT actually soak.** | **VACUOUS** (see V-001) |
| `tests/e2e/chaos_failpoints.rs` | "Chaos failpoints fail closed with SoT readback" | Real: `fail::cfg(...)` failpoint injection at prediction-write, feedback-write, flush. expect_err verification. Unknown-failpoint rejection. ✓ | **GENUINE** |
| `tests/e2e/storage_exhaustion.rs` | "Storage exhaustion recovers within quota" | Real: seeds 6 × 1MiB shift-watermark rows → asserts over_budget → calls quota_check_and_evict → asserts non-empty eviction list AND within 110% of cap. ✓ | **GENUINE (test body)** — but **NEVER INVOKED BY CI** due to V-003. |
| `tests/e2e/adversarial_corpus.rs` | "Adversarial corpus rejection" | (Cold-read sample 838+) Per-step pass evidence computed from rejection counts. | **GENUINE** (per spot-check; full audit would require deeper read) |
| `tests/e2e/catastrophic_event.rs` | "Catastrophic event recovery" | Similar pattern to storage_exhaustion. | **GENUINE** (per spot-check) |
| `tests/e2e/heal_ticker_telemetry.rs` | "Heal ticker telemetry persists observe runs and restart counts" | observe-tick part is real (state.run_tick(Observe) → counter check). Restart-counter part uses synthetic in-test `panic!()` rather than real scheduler panic. | **PARTIALLY VACUOUS** (see V-005) |
| `tests/e2e/pause_active_learning.rs` | "Pause active learning preserves state" | (Spot-check 593+) Computed pass evidence. | **GENUINE** |
| `tests/e2e/pause_consumer.rs` | "Pause consumer preserves state" | (Spot-check 554+) ditto. | **GENUINE** |
| `tests/e2e/harness.rs` | (shared utility, not a test) | Contains `pass: true` literals at lines 199, 218 (V-011). | **HARNESS HAZARD** |

**Summary:** Of the 13 root-level e2e tests, **10 are genuine and substantive**, **2 are partially vacuous** (soak.rs is named soak but doesn't soak; heal_ticker_telemetry.rs uses a synthetic panic), **1 is genuine but never invoked by CI** (storage_exhaustion.rs via V-003 workflow bug).

---

## Forensic Method Notes

Investigation tiers used:
- **Tier 1 (Glance):** Ripgrep across 22 active crates for 16 suspect patterns (tautology, vacuous is_some/is_ok, discarded results, debug_assert in tests, early-return-on-error, wide float tolerances, hardcoded ceiling-only length asserts).
- **Tier 2 (Scan):** Opened 35+ files at flagged lines and read the full test function plus 20-line context.
- **Tier 3 (Investigate):** For each HIGH candidate, generated 3 hypotheses for production mutations; tested mentally whether the test catches each.
- **Tier 4 (Deep Dive):** Read full `tests/e2e/*.rs` files (happy_path, restart_safety, multi_agent, load, soak, chaos_failpoints, storage_exhaustion, heal_ticker_telemetry) end-to-end, traced the harness shim chain, and cross-referenced against `.github/workflows/*.yml`.

Cold Read targets that turned out fine after investigation:
- 131 `assert!(x.is_some())` and `assert!(x.is_ok())` sites — vast majority were followed by `.unwrap()` and value checks.
- 41 `let _ = ...` patterns in tests — most paired with `#[should_panic]` (which is the correct way to write a fail-fast panic test) or with subsequent assertions on side effects.
- 13 `assert!(.len() <= N)` ceiling-only sites — most were paired with `assert!(!x.is_empty())` or specific value lookups elsewhere in the same test.
- 32 `#[should_panic(expected = "...")]` sites — all had specific message expectations, none bare.

Contradiction Engine catches (test name vs what it actually proves):
- V-001 "Soak harness" vs "load harness twice" — CONTRADICTION
- V-002 "Fail fast on allocation error" vs "early-return when CUDA missing" — CONTRADICTION
- V-005 "Heal scheduler restart telemetry" vs "counter.fetch_add increments counter" — CONTRADICTION
- V-006 "compute_similarity under 5ms" vs "anything-completes (even Err) under 5ms" — CONTRADICTION
- V-007 "UUID generation timing" vs "any 1000-loop body executes in <1µs/iter" — CONTRADICTION

---

## Self-Verification

Per doctrine §11 Self-Verification Protocol — before any GUILTY verdict:

- ☑ Considered ≥3 alternative hypotheses for each HIGH finding (V-001 through V-006).
- ☑ Sought evidence that DISPROVES — for V-003 specifically grepped both crates for the storage_exhaustion shim, and for V-001 grepped for any consumer of `requested_minutes` and `accelerated`.
- ☑ Asked: am I suffering from confirmation bias? Cross-checked by running through the 131 `is_some`/`is_ok` sites and 41 `let _ =` sites and exonerating most.
- ☑ Falsifiable? Yes — every finding includes the specific production mutation that should but does not fail the test.
- ☑ Bias check: did I want to find vacuous tests? Yes (that was the task). Countermeasure: explicit "Robust Under Mutation" exoneration list for tests that looked vacuous but weren't.

**Confidence:** HIGH for V-001 through V-005 (multiple corroborating signals each), MEDIUM for V-006 through V-012 (single-pattern hits with mitigating context for some).

---

## Recommendations (do NOT auto-implement — operator review required)

Per task contract: this investigation is read-only. The following are FOR ISSUE-FILING PER CLAUDE.md §11 FILE RULE, not for immediate action:

1. **Open `type:discovery` issue** for V-001 — soak.rs does not implement a real soak; rename to `repeated_load_smoke` or wire `requested_minutes` to an actual sleep/deadline budget.
2. **Open `type:discovery` issue** for V-002 — split `test_fail_fast_on_allocation_error` into a CUDA-required test (mark `#[ignore]`) and a no-CUDA test that explicitly asserts `WarmError::CudaInitFailed`.
3. **Open `type:discovery` issue** for V-003 — `chaos.yml:56` looks in the wrong crate for `storage_exhaustion.rs`; either move the shim or update the workflow path.
4. **Open `type:discovery` issue** for V-004 — `global_provider_fsv.rs` long-query / api-contract tests need real value assertions, not just panic-freedom.
5. **Open `type:discovery` issue** for V-005 — heal_ticker_telemetry.rs should exercise a real scheduler-internal panic path via `state.run_tick(TickName::SomePanickingTick)`, not a hand-written `panic!()`.
6. **Open `type:discovery` issue** for V-011 — replace literal `pass: true` in `harness.rs:199,218` with derived conditions to preserve FSV evidence integrity.
7. **Open `type:context` issue** for the CI gap — only the `mejepa` crate's `e2e` / `load` / `soak` / `chaos_failpoints` tests run in CI; 17+ other workspace crates have ZERO CI test coverage. This is a doctrine §3 / FSV-PROTOCOL §6 gap.

---

## Case File Closing

**Verdict:** **GUILTY on 12 counts** (5 HIGH/CRITICAL severity, 6 MEDIUM, 1 LOW notable). The doctrine §15.13 invariant *"tests that pass when the project is broken"* is being violated in at least 12 specific sites. Two of those (V-001 soak and V-003 chaos workflow shim) are silent failures in workflows that CLAUDE.md and the futurebuild roadmap explicitly rely on as Phase F gates.

**Chain of Custody:**
- 2026-05-19, Sherlock Holmes agent — investigation started, read CLAUDE.md + FSV-PROTOCOL.md + AICodingAgentSuperPrompt.md doctrine refs.
- 2026-05-19 — completed Tier 1-4 investigation; wrote findings.
- Test-outcome hook captured an inconclusive event at `/zfs/archive/contextgraph/runtime/test-outcomes/af6f0fce2f4cec6b270127711e25cf44.jsonl` (informational — no tests executed, this was a read-only investigation).

**Files referenced (all absolute):**

Doctrine:
- `/home/cabdru/contextgraph/CLAUDE.md`
- `/home/cabdru/contextgraph/docs/futurebuild/specs/FSV-PROTOCOL.md`
- `/home/cabdru/contextgraph/docs2/AICodingAgentSuperPrompt.md`

Findings (suspect tests):
- `/home/cabdru/contextgraph/tests/e2e/soak.rs` (V-001)
- `/home/cabdru/contextgraph/crates/context-graph-embeddings/src/warm/loader/tests/loader_tests.rs` (V-002)
- `/home/cabdru/contextgraph/.github/workflows/chaos.yml` (V-003 — workflow bug)
- `/home/cabdru/contextgraph/tests/e2e/storage_exhaustion.rs` (V-003 — affected test)
- `/home/cabdru/contextgraph/crates/context-graph-mejepa-hygiene/tests/storage_exhaustion.rs` (V-003 — misplaced shim)
- `/home/cabdru/contextgraph/crates/context-graph-embeddings/tests/global_provider_fsv.rs` (V-004)
- `/home/cabdru/contextgraph/tests/e2e/heal_ticker_telemetry.rs` (V-005)
- `/home/cabdru/contextgraph/crates/context-graph-core/src/similarity/tests.rs` (V-006)
- `/home/cabdru/contextgraph/crates/context-graph-embeddings/tests/benchmark_test.rs` (V-007)
- `/home/cabdru/contextgraph/crates/context-graph-storage/src/teleological/search/token_storage.rs` (V-008)
- `/home/cabdru/contextgraph/crates/context-graph-core/src/clustering/fingerprint_matrix.rs` (V-009)
- `/home/cabdru/contextgraph/crates/context-graph-core/src/stubs/teleological_store_stub/tests.rs` (V-010)
- `/home/cabdru/contextgraph/tests/e2e/harness.rs` (V-011)
- `/home/cabdru/contextgraph/crates/context-graph-embeddings/src/batch/types/tests.rs` (V-012)
- `/home/cabdru/contextgraph/tests/e2e/happy_path.rs` (V-013)
- `/home/cabdru/contextgraph/tests/e2e/catastrophic_event.rs` (V-013)
- `/home/cabdru/contextgraph/crates/context-graph-mejepa-corpus/src/split_tests.rs` (V-014)

CI workflow cross-reference:
- `/home/cabdru/contextgraph/.github/workflows/ci.yml`
- `/home/cabdru/contextgraph/.github/workflows/ci_smoke_a.yml`
- `/home/cabdru/contextgraph/.github/workflows/ci_smoke_b.yml`
- `/home/cabdru/contextgraph/.github/workflows/ci_smoke_c.yml`
- `/home/cabdru/contextgraph/.github/workflows/chaos.yml`
- `/home/cabdru/contextgraph/.github/workflows/e2e.yml`
- `/home/cabdru/contextgraph/.github/workflows/load.yml`
- `/home/cabdru/contextgraph/.github/workflows/soak.yml`

**The game is afoot — and the case is closed.**
