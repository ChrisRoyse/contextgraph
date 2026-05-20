# SHERLOCK HOLMES CASE FILE: Mock Data Hunt

**Case ID**: SHM-MOCK-2026-05-19
**Subject**: ContextGraph / ME-JEPA-Code workspace — hunt for mock/stub/fake/synthetic test data that violates the FSV-PROTOCOL "NO MOCK DATA" doctrine.
**Method**: Tiered investigation (Glance → Scan → Investigate → Deep Dive). Linear Sequential Unmasking applied. ≥3 hypotheses generated for each HIGH finding. Read-only; no code modified.
**Verdict**: **PARTIAL GUILT** — the workspace has strong DEFENSIVE machinery against mocks in production (Phase H absence checks, magic-byte rejection, `#[cfg(test)]` gating, fail-closed providers, runtime stub-config refusal). However, several test surfaces semantically deceive via naming (calling synthetic random data "real") and one production CLI subcommand reports a hardcoded perfect-separation AUC as its OOD metric.

---

## Executive Summary — Top 5 Worst Mock Violations

Ranked by **how badly each hides broken-ness if the real implementation were broken**.

| # | Severity | File:Line | Crime |
|---|----------|-----------|-------|
| 1 | **HIGH** | `crates/context-graph-mejepa/src/cli.rs:114-116` | Production CLI subcommand `mejepa infer-test` computes its reported `ood_separation_auc` from `vec![0.05; 100]` vs `vec![0.95; 100]` — a HARDCODED perfectly-separated synthetic distribution. The `dod_satisfied()` predicate at line 132-136 then checks `auc > 0.85` against this trivially-1.0 value. The real OOD detection subsystem is **never exercised** by this DoD check. Operators reading the report think OOD is validated; it is not. |
| 2 | **HIGH (semantic deception)** | `crates/context-graph-test-utils/src/fingerprints.rs:18-80` | Crate Cargo.toml description: `"Shared test utilities — NO MOCK DATA"`. Functions named `generate_real_unit_vector`, `generate_real_semantic_fingerprint`, `create_real_fingerprint`. They produce **uniformly random floats** via `rand::thread_rng()` with no semantic structure of any real embedder model. Used by `crates/context-graph-storage/tests/full_integration_real_data/*` whose module docstring repeats `"CRITICAL: NO MOCK DATA"`. Tests using these helpers cannot detect bugs in *embedding semantics*; they only verify *storage shape & pipeline aliveness*. The naming actively misleads future agents into believing these tests gate embedder-quality FSV. |
| 3 | **HIGH (semantic deception)** | `crates/context-graph-storage/tests/teleological_integration.rs:33-45` | Test file docstring (line 3): `"# CRITICAL: NO MOCK DATA"`. Then `create_real_semantic()` returns `SemanticFingerprint::stub()` (every embedder filled with constant `0.1`), and `create_real_hash()` returns the **identical `0xDE 0xAD … 0xBE 0xEF` 32-byte hash for every fingerprint**. The latter is the **literal "DEADBEEF" fake checksum** the AP-007 detection in `crates/context-graph-embeddings/tests/no_fake_data_test.rs` is built to reject. Acceptable for serialization round-trip (which is what these tests do) but the docstring is a lie. |
| 4 | **MEDIUM-HIGH** | `crates/context-graph-core/src/monitoring.rs:529-587` ↗ `crates/context-graph-mcp/examples/task_py_g_063_project_ingest_fsv.rs:972`, `task_py_g_064_project_report_fsv.rs:762` | `StubLayerStatusProvider::perception_status()` and `memory_status()` **hardcode `Ok(LayerStatus::Active)`** without checking anything. Used inside **FSV harness examples** that exist specifically to produce closeout evidence for tasks PY-G-063/064. Any FSV evidence file produced by those examples reports "L1/L3 Active" without verification. (Mitigated by `crates/context-graph-mcp/src/monitoring.rs:3` documenting a real replacement.) |
| 5 | **MEDIUM (contradiction)** | `crates/context-graph-embeddings/src/cache/mod.rs:538-552` and `crates/context-graph-storage/src/teleological/search/maxsim.rs:380-394` | Comment: `"Create a test embedding with real data (NOT mocks)"`. Body: `(0..dim).map(|i| (i as f32 * 0.001).sin()).collect()`. This is **the canonical sin-wave pattern** that `no_fake_data_test.rs:detect_sin_wave_pattern` is built to DETECT as fake. Acceptable for cache-roundtrip / MaxSim-math-kernel tests (functionality under test is not embedding quality), but comments contradict the AP-007 detection rules and would confuse a future investigator. |

---

## Full Findings List

### HIGH severity (3)

#### H1. `mejepa infer-test` reports synthetic AUC against hardcoded perfectly-separated distributions

- **File**: `/home/cabdru/contextgraph/crates/context-graph-mejepa/src/cli.rs:114-116` (production code; called from `crates/context-graph-mejepa/src/bin/mejepa.rs:575` via the `InferTest` CLI subcommand)
- **Pattern**:
  ```rust
  let in_dist_scores = vec![0.05; 100];
  let ood_scores = vec![0.95; 100];
  let auc = separation_auc(&in_dist_scores, &ood_scores)?;
  ```
- **Production code path bypassed**: The real OOD subsystem (`crates/context-graph-mejepa/src/ood.rs::separation_auc` is called against real predictor scores in production). The reported `ood_separation_auc` field in `InferTestReport` is, by construction, the AUC of `[0.05;100]` vs `[0.95;100]` — i.e., perfect separation = 1.0. `dod_satisfied()` (line 132-136) then accepts `auc > 0.85` as PASS.
- **Hypotheses (LSU)**:
  - **H1.a**: This is a known leftover scaffolding from before the OOD subsystem was wired. The function exists to test the *latency/coverage* checks (the real work), and the AUC is a placeholder for now. → If true, it should be commented as `TODO: replace with real OOD eval` and `dod_satisfied` should not include `auc > 0.85`.
  - **H1.b**: The function is intentionally fixture-only and not meant to be ship-gate evidence. → Then the `InferTestReport.ood_separation_auc` field should not be emitted at all, or should be marked `synthetic_placeholder = true`. It isn't.
  - **H1.c**: The function is a regression smoke test (verifies the math kernel `separation_auc` returns >0.5 for separated inputs). → Then `dod_satisfied` should not be gating on it.
- **Verdict**: GUILTY. Either `dod_satisfied`'s `auc > 0.85` clause is dead/misleading or the real OOD eval is not wired. Should be either removed (and `ood_separation_auc` dropped from the report) or replaced with real predictor-output OOD scores from the calibration holdout split.
- **Bayesian update**: Probability that this is a real defect ≈ 90%. The hardcoded values + the gate predicate that consumes them is exactly the FSV-PROTOCOL §10 failure mode "the test checks a constant instead of exercising the implementation."

#### H2. `context-graph-test-utils` description "NO MOCK DATA" contradicts what its `generate_real_*` helpers actually produce

- **Files**:
  - `/home/cabdru/contextgraph/crates/context-graph-test-utils/Cargo.toml:7` — description `"Shared test utilities for context-graph workspace — NO MOCK DATA"`
  - `/home/cabdru/contextgraph/crates/context-graph-test-utils/src/lib.rs:4` — `"ALL data is REAL — no mocks."`
  - `/home/cabdru/contextgraph/crates/context-graph-test-utils/src/fingerprints.rs:18-80` — function bodies
- **Pattern**:
  ```rust
  pub fn generate_real_unit_vector(dim: usize) -> Vec<f32> {
      let mut rng = rand::thread_rng();
      let mut vec: Vec<f32> = (0..dim).map(|_| rng.gen_range(-1.0..1.0)).collect();
      // … L2 normalize …
  }
  ```
  Then `generate_real_semantic_fingerprint()` calls this for each of 14 embedders.
- **Production code path bypassed**: Real embedders (`ProductionMultiArrayProvider` at `crates/context-graph-embeddings/src/provider/multi_array.rs:1066`) consume content through 14 frozen-weight model forward passes producing vectors with *strong semantic structure* (clustering, low-rank, language-specific signatures). The helpers produce **i.i.d. uniform random noise on a hypersphere** — which has NONE of that structure. The two are not interchangeable for any property test that depends on embedding semantics.
- **Hypotheses (LSU)**:
  - **H2.a**: The author meant "no mocks" in the sense of "no `MockEmbedder` test doubles that fake `embed()` return values" — i.e., the function PRODUCES vectors directly rather than wrapping a fake provider. Strictly, the *result* of `generate_real_unit_vector` IS a real `Vec<f32>` of unit-normalized floats. → Plausible reading; but the function names `generate_real_semantic_fingerprint` and module names `full_integration_real_data` overreach. Future agents will read "real" and assume "real embedder output."
  - **H2.b**: Random vectors ARE adequate for the tests using them (storage roundtrip, CRUD, count semantics, soft-delete). → Confirmed by reading the callers — they verify pipeline shape, not embedding quality. The HARM is purely in the documentation deception, not in the test logic.
  - **H2.c**: The naming was an early-phase aspiration before real embedders were wired, and was never renamed. → Most plausible. Should be renamed: `generate_random_unit_vector`, `generate_random_semantic_fingerprint`, `create_random_fingerprint`, module `full_integration_storage_pipeline`.
- **Verdict**: GUILTY of misleading naming/documentation. Tests themselves are not doctrine-violating (they don't claim to validate embedding quality), but a fresh investigator following the doctrine §11 LSU principle would read "REAL" and trust the FSV evidence they emit too far.

#### H3. `teleological_integration.rs` docstring "CRITICAL: NO MOCK DATA" but uses identical 0.1-fill + DEADBEEF hash for every fingerprint

- **File**: `/home/cabdru/contextgraph/crates/context-graph-storage/tests/teleological_integration.rs:1-45`
- **Pattern**:
  ```rust
  //! # CRITICAL: NO MOCK DATA
  //! All tests use REAL RocksDB instances and REAL data structures.

  fn create_real_semantic() -> SemanticFingerprint {
      SemanticFingerprint::stub()       // every embedder = vec![0.1; N]
  }

  fn create_real_hash() -> [u8; 32] {
      let mut hash = [0u8; 32];
      hash[0] = 0xDE; hash[1] = 0xAD; hash[30] = 0xBE; hash[31] = 0xEF;
      hash                              // SAME hash for every fingerprint
  }
  ```
- **Production code path bypassed**: Real fingerprints have unique content hashes (SHA-256 of input bytes) and unique embedder outputs per content. Every "fingerprint" in these tests has the SAME hash, meaning any deduplication / collision-detection code path would silently treat all 50 stored fingerprints as a single content hash class. The 0.1-filled vectors all have cosine similarity exactly 1.0 with each other — so any similarity-ordering test would be ambiguous.
- **Why these tests still pass**: The tests verify RocksDB serialization round-trips, CRUD ID-based operations, CF isolation, persistence across restart — all of which key on `fp.id` (a `Uuid::new_v4()` per fingerprint), not on content_hash or vector values. So fingerprint *content* is irrelevant to test correctness. Mitigation works — but the docstring lies.
- **Hypotheses (LSU)**:
  - **H3.a**: The docstring is aspirational. The helpers were inherited from when `zeroed()` was rejected by `validate_vector()`, so `stub()` (0.1-fill) was the minimum non-zero valid input. → Confirmed by inline comment at line 31-32 referencing `STOR-M1`.
  - **H3.b**: These tests SHOULD use `context-graph-test-utils::generate_real_semantic_fingerprint()` (which is at least random per-fingerprint, not constant) for closer fidelity. → True, this would catch a class of bugs the current tests cannot (e.g., dimension-shuffling at serialize, identity-collision bugs).
  - **H3.c**: The tests are FINE — the field-under-test is the storage layer, not the semantic content. → True for the *test logic*. The docstring "NO MOCK DATA" is the problem, not the helpers.
- **Verdict**: GUILTY of docstring deception. Either change docstring to "ROCKSDB STRUCTURAL TESTS — fingerprint content is fixed placeholder, content_hash is `DEADBEEF` for every fp" OR migrate helpers to use `generate_real_semantic_fingerprint()` from test-utils.

### MEDIUM severity (5)

#### M1. `StubLayerStatusProvider` returns hardcoded "Active" status; used by FSV harness examples

- **Files**:
  - `/home/cabdru/contextgraph/crates/context-graph-core/src/monitoring.rs:529-587`
  - `/home/cabdru/contextgraph/crates/context-graph-mcp/examples/task_py_g_063_project_ingest_fsv.rs:972`
  - `/home/cabdru/contextgraph/crates/context-graph-mcp/examples/task_py_g_064_project_report_fsv.rs:762`
- **Pattern**: `perception_status()` and `memory_status()` both return `Ok(LayerStatus::Active)` without any health check. `all_layer_info()` returns `health_check_passed: Some(true)` for L1/L3 unconditionally.
- **Production code path bypassed**: The MCP server itself uses `RealLayerStatusProvider` (see `crates/context-graph-mcp/src/monitoring.rs:3` — "Replaces `StubLayerStatusProvider` (which always returns 'Active') with real…"). Production is safe. But the **FSV harness examples** at task_py_g_063/064 use the STUB. If those harness outputs ever get reported up as "FSV evidence shows L1/L3 active," that claim is unsubstantiated.
- **Recommendation**: Either replace `StubLayerStatusProvider` with the real provider in those FSV examples, or rename the stub to `HardcodedActiveLayerStatusProvider` and re-audit whether the FSV harness needs a layer-status check at all (typically these examples are about CF state, not layer health).

#### M2. `MockTokenStorage` in `maxsim.rs` test uses sin-wave-fill vectors

- **File**: `/home/cabdru/contextgraph/crates/context-graph-storage/src/teleological/search/maxsim.rs:357-394`
- **Pattern**:
  ```rust
  // In #[cfg(test)] mod
  struct MockTokenStorage { tokens: RwLock<HashMap<Uuid, Vec<Vec<f32>>>> }
  // Test helper that builds tokens:
  fn generate_normalized_token(seed: usize) -> Vec<f32> {
      (0..E12_TOKEN_DIM).map(|j| ((seed * 128 + j) as f32 / 1000.0).sin()).collect() // SIN WAVE
  }
  ```
- **Assessment**: `#[cfg(test)]`-gated. MaxSim is a math kernel (cosine over a set of 128-d vectors with max-pooling). The kernel produces deterministic outputs for any input. The sin-wave pattern is convenient because we can analytically reason about it. **However**, this directly contradicts `crates/context-graph-embeddings/tests/no_fake_data_test.rs::detect_sin_wave_pattern` — which is built specifically to REJECT this pattern as fake data in production. The contradiction is doctrinal, not functional.
- **Recommendation**: Use deterministic Gaussian samples (per `crates/context-graph-tier-compression/src/tests.rs:14-24` — pure-Rust SplitMix64 + Box-Muller) instead of `.sin()`. The kernel tests would be equivalent; the documentation no longer contradicts AP-007 detection.

#### M3. `MockProvider` in `multi_array_embedding.rs` test returns hardcoded NUM_EMBEDDERS dimensions

- **File**: `/home/cabdru/contextgraph/crates/context-graph-core/src/traits/multi_array_embedding.rs:1305-1334`
- **Pattern**: `MockProvider::embed_all` returns `Err(...)`, but `model_ids()` returns `["mock"; NUM_EMBEDDERS]` and `is_ready()` returns `true`. The test uses `MockProvider.dimensions()` (default trait method) to verify dimensions match constants.
- **Assessment**: Acceptable — this tests the DEFAULT TRAIT METHOD `dimensions()` which has no business calling `embed_all`. The error-returning embed methods correctly fail-closed.
- **Recommendation**: None. Clear, well-bounded use of a test double for testing a default trait method.

#### M4. `MockCodeEmbedder` returns zeroed fingerprints; `MockCodeStorage::search_by_fingerprint` returns empty

- **File**: `/home/cabdru/contextgraph/crates/context-graph-core/src/memory/code_capture.rs:483-594`
- **Pattern**:
  - `MockCodeEmbedder::embed_code(...)` always returns `SemanticFingerprint::zeroed()`.
  - `MockCodeStorage::search_by_fingerprint(...)` always returns `Ok(Vec::new())`.
- **Assessment**: `#[cfg(test)]`-gated. The tests using these mocks (`test_capture_chunk`, `test_capture_batch`, `test_delete_by_file`) verify the CRUD glue logic — they don't claim to test search quality or embedding correctness. `SemanticFingerprint::is_complete()` only checks dimensions, so the zeroed fingerprint passes the test assertion at line 638-643 because dimensions are correct.
- **Recommendation**: Rename `MockCodeEmbedder` → `IdentityCodeEmbedder` and add an inline note `// Returns zeroed fingerprints (dimensions correct, values uniform). Used only for CRUD glue-logic tests, NEVER for search-quality tests.`

#### M5. `create_test_embedding` in cache/mod.rs uses sin-wave vectors, documented as "real data (NOT mocks)"

- **File**: `/home/cabdru/contextgraph/crates/context-graph-embeddings/src/cache/mod.rs:538-552`
- **Pattern**: Comment `"Create a test embedding with real data (NOT mocks)."` then body `(0..dim).map(|i| (i as f32 * 0.001).sin()).collect()`.
- **Assessment**: Same defect class as M2. Used in cache insert/get/expire tests where vector values are irrelevant. Functionally fine; documentation contradicts AP-007 detection rules.

### LOW severity (4)

#### L1. `TestStorage` and `TestHnswManager` in `multi_space.rs` claim "REAL DATA, NO MOCKS" but ARE test doubles

- **File**: `/home/cabdru/contextgraph/crates/context-graph-embeddings/src/storage/multi_space.rs:367-442`
- **Pattern**: Banner comment `"TEST FIXTURES (REAL DATA, NO MOCKS)"`. Then `TestStorage::get_fingerprint` ALWAYS returns `Ok(None)`. `TestHnswManager::search_embedder` returns pre-seeded results from a `HashMap<u8, Vec<(Uuid, f32)>>` set by the test author via `set_results()` (i.e., the test PRE-COMPUTES the search output, then injects it).
- **Assessment**: Documented honestly at lines 447-452: tests test "index *infrastructure* — ranking, deduplication, RRF formula, weight application…" not search quality. The banner is misleading; the inline note is accurate.
- **Recommendation**: Change banner to `"TEST DOUBLES — pre-seeded results for index-infrastructure testing only (NOT search-quality tests)"`.

#### L2. `create_test_embeddings` in `storage/types/tests.rs` uses all-zero quantized bytes + zero content hash

- **File**: `/home/cabdru/contextgraph/crates/context-graph-embeddings/src/storage/types/tests.rs:13-64` + `:71` (`content_hash = [0u8; 32]`)
- **Pattern**: `data: vec![0u8; data_len]` for every embedder and `content_hash: [0u8; 32]`.
- **Assessment**: `no_fake_data_test.rs::validate_checksum` would REJECT `[0u8; 32]` as invalid. But this test is testing `StoredQuantizedFingerprint::new`'s SHAPE validation, not its checksum. `new()` doesn't validate checksums. The test passes because shape is correct.
- **Recommendation**: Use `[0x42u8; 32]` for content hash (matches `no_fake_data_test.rs:418`) to make the test consistent with the AP-007 example, and document the intent: "tests construction-time shape validation only; checksum content is irrelevant".

#### L3. Hardcoded `evidence_path` strings in FSV exemplar fixtures

- **File**: `/home/cabdru/contextgraph/crates/context-graph-mejepa/examples/task_py_g_073_predicted_works_fsv.rs:410-413`
- **Pattern**: `evidence_path: Some("/zfs/archive/contextgraph/corpus/python-forward-cache-v1-aiwonder-prod-20260515-191116Z/known-good-binary-search".to_string())`
- **Assessment**: The hardcoded path is stored as a STRING FIELD on the `ExemplarMatch` struct. The test doesn't `fs::metadata()` it. The test purpose is RocksDB round-trip (verify `readback == case.prediction`). Acceptable for serialization round-trip; would be guilty if anywhere claimed the path actually contained data.

#### L4. `WitnessHash([0x99; 32])` and `[0xAB; 32]` patterns in FSV examples

- **Files**:
  - `/home/cabdru/contextgraph/crates/context-graph-mejepa/examples/task_py_g_073_predicted_works_fsv.rs:414` — `WitnessHash([0x99; 32])`
  - `/home/cabdru/contextgraph/crates/context-graph-mejepa/examples/cf_round_trip_fsv.rs:233` — `WitnessHash([0xAB; 32])`
  - `/home/cabdru/contextgraph/crates/context-graph-mejepa/examples/task_py_g_042_patch_similarity_exemplar_fsv.rs:266+` — `exemplar_prediction_id: Some([0x42; 16])`
- **Assessment**: These are repetitive byte patterns that `no_fake_data_test.rs:validate_checksum` would in production reject as suspicious (low entropy). In FSV examples they're input data for testing round-trips, not produced as cryptographic evidence. Acceptable but worth a comment: `// Synthetic 32-byte hash; tests round-trip serialization only, not cryptographic validity`.

---

## Legitimate Synthetic Data (Exoneration List)

These uses of synthetic data are valid under FSV-PROTOCOL §4 — they preserve the property under test or are math-kernel tests where embedding semantics don't apply.

| File | Why legitimate |
|------|----------------|
| `crates/context-graph-mejepa-corpus/tests/source_prep_timeout_test.rs` | "Fake git" scripts are real shell scripts on disk; they exercise the **real** `run_git_timed` subprocess code path. The test would HANG (not silently pass) if the production timeout logic were broken. Materializes the property under test. Gold-standard example. |
| `crates/context-graph-embeddings/tests/no_fake_data_test.rs` | The "fake" data (sin waves, zero vectors, fake pointers, DEADBEEF checksums) is the **input to the DETECTION logic being tested**. The test verifies that fake-detection correctly REJECTS them. Synthetic-by-purpose. |
| `crates/context-graph-tier-compression/src/tests.rs:1-37` | Deterministic SplitMix64 + Box-Muller Gaussian samples for testing groupwise-quantization round-trip. Math-kernel test — the synthetic data has the *statistical* properties (Gaussian) that the kernel is designed for, with reproducible seeds. Doctrine §4 conformant. |
| `crates/context-graph-mejepa/examples/task_py_g_127_trained_checkpoint_export_fsv.rs:343-396` | Writes a fake checkpoint with magic bytes `MEJEPA_BEST_STUB_V1`, verifies that production rejects it. Defensive negative test against the very class of violations being investigated. |
| `crates/context-graph-mejepa/src/predictor_checkpoint.rs:18+, 134+, 224+` | Production code that DETECTS the `MEJEPA_BEST_STUB_V1` magic and fails closed. The constant exists to assert absence; not a violation. |
| `crates/context-graph-storage/tests/llm_validation_integration.rs:1-9` | Tests RocksDB encode/decode round-trip for `LLMEdgeValidation`. Docstring explicitly: `"No mocks; no LLM call — the LLM is a different module's responsibility"`. Storage-only scope is correctly bounded. |
| `crates/context-graph-mejepa-corpus/src/swebench.rs::run_swebench_lite_oracle()` | Calls real `python -m swebench.harness.run_evaluation`. The Q2 oracle path is real Docker. No mocks. |
| `crates/context-graph-core/src/graph_linking/weight_projector.rs:60-91` | `MockProjector { fixed_weight }` returns its fixed weight regardless of input — used to test that the DEFAULT TRAIT METHOD `project_batch` correctly iterates. Tests the trait default, not projection logic. Clear scope. |
| `crates/context-graph-cli/src/commands/dynamicjepa.rs:1349,9682` | Refuses to run HP sweeps without real O*NET data: `"download the O*NET 30.2 text database ZIP locally and pass --onet-text-zip; HP sweeps do not use mock data"`. Defensive. |
| `crates/context-graph-mcp/src/handlers/tests/mod.rs:287-319` (`FailClosedEmbeddingProvider`) | Returns `Err(...)` from `embed_all`, `is_ready()=false`, `health_status() all false`. Any test that would silently pass with mocked embeddings will instead error out. Gold-standard fail-closed pattern. |
| `crates/context-graph-mejepa-corpus/src/bin/mejepa-corpus.rs:1718-1735` (`toolchain_verdict`) | Synthesizes an `OracleVerdict` from category metadata for **non-Python fixtures** (Phase C). The actual `validate_toolchain` call IS a real toolchain invocation (see line 1578). The synthesized verdict is a hash-shaped storage artifact; the truth is in the toolchain report. Bounded scope. |
| `crates/context-graph-mejepa-train/examples/task_py_g_120_skill_linkage_surface_fsv.rs:64-87` | The word "mock" in `"skill:mock_only_change:fsv"` is a **vocabulary token** naming a class of patches that only edit test mocks. Not a test-data mock. |

---

## Special Section: `context-graph-test-utils` — does it comply with its docstring?

**Verdict**: **PARTIAL COMPLIANCE — naming is misleading, mechanics are bounded.**

**Cargo.toml description (`crates/context-graph-test-utils/Cargo.toml:7`)**:
```
description = "Shared test utilities for context-graph workspace — NO MOCK DATA"
```

**`src/lib.rs:4`**:
```
//! ALL data is REAL — no mocks.
```

**What the code actually does (`src/fingerprints.rs`)**:
- `generate_real_unit_vector(dim)` → uniformly random `[-1, 1]` floats via `rand::thread_rng()`, then L2-normalized.
- `generate_real_sparse_vector(target_nnz)` → random indices in `0..30522`, random positive values in `0.1..2.0`.
- `generate_real_semantic_fingerprint()` → builds a `SemanticFingerprint` by calling `generate_real_unit_vector` 14 times (with the appropriate dimensions per embedder).
- `generate_real_content_hash()` → 32 random bytes from `rand::thread_rng()`.
- `create_real_fingerprint()` / `create_real_fingerprint_with_id(id)` → wraps the above into a `TeleologicalFingerprint`.

**Compliance assessment by criterion**:

| Criterion (FSV-PROTOCOL §3, §4, §10) | Test-utils status |
|---------------------------------------|--------------------|
| §3.1 "No mocks for completion gates" | **PARTIAL**: data is real `Vec<f32>` but is not from a real embedder. If used as completion gate evidence for an embedder-quality test, it would silently pass. If used only for storage / CRUD / pipeline shape tests (the actual use), it's fine. |
| §4 "deterministic" | **FAIL**: uses `rand::thread_rng()` (non-deterministic). Re-running tests produces different fingerprints. The properties verified (storage roundtrip via id-based lookup) survive non-determinism, so tests still pass. But this violates §4's "deterministic" property. |
| §4 "distinguishable" | **PASS**: random vectors are statistically distinguishable. |
| §4 "boundary-rich" | **FAIL**: no edge-case generators (zero-norm-but-non-empty, max-dim, NaN-injection, etc.). |
| §4 "representative" | **FAIL**: random `[-1,1]` uniform is statistically *unlike* real embedder outputs (which cluster on a manifold). |
| §4 "Mocked subsystems are not acceptable completion evidence" | **PARTIAL**: the data path is real (not a mocked embedder return), so technically compliant. But "REAL" naming over-claims. |
| §10 "tests must fail when feature is broken" | **DEPENDS** on caller: for storage-shape tests, yes — they would fail if id-based lookup broke. For embedder-quality tests they would NOT fail because the input data doesn't have semantic structure to begin with. |

**Recommendation**: Rename functions and update docstring to reflect actual mechanics:
```toml
description = "Shared test utilities — deterministic-by-shape, random-by-content. NOT a substitute for a real embedder."
```
```rust
// In src/lib.rs:
//! Helpers produce real Rust types (Vec<f32>, SparseVector, TeleologicalFingerprint)
//! with correct dimensions and non-zero norms. Vector CONTENT is random uniform noise.
//! Suitable for: storage CRUD, RocksDB roundtrip, pipeline shape verification.
//! NOT suitable for: similarity ranking quality, embedder-output property tests,
//! clustering-aware tests. For those, use the production embedder pipeline.
```
Rename: `generate_real_unit_vector` → `generate_random_unit_vector`, etc.

---

## Special Section: `tests/fixtures/adversarial/` — real or synthetic?

**Verdict**: **REAL adversarial diffs against synthetic-but-purposeful target files.** Doctrine-compliant.

**Directory layout** (from `tests/fixtures/adversarial/`):
- `buggy_passing/buggy_passing_{01..10}.diff` (10 files)
- `embedder_targeted/embedder_targeted_{01..10}.diff` (10 files)
- `knowngood_lookalike/lookalike_{01..10}.diff` (10 files)

Total: **30 adversarial diff fixtures**, matching the TASK-PY-G-060 manifest (CLAUDE.md §8).

**Sample inspection**:

`buggy_passing_01.diff` (off-by-one attack — predicted_failure scenario):
```diff
# me-jepa-adversarial-case id=buggy_passing_01 category=buggy_passing scenario=predicted_failure target=src/buggy_passing_01.py
--- a/src/buggy_passing_01.py
+++ b/src/buggy_passing_01.py
@@
-def upper_bound(items):
-    return len(items)
+def upper_bound(items):
+    # Tests only cover non-empty collections.
+    return len(items) - 1
```

`lookalike_01.diff` (granger_rejection scenario — fake redaction):
```diff
# me-jepa-adversarial-case id=lookalike_01 category=knowngood_lookalike scenario=granger_rejection target=src/lookalike_01.py
--- a/src/lookalike_01.py
+++ b/src/lookalike_01.py
@@
-def redact_token(value):
-    return value[:4] + "...redacted"
+def redact_token(value):
+    # Looks like redaction but keeps the credential body.
+    return value[:4] + value[4:]
```

`embedder_targeted_01.diff` (constellation_reject scenario — known-good-vocabulary-flood):
```diff
+def validate_and_normalize_user(user):
+    """Known-good words: validate normalize sanitize authorize preserve."""
+    return {"authorized": True, "user": user}
```

**Assessment**:
- Each diff exemplifies a **real attack vector** against the ME-JEPA Q2 verdict (off-by-one, vocabulary flooding, lookalike-pattern fakery).
- Target files (`src/buggy_passing_01.py`, etc.) are **synthetic minimal-repro Python files** purpose-built to fit on a single diff hunk. They are NOT real SWE-bench repos; they're isolated probes.
- Each fixture has a tagged comment specifying `category=<class> scenario=<predicted attack mechanism> target=<relative-path>` — machine-readable metadata.
- This is doctrine §4-conformant synthetic input: **deterministic, distinguishable, representative of an attack class, boundary-rich (10 per category), cleanup-tagged**.

**Verdict**: INNOCENT. These are adversarial-corpus fixtures, not mocks. They probe the predictor's resistance to known evasion patterns.

---

## Special Section: defensive machinery that PROTECTS against mock data

The workspace has notable defensive infrastructure that catches mock-data violations:

| Defense | Location | What it catches |
|---------|----------|-----------------|
| **Config-phase stub refusal** | `crates/context-graph-core/src/config/mod.rs:234-258` | Production phase hard-fails if `embedding.model == "stub"`, `storage.backend == "memory"`, `index.backend == "memory"`, or `utl.mode == "stub"`. Returns `CoreError::ConfigError(...)` with specific remediation. |
| **AP-007 fake-pointer detection** | `crates/context-graph-embeddings/src/warm/cuda_alloc/allocator_cuda.rs:409-477` | Detects `0x7f80_0000_0000` fake CUDA pointer pattern; raises `WarmError::FakeAllocationDetected` (exit code 109). |
| **AP-007 sin-wave / zero-vector detection** | `crates/context-graph-embeddings/tests/no_fake_data_test.rs` | 600+ line test suite verifying that fake patterns (sin waves, zero vectors, fake checksums, VRAM delta mismatch) are detectable. |
| **`StubEmbeddingProvider` deletion** | `crates/context-graph-core/src/stubs/mod.rs:21-24` | Documents removal of the old single-embedding stub provider; archive note explains why. |
| **CLI → MCP migration** | `crates/context-graph-cli/src/mcp_client.rs:1-19` | Replaces in-CLI `StubMultiArrayProvider` with TCP connection to MCP server; closes a prior architectural vulnerability. |
| **Stub-magic-byte rejection in checkpoint loader** | `crates/context-graph-mejepa/src/predictor_checkpoint.rs:134-141, 224-231` | Refuses to load any `.safetensors` checkpoint starting with `MEJEPA_BEST_STUB_V1`; raises `MEJEPA_INFER_PREDICTOR`. |
| **Q4 doctrine freeze gate** | `crates/context-graph-mejepa/src/q4_trust_gate.rs::Q4_DOCTRINE_FREEZE_ACTIVE = true` | Hard-coded boolean preventing Q4 producer code paths from going live. |
| **Phase H absence-checks** | `scripts/ci/phase_h_*.sh` | Verifies that retired LLM (Qwen, Hermes, Ollama, llama.cpp) stack is absent. |
| **`#[cfg(test)]` gating of all stub modules** | `crates/context-graph-cuda/src/lib.rs:46-48`, `crates/context-graph-core/src/stubs/mod.rs:52-66`, `crates/context-graph-core/src/retrieval/mod.rs:108-110` | Stubs cannot leak into production builds. |
| **`FailClosedEmbeddingProvider`** | `crates/context-graph-mcp/src/handlers/tests/mod.rs:287-319` | When protocol-only tests use this, any embedding attempt FAILS with a clear error rather than producing fake vectors. |
| **`StubSystemMonitor` returns errors** | `crates/context-graph-core/src/monitoring.rs:470-512` | Every method returns `Err(SystemMonitorError::NotImplemented{...})` to make absence of monitoring EXPLICIT. |
| **C-drive path lint** | `scripts/ci/check_no_c_drive_paths.sh` and `scripts/ci/no_mnt_d_production_paths_fsv.py` | Blocks C-drive paths and classifies `/mnt/d` references for the aiwonder migration. |

---

## Investigation Methodology (transparency)

1. **READ** doctrine: CLAUDE.md (§15.12, §15.13), FSV-PROTOCOL.md (§3 "Hard Rules", §4 "Synthetic Data Properties", §10 "Failure Modes"), `context-graph-test-utils` README/Cargo.toml.
2. **Tier-1 Glance**: `rg -ci "mock|fake|stub|dummy"` across all 22 active crates. Top offenders: `embeddings/stubs.rs` (64), `embeddings/provider.rs` (53), `stubs/multi_array_stub.rs` (47), `memory/code_capture.rs` (26).
3. **Tier-2 Scan**: Each top hit opened, `#[cfg(test)]` gating verified, production exposure checked via `gh`-style cross-reference. `Cargo.toml` `[dev-dependencies]` vs `[dependencies]` segregation verified.
4. **Tier-3 Investigation**: For each suspect, traced production code path it would replace. Verified via `rg findReferences`-equivalent calls.
5. **Tier-4 Deep Dive**: For the top 5 ranked findings, generated ≥3 hypotheses (LSU), Bayesian-updated against evidence, recorded most-plausible explanation. Verified that the test would fail-or-not-fail under each hypothesis.
6. **Cross-checked against doctrine §11**: Cold Read on `context-graph-test-utils` source files BEFORE reading the docstring claims (Linear Sequential Unmasking); the contradiction between claim and content was discovered by examining the code first, then comparing to the description.

---

## Recommendations Summary

| Action | Priority | Files |
|--------|----------|-------|
| **Replace `mejepa infer-test` synthetic AUC with real predictor-output OOD scores from calibration holdout** | P0 | `crates/context-graph-mejepa/src/cli.rs:114-116`, and `bin/mejepa.rs:575` callsite |
| **Rename `generate_real_*` → `generate_random_*` in test-utils; update Cargo.toml description and lib.rs docstring** | P1 | `crates/context-graph-test-utils/{Cargo.toml,src/lib.rs,src/fingerprints.rs}` |
| **Update `teleological_integration.rs` docstring to "ROCKSDB STRUCTURAL ROUND-TRIP TESTS — fingerprint content is fixed placeholder"** | P1 | `crates/context-graph-storage/tests/teleological_integration.rs:1-10` |
| **Replace `StubLayerStatusProvider` with `RealLayerStatusProvider` in task_py_g_063/064 FSV examples** | P2 | `crates/context-graph-mcp/examples/task_py_g_063_project_ingest_fsv.rs:972`, `task_py_g_064_project_report_fsv.rs:762` |
| **Replace sin-wave token generation in maxsim and cache tests with deterministic Gaussian** | P2 | `crates/context-graph-storage/src/teleological/search/maxsim.rs:380-394`, `crates/context-graph-embeddings/src/cache/mod.rs:538-552` |
| **Update banner comment "TEST FIXTURES (REAL DATA, NO MOCKS)" → "TEST DOUBLES — pre-seeded results for index-infrastructure testing only"** | P3 | `crates/context-graph-embeddings/src/storage/multi_space.rs:367` |
| **Add `// Synthetic 32-byte hash; round-trip-only` annotations to `WitnessHash([0x99; 32])` and `[0xAB; 32]` patterns** | P3 | task_py_g_073, task_py_g_042, cf_round_trip_fsv examples |
| **Use `[0x42u8; 32]` (matching no_fake_data_test.rs) for content_hash in `create_test_embeddings`** | P3 | `crates/context-graph-embeddings/src/storage/types/tests.rs:71` |

---

## Verdict Summary

| Aspect | Status |
|--------|--------|
| Q2 Docker oracle path | **CLEAN** — `run_swebench_lite_oracle()` invokes real `python -m swebench.harness.run_evaluation`. No mocked oracle. |
| LLM responses in tests | **CLEAN** — Phase H retired the inner LLM stack; remaining `LLMEdgeValidation` tests explicitly skip LLM calls (storage roundtrip only). |
| Production runtime stub leakage | **DEFENDED** — `Config::validate_phase_safety()` hard-fails in production; all stubs `#[cfg(test)]`-gated; `test-utils` only in `[dev-dependencies]`. |
| Mocked Q2 verdicts in tests | **MOSTLY CLEAN** — `OracleVerdict { … }` literals in tests are INPUT to similarity/storage round-trip kernels, NOT Q2 oracle claims. One borderline: `toolchain_verdict` synthesizes verdicts from category metadata for non-Python fixtures, but the underlying `validate_toolchain` IS real. |
| Hardcoded perfect-separation in production CLI | **GUILTY** — finding H1: `mejepa infer-test` reports `ood_separation_auc` from `vec![0.05;100]` vs `vec![0.95;100]`. |
| `context-graph-test-utils` "NO MOCK DATA" claim | **PARTIAL** — produces real `Vec<f32>` but content is uniform random noise, not real embedder output. Naming overclaims. |
| `tests/fixtures/adversarial/` | **CLEAN** — 30 real adversarial diffs probing real attack classes. |
| AP-007 fake-detection defenses | **STRONG** — fake-pointer, sin-wave, zero-vector, fake-checksum, stub-magic-byte rejection all wired and tested. |

---

*Compiled by Sherlock Holmes, Consulting Detective.*
*"It is a capital mistake to theorize before one has data."*
*Investigation complete: 2026-05-19.*
